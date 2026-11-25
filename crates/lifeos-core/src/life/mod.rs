//! `Life`: every operation lifeos can do, over the vault and the database.
//! The agent, MCP, the web app and Telegram all go through here (via `tools`).
//!
//! One module per area of a life; this one holds what they share: opening,
//! the derived document index, and reading and writing documents.

pub mod chats;
pub mod crew;
pub mod dashboard;
pub mod goals;
pub mod habits;
pub mod inbox;
pub mod money;
pub mod notes;
pub mod people;
pub mod memory;
pub mod tasks;
pub mod health;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use anyhow::{Context, Result, bail};
use chrono::{Local, NaiveDate};
use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::Serialize;

use crate::db;
use crate::vault::{Doc, Frontmatter, Vault};
pub use lifeos_wire::*;

pub struct Life {
    vault: Vault,
    conn: Connection,
}

#[derive(Debug, Default, Clone, Copy, Serialize)]
pub struct SyncStats {
    pub indexed: usize,
    pub removed: usize,
    pub unchanged: usize,
}

pub fn today() -> NaiveDate {
    Local::now().date_naive()
}

/// Columns `doc_summary` reads, in order.
const DOC_COLUMNS: &str =
    "id, kind, title, status, due, scheduled, priority, goal, horizon, progress, tags, path";

fn doc_summary(r: &Row<'_>) -> rusqlite::Result<DocSummary> {
    let kind: String = r.get(1)?;
    let tags: String = r.get(10)?;
    Ok(DocSummary {
        id: r.get(0)?,
        kind: Kind::parse(&kind).unwrap_or(Kind::Note),
        title: r.get(2)?,
        status: r.get(3)?,
        due: r.get(4)?,
        scheduled: r.get(5)?,
        priority: r.get(6)?,
        goal: r.get(7)?,
        horizon: r.get(8)?,
        progress: r.get(9)?,
        tags: tags.split(',').filter(|t| !t.is_empty()).map(str::to_owned).collect(),
        path: r.get(11)?,
    })
}

fn summary(doc: &Doc) -> DocSummary {
    let m = &doc.meta;
    DocSummary {
        id: doc.id.clone(),
        kind: doc.kind,
        title: m.title.clone(),
        status: m.status.clone(),
        due: m.due.map(|d| d.to_string()),
        scheduled: m.scheduled.map(|d| d.to_string()),
        priority: m.priority.clone(),
        goal: m.goal.clone(),
        horizon: m.horizon.clone(),
        progress: m.progress,
        tags: m.tags.clone(),
        path: doc.rel_path.clone(),
    }
}

fn full(doc: Doc) -> DocFull {
    DocFull {
        summary: summary(&doc),
        body: doc.body,
    }
}

fn mtime_ns(path: &Path) -> i64 {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_nanos() as i64)
        .unwrap_or(0)
}

/// Lowercase and trimmed: the one spelling of a habit, category or metric name.
fn key(name: &str) -> String {
    name.trim().to_lowercase()
}

impl Life {
    /// Opens (and creates if needed) a lifeos home, then syncs the index.
    pub fn open(home: impl Into<PathBuf>) -> Result<Life> {
        let home = home.into();
        let vault = Vault::new(&home);
        vault.ensure_layout()?;
        let conn = db::open(&home.join(".lifeos").join("lifeos.db"))?;
        let mut life = Life { vault, conn };
        life.sync_index()?;
        Ok(life)
    }

    /// For tests: vault on disk, database in memory.
    pub fn open_with_memory_db(home: impl Into<PathBuf>) -> Result<Life> {
        let vault = Vault::new(home.into());
        vault.ensure_layout()?;
        let mut life = Life {
            vault,
            conn: db::open_in_memory()?,
        };
        life.sync_index()?;
        Ok(life)
    }

    pub fn home(&self) -> &Path {
        self.vault.root()
    }

    // ---------------------------------------------------------------- index

