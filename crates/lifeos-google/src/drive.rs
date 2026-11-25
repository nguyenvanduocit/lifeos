//! Google Drive: find, read, write and trash the owner's files.

use anyhow::{Result, ensure};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::api::{count, url};
use crate::text::clip;
use crate::{Google, random_token};

const BASE: &str = "https://www.googleapis.com/drive/v3/";
const UPLOAD: &str = "https://www.googleapis.com/upload/drive/v3/";
const FIELDS: &str = "id,name,mimeType,modifiedTime,size,webViewLink,parents,owners(emailAddress)";
const DOC: &str = "application/vnd.google-apps.document";
/// The most of a plain file read as text.
const READ_LIMIT: u64 = 10 * 1024 * 1024;

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DriveSearch {
    /// Words to find in the files' content or names.
    pub query: Option<String>,
    /// Words in the file name only.
    pub name: Option<String>,
    /// A MIME type, e.g. application/vnd.google-apps.document, application/vnd.google-apps.spreadsheet, application/pdf.
    pub mime_type: Option<String>,
    /// Only files in this folder.
    pub folder_id: Option<String>,
    /// 1–100; 25 when not given.
    pub max_results: Option<u32>,
    pub page_token: Option<String>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DriveRead {
    pub file_id: String,
    /// The most characters to return; 50000 when not given.
    pub max_chars: Option<u32>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DriveCreate {
    pub name: String,
    /// The text to put in it.
    #[serde(default)]
    pub content: String,
    /// A Google Doc (the default) or, when false, a plain text file.
    pub google_doc: Option<bool>,
    /// The folder to put it in; My Drive when not given.
    pub folder_id: Option<String>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DriveUpdate {
    pub file_id: String,
    /// A new name.
    pub name: Option<String>,
    /// New text that replaces the whole content.
    pub content: Option<String>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DriveFile {
    pub file_id: String,
}

/// A value inside a Drive query's quotes.
fn quoted(s: &str) -> String {
    format!("'{}'", s.replace('\\', "\\\\").replace('\'', "\\'"))
}

fn file(f: &Value) -> Value {
    json!({
        "id": f.get("id"),
        "name": f.get("name"),
        "mime_type": f.get("mimeType"),
        "modified": f.get("modifiedTime"),
        "size": f.get("size"),
        "link": f.get("webViewLink"),
        "owner": f.pointer("/owners/0/emailAddress"),
    })
}

pub(crate) async fn drive_search(g: &Google, i: DriveSearch) -> Result<Value> {
    let mut terms = vec!["trashed = false".to_owned()];
    if let Some(q) = i.query.as_deref().filter(|q| !q.trim().is_empty()) {
        terms.push(format!("fullText contains {}", quoted(q)));
    }
    if let Some(n) = i.name.as_deref().filter(|n| !n.trim().is_empty()) {
        terms.push(format!("name contains {}", quoted(n)));
    }
    if let Some(m) = &i.mime_type {
        terms.push(format!("mimeType = {}", quoted(m)));
    }
    if let Some(f) = &i.folder_id {
        terms.push(format!("{} in parents", quoted(f)));
    }
    let q = terms.join(" and ");
    let fields = format!("nextPageToken,files({FIELDS})");
    let size = count(i.max_results, 25, 100).to_string();
    let full_text = i.query.as_deref().is_some_and(|q| !q.trim().is_empty());
    let list = g
        .json(|http| {
            let mut params = vec![
                ("q", q.as_str()),
                ("fields", fields.as_str()),
                ("pageSize", size.as_str()),
                ("supportsAllDrives", "true"),
                ("includeItemsFromAllDrives", "true"),
            ];
            // Drive cannot sort a full-text search.
            if !full_text {
                params.push(("orderBy", "modifiedTime desc"));
            }
            if let Some(page) = &i.page_token {
                params.push(("pageToken", page.as_str()));
            }
            http.get(format!("{BASE}files")).query(&params)
        })
        .await?;
    Ok(json!({
        "files": list.get("files").and_then(Value::as_array).into_iter().flatten().map(file).collect::<Vec<_>>(),
        "next_page_token": list.get("nextPageToken"),
    }))
}

async fn metadata(g: &Google, id: &str) -> Result<Value> {
    g.json(|http| http.get(url(BASE, &["files", id])).query(&[("fields", FIELDS), ("supportsAllDrives", "true")]))
        .await
}

pub(crate) async fn drive_read(g: &Google, i: DriveRead) -> Result<Value> {
    let meta = metadata(g, &i.file_id).await?;
    let mime = meta.get("mimeType").and_then(Value::as_str).unwrap_or_default().to_owned();
    let export = match mime.as_str() {
        DOC => Some("text/markdown"),
        "application/vnd.google-apps.spreadsheet" => Some("text/csv"),
        "application/vnd.google-apps.presentation" => Some("text/plain"),
        _ => None,
    };
    let readable = mime.starts_with("text/")
        || matches!(
            mime.as_str(),
            "application/json" | "application/xml" | "application/javascript" | "application/x-yaml"
        );
    let size: u64 = meta.get("size").and_then(Value::as_str).and_then(|s| s.parse().ok()).unwrap_or(0);
    let text = if let Some(format) = export {
        let res = g
            .send(|http| http.get(url(BASE, &["files", &i.file_id, "export"])).query(&[("mimeType", format)]))
            .await?;
        Some(res.text().await?)
    } else if readable && size <= READ_LIMIT {
        let res = g
            .send(|http| {
                http.get(url(BASE, &["files", &i.file_id])).query(&[("alt", "media"), ("supportsAllDrives", "true")])
            })
            .await?;
        Some(res.text().await?)
    } else {
        None
    };
    let mut out = file(&meta);
    match text {
        Some(t) => out["content"] = json!(clip(&t, count(i.max_chars, 50_000, 1_000_000) as usize)),
        None => out["note"] = json!("This file is not text lifeos can read; open its link."),
    }
    Ok(out)
}

/// A Drive multipart upload: the metadata, then the text.
fn multipart(meta: &Value, content: &str) -> Result<(String, String)> {
    let boundary = format!("lifeos-{}", random_token(12)?);
    let body = format!(
        "--{boundary}\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n{meta}\r\n\
         --{boundary}\r\nContent-Type: text/plain; charset=UTF-8\r\n\r\n{content}\r\n--{boundary}--\r\n"
    );
    Ok((format!("multipart/related; boundary={boundary}"), body))
}

pub(crate) async fn drive_create(g: &Google, i: DriveCreate) -> Result<Value> {
    ensure!(!i.name.trim().is_empty(), "give the file a name");
    let mut meta = json!({ "name": i.name.trim() });
    meta["mimeType"] = json!(if i.google_doc.unwrap_or(true) { DOC } else { "text/plain" });
    if let Some(folder) = &i.folder_id {
        meta["parents"] = json!([folder]);
    }
    let (kind, body) = multipart(&meta, &i.content)?;
    let created = g
        .json(|http| {
            http.post(format!("{UPLOAD}files"))
                .query(&[("uploadType", "multipart"), ("fields", FIELDS), ("supportsAllDrives", "true")])
                .header(reqwest::header::CONTENT_TYPE, &kind)
                .body(body.clone())
        })
        .await?;
    Ok(file(&created))
}

pub(crate) async fn drive_update(g: &Google, i: DriveUpdate) -> Result<Value> {
    let mut meta = Map::new();
    if let Some(name) = i.name.as_deref().filter(|n| !n.trim().is_empty()) {
        meta.insert("name".into(), json!(name.trim()));
    }
    let meta = Value::Object(meta);
    let changed = match &i.content {
        Some(content) => {
            let (kind, body) = multipart(&meta, content)?;
            g.json(|http| {
                http.patch(url(UPLOAD, &["files", &i.file_id]))
                    .query(&[("uploadType", "multipart"), ("fields", FIELDS), ("supportsAllDrives", "true")])
                    .header(reqwest::header::CONTENT_TYPE, &kind)
                    .body(body.clone())
            })
            .await?
        }
        None => {
            ensure!(meta.as_object().is_some_and(|m| !m.is_empty()), "say what to change");
            g.json(|http| {
                http.patch(url(BASE, &["files", &i.file_id]))
                    .query(&[("fields", FIELDS), ("supportsAllDrives", "true")])
                    .json(&meta)
            })
            .await?
        }
    };
    Ok(file(&changed))
}

pub(crate) async fn drive_trash(g: &Google, i: DriveFile) -> Result<Value> {
    g.json(|http| {
        http.patch(url(BASE, &["files", &i.file_id]))
            .query(&[("supportsAllDrives", "true")])
            .json(&json!({ "trashed": true }))
    })
    .await?;
    Ok(json!({ "trashed": i.file_id }))
}

#[cfg(test)]
mod tests {
    #[test]
    fn query_values_are_escaped() {
        assert_eq!(super::quoted(r"it's a\b"), r"'it\'s a\\b'");
    }
}
