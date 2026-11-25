//! Teachings: what the owner taught an agent, which the librarian reads and writes memory from (remit's ADR-0117).
//! A turn on the owner's words teaches its agent (`teach_turn`, role `teaching`); the owner overturning a decision
//! teaches the Assistant (`teach_text`); a memory that misled more than it helped is put to review (role `review`).
//!
//! The rails are here, in code, not in the librarian's brief: a teaching writes only its holder's memory; evidence is
//! required; a teaching settled writes nothing; the same claim from the same teaching is written once; a memory the
//! holder does not hold is never ended by it (a correction is written beside it, contradicting it); only a review
//! retracts, and never what the owner confirmed.

use anyhow::{Result, bail};
use rusqlite::{OptionalExtension, Row, params};

use super::Life;
use super::memory::{LinkKind, MemoryKind, Remember, SUPERSEDES};
use lifeos_wire::{ActorKind, EventKind, Memory, Teaching};

const COLUMNS: &str =
    "id, holder, card, role, source, teacher, from_event, to_event, text, status, note, written, created, settled";

/// What `read_teaching` gives of a stream, at most, per message.
const READ_CHARS: usize = 4000;

/// How long a memory's claim may be.
const TITLE_CHARS: usize = 200;

pub const ROLE_TEACHING: &str = "teaching";
pub const ROLE_REVIEW: &str = "review";

fn teaching(r: &Row) -> rusqlite::Result<Teaching> {
    let written: String = r.get(11)?;
    Ok(Teaching {
        id: r.get(0)?,
        holder: r.get(1)?,
        card: r.get(2)?,
        role: r.get(3)?,
        source: r.get(4)?,
        teacher: r.get(5)?,
        from_event: r.get(6)?,
        to_event: r.get(7)?,
        text: r.get(8)?,
        status: r.get(9)?,
        note: r.get(10)?,
        written: serde_json::from_str(&written).unwrap_or_default(),
        created: r.get(12)?,
        settled: r.get(13)?,
    })
}

input! {
    pub struct ListTeachings {
        /// `assistant` or a crew member id. Omit for everyone.
        pub holder: Option<String>,
    }
}

input! {
    pub struct Learn {
        /// The teaching this memory comes from.
        pub teaching: i64,
        pub kind: MemoryKind,
        /// One line, the claim itself.
        pub title: String,
        /// Why, the details, the context. Short.
        #[serde(default)]
        pub body: String,
        /// The teaching's words it rests on, quoted.
        pub evidence: String,
        /// Ids of the holder's memories it corrects: each ended, superseded by this one; one the holder does not hold
        /// stands, contradicted by it.
        #[serde(default)]
        pub supersedes: Vec<i64>,
        /// Its links to memories the holder may recall.
        #[serde(default)]
        pub links: Vec<super::memory::LinkTo>,
        /// Memories found close to it that say something else: named, so it is not refused as the same.
        #[serde(default)]
        pub distinct_from: Vec<i64>,
    }
}

impl Life {
    /// The owner's turn with agent `card` (events `from`..=`to` of its stream) teaches `holder`.
    pub fn teach_turn(&self, holder: &str, card: &str, from: i64, to: i64, source: &str) -> Result<Teaching> {
        self.conn.execute(
            "INSERT INTO teachings(holder, card, role, source, teacher, from_event, to_event, created)
             VALUES (?1, ?2, 'teaching', ?3, 'owner', ?4, ?5, ?6)",
            params![holder, card, source, from, to, super::cards::now()],
        )?;
        self.teaching(self.conn.last_insert_rowid())
    }

    /// A teaching of `holder` in words alone: an overturn, or a review.
    pub fn teach_text(&self, holder: &str, role: &str, source: &str, teacher: &str, text: &str) -> Result<Teaching> {
        self.conn.execute(
            "INSERT INTO teachings(holder, role, source, teacher, text, created) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![holder, role, source, teacher, text, super::cards::now()],
        )?;
        self.teaching(self.conn.last_insert_rowid())
    }

    pub fn teaching(&self, id: i64) -> Result<Teaching> {
        self.conn
            .query_row(&format!("SELECT {COLUMNS} FROM teachings WHERE id = ?1"), [id], teaching)
            .optional()?
            .ok_or_else(|| anyhow::anyhow!("no teaching {id}; next_teachings lists those that wait"))
    }

