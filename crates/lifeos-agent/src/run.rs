//! One card at work (remit's `chains::session`, `outbox` and `stream`): its loop takes the next queued prompt of its
//! stream, opens or resumes its Claude Code session, and turns what the session does into events, pushed to every page
//! as they happen. A message and a thought grow as one event each, saved every `SAVE_EVERY` and pushed as deltas.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};
use std::time::{Duration, Instant};

use agent::{BoxFuture, Exit, Handler, Update};
use lifeos_core::life::cards::NewEvent;
use lifeos_wire::{ActorKind, Card, CardStatus, Event, EventDelta, EventKind, PlanEntry};
use model::runtime::{
    FailureCategory, Limit, Moment, Note, PermissionAnswer, PermissionRequest, Task, ToolCall, Usage,
};
use model::{StopReason, SystemNote};
use serde_json::json;
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

use crate::accounts::{AccountObservation, Lease};
use crate::activity::{NewSessionData, StopData, turn_summary};
use crate::notify::Notice;
use crate::{Inner, brief};

/// How often a growing message is saved; every chunk is pushed at once.
const SAVE_EVERY: Duration = Duration::from_millis(500);

/// A tool's output kept in its event, at most (in characters).
const OUTPUT_CHARS: usize = 4000;

pub(crate) struct Run {
    pub card: String,
    wake: Notify,
    state: Mutex<RunState>,
    updates: Mutex<()>,
}

#[derive(Default)]
struct RunState {
    session: Option<Arc<dyn agent::Session>>,
    lease: Option<Lease>,
    token: Option<String>,
    /// A turn is in progress: the prompt it answers and when it began.
    turn: Option<(i64, Instant)>,
    /// Ends the wait for an account, or the turn in progress.
    cancel: Option<CancellationToken>,
    /// The message or thought growing now.
    growing: Option<Growing>,
    /// Tool calls and tasks of this session, by Claude Code's id: their events.
    tools: HashMap<String, i64>,
    tasks: HashMap<String, i64>,
    /// The turn's prompt came from a routine, not the owner.
    automated: bool,
    /// Connection secrets and the account token, never shown.
    redactions: Vec<String>,
    external_connections: Vec<String>,
    /// The session was opened new: the next prompt sent is its opening.
    opening: bool,
    /// Counts the sessions opened and closed: a session's handler holds the count it was opened at, so the end of a
    /// session lifeos closed itself, or one already replaced, is not taken for the end of the one open now.
    generation: u64,
}

struct Growing {
    id: i64,
    kind: EventKind,
    text: String,
    saved_at: Instant,
    saved_len: usize,
}

impl Run {
    pub fn new(card: &str) -> Self {
        Self { card: card.to_owned(), wake: Notify::new(), state: Mutex::default(), updates: Mutex::default() }
    }

