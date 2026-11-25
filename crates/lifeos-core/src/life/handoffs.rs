//! What agents ask and hand each other (docs/adr/0002, after agentes.chat's ADR 0034 and 0095). Each `ask_agent` and
//! `hand_off` is a row from the moment it is sent until what came of it went back to its sender: who has it, how it
//! stands, what came of it. The rows of one batch (what one piece of a sender's work handed out) go back together, once
//! none of them is still out. Sending, delivering and the health check act on the agents and live in lifeos-agent; this
//! keeps the rows.

use anyhow::{Result, bail};
use rusqlite::{OptionalExtension, Row, params};

use super::Life;
use super::cards::now;
use lifeos_wire::{Handoff, HandoffKind, HandoffStatus};

const COLUMNS: &str = "id, kind, sender, sender_name, target, target_name, text, status, result, parent, chain, attempts, \
                       note, created, updated, closed, reported";

/// The statuses of a row still out, as SQL.
const OPEN: &str = "('queued', 'working', 'waiting')";

/// The hand-offs a list shows, at most.
const LISTED: u32 = 200;

input! {
    pub struct ListHandoffs {
        /// Only those still out: queued, being worked on, or waiting on what was handed on.
        #[serde(default)]
        pub open: bool,
        /// Only those this agent sent or has (its card id).
        #[serde(default)]
        pub agent: Option<String>,
    }
}

/// A hand-off about to be sent.
#[derive(Debug, Clone, Default)]
pub struct NewHandoff {
    pub kind: HandoffKind,
    pub sender: String,
    pub sender_name: String,
    pub target: String,
    pub target_name: String,
    pub text: String,
    pub parent: Option<i64>,
    /// What it comes back with (`batch_of_handoff`, `batch_of_prompt`).
    pub batch: String,
    pub chain: Vec<String>,
}

/// The batch of what a sender handed out while working on hand-off `id`: it goes back into that hand-off's answer.
pub fn batch_of_handoff(id: i64) -> String {
    format!("h:{id}")
}

/// The batch of what a sender handed out in the turn on its prompt event `id`, working for no hand-off.
pub fn batch_of_prompt(id: i64) -> String {
    format!("p:{id}")
}

fn handoff(r: &Row) -> rusqlite::Result<Handoff> {
    let kind: String = r.get(1)?;
    let status: String = r.get(7)?;
    let chain: String = r.get(10)?;
    Ok(Handoff {
        id: r.get(0)?,
        kind: HandoffKind::parse(&kind).unwrap_or_default(),
        sender: r.get(2)?,
        sender_name: r.get(3)?,
        target: r.get(4)?,
        target_name: r.get(5)?,
        text: r.get(6)?,
        status: HandoffStatus::parse(&status).unwrap_or_default(),
        result: r.get(8)?,
        parent: r.get(9)?,
        chain: serde_json::from_str(&chain).unwrap_or_default(),
        attempts: r.get(11)?,
        note: r.get(12)?,
        created: r.get(13)?,
        updated: r.get(14)?,
        closed: r.get(15)?,
        reported: r.get::<_, i64>(16)? != 0,
    })
}

