//! Agents talk to each other directly (docs/adr/0002, after agentes.chat's ADR 0034 and 0095). Any agent asks another
//! (`ask_agent`) or hands it work (`hand_off`, news with `heads_up`); each is a row (`handoffs`) from the moment it is
//! sent until what came of it went back. Nothing waits: the tool returns at once, and what one piece of the sender's
//! work handed out comes back to it together, once, as one message. An agent that handed part of its own hand-off on
//! keeps it open; when the parts come back it takes a turn on them (to check, fix, put together) and that turn's answer
//! is the answer that goes up. Every turn reads what its agent handed out and how each stands, and the news handed it.
//!
//! The health check (`keep_handoffs`) looks at what is still out every minute and when the owner stops an agent: work
//! lost to a restart or dropped from an outbox is handed again once, then given up on; work that sat quiet is marked,
//! and after `HUNG` its turn is stopped. Whatever closes a hand-off reports its batch.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Weak};
use std::time::Duration;

use anyhow::{Result, bail};
use chrono::{DateTime, Local};
use lifeos_core::life::handoffs::{NewHandoff, batch_of_handoff, batch_of_prompt};
use lifeos_wire::{ASSISTANT, Actor, ActorKind, Delivery, Event, EventKind, Handoff, HandoffKind, HandoffStatus, LIBRARIAN};

use crate::activity::PromptData;
use crate::messages::{Exchange, exchange};
use crate::{Agents, Inner};

/// The agents one piece of work may pass through, from the one it started with to the last it was handed to.
pub(crate) const MAX_CHAIN: usize = 4;

/// How often the health check looks.
const TICK: Duration = Duration::from_secs(60);

/// Quiet this long, a hand-off at work is marked as quiet; this long, its turn is stopped and it is given up on.
const STALL: chrono::TimeDelta = chrono::TimeDelta::minutes(15);
const HUNG: chrono::TimeDelta = chrono::TimeDelta::hours(2);

/// How many times a hand-off is given to its target, at most, when lifeos loses it.
const ATTEMPTS: i64 = 2;

/// What a turn reads of what its agent handed out: still out, or closed within `RECENT`, at most `SHOWN`.
const RECENT: chrono::TimeDelta = chrono::TimeDelta::hours(1);
const SHOWN: u32 = 8;

/// The characters of a result a line quotes, at most.
const QUOTED: usize = 160;

fn first_line(text: &str) -> String {
    let line = text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or_default();
    match line.char_indices().nth(QUOTED) {
        Some((i, _)) => format!("{}…", line.get(..i).unwrap_or(line)),
        None => line.to_owned(),
    }
}

fn kind_words(kind: HandoffKind) -> &'static str {
    match kind {
        HandoffKind::Ask => "question",
        HandoffKind::Task => "task",
        HandoffKind::News => "news",
    }
}

/// How long ago `at` (RFC 3339) was, in words; empty when it cannot be read. Pure but for the clock.
fn ago(at: &str) -> String {
    let Ok(t) = DateTime::parse_from_rfc3339(at) else { return String::new() };
    let mins = (Local::now().fixed_offset() - t).num_minutes().max(0);
    match mins {
        0 => "just now".into(),
        m if m < 60 => format!("{m} min"),
        m => format!("{} h {} min", m / 60, m % 60),
    }
}

/// What the target reads as the hand-off's message. Pure.
pub(crate) fn handoff_text(h: &Handoff, again: bool) -> String {
    let via = if h.chain.len() > 1 { format!(", for {}", h.chain.join(" → ")) } else { String::new() };
    let tail = match h.kind {
        HandoffKind::Ask => "Answer it in your reply: your last message goes back to it as the answer.",
        _ => "Do it with your own tools. Your last message goes back to it as what came of it: say what you did, what \
              you found, and what is left.",
    };
    let again = if again {
        "\n(Handed to you again: lifeos lost the first while it was being worked on. Check what was already done before \
         you do it twice.)"
    } else {
        ""
    };
    format!("[hand-off #{} · {} from {}{via}]\n{}\n\n{tail}{again}", h.id, kind_words(h.kind), h.sender_name, h.text.trim())
}

