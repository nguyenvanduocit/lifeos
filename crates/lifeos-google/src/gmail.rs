//! Gmail: search, read, send, draft, label and trash the owner's mail.

use anyhow::{Context, Result, ensure};
use base64::Engine as _;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use futures::future::try_join_all;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::api::{count, url};
use crate::text::{clip, html_to_text};
use crate::tools::Nothing;
use crate::Google;

const BASE: &str = "https://gmail.googleapis.com/gmail/v1/users/me/";
const SUMMARY_HEADERS: &[&str] = &["From", "To", "Subject", "Date"];

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GmailSearch {
    /// Gmail search syntax, as typed in Gmail's search box. Empty lists the newest mail.
    #[serde(default)]
    pub query: String,
    /// 1–50; 20 when not given.
    pub max_results: Option<u32>,
    /// From the previous answer's next_page_token, for more.
    pub page_token: Option<String>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GmailRead {
    /// The message id.
    pub id: String,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GmailThread {
    pub thread_id: String,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GmailCompose {
    /// Addresses, `name@example.com` or `Name <name@example.com>`.
    #[serde(default)]
    pub to: Vec<String>,
    #[serde(default)]
    pub cc: Vec<String>,
    #[serde(default)]
    pub bcc: Vec<String>,
    /// When answering, "Re: " and the original subject if not given.
    pub subject: Option<String>,
    /// Plain text.
    pub body: String,
    /// The id of the message this answers.
    pub reply_to: Option<String>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GmailModify {
    /// Message ids.
    pub ids: Vec<String>,
    /// Label names or ids to add.
    #[serde(default)]
    pub add: Vec<String>,
    /// Label names or ids to remove.
    #[serde(default)]
    pub remove: Vec<String>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GmailIds {
    /// Message ids.
    pub ids: Vec<String>,
}

pub(crate) async fn gmail_search(g: &Google, i: GmailSearch) -> Result<Value> {
    let max = count(i.max_results, 20, 50).to_string();
    let list = g
        .json(|http| {
            let mut q = vec![("maxResults", max.as_str())];
            if !i.query.trim().is_empty() {
                q.push(("q", i.query.as_str()));
            }
            if let Some(page) = &i.page_token {
                q.push(("pageToken", page.as_str()));
            }
            http.get(format!("{BASE}messages")).query(&q)
        })
        .await?;
    let ids: Vec<String> = list
        .get("messages")
        .and_then(Value::as_array)
        .map(|m| m.iter().filter_map(|m| m.get("id")?.as_str().map(String::from)).collect())
        .unwrap_or_default();
    let messages = try_join_all(ids.iter().map(|id| metadata(g, id))).await?;
    Ok(json!({
        "messages": messages.iter().map(summary).collect::<Vec<_>>(),
        "next_page_token": list.get("nextPageToken"),
    }))
}

async fn metadata(g: &Google, id: &str) -> Result<Value> {
    g.json(|http| {
        let mut q = vec![("format", "metadata")];
        q.extend(SUMMARY_HEADERS.iter().map(|h| ("metadataHeaders", *h)));
        q.extend(["Message-ID", "References", "Reply-To", "Cc"].map(|h| ("metadataHeaders", h)));
        http.get(url(BASE, &["messages", id])).query(&q)
    })
    .await
}

fn header<'a>(message: &'a Value, name: &str) -> Option<&'a str> {
    message
        .pointer("/payload/headers")?
        .as_array()?
        .iter()
        .find(|h| h.get("name").and_then(Value::as_str).is_some_and(|n| n.eq_ignore_ascii_case(name)))?
        .get("value")?
        .as_str()
}

fn labels(message: &Value) -> Vec<&str> {
    message
        .get("labelIds")
        .and_then(Value::as_array)
        .map(|l| l.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default()
}

fn summary(message: &Value) -> Value {
    let labels = labels(message);
    json!({
        "id": message.get("id"),
        "thread_id": message.get("threadId"),
        "from": header(message, "From"),
        "to": header(message, "To"),
        "subject": header(message, "Subject"),
        "date": header(message, "Date"),
        "snippet": message.get("snippet").and_then(Value::as_str).map(decode_snippet),
        "unread": labels.contains(&"UNREAD"),
        "labels": labels,
    })
}

/// Gmail's snippets carry HTML entities.
fn decode_snippet(s: &str) -> String {
    html_to_text(s)
}

fn full(message: &Value, most: usize) -> Value {
    let (mut plain, mut html, mut files) = (None, None, Vec::new());
    if let Some(payload) = message.get("payload") {
        walk(payload, &mut plain, &mut html, &mut files);
    }
    let body = plain.or_else(|| html.map(|h| html_to_text(&h))).unwrap_or_default();
    let mut out = summary(message);
    out["cc"] = json!(header(message, "Cc"));
    out["body"] = json!(clip(body.trim(), most));
    out["attachments"] = json!(files);
    out
}

fn walk(part: &Value, plain: &mut Option<String>, html: &mut Option<String>, files: &mut Vec<Value>) {
    let mime = part.get("mimeType").and_then(Value::as_str).unwrap_or_default();
    let filename = part.get("filename").and_then(Value::as_str).unwrap_or_default();
    if !filename.is_empty() {
        files.push(json!({
            "filename": filename,
            "mime_type": mime,
            "size": part.pointer("/body/size"),
        }));
    } else if let Some(data) = part.pointer("/body/data").and_then(Value::as_str) {
        let text = decode_body(data);
        match mime {
            "text/plain" if plain.is_none() => *plain = Some(text),
            "text/html" if html.is_none() => *html = Some(text),
            _ => {}
        }
    }
    for child in part.get("parts").and_then(Value::as_array).into_iter().flatten() {
        walk(child, plain, html, files);
    }
}

fn decode_body(data: &str) -> String {
    let bytes = URL_SAFE_NO_PAD.decode(data.trim_end_matches('=')).unwrap_or_default();
    String::from_utf8_lossy(&bytes).into_owned()
}

pub(crate) async fn gmail_read(g: &Google, i: GmailRead) -> Result<Value> {
    let message = g.json(|http| http.get(url(BASE, &["messages", &i.id])).query(&[("format", "full")])).await?;
    Ok(full(&message, 50_000))
}

pub(crate) async fn gmail_thread(g: &Google, i: GmailThread) -> Result<Value> {
    let thread =
        g.json(|http| http.get(url(BASE, &["threads", &i.thread_id])).query(&[("format", "full")])).await?;
    let messages: Vec<Value> =
        thread.get("messages").and_then(Value::as_array).into_iter().flatten().map(|m| full(m, 15_000)).collect();
    Ok(json!({ "thread_id": i.thread_id, "messages": messages }))
}

pub(crate) async fn gmail_labels(g: &Google, _: Nothing) -> Result<Value> {
    let list = g.json(|http| http.get(format!("{BASE}labels"))).await?;
    Ok(json!(
        list.get("labels")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(|l| json!({ "id": l.get("id"), "name": l.get("name"), "type": l.get("type") }))
            .collect::<Vec<_>>()
    ))
}

/// The message to send and the conversation it goes in.
async fn compose(g: &Google, i: GmailCompose) -> Result<(String, Option<String>)> {
    let GmailCompose { mut to, cc, bcc, subject, body, reply_to } = i;
    let mut subject = subject.unwrap_or_default();
    let (mut thread, mut in_reply_to, mut references) = (None, None, None);
    if let Some(id) = reply_to {
        let original = metadata(g, &id).await?;
        thread = original.get("threadId").and_then(Value::as_str).map(String::from);
        if to.is_empty() {
            to.extend(header(&original, "Reply-To").or(header(&original, "From")).map(String::from));
        }
        if subject.trim().is_empty() {
            let old = header(&original, "Subject").unwrap_or_default();
            subject = if old.to_ascii_lowercase().starts_with("re:") { old.to_owned() } else { format!("Re: {old}") };
        }
        if let Some(message_id) = header(&original, "Message-ID") {
            in_reply_to = Some(message_id.to_owned());
            references = Some(match header(&original, "References") {
                Some(r) => format!("{r} {message_id}"),
                None => message_id.to_owned(),
            });
        }
    }
    ensure!(!to.is_empty() || !cc.is_empty() || !bcc.is_empty(), "say who the email goes to");
    let raw = mime(&Mime {
        to: &to,
        cc: &cc,
        bcc: &bcc,
        subject: &subject,
        body: &body,
        in_reply_to: in_reply_to.as_deref(),
        references: references.as_deref(),
    })?;
    Ok((URL_SAFE_NO_PAD.encode(raw), thread))
}

pub(crate) async fn gmail_send(g: &Google, i: GmailCompose) -> Result<Value> {
    let (raw, thread) = compose(g, i).await?;
    let sent = g
        .json(|http| http.post(format!("{BASE}messages/send")).json(&json!({ "raw": raw, "threadId": thread })))
        .await?;
    Ok(json!({ "id": sent.get("id"), "thread_id": sent.get("threadId") }))
}

pub(crate) async fn gmail_draft(g: &Google, i: GmailCompose) -> Result<Value> {
    let (raw, thread) = compose(g, i).await?;
    let draft = g
        .json(|http| {
            http.post(format!("{BASE}drafts")).json(&json!({ "message": { "raw": raw, "threadId": thread } }))
        })
        .await?;
    Ok(json!({ "draft_id": draft.get("id"), "message_id": draft.pointer("/message/id") }))
}

pub(crate) async fn gmail_modify(g: &Google, i: GmailModify) -> Result<Value> {
    ensure!(!i.ids.is_empty(), "give the message ids");
    ensure!(!i.add.is_empty() || !i.remove.is_empty(), "give labels to add or remove");
    let list = g.json(|http| http.get(format!("{BASE}labels"))).await?;
    let known: Vec<(String, String)> = list
        .get("labels")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|l| Some((l.get("id")?.as_str()?.to_owned(), l.get("name")?.as_str()?.to_owned())))
        .collect();
    let resolve = |names: &[String]| -> Result<Vec<String>> {
        names
            .iter()
            .map(|n| {
                known
                    .iter()
                    .find(|(id, name)| id == n || name.eq_ignore_ascii_case(n))
                    .map(|(id, _)| id.clone())
                    .with_context(|| format!("no Gmail label {n:?}"))
            })
            .collect()
    };
    let (add, remove) = (resolve(&i.add)?, resolve(&i.remove)?);
    g.json(|http| {
        http.post(format!("{BASE}messages/batchModify"))
            .json(&json!({ "ids": i.ids, "addLabelIds": add, "removeLabelIds": remove }))
    })
    .await?;
    Ok(json!({ "modified": i.ids.len() }))
}

pub(crate) async fn gmail_trash(g: &Google, i: GmailIds) -> Result<Value> {
    ensure!(!i.ids.is_empty(), "give the message ids");
    for id in &i.ids {
        g.json(|http| http.post(url(BASE, &["messages", id, "trash"]))).await?;
    }
    Ok(json!({ "trashed": i.ids.len() }))
}

struct Mime<'a> {
    to: &'a [String],
    cc: &'a [String],
    bcc: &'a [String],
    subject: &'a str,
    body: &'a str,
    in_reply_to: Option<&'a str>,
    references: Option<&'a str>,
}

/// An RFC 5322 message: UTF-8 text, base64 in transfer, the subject encoded when it is not ASCII.
fn mime(m: &Mime) -> Result<String> {
    let mut headers: Vec<(&str, String)> = Vec::new();
    for (name, list) in [("To", m.to), ("Cc", m.cc), ("Bcc", m.bcc)] {
        if !list.is_empty() {
            headers.push((name, list.iter().map(|a| a.trim()).collect::<Vec<_>>().join(", ")));
        }
    }
    headers.push(("Subject", encode_word(m.subject.trim())));
    if let Some(r) = m.in_reply_to {
        headers.push(("In-Reply-To", r.to_owned()));
    }
    if let Some(r) = m.references {
        headers.push(("References", r.to_owned()));
    }
    headers.push(("MIME-Version", "1.0".into()));
    headers.push(("Content-Type", "text/plain; charset=\"UTF-8\"".into()));
    headers.push(("Content-Transfer-Encoding", "base64".into()));
    let mut out = String::new();
    for (name, value) in headers {
        ensure!(!value.contains(['\r', '\n']), "{name} cannot span lines");
        out.push_str(name);
        out.push_str(": ");
        out.push_str(&value);
        out.push_str("\r\n");
    }
    out.push_str("\r\n");
    let body = m.body.replace("\r\n", "\n").replace('\n', "\r\n");
    let encoded = STANDARD.encode(body.as_bytes());
    for chunk in encoded.as_bytes().chunks(76) {
        out.push_str(std::str::from_utf8(chunk).expect("base64 is ASCII"));
        out.push_str("\r\n");
    }
    Ok(out)
}

/// RFC 2047 encoded words for a header that is not ASCII, each under the 75-character limit.
fn encode_word(text: &str) -> String {
    if text.is_ascii() {
        return text.to_owned();
    }
    let mut words = Vec::new();
    let mut chunk = String::new();
    for c in text.chars() {
        if chunk.len() + c.len_utf8() > 45 {
            words.push(format!("=?UTF-8?B?{}?=", STANDARD.encode(&chunk)));
            chunk.clear();
        }
        chunk.push(c);
    }
    if !chunk.is_empty() {
        words.push(format!("=?UTF-8?B?{}?=", STANDARD.encode(&chunk)));
    }
    words.join("\r\n ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_message_is_utf8_base64_and_refuses_header_injection() {
        let to = vec!["Anna <anna@example.com>".to_owned()];
        let raw = mime(&Mime {
            to: &to,
            cc: &[],
            bcc: &[],
            subject: "Chào bạn",
            body: "Dòng 1\nDòng 2",
            in_reply_to: Some("<a@b>"),
            references: Some("<a@b>"),
        })
        .unwrap();
        assert!(raw.starts_with("To: Anna <anna@example.com>\r\nSubject: =?UTF-8?B?"));
        assert!(raw.contains("In-Reply-To: <a@b>\r\n"));
        let body = raw.split("\r\n\r\n").nth(1).unwrap().replace("\r\n", "");
        assert_eq!(String::from_utf8(STANDARD.decode(body).unwrap()).unwrap(), "Dòng 1\r\nDòng 2");
        let evil = vec!["a@b.c\r\nBcc: x@y.z".to_owned()];
        assert!(
            mime(&Mime {
                to: &evil,
                cc: &[],
                bcc: &[],
                subject: "",
                body: "",
                in_reply_to: None,
                references: None
            })
            .is_err()
        );
    }

    #[test]
    fn long_subjects_become_several_short_words() {
        let subject = "Ừ ".repeat(40);
        let encoded = encode_word(&subject);
        assert!(encoded.split("\r\n ").all(|w| w.len() <= 75 && w.starts_with("=?UTF-8?B?")));
    }

    #[test]
    fn the_plain_part_is_preferred_and_attachments_are_named() {
        let data = |s: &str| URL_SAFE_NO_PAD.encode(s);
        let message = json!({
            "id": "m1", "threadId": "t1", "labelIds": ["INBOX", "UNREAD"],
            "payload": {
                "mimeType": "multipart/mixed",
                "headers": [{"name": "From", "value": "Anna <anna@example.com>"}, {"name": "Subject", "value": "Hi"}],
                "parts": [
                    {"mimeType": "multipart/alternative", "parts": [
                        {"mimeType": "text/html", "body": {"data": data("<p>Hello <b>there</b></p>")}},
                        {"mimeType": "text/plain", "body": {"data": data("Hello there")}}
                    ]},
                    {"mimeType": "application/pdf", "filename": "plan.pdf", "body": {"attachmentId": "x", "size": 10}}
                ]
            }
        });
        let read = full(&message, 1000);
        assert_eq!(read["body"], "Hello there");
        assert_eq!(read["unread"], true);
        assert_eq!(read["from"], "Anna <anna@example.com>");
        assert_eq!(read["attachments"][0]["filename"], "plan.pdf");
    }
}
