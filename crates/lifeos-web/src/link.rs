//! The link's server side: every session at `PATH`, every call on its own
//! bidirectional stream (see `lifeos_wire` for the stream rules).

use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use lifeos_agent::LifeAgent;
use lifeos_wire::frame::MAX_FRAME;
use lifeos_wire::{Call, ChatDone, PATH, Progress, Reader, Reply, encode};
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

/// The final reply to a call; progress frames are written as they come.
async fn answer(call: Call, state: &AppState, send: &mut SendStream) -> Result<Reply> {
    match call {
        Call::Today => {
            let mut life = state.life.lock().expect("life lock");
            life.sync_index()?;
            Ok(Reply::Today(life.today(lifeos_core::life::today())?))
        }
        Call::ResetChat => {
            if let Some(agent) = state.agent.lock().await.take() {
                agent.close().await;
            }
            Ok(Reply::Done)
        }
        Call::Chat { message } => {
            let mut slot = state.agent.lock().await;
            if slot.is_none() {
                *slot = Some(LifeAgent::spawn(state.life.clone(), state.agent_config.clone()).await?);
            }
            let agent = slot.as_ref().expect("agent just started");

            let (tx, mut rx) = mpsc::unbounded_channel::<Progress>();
            let ask = agent.ask(&message, move |p| {
                let _ = tx.send(p);
            });
            tokio::pin!(ask);
            let mut tools = Vec::new();
            let reply = loop {
                tokio::select! {
                    biased;
                    Some(progress) = rx.recv() => {
                        if let Progress::Tool(name) = &progress {
                            tools.push(name.clone());
                        }
                        write(send, &Reply::Progress(progress)).await?;
                    }
                    reply = &mut ask => break reply?,
                }
            };
            // What the agent reported just before it finished.
            while let Ok(progress) = rx.try_recv() {
                if let Progress::Tool(name) = &progress {
                    tools.push(name.clone());
                }
                write(send, &Reply::Progress(progress)).await?;
            }
            Ok(Reply::ChatDone(ChatDone {
                text: reply.text,
                tools,
                cost_usd: reply.cost_usd,
            }))
        }
    }
}