/// What comes back to the sender: what came of each hand-off of a batch, and what to do with it. `parent` is the
/// hand-off the sender was working on when it handed these out. Pure.
pub(crate) fn results_text(rows: &[Handoff], parent: Option<&Handoff>) -> String {
    let mut b = String::from("[what came of what you handed off]");
    for h in rows {
        let how = match h.status {
            HandoffStatus::Done => "done".to_owned(),
            HandoffStatus::Failed if h.note.is_empty() => "failed".to_owned(),
            HandoffStatus::Failed => format!("failed: {}", h.note),
            HandoffStatus::Cancelled => "cancelled by you".to_owned(),
            s => s.as_str().to_owned(),
        };
        let result = if h.result.trim().is_empty() { "(nothing)".to_owned() } else { h.result.trim().to_owned() };
        b.push_str(&format!("\n\n— #{} {} ({}) · {} · {how}:\n{result}", h.id, h.target_name, h.target, kind_words(h.kind)));
    }
    b.push_str("\n\n");
    b.push_str(&match parent {
        Some(p) => format!(
            "These were for hand-off #{} from {}. Check them: fix what they found if it is yours to fix (hand it back \
             only with what is wrong or missing), put the parts together, then answer: your reply goes back to {} as \
             what came of #{}.",
            p.id, p.sender_name, p.sender_name, p.id
        ),
        None => "Act on them: fix what they found, check what needs checking, and tell the owner what matters. Hand \
                 work again only with what is wrong or missing, never the same thing twice."
            .to_owned(),
    });
    b
}

/// One line of what an agent handed out, as its turn reads it. Pure but for the clock.
fn handed_line(h: &Handoff) -> String {
    let state = match h.status {
        HandoffStatus::Queued => format!("queued · {}", ago(&h.created)),
        HandoffStatus::Working => format!("working · {}", ago(&h.updated)),
        HandoffStatus::Waiting => "waiting on what it handed on".to_owned(),
        HandoffStatus::Done if h.reported => "done".to_owned(),
        HandoffStatus::Done => "done, the result is on its way to you".to_owned(),
        HandoffStatus::Failed => "failed".to_owned(),
        HandoffStatus::Cancelled => "cancelled".to_owned(),
    };
    let note = if h.note.is_empty() || !h.status.open() { String::new() } else { format!(" ({})", h.note) };
    let said = if h.status.open() { first_line(&h.text) } else { first_line(&h.result) };
    format!("- #{} → {} ({}, {state}{note}): {said}", h.id, h.target_name, kind_words(h.kind))
}

fn data_of(e: &Event) -> PromptData {
    if e.data.is_empty() { PromptData::default() } else { serde_json::from_str(&e.data).unwrap_or_default() }
}

fn lifeos() -> Actor {
    Actor { kind: ActorKind::Lifeos, id: String::new(), name: "lifeos".into() }
}

impl Inner {
    /// The hand-off agent `card`'s turn in progress works for, and that turn's prompt.
    pub(crate) fn working_for(&self, card: &str) -> (Option<i64>, Option<i64>) {
        let Some(prompt) = self.run_of(card).and_then(|r| r.prompt_id()) else { return (None, None) };
        let handoff = self.handoff_of(card, prompt);
        (handoff, Some(prompt))
    }

    /// The hand-off prompt event `prompt` of `card` is a turn on, when it is one `card` has.
    fn handoff_of(&self, card: &str, prompt: i64) -> Option<i64> {
        let life = self.life();
        let e = life.event(prompt).ok()?;
        let h = data_of(&e).handoff?;
        life.handoff(h).ok().filter(|h| h.target == card).map(|h| h.id)
    }

