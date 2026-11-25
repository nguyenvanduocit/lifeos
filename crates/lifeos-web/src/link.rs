//! The link's server side: every session at `PATH`, every call on its own
//! bidirectional stream (see `lifeos_wire` for the stream rules).

use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use lifeos_agent::{BRIEF_PROMPT, LifeAgent};
use lifeos_core::tools;
use lifeos_wire::frame::MAX_FRAME;
use lifeos_wire::{Call, ChatDone, PATH, Progress, Reader, Reply, Turn, encode};
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
            let value = tools::call(&mut state.life.lock().expect("life lock"), &tool, input)?;
            Ok(Reply::Json(value.to_string()))
        }
        Call::History => Ok(Reply::History(state.history.lock().expect("history lock").clone())),
        Call::ResetChat => {
            if let Some(agent) = state.agent.lock().await.take() {
                agent.close().await;
            }
            state.history.lock().expect("history lock").clear();
            Ok(Reply::Done)
        }
        Call::Chat { message } => Ok(Reply::ChatDone(chat(state, &message, Some(send)).await?)),
        Call::Brief => Ok(Reply::ChatDone(chat(state, BRIEF_PROMPT, Some(send)).await?)),
        Call::Watch => unreachable!("watch streams are served by `watch`"),
    }
}

/// One message to the web app's conversation, streaming progress to `send`
/// when there is a page waiting. Kept in the history either way.
pub(crate) async fn chat(state: &AppState, message: &str, mut send: Option<&mut SendStream>) -> Result<ChatDone> {
    let mut slot = state.agent.lock().await;
    if slot.is_none() {
        *slot = Some(LifeAgent::spawn(state.life.clone(), state.agent_config.clone()).await?);
    }
    let agent = slot.as_ref().expect("agent just started");

    let (tx, mut rx) = mpsc::unbounded_channel::<Progress>();
    let ask = agent.ask(message, move |p| {
        let _ = tx.send(p);
    });
    tokio::pin!(ask);
    let mut tools = Vec::new();
    let mut forward = async |progress: Progress, tools: &mut Vec<String>| -> Result<()> {
        if let Progress::Tool(name) = &progress {
            tools.push(name.clone());
        }
        if let Some(send) = send.as_deref_mut() {
            // A page that went away does not stop the agent.
            let _ = write(send, &Reply::Progress(progress)).await;
        }
        Ok(())
    };
    let reply = loop {
        tokio::select! {
            biased;
            Some(progress) = rx.recv() => forward(progress, &mut tools).await?,
            reply = &mut ask => break reply,
        }
    };
    // What the agent reported just before it finished.
    while let Ok(progress) = rx.try_recv() {
        forward(progress, &mut tools).await?;
    }
    // The agent may have changed files with its own Edit and Write tools.
    state.bump();
    let reply = reply?;
    let done = ChatDone { text: reply.text, tools, cost_usd: reply.cost_usd };
    state
        .history
        .lock()
        .expect("history lock")
        .push(Turn { ask: message.to_string(), reply: done.clone() });
    Ok(done)
}
