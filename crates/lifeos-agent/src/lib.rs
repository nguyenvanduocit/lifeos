//! lifeos's agents at work, on remit's backbone (docs/adr/0001): every agent (the Assistant, each crew member) works on
//! a card whose stream is its conversation and outbox; its session is a Claude Code process started by `agent`'s
//! launcher, reaching lifeos's tools through the MCP door on loopback. Every interface (web, Telegram, CLI) talks to the
//! agents through `Agents`: it queues a message, and watches the stream, or waits for the turn that answers it.

mod accounts;
mod activity;
mod blocks;
mod brief;
mod cardcontext;
mod computers;
mod door;
mod handoffs;
#[cfg(test)]
mod images_tests;
mod librarian;
mod memory;
mod messages;
mod recall;
mod notify;
mod run;
mod otel;
#[cfg(test)]
mod owner_tests;
mod teach;
mod toolbox;
mod workflows;

pub use accounts::AccountPool;
pub use computers::{Attached, Computers};
pub use lifeos_google::Google;
pub use activity::{read as event_read, shown_name};
pub use notify::Notice;

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError, Weak};

use anyhow::{Context, Result, bail};
use chrono::Local;
use lifeos_core::life::cards::{EVENTS_PAGE, NewEvent};
use lifeos_core::{Life, tools};
use lifeos_wire::{LIBRARIAN, 
    ASSISTANT, Activity, Actor, ActorKind, AgentStanding, AgentTone, AgentView, Ask, Card, CardStatus, CrewMember, Delivery,
    DoingTone, Event, EventKind, EventRead, Image, ImageRef, ImageType, OnMessage, TurnMark,
};
use model::SystemNote;
use tokio::sync::broadcast;

use crate::accounts::Lease;
use crate::door::{Opens, Server, Tokens};
use crate::notify::Notifier;
use crate::run::Run;
use crate::toolbox::Toolbox;

/// The one `Life` shared by the agents' tools and the interface around them.
pub type SharedLife = Arc<Mutex<Life>>;

/// What the page sends when the owner asks for the morning brief.
pub const BRIEF_PROMPT: &str = "Write today's morning brief.";

/// Built-in Claude Code tools an agent keeps. No Bash: an agent manages a life, not a computer.
const BUILTIN_TOOLS: &[&str] = &["Read", "Glob", "Grep", "Edit", "Write", "WebSearch", "WebFetch", "TodoWrite"];

/// Every agent keeps its memory, whatever tools it was given.
/// The memory tools every agent keeps, whatever tools it was given: it reads memory and says what helped.
const MEMORY_TOOLS: &[&str] = &["recall", "rate_memory", "list_memories"];

/// lifeos's tools no agent but the librarian writes memory with (`remember`), and only the Assistant on the owner's
/// word ends one with (`forget`).
const LIBRARIAN_WRITES: &[&str] = &["remember"];
const ASSISTANT_ONLY: &[&str] = &["forget"];


/// Priorities of the outbox: the higher is read first. The owner's words come before what agents and routines send.
pub const PRIORITY_OTHER: i64 = 0;
pub const PRIORITY_OWNER: i64 = 1;

/// A callback for "a life's data changed", e.g. to refresh open dashboards.
#[derive(Clone, Default)]
pub struct OnChange(Option<Arc<dyn Fn() + Send + Sync>>);

impl OnChange {
    pub fn new(f: impl Fn() + Send + Sync + 'static) -> OnChange {
        OnChange(Some(Arc::new(f)))
    }

    pub fn fire(&self) {
        if let Some(f) = &self.0 {
            f();
        }
    }
}

impl std::fmt::Debug for OnChange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(if self.0.is_some() { "OnChange(set)" } else { "OnChange(none)" })
    }
}

#[derive(Debug, Clone, Default)]
pub struct AgentsConfig {
    /// Claude Code model alias or id for the Assistant; `None` = the account default.
    pub model: Option<String>,
    /// low | medium | high | xhigh | max; `None` = Claude Code's default.
    pub effort: Option<String>,
    /// Called after every write tool an agent runs.
    pub on_change: OnChange,
    /// A pool shared with the interface that manages accounts; one is made when none is given.
    pub accounts: Option<Arc<AccountPool>>,
    /// The Google app shared with the interface the owner signs in from; one is made when none is given.
    pub google: Option<Arc<Google>>,
    /// What starts sessions in place of Claude Code: for tests that run turns without a model.
    #[doc(hidden)]
    pub launcher: Option<Launch>,
    /// Where OpenRouter's API is, in place of OpenRouter's own: for tests.
    #[doc(hidden)]
    pub openrouter_base: Option<String>,
    /// How long teachings wait quiet before the librarian is woken; `librarian::QUIET` when not said.
    #[doc(hidden)]
    pub librarian_quiet: Option<std::time::Duration>,
}

/// A launcher given in place of Claude Code (`AgentsConfig::launcher`).
#[derive(Clone)]
pub struct Launch(pub Arc<dyn agent::Launcher>);

impl std::fmt::Debug for Launch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Launch")
    }
}

/// How a turn an interface waited for ended: the agent's final words, or why it failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    pub text: String,
    pub error: Option<String>,
    pub ms: Option<i64>,
}

impl Answer {
    /// The words to show for it.
    pub fn shown(&self) -> String {
        match (&self.error, self.text.trim().is_empty()) {
            (Some(err), true) => format!("⚠️ {err}"),
            _ => self.text.clone(),
        }
    }
}

/// lifeos's agents. Cheap to clone; every clone is the same agents.
#[derive(Clone)]
pub struct Agents {
    inner: Arc<Inner>,
}