    /// Brings `docs`/`docs_fts` in line with the files on disk. Files edited
    /// by hand (Obsidian, an editor, Claude's own Edit tool) are picked up here.
    pub fn sync_index(&mut self) -> Result<SyncStats> {
        let mut stats = SyncStats::default();
        let known: BTreeMap<String, i64> = {
            let mut stmt = self.conn.prepare("SELECT path, mtime FROM docs")?;
            stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<rusqlite::Result<_>>()?
        };
        let mut seen = std::collections::HashSet::new();
        let files: Vec<_> = self.vault.walk().collect();
        for (kind, rel, abs) in files {
            let mtime = mtime_ns(&abs);
            seen.insert(rel.clone());
            if known.get(&rel) == Some(&mtime) {
                stats.unchanged += 1;
                continue;
            }
            match self.vault.read(kind, &rel) {
                Ok(doc) => {
                    self.index_doc(&doc, mtime)?;
                    stats.indexed += 1;
                }
                Err(err) => tracing::warn!("skipping {rel}: {err:#}"),
            }
        }
        for path in known.keys().filter(|p| !seen.contains(*p)) {
            self.unindex(path)?;
            stats.removed += 1;
        }
        Ok(stats)
    }

    /// Drops the derived index and rebuilds it from the vault.
    pub fn reindex(&mut self) -> Result<SyncStats> {
        self.conn
            .execute_batch("DELETE FROM docs; DELETE FROM docs_fts;")?;
        self.sync_index()
    }

