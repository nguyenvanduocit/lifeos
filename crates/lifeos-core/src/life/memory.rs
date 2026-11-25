//! Memory: what agents remember across conversations, in `memories` (with a
//! full-text index). Each memory has a holder: the Assistant, or a crew
//! member. The Assistant's memories are about the user and are shared with
//! the whole crew; a member's own memories are its alone.
//!
//! Agents write memories themselves (`remember`, `forget`). Before each
//! message, lifeos recalls the few that bear on it and puts them in front of
//! the message, so nothing depends on the agent searching. How they are ranked
//! (words, meaning, links, a judge) is lifeos-agent's `recall`, as remit's
//! ADR-0117 has it; this keeps the record, its use, its vectors and its links.

use anyhow::{Result, bail};
use chrono::Local;
use rusqlite::{OptionalExtension, Row, params};

use super::Life;
use lifeos_wire::{ASSISTANT, Memory, MemoryLink};

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
        /// Ids of memories this one rests on, relates to, or contradicts.
        #[serde(default)]
        pub links: Vec<LinkTo>,
    }
}

choice! {
    pub enum LinkKind {
        /// It rests on the other: the other is its reason or evidence.
        Because => "because",
        /// It bears on the other.
        Related => "related",
        /// It says the other is wrong, and both stand until one is forgotten.
        Contradicts => "contradicts",
    }
}

input! {
    pub struct LinkTo {
        pub id: i64,
        pub kind: LinkKind,
    }
}

input! {
    pub struct RateMemory {
        /// The memory's id, from the `<recalled>` note or `recall`.
        pub id: i64,
        /// It helped with what you did; false: it misled you.
        pub helped: bool,
    }
}

/// The link a replacing memory holds to the one it replaces.
pub const SUPERSEDES: &str = "supersedes";

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

const COLUMNS: &str = "m.id, m.holder, m.kind, m.title, m.body, m.created, m.updated, m.ended, m.why_ended, m.recalled, \
                       m.trust, COALESCE(u.helped, 0), COALESCE(u.misled, 0), u.last_recalled, m.source, m.evidence";

/// What every read of memories joins: their use, counted beside them.
const USE: &str = "LEFT JOIN memory_use u ON u.memory = m.id";

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
        trust: r.get(10)?,
        helped: r.get(11)?,
        misled: r.get(12)?,
        last_recalled: r.get(13)?,
        links: Vec::new(),
        source: r.get(14)?,
        evidence: r.get(15)?,
    })
}

/// What a memory's meaning is read from: the text its vector is made of and the reranker judges (remit's
/// `Memory::passage`). Pure.
pub fn passage(m: &Memory) -> String {
    format!("{}: {}. {}", m.kind, m.title.trim(), m.body.trim())
}

/// A short, stable fingerprint of a passage: a vector is kept only while it matches. Pure.
pub fn passage_hash(passage: &str) -> String {
    // FNV-1a, 64 bits: enough to tell a changed passage, not a secret.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in passage.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")
}

fn vector_bytes(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}