pub(crate) struct Inner {
    life: SharedLife,
    config: AgentsConfig,
    pub(crate) accounts: Arc<AccountPool>,
    pub(crate) google: Arc<Google>,
    /// The owner's Macs online now, for the desktop tools.
    pub(crate) computers: Arc<Computers>,
    pub(crate) launcher: Arc<dyn agent::Launcher>,
    pub(crate) tokens: Tokens,
    pub(crate) notifier: Notifier,
    door_url: OnceLock<String>,
    runs: Mutex<HashMap<String, Arc<Run>>>,
    /// The memories each card's session was shown, so a recall does not repeat them.
    recalled: Mutex<HashMap<String, Vec<i64>>>,
    turns: broadcast::Sender<(String, i64, Answer)>,
    /// The models memory reads meaning and relevance with.
    models: memory::Models,
    /// Rung when a teaching is recorded.
    librarian_wake: Arc<tokio::sync::Notify>,
    /// One lock per workflow run, so it is evaluated once at a time; and the waits' ticker's bell.
    workflow_locks: Mutex<HashMap<i64, Arc<tokio::sync::Mutex<()>>>>,
    workflow_tick: Arc<tokio::sync::Notify>,
    /// Rings the hand-offs' health check (`handoffs::keep_handoffs`) before its next tick.
    handoff_tick: Arc<tokio::sync::Notify>,
    /// Where what the sessions export as OpenTelemetry is kept.
    pub(crate) telemetry: otel::Store,
    /// Held by a pick in a block from its read to its store, so two picks never queue two messages.
    picking: tokio::sync::Mutex<()>,
}

impl Agents {
    /// Starts the agents' MCP door. Sessions open on each agent's first message.
    pub async fn start(life: SharedLife, config: AgentsConfig) -> Result<Agents> {
        let accounts =
            config.accounts.clone().unwrap_or_else(|| AccountPool::new(life.clone(), config.on_change.clone()));
        let (hide, key_file, google): (Vec<String>, _, _) = {
            let life = life.lock().unwrap_or_else(PoisonError::into_inner);
            let hide = life.private_paths()?.iter().map(|p| p.display().to_string()).collect();
            let google = match &config.google {
                Some(g) => Arc::clone(g),
                None => Arc::new(Google::new(life.google_dir()?)),
            };
            (hide, memory::key_file(life.home()), google)
        };
        let telemetry = {
            let life = life.lock().unwrap_or_else(PoisonError::into_inner);
            otel::Store::open(&life.home().join(".lifeos").join("telemetry.db"))?
        };
        let launcher: Arc<dyn agent::Launcher> = match &config.launcher {
            Some(Launch(l)) => Arc::clone(l),
            None => Arc::new(accounts.launcher(hide)),
        };
        let models = memory::Models::new(key_file, config.openrouter_base.clone());
        let inner = Arc::new(Inner {
            life,
            config,
            accounts,
            google,
            computers: Arc::default(),
            launcher,
            tokens: Tokens::default(),
            notifier: Notifier::default(),
            door_url: OnceLock::new(),
            runs: Mutex::default(),
            recalled: Mutex::default(),
            turns: broadcast::channel(256).0,
            models,
            librarian_wake: Arc::default(),
            workflow_locks: Mutex::default(),
            workflow_tick: Arc::default(),
            handoff_tick: Arc::default(),
            telemetry,
            picking: tokio::sync::Mutex::new(()),
        });
        let url = door::open(Arc::new(DoorOpens(Arc::downgrade(&inner)))).await?;
        inner.door_url.set(url).map_err(|_| anyhow::anyhow!("the door opened twice"))?;
        memory::keep_embedding(Arc::downgrade(&inner));
        workflows::keep_workflows(Arc::downgrade(&inner));
        handoffs::keep_handoffs(Arc::downgrade(&inner));
        otel::keep_forgetting(Arc::downgrade(&inner));
        librarian::keep_librarian(Arc::downgrade(&inner), inner.config.librarian_quiet.unwrap_or(librarian::QUIET));
        if !inner.life().open_teachings(1)?.is_empty() {
            inner.librarian_wake.notify_one();
        }
        Ok(Agents { inner })
    }

    pub fn accounts(&self) -> Arc<AccountPool> {
        self.inner.accounts.clone()
    }

    pub fn google(&self) -> Arc<Google> {
        self.inner.google.clone()
    }

