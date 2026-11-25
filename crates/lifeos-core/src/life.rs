//! `Life`: every operation lifeos can do, over the vault and the database.
//! Agents, MCP, web and Telegram all go through here (via `tools`).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use anyhow::{Context, Result, bail};
use chrono::{Duration, Local, NaiveDate};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

use crate::db;
use crate::vault::{Doc, Frontmatter, Kind, Vault};
pub use lifeos_wire::{
    CategoryAmount, CurrencySummary, DocFull, DocSummary, HabitStatus, Metric, SearchHit, Today,
    Transaction,
};

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

/// Digits after the decimal point for a currency's minor unit.
fn minor_exponent(currency: &str) -> u32 {
    match currency {
        "VND" | "JPY" | "KRW" | "IDR" => 0,
        _ => 2,
    }
}

fn to_minor(amount: f64, currency: &str) -> i64 {
    (amount * 10f64.powi(minor_exponent(currency) as i32)).round() as i64
}

fn from_minor(minor: i64, currency: &str) -> f64 {
    minor as f64 / 10f64.powi(minor_exponent(currency) as i32)
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
        let mut seen = Vec::new();
        let files: Vec<_> = self.vault.walk().collect();
        for (kind, rel, abs) in files {
            let mtime = mtime_ns(&abs);
            seen.push(rel.clone());
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
        for path in known.keys().filter(|p| !seen.contains(p)) {
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
            "INSERT INTO docs(path, kind, id, title, status, due, goal, horizon, tags, mtime)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(path) DO UPDATE SET kind=?2, id=?3, title=?4, status=?5, due=?6,
                 goal=?7, horizon=?8, tags=?9, mtime=?10",
            params![
                doc.rel_path,
                doc.kind.as_str(),
                doc.id,
                m.title,
                m.status,
                m.due.map(|d| d.to_string()),
                m.goal,
                m.horizon,
                m.tags.join(","),
                mtime
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

    fn save(&self, doc: &Doc) -> Result<()> {
        self.vault.write(doc)?;
        self.index_doc(doc, mtime_ns(&self.vault.abs(&doc.rel_path)))
    }

    fn create(&self, kind: Kind, meta: Frontmatter, body: String) -> Result<Doc> {
        let rel_path = self.vault.new_rel_path(kind, &meta.title)?;
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

    fn list(&self, kind: Kind, status: Option<&str>) -> Result<Vec<DocSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, status, due, goal, horizon, path FROM docs
             WHERE kind = ?1 AND (?2 IS NULL OR status = ?2)
             ORDER BY due IS NULL, due, title",
        )?;
        let rows = stmt.query_map(params![kind.as_str(), status], |r| {
            Ok(DocSummary {
                kind,
                id: r.get(0)?,
                title: r.get(1)?,
                status: r.get(2)?,
                due: r.get(3)?,
                goal: r.get(4)?,
                horizon: r.get(5)?,
                path: r.get(6)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    // ---------------------------------------------------------------- tasks

    pub fn add_task(
        &self,
        title: &str,
        due: Option<NaiveDate>,
        goal: Option<String>,
        tags: Vec<String>,
        notes: Option<String>,
    ) -> Result<DocSummary> {
        if let Some(goal) = &goal {
            self.find(Kind::Goal, goal)
                .with_context(|| format!("goal '{goal}' does not exist; list_goals shows ids"))?;
        }
        let meta = Frontmatter {
            title: title.trim().to_string(),
            status: Some("open".into()),
            due,
            goal,
            tags,
            created: Some(Local::now()),
            ..Default::default()
        };
        Ok(summary(&self.create(Kind::Task, meta, notes.unwrap_or_default())?))
    }

    /// `status`: "open", "done" or `None` for all.
    pub fn list_tasks(&self, status: Option<&str>) -> Result<Vec<DocSummary>> {
        self.list(Kind::Task, status)
    }

    pub fn complete_task(&self, id: &str) -> Result<DocSummary> {
        let mut doc = self.find(Kind::Task, id)?;
        doc.meta.status = Some("done".into());
        doc.meta.completed = Some(today());
        self.save(&doc)?;
        Ok(summary(&doc))
    }

    // ---------------------------------------------------------------- goals

    pub fn add_goal(
        &self,
        title: &str,
        horizon: Option<String>,
        due: Option<NaiveDate>,
        why: Option<String>,
    ) -> Result<DocSummary> {
        let meta = Frontmatter {
            title: title.trim().to_string(),
            status: Some("active".into()),
            horizon,
            due,
            created: Some(Local::now()),
            ..Default::default()
        };
        let body = match why {
            Some(why) => format!("## Vì sao\n\n{why}\n\n## Tiến độ\n\n"),
            None => "## Vì sao\n\n## Tiến độ\n\n".to_string(),
        };
        Ok(summary(&self.create(Kind::Goal, meta, body)?))
    }

    pub fn list_goals(&self, status: Option<&str>) -> Result<Vec<DocSummary>> {
        self.list(Kind::Goal, status)
    }

    pub fn set_goal_status(&self, id: &str, status: &str) -> Result<DocSummary> {
        let mut doc = self.find(Kind::Goal, id)?;
        doc.meta.status = Some(status.to_string());
        if status == "done" {
            doc.meta.completed = Some(today());
        }
        self.save(&doc)?;
        Ok(summary(&doc))
    }

    // -------------------------------------------------------------- journal

    pub fn journal_append(&self, day: NaiveDate, text: &str) -> Result<DocSummary> {
        let rel = Vault::journal_rel_path(day);
        let mut doc = if self.vault.abs(&rel).exists() {
            self.vault.read(Kind::Journal, &rel)?
        } else {
            Doc {
                kind: Kind::Journal,
                id: day.to_string(),
                rel_path: rel,
                meta: Frontmatter {
                    title: format!("Nhật ký {day}"),
                    created: Some(Local::now()),
                    ..Default::default()
                },
                body: String::new(),
            }
        };
        let stamp = Local::now().format("%H:%M");
        if !doc.body.is_empty() && !doc.body.ends_with('\n') {
            doc.body.push('\n');
        }
        doc.body
            .push_str(&format!("\n## {stamp}\n\n{}\n", text.trim()));
        self.save(&doc)?;
        Ok(summary(&doc))
    }

    pub fn journal_read(&self, day: NaiveDate) -> Result<Option<DocFull>> {
        let rel = Vault::journal_rel_path(day);
        if !self.vault.abs(&rel).exists() {
            return Ok(None);
        }
        Ok(Some(full(self.vault.read(Kind::Journal, &rel)?)))
    }

    // ---------------------------------------------------------------- notes

    pub fn save_note(&self, title: &str, body: &str, tags: Vec<String>) -> Result<DocSummary> {
        let meta = Frontmatter {
            title: title.trim().to_string(),
            tags,
            created: Some(Local::now()),
            ..Default::default()
        };
        Ok(summary(&self.create(Kind::Note, meta, body.to_string())?))
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

    // --------------------------------------------------------------- habits

    pub fn log_habit(
        &self,
        habit: &str,
        day: NaiveDate,
        value: f64,
        note: Option<String>,
    ) -> Result<HabitStatus> {
        let habit = habit.trim().to_lowercase();
        if habit.is_empty() {
            bail!("habit name is empty");
        }
        self.conn.execute(
            "INSERT INTO habit_logs(habit, day, value, note) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(habit, day) DO UPDATE SET value = ?3, note = ?4",
            params![habit, day.to_string(), value, note],
        )?;
        self.habit_status(&habit, today(), 7)
    }

    pub fn habits(&self, window_days: u32) -> Result<Vec<HabitStatus>> {
        let names: Vec<String> = {
            let mut stmt = self
                .conn
                .prepare("SELECT DISTINCT habit FROM habit_logs ORDER BY habit")?;
            stmt.query_map([], |r| r.get(0))?
                .collect::<rusqlite::Result<_>>()?
        };
        names
            .iter()
            .map(|h| self.habit_status(h, today(), window_days))
            .collect()
    }

    fn habit_status(&self, habit: &str, on: NaiveDate, window_days: u32) -> Result<HabitStatus> {
        let days: Vec<NaiveDate> = {
            let mut stmt = self.conn.prepare(
                "SELECT day FROM habit_logs WHERE habit = ?1 AND day <= ?2 AND value > 0
                 ORDER BY day DESC",
            )?;
            stmt.query_map(params![habit, on.to_string()], |r| r.get::<_, String>(0))?
                .filter_map(|d| d.ok()?.parse().ok())
                .collect()
        };
        let done_today = days.first() == Some(&on);
        // A streak survives until the end of today: count from today if done,
        // otherwise from yesterday.
        let mut cursor = if done_today { on } else { on - Duration::days(1) };
        let mut streak = 0;
        for d in &days {
            if *d == cursor {
                streak += 1;
                cursor -= Duration::days(1);
            } else if *d < cursor {
                break;
            }
        }
        let window_start = on - Duration::days(window_days as i64 - 1);
        let in_window = days.iter().filter(|d| **d >= window_start).count() as u32;
        Ok(HabitStatus {
            habit: habit.to_string(),
            done_today,
            streak,
            days_done_in_window: in_window,
            window_days,
        })
    }

    // -------------------------------------------------------------- finance

    pub fn add_transaction(
        &self,
        day: NaiveDate,
        amount: f64,
        currency: &str,
        category: &str,
        note: Option<String>,
    ) -> Result<Transaction> {
        let currency = currency.trim().to_uppercase();
        if currency.len() != 3 {
            bail!("currency must be a 3-letter ISO code such as VND or USD");
        }
        let category = category.trim().to_lowercase();
        self.conn.execute(
            "INSERT INTO transactions(day, amount_minor, currency, category, note)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![day.to_string(), to_minor(amount, &currency), currency, category, note],
        )?;
        Ok(Transaction {
            id: self.conn.last_insert_rowid(),
            day: day.to_string(),
            amount: from_minor(to_minor(amount, &currency), &currency),
            currency,
            category,
            note,
        })
    }

    pub fn transactions(&self, from: NaiveDate, to: NaiveDate) -> Result<Vec<Transaction>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, day, amount_minor, currency, category, note FROM transactions
             WHERE day BETWEEN ?1 AND ?2 ORDER BY day, id",
        )?;
        let rows = stmt.query_map(params![from.to_string(), to.to_string()], |r| {
            let currency: String = r.get(3)?;
            Ok(Transaction {
                id: r.get(0)?,
                day: r.get(1)?,
                amount: from_minor(r.get(2)?, &currency),
                currency,
                category: r.get(4)?,
                note: r.get(5)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn delete_transaction(&self, id: i64) -> Result<()> {
        if self
            .conn
            .execute("DELETE FROM transactions WHERE id = ?1", [id])?
            == 0
        {
            bail!("no transaction {id}");
        }
        Ok(())
    }

    pub fn finance_summary(&self, from: NaiveDate, to: NaiveDate) -> Result<Vec<CurrencySummary>> {
        let mut by_currency: BTreeMap<String, (i64, i64, BTreeMap<String, i64>)> = BTreeMap::new();
        let mut stmt = self.conn.prepare(
            "SELECT currency, category, amount_minor FROM transactions WHERE day BETWEEN ?1 AND ?2",
        )?;
        let rows = stmt.query_map(params![from.to_string(), to.to_string()], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?))
        })?;
        for row in rows {
            let (currency, category, minor) = row?;
            let entry = by_currency.entry(currency).or_default();
            if minor >= 0 {
                entry.0 += minor;
            } else {
                entry.1 += -minor;
                *entry.2.entry(category).or_default() += -minor;
            }
        }
        Ok(by_currency
            .into_iter()
            .map(|(currency, (income, spending, cats))| {
                let mut cats: Vec<CategoryAmount> = cats
                    .into_iter()
                    .map(|(category, m)| CategoryAmount { category, amount: from_minor(m, &currency) })
                    .collect();
                cats.sort_by(|a, b| b.amount.total_cmp(&a.amount));
                CurrencySummary {
                    income: from_minor(income, &currency),
                    spending: from_minor(spending, &currency),
                    net: from_minor(income - spending, &currency),
                    spending_by_category: cats,
                    currency,
                }
            })
            .collect())
    }

    // -------------------------------------------------------------- metrics

    pub fn log_metric(
        &self,
        day: NaiveDate,
        name: &str,
        value: f64,
        unit: Option<String>,
        note: Option<String>,
    ) -> Result<Metric> {
        let name = name.trim().to_lowercase();
        self.conn.execute(
            "INSERT INTO metrics(day, name, value, unit, note) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![day.to_string(), name, value, unit, note],
        )?;
        Ok(Metric {
            day: day.to_string(),
            name,
            value,
            unit,
            note,
        })
    }

    pub fn metrics(&self, name: Option<&str>, from: NaiveDate, to: NaiveDate) -> Result<Vec<Metric>> {
        let mut stmt = self.conn.prepare(
            "SELECT day, name, value, unit, note FROM metrics
             WHERE (?1 IS NULL OR name = ?1) AND day BETWEEN ?2 AND ?3
             ORDER BY day, id",
        )?;
        let name = name.map(|n| n.trim().to_lowercase());
        let rows = stmt.query_map(params![name, from.to_string(), to.to_string()], |r| {
            Ok(Metric {
                day: r.get(0)?,
                name: r.get(1)?,
                value: r.get(2)?,
                unit: r.get(3)?,
                note: r.get(4)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
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

    // ---------------------------------------------------------------- today

    pub fn today(&self, day: NaiveDate) -> Result<Today> {
        let open = self.list_tasks(Some("open"))?;
        let day_s = day.to_string();
        let mut overdue = vec![];
        let mut due_today = vec![];
        let mut without_due = 0u32;
        for t in open {
            match t.due.as_deref() {
                Some(d) if d < day_s.as_str() => overdue.push(t),
                Some(d) if d == day_s => due_today.push(t),
                Some(_) => {}
                None => without_due += 1,
            }
        }
        Ok(Today {
            date: day_s,
            weekday: day.format("%A").to_string(),
            overdue_tasks: overdue,
            due_today,
            open_tasks_without_due: without_due,
            active_goals: self.list_goals(Some("active"))?,
            habits: self.habits(7)?,
            money_today: self.finance_summary(day, day)?,
            journal_written: self.vault.abs(&Vault::journal_rel_path(day)).exists(),
        })
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

fn summary(doc: &Doc) -> DocSummary {
    DocSummary {
        id: doc.id.clone(),
        kind: doc.kind,
        title: doc.meta.title.clone(),
        status: doc.meta.status.clone(),
        due: doc.meta.due.map(|d| d.to_string()),
        goal: doc.meta.goal.clone(),
        horizon: doc.meta.horizon.clone(),
        path: doc.rel_path.clone(),
    }
}

fn full(doc: Doc) -> DocFull {
    DocFull {
        summary: summary(&doc),
        tags: doc.meta.tags.clone(),
        body: doc.body,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn life() -> (tempfile::TempDir, Life) {
        let dir = tempfile::tempdir().unwrap();
        let life = Life::open_with_memory_db(dir.path()).unwrap();
        (dir, life)
    }

    fn d(s: &str) -> NaiveDate {
        s.parse().unwrap()
    }

    #[test]
    fn task_lifecycle_and_goal_link() {
        let (_dir, life) = life();
        assert!(life.add_task("Đọc sách", None, Some("nope".into()), vec![], None).is_err());
        let goal = life.add_goal("Học Rust", Some("2026".into()), None, None).unwrap();
        let t = life
            .add_task("Đọc chương 1", Some(d("2026-11-20")), Some(goal.id.clone()), vec![], None)
            .unwrap();
        assert_eq!(t.id, "doc-chuong-1");
        assert_eq!(life.add_task("Đọc chương 1", None, None, vec![], None).unwrap().id, "doc-chuong-1-2");

        let today = life.today(d("2026-11-25")).unwrap();
        assert_eq!(today.overdue_tasks.len(), 1);
        assert_eq!(today.open_tasks_without_due, 1);
        assert_eq!(today.active_goals.len(), 1);

        life.complete_task("doc-chuong-1").unwrap();
        assert_eq!(life.list_tasks(Some("open")).unwrap().len(), 1);
        assert_eq!(life.list_tasks(Some("done")).unwrap().len(), 1);
    }

    #[test]
    fn hand_edits_are_picked_up_by_sync() {
        let (dir, mut life) = life();
        fs::write(
            dir.path().join("notes/tay.md"),
            "---\ntitle: Viết tay\n---\nnội dung về cà phê\n",
        )
        .unwrap();
        let stats = life.sync_index().unwrap();
        assert_eq!(stats.indexed, 1);
        // diacritics-insensitive search
        let hits = life.search("ca phe", 5).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "tay");

        fs::remove_file(dir.path().join("notes/tay.md")).unwrap();
        assert_eq!(life.sync_index().unwrap().removed, 1);
        assert!(life.search("ca phe", 5).unwrap().is_empty());
    }

    #[test]
    fn journal_appends_to_one_file_per_day() {
        let (_dir, life) = life();
        let day = d("2026-11-25");
        life.journal_append(day, "Sáng chạy bộ 5km").unwrap();
        life.journal_append(day, "Tối đọc sách").unwrap();
        let entry = life.journal_read(day).unwrap().unwrap();
        assert!(entry.body.contains("chạy bộ") && entry.body.contains("đọc sách"));
        assert_eq!(entry.summary.path, "journal/2026/2026-11-25.md");
        assert!(life.search("chay bo", 5).unwrap().len() == 1);
    }

    #[test]
    fn habit_streak_counts_consecutive_days() {
        let (_dir, life) = life();
        for day in ["2026-11-21", "2026-11-23", "2026-11-24"] {
            life.log_habit("Thiền", d(day), 1.0, None).unwrap();
        }
        let s = life.habit_status("thiền", d("2026-11-25"), 7).unwrap();
        assert!(!s.done_today);
        assert_eq!(s.streak, 2);
        assert_eq!(s.days_done_in_window, 3);
        life.log_habit("thiền", d("2026-11-25"), 1.0, None).unwrap();
        let s = life.habit_status("thiền", d("2026-11-25"), 7).unwrap();
        assert!(s.done_today);
        assert_eq!(s.streak, 3);
    }

    #[test]
    fn money_is_stored_in_minor_units_per_currency() {
        let (_dir, life) = life();
        let day = d("2026-11-25");
        life.add_transaction(day, -45_000.0, "vnd", "Ăn uống", None).unwrap();
        life.add_transaction(day, -30_000.0, "VND", "ăn uống", None).unwrap();
        life.add_transaction(day, 20_000_000.0, "VND", "lương", None).unwrap();
        life.add_transaction(day, -9.99, "USD", "software", None).unwrap();
        let s = life.finance_summary(day, day).unwrap();
        let usd = s.iter().find(|c| c.currency == "USD").unwrap();
        assert_eq!(usd.spending, 9.99);
        let vnd = s.iter().find(|c| c.currency == "VND").unwrap();
        assert_eq!(vnd.spending, 75_000.0);
        assert_eq!(vnd.net, 19_925_000.0);
        assert_eq!(vnd.spending_by_category[0].category, "ăn uống");
        assert_eq!(vnd.spending_by_category[0].amount, 75_000.0);
    }
}