    fn state(&self) -> MutexGuard<'_, RunState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub(crate) fn lock_updates(&self) -> MutexGuard<'_, ()> {
        self.updates.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Stops the session, including any work it started outside a user turn.
    pub(crate) fn stop(&self, i: &Inner) {
        let cancel = self.state().cancel.clone();
        self.close(i);
        if let Some(cancel) = cancel {
            cancel.cancel();
        } else {
            close_growing(i, self);
            end_open_calls(i, self, true);
            i.edit_card(&self.card, |c| {
                c.status = CardStatus::Idle;
                c.activity.clear();
                c.error.clear();
                c.plan.clear();
            });
        }
    }

    /// Wakes the loop: a prompt was queued.
    pub fn ring(&self) {
        self.wake.notify_one();
    }

    pub fn busy(&self) -> bool {
        self.state().turn.is_some()
    }

    /// The prompt the turn in progress answers.
    pub fn prompt_id(&self) -> Option<i64> {
        self.state().turn.map(|(id, _)| id)
    }

    /// The message or thought being written now.
    pub fn writing(&self) -> Option<i64> {
        self.state().growing.as_ref().map(|g| g.id)
    }

    pub fn automated(&self) -> bool {
        self.state().automated
    }

    pub fn session(&self) -> Option<Arc<dyn agent::Session>> {
        self.state().session.clone()
    }

    pub fn redact(&self, text: &str) -> String {
        redact(text, &self.state().redactions)
    }

    /// Ends the turn in progress, or the wait for an account.
    pub async fn interrupt(&self) {
        let (cancel, session) = {
            let st = self.state();
            (st.cancel.clone(), st.session.clone().filter(|_| st.turn.is_some()))
        };
        if let Some(c) = cancel {
            c.cancel();
        }
        if let Some(s) = session
            && let Err(err) = s.cancel().await
        {
            tracing::warn!("could not cancel the turn: {err}");
        }
    }

    /// Whether the open or starting session has any of these connections.
    pub fn uses_connection(&self, names: &[String]) -> bool {
        self.state().external_connections.iter().any(|name| names.contains(name))
    }

    /// Closes the session: the next prompt opens or resumes one.
    pub fn close(&self, inner: &Inner) {
        let (session, token) = {
            let mut st = self.state();
            st.generation = st.generation.wrapping_add(1);
            st.lease = None;
            st.external_connections.clear();
            st.tools.clear();
            st.tasks.clear();
            (st.session.take(), st.token.take())
        };
        if let Some(t) = token {
            inner.tokens.revoke(&t);
        }
        if let Some(s) = session {
            s.close();
        }
    }
}

pub(crate) fn redact(text: &str, secrets: &[String]) -> String {
    secrets.iter().filter(|s| !s.is_empty()).fold(text.to_owned(), |text, secret| text.replace(secret, "[secret]"))
}

fn cut(text: &str, limit: usize) -> String {
    match text.char_indices().nth(limit) {
        Some((i, _)) => format!("{}…", text.get(..i).unwrap_or(text)),
        None => text.to_owned(),
    }
}

fn utf16_len(s: &str) -> i64 {
    i64::try_from(s.encode_utf16().count()).unwrap_or(i64::MAX)
}

/// The loop of card `run`, for as long as lifeos runs.
pub(crate) async fn work(inner: Weak<Inner>, run: Arc<Run>) {
    loop {
        let Some(i) = inner.upgrade() else { return };
        let next = {
            let _updates = run.lock_updates();
            let next = i.life().take_prompt(&run.card);
            next.map(|prompt| {
                prompt.map(|prompt| {
                    let started = Instant::now();
                    let cancel = CancellationToken::new();
                    let mut st = run.state();
                    st.turn = Some((prompt.id, started));
                    st.cancel = Some(cancel.clone());
                    st.automated = prompt.by.as_ref().is_some_and(|b| b.kind == ActorKind::Routine);
                    (prompt, started, cancel)
                })
            })
        };
        match next {
            Ok(Some((prompt, started, cancel))) => {
                i.tell_event(prompt.clone());
                i.tell_agent(&run.card);
                turn(&i, &run, prompt, started, cancel).await;
            }
            Ok(None) => {
                drop(i);
                run.wake.notified().await;
            }
            Err(err) => {
                tracing::error!("cannot read {}'s outbox: {err:#}", run.card);
                drop(i);
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        }
    }
}

/// One turn: the prompt sent to the session (opened first when none is), what it does streamed into events, then its
/// stop event. A provider that refuses before any tool ran hands the turn to another account.
async fn turn(i: &Arc<Inner>, run: &Arc<Run>, prompt: Event, started: Instant, cancel: CancellationToken) {
    i.edit_card(&run.card, |c| {
        c.status = CardStatus::Starting;
        c.activity = "Opening its session".into();
        c.error.clear();
    });
    i.handoff_started(&run.card, &prompt);
    let recalled = tokio::select! {
        biased;
        () = cancel.cancelled() => Vec::new(),
        recalled = i.recall_for_turn(&run.card, &prompt.text) => recalled,
    };
    if !recalled.is_empty() {
        i.note_recalled(&prompt, &recalled);
    }
    let outcome = loop {
        if cancel.is_cancelled() {
            break Ok(StopReason::Cancelled);
        }
        let (session, lead) = match open(i, run, prompt.id, &cancel).await {
            Ok(opened) => opened,
            Err(_) if cancel.is_cancelled() => break Ok(StopReason::Cancelled),
            Err(err) => break Err(err.to_string()),
        };
        i.edit_card(&run.card, |c| {
            c.status = CardStatus::Working;
            c.activity = "Reading your message".into();
        });
        let text = i.compose(&prompt, lead.as_deref(), &recalled);
        if std::mem::take(&mut run.state().opening) {
            i.record_opening(&run.card, session.id(), &text);
        }
        let message = match i.prompt_of(&prompt, text) {
            Ok(message) => message,
            Err(err) => break Err(format!("{err:#}")),
        };
        let tools_before = run.state().tools.len();
        let answered = tokio::select! {
            biased;
            () = cancel.cancelled() => Ok(StopReason::Cancelled),
            r = session.prompt(message) => r,
        };
        close_growing(i, run);
        match answered {
            Ok(reason) => break Ok(reason),
            Err(agent::Error::Failure(f))
                if matches!(f.category, FailureCategory::Limit | FailureCategory::Access)
                    && run.state().tools.len() == tools_before
                    && !cancel.is_cancelled() =>
            {
                // Nothing ran: the turn is the next account's to take.
                if let Some(lease) = &run.state().lease {
                    let mut o = AccountObservation::default();
                    o.failed(&f);
                    i.accounts.observe(&lease.account, &o);
                }
                i.system(&run.card, SystemNote::Moved, &format!("{f}. Trying another Claude account."));
                run.close(i);
            }
            Err(err) => break Err(run.redact(&err.to_string())),
        }
    };
    end_open_calls(i, run, outcome.is_err() || matches!(outcome, Ok(StopReason::Cancelled)));
    let since = prompt.id;
    let stream = i.life().events_after(&run.card, since.saturating_sub(1)).unwrap_or_default();
    let output = stream.iter().rev().find(|e| e.kind == EventKind::Message).map(|e| e.text.clone()).unwrap_or_default();
    let status = match &outcome {
        Ok(StopReason::EndTurn) => "end_turn",
        _ => "cancelled",
    };
    if let Err(why) = &outcome {
        i.system(&run.card, SystemNote::Error, why);
    }
    let data = StopData {
        prompt: Some(prompt.id),
        ms: i64::try_from(started.elapsed().as_millis()).ok(),
        turn: Some(turn_summary(&stream)),
    };
    let stop = i.add_event(NewEvent {
        card: run.card.clone(),
        kind: Some(EventKind::Stop),
        status: status.into(),
        data: serde_json::to_string(&data).unwrap_or_default(),
        ..NewEvent::default()
    });
    {
        let mut st = run.state();
        st.turn = None;
        st.cancel = None;
        st.automated = false;
    }
    if prompt.by.as_ref().is_some_and(|b| b.kind == ActorKind::Owner) && stop.id > 0 {
        i.teach_turn(&run.card, prompt.id, stop.id);
    }
    let failed = match &outcome {
        Err(why) => Some(format!("its turn failed: {why}")),
        Ok(StopReason::Cancelled) => Some("its turn was stopped".to_owned()),
        Ok(_) => None,
    };
    i.handoff_ended(&run.card, &prompt, &output, failed.as_deref()).await;
    i.edit_card(&run.card, |c| {
        c.turns = c.turns.saturating_add(1);
        c.output = output;
        c.activity.clear();
        match &outcome {
            Err(why) => {
                c.status = CardStatus::Failed;
                c.error = why.clone();
            }
            Ok(_) => c.status = CardStatus::Idle,
        }
    });
    i.turn_ended(&run.card, prompt.id);
}

/// The card's session: the open one, or one opened now on an account the pool gives, resuming the card's session when
/// it has one. A new session's lead is the recap of the conversation it does not hold.
async fn open(
    i: &Arc<Inner>,
    run: &Arc<Run>,
    prompt: i64,
    cancel: &CancellationToken,
) -> anyhow::Result<(Arc<dyn agent::Session>, Option<String>)> {
    if let Some(s) = run.session() {
        return Ok((s, None));
    }
    let card = i.card(&run.card)?;
    let preferred = (!card.account.is_empty()).then_some(card.account.as_str());
    let lease = i.accounts.acquire(preferred, cancel, || {
        i.edit_card(&run.card, |c| {
            c.status = CardStatus::Waiting;
            c.activity = "Waiting for a Claude account".into();
        });
    }).await?;
    i.edit_card(&run.card, |c| {
        c.status = CardStatus::Starting;
        c.activity = "Opening its session".into();
    });
    let token = i.tokens.mint(&run.card);
    let (spec, redactions) = match i.spec(&card, &lease, &token) {
        Ok(spec) => spec,
        Err(error) => {
            i.tokens.revoke(&token);
            return Err(error);
        }
    };
    let generation = {
        let mut st = run.state();
        st.redactions = redactions;
        st.external_connections = spec.outside.keys().cloned().collect();
        st.generation
    };
    let still_allowed = (|| -> anyhow::Result<()> {
        if !spec.outside.is_empty() {
            let life = i.life();
            anyhow::ensure!(
                life.github_plugin_enabled()
                    || !life.github_connection_names()?.iter().any(|name| spec.outside.contains_key(name)),
                "GitHub was disabled while opening the session."
            );
        }
        Ok(())
    })();
    if let Err(error) = still_allowed {
        i.tokens.revoke(&token);
        return Err(error);
    }
    let handler: Arc<dyn Handler> = Arc::new(RunHandler { inner: Arc::downgrade(i), run: Arc::clone(run), generation });
    let resumed = !spec.resume.is_empty();
    let given = i.launch_given(&run.card, &spec);
    let started = tokio::select! {
        biased;
        () = cancel.cancelled() => {
            i.tokens.revoke(&token);
            anyhow::bail!("Stopped while opening the session.");
        }
        s = i.launcher.start(spec, handler) => s,
    };
    let session: Arc<dyn agent::Session> = match started {
        Ok(s) => Arc::from(s),
        Err(err) => {
            i.tokens.revoke(&token);
            let mut o = AccountObservation::default();
            o.failure(&err.to_string());
            i.accounts.observe(&lease.account, &o);
            anyhow::bail!("Claude Code did not start: {}", run.redact(&err.to_string()));
        }
    };
    let fresh = !resumed || session.context_lost();
    let session_id = session.id().to_owned();
    let account = lease.account.id.clone();
    {
        let mut st = run.state();
        if st.generation != generation || cancel.is_cancelled() {
            i.tokens.revoke(&token);
            session.close();
            anyhow::bail!("Session configuration changed while opening.");
        }
        st.session = Some(Arc::clone(&session));
        st.lease = Some(lease);
        st.token = Some(token);
        st.opening = fresh;
    }
    i.record_launch(&run.card, &session_id, !fresh, given);
    i.edit_card(&run.card, |c| {
        c.session_id = session_id;
        c.account = account;
    });
    if !fresh {
        return Ok((session, None));
    }
    let lead = if card.session_id.is_empty() && !resumed {
        let before = i.life().events(&run.card, Some(prompt), 200).unwrap_or_default();
        brief::recap(&before)
    } else {
        // The session to resume was gone: say so, and catch it up.
        i.add_event(NewEvent {
            card: run.card.clone(),
            kind: Some(EventKind::System),
            status: SystemNote::NewSession.as_str().into(),
            text: "The session it had was gone: a new one starts from a recap.".into(),
            data: serde_json::to_string(&NewSessionData { started_over: false }).unwrap_or_default(),
            ..NewEvent::default()
        });
        let before = i.life().events(&run.card, Some(prompt), 200).unwrap_or_default();
        brief::recap(&before)
    };
    Ok((session, lead))
}

fn close_growing(i: &Inner, run: &Run) {
    let growing = run.state().growing.take();
    if let Some(g) = growing {
        save_text(i, g.id, &g.text);
    }
}

fn save_text(i: &Inner, id: i64, text: &str) {
    let saved = {
        let life = i.life();
        life.event(id).and_then(|mut e| {
            e.text = text.to_owned();
            life.update_event(&e)
        })
    };
    match saved {
        Ok(e) => i.tell_event(e),
        Err(err) => tracing::warn!("could not save event {id}: {err:#}"),
    }
}

/// The tool calls still open when a turn ends end with it: failed when it was cut.
fn end_open_calls(i: &Inner, run: &Run, cut: bool) {
    let open = i.life().open_tool_calls(&run.card).unwrap_or_default();
    for mut e in open {
        e.status = if cut { "failed" } else { "completed" }.into();
        let saved = i.life().update_event(&e);
        if let Ok(e) = saved {
            i.tell_event(e);
        }
    }
}

struct RunHandler {
    inner: Weak<Inner>,
    run: Arc<Run>,
    /// The run's session count when this handler's session was opened.
    generation: u64,
}

impl RunHandler {
    fn grow(&self, i: &Inner, kind: EventKind, chunk: &str) {
        let chunk = self.run.redact(chunk);
        let mut st = self.run.state();
        let same = st.growing.as_ref().is_some_and(|g| g.kind == kind);
        if !same {
            if let Some(g) = st.growing.take() {
                save_text(i, g.id, &g.text);
            }
            let added = i.life().add_event(NewEvent {
                card: self.run.card.clone(),
                kind: Some(kind),
                text: chunk.clone(),
                ..NewEvent::default()
            });
            match added {
                Ok(e) => {
                    st.growing = Some(Growing {
                        id: e.id,
                        kind,
                        text: chunk.clone(),
                        saved_at: Instant::now(),
                        saved_len: chunk.len(),
                    });
                    drop(st);
                    i.tell_event(e);
                }
                Err(err) => tracing::warn!("could not record a message: {err:#}"),
            }
            return;
        }
        let Some(g) = st.growing.as_mut() else { return };
        let offset = utf16_len(&g.text);
        g.text.push_str(&chunk);
        let delta = EventDelta { id: g.id, card: self.run.card.clone(), kind, text: chunk, offset };
        let save = (g.saved_at.elapsed() >= SAVE_EVERY && g.text.len() > g.saved_len).then(|| {
            g.saved_at = Instant::now();
            g.saved_len = g.text.len();
            (g.id, g.text.clone())
        });
        drop(st);
        i.notifier.tell(Notice::Delta(delta));
        if let Some((id, text)) = save {
            let saved = change_event(i, id, |e| e.text = text);
            if let Err(err) = saved {
                tracing::warn!("could not save a message: {err:#}");
            }
        }
    }

    fn tool(&self, i: &Inner, tc: ToolCall) {
        close_growing(i, &self.run);
        let input = self.run.redact(&serde_json::Value::Object(tc.input).to_string());
        let output = cut(&self.run.redact(&tc.output), OUTPUT_CHARS);
        let data = json!({ "input": serde_json::from_str::<serde_json::Value>(&input).unwrap_or_default(), "output": output })
            .to_string();
        let known = self.run.state().tools.get(&tc.id).copied();
        let title = self.run.redact(&tc.title);
        let saved = match known {
            Some(id) => change_event(i, id, |e| {
                e.status = tc.status.as_str().into();
                e.tool_kind = tc.kind.as_str().into();
                e.tool_name = tc.name.clone();
                e.title = title;
                e.data = data;
            }),
            None => i.life().add_event(NewEvent {
                card: self.run.card.clone(),
                kind: Some(EventKind::Tool),
                status: tc.status.as_str().into(),
                tool_call_id: tc.id.clone(),
                tool_kind: tc.kind.as_str().into(),
                tool_name: tc.name.clone(),
                title,
                data,
                ..NewEvent::default()
            }),
        };
        match saved {
            Ok(e) => {
                self.run.state().tools.insert(tc.id, e.id);
                i.edit_card(&self.run.card, |c| c.activity = format!("Using {}", crate::activity::shown_name(&e.tool_name)));
                i.tell_event(e);
            }
            Err(err) => tracing::warn!("could not record a tool call: {err:#}"),
        }
    }

    fn note(&self, i: &Inner, n: Note) {
        close_growing(i, &self.run);
        i.add_event(NewEvent {
            card: self.run.card.clone(),
            kind: Some(EventKind::System),
            status: SystemNote::from(n.kind).as_str().into(),
            title: n.title,
            text: n.text,
            ..NewEvent::default()
        });
    }

    fn task(&self, i: &Inner, t: Task) {
        let data = json!({ "kind": t.kind, "summary": t.summary, "toolCallId": t.tool_call_id }).to_string();
        let known = self.run.state().tasks.get(&t.id).copied();
        let saved = match known {
            Some(id) => change_event(i, id, |e| {
                e.status = t.status.as_str().into();
                e.title = t.title.clone();
                e.data = data;
            }),
            None => i.life().add_event(NewEvent {
                card: self.run.card.clone(),
                kind: Some(EventKind::Task),
                status: t.status.as_str().into(),
                title: t.title.clone(),
                data,
                ..NewEvent::default()
            }),
        };
        if let Ok(e) = saved {
            self.run.state().tasks.insert(t.id, e.id);
            i.tell_event(e);
        }
    }
}

impl Handler for RunHandler {
    fn on_update(&self, u: Update) {
        let _updates = self.run.lock_updates();
        if self.run.state().generation != self.generation {
            return;
        }
        let Some(i) = self.inner.upgrade() else { return };
        match u {
            Update::MessageChunk(text) => {
                self.grow(&i, EventKind::Message, &text);
                i.edit_card_quiet(&self.run.card, |c| {
                    if c.activity != "Writing" {
                        c.activity = "Writing".into();
                        return true;
                    }
                    false
                });
            }
            Update::ThoughtChunk(text) => self.grow(&i, EventKind::Thought, &text),
            Update::ToolCall(tc) => self.tool(&i, tc),
            Update::Plan(plan) => i.edit_card(&self.run.card, |c| {
                c.plan = plan
                    .into_iter()
                    .map(|p| PlanEntry { content: p.content, priority: p.priority, status: p.status })
                    .collect();
            }),
            Update::Usage(Usage { used, size }) => i.edit_card_quiet(&self.run.card, |c| {
                let (used, size) = (i64::try_from(used).unwrap_or(i64::MAX), i64::try_from(size).unwrap_or(i64::MAX));
                let changed = c.ctx_used != used || c.ctx_size != size;
                c.ctx_used = used;
                c.ctx_size = size;
                changed
            }),
            Update::Note(n) => self.note(&i, n),
        }
    }

    fn on_permission(&self, _req: PermissionRequest) -> BoxFuture<'_, PermissionAnswer> {
        // Sessions run in dontAsk: a call no rule allows is refused, and lifeos never parks work on the owner.
        Box::pin(async {
            PermissionAnswer::Deny { message: "lifeos does not ask: this tool is not allowed here.".into() }
        })
    }

    fn on_init(&self, init: model::SessionInit) {
        if let Some(i) = self.inner.upgrade() {
            i.on_init(&self.run.card, init);
        }
    }

    fn on_context(&self, context: model::SessionContext) {
        let Some(i) = self.inner.upgrade() else { return };
        i.on_context(&self.run.card, context.clone());
        i.edit_card_quiet(&self.run.card, |c| {
            c.ctx_used = i64::try_from(context.total).unwrap_or(i64::MAX);
            c.ctx_size = i64::try_from(context.size).unwrap_or(i64::MAX);
            true
        });
    }

    fn on_task(&self, t: Task) {
        if let Some(i) = self.inner.upgrade() {
            self.task(&i, t);
        }
    }

    fn on_working(&self, working: bool) {
        let _updates = self.run.lock_updates();
        if self.run.state().generation != self.generation {
            return;
        }
        let Some(i) = self.inner.upgrade() else { return };
        // A turn the agent takes on its own (a background task's news) shows as work too.
        if self.run.busy() {
            return;
        }
        i.edit_card(&self.run.card, |c| {
            c.status = if working { CardStatus::Working } else { CardStatus::Idle };
            if !working {
                c.activity.clear();
            }
        });
    }

    fn on_exit(&self, end: Option<Exit>) {
        let Some(i) = self.inner.upgrade() else { return };
        // lifeos closed this session itself (Start over, Stop, another account), or has opened another since: nothing
        // ended under it, and the session open now is not this one to close.
        if self.run.state().generation != self.generation {
            return;
        }
        self.run.close(&i);
        if let Some(e) = end.filter(|e| !matches!(e, Exit::Process(claudecode::ExitError::Stopped))) {
            i.system(&self.run.card, SystemNote::Error, &format!("The session ended: {}", self.run.redact(&e.to_string())));
        }
    }

    fn on_limit(&self, limit: Limit) {
        let Some(i) = self.inner.upgrade() else { return };
        let lease = self.run.state().lease.as_ref().map(|l| l.account.clone());
        if let Some(account) = lease {
            let mut o = AccountObservation::default();
            o.limit(&limit);
            i.accounts.observe(&account, &o);
        }
    }

    /// Just-in-time recall (remit's `tool_recall`): beside a lifeos tool call, what the agent remembers about it.
    fn on_moment(&self, m: Moment) -> BoxFuture<'_, Option<String>> {
        Box::pin(async move {
            let Moment::ToolUsed { tool, input } = m else { return None };
            let i = self.inner.upgrade()?;
            i.recall_for_tool(&self.run.card, &tool, &input).await
        })
    }
}

/// The card as the page sees it now: its status read with its run.
pub(crate) fn live(mut card: Card, run: Option<&Run>) -> Card {
    let (busy, open) = run.map_or((false, false), |r| {
        let st = r.state();
        (st.turn.is_some(), st.session.is_some())
    });
    if busy && card.status == CardStatus::Idle {
        card.status = CardStatus::Working;
    } else if !busy && !open
        && matches!(card.status, CardStatus::Starting | CardStatus::Working | CardStatus::Waiting)
    {
        card.status = CardStatus::Idle;
        card.activity.clear();
        card.plan.clear();
    }
    card
}

/// Changes event id under one hold of the life lock. (`i.life().event(id).and_then(|e| i.life()…)` holds the first
/// guard to the end of the statement and locks again on the same thread: the server hangs.)
fn change_event(i: &Inner, id: i64, f: impl FnOnce(&mut Event)) -> anyhow::Result<Event> {
    let life = i.life();
    let mut e = life.event(id)?;
    f(&mut e);
    life.update_event(&e)
}