    /// The owner's Macs online now: the link attaches each one's stream here.
    pub fn computers(&self) -> Arc<Computers> {
        self.inner.computers.clone()
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Notice> {
        self.inner.notifier.subscribe()
    }

    /// Queues `text` from `by` for agent `card`, delivered as `delivery` says. The prompt event, as stored.
    pub async fn say(&self, card: &str, text: &str, by: Actor, delivery: Delivery) -> Result<Event> {
        self.say_with(card, text, &[], by, delivery, String::new()).await
    }

    /// `say` with images put first (`put_image`): the prompt names them, and the session reads their bytes with the
    /// words, in a queued message's turn or steered into the turn in progress.
    pub async fn say_images(
        &self,
        card: &str,
        text: &str,
        images: &[ImageRef],
        by: Actor,
        delivery: Delivery,
    ) -> Result<Event> {
        self.say_with(card, text, images, by, delivery, String::new()).await
    }

    /// `say`, the prompt keeping `data` (`activity::PromptData` for one agent's message to another) and naming images.
    pub(crate) async fn say_with(
        &self,
        card: &str,
        text: &str,
        images: &[ImageRef],
        by: Actor,
        delivery: Delivery,
        data: String,
    ) -> Result<Event> {
        let i = &self.inner;
        i.check_agent(card)?;
        let data = if images.is_empty() {
            data
        } else {
            i.life().check_images(images)?;
            let mut d: activity::PromptData =
                if data.is_empty() { activity::PromptData::default() } else { serde_json::from_str(&data)? };
            d.images = images.to_vec();
            serde_json::to_string(&d)?
        };
        let priority = if by.kind == ActorKind::Owner { PRIORITY_OWNER } else { PRIORITY_OTHER };
        let run = i.run(card);
        if delivery == Delivery::Steer
            && let Some(session) = run.session().filter(|_| run.busy())
        {
            let prompt = i.message_with_attachments(i.compose_steer(card, text, &by), images)?;
            if session.steer(prompt).await.unwrap_or(false) {
                return Ok(i.add_event(NewEvent {
                    card: card.into(),
                    kind: Some(EventKind::Prompt),
                    status: "steered".into(),
                    text: text.into(),
                    by: Some(by),
                    priority,
                    data,
                    ..NewEvent::default()
                }));
            }
        }
        let e = i.add_event(NewEvent {
            card: card.into(),
            kind: Some(EventKind::Prompt),
            status: "queued".into(),
            text: text.into(),
            by: Some(by),
            priority,
            data,
            ..NewEvent::default()
        });
        if e.id == 0 {
            bail!("could not queue the message");
        }
        i.tell_agent(card);
        if delivery == Delivery::Interrupt {
            run.interrupt().await;
        }
        run.ring();
        Ok(e)
    }

    /// Sends `text` to agent `card` and waits for the turn that answers it.
    pub async fn ask(&self, card: &str, text: &str, by: Actor) -> Result<Answer> {
        let mut turns = self.inner.turns.subscribe();
        let prompt = self.say(card, text, by, Delivery::Queue).await?;
        loop {
            match turns.recv().await {
                Ok((c, id, answer)) if c == card && id == prompt.id => return Ok(answer),
                Ok(_) | Err(broadcast::error::RecvError::Lagged(_)) => {}
                Err(broadcast::error::RecvError::Closed) => bail!("lifeos stopped"),
            }
        }
    }

    /// Ends the turn agent `card` is in.
    pub async fn interrupt(&self, card: &str) -> Result<()> {
        self.inner.check_agent(card)?;
        self.inner.run(card).interrupt().await;
        Ok(())
    }

    /// Start over: the agent's next message opens a new session, which reads a recap of the conversation.
    pub async fn restart(&self, card: &str) -> Result<()> {
        let i = &self.inner;
        i.check_agent(card)?;
        let run = i.run(card);
        run.interrupt().await;
        run.close(i);
        i.recalled.lock().unwrap_or_else(PoisonError::into_inner).remove(card);
        i.edit_card(card, |c| {
            c.session_id.clear();
            c.ctx_used = 0;
            c.plan.clear();
            c.error.clear();
            c.status = CardStatus::Idle;
        });
        i.add_event(NewEvent {
            card: card.into(),
            kind: Some(EventKind::System),
            status: SystemNote::NewSession.as_str().into(),
            text: "Started over.".into(),
            by: Some(owner()),
            data: serde_json::to_string(&activity::NewSessionData { started_over: true }).unwrap_or_default(),
            ..NewEvent::default()
        });
        Ok(())
    }

    /// The owner stops agent `card` (remit's Stop agent…): the messages waiting for it are dropped, the turn it is in
    /// ends, and his why stays in its stream, which it reads with the next message.
    pub async fn stop(&self, card: &str, reason: &str) -> Result<()> {
        let i = &self.inner;
        i.check_agent(card)?;
        let run = i.run(card);
        let _updates = run.lock_updates();
        let dropped = i.life().drop_queued(card)?;
        for e in &dropped {
            i.tell_event(e.clone());
        }
        run.stop(i);
        let reason = reason.trim().to_owned();
        let note = activity::StopNote { reason: reason.clone(), dropped: i64::try_from(dropped.len()).unwrap_or(i64::MAX) };
        i.add_event(NewEvent {
            card: card.into(),
            kind: Some(EventKind::System),
            status: activity::STOPPED.into(),
            text: if reason.is_empty() { "You stopped it.".into() } else { format!("You stopped it: {reason}") },
            by: Some(owner()),
            data: serde_json::to_string(&note)?,
            ..NewEvent::default()
        });
        i.tell_agent(card);
        // What was handed to it and dropped now is the health check's to give up on and report.
        i.handoff_tick.notify_one();
        Ok(())
    }

    /// The owner picks `picked` in the choice `part` of block event `event` (remit's ADR-0091): the agent is told in one
    /// queued message from him.
    pub async fn block_pick(&self, event: i64, part: &str, picked: Vec<String>) -> Result<()> {
        self.inner.pick_in_block(event, part, picked).await
    }

    /// The owner overturns the answer given to question `question` with `word`, or answers one left to him.
    pub async fn overturn(&self, question: i64, word: &str) -> Result<()> {
        self.inner.overturn(question, word).await.map(|_| ())
    }

    /// The owner marks agent reply `event` (remit's ADR-0093); the mark teaches its agent.
    pub fn teach(&self, event: i64, mark: TurnMark, words: &str) -> Result<()> {
        self.inner.teach_reply(event, mark, words)
    }

    /// Agent `card` as its drawer shows it.
    pub fn view(&self, card: &str) -> Result<AgentView> {
        self.inner.view(card)
    }

    /// The newest events of agent `card`'s stream, or those before event `before`, oldest first. A page begins at the
    /// newest Start over in it (remit's `card_events`): the conversation before it is the page before.
    pub fn events(&self, card: &str, before: Option<i64>) -> Result<Vec<EventRead>> {
        self.inner.check_agent(card)?;
        let events = self.inner.life().events(card, before, EVENTS_PAGE)?;
        let mut page: Vec<EventRead> = events.into_iter().map(activity::read).collect();
        if let Some(i) = page.iter().rposition(|e| e.activity == Some(Activity::StartedOver)) {
            page.drain(..i);
        }
        Ok(page)
    }

    /// What fills agent `card`'s context and what it was given, counted afresh while its session runs.
    pub async fn context(&self, card: &str) -> Result<lifeos_wire::CardContext> {
        self.inner.check_agent(card)?;
        self.inner.card_context(card).await
    }

    /// What agent `card`'s model requests and tools did, as its sessions exported it.
    pub async fn telemetry(&self, card: &str) -> Result<lifeos_wire::CardTelemetry> {
        self.inner.check_agent(card)?;
        self.inner.card_telemetry(card).await
    }

    /// Keeps what the owner is typing to agent `card`: its words and the images put for it.
    pub fn set_draft(&self, card: &str, text: &str, images: &[ImageRef]) -> Result<()> {
        self.inner.check_agent(card)?;
        self.inner.life().set_draft(card, text, images)?;
        self.inner.tell_agent(card);
        Ok(())
    }

    /// Keeps an image's bytes as `id`, for a draft or a message to name; an id put again keeps its first bytes.
    pub fn put_image(&self, id: &str, image: &Image) -> Result<()> {
        self.inner.life().put_image(id, image)
    }

    /// The image kept as `id`.
    pub fn image(&self, id: &str) -> Result<Image> {
        self.inner.life().image(id)
    }

    /// Retires only sessions whose MCP configuration contains a revoked connection.
    pub fn revoke_connections(&self, names: &[String]) {
        let runs: Vec<Arc<Run>> = self.inner.runs.lock().unwrap_or_else(PoisonError::into_inner).values().cloned().collect();
        for run in runs {
            if run.uses_connection(names) {
                run.close(&self.inner);
            }
        }
    }

    /// Closes every session.
    pub fn close(&self) {
        let runs: Vec<Arc<Run>> = self.inner.runs.lock().unwrap_or_else(PoisonError::into_inner).values().cloned().collect();
        for r in runs {
            r.close(&self.inner);
        }
    }
}

/// The owner, as an actor.
pub fn owner() -> Actor {
    Actor { kind: ActorKind::Owner, id: String::new(), name: "You".into() }
}

/// A routine, as an actor.
pub fn routine(id: &str, name: &str) -> Actor {
    Actor { kind: ActorKind::Routine, id: id.into(), name: name.into() }
}

/// What the door's tokens open: the toolbox of the card whose run holds one.
struct DoorOpens(Weak<Inner>);

impl Opens for DoorOpens {
    fn toolbox(&self, token: &str, server: Server) -> Option<Toolbox> {
        let inner = self.0.upgrade()?;
        let card = inner.tokens.card_of(token)?;
        match server {
            Server::Lifeos => inner.toolbox(&card).ok(),
            Server::Google => inner.google_toolbox(&card).ok().flatten(),
            Server::Desktop => inner.desktop_toolbox(&card).ok().flatten(),
        }
    }