    fn index_doc(&self, doc: &Doc, mtime: i64) -> Result<()> {
        let m = &doc.meta;
        self.conn.execute(
            "INSERT INTO docs(path, kind, id, title, status, due, scheduled, priority, goal, horizon,
                              progress, relation, birthday, contact_every, tags, mtime, completed)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
             ON CONFLICT(path) DO UPDATE SET kind=?2, id=?3, title=?4, status=?5, due=?6,
                 scheduled=?7, priority=?8, goal=?9, horizon=?10, progress=?11, relation=?12,
                 birthday=?13, contact_every=?14, tags=?15, mtime=?16, completed=?17",
            params![
                doc.rel_path,
                doc.kind.as_str(),
                doc.id,
                m.title,
                m.status,
                m.due.map(|d| d.to_string()),
                m.scheduled.map(|d| d.to_string()),
                m.priority,
                m.goal,
                m.horizon,
                m.progress,
                m.relation,
                m.birthday,
                m.contact_every_days,
                m.tags.join(","),
                mtime,
                m.completed.map(|d| d.to_string())
            ],
        )?;
        self.conn
            .execute("DELETE FROM docs_fts WHERE path = ?1", [&doc.rel_path])?;
        self.conn.execute(
            "INSERT INTO docs_fts(path, title, body) VALUES (?1, ?2, ?3)",
            params![doc.rel_path, m.title, doc.body],
        )?;
        Ok(())
    }

    fn unindex(&self, path: &str) -> Result<()> {
        self.conn.execute("DELETE FROM docs WHERE path = ?1", [path])?;
        self.conn
            .execute("DELETE FROM docs_fts WHERE path = ?1", [path])?;
        Ok(())
    }

    // ------------------------------------------------------------ documents

    fn save(&self, doc: &Doc) -> Result<()> {
        self.vault.write(doc)?;
        self.index_doc(doc, mtime_ns(&self.vault.abs(&doc.rel_path)))
    }

    fn create(&self, kind: Kind, meta: Frontmatter, body: String) -> Result<Doc> {
        let rel_path = self.vault.new_rel_path(kind, &meta.title)?;
        self.create_at(kind, rel_path, meta, body)
    }

    fn create_at(&self, kind: Kind, rel_path: String, meta: Frontmatter, body: String) -> Result<Doc> {
        let id = Path::new(&rel_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string();
        let doc = Doc {
            kind,
            id,
            rel_path,
            meta,
            body,
        };
        self.save(&doc)?;
        Ok(doc)
    }

    fn find(&self, kind: Kind, id: &str) -> Result<Doc> {
        let path: Option<String> = self
            .conn
            .query_row(
                "SELECT path FROM docs WHERE kind = ?1 AND id = ?2",
                params![kind.as_str(), id],
                |r| r.get(0),
            )
            .optional()?;
        let Some(path) = path else {
            bail!("no {} with id '{id}'", kind.as_str());
        };
        self.vault.read(kind, &path)
    }

    /// Documents of a kind matching an SQL condition over `docs` (with `?1` = kind).
    fn docs_where(&self, kind: Kind, condition: &str, order: &str) -> Result<Vec<DocSummary>> {
        let sql = format!("SELECT {DOC_COLUMNS} FROM docs WHERE kind = ?1 AND ({condition}) ORDER BY {order}");
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([kind.as_str()], doc_summary)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Reads any document by its id (file stem) or vault-relative path.
    pub fn read_doc(&self, id_or_path: &str) -> Result<DocFull> {
        let found: Option<(String, String)> = self
            .conn
            .query_row(
                "SELECT kind, path FROM docs WHERE path = ?1 OR id = ?1 ORDER BY path = ?1 DESC LIMIT 1",
                [id_or_path],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let Some((kind, path)) = found else {
            bail!("no document '{id_or_path}'");
        };
        let kind = Kind::parse(&kind).context("unknown kind in index")?;
        Ok(full(self.vault.read(kind, &path)?))
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchHit>> {
        // Quote every term so user text can't break FTS5 syntax.
        let fts_query = query
            .split_whitespace()
            .map(|t| format!("\"{}\"", t.replace('"', "")))
            .collect::<Vec<_>>()
            .join(" ");
        if fts_query.is_empty() {
            return Ok(vec![]);
        }
        let mut stmt = self.conn.prepare(
            "SELECT d.id, d.kind, d.title, d.path,
                    snippet(docs_fts, 2, '«', '»', '…', 16)
             FROM docs_fts JOIN docs d ON d.path = docs_fts.path
             WHERE docs_fts MATCH ?1
             ORDER BY rank LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![fts_query, limit as i64], |r| {
            let kind: String = r.get(1)?;
            Ok(SearchHit {
                id: r.get(0)?,
                kind: Kind::parse(&kind).unwrap_or(Kind::Note),
                title: r.get(2)?,
                path: r.get(3)?,
                snippet: r.get(4)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Changes whenever another connection (the MCP server, the Telegram
    /// bot, another `lifeos`) commits to the database.
    pub fn data_version(&self) -> Result<i64> {
        Ok(self.conn.query_row("PRAGMA data_version", [], |r| r.get(0))?)
    }

    // ------------------------------------------------------------------- kv

    pub fn kv_get(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row("SELECT value FROM kv WHERE key = ?1", [key], |r| r.get(0))
            .optional()?)
    }

    pub fn kv_set(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO kv(key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = ?2",
            params![key, value],
        )?;
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::Life;

    pub fn life() -> (tempfile::TempDir, Life) {
        let dir = tempfile::tempdir().unwrap();
        let life = Life::open_with_memory_db(dir.path()).unwrap();
        (dir, life)
    }

    pub fn d(s: &str) -> chrono::NaiveDate {
        s.parse().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::*;
    use std::fs;

    #[test]
    fn hand_edits_are_picked_up_by_sync() {
        let (dir, mut life) = life();
        fs::write(
            dir.path().join("notes/tay.md"),
            "---\ntitle: Viết tay\ntags: [cafe]\n---\nnội dung về cà phê\n",
        )
        .unwrap();
        let stats = life.sync_index().unwrap();
        assert_eq!(stats.indexed, 1);
        // diacritics-insensitive search
        let hits = life.search("ca phe", 5).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "tay");
        assert_eq!(life.read_doc("tay").unwrap().summary.tags, vec!["cafe"]);

        fs::remove_file(dir.path().join("notes/tay.md")).unwrap();
        assert_eq!(life.sync_index().unwrap().removed, 1);
        assert!(life.search("ca phe", 5).unwrap().is_empty());
    }
}
