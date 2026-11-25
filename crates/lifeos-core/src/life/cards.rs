//! Cards and their streams (docs/adr/0001, remit's `cards` and `events`): each agent works on one card, whose stream of
//! events is its conversation and, while prompts wait in it, its outbox. A queued prompt is taken highest priority
//! first, then oldest. What the owner is typing to an agent is its draft.

use anyhow::{Context, Result};
use chrono::{Local, SecondsFormat};
use rusqlite::{OptionalExtension, Row, params};

use super::Life;
use lifeos_wire::{Actor, Card, Draft, Event, EventKind, ImageRef};

/// Events one read of a stream returns, at most.
pub const EVENTS_PAGE: u32 = 400;

/// Who wrote a prompt and what it is, before it is stored.
#[derive(Debug, Clone, Default)]
pub struct NewEvent {
    pub card: String,
    pub kind: Option<EventKind>,
    pub status: String,
    pub tool_call_id: String,
    pub tool_kind: String,
    pub tool_name: String,
    pub title: String,
    pub text: String,
    pub by: Option<Actor>,
    pub priority: i64,
    pub data: String,
}

pub fn now() -> String {
    Local::now().to_rfc3339_opts(SecondsFormat::Millis, false)
}

fn kind_text(k: EventKind) -> &'static str {
    match k {
        EventKind::Message => "message",
        EventKind::Thought => "thought",
        EventKind::Tool => "tool",
        EventKind::Prompt => "prompt",
        EventKind::Stop => "stop",
        EventKind::System => "system",
        EventKind::Task => "task",
    }
}

fn kind_of(text: &str) -> rusqlite::Result<EventKind> {
    Ok(match text {
        "message" => EventKind::Message,
        "thought" => EventKind::Thought,
        "tool" => EventKind::Tool,
        "prompt" => EventKind::Prompt,
        "stop" => EventKind::Stop,
        "system" => EventKind::System,
        "task" => EventKind::Task,
        other => {
            return Err(rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                format!("unknown event kind {other:?}").into(),
            ));
        }
    })
}

const EVENT_COLUMNS: &str =
    "id, card, kind, status, tool_call_id, tool_kind, tool_name, title, text, by_actor, data, created, updated, rev";

fn event(r: &Row<'_>) -> rusqlite::Result<Event> {
    let by: Option<String> = r.get(9)?;
    Ok(Event {
        id: r.get(0)?,
        card: r.get(1)?,
        kind: kind_of(&r.get::<_, String>(2)?)?,
        status: r.get(3)?,
        tool_call_id: r.get(4)?,
        tool_kind: r.get(5)?,
        tool_name: r.get(6)?,
        title: r.get(7)?,
        text: r.get(8)?,
        by: by.and_then(|b| serde_json::from_str(&b).ok()),
        data: r.get(10)?,
        created: r.get(11)?,
        updated: r.get(12)?,
        rev: r.get(13)?,
    })
}

impl Life {
    pub fn card(&self, id: &str) -> Result<Option<Card>> {
        let doc: Option<String> =
            self.conn.query_row("SELECT doc FROM cards WHERE id = ?1", [id], |r| r.get(0)).optional()?;
        let Some(doc) = doc else { return Ok(None) };
        let mut card: Card = serde_json::from_str(&doc).with_context(|| format!("card {id}"))?;
        card.queued = self.queued(id)?;
        Ok(Some(card))
    }

