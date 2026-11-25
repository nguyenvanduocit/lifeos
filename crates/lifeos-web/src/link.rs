//! The link's server side: every session at `PATH`, every call on its own
//! bidirectional stream (see `lifeos_wire` for the stream rules).

use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use lifeos_agent::{BRIEF_PROMPT, Notice, owner};
use lifeos_core::tools;
use lifeos_wire::frame::MAX_FRAME;
use lifeos_wire::{ASSISTANT, Call, Delivery, PATH, Reader, Reply, encode};
use tokio::sync::broadcast;
use web_transport_quinn::http::StatusCode;
use web_transport_quinn::{RecvStream, SendStream, Server, Session};

use crate::{AppState, computer};

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
    // The first frame is the call. A page finishes its side after it; a computer keeps writing on it.
    let mut reader = Reader::default();
    let mut buf = vec![0_u8; 64 * 1024];
    let call: Call = loop {
        if let Some(call) = reader.next_frame()? {
            break call;
        }
        anyhow::ensure!(reader.pending() <= MAX_FRAME + 4, "the call is too large");
        match recv.read(&mut buf).await.context("reading the call")? {
            Some(n) => reader.push(&buf[..n]),
            None => anyhow::bail!("the stream ended before a whole call"),
        }
    };

    if call == Call::Watch {
        return watch(send, &state).await;
    }
    if let Call::Computer(hello) = call {
        return computer::serve(hello, send, recv, reader, &state).await;
    }
    let last = match answer(call, &state).await {
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

/// Sends `Changed` now and after every change of a life's data, and the agents' work as it happens: each event, each
/// chunk of a growing message, each agent whose standing changed. Ends when the page goes away (the write fails) or the
/// server stops.
async fn watch(mut send: SendStream, state: &AppState) -> Result<()> {
    let mut changes = state.changes.subscribe();
    let mut notices = state.agents.subscribe();
    let now = *changes.borrow_and_update();
    if write(&mut send, &Reply::Changed(now)).await.is_err() {
        return Ok(());
    }
    loop {
        let reply = tokio::select! {
            changed = changes.changed() => {
                if changed.is_err() { return Ok(()); }
                let now = *changes.borrow_and_update();
                Reply::Changed(now)
            }
            notice = notices.recv() => match notice {
                Ok(Notice::Event(e)) => Reply::Event(e),
                Ok(Notice::Delta(d)) => Reply::Delta(d),
                Ok(Notice::Agent(card)) => match state.agents.view(&card) {
                    Ok(view) => Reply::Agent(view),
                    Err(_) => continue,
                },
                // Fell behind: the page reads everything again.
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    let now = *changes.borrow();
                    Reply::Changed(now)
                }
                Err(broadcast::error::RecvError::Closed) => return Ok(()),
            },
        };
        if write(&mut send, &reply).await.is_err() {
            return Ok(());
        }
    }
}

/// The one reply to a call.
async fn answer(call: Call, state: &AppState) -> Result<Reply> {
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
        Call::Agent { card } => Ok(Reply::Agent(state.agents.view(&card)?)),
        Call::Events { card, before } => Ok(Reply::Events(state.agents.events(&card, before)?)),
        Call::Say { card, text, images, delivery } => {
            state.agents.say_images(&card, &text, &images, owner(), delivery).await?;
            state.agents.set_draft(&card, "", &[])?;
            Ok(Reply::Done)
        }
        Call::Brief => {
            state.agents.say(ASSISTANT, BRIEF_PROMPT, owner(), Delivery::Queue).await?;
            Ok(Reply::Done)
        }
        Call::Interrupt { card } => {
            state.agents.interrupt(&card).await?;
            Ok(Reply::Done)
        }
        Call::Restart { card } => {
            state.agents.restart(&card).await?;
            Ok(Reply::Done)
        }
        Call::Draft { card, text, images } => {
            state.agents.set_draft(&card, &text, &images)?;
            Ok(Reply::Done)
        }
        Call::ImagePut { id, image } => {
            state.agents.put_image(&id, &image)?;
            Ok(Reply::Done)
        }
        Call::ImageGet { id } => Ok(Reply::Image(state.agents.image(&id)?)),
        Call::Context { card } => Ok(Reply::Context(state.agents.context(&card).await?)),
        Call::Telemetry { card } => Ok(Reply::Telemetry(state.agents.telemetry(&card).await?)),
        // The owner's own acts on the agents, which a page may write (docs/adr/0001).
        Call::BlockPick { event, part, picked } => {
            state.agents.block_pick(event, &part, picked).await?;
            Ok(Reply::Done)
        }
        Call::Overturn { question, word } => {
            state.agents.overturn(question, &word).await?;
            Ok(Reply::Done)
        }
        Call::Teach { event, mark, words } => {
            state.agents.teach(event, mark, &words)?;
            Ok(Reply::Done)
        }
        Call::Stop { card, reason } => {
            state.agents.stop(&card, &reason).await?;
            Ok(Reply::Done)
        }
        Call::Watch => anyhow::bail!("a watch is served on its own stream"),
        Call::Computer(_) => anyhow::bail!("a computer is served on its own stream"),
    }
}
