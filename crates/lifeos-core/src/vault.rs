//! The vault: plain Markdown files with YAML frontmatter.
//!
//! Markdown is the source of truth for everything that is mostly text:
//! tasks, goals, people, journal entries, notes and the agent's reviews. The SQLite index is derived from
//! it and can be rebuilt at any time.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Local, NaiveDate};
use serde::{Deserialize, Serialize};

pub use lifeos_wire::Kind;

/// The vault folder a kind lives in: the folder decides the kind, nothing else.
pub fn folder(kind: Kind) -> &'static str {
    match kind {
        Kind::Task => "tasks",
        Kind::Goal => "goals",
        Kind::Person => "people",
        Kind::Journal => "journal",
        Kind::Note => "notes",
        Kind::Review => "reviews",
        Kind::Crew => "crew",
        Kind::Routine => "routines",
    }
}

/// Frontmatter fields lifeos understands. Anything else the user writes by
/// hand is kept in `extra` and written back untouched.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Frontmatter {
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due: Option<NaiveDate>,
    /// Task: the day it is planned for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scheduled: Option<NaiveDate>,
    /// Task: high | normal | low.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub goal: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub horizon: Option<String>,
    /// Goal: 0–100.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub progress: Option<u8>,
    /// Person: how they relate to the user.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relation: Option<String>,
    /// Person: `YYYY-MM-DD` or `MM-DD`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub birthday: Option<String>,
    /// Person: be in touch at least this often.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contact_every_days: Option<u32>,
    /// Crew member: what they do for the user, e.g. "huấn luyện viên sức khoẻ".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    /// Crew member: Claude Code model alias or id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Crew member: low | medium | high | xhigh | max.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    /// Crew member: the tools it may use; empty means every lifeos tool.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<String>,
    /// Crew member: the connections (outside MCP servers) it may use.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub connections: Vec<String>,
    /// Routine: who does it, `assistant` or a crew member id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    /// Routine: repeats on these five cron fields, local time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cron: Option<String>,
    /// Routine: runs once at `YYYY-MM-DD HH:MM`, local time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<String>,
    /// Routine: runs after any of these lifeos write tools.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub on: Vec<String>,
    /// Routine: runs when its webhook is posted.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hook: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created: Option<DateTime<Local>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed: Option<NaiveDate>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_yaml::Value>,
}

#[derive(Debug, Clone)]
pub struct Doc {
    pub kind: Kind,
    /// File stem; stable handle used by tools (`tasks/<id>.md`).
    pub id: String,
    /// Path relative to the vault root, always with `/` separators.
    pub rel_path: String,
    pub meta: Frontmatter,
    pub body: String,
}