    pub fn cards(&self) -> Result<Vec<Card>> {
        let mut stmt = self.conn.prepare("SELECT id FROM cards ORDER BY id")?;
        let ids = stmt.query_map([], |r| r.get::<_, String>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;
        ids.iter().filter_map(|id| self.card(id).transpose()).collect()
    }

    /// The card of agent `id`, made the first time it is asked for.
    pub fn ensure_card(&self, id: &str, name: &str) -> Result<Card> {
        if let Some(card) = self.card(id)? {
            return Ok(card);
        }
        let at = now();
        let card = Card { id: id.into(), name: name.into(), created: at.clone(), updated: at, ..Card::default() };
        self.conn.execute(
            "INSERT OR IGNORE INTO cards(id, doc) VALUES (?1, ?2)",
            params![id, serde_json::to_string(&card)?],
        )?;
        Ok(self.card(id)?.unwrap_or(card))
    }

    /// Stores card as it now is, counting the change.
    pub fn save_card(&self, card: &mut Card) -> Result<()> {
        card.rev += 1;
        card.updated = now();
        self.conn.execute(
            "INSERT INTO cards(id, doc) VALUES (?1, ?2) ON CONFLICT(id) DO UPDATE SET doc = excluded.doc",
            params![card.id, serde_json::to_string(card)?],
        )?;
        Ok(())
    }

    pub fn add_event(&self, e: NewEvent) -> Result<Event> {
        let at = now();
        let kind = e.kind.context("an event needs a kind")?;
        self.conn.execute(
            "INSERT INTO events(card, kind, status, tool_call_id, tool_kind, tool_name, title, text, by_actor, priority,
                                data, created, updated)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12)",
            params![
                e.card,
                kind_text(kind),
                e.status,
                e.tool_call_id,
                e.tool_kind,
                e.tool_name,
                e.title,
                e.text,
                e.by.as_ref().map(serde_json::to_string).transpose()?,
                e.priority,
                e.data,
                at,
            ],
        )?;
        self.event(self.conn.last_insert_rowid())
    }

    pub fn event(&self, id: i64) -> Result<Event> {
        Ok(self.conn.query_row(&format!("SELECT {EVENT_COLUMNS} FROM events WHERE id = ?1"), [id], event)?)
    }

    /// Stores what an event now says (status, title, text, data), counting the change.
    pub fn update_event(&self, e: &Event) -> Result<Event> {
        self.conn.execute(
            "UPDATE events SET status = ?2, tool_kind = ?3, tool_name = ?4, title = ?5, text = ?6, data = ?7,
                               updated = ?8, rev = rev + 1
             WHERE id = ?1",
            params![e.id, e.status, e.tool_kind, e.tool_name, e.title, e.text, e.data, now()],
        )?;
        self.event(e.id)
    }

    /// The tool call `call_id` of card's stream, newest first.
    pub fn tool_event(&self, card: &str, call_id: &str) -> Result<Option<Event>> {
        Ok(self
            .conn
            .query_row(
                &format!(
                    "SELECT {EVENT_COLUMNS} FROM events WHERE card = ?1 AND kind = 'tool' AND tool_call_id = ?2
                     ORDER BY id DESC LIMIT 1"
                ),
                params![card, call_id],
                event,
            )
            .optional()?)
    }

    /// The newest events of card's stream, or those before event `before`, oldest first.
    pub fn events(&self, card: &str, before: Option<i64>, limit: u32) -> Result<Vec<Event>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT * FROM (SELECT {EVENT_COLUMNS} FROM events WHERE card = ?1 AND id < ?2 ORDER BY id DESC LIMIT ?3)
             ORDER BY id"
        ))?;
        let rows = stmt.query_map(params![card, before.unwrap_or(i64::MAX), limit], event)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// The events of card's stream after event `after`, oldest first.
    pub fn events_after(&self, card: &str, after: i64) -> Result<Vec<Event>> {
        let mut stmt =
            self.conn.prepare(&format!("SELECT {EVENT_COLUMNS} FROM events WHERE card = ?1 AND id > ?2 ORDER BY id"))?;
        let rows = stmt.query_map(params![card, after], event)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// How many prompts wait in card's outbox.
    pub fn queued(&self, card: &str) -> Result<i64> {
        Ok(self.conn.query_row(
            "SELECT COUNT(*) FROM events WHERE card = ?1 AND kind = 'prompt' AND status = 'queued'",
            [card],
            |r| r.get(0),
        )?)
    }

    /// Takes the next prompt of card's outbox (highest priority, then oldest), marking it delivered.
    pub fn take_prompt(&self, card: &str) -> Result<Option<Event>> {
        let id: Option<i64> = self
            .conn
            .query_row(
                "SELECT id FROM events WHERE card = ?1 AND kind = 'prompt' AND status = 'queued'
                 ORDER BY priority DESC, id LIMIT 1",
                [card],
                |r| r.get(0),
            )
            .optional()?;
        let Some(id) = id else { return Ok(None) };
        self.conn.execute(
            "UPDATE events SET status = 'delivered', updated = ?2, rev = rev + 1 WHERE id = ?1",
            params![id, now()],
        )?;
        Ok(Some(self.event(id)?))
    }

    /// Marks every queued prompt of card dropped: the session they waited for ended for good.
    pub fn drop_queued(&self, card: &str) -> Result<Vec<Event>> {
        let ids: Vec<i64> = {
            let mut stmt =
                self.conn.prepare("SELECT id FROM events WHERE card = ?1 AND kind = 'prompt' AND status = 'queued'")?;
            stmt.query_map([card], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?
        };
        ids.into_iter()
            .map(|id| {
                self.conn.execute(
                    "UPDATE events SET status = 'dropped', updated = ?2, rev = rev + 1 WHERE id = ?1",
                    params![id, now()],
                )?;
                self.event(id)
            })
            .collect()
    }

    /// The tool calls of card still open: they end with their turn.
    pub fn open_tool_calls(&self, card: &str) -> Result<Vec<Event>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {EVENT_COLUMNS} FROM events
             WHERE card = ?1 AND kind = 'tool' AND status IN ('pending', 'in_progress') ORDER BY id"
        ))?;
        let rows = stmt.query_map([card], event)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn draft(&self, card: &str) -> Result<Draft> {
        let found = self
            .conn
            .query_row("SELECT text, images, rev FROM drafts WHERE card = ?1", [card], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?))
            })
            .optional()?;
        Ok(match found {
            Some((text, images, rev)) => {
                Draft { card: card.into(), text, images: serde_json::from_str(&images).unwrap_or_default(), rev }
            }
            None => Draft { card: card.into(), ..Draft::default() },
        })
    }

    /// Keeps what the owner is typing to card, refusing images that were not put (`check_images`).
    pub fn set_draft(&self, card: &str, text: &str, images: &[ImageRef]) -> Result<Draft> {
        self.check_images(images)?;
        self.conn.execute(
            "INSERT INTO drafts(card, text, images, rev) VALUES (?1, ?2, ?3, 1)
             ON CONFLICT(card) DO UPDATE SET text = excluded.text, images = excluded.images, rev = rev + 1",
            params![card, text, serde_json::to_string(images)?],
        )?;
        self.draft(card)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lifeos_wire::ActorKind;

    fn owner() -> Actor {
        Actor { kind: ActorKind::Owner, id: String::new(), name: "You".into() }
    }

    fn prompt(card: &str, text: &str, priority: i64) -> NewEvent {
        NewEvent {
            card: card.into(),
            kind: Some(EventKind::Prompt),
            status: "queued".into(),
            text: text.into(),
            by: Some(owner()),
            priority,
            ..NewEvent::default()
        }
    }

    #[test]
    fn the_outbox_gives_the_highest_priority_then_the_oldest() {
        let (_dir, life) = crate::life::test_support::life();
        life.ensure_card("assistant", "Assistant").unwrap();
        life.add_event(prompt("assistant", "một", 1)).unwrap();
        life.add_event(prompt("assistant", "hai", 1)).unwrap();
        life.add_event(prompt("assistant", "mở đầu", 3)).unwrap();
        assert_eq!(life.card("assistant").unwrap().unwrap().queued, 3);
        let taken: Vec<String> =
            std::iter::from_fn(|| life.take_prompt("assistant").unwrap()).map(|e| e.text).collect();
        assert_eq!(taken, ["mở đầu", "một", "hai"]);
        let stream = life.events("assistant", None, EVENTS_PAGE).unwrap();
        assert!(stream.iter().all(|e| e.status == "delivered" && e.by == Some(owner())));
    }
}
