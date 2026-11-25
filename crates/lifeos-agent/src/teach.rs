//! The owner teaching an agent from one of its replies (remit's ADR-0093): Helpful, Not helpful (with why, which may
//! stay empty) or Correct reply (with the reply it should have given). The mark stays on the reply (its data, shown as
//! the `marked` activity), his newer mark replaces it, and each change is a teaching of that agent the librarian reads.

use anyhow::{Result, bail};
use lifeos_core::life::cards::now;
use lifeos_core::life::teachings::ROLE_TEACHING;
use lifeos_wire::{EventKind, LIBRARIAN, ReplyMark, TurnMark};

use crate::activity::{MarkData, data_of};
use crate::{Inner, owner};

/// How much of the reply a teaching quotes, at most (in characters).
const QUOTED: usize = 8000;

/// What the librarian reads of a mark on a reply. Pure.
pub(crate) fn teach_words(name: &str, reply: &str, mark: TurnMark, words: &str) -> String {
    let reply = match reply.char_indices().nth(QUOTED) {
        Some((i, _)) => format!("{}…", reply.get(..i).unwrap_or(reply)),
        None => reply.to_owned(),
    };
    let said = match (mark, words.is_empty()) {
        (TurnMark::Helpful, _) => "The owner marked it helpful.".to_owned(),
        (TurnMark::NotHelpful, true) => "The owner marked it not helpful, without saying why.".to_owned(),
        (TurnMark::NotHelpful, false) => format!("The owner marked it not helpful. Why:\n{words}"),
        (TurnMark::Correct, _) => format!("The owner marked it wrong. The reply it should have given:\n{words}"),
    };
    format!("{name} replied:\n{reply}\n\n{said}")
}

impl Inner {
    /// The owner marks agent reply `event`: the mark stays on it and teaches its agent. The same mark with the same words
    /// again changes nothing.
    pub(crate) fn teach_reply(&self, event: i64, mark: TurnMark, words: &str) -> Result<()> {
        let e = self.life().event(event)?;
        // An agent's own reply is a message its session wrote, which names no writer.
        if e.kind != EventKind::Message || e.by.is_some() {
            bail!("event {event} is no reply of an agent's");
        }
        if e.card == LIBRARIAN {
            bail!("the librarian's replies teach nothing");
        }
        if self.run_of(&e.card).and_then(|r| r.writing()) == Some(event) {
            bail!("the agent is still writing this reply");
        }
        let words = words.trim();
        if mark == TurnMark::Correct && words.is_empty() {
            bail!("say the reply it should have given");
        }
        let words = if mark == TurnMark::Helpful { "" } else { words };
        let mut data: MarkData = data_of(&e);
        if data.marks.iter().any(|m| m.by.kind == lifeos_wire::ActorKind::Owner && m.mark == mark && m.words == words) {
            return Ok(());
        }
        data.marks.retain(|m| m.by.kind != lifeos_wire::ActorKind::Owner);
        data.marks.push(ReplyMark { by: owner(), mark, words: words.to_owned(), at: now() });
        let mut marked = e.clone();
        marked.data = serde_json::to_string(&data)?;
        let marked = self.life().update_event(&marked)?;
        self.tell_event(marked);
        let name = self.actor_of(&e.card).name;
        let text = teach_words(&name, &e.text, mark, words);
        let source = format!("the owner marked a reply of {name} {} (event {event})", mark.as_str());
        let taught = self.life().teach_text(&e.card, ROLE_TEACHING, &source, "owner", &text);
        match taught {
            Ok(_) => self.librarian_wake.notify_one(),
            Err(err) => tracing::warn!("memory: could not record the owner's mark: {err:#}"),
        }
        Ok(())
    }
}