    fn telemetry(&self, token: &str) -> Option<(String, otel::Store)> {
        let inner = self.0.upgrade()?;
        let card = inner.tokens.card_of(token)?;
        Some((card, inner.telemetry.clone()))
    }
}

impl Inner {
    pub(crate) fn life(&self) -> MutexGuard<'_, Life> {
        self.life.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn member(&self, card: &str) -> Result<Option<CrewMember>> {
        if card == ASSISTANT || card == LIBRARIAN {
            return Ok(None);
        }
        Ok(Some(self.life().member(card)?))
    }

    fn check_agent(&self, card: &str) -> Result<()> {
        if let Some(m) = self.member(card)? {
            if m.status != "active" {
                bail!("{} is retired. Ask the Assistant to bring it back.", m.name);
            }
            self.life().ensure_card(card, &m.name)?;
        } else if card == LIBRARIAN {
            self.life().ensure_card(LIBRARIAN, "Librarian")?;
        } else {
            self.life().ensure_card(ASSISTANT, "Assistant")?;
        }
        Ok(())
    }

    /// The run of card, its loop started the first time.
    pub(crate) fn run(self: &Arc<Self>, card: &str) -> Arc<Run> {
        let mut runs = self.runs.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(r) = runs.get(card) {
            return Arc::clone(r);
        }
        let r = Arc::new(Run::new(card));
        runs.insert(card.to_owned(), Arc::clone(&r));
        tokio::spawn(run::work(Arc::downgrade(self), Arc::clone(&r)));
        r
    }

    fn run_of(&self, card: &str) -> Option<Arc<Run>> {
        self.runs.lock().unwrap_or_else(PoisonError::into_inner).get(card).cloned()
    }

    pub(crate) fn card(&self, id: &str) -> Result<Card> {
        self.life().card(id)?.with_context(|| format!("no card {id}"))
    }

    pub(crate) fn tell_event(&self, e: Event) {
        self.notifier.tell(Notice::Event(activity::read(e)));
    }

    pub(crate) fn tell_agent(&self, card: &str) {
        self.notifier.tell(Notice::Agent(card.to_owned()));
    }

    pub(crate) fn add_event(&self, e: NewEvent) -> Event {
        let added = self.life().add_event(e);
        match added {
            Ok(e) => {
                self.tell_event(e.clone());
                e
            }
            Err(err) => {
                tracing::warn!("could not record an event: {err:#}");
                Event {
                    id: 0,
                    card: String::new(),
                    kind: EventKind::System,
                    status: String::new(),
                    tool_call_id: String::new(),
                    tool_kind: String::new(),
                    tool_name: String::new(),
                    title: String::new(),
                    text: String::new(),
                    by: None,
                    data: String::new(),
                    created: String::new(),
                    updated: String::new(),
                    rev: 0,
                }
            }
        }
    }

