//! Memory: what agents remember across conversations, in `memories` (with a
//! full-text index). Each memory has a holder: the Assistant, or a crew
//! member. The Assistant's memories are about the user and are shared with
//! the whole crew; a member's own memories are its alone.
//!
//! Agents write memories themselves (`remember`, `forget`). Before each
//! message, lifeos recalls the few that bear on it (`recall_for`) and puts
//! them in front of the message, so nothing depends on the agent searching.

use anyhow::{Result, bail};
use chrono::Local;
use rusqlite::{OptionalExtension, Row, params};

use super::Life;
use lifeos_wire::{ASSISTANT, Memory};

choice! {
    pub enum MemoryKind {
        /// Something true about the user or their life ("làm việc ở công ty X").
        Fact => "fact",
        /// How the user likes things done ("không thích nhắc vào cuối tuần").
        Preference => "preference",
        /// A rule the user lives or decides by.
        Principle => "principle",
        /// Something learned the hard way.
        Lesson => "lesson",
        /// A choice made, with its reason.
        Decision => "decision",
        /// Something the user means to do or become.
        Intent => "intent",
    }
}

input! {
    pub struct Remember {
        pub kind: MemoryKind,
        /// One line, the claim itself.
        pub title: String,
        /// Why, the details, the context. Short.
        pub body: String,
        /// Id of a memory this one replaces (it is then forgotten).
        pub replaces: Option<i64>,
    }
}

input! {
    pub struct Forget {
        pub id: i64,
        /// Why it no longer holds.
        pub why: String,
    }
}

input! {
    pub struct Recall {
        /// Words to look for (accents optional).
        pub query: String,
        /// Defaults to 8.
        pub limit: Option<u32>,
    }
}

input! {
    pub struct ListMemories {
        /// `assistant` or a crew member id. Omit for everyone.
        pub holder: Option<String>,
        /// Words to look for. Omit for all, newest first.
        pub query: Option<String>,
        /// Also what was forgotten or replaced.
        #[serde(default)]
        pub include_forgotten: bool,
    }
}

const COLUMNS: &str = "m.id, m.holder, m.kind, m.title, m.body, m.created, m.updated, m.ended, m.why_ended, m.recalled";

fn memory(r: &Row<'_>) -> rusqlite::Result<Memory> {
    Ok(Memory {
        id: r.get(0)?,
        holder: r.get(1)?,
        kind: r.get(2)?,
        title: r.get(3)?,
        body: r.get(4)?,
        created: r.get(5)?,
        updated: r.get(6)?,
        ended: r.get(7)?,
        why_ended: r.get(8)?,
        recalled: r.get(9)?,
    })
}

fn now() -> String {
    Local::now().format("%Y-%m-%d %H:%M").to_string()
}

/// Any of the words, each quoted so user text can't break FTS5 syntax.
fn any_of(text: &str) -> String {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.chars().count() > 1)
        .map(|w| format!("\"{w}\""))
        .collect::<Vec<_>>()
        .join(" OR ")
}

impl Life {
    pub fn remember(&self, holder: &str, r: Remember) -> Result<Memory> {
        let (title, body) = (r.title.trim(), r.body.trim());
        if title.is_empty() {
            bail!("a memory needs a title");
        }
        let at = now();
        if let Some(old) = r.replaces {
            self.end_memory(holder, old, &format!("thay bằng: {title}"))?;
        }
        self.conn.execute(
            "INSERT INTO memories(holder, kind, title, body, created, updated) VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
            params![holder, r.kind.as_str(), title, body, at],
        )?;
        let id = self.conn.last_insert_rowid();
        self.conn
            .execute("INSERT INTO memories_fts(rowid, title, body) VALUES (?1, ?2, ?3)", params![id, title, body])?;
        self.memory(id)
    }

    pub fn forget(&self, holder: &str, f: Forget) -> Result<()> {
        self.end_memory(holder, f.id, f.why.trim())
    }

    /// The Assistant may end anyone's memory (it acts for the user); a crew
    /// member only its own.
    fn end_memory(&self, holder: &str, id: i64, why: &str) -> Result<()> {
        let owner: Option<String> = self
            .conn
            .query_row("SELECT holder FROM memories WHERE id = ?1 AND ended IS NULL", [id], |r| r.get(0))
            .optional()?;
        match owner {
            None => bail!("no remembered memory {id}"),
            Some(o) if o != holder && holder != ASSISTANT => bail!("memory {id} is {o}'s, not yours"),
            Some(_) => {}
        }
        self.conn.execute(
            "UPDATE memories SET ended = ?2, why_ended = ?3, updated = ?2 WHERE id = ?1",
            params![id, now(), why],
        )?;
        self.conn.execute("DELETE FROM memories_fts WHERE rowid = ?1", [id])?;
        Ok(())
    }

    pub fn memory(&self, id: i64) -> Result<Memory> {
        Ok(self.conn.query_row(&format!("SELECT {COLUMNS} FROM memories m WHERE m.id = ?1"), [id], memory)?)
    }

