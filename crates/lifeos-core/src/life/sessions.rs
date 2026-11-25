//! What each card's session was given, loaded and holds (remit's `session_launches`, `session_inits` and
//! `session_contexts`): what lifeos started it with, Claude Code's newest init, and its context as counted when it last
//! went to rest. Each is the newest of its card, replacing the one before; each names the session it is of, so a reader
//! can tell one of an older session.

use anyhow::Result;
use rusqlite::{OptionalExtension, params};

use super::Life;
use lifeos_wire::{ContextCount, ContextGiven, InitReport};

/// What lifeos started a card's session with, and when.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessionLaunch {
    pub card: String,
    pub session_id: String,
    /// The process resumed the session rather than opening it.
    pub resumed: bool,
    /// RFC 3339.
    pub at: String,
    pub given: ContextGiven,
}

impl Life {
    /// Records what card's session was started with. A resumed session keeps the opening recorded of it; a new one's
    /// is recorded as it is sent (`record_opening`).
    pub fn put_session_launch(&self, launch: &SessionLaunch) -> Result<()> {
        let mut given = launch.given.clone();
        if launch.resumed
            && let Some(before) = self.session_launch(&launch.card)?
            && before.session_id == launch.session_id
        {
            given.opening = before.given.opening;
        }
        self.conn.execute(
            "INSERT INTO session_launches(card, session_id, resumed, at, given) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(card) DO UPDATE SET session_id = excluded.session_id, resumed = excluded.resumed,
                 at = excluded.at, given = excluded.given",
            params![launch.card, launch.session_id, launch.resumed, launch.at, serde_json::to_string(&given)?],
        )?;
        Ok(())
    }

    pub fn session_launch(&self, card: &str) -> Result<Option<SessionLaunch>> {
        let row = self
            .conn
            .query_row(
                "SELECT session_id, resumed, at, given FROM session_launches WHERE card = ?1",
                [card],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, bool>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?)),
            )
            .optional()?;
        let Some((session_id, resumed, at, given)) = row else { return Ok(None) };
        Ok(Some(SessionLaunch { card: card.to_owned(), session_id, resumed, at, given: serde_json::from_str(&given)? }))
    }

    /// Records text, sent as the first message of card's session `session_id`, as the session's opening; a session
    /// whose start was not recorded, or whose opening was, is left as it is.
    pub fn record_opening(&self, card: &str, session_id: &str, text: &str) -> Result<()> {
        let Some(mut l) = self.session_launch(card)? else { return Ok(()) };
        if l.session_id != session_id || l.given.opening.is_some() {
            return Ok(());
        }
        l.given.opening = Some(text.to_owned());
        self.conn.execute(
            "UPDATE session_launches SET given = ?2 WHERE card = ?1",
            params![card, serde_json::to_string(&l.given)?],
        )?;
        Ok(())
    }

    /// Keeps what Claude Code says card's session loaded, over what it said before.
    pub fn put_session_init(&self, card: &str, report: &InitReport) -> Result<()> {
        self.conn.execute(
            "INSERT INTO session_inits(card, session_id, doc) VALUES (?1, ?2, ?3)
             ON CONFLICT(card) DO UPDATE SET session_id = excluded.session_id, doc = excluded.doc",
            params![card, report.init.session_id, serde_json::to_string(report)?],
        )?;
        Ok(())
    }

    pub fn session_init(&self, card: &str) -> Result<Option<InitReport>> {
        let doc: Option<String> =
            self.conn.query_row("SELECT doc FROM session_inits WHERE card = ?1", [card], |r| r.get(0)).optional()?;
        doc.map(|d| Ok(serde_json::from_str(&d)?)).transpose()
    }

    /// Keeps what fills card's session's context, counted as it went to rest, over the count before.
    pub fn put_session_context(&self, card: &str, count: &ContextCount) -> Result<()> {
        self.conn.execute(
            "INSERT INTO session_contexts(card, session_id, doc) VALUES (?1, ?2, ?3)
             ON CONFLICT(card) DO UPDATE SET session_id = excluded.session_id, doc = excluded.doc",
            params![card, count.context.session_id, serde_json::to_string(count)?],
        )?;
        Ok(())
    }

    pub fn session_context(&self, card: &str) -> Result<Option<ContextCount>> {
        let doc: Option<String> =
            self.conn.query_row("SELECT doc FROM session_contexts WHERE card = ?1", [card], |r| r.get(0)).optional()?;
        doc.map(|d| Ok(serde_json::from_str(&d)?)).transpose()
    }
}

#[cfg(test)]
mod tests {
    use lifeos_wire::{ContextPart, SessionContext, SessionInit};

    use super::*;

    #[test]
    fn launches_inits_and_counts_are_kept_by_card_and_read_back() {
        let dir = tempfile::tempdir().unwrap();
        let life = Life::open(dir.path()).unwrap();
        assert_eq!(life.session_launch("assistant").unwrap(), None);
        assert_eq!(life.session_init("assistant").unwrap(), None);
        assert_eq!(life.session_context("assistant").unwrap(), None);

        let launch = SessionLaunch {
            card: "assistant".into(),
            session_id: "s1".into(),
            resumed: false,
            at: "2026-11-25T09:00:00+07:00".into(),
            given: ContextGiven { setting_sources: vec!["project".into()], ..ContextGiven::default() },
        };
        life.put_session_launch(&launch).unwrap();
        life.record_opening("assistant", "s1", "Hello").unwrap();
        life.record_opening("assistant", "s1", "Again").unwrap();
        life.record_opening("assistant", "other", "Not this one").unwrap();
        assert_eq!(life.session_launch("assistant").unwrap().unwrap().given.opening.as_deref(), Some("Hello"));
        // A resume of the same session keeps its opening; a new session does not.
        life.put_session_launch(&SessionLaunch { resumed: true, ..launch.clone() }).unwrap();
        assert_eq!(life.session_launch("assistant").unwrap().unwrap().given.opening.as_deref(), Some("Hello"));
        life.put_session_launch(&SessionLaunch { session_id: "s2".into(), ..launch }).unwrap();
        assert_eq!(life.session_launch("assistant").unwrap().unwrap().given.opening, None);

        let init = InitReport {
            init: SessionInit { session_id: "s2".into(), version: "2.1.0".into(), ..SessionInit::default() },
            at: "2026-11-25T09:01:00+07:00".into(),
        };
        life.put_session_init("assistant", &init).unwrap();
        assert_eq!(life.session_init("assistant").unwrap(), Some(init));

        let count = ContextCount {
            context: SessionContext {
                session_id: "s2".into(),
                total: 1200,
                size: 200_000,
                parts: vec![ContextPart { id: "messages".into(), name: "Messages".into(), tokens: 1200, kind: "used".into() }],
                ..SessionContext::default()
            },
            at: "2026-11-25T09:02:00+07:00".into(),
        };
        life.put_session_context("assistant", &count).unwrap();
        assert_eq!(life.session_context("assistant").unwrap(), Some(count));
        assert_eq!(life.session_context("other").unwrap(), None);
    }
}