    pub(crate) fn system(&self, card: &str, note: SystemNote, text: &str) {
        self.add_event(NewEvent {
            card: card.into(),
            kind: Some(EventKind::System),
            status: note.as_str().into(),
            text: text.into(),
            ..NewEvent::default()
        });
    }

    /// Changes card and tells every page.
    pub(crate) fn edit_card(&self, card: &str, f: impl FnOnce(&mut Card)) {
        self.edit_card_quiet(card, |c| {
            f(c);
            true
        });
    }

    /// Changes card when f says it changed, and only then tells every page.
    pub(crate) fn edit_card_quiet(&self, card: &str, f: impl FnOnce(&mut Card) -> bool) {
        let saved = {
            let life = self.life();
            match life.card(card) {
                Ok(Some(mut c)) => {
                    if !f(&mut c) {
                        return;
                    }
                    life.save_card(&mut c)
                }
                Ok(None) => return,
                Err(err) => Err(err),
            }
        };
        match saved {
            Ok(()) => self.tell_agent(card),
            Err(err) => tracing::warn!("could not save card {card}: {err:#}"),
        }
    }

    pub(crate) fn turn_ended(&self, card: &str, prompt: i64) {
        let c = self.card(card).ok();
        let answer = Answer {
            text: c.as_ref().map(|c| c.output.clone()).unwrap_or_default(),
            error: c.as_ref().map(|c| c.error.clone()).filter(|e| !e.is_empty()),
            ms: None,
        };
        let _told = self.turns.send((card.to_owned(), prompt, answer));
    }

    /// What the session reads for prompt: the time, the memories recalled for it, the lead of a new session, what its
    /// agent handed out and the news handed it, who it is from when it is not the owner, then its words.
    pub(crate) fn compose(&self, prompt: &Event, lead: Option<&str>, recalled: &[lifeos_wire::Memory]) -> String {
        let now = Local::now().format("%Y-%m-%d %A %H:%M");
        let mut out = format!("[{now}]");
        if let Some(lead) = lead {
            out.push_str("\n\n");
            out.push_str(lead);
        }
        if let Some(note) = memory::note(recalled, memory::TURN_LEAD) {
            out.push('\n');
            out.push_str(&note);
        }
        if let Some(note) = self.handoff_note(&prompt.card) {
            out.push_str("\n\n");
            out.push_str(&note);
        }
        if let Some(note) = self.stop_note(prompt) {
            out.push_str("\n\n");
            out.push_str(&note);
        }
        out.push_str("\n\n");
        out.push_str(&from_line(prompt.by.as_ref()));
        out.push_str(&prompt.text);
        out
    }

    /// What the owner said as he stopped the agent since it last read a message: the stops recorded after the newest
    /// prompt it took before `prompt`.
    fn stop_note(&self, prompt: &Event) -> Option<String> {
        let before = self.life().events(&prompt.card, Some(prompt.id), 200).ok()?;
        let read = before
            .iter()
            .rev()
            .find(|e| e.kind == EventKind::Prompt && matches!(e.status.as_str(), "delivered" | "steered"))
            .map_or(0, |e| e.id);
        let notes: Vec<String> = before
            .iter()
            .filter(|e| e.id > read && e.kind == EventKind::System && e.status == activity::STOPPED)
            .map(|e| {
                let note: activity::StopNote = activity::data_of(e);
                stop_words(&note)
            })
            .collect();
        (!notes.is_empty()).then(|| notes.join("\n"))
    }

    /// The bytes of images, as the session reads them with a message's words.
    pub(crate) fn images(&self, images: &[ImageRef]) -> Result<Vec<model::Image>> {
        let life = self.life();
        images
            .iter()
            .filter(|r| r.media_type != ImageType::File)
            .map(|r| {
                let i = life.image(&r.id)?;
                let media_type = match i.media_type {
                    ImageType::Png => model::ImageType::Png,
                    ImageType::Jpeg => model::ImageType::Jpeg,
                    ImageType::Gif => model::ImageType::Gif,
                    ImageType::Webp => model::ImageType::Webp,
                    ImageType::File => bail!("expected an image for {}", r.id),
                };
                Ok(model::Image { media_type, data: i.data })
            })
            .collect()
    }

    /// What the session reads of prompt with `text`: the words, then the bytes of the images the prompt names.
    pub(crate) fn prompt_of(&self, prompt: &Event, text: String) -> Result<model::Prompt> {
        let data: activity::PromptData =
            if prompt.data.is_empty() { activity::PromptData::default() } else { serde_json::from_str(&prompt.data).unwrap_or_default() };
        self.message_with_attachments(text, &data.images)
    }

    fn message_with_attachments(&self, mut text: String, attachments: &[ImageRef]) -> Result<model::Prompt> {
        let mut files = attachments.iter().filter(|r| r.media_type == ImageType::File).peekable();
        if files.peek().is_some() {
            text.push_str("\n\nAttached files (paths relative to the workspace):");
            for file in files {
                let path = lifeos_core::life::images::attachment_path(&file.id)?;
                text.push_str("\n- ");
                text.push_str(&serde_json::to_string(&path)?);
            }
        }
        Ok(model::Prompt { text, images: self.images(attachments)? })
    }

    fn compose_steer(&self, _card: &str, text: &str, by: &Actor) -> String {
        format!("{}{text}", from_line(Some(by)))
    }

