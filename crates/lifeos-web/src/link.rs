//! The link's server side: every session at `PATH`, every call on its own
//! bidirectional stream (see `lifeos_wire` for the stream rules).

use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use chrono::Local;
use lifeos_agent::{AgentConfig, BRIEF_PROMPT, Interrupter, LifeAgent};
use lifeos_core::tools;
use lifeos_wire::frame::MAX_FRAME;
use lifeos_wire::{ASSISTANT, Call, ChatDone, CrewMember, PATH, Reader, Reply, Step, Turn, encode};
use tokio::sync::mpsc;
use web_transport_quinn::http::StatusCode;
use web_transport_quinn::{RecvStream, SendStream, Server, Session};

use crate::AppState;

pub async fn accept(mut server: Server, state: Arc<AppState>) {
    while let Some(request) = server.accept().await {
        let state = state.clone();
        tokio::spawn(async move {
            if request.url.path() != PATH {
                let _ = request.reject(StatusCode::NOT_FOUND).await;
                return;
            }
            match request.ok().await {
                Ok(session) => serve_session(session, state).await,
                Err(err) => tracing::warn!("link: session refused: {err}"),
            }
        });
    }
}

async fn serve_session(session: Session, state: Arc<AppState>) {
    // Ends when the page goes away: its error is the session closing.
    while let Ok((send, recv)) = session.accept_bi().await {
        let state = state.clone();
        tokio::spawn(async move {
            if let Err(err) = serve_call(send, recv, state).await {
                tracing::warn!("link: call failed: {err:#}");
            }
        });
    }
}

async fn serve_call(mut send: SendStream, mut recv: RecvStream, state: Arc<AppState>) -> Result<()> {
    let bytes = recv.read_to_end(MAX_FRAME + 4).await.context("reading the call")?;
    let mut reader = Reader::default();
    reader.push(&bytes);
    let call: Call = reader.next_frame()?.context("the stream ended before a whole call")?;

    if call == Call::Watch {
        return watch(send, &state).await;
    }
    let last = match answer(call, &state, &mut send).await {
        Ok(reply) => reply,
        Err(err) => Reply::Failed(format!("{err:#}")),
    };
    write(&mut send, &last).await?;
    send.finish().map_err(|e| anyhow!("finishing the stream: {e}"))?;
    Ok(())
}

async fn write(send: &mut SendStream, reply: &Reply) -> Result<()> {
    send.write_all(&encode(reply)?).await.context("writing a reply")
}

/// Sends `Changed` now and after every change. Ends when the page goes
/// away (the write fails) or the server stops.
async fn watch(mut send: SendStream, state: &AppState) -> Result<()> {
    let mut changes = state.changes.subscribe();
    loop {
        let revision = *changes.borrow_and_update();
        if write(&mut send, &Reply::Changed(revision)).await.is_err() {
            return Ok(());
        }
        if changes.changed().await.is_err() {
            return Ok(());
        }
    }
}

/// An agent the page talks to: its Claude Code session once started, the
/// crew member it was started as (to restart it when the member changes),
/// and how to stop its running turn.
#[derive(Default)]
pub struct Seat {
    agent: tokio::sync::Mutex<Option<(LifeAgent, Option<CrewMember>)>>,
    interrupter: std::sync::Mutex<Option<Interrupter>>,
}