    /// Agent `from` asks or hands `to` `text`. What the sender reads.
    pub(crate) async fn hand(self: &Arc<Self>, from: &str, to: &str, text: &str, kind: HandoffKind) -> Result<String> {
        let to = to.trim().trim_start_matches('@');
        let text = text.trim();
        if to == from {
            bail!("that is you: say it in your answer instead");
        }
        if to == LIBRARIAN {
            bail!("the librarian takes no messages: it reads what the owner says by itself");
        }
        if text.is_empty() {
            bail!("say what you hand over");
        }
        self.check_agent(to).map_err(|e| anyhow::anyhow!("{e:#}. list_crew shows who there is."))?;
        let note = match exchange(self.replies_in_a_row(from)) {
            Exchange::Go => String::new(),
            Exchange::Remind(note) => format!("\n{note}"),
            Exchange::Refuse(why) => bail!(why),
        };
        let (working, prompt) = self.working_for(from);
        let parent = match working {
            Some(id) => Some(self.life().handoff(id)?),
            None => None,
        };
        let mut chain = parent.as_ref().map(|p| p.chain.clone()).unwrap_or_default();
        chain.push(from.to_owned());
        if kind != HandoffKind::News {
            if chain.iter().any(|c| c == to) {
                bail!(
                    "{to} is already in this chain of work ({}): it is waiting on you. Answer with what you have, and \
                     say in your answer what is missing.",
                    chain.join(" → ")
                );
            }
            if chain.len() >= MAX_CHAIN {
                bail!(
                    "this work has passed through {} agents already ({}): do the rest yourself, or say in your answer \
                     what is missing.",
                    chain.len(),
                    chain.join(" → ")
                );
            }
            if let Some(d) = self.life().open_duplicate(from, to, text)? {
                bail!(
                    "you already handed {} this as #{}, and it is {}: what comes of it reaches you by itself.",
                    d.target_name,
                    d.id,
                    d.status.as_str()
                );
            }
        }
        let batch = match (&parent, prompt) {
            (Some(p), _) => batch_of_handoff(p.id),
            (None, Some(p)) => batch_of_prompt(p),
            (None, None) => batch_of_prompt(0),
        };
        let (me, them) = (self.actor_of(from), self.actor_of(to));
        let h = self.life().add_handoff(NewHandoff {
            kind,
            sender: from.to_owned(),
            sender_name: me.name.clone(),
            target: to.to_owned(),
            target_name: them.name.clone(),
            text: text.to_owned(),
            parent: parent.as_ref().map(|p| p.id),
            batch,
            chain,
        })?;
        self.config.on_change.fire();
        if kind == HandoffKind::News {
            return Ok(format!(
                "Told {} as news (#{}): it reads it with its next message. Nothing comes back, and no turn starts.{note}",
                them.name, h.id
            ));
        }
        if let Err(err) = self.give(&h, false).await {
            let _closed = self.life().close_handoff(h.id, HandoffStatus::Failed, "", &format!("{err:#}"));
            self.config.on_change.fire();
            return Err(err);
        }
        let what = if kind == HandoffKind::Ask { "Asked" } else { "Handed it to" };
        Ok(format!(
            "{what} {} as #{}. It returns at once; carry on with what does not need it. What comes of it reaches you by \
             itself, with everything else you handed out for this piece of work: don't check on it or send it again.{note}",
            them.name, h.id
        ))
    }

    /// Queues hand-off `h` in its target's outbox, again when `again`.
    async fn give(self: &Arc<Self>, h: &Handoff, again: bool) -> Result<()> {
        let data = serde_json::to_string(&PromptData { handoff: Some(h.id), ..PromptData::default() })?;
        let agents = Agents { inner: Arc::clone(self) };
        let by = self.actor_of(&h.sender);
        let e = agents.say_with(&h.target, &handoff_text(h, again), &[], by, Delivery::Queue, data).await?;
        if again {
            self.life().requeue_handoff(h.id, e.id, "lifeos lost it while it was worked on, and handed it again")?;
        } else {
            self.life().set_handoff_prompt(h.id, e.id)?;
        }
        self.tell_agent(&h.sender);
        Ok(())
    }

    /// A turn of `card` on `prompt` begins: the hand-off it is on is at work.
    pub(crate) fn handoff_started(&self, card: &str, prompt: &Event) {
        let Some(h) = data_of(prompt).handoff else { return };
        let mine = self.life().handoff(h).ok().filter(|h| h.target == card && h.status.open());
        if mine.is_some() && self.life().mark_handoff(h, HandoffStatus::Working, Some("")).is_ok() {
            self.config.on_change.fire();
        }
    }