    /// What `holder` can draw on: its own memories, and for a crew member
    /// the Assistant's too. Best matches first.
    pub fn recall(&self, holder: &str, query: &str, limit: u32) -> Result<Vec<Memory>> {
        let q = any_of(query);
        if q.is_empty() {
            return Ok(vec![]);
        }
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {COLUMNS} FROM memories_fts f JOIN memories m ON m.id = f.rowid
             WHERE memories_fts MATCH ?1 AND m.ended IS NULL AND m.holder IN (?2, '{ASSISTANT}')
             ORDER BY bm25(memories_fts) LIMIT ?3"
        ))?;
        let rows = stmt.query_map(params![q, holder, limit], memory)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// The memories to put in front of a message: the best matches not
    /// already shown in this conversation. Counts them as recalled.
    pub fn recall_for(&self, holder: &str, message: &str, shown: &[i64], limit: u32) -> Result<Vec<Memory>> {
        let mut found = self.recall(holder, message, limit + shown.len() as u32)?;
        found.retain(|m| !shown.contains(&m.id));
        found.truncate(limit as usize);
        for m in &found {
            self.conn.execute("UPDATE memories SET recalled = recalled + 1 WHERE id = ?1", [m.id])?;
        }
        Ok(found)
    }

    pub fn memories(&self, q: &ListMemories) -> Result<Vec<Memory>> {
        let ended = if q.include_forgotten { "1" } else { "m.ended IS NULL" };
        let (sql, words) = match q.query.as_deref().map(any_of).filter(|w| !w.is_empty()) {
            Some(words) => (
                format!(
                    "SELECT {COLUMNS} FROM memories_fts f JOIN memories m ON m.id = f.rowid
                     WHERE memories_fts MATCH ?2 AND (?1 IS NULL OR m.holder = ?1) AND {ended}
                     ORDER BY bm25(memories_fts) LIMIT 200"
                ),
                Some(words),
            ),
            None => (
                format!(
                    "SELECT {COLUMNS} FROM memories m WHERE (?1 IS NULL OR m.holder = ?1) AND {ended}
                     AND (?2 IS NULL OR 1) ORDER BY m.ended IS NOT NULL, m.updated DESC, m.id DESC LIMIT 500"
                ),
                None,
            ),
        };
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params![q.holder, words], memory)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn memory_count(&self, holder: &str) -> Result<u32> {
        Ok(self.conn.query_row(
            "SELECT COUNT(*) FROM memories WHERE holder = ?1 AND ended IS NULL",
            [holder],
            |r| r.get(0),
        )?)
    }
}

/// The note put in front of a message: what the agent remembers that bears on it.
pub fn recall_note(memories: &[Memory]) -> Option<String> {
    if memories.is_empty() {
        return None;
    }
    let mut note = String::from(
        "<recalled>\nNhững điều bạn nhớ có liên quan đến tin nhắn này (dùng nếu hữu ích; nếu điều nào sai thì `forget`):\n",
    );
    for m in memories {
        let clean = |s: &str| s.replace('<', "‹").replace('>', "›");
        note.push_str(&format!(
            "<memory id=\"{}\" kind=\"{}\" from=\"{}\">{}\n{}</memory>\n",
            m.id,
            m.kind,
            m.holder,
            clean(&m.title),
            clean(&m.body)
        ));
    }
    note.push_str("</recalled>");
    Some(note)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::life::test_support::*;

    fn fact(title: &str, body: &str) -> Remember {
        Remember { kind: MemoryKind::Fact, title: title.into(), body: body.into(), replaces: None }
    }

    #[test]
    fn remember_recall_forget() {
        let (_dir, life) = life();
        let a = life.remember(ASSISTANT, fact("Mẹ thích hoa lan", "Tặng lan hồ điệp dịp sinh nhật")).unwrap();
        life.remember("hlv", fact("Chạy bộ buổi sáng hợp hơn", "Tối hay mệt")).unwrap();
        life.remember("tai-chinh", fact("Tiết kiệm 20% lương", "")).unwrap();

        // A member sees its own and the Assistant's, not another member's.
        let hlv = life.recall("hlv", "sinh nhật mẹ, chạy bộ hay tiết kiệm?", 10).unwrap();
        assert_eq!(hlv.len(), 2);
        // Accents optional.
        assert_eq!(life.recall(ASSISTANT, "hoa lan", 5).unwrap()[0].id, a.id);

        let first = life.recall_for(ASSISTANT, "mua hoa cho mẹ", &[], 5).unwrap();
        assert_eq!(first.len(), 1);
        assert!(life.recall_for(ASSISTANT, "mua hoa cho mẹ", &[a.id], 5).unwrap().is_empty());
        assert_eq!(life.memory(a.id).unwrap().recalled, 1);
        assert!(recall_note(&first).unwrap().contains("hoa lan"));

        assert!(life.forget("hlv", Forget { id: a.id, why: "x".into() }).is_err());
        let b = life
            .remember(ASSISTANT, Remember { replaces: Some(a.id), ..fact("Mẹ thích hoa hồng", "Mẹ nói lại") })
            .unwrap();
        assert!(life.recall(ASSISTANT, "hoa lan", 5).unwrap().iter().all(|m| m.id == b.id));
        assert!(life.memory(a.id).unwrap().ended.is_some());

        let all = ListMemories { include_forgotten: true, ..Default::default() };
        assert_eq!(life.memories(&all).unwrap().len(), 4);
        assert_eq!(life.memories(&ListMemories::default()).unwrap().len(), 3);
        let hoa = ListMemories { query: Some("hoa".into()), ..Default::default() };
        assert_eq!(life.memories(&hoa).unwrap().len(), 1);
        assert_eq!(life.memory_count(ASSISTANT).unwrap(), 1);
    }
}