/// The final reply to a call; progress frames are written as they come.
async fn answer(call: Call, state: &AppState, send: &mut SendStream) -> Result<Reply> {
    match call {
        Call::Read { tool, input } => {
            // The page only looks: anything that changes a life goes through the agent.
            let spec = tools::spec(&tool).with_context(|| format!("unknown tool '{tool}'"))?;
            if !spec.read_only {
                anyhow::bail!("'{tool}' changes data; ask the agent instead");
            }
            let input = if input.trim().is_empty() { "{}" } else { &input };
            let input = serde_json::from_str(input).context("the input is not JSON")?;
            let value = tools::call(&mut state.life.lock().expect("life lock"), ASSISTANT, &tool, input)?;
            Ok(Reply::Json(value.to_string()))
        }
        Call::History { agent } => Ok(Reply::History(state.life.lock().expect("life lock").turns(&agent)?)),
        Call::ResetChat { agent } => {
            let seat = state.seat(&agent);
            if let Some((running, _)) = seat.agent.lock().await.take() {
                running.close().await;
            }
            state.life.lock().expect("life lock").reset_conversation(&agent)?;
            state.bump();
            Ok(Reply::Done)
        }
        Call::Stop { agent } => {
            let interrupter = state.seat(&agent).interrupter.lock().expect("interrupter lock").clone();
            if let Some(i) = interrupter {
                i.interrupt().await?;
            }
            Ok(Reply::Done)
        }
        Call::Chat { agent, message } => Ok(Reply::ChatDone(chat(state, &agent, &message, Some(send)).await?)),
        Call::Brief => Ok(Reply::ChatDone(chat(state, ASSISTANT, BRIEF_PROMPT, Some(send)).await?)),
        Call::Watch => unreachable!("watch streams are served by `watch`"),
    }
}

/// One message to an agent, streaming its steps to `send` when a page is
/// waiting. The turn is kept either way.
pub(crate) async fn chat(state: &AppState, agent: &str, message: &str, mut send: Option<&mut SendStream>) -> Result<ChatDone> {
    let (member, resume) = {
        let life = state.life.lock().expect("life lock");
        let member = if agent == ASSISTANT { None } else { Some(life.member(agent)?) };
        if member.as_ref().is_some_and(|m| m.status != "active") {
            anyhow::bail!("{agent} is retired; ask the Assistant to bring it back");
        }
        (member, life.session_of(agent)?)
    };
    let seat = state.seat(agent);
    let mut slot = seat.agent.lock().await;
    // A member whose role, instructions or tools changed starts again, in
    // the same conversation.
    let stale = slot.as_ref().is_some_and(|(_, was)| was.as_ref().map(|m| (&m.instructions, &m.role, &m.tools, &m.model, &m.effort))
        != member.as_ref().map(|m| (&m.instructions, &m.role, &m.tools, &m.model, &m.effort)));
    if stale && let Some((old, _)) = slot.take() {
        old.close().await;
    }
    if slot.is_none() {
        let config = AgentConfig { member: member.clone(), resume, ..state.agent_config.clone() };
        let spawned = LifeAgent::spawn(state.life.clone(), config).await?;
        *slot = Some((spawned, member));
    }
    let (running, _) = slot.as_ref().expect("agent just started");
    *seat.interrupter.lock().expect("interrupter lock") = Some(running.interrupter());

    let at = Local::now().format("%Y-%m-%d %H:%M").to_string();
    let (tx, mut rx) = mpsc::unbounded_channel::<Step>();
    let ask = running.ask(message, move |step| {
        let _ = tx.send(step);
    });
    tokio::pin!(ask);
    let mut forward = async |step: Step| {
        if let Some(send) = send.as_deref_mut() {
            // A page that went away does not stop the agent.
            let _ = write(send, &Reply::Progress(step)).await;
        }
    };
    let done = loop {
        tokio::select! {
            biased;
            Some(step) = rx.recv() => forward(step).await,
            done = &mut ask => break done,
        }
    };
    // What the agent reported just before it finished.
    while let Ok(step) = rx.try_recv() {
        forward(step).await;
    }
    *seat.interrupter.lock().expect("interrupter lock") = None;
    let session = running.session_id();
    // The agent may have changed files with its own Edit and Write tools.
    state.bump();
    let done = done?;
    {
        let life = state.life.lock().expect("life lock");
        if let Some(id) = session {
            life.set_session_of(agent, &id)?;
        }
        life.save_turn(agent, &Turn { ask: message.to_string(), at, reply: done.clone(), source: None })?;
    }
    state.bump();
    Ok(done)
}