fn vector_of(bytes: &[u8]) -> Vec<f32> {
    bytes.as_chunks::<4>().0.iter().map(|c| f32::from_le_bytes(*c)).collect()
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
        if let Some(old) = r.replaces {
            self.link(id, old, SUPERSEDES)?;
        }
        for l in &r.links {
            if l.id != id && self.memory(l.id).is_ok() {
                self.link(id, l.id, l.kind.as_str())?;
            }
        }
        self.memory(id)
    }

    fn link(&self, src: i64, dst: i64, kind: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO memory_links(src, dst, kind) VALUES (?1, ?2, ?3)",
            params![src, dst, kind],
        )?;
        Ok(())
    }

    /// The links between memories, each as (from, to, kind).
    pub fn memory_links(&self) -> Result<Vec<(i64, i64, String)>> {
        let mut stmt = self.conn.prepare("SELECT src, dst, kind FROM memory_links")?;
        Ok(stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<rusqlite::Result<_>>()?)
    }

    fn with_links(&self, mut ms: Vec<Memory>) -> Result<Vec<Memory>> {
        let mut stmt = self.conn.prepare("SELECT dst, kind FROM memory_links WHERE src = ?1 ORDER BY dst")?;
        for m in &mut ms {
            m.links = stmt
                .query_map([m.id], |r| Ok(MemoryLink { to: r.get(0)?, kind: r.get(1)? }))?
                .collect::<rusqlite::Result<_>>()?;
        }
        Ok(ms)
    }

    /// What `holder` may recall: its own active memories, and for a crew member the Assistant's too.
    pub fn readable_memories(&self, holder: &str) -> Result<Vec<Memory>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {COLUMNS} FROM memories m {USE} WHERE m.ended IS NULL AND m.holder IN (?1, '{ASSISTANT}')"
        ))?;
        let rows = stmt.query_map([holder], memory)?.collect::<rusqlite::Result<Vec<_>>>()?;
        self.with_links(rows)
    }

    /// The ids of what `holder` may recall whose words match `text`, best first.
    pub fn memory_words(&self, holder: &str, text: &str, limit: u32) -> Result<Vec<i64>> {
        Ok(self.recall(holder, text, limit)?.into_iter().map(|m| m.id).collect())
    }

    /// The vectors under `model` of what `holder` may recall.
    pub fn memory_vectors(&self, holder: &str, model: &str) -> Result<Vec<(i64, Vec<f32>)>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT m.id, e.vector FROM memory_embeddings e JOIN memories m ON m.id = e.memory
             WHERE e.model = ?2 AND m.ended IS NULL AND m.holder IN (?1, '{ASSISTANT}')"
        ))?;
        let rows = stmt.query_map(params![holder, model], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Vec<u8>>(1)?)))?;
        Ok(rows.map(|r| r.map(|(id, b)| (id, vector_of(&b)))).collect::<rusqlite::Result<_>>()?)
    }

    /// Active memories with no vector under `model` for their passage as it now is, and none refused lately: each with
    /// its passage and the passage's hash.
    pub fn unembedded(&self, model: &str, limit: u32) -> Result<Vec<(i64, String, String)>> {
        let mut stmt = self.conn.prepare(&format!("SELECT {COLUMNS} FROM memories m {USE} WHERE m.ended IS NULL ORDER BY m.id"))?;
        let all = stmt.query_map([], memory)?.collect::<rusqlite::Result<Vec<_>>>()?;
        let today = now();
        let mut out = Vec::new();
        for m in all {
            let p = passage(&m);
            let hash = passage_hash(&p);
            let held: Option<String> = self
                .conn
                .query_row("SELECT hash FROM memory_embeddings WHERE memory = ?1 AND model = ?2", params![m.id, model], |r| {
                    r.get(0)
                })
                .optional()?;
            let refused: Option<String> = self
                .conn
                .query_row(
                    "SELECT until FROM memory_unembedded WHERE memory = ?1 AND model = ?2 AND hash = ?3",
                    params![m.id, model, hash],
                    |r| r.get(0),
                )
                .optional()?;
            if held.as_deref() != Some(hash.as_str()) && refused.is_none_or(|until| until < today) {
                out.push((m.id, p, hash));
                if out.len() >= limit as usize {
                    break;
                }
            }
        }
        Ok(out)
    }

    /// Keeps `vector` as memory `id`'s under `model`, only while `hash` is still its passage's: a memory changed or
    /// ended meanwhile keeps nothing. Whether it was kept.
    pub fn keep_vector(&self, id: i64, model: &str, hash: &str, vector: &[f32]) -> Result<bool> {
        let Ok(m) = self.memory(id) else { return Ok(false) };
        if m.ended.is_some() || passage_hash(&passage(&m)) != hash {
            return Ok(false);
        }
        self.conn.execute(
            "INSERT INTO memory_embeddings(memory, model, hash, vector) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(memory, model) DO UPDATE SET hash = excluded.hash, vector = excluded.vector",
            params![id, model, hash, vector_bytes(vector)],
        )?;
        Ok(true)
    }

    /// Leaves a passage the model refused alone for a day.
    pub fn refused_vector(&self, id: i64, model: &str, hash: &str) -> Result<()> {
        let until = (Local::now() + chrono::Duration::days(1)).format("%Y-%m-%d %H:%M").to_string();
        self.conn.execute(
            "INSERT INTO memory_unembedded(memory, model, hash, until) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(memory, model) DO UPDATE SET hash = excluded.hash, until = excluded.until",
            params![id, model, hash, until],
        )?;
        Ok(())
    }

    /// Counts memories as given to a conversation.
    pub fn count_given(&self, ids: &[i64]) -> Result<()> {
        let at = now();
        for id in ids {
            self.conn.execute("UPDATE memories SET recalled = recalled + 1 WHERE id = ?1", [id])?;
            self.conn.execute(
                "INSERT INTO memory_use(memory, last_recalled) VALUES (?1, ?2)
                 ON CONFLICT(memory) DO UPDATE SET last_recalled = excluded.last_recalled",
                params![id, at],
            )?;
        }
        Ok(())
    }

    /// What agent `holder` said of a memory it was given: it helped, or it misled. Any memory it may recall.
    pub fn rate_memory(&self, holder: &str, r: RateMemory) -> Result<Memory> {
        let m = self.memory(r.id)?;
        if m.holder != holder && m.holder != ASSISTANT {
            bail!("memory {} is not one you recall", r.id);
        }
        let column = if r.helped { "helped" } else { "misled" };
        self.conn.execute(
            &format!(
                "INSERT INTO memory_use(memory, {column}) VALUES (?1, 1)
                 ON CONFLICT(memory) DO UPDATE SET {column} = {column} + 1"
            ),
            [r.id],
        )?;
        self.memory(r.id)
    }

    pub fn forget(&self, holder: &str, f: Forget) -> Result<()> {
        self.end_memory(holder, f.id, f.why.trim())
    }

    /// The Assistant may end anyone's memory (it acts for the user); a crew
    /// member only its own.
    pub(crate) fn end_memory(&self, holder: &str, id: i64, why: &str) -> Result<()> {
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
        let m = self.conn.query_row(&format!("SELECT {COLUMNS} FROM memories m {USE} WHERE m.id = ?1"), [id], memory)?;
        Ok(self.with_links(vec![m])?.remove(0))
    }

    /// What `holder` can draw on: its own memories, and for a crew member
    /// the Assistant's too. Best matches first.
    pub fn recall(&self, holder: &str, query: &str, limit: u32) -> Result<Vec<Memory>> {
        let q = any_of(query);
        if q.is_empty() {
            return Ok(vec![]);
        }
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {COLUMNS} FROM memories_fts f JOIN memories m ON m.id = f.rowid {USE}
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
        self.count_given(&found.iter().map(|m| m.id).collect::<Vec<_>>())?;
        Ok(found)
    }

    pub fn memories(&self, q: &ListMemories) -> Result<Vec<Memory>> {
        let ended = if q.include_forgotten { "1" } else { "m.ended IS NULL" };
        let (sql, words) = match q.query.as_deref().map(any_of).filter(|w| !w.is_empty()) {
            Some(words) => (
                format!(
                    "SELECT {COLUMNS} FROM memories_fts f JOIN memories m ON m.id = f.rowid {USE}
                     WHERE memories_fts MATCH ?2 AND (?1 IS NULL OR m.holder = ?1) AND {ended}
                     ORDER BY bm25(memories_fts) LIMIT 200"
                ),
                Some(words),
            ),
            None => (
                format!(
                    "SELECT {COLUMNS} FROM memories m {USE} WHERE (?1 IS NULL OR m.holder = ?1) AND {ended}
                     AND (?2 IS NULL OR 1) ORDER BY m.ended IS NOT NULL, m.updated DESC, m.id DESC LIMIT 500"
                ),
                None,
            ),
        };
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params![q.holder, words], memory)?.collect::<rusqlite::Result<Vec<_>>>()?;
        self.with_links(rows)
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
        Remember { kind: MemoryKind::Fact, title: title.into(), body: body.into(), replaces: None, links: vec![] }
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

    #[test]
    fn use_vectors_and_links_are_kept_beside_the_memory() {
        let (_dir, life) = life();
        let a = life.remember(ASSISTANT, fact("Mẹ thích hoa lan", "")).unwrap();
        let b = life
            .remember(ASSISTANT, Remember {
                links: vec![LinkTo { id: a.id, kind: LinkKind::Because }],
                ..fact("Sinh nhật mẹ tặng lan", "")
            })
            .unwrap();
        assert_eq!(b.links, [MemoryLink { to: a.id, kind: "because".into() }]);

        life.count_given(&[a.id]).unwrap();
        life.rate_memory("hlv", RateMemory { id: a.id, helped: true }).unwrap();
        let a2 = life.rate_memory(ASSISTANT, RateMemory { id: a.id, helped: false }).unwrap();
        assert_eq!((a2.recalled, a2.helped, a2.misled, a2.trust.as_str()), (1, 1, 1, "auto"));
        assert!(a2.last_recalled.is_some());

        let todo = life.unembedded("m", 10).unwrap();
        assert_eq!(todo.iter().map(|t| t.0).collect::<Vec<_>>(), [a.id, b.id]);
        let (id, _, hash) = todo[0].clone();
        assert!(life.keep_vector(id, "m", &hash, &[0.5, -1.0]).unwrap());
        assert!(!life.keep_vector(b.id, "m", "stale", &[1.0]).unwrap());
        life.refused_vector(b.id, "m", &todo[1].2).unwrap();
        assert!(life.unembedded("m", 10).unwrap().is_empty());
        assert_eq!(life.memory_vectors("hlv", "m").unwrap(), [(a.id, vec![0.5, -1.0])]);
        assert!(life.memory_vectors("hlv", "other").unwrap().is_empty());

        let c = life.remember(ASSISTANT, Remember { replaces: Some(a.id), ..fact("Mẹ thích hoa hồng", "") }).unwrap();
        assert!(life.memory_links().unwrap().contains(&(c.id, a.id, SUPERSEDES.to_owned())));
        assert!(life.memory_vectors("hlv", "m").unwrap().is_empty(), "an ended memory is not searched");
    }
}
