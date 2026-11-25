//! Conversations: every turn with every agent, kept so a page opened later
//! (or a restart) shows the whole conversation; and the Claude Code session
//! each agent resumes.

use anyhow::Result;
use rusqlite::params;

use super::Life;
use lifeos_wire::Turn;

/// Turns of an agent kept for the page; older ones stay in the database.
pub const HISTORY_TURNS: u32 = 200;

impl Life {
    pub fn save_turn(&self, agent: &str, turn: &Turn) -> Result<()> {
        self.conn.execute(
            "INSERT INTO turns(agent, at, turn) VALUES (?1, ?2, ?3)",
            params![agent, turn.at, serde_json::to_string(turn)?],
        )?;
        Ok(())
    }

    /// The latest turns since the conversation was last started over, oldest first.
    pub fn turns(&self, agent: &str) -> Result<Vec<Turn>> {
        let since: i64 = self.kv_get(&reset_key(agent))?.and_then(|v| v.parse().ok()).unwrap_or(0);
        let mut stmt = self.conn.prepare(
            "SELECT turn FROM (SELECT id, turn FROM turns WHERE agent = ?1 AND id > ?2 ORDER BY id DESC LIMIT ?3)
             ORDER BY id",
        )?;
        let rows = stmt.query_map(params![agent, since, HISTORY_TURNS], |r| r.get::<_, String>(0))?;
        rows.map(|t| Ok(serde_json::from_str(&t?)?)).collect()
    }

    /// Starts the conversation over: the page shows nothing before now, and
    /// the next message opens a fresh Claude Code session.
    pub fn reset_conversation(&self, agent: &str) -> Result<()> {
        let last: i64 = self.conn.query_row("SELECT COALESCE(MAX(id), 0) FROM turns", [], |r| r.get(0))?;
        self.kv_set(&reset_key(agent), &last.to_string())?;
        self.conn.execute("DELETE FROM kv WHERE key = ?1", [session_key(agent)])?;
        Ok(())
    }

    pub fn session_of(&self, agent: &str) -> Result<Option<String>> {
        self.kv_get(&session_key(agent))
    }

    pub fn set_session_of(&self, agent: &str, session: &str) -> Result<()> {
        self.kv_set(&session_key(agent), session)
    }
}

fn reset_key(agent: &str) -> String {
    format!("chat.reset.{agent}")
}

fn session_key(agent: &str) -> String {
    format!("chat.session.{agent}")
}

#[cfg(test)]
mod tests {
    use crate::life::test_support::*;
    use lifeos_wire::{ChatDone, Step, Turn};

    #[test]
    fn turns_survive_and_reset() {
        let (_dir, life) = life();
        let turn = |ask: &str| Turn {
            ask: ask.into(),
            at: "2026-11-25 09:00".into(),
            reply: ChatDone { steps: vec![Step::Text("ừ".into())], ms: 5, cost_usd: None, error: None },
            source: None,
        };
        life.save_turn("assistant", &turn("một")).unwrap();
        life.save_turn("coach", &turn("hai")).unwrap();
        life.set_session_of("assistant", "s1").unwrap();
        assert_eq!(life.turns("assistant").unwrap(), vec![turn("một")]);
        life.reset_conversation("assistant").unwrap();
        assert!(life.turns("assistant").unwrap().is_empty());
        assert_eq!(life.session_of("assistant").unwrap(), None);
        assert_eq!(life.turns("coach").unwrap().len(), 1);
        life.save_turn("assistant", &turn("ba")).unwrap();
        assert_eq!(life.turns("assistant").unwrap().len(), 1);
    }
}
