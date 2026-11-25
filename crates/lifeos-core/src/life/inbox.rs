//! Inbox: things the user threw at lifeos without saying what they are yet.
//! The agent sorts them later into tasks, notes, people… and resolves them.

use anyhow::{Result, bail};
use chrono::Local;
use rusqlite::params;

use super::Life;
use lifeos_wire::InboxItem;

input! {
    pub struct Capture {
        pub text: String,
    }
}

impl Life {
    pub fn capture(&self, c: Capture) -> Result<InboxItem> {
        let text = c.text.trim().to_string();
        if text.is_empty() {
            bail!("nothing to capture");
        }
        let created = Local::now().format("%Y-%m-%d %H:%M").to_string();
        self.conn.execute("INSERT INTO inbox(created, text) VALUES (?1, ?2)", params![created, text])?;
        Ok(InboxItem { id: self.conn.last_insert_rowid(), created, text })
    }

    /// What is still unsorted, oldest first.
    pub fn inbox(&self) -> Result<Vec<InboxItem>> {
        let mut stmt = self.conn.prepare("SELECT id, created, text FROM inbox WHERE done = 0 ORDER BY id")?;
        let rows = stmt.query_map([], |r| Ok(InboxItem { id: r.get(0)?, created: r.get(1)?, text: r.get(2)? }))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn resolve_inbox(&self, id: i64) -> Result<()> {
        if self.conn.execute("UPDATE inbox SET done = 1 WHERE id = ?1 AND done = 0", [id])? == 0 {
            bail!("no open inbox item {id}");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::life::test_support::*;

    #[test]
    fn capture_then_resolve() {
        let (_dir, life) = life();
        let a = life.capture(Capture { text: " mua quà sinh nhật mẹ ".into() }).unwrap();
        life.capture(Capture { text: "sách: Atomic Habits".into() }).unwrap();
        assert!(life.capture(Capture { text: "  ".into() }).is_err());
        assert_eq!(a.text, "mua quà sinh nhật mẹ");
        life.resolve_inbox(a.id).unwrap();
        assert!(life.resolve_inbox(a.id).is_err());
        assert_eq!(life.inbox().unwrap().len(), 1);
    }
}