impl Life {
    /// Records a hand-off. News is done at once: nothing comes of it, it is only read.
    pub fn add_handoff(&self, h: NewHandoff) -> Result<Handoff> {
        let text = h.text.trim();
        if text.is_empty() {
            bail!("say what you hand over");
        }
        let at = now();
        let news = h.kind == HandoffKind::News;
        self.conn.execute(
            "INSERT INTO handoffs(kind, sender, sender_name, target, target_name, text, status, parent, batch, chain,
                                  created, updated, closed, reported)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11, ?12, ?13)",
            params![
                h.kind.as_str(),
                h.sender,
                h.sender_name,
                h.target,
                h.target_name,
                text,
                if news { HandoffStatus::Done } else { HandoffStatus::Queued }.as_str(),
                h.parent,
                h.batch,
                serde_json::to_string(&h.chain)?,
                at,
                if news { at.as_str() } else { "" },
                i64::from(news),
            ],
        )?;
        self.handoff(self.conn.last_insert_rowid())
    }

    pub fn handoff(&self, id: i64) -> Result<Handoff> {
        self.conn
            .query_row(&format!("SELECT {COLUMNS} FROM handoffs WHERE id = ?1"), [id], handoff)
            .optional()?
            .ok_or_else(|| anyhow::anyhow!("no hand-off {id}"))
    }

    /// The batch hand-off `id` goes back with.
    pub fn handoff_batch(&self, id: i64) -> Result<String> {
        Ok(self.conn.query_row("SELECT batch FROM handoffs WHERE id = ?1", [id], |r| r.get(0))?)
    }

    /// The prompt event hand-off `id` was queued as in its target's outbox; 0 for news.
    pub fn handoff_prompt(&self, id: i64) -> Result<i64> {
        Ok(self.conn.query_row("SELECT prompt FROM handoffs WHERE id = ?1", [id], |r| r.get(0))?)
    }

    /// Names the prompt event hand-off `id` was queued as.
    pub fn set_handoff_prompt(&self, id: i64, prompt: i64) -> Result<()> {
        self.conn.execute("UPDATE handoffs SET prompt = ?2 WHERE id = ?1", params![id, prompt])?;
        Ok(())
    }

    /// Newest first: all, or those still out; those `agent` sent or has, when said.
    pub fn handoffs(&self, open: bool, agent: Option<&str>) -> Result<Vec<Handoff>> {
        let mut filter = Vec::new();
        if open {
            filter.push(format!("status IN {OPEN}"));
        }
        if agent.is_some() {
            filter.push("(sender = ?1 OR target = ?1)".to_owned());
        }
        let filter = if filter.is_empty() { String::new() } else { format!("WHERE {}", filter.join(" AND ")) };
        let mut stmt =
            self.conn.prepare(&format!("SELECT {COLUMNS} FROM handoffs {filter} ORDER BY id DESC LIMIT {LISTED}"))?;
        let rows = match agent {
            Some(a) => stmt.query_map([a], handoff)?.collect::<rusqlite::Result<Vec<_>>>()?,
            None => stmt.query_map([], handoff)?.collect::<rusqlite::Result<Vec<_>>>()?,
        };
        Ok(rows)
    }

    /// What `sender` handed out that a turn of its reads about: still out, or closed since `since` (RFC 3339), newest
    /// first, at most `limit`. News is left out: nothing comes of it.
    pub fn handed_by(&self, sender: &str, since: &str, limit: u32) -> Result<Vec<Handoff>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {COLUMNS} FROM handoffs WHERE sender = ?1 AND kind != 'news' AND (status IN {OPEN} OR closed >= ?2)
             ORDER BY id DESC LIMIT ?3"
        ))?;
        Ok(stmt.query_map(params![sender, since, limit], handoff)?.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// The same words `sender` already handed `target` that are still out: what a second send would repeat.
    pub fn open_duplicate(&self, sender: &str, target: &str, text: &str) -> Result<Option<Handoff>> {
        Ok(self
            .conn
            .query_row(
                &format!(
                    "SELECT {COLUMNS} FROM handoffs
                     WHERE sender = ?1 AND target = ?2 AND text = ?3 AND status IN {OPEN} ORDER BY id DESC LIMIT 1"
                ),
                params![sender, target, text.trim()],
                handoff,
            )
            .optional()?)
    }

    /// Every hand-off still out, oldest first: what the health check looks at.
    pub fn open_handoffs(&self) -> Result<Vec<Handoff>> {
        let mut stmt =
            self.conn.prepare(&format!("SELECT {COLUMNS} FROM handoffs WHERE status IN {OPEN} ORDER BY id"))?;
        Ok(stmt.query_map([], handoff)?.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// How many of what was handed on while working on hand-off `parent` are still out.
    pub fn open_children(&self, parent: i64) -> Result<i64> {
        Ok(self.conn.query_row(
            &format!("SELECT COUNT(*) FROM handoffs WHERE parent = ?1 AND status IN {OPEN}"),
            [parent],
            |r| r.get(0),
        )?)
    }

    /// Moves open hand-off `id` to `status` (still open: working or waiting), keeping `note` when one is said.
    pub fn mark_handoff(&self, id: i64, status: HandoffStatus, note: Option<&str>) -> Result<()> {
        self.conn.execute(
            &format!(
                "UPDATE handoffs SET status = ?2, note = COALESCE(?3, note), updated = ?4 WHERE id = ?1 AND status IN {OPEN}"
            ),
            params![id, status.as_str(), note, now()],
        )?;
        Ok(())
    }

    /// Keeps `result` on open hand-off `id` without closing it: the answer so far, while what it handed on is out.
    pub fn hold_result(&self, id: i64, result: &str) -> Result<()> {
        self.conn.execute(
            &format!("UPDATE handoffs SET result = ?2, updated = ?3 WHERE id = ?1 AND status IN {OPEN}"),
            params![id, result, now()],
        )?;
        Ok(())
    }

    /// Closes hand-off `id` as `status` with what came of it. The row as it now is, or `None` when it was already
    /// closed: whoever closes it first decides.
    pub fn close_handoff(&self, id: i64, status: HandoffStatus, result: &str, note: &str) -> Result<Option<Handoff>> {
        if status.open() {
            bail!("a hand-off closes as done, failed or cancelled");
        }
        let at = now();
        let n = self.conn.execute(
            &format!(
                "UPDATE handoffs SET status = ?2, result = ?3, note = CASE WHEN ?4 = '' THEN note ELSE ?4 END,
                                     updated = ?5, closed = ?5
                 WHERE id = ?1 AND status IN {OPEN}"
            ),
            params![id, status.as_str(), result, note, at],
        )?;
        if n == 0 {
            return Ok(None);
        }
        self.handoff(id).map(Some)
    }

    /// Gives hand-off `id` to its target again as prompt `prompt`, after lifeos lost the first.
    pub fn requeue_handoff(&self, id: i64, prompt: i64, note: &str) -> Result<()> {
        self.conn.execute(
            &format!(
                "UPDATE handoffs SET status = 'queued', prompt = ?2, attempts = attempts + 1, note = ?3, updated = ?4
                 WHERE id = ?1 AND status IN {OPEN}"
            ),
            params![id, prompt, note, now()],
        )?;
        Ok(())
    }

    /// Takes the rows of `batch` not reported yet, once none of them is still out, marking them reported; `None` while
    /// one is out or when there is nothing to report. Whoever claims a batch reports it, once.
    pub fn claim_batch(&self, batch: &str) -> Result<Option<Vec<Handoff>>> {
        let rows: Vec<Handoff> = {
            let mut stmt = self.conn.prepare(&format!(
                "SELECT {COLUMNS} FROM handoffs WHERE batch = ?1 AND reported = 0 AND kind != 'news' ORDER BY id"
            ))?;
            stmt.query_map([batch], handoff)?.collect::<rusqlite::Result<Vec<_>>>()?
        };
        if rows.is_empty() || rows.iter().any(|h| h.status.open()) {
            return Ok(None);
        }
        for h in &rows {
            self.conn.execute("UPDATE handoffs SET reported = 1 WHERE id = ?1", [h.id])?;
        }
        Ok(Some(rows.into_iter().map(|h| Handoff { reported: true, ..h }).collect()))
    }

    /// The news handed to `target` it has not read, oldest first, marked read.
    pub fn take_news(&self, target: &str) -> Result<Vec<Handoff>> {
        let rows: Vec<Handoff> = {
            let mut stmt = self.conn.prepare(&format!(
                "SELECT {COLUMNS} FROM handoffs WHERE target = ?1 AND kind = 'news' AND seen = 0 ORDER BY id"
            ))?;
            stmt.query_map([target], handoff)?.collect::<rusqlite::Result<Vec<_>>>()?
        };
        for h in &rows {
            self.conn.execute("UPDATE handoffs SET seen = 1 WHERE id = ?1", [h.id])?;
        }
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new(kind: HandoffKind, target: &str, batch: &str, parent: Option<i64>) -> NewHandoff {
        NewHandoff {
            kind,
            sender: "assistant".into(),
            sender_name: "Assistant".into(),
            target: target.into(),
            target_name: target.into(),
            text: format!("việc cho {target}"),
            parent,
            batch: batch.into(),
            chain: vec!["assistant".into()],
        }
    }

    #[test]
    fn a_batch_comes_back_once_when_none_is_out() {
        let dir = tempfile::tempdir().unwrap();
        let life = Life::open_with_memory_db(dir.path()).unwrap();
        let batch = batch_of_prompt(9);
        let a = life.add_handoff(new(HandoffKind::Task, "fin", &batch, None)).unwrap();
        let b = life.add_handoff(new(HandoffKind::Ask, "coach", &batch, None)).unwrap();
        life.add_handoff(new(HandoffKind::News, "scribe", &batch, None)).unwrap();
        assert_eq!((a.status, a.chain.clone()), (HandoffStatus::Queued, vec!["assistant".to_owned()]));

        life.close_handoff(a.id, HandoffStatus::Done, "xong", "").unwrap().unwrap();
        assert!(life.claim_batch(&batch).unwrap().is_none(), "coach is still out");
        assert!(life.close_handoff(a.id, HandoffStatus::Failed, "", "").unwrap().is_none(), "closed once");

        life.close_handoff(b.id, HandoffStatus::Failed, "", "dừng").unwrap().unwrap();
        let claimed = life.claim_batch(&batch).unwrap().unwrap();
        let ids: Vec<i64> = claimed.iter().map(|h| h.id).collect();
        assert_eq!(ids, [a.id, b.id], "news comes back with nothing");
        assert!(life.claim_batch(&batch).unwrap().is_none(), "claimed once");
        assert_eq!(life.handoff(b.id).unwrap().note, "dừng");
    }

    #[test]
    fn news_is_read_once_and_children_hold_their_parent() {
        let dir = tempfile::tempdir().unwrap();
        let life = Life::open_with_memory_db(dir.path()).unwrap();
        let parent = life.add_handoff(new(HandoffKind::Task, "fin", &batch_of_prompt(1), None)).unwrap();
        let child = life.add_handoff(new(HandoffKind::Ask, "coach", &batch_of_handoff(parent.id), Some(parent.id))).unwrap();
        assert_eq!(life.open_children(parent.id).unwrap(), 1);
        life.close_handoff(child.id, HandoffStatus::Done, "ok", "").unwrap();
        assert_eq!(life.open_children(parent.id).unwrap(), 0);

        life.add_handoff(new(HandoffKind::News, "fin", &batch_of_prompt(1), None)).unwrap();
        assert_eq!(life.take_news("fin").unwrap().len(), 1);
        assert!(life.take_news("fin").unwrap().is_empty());

        assert_eq!(life.open_duplicate("assistant", "fin", " việc cho fin ").unwrap().map(|h| h.id), Some(parent.id));
        let listed = life.handoffs(true, Some("coach")).unwrap();
        assert!(listed.is_empty(), "coach's one is closed");
        assert_eq!(life.handed_by("assistant", "2000-01-01", 8).unwrap().len(), 2, "news left out");
    }
}