    /// What a session of card is started with, and the secrets it must never show.
    pub(crate) fn spec(&self, card: &Card, lease: &Lease, token: &str) -> Result<(agent::Spec, Vec<String>)> {
        let member = self.member(&card.id)?;
        let (external, mut redactions, home) = {
            let life = self.life();
            let (external, redactions) = life.runtime_connections(member.as_ref().map(|m| m.connections.as_slice()), &card.id)?;
            (external, redactions, life.home().canonicalize()?)
        };
        let env = self.accounts.environment(&lease.account)?;
        let account_env = match &env.token {
            Some(t) => {
                redactions.push(t.clone());
                agent::parse_account_env(BTreeMap::from([("CLAUDE_CODE_OAUTH_TOKEN".to_owned(), t.clone())]))?
            }
            None => agent::AccountEnv::default(),
        };
        let outside = if card.id == LIBRARIAN { BTreeMap::new() } else { outside_servers(&external)? };
        let door_url = self.door_url.get().cloned().unwrap_or_default();
        let bearer = BTreeMap::from([("Authorization".to_owned(), format!("Bearer {token}"))]);
        let mut mcp = vec![agent::MCPServer { name: door::SERVER_NAME.into(), url: door_url.clone(), headers: bearer.clone() }];
        let door_root = door_url.strip_suffix(door::PATTERN).unwrap_or(&door_url);
        if self.may_use_google(&card.id, member.as_ref()) && self.google.signed_in() {
            mcp.push(agent::MCPServer {
                name: lifeos_core::life::google::CONNECTION.into(),
                url: format!("{door_root}{}", door::GOOGLE_PATTERN),
                headers: bearer.clone(),
            });
        }
        if self.may_use_desktop(&card.id, member.as_ref()) {
            mcp.push(agent::MCPServer {
                name: lifeos_core::life::desktop::CONNECTION.into(),
                url: format!("{door_root}{}", door::DESKTOP_PATTERN),
                headers: bearer,
            });
        }
        let mut allowed: Vec<String> = vec![
            format!("mcp__{}", door::SERVER_NAME),
            "Read(./**)".into(),
            "Edit(./**)".into(),
            "Glob".into(),
            "Grep".into(),
            "WebSearch".into(),
            "WebFetch".into(),
            "TodoWrite".into(),
        ];
        allowed.extend(outside.keys().chain(mcp.iter().skip(1).map(|m| &m.name)).map(|name| format!("mcp__{name}")));
        let mut deny: Vec<String> =
            ["Read(./.*/**)", "Edit(./.*/**)", "Edit(./crew/**)", "Edit(./routines/**)", "Edit(./attachments/**)", "Write(./attachments/**)"].map(String::from).to_vec();
        if member.is_some() {
            // The owner's own words about themself are the Assistant's to keep.
            deny.push("Edit(./CLAUDE.md)".into());
        }
        let (model, effort) = match &member {
            Some(m) => (m.model.clone(), m.effort.clone()),
            None => (self.config.model.clone(), self.config.effort.clone()),
        };
        let spec = agent::Spec {
            card_id: card.id.clone(),
            cwd: home.display().to_string(),
            model: model.unwrap_or_default(),
            effort: effort.unwrap_or_default(),
            account: if lease.account.id == lifeos_core::life::accounts::DEFAULT_ACCOUNT {
                String::new()
            } else {
                lease.account.id.clone()
            },
            account_env,
            mode: "dontAsk".into(),
            mcp,
            outside,
            allowed_tools: allowed,
            deny_tools: deny,
            tools: Some(if card.id == LIBRARIAN {
                Vec::new()
            } else {
                BUILTIN_TOOLS.iter().map(|t| (*t).to_owned()).collect()
            }),
            builtin_agents: Some(Vec::new()),
            system_append: if card.id == LIBRARIAN {
                librarian::BRIEF.to_owned()
            } else {
                brief::system_prompt(member.as_ref())
            },
            resume: card.session_id.clone(),
            telemetry: Some(agent::Telemetry {
                endpoint: format!("{}{}", door_url.strip_suffix(door::PATTERN).unwrap_or(&door_url), door::OTLP_PATTERN),
                token: token.to_owned(),
            }),
            ..agent::Spec::default()
        };
        Ok((spec, redactions))
    }

