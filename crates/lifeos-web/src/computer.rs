//! A computer's stream on the link (`lifeos_wire::computer`): LifeOS Computer said who it is with `Call::Computer`
//! and keeps the stream open. The server writes it the desktop tools agents call, and a ping now and then; the app
//! writes back each answer, and its status whenever that changes. Its stream ending, or a ping it leaves unanswered,
//! means it went away.

use std::time::Duration;

use anyhow::{Context, Result};
use axum::body::Body;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use lifeos_wire::{ComputerAsk, ComputerHello, ComputerSays, Reader, Reply, encode};
use web_transport_quinn::{RecvStream, SendStream};

use crate::AppState;

/// How often the server checks the computer is still there, and how long it waits for the answer.
const PING: Duration = Duration::from_secs(20);
const PONG: Duration = Duration::from_secs(15);

pub(crate) async fn serve(
    hello: ComputerHello,
    mut send: SendStream,
    mut recv: RecvStream,
    mut reader: Reader,
    state: &AppState,
) -> Result<()> {
    anyhow::ensure!(!hello.id.trim().is_empty() && hello.id.len() <= 100, "a computer needs an id");
    let id = hello.id.clone();
    let name = hello.name.clone();
    state.life.lock().expect("life lock").computer_hello(&hello)?;
    let computers = state.agents.computers();
    let mut attached = computers.attach(hello);
    state.bump();
    tracing::info!("computer {name:?} ({id}) connected");

    let mut buf = vec![0_u8; 64 * 1024];
    let mut ping = tokio::time::interval(PING);
    ping.tick().await;
    let mut pinged: Option<tokio::time::Instant> = None;
    let why: Result<()> = loop {
        // What came already, before reading more.
        while let Some(says) = reader.next_frame::<ComputerSays>()? {
            match says {
                ComputerSays::Answer(a) if a.id == 0 => pinged = None,
                ComputerSays::Answer(a) => computers.answer(&id, a),
                ComputerSays::Status(h) => {
                    let h = ComputerHello { id: id.clone(), ..h };
                    state.life.lock().expect("life lock").computer_hello(&h)?;
                    computers.update(attached.epoch, h);
                    state.bump();
                }
            }
        }
        tokio::select! {
            read = recv.read(&mut buf) => match read.context("reading from the computer") {
                Ok(Some(n)) => reader.push(&buf[..n]),
                Ok(None) => break Ok(()),
                Err(e) => break Err(e),
            },
            ask = attached.asks.recv() => match ask {
                Some(ask) => {
                    if let Err(e) = write(&mut send, ask).await {
                        break Err(e);
                    }
                }
                // Replaced by a newer stream of the same computer.
                None => break Ok(()),
            },
            _ = ping.tick() => {
                if pinged.is_some_and(|at| at.elapsed() > PONG) {
                    break Err(anyhow::anyhow!("it stopped answering"));
                }
                if pinged.is_none() {
                    pinged = Some(tokio::time::Instant::now());
                    if let Err(e) = write(&mut send, ComputerAsk { id: 0, op: "ping".into(), input: String::new() }).await {
                        break Err(e);
                    }
                }
            }
        }
    };
    if computers.detach(&id, attached.epoch) {
        state.life.lock().expect("life lock").computer_gone(&id)?;
        state.bump();
    }
    match why {
        Ok(()) => tracing::info!("computer {name:?} ({id}) went away"),
        Err(e) => tracing::info!("computer {name:?} ({id}) went away: {e:#}"),
    }
    Ok(())
}

async fn write(send: &mut SendStream, ask: ComputerAsk) -> Result<()> {
    send.write_all(&encode(&Reply::ComputerAsk(ask))?).await.context("writing to the computer")
}

/// Where the page offers LifeOS Computer for another Mac: the zip `scripts/build-computer.sh` made, which the install
/// script copies to `computer_zip()`.
pub(crate) const DOWNLOAD_PATH: &str = "/downloads/LifeOS-Computer.zip";

/// The zip served at [`DOWNLOAD_PATH`]: `$LIFEOS_COMPUTER_ZIP`, or beside the installed binary's data.
fn computer_zip() -> Option<std::path::PathBuf> {
    std::env::var_os("LIFEOS_COMPUTER_ZIP")
        .map(Into::into)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".local/share/lifeos/LifeOS-Computer.zip")))
}

pub(crate) async fn download() -> Response {
    let Some(path) = computer_zip() else { return StatusCode::NOT_FOUND.into_response() };
    match tokio::fs::read(&path).await {
        Ok(bytes) => (
            [
                (header::CONTENT_TYPE, "application/zip"),
                (header::CONTENT_DISPOSITION, "attachment; filename=\"LifeOS-Computer.zip\""),
                (header::CACHE_CONTROL, "no-store"),
            ],
            Body::from(bytes),
        )
            .into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "LifeOS Computer isn't built on the server yet: make computer\n").into_response(),
    }
}