impl Doc {
    pub fn parse(kind: Kind, rel_path: String, text: &str) -> Result<Doc> {
        let id = Path::new(&rel_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string();
        let (meta, body) = match split_frontmatter(text) {
            Some((yaml, body)) => {
                let meta: Frontmatter = serde_yaml::from_str(yaml)
                    .with_context(|| format!("invalid frontmatter in {rel_path}"))?;
                // `render` puts a blank line after the frontmatter; it is not part of the body.
                (meta, body.trim_start_matches(['\r', '\n']).to_string())
            }
            None => (
                Frontmatter {
                    title: id.clone(),
                    ..Default::default()
                },
                text.to_string(),
            ),
        };
        Ok(Doc {
            kind,
            id,
            rel_path,
            meta,
            body,
        })
    }

    pub fn render(&self) -> Result<String> {
        let yaml = serde_yaml::to_string(&self.meta)?;
        Ok(format!("---\n{yaml}---\n\n{}", self.body.trim_start_matches('\n')))
    }
}

fn split_frontmatter(text: &str) -> Option<(&str, &str)> {
    let rest = text
        .strip_prefix("---\n")
        .or_else(|| text.strip_prefix("---\r\n"))?;
    let end = rest.find("\n---")?;
    let yaml = &rest[..end + 1];
    let after = &rest[end + 4..];
    let body = after
        .strip_prefix("\r\n")
        .or_else(|| after.strip_prefix('\n'))
        .unwrap_or(after);
    Some((yaml, body))
}

#[derive(Debug, Clone)]
pub struct Vault {
    root: PathBuf,
}

impl Vault {
    pub fn new(root: impl Into<PathBuf>) -> Vault {
        Vault { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn ensure_layout(&self) -> Result<()> {
        for kind in Kind::ALL {
            fs::create_dir_all(self.root.join(folder(kind)))?;
        }
        Ok(())
    }

    pub fn abs(&self, rel_path: &str) -> PathBuf {
        self.root.join(rel_path)
    }

    pub fn read(&self, kind: Kind, rel_path: &str) -> Result<Doc> {
        let text = fs::read_to_string(self.abs(rel_path))
            .with_context(|| format!("cannot read {rel_path}"))?;
        Doc::parse(kind, rel_path.to_string(), &text)
    }

    pub fn write(&self, doc: &Doc) -> Result<()> {
        let path = self.abs(&doc.rel_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, doc.render()?).with_context(|| format!("cannot write {}", doc.rel_path))
    }

    /// Picks `<folder>/<slug>.md`, adding `-2`, `-3`… until the name is free.
    pub fn new_rel_path(&self, kind: Kind, title: &str) -> Result<String> {
        let base = slugify(title);
        if base.is_empty() {
            bail!("title must contain at least one letter or digit");
        }
        let mut candidate = format!("{}/{base}.md", folder(kind));
        let mut n = 2;
        while self.abs(&candidate).exists() {
            candidate = format!("{}/{base}-{n}.md", folder(kind));
            n += 1;
        }
        Ok(candidate)
    }

    pub fn journal_rel_path(day: NaiveDate) -> String {
        format!("journal/{}/{}.md", day.format("%Y"), day.format("%Y-%m-%d"))
    }

    /// Every Markdown file under a known folder, with its kind.
    pub fn walk(&self) -> impl Iterator<Item = (Kind, String, PathBuf)> + '_ {
        Kind::ALL.into_iter().flat_map(move |kind| {
            walkdir::WalkDir::new(self.root.join(folder(kind)))
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
                .filter(|e| e.path().extension().is_some_and(|x| x == "md"))
                .filter_map(move |e| {
                    let rel = e.path().strip_prefix(&self.root).ok()?;
                    let rel = rel.to_str()?.replace('\\', "/");
                    Some((kind, rel, e.path().to_path_buf()))
                })
        })
    }
}

/// ASCII slug that keeps Vietnamese readable: "Học Rust" -> "hoc-rust".
pub fn slugify(title: &str) -> String {
    let ascii = deunicode::deunicode(title).to_lowercase();
    let mut out = String::new();
    for c in ascii.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let out = out.trim_end_matches('-');
    out.chars().take(60).collect::<String>().trim_end_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_strips_vietnamese_diacritics() {
        assert_eq!(slugify("Học Rust thật giỏi!"), "hoc-rust-that-gioi");
        assert_eq!(slugify("Đi chợ"), "di-cho");
        assert_eq!(slugify("  ?? "), "");
    }

    #[test]
    fn frontmatter_round_trip_keeps_unknown_fields() {
        let text = "---\ntitle: Mua sách\nstatus: open\nmood: 7\n---\n\nghi chú\n";
        let doc = Doc::parse(Kind::Task, "tasks/mua-sach.md".into(), text).unwrap();
        assert_eq!(doc.id, "mua-sach");
        assert_eq!(doc.meta.title, "Mua sách");
        assert_eq!(doc.body, "ghi chú\n");
        let again = Doc::parse(Kind::Task, doc.rel_path.clone(), &doc.render().unwrap()).unwrap();
        assert_eq!(again.meta.extra.get("mood"), doc.meta.extra.get("mood"));
        assert_eq!(again.body, doc.body);
    }

    #[test]
    fn file_without_frontmatter_uses_stem_as_title() {
        let doc = Doc::parse(Kind::Note, "notes/y-tuong.md".into(), "# Ý tưởng").unwrap();
        assert_eq!(doc.meta.title, "y-tuong");
        assert_eq!(doc.body, "# Ý tưởng");
    }
}