    /// The tools card's session is served: lifeos's own (a crew member's narrowed to those it was given, memory
    /// always), automation and crew only for the Assistant in a conversation with the owner.
    pub(crate) fn toolbox(self: &Arc<Self>, card: &str) -> Result<Toolbox> {
        let member = self.member(card)?;
        let only: Option<Vec<String>> = member.as_ref().filter(|m| !m.tools.is_empty()).map(|m| {
            m.tools.iter().cloned().chain(MEMORY_TOOLS.iter().map(|t| (*t).to_owned())).collect()
        });
        let holder = card.to_owned();
        let mut toolbox = Toolbox::default();
        if card == LIBRARIAN {
            self.add_librarian_tools(&mut toolbox);
            return Ok(toolbox);
        }
        for spec in tools::specs() {
            let allowed = (holder == ASSISTANT || !tools::manages_automation(spec.name))
                && !LIBRARIAN_WRITES.contains(&spec.name)
                && (holder == ASSISTANT || !ASSISTANT_ONLY.contains(&spec.name))
                && only.as_ref().is_none_or(|o| o.iter().any(|t| t == spec.name));
            if !allowed {
                continue;
            }
            let schema = spec.input_schema.as_object().cloned().unwrap_or_default();
            let (name, writes) = (spec.name, !spec.read_only);
            let inner = Arc::downgrade(self);
            let holder = holder.clone();
            toolbox.add(name, spec.description, schema, spec.read_only, move |args| {
                let inner = inner.clone();
                let holder = holder.clone();
                Box::pin(async move {
                    let i = inner.upgrade().context("lifeos stopped")?;
                    let automated = i.run_of(&holder).is_some_and(|r| r.automated());
                    let value = {
                        let mut life = i.life();
                        tools::call_with_origin(&mut life, &holder, name, serde_json::Value::Object(args), automated)?
                    };
                    if writes {
                        i.config.on_change.fire();
                    }
                    if name == "rate_memory"
                        && let Some(id) = value.get("id").and_then(serde_json::Value::as_i64)
                    {
                        i.review_if_misleading(id);
                    }
                    Ok(value.to_string())
                })
            });
        }
        if let Some(spec) = tools::specs().into_iter().find(|t| t.name == "recall") {
            let schema = spec.input_schema.as_object().cloned().unwrap_or_default();
            let (inner, holder) = (Arc::downgrade(self), holder.clone());
            toolbox.add("recall", spec.description, schema, true, move |args| {
                let (inner, holder) = (inner.clone(), holder.clone());
                Box::pin(async move {
                    let i = inner.upgrade().context("lifeos stopped")?;
                    let query = args.get("query").and_then(|q| q.as_str()).unwrap_or_default().to_owned();
                    let limit = args.get("limit").and_then(serde_json::Value::as_u64).and_then(|n| usize::try_from(n).ok());
                    Ok(serde_json::to_string(&i.recall_search(&holder, &query, limit).await?)?)
                })
            });
        }
        self.add_message_tools(&mut toolbox, card);
        self.add_block_tool(&mut toolbox, card);
        if card == ASSISTANT {
            self.add_workflow_tools(&mut toolbox);
        }
        Ok(toolbox)
    }

    /// Whether card may use the Google app: the Assistant always, a crew member granted the `google` connection,
    /// never the librarian.
    fn may_use_google(&self, card: &str, member: Option<&CrewMember>) -> bool {
        card != LIBRARIAN && member.is_none_or(|m| m.connections.iter().any(|c| c == lifeos_core::life::google::CONNECTION))
    }

    /// The Google app's tools for card's session, when it may use them.
    pub(crate) fn google_toolbox(&self, card: &str) -> Result<Option<Toolbox>> {
        if !self.may_use_google(card, self.member(card)?.as_ref()) {
            return Ok(None);
        }
        let mut toolbox = Toolbox::default();
        for spec in lifeos_google::specs() {
            let (google, name) = (Arc::clone(&self.google), spec.name);
            let schema = spec.schema.as_object().cloned().unwrap_or_default();
            toolbox.add(name, spec.description, schema, spec.reads, move |args| {
                let google = Arc::clone(&google);
                Box::pin(async move { Ok(google.call(name, serde_json::Value::Object(args)).await?.to_string()) })
            });
        }
        Ok(Some(toolbox))
    }

    /// Whether card may use the owner's computers: the Assistant always, a crew member granted the `desktop`
    /// connection, never the librarian.
    fn may_use_desktop(&self, card: &str, member: Option<&CrewMember>) -> bool {
        card != LIBRARIAN && member.is_none_or(|m| m.connections.iter().any(|c| c == lifeos_core::life::desktop::CONNECTION))
    }

    /// The desktop tools for card's session, when it may use them: `desktop_computers` answered here, the rest sent
    /// to the computer they name.
    pub(crate) fn desktop_toolbox(&self, card: &str) -> Result<Option<Toolbox>> {
        if !self.may_use_desktop(card, self.member(card)?.as_ref()) {
            return Ok(None);
        }
        let mut toolbox = Toolbox::default();
        for spec in lifeos_desktop::specs() {
            let (computers, name) = (Arc::clone(&self.computers), spec.name);
            let schema = spec.schema.as_object().cloned().unwrap_or_default();
            toolbox.add_shown(name, spec.description, schema, spec.reads, move |args| {
                let computers = Arc::clone(&computers);
                Box::pin(async move {
                    if name == "desktop_computers" {
                        return Ok(toolbox::Output { text: computers_text(&computers.online()), images: Vec::new() });
                    }
                    let which = args.get("computer").and_then(|c| c.as_str()).map(str::to_owned);
                    let input = serde_json::Value::Object(args).to_string();
                    let answer = computers.ask(which.as_deref(), name, input).await?;
                    anyhow::ensure!(answer.ok, "{}", answer.text);
                    Ok(toolbox::Output {
                        text: answer.text,
                        images: answer.jpeg.into_iter().map(|j| ("image/jpeg".to_owned(), j)).collect(),
                    })
                })
            });
        }
        Ok(Some(toolbox))
    }