    /// A turn of `card` on `prompt` ended with `output`, or failed for `failed`: the hand-off it was on closes with
    /// it, or waits on what it handed on, and what closed goes back.
    pub(crate) async fn handoff_ended(self: &Arc<Self>, card: &str, prompt: &Event, output: &str, failed: Option<&str>) {
        let Some(id) = data_of(prompt).handoff else { return };
        let Ok(h) = self.life().handoff(id) else { return };
        if h.target != card || !h.status.open() {
            return;
        }
        let closed = {
            let life = self.life();
            match failed {
                Some(why) => life.close_handoff(id, HandoffStatus::Failed, output, why),
                None if life.open_children(id).unwrap_or(0) > 0 => {
                    let held = life.hold_result(id, output).and_then(|()| life.mark_handoff(id, HandoffStatus::Waiting, Some("")));
                    held.map(|()| None)
                }
                None => life.close_handoff(id, HandoffStatus::Done, output, ""),
            }
        };
        self.config.on_change.fire();
        match closed {
            Ok(Some(h)) => self.report_of(&h).await,
            Ok(None) => {}
            Err(err) => tracing::warn!("could not settle hand-off {id}: {err:#}"),
        }
    }

    /// Reports the batch hand-off `h` came back with.
    fn report_of<'a>(self: &'a Arc<Self>, h: &'a Handoff) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            let batch = self.life().handoff_batch(h.id);
            if let Ok(batch) = batch {
                self.report(&batch).await;
            }
        })
    }

    /// Sends what came of batch `batch` back to its sender, once none of it is still out.
    pub(crate) async fn report(self: &Arc<Self>, batch: &str) {
        let claimed = self.life().claim_batch(batch);
        let rows = match claimed {
            Ok(Some(rows)) => rows,
            Ok(None) => return,
            Err(err) => {
                tracing::warn!("could not read batch {batch}: {err:#}");
                return;
            }
        };
        self.config.on_change.fire();
        // What the sender took back itself it knows already.
        if rows.iter().all(|h| h.status == HandoffStatus::Cancelled) {
            return;
        }
        let Some(sender) = rows.first().map(|h| h.sender.clone()) else { return };
        let parent = batch
            .strip_prefix("h:")
            .and_then(|id| id.parse::<i64>().ok())
            .and_then(|id| self.life().handoff(id).ok())
            .filter(|p| p.target == sender && p.status.open());
        let data = PromptData {
            handoff: parent.as_ref().map(|p| p.id),
            results: rows.iter().map(|h| h.id).collect(),
            ..PromptData::default()
        };
        let by = match rows.as_slice() {
            [one] => self.actor_of(&one.target),
            _ => lifeos(),
        };
        let text = results_text(&rows, parent.as_ref());
        let agents = Agents { inner: Arc::clone(self) };
        let data = serde_json::to_string(&data).unwrap_or_default();
        if let Err(err) = agents.say_with(&sender, &text, &[], by, Delivery::Queue, data).await {
            tracing::warn!("could not bring what came of batch {batch} back to {sender}: {err:#}");
            // Nobody is left to take the parent's answer: close it with what it holds.
            let closed = parent.and_then(|p| self.life().close_handoff(p.id, HandoffStatus::Done, &p.result, "").ok().flatten());
            if let Some(p) = closed {
                self.report_of(&p).await;
            }
        }
    }

    /// Agent `from` takes back hand-off `id` (the Assistant any, on the owner's word), and what was handed on for it.
    pub(crate) async fn cancel_handoff(self: &Arc<Self>, from: &str, id: i64, why: &str) -> Result<String> {
        if why.trim().is_empty() {
            bail!("say what changed (why)");
        }
        let h = self.life().handoff(id)?;
        if h.sender != from && from != ASSISTANT {
            bail!("#{id} is {}'s, not yours: ask the Assistant", h.sender_name);
        }
        if !h.status.open() {
            bail!("#{id} is {} already", h.status.as_str());
        }
        let n = self.cancel_tree(&h, why.trim()).await;
        self.config.on_change.fire();
        self.report_of(&h).await;
        Ok(format!("Took back #{id} from {}{}.", h.target_name, if n > 1 { format!(" and {} it handed on", n - 1) } else { String::new() }))
    }

    /// Cancels open hand-off `h` and what was handed on for it: dropped from an outbox, or its turn stopped. How many.
    fn cancel_tree<'a>(self: &'a Arc<Self>, h: &'a Handoff, why: &'a str) -> Pin<Box<dyn Future<Output = usize> + Send + 'a>> {
        Box::pin(async move {
            let note = format!("taken back: {why}");
            let closed = self.life().close_handoff(h.id, HandoffStatus::Cancelled, &h.result, &note);
            let Ok(Some(_)) = closed else { return 0 };
            self.drop_prompt(h);
            if self.working_for(&h.target).0 == Some(h.id) {
                self.run(&h.target).interrupt().await;
            }
            let children: Vec<Handoff> = {
                let open = self.life().handoffs(true, Some(&h.target));
                open.unwrap_or_default().into_iter().filter(|c| c.parent == Some(h.id)).collect()
            };
            let mut n = 1;
            for c in children {
                n += self.cancel_tree(&c, why).await;
            }
            n
        })
    }

    /// Drops hand-off `h`'s prompt from its target's outbox while it is still queued there.
    fn drop_prompt(&self, h: &Handoff) {
        let dropped = {
            let life = self.life();
            life.handoff_prompt(h.id).ok().filter(|p| *p > 0).and_then(|p| life.event(p).ok()).filter(|e| e.status == "queued").and_then(
                |mut e| {
                    e.status = "dropped".into();
                    life.update_event(&e).ok()
                },
            )
        };
        if let Some(e) = dropped {
            self.tell_event(e);
            self.tell_agent(&h.target);
        }
    }

    /// What a turn of `card` reads of the agents' work: what it handed out and how each stands, and the news handed
    /// it since its last message. None when there is neither.
    pub(crate) fn handoff_note(&self, card: &str) -> Option<String> {
        let since = (Local::now() - RECENT).to_rfc3339();
        let (handed, news) = {
            let life = self.life();
            (life.handed_by(card, &since, SHOWN).unwrap_or_default(), life.take_news(card).unwrap_or_default())
        };
        let mut parts = Vec::new();
        if !handed.is_empty() {
            let lines: Vec<String> = handed.iter().map(handed_line).collect();
            parts.push(format!(
                "<handed-off>\nWhat you handed out (newest first). What comes of each reaches you by itself; don't check \
                 on it or send it again. cancel_handoff takes back one no longer needed.\n{}\n</handed-off>",
                lines.join("\n")
            ));
        }
        if !news.is_empty() {
            let lines: Vec<String> = news.iter().map(|h| format!("- {} (#{}): {}", h.sender_name, h.id, h.text.trim())).collect();
            parts.push(format!("<news>\nNews other agents handed you; nothing to answer:\n{}\n</news>", lines.join("\n")));
        }
        (!parts.is_empty()).then(|| parts.join("\n"))
    }

    /// Where every agent stands, for an agent to see who is free and what is out (`team_status`).
    pub(crate) fn team_status(&self) -> Result<String> {
        let mut cards = vec![ASSISTANT.to_owned()];
        cards.extend(self.life().crew()?.into_iter().filter(|m| m.status == "active").map(|m| m.id));
        let open = self.life().open_handoffs()?;
        let mut lines = Vec::new();
        for card in cards {
            let Ok(v) = self.view(&card) else { continue };
            let s = &v.standing;
            let doing = if s.doing.is_empty() { String::new() } else { format!(" · {}", s.doing) };
            let queued = if v.card.queued > 0 { format!(" · {} queued", v.card.queued) } else { String::new() };
            let has: Vec<String> = open.iter().filter(|h| h.target == card).map(|h| format!("#{} {}", h.id, h.status.as_str())).collect();
            let has = if has.is_empty() { String::new() } else { format!(" · has {}", has.join(", ")) };
            lines.push(format!("- {} ({card}): {}{doing}{queued}{has} · last active {}", v.card.name, s.label, ago(&v.card.updated)));
        }
        let out: Vec<String> = open.iter().map(|h| {
            let note = if h.note.is_empty() { String::new() } else { format!(" ({})", h.note) };
            format!("- #{} {} → {} · {} · {}{note}: {}", h.id, h.sender_name, h.target_name, kind_words(h.kind), h.status.as_str(), first_line(&h.text))
        }).collect();
        Ok(format!(
            "Agents:\n{}\n\nStill out ({}):\n{}",
            lines.join("\n"),
            out.len(),
            if out.is_empty() { "(nothing)".to_owned() } else { out.join("\n") }
        ))
    }

    /// One look at everything still out: what was lost is handed again or given up on, what sat quiet is marked or
    /// stopped, what is done but held is let go. Every close reports its batch.
    pub(crate) async fn check_handoffs(self: &Arc<Self>) {
        let open = self.life().open_handoffs().unwrap_or_default();
        for h in open {
            if let Err(err) = self.check_handoff(&h).await {
                tracing::warn!("health check of hand-off {}: {err:#}", h.id);
            }
        }
    }

    async fn check_handoff(self: &Arc<Self>, h: &Handoff) -> Result<()> {
        let on_it = self.working_for(&h.target).0 == Some(h.id);
        let prompt = {
            let life = self.life();
            let p = life.handoff_prompt(h.id)?;
            (p > 0).then(|| life.event(p).ok()).flatten()
        };
        if self.member(&h.target).ok().flatten().is_some_and(|m| m.status != "active") {
            return self.give_up(h, &format!("{} was retired", h.target_name)).await;
        }
        match h.status {
            HandoffStatus::Queued => match prompt.as_ref().map(|e| e.status.as_str()) {
                Some("queued") => self.run(&h.target).ring(),
                Some("dropped") => self.give_up(h, &format!("dropped from {}'s outbox when the owner stopped it", h.target_name)).await?,
                _ if on_it => {}
                _ => self.lost(h).await?,
            },
            HandoffStatus::Working if !on_it => self.lost(h).await?,
            HandoffStatus::Working => {
                let quiet = self.quiet_for(&h.target);
                if quiet >= HUNG {
                    self.run(&h.target).interrupt().await;
                    self.give_up(h, &format!("{} did nothing for {} min: lifeos stopped its turn", h.target_name, quiet.num_minutes())).await?;
                } else if quiet >= STALL {
                    let note = format!("quiet for {} min", quiet.num_minutes());
                    if note != h.note {
                        self.life().mark_handoff(h.id, HandoffStatus::Working, Some(&note))?;
                        self.config.on_change.fire();
                    }
                }
            }
            HandoffStatus::Waiting => {
                if self.life().open_children(h.id)? > 0 {
                    return Ok(());
                }
                // Everything it handed on is back. Its results were reported, or are about to be.
                self.report(&batch_of_handoff(h.id)).await;
                let stream = self.life().events(&h.target, None, 200)?;
                let pending = stream.into_iter().any(|e| {
                    e.kind == EventKind::Prompt && e.status == "queued" && data_of(&e).handoff == Some(h.id)
                });
                if !pending && !on_it {
                    let done = self.life().close_handoff(h.id, HandoffStatus::Done, &h.result, "")?;
                    if let Some(done) = done {
                        self.config.on_change.fire();
                        self.report_of(&done).await;
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Hand-off `h` was lost (lifeos restarted while it was worked on): handed again, or given up on.
    async fn lost(self: &Arc<Self>, h: &Handoff) -> Result<()> {
        if h.attempts >= ATTEMPTS {
            return self.give_up(h, &format!("lost {} times while {} worked on it", h.attempts, h.target_name)).await;
        }
        self.give(h, true).await?;
        self.config.on_change.fire();
        Ok(())
    }

    async fn give_up(self: &Arc<Self>, h: &Handoff, why: &str) -> Result<()> {
        self.drop_prompt(h);
        let closed = self.life().close_handoff(h.id, HandoffStatus::Failed, &h.result, why)?;
        if let Some(h) = closed {
            self.config.on_change.fire();
            self.report_of(&h).await;
        }
        Ok(())
    }

    /// How long agent `card` has done nothing: since its newest event.
    fn quiet_for(&self, card: &str) -> chrono::TimeDelta {
        let newest = self.life().events(card, None, 1).ok().and_then(|e| e.into_iter().next_back());
        newest
            .and_then(|e| DateTime::parse_from_rfc3339(&e.updated).ok())
            .map_or(chrono::TimeDelta::zero(), |t| Local::now().fixed_offset() - t)
    }
}

/// The health check: once soon after lifeos starts (what a restart lost), then every `TICK` and whenever it is rung.
pub(crate) fn keep_handoffs(inner: Weak<Inner>) {
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(2)).await;
        loop {
            let Some(i) = inner.upgrade() else { return };
            i.check_handoffs().await;
            let bell = Arc::clone(&i.handoff_tick);
            drop(i);
            tokio::select! {
                () = tokio::time::sleep(TICK) => {}
                () = bell.notified() => {}
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: i64, status: HandoffStatus, result: &str) -> Handoff {
        Handoff {
            id,
            kind: HandoffKind::Task,
            sender: "assistant".into(),
            sender_name: "Assistant".into(),
            target: "fin".into(),
            target_name: "Fin".into(),
            text: "Kiểm tra ngân sách\nchi tiết".into(),
            status,
            result: result.into(),
            parent: None,
            chain: vec!["assistant".into()],
            attempts: 1,
            note: String::new(),
            created: String::new(),
            updated: String::new(),
            closed: String::new(),
            reported: false,
        }
    }

    #[test]
    fn the_target_reads_who_hands_it_what_and_where_its_answer_goes() {
        let mut h = row(12, HandoffStatus::Queued, "");
        assert!(handoff_text(&h, false).starts_with("[hand-off #12 · task from Assistant]\nKiểm tra ngân sách"));
        h.chain = vec!["assistant".into(), "fin".into()];
        h.kind = HandoffKind::Ask;
        let text = handoff_text(&h, true);
        assert!(text.starts_with("[hand-off #12 · question from Assistant, for assistant → fin]"), "{text}");
        assert!(text.contains("Answer it in your reply") && text.contains("Handed to you again"), "{text}");
    }

    #[test]
    fn results_come_back_together_with_what_to_do_next() {
        let mut failed = row(13, HandoffStatus::Failed, "");
        failed.note = "its turn failed".into();
        let rows = [row(12, HandoffStatus::Done, "Còn 4 triệu."), failed];
        let text = results_text(&rows, None);
        assert!(text.starts_with("[what came of what you handed off]\n\n— #12 Fin (fin) · task · done:\nCòn 4 triệu."), "{text}");
        assert!(text.contains("— #13 Fin (fin) · task · failed: its turn failed:\n(nothing)"), "{text}");
        assert!(text.contains("fix what they found"), "{text}");
        let mut parent = row(7, HandoffStatus::Waiting, "");
        parent.sender_name = "Coach".into();
        let text = results_text(&rows[..1], Some(&parent));
        assert!(text.ends_with("your reply goes back to Coach as what came of #7."), "{text}");
    }

    #[test]
    fn a_line_says_where_each_hand_off_stands() {
        let mut h = row(12, HandoffStatus::Waiting, "");
        assert_eq!(handed_line(&h), "- #12 → Fin (task, waiting on what it handed on): Kiểm tra ngân sách");
        h.status = HandoffStatus::Done;
        h.result = "\n Còn 4 triệu.\nThêm".into();
        assert_eq!(handed_line(&h), "- #12 → Fin (task, done, the result is on its way to you): Còn 4 triệu.");
        h.status = HandoffStatus::Working;
        h.note = "quiet for 20 min".into();
        assert!(handed_line(&h).contains("(quiet for 20 min)"));
    }
}