    /// The teachings that wait for the librarian, oldest first.
    pub fn open_teachings(&self, limit: u32) -> Result<Vec<Teaching>> {
        let mut stmt =
            self.conn.prepare(&format!("SELECT {COLUMNS} FROM teachings WHERE status = 'open' ORDER BY id LIMIT ?1"))?;
        Ok(stmt.query_map([limit], teaching)?.collect::<rusqlite::Result<_>>()?)
    }

    /// The newest teachings, of `holder` when given.
    pub fn teachings(&self, holder: Option<&str>, limit: u32) -> Result<Vec<Teaching>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {COLUMNS} FROM teachings WHERE (?1 IS NULL OR holder = ?1) ORDER BY id DESC LIMIT ?2"
        ))?;
        Ok(stmt.query_map(params![holder, limit], teaching)?.collect::<rusqlite::Result<_>>()?)
    }

    fn open_teaching(&self, id: i64) -> Result<Teaching> {
        let t = self.teaching(id)?;
        if t.status != "open" {
            bail!("teaching {id} is settled: it writes nothing more");
        }
        Ok(t)
    }

    /// What teaching `id` holds, as the librarian reads it: its text, or its turn's conversation, the owner's words and
    /// the agent's answer.
    pub fn read_teaching(&self, id: i64) -> Result<String> {
        let t = self.teaching(id)?;
        let mut out = format!("Teaching {} of {} ({}): {}\n", t.id, t.holder, t.role, t.source);
        if t.from_event == 0 {
            out.push('\n');
            out.push_str(&t.text);
            return Ok(out);
        }
        let cut = |s: &str| -> String {
            match s.char_indices().nth(READ_CHARS) {
                Some((i, _)) => format!("{}…", s.get(..i).unwrap_or(s)),
                None => s.to_owned(),
            }
        };
        for e in self.events_after(&t.card, t.from_event.saturating_sub(1))? {
            if e.id > t.to_event {
                break;
            }
            let who = match (&e.kind, &e.by) {
                (EventKind::Prompt, Some(by)) if by.kind == ActorKind::Owner => "The owner said".to_owned(),
                (EventKind::Prompt, Some(by)) => format!("{} said", by.name),
                (EventKind::Message, _) => "The agent answered".to_owned(),
                (EventKind::Tool, _) if e.status == "completed" => {
                    out.push_str(&format!("\n[the agent ran {}: {}]\n", e.tool_name, e.title));
                    continue;
                }
                _ => continue,
            };
            if !e.text.trim().is_empty() {
                out.push_str(&format!("\n{who}:\n{}\n", cut(e.text.trim())));
            }
        }
        Ok(out)
    }

    /// Writes a memory teaching `l.teaching` teaches, its holder's, within the rails. Written twice, it is the memory
    /// already written.
    pub fn learn(&self, l: Learn) -> Result<Memory> {
        let t = self.open_teaching(l.teaching)?;
        let title = l.title.trim();
        if title.is_empty() || title.contains('\n') || title.chars().count() > TITLE_CHARS {
            bail!("the title is the claim itself, one line of at most {TITLE_CHARS} characters");
        }
        if l.evidence.trim().is_empty() {
            bail!("quote the teaching's words this memory rests on (evidence)");
        }
        let again: Option<i64> = self
            .conn
            .query_row("SELECT id FROM memories WHERE source = ?1 AND title = ?2", params![t.id, title], |r| r.get(0))
            .optional()?;
        if let Some(id) = again {
            return self.memory(id);
        }
        let readable: Vec<i64> = self.readable_memories(&t.holder)?.iter().map(|m| m.id).collect();
        for id in l.links.iter().map(|k| k.id).chain(l.supersedes.iter().copied()) {
            if !readable.contains(&id) {
                bail!("memory {id} is not one {} may recall", t.holder);
            }
        }
        let (own, beside): (Vec<i64>, Vec<i64>) =
            l.supersedes.iter().partition(|id| self.memory(**id).is_ok_and(|m| m.holder == t.holder));
        let mut links = l.links.clone();
        links.extend(beside.iter().map(|id| super::memory::LinkTo { id: *id, kind: LinkKind::Contradicts }));
        let m = self.remember(
            &t.holder,
            Remember { kind: l.kind, title: title.to_owned(), body: l.body.trim().to_owned(), replaces: None, links },
        )?;
        for old in &own {
            self.end_memory(&t.holder, *old, &format!("thay bằng {}: {title}", m.id))?;
            self.conn.execute(
                "INSERT OR IGNORE INTO memory_links(src, dst, kind) VALUES (?1, ?2, ?3)",
                params![m.id, old, SUPERSEDES],
            )?;
        }
        self.conn.execute(
            "UPDATE memories SET source = ?2, evidence = ?3 WHERE id = ?1",
            params![m.id, t.id, l.evidence.trim()],
        )?;
        let mut written = t.written.clone();
        written.push(m.id);
        self.conn
            .execute("UPDATE teachings SET written = ?2 WHERE id = ?1", params![t.id, serde_json::to_string(&written)?])?;
        self.memory(m.id)
    }

    /// Links memory `from`, the teaching's holder's own, to `to`, one it may recall.
    pub fn link_memories(&self, teaching: i64, from: i64, to: i64, kind: LinkKind) -> Result<()> {
        let t = self.open_teaching(teaching)?;
        if self.memory(from)?.holder != t.holder {
            bail!("memory {from} is not {}'s own: link from one this teaching may change", t.holder);
        }
        if !self.readable_memories(&t.holder)?.iter().any(|m| m.id == to) {
            bail!("memory {to} is not one {} may recall", t.holder);
        }
        self.conn.execute(
            "INSERT OR IGNORE INTO memory_links(src, dst, kind) VALUES (?1, ?2, ?3)",
            params![from, to, kind.as_str()],
        )?;
        Ok(())
    }

    /// Under a review, stops a memory of the holder's own from being recalled, saying why. Never one the owner confirmed.
    pub fn retract_memory(&self, teaching: i64, id: i64, why: &str) -> Result<()> {
        let t = self.open_teaching(teaching)?;
        if t.role != ROLE_REVIEW {
            bail!("only a review retracts a memory; a teaching corrects one with `supersedes`");
        }
        let m = self.memory(id)?;
        if m.holder != t.holder {
            bail!("memory {id} is not {}'s own", t.holder);
        }
        if m.trust == "confirmed" {
            bail!("the owner confirmed memory {id}: only they take it back");
        }
        if why.trim().is_empty() {
            bail!("say why it no longer holds");
        }
        self.end_memory(&t.holder, id, why.trim())
    }

    /// Settles a teaching: the note says which memories it wrote, or why none.
    pub fn settle_teaching(&self, id: i64, note: &str) -> Result<Teaching> {
        self.open_teaching(id)?;
        if note.trim().is_empty() {
            bail!("say what it taught, or why nothing");
        }
        self.conn.execute(
            "UPDATE teachings SET status = 'settled', note = ?2, settled = ?3 WHERE id = ?1",
            params![id, note.trim(), super::cards::now()],
        )?;
        self.teaching(id)
    }

    /// The memory teaching `teaching` already wrote with title, if it did.
    pub fn memories_written_by(&self, teaching: i64, title: &str) -> Result<Option<i64>> {
        Ok(self
            .conn
            .query_row("SELECT id FROM memories WHERE source = ?1 AND title = ?2", params![teaching, title], |r| r.get(0))
            .optional()?)
    }

    /// Puts a memory that misled more than it helped to review, once while one waits.
    pub fn review_if_misleading(&self, id: i64) -> Result<Option<Teaching>> {
        let m = self.memory(id)?;
        if m.misled <= m.helped || m.ended.is_some() {
            return Ok(None);
        }
        let source = format!("memory {id} misled more than it helped");
        let waiting: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM teachings WHERE status = 'open' AND role = 'review' AND source = ?1)",
            [&source],
            |r| r.get(0),
        )?;
        if waiting {
            return Ok(None);
        }
        let text = format!(
            "Memory {id} ({kind}): {title}\n{body}\n\nAgents said it misled them {misled} time(s) and helped {helped}. Check \
             whether it still holds: correct it (supersedes), retract it, or settle saying why it stands.",
            kind = m.kind,
            title = m.title,
            body = m.body,
            misled = m.misled,
            helped = m.helped,
        );
        self.teach_text(&m.holder, ROLE_REVIEW, &source, "", &text).map(Some)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::life::cards::NewEvent;
    use crate::life::memory::{LinkTo, RateMemory};
    use crate::life::test_support::life;
    use lifeos_wire::{ASSISTANT, Actor};

    fn learn(teaching: i64, title: &str) -> Learn {
        Learn {
            teaching,
            kind: MemoryKind::Preference,
            title: title.into(),
            body: String::new(),
            evidence: "\"đừng nhắc cuối tuần\"".into(),
            supersedes: vec![],
            links: vec![],
            distinct_from: vec![],
        }
    }

    #[test]
    fn a_turn_teaches_its_holder_within_the_rails() {
        let (_dir, life) = life();
        life.ensure_card(ASSISTANT, "Assistant").unwrap();
        let owner = Actor { kind: ActorKind::Owner, id: String::new(), name: "You".into() };
        let said = life
            .add_event(NewEvent {
                card: ASSISTANT.into(),
                kind: Some(EventKind::Prompt),
                status: "delivered".into(),
                text: "Đừng nhắc việc vào cuối tuần".into(),
                by: Some(owner),
                ..NewEvent::default()
            })
            .unwrap();
        let answer = life
            .add_event(NewEvent {
                card: ASSISTANT.into(),
                kind: Some(EventKind::Message),
                text: "Đã hiểu.".into(),
                ..NewEvent::default()
            })
            .unwrap();
        let t = life.teach_turn(ASSISTANT, ASSISTANT, said.id, answer.id, "a conversation").unwrap();
        let read = life.read_teaching(t.id).unwrap();
        assert!(read.contains("The owner said:\nĐừng nhắc việc vào cuối tuần"), "{read}");
        assert!(read.contains("The agent answered:\nĐã hiểu."), "{read}");

        assert!(life.learn(Learn { evidence: " ".into(), ..learn(t.id, "Không nhắc cuối tuần") }).is_err());
        assert!(life.learn(learn(t.id, "dòng một\ndòng hai")).is_err());
        let m = life.learn(learn(t.id, "Không nhắc việc vào cuối tuần")).unwrap();
        assert_eq!((m.holder.as_str(), m.source), (ASSISTANT, Some(t.id)));
        assert_eq!(life.learn(learn(t.id, "Không nhắc việc vào cuối tuần")).unwrap().id, m.id, "written once");

        // A member's memory is not the Assistant's to end: corrected beside it, contradicted.
        let hlv = life.remember("hlv", Remember {
            kind: MemoryKind::Fact,
            title: "Chạy tối".into(),
            body: String::new(),
            replaces: None,
            links: vec![],
        });
        let hlv = hlv.unwrap();
        assert!(life.learn(Learn { supersedes: vec![hlv.id], ..learn(t.id, "x") }).is_err(), "not one it may recall");

        let fixed = life.learn(Learn { supersedes: vec![m.id], ..learn(t.id, "Chỉ nhắc cuối tuần khi gấp") }).unwrap();
        assert!(life.memory(m.id).unwrap().ended.is_some());
        assert!(fixed.links.iter().any(|l| l.to == m.id && l.kind == SUPERSEDES));
        assert!(life.retract_memory(t.id, fixed.id, "sai").is_err(), "only a review retracts");

        let settled = life.settle_teaching(t.id, &format!("wrote {}, {}", m.id, fixed.id)).unwrap();
        assert_eq!(settled.written, [m.id, fixed.id]);
        assert!(life.learn(learn(t.id, "Thêm")).is_err(), "a settled teaching writes nothing");
        assert!(life.open_teachings(10).unwrap().is_empty());
    }

    #[test]
    fn a_memory_that_misleads_goes_to_review_once() {
        let (_dir, life) = life();
        let m = life
            .remember(ASSISTANT, Remember {
                kind: MemoryKind::Fact,
                title: "Mẹ thích hoa lan".into(),
                body: String::new(),
                replaces: None,
                links: vec![LinkTo { id: 999, kind: LinkKind::Related }],
            })
            .unwrap();
        assert!(life.review_if_misleading(m.id).unwrap().is_none());
        life.rate_memory(ASSISTANT, RateMemory { id: m.id, helped: false }).unwrap();
        let r = life.review_if_misleading(m.id).unwrap().unwrap();
        assert_eq!((r.role.as_str(), r.holder.as_str()), (ROLE_REVIEW, ASSISTANT));
        assert!(life.review_if_misleading(m.id).unwrap().is_none(), "once while one waits");
        life.retract_memory(r.id, m.id, "mẹ đổi ý").unwrap();
        assert!(life.memory(m.id).unwrap().ended.is_some());
    }
}