    fn view(&self, card: &str) -> Result<AgentView> {
        self.check_agent(card)?;
        let (c, draft, stream) = {
            let life = self.life();
            let c = life.card(card)?.with_context(|| format!("no card {card}"))?;
            (c, life.draft(card)?, life.events(card, None, 40)?)
        };
        let run = self.run_of(card);
        let c = run::live(c, run.as_deref());
        let mut standing = standing(&c, run.is_some_and(|r| r.session().is_some()), &stream);
        standing.waiting = self.life().waiting_on(card)?;
        Ok(AgentView { card: c, standing, draft })
    }
}

/// Where an agent stands, in words the server chooses.
fn standing(c: &Card, open: bool, stream: &[Event]) -> AgentStanding {
    let working = matches!(c.status, CardStatus::Working | CardStatus::Starting);
    let (label, tone, doing_tone) = match c.status {
        CardStatus::Working | CardStatus::Starting => ("Working", AgentTone::Running, DoingTone::Busy),
        CardStatus::Waiting => ("Waiting", AgentTone::Waiting, DoingTone::Waiting),
        CardStatus::Failed => ("Failed", AgentTone::Failed, DoingTone::Error),
        CardStatus::Idle => ("Idle", AgentTone::Idle, DoingTone::Idle),
    };
    let taken = stream
        .iter()
        .rev()
        .find(|e| e.kind == EventKind::Prompt && e.status == "delivered")
        .filter(|_| working);
    let on_message = taken.map(|p| OnMessage {
        event_id: p.id,
        answering: stream.iter().any(|e| e.id > p.id && e.kind == EventKind::Message),
        at: p.updated.clone(),
    });
    let mut asks = vec![Ask::Message, Ask::Restart];
    if working {
        asks.push(Ask::Interrupt);
        if open {
            asks.push(Ask::Steer);
        }
    }
    let stop_says = if working || c.queued > 0 {
        asks.push(Ask::Stop);
        STOP_SAYS.to_owned()
    } else {
        String::new()
    };
    let on_send = (working || c.queued > 0).then(|| match c.queued {
        0 => "Waits for the current turn".to_owned(),
        n => format!("Waits for the current turn (+{n} queued)"),
    });
    AgentStanding {
        card: c.id.clone(),
        label: label.into(),
        tone,
        doing: if c.status == CardStatus::Failed { c.error.clone() } else { c.activity.clone() },
        doing_tone,
        work: if working {
            c.plan.iter().find(|p| p.status == "in_progress").map(|p| p.content.clone()).unwrap_or_default()
        } else {
            String::new()
        },
        since: on_message.as_ref().map(|m| m.at.clone()),
        waiting: Vec::new(),
        asks,
        stop_says,
        remembers: true,
        on_message,
        on_send,
    }
}

/// The computers online, as an agent reads them.
fn computers_text(online: &[lifeos_wire::ComputerHello]) -> String {
    if online.is_empty() {
        return "No computer is online. The owner runs LifeOS Computer on a Mac to let you use it (Integrations → Desktop)."
            .to_owned();
    }
    online
        .iter()
        .map(|c| {
            let mut missing = Vec::new();
            if !c.accessibility {
                missing.push("Accessibility");
            }
            if !c.screen_recording {
                missing.push("Screen Recording");
            }
            format!(
                "{:?} (id {}): screen {}×{} points{}{}",
                c.name,
                c.id,
                c.width,
                c.height,
                if c.paused { ", paused by the owner: don't use it" } else { "" },
                if missing.is_empty() { String::new() } else { format!(", macOS hasn't granted {}", missing.join(" and ")) }
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// What stopping an agent does, as the owner reads it before he stops it.
const STOP_SAYS: &str = "Its turn ends and the messages waiting for it are dropped. Your why stays in its stream, and it \
                         reads it with your next message.";

/// What an agent reads of a stop of the owner's. Pure.
fn stop_words(note: &activity::StopNote) -> String {
    let dropped = match note.dropped {
        0 => String::new(),
        1 => ", and dropped 1 message that waited for you".to_owned(),
        n => format!(", and dropped {n} messages that waited for you"),
    };
    let why = if note.reason.is_empty() { "He did not say why.".to_owned() } else { format!("Why: {}", note.reason) };
    format!("[the owner stopped you{dropped}] {why}")
}

/// The line that says who a prompt is from, when it is not the owner.
fn from_line(by: Option<&Actor>) -> String {
    match by {
        Some(a) if a.kind == ActorKind::Routine => format!("[Routine {}]\n", a.name),
        Some(a) if a.kind == ActorKind::Workflow => format!("[Workflow {}]\n", a.name),
        Some(a) if matches!(a.kind, ActorKind::Agent | ActorKind::Assistant) => format!("[from {}]\n", a.name),
        Some(a) if a.kind == ActorKind::Lifeos => "[lifeos]\n".to_owned(),
        _ => String::new(),
    }
}

/// The owner's connections as Claude Code's `--mcp-config` takes them.
fn outside_servers(
    external: &serde_json::Map<String, serde_json::Value>,
) -> Result<BTreeMap<String, claudecode::MCPServer>> {
    let text_map = |v: Option<&serde_json::Value>| -> BTreeMap<String, String> {
        v.and_then(|v| v.as_object())
            .map(|o| o.iter().filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_owned()))).collect())
            .unwrap_or_default()
    };
    external
        .iter()
        .map(|(name, v)| {
            let kind = v.get("type").and_then(|t| t.as_str()).unwrap_or("stdio");
            let server = match kind {
                "http" => claudecode::MCPServer::Http {
                    url: v.get("url").and_then(|u| u.as_str()).unwrap_or_default().to_owned(),
                    headers: text_map(v.get("headers")),
                    timeout: 300_000,
                },
                "sse" => claudecode::MCPServer::Sse {
                    url: v.get("url").and_then(|u| u.as_str()).unwrap_or_default().to_owned(),
                    headers: text_map(v.get("headers")),
                },
                _ => claudecode::MCPServer::Stdio {
                    command: v.get("command").and_then(|c| c.as_str()).context("a connection without a command")?.to_owned(),
                    args: v
                        .get("args")
                        .and_then(|a| a.as_array())
                        .map(|a| a.iter().filter_map(|s| s.as_str().map(String::from)).collect())
                        .unwrap_or_default(),
                    env: text_map(v.get("env")),
                },
            };
            Ok((name.clone(), server))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_routine_says_who_it_is() {
        assert_eq!(from_line(Some(&routine("r1", "Tóm tắt"))), "[Routine Tóm tắt]\n");
        assert_eq!(from_line(Some(&owner())), "");
    }
}
