//! The link to the server: finds it through `/link.json`, dials it over QUIC with the certificate pinned as a page
//! does, says who this computer is, and runs what the agents ask until the stream ends; then dials again.

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use lifeos_wire::{Call, ComputerAnswer, ComputerAsk, ComputerHello, ComputerSays, LinkInfo, Reader, Reply, encode};
use tokio::sync::{mpsc, oneshot};
use web_transport_quinn::ClientBuilder;

use crate::shared::{Shared, Status, computer_name};

/// One desktop tool for the desktop thread to run.
pub struct Job {
    pub op: String,
    pub input: String,
    pub reply: oneshot::Sender<Result<lifeos_desktop::Answer, String>>,
}

/// Dials the server for as long as the app runs, waiting longer after each failure (up to half a minute).
pub async fn keep(shared: Arc<Shared>, desktop: std::sync::mpsc::Sender<Job>) {
    let mut wait = Duration::from_secs(1);
    loop {
        shared.set_status(Status::Connecting);
        let outcome = tokio::select! {
            r = session(&shared, &desktop) => r,
            () = shared.redial.notified() => Ok(()),
        };
        match outcome {
            Ok(()) => wait = Duration::from_secs(1),
            Err(e) => {
                eprintln!("lifeos-computer: {e:#}");
                shared.set_status(Status::Failed(format!("{e:#}")));
            }
        }
        tokio::select! {
            () = tokio::time::sleep(wait) => {}
            () = shared.redial.notified() => wait = Duration::ZERO,
        }
        wait = (wait * 2).clamp(Duration::from_secs(1), Duration::from_secs(30));
    }
}

/// Who this computer is now.
pub fn hello(shared: &Shared) -> ComputerHello {
    let c = shared.config();
    #[cfg(target_os = "macos")]
    let (p, (width, height)) = (lifeos_desktop::Permissions::check(false), lifeos_desktop::screen_size());
    #[cfg(target_os = "macos")]
    let (accessibility, screen_recording) = (p.accessibility, p.screen_recording);
    #[cfg(not(target_os = "macos"))]
    let (accessibility, screen_recording, width, height) = (false, false, 0, 0);
    ComputerHello {
        id: c.id,
        name: computer_name(),
        version: env!("CARGO_PKG_VERSION").into(),
        accessibility,
        screen_recording,
        width,
        height,
        paused: c.paused,
    }
}

fn unhex(hex: &str) -> Result<Vec<u8>> {
    anyhow::ensure!(hex.len() % 2 == 0, "a certificate hash is hex");
    (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(&hex[i..i + 2], 16).map_err(Into::into)).collect()
}

async fn session(shared: &Shared, desktop: &std::sync::mpsc::Sender<Job>) -> Result<()> {
    let server = shared.config().server;
    let info: LinkInfo = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?
        .get(format!("{server}/link.json"))
        .send()
        .await
        .with_context(|| format!("can't reach {server}"))?
        .error_for_status()?
        .json()
        .await
        .context("link.json")?;
    let hashes = info.certificate_hashes.iter().map(|h| unhex(h)).collect::<Result<Vec<_>>>()?;
    let client = ClientBuilder::new().with_server_certificate_hashes(hashes)?;
    let url: url::Url = info.url.parse().context("the link's URL")?;
    let session = tokio::time::timeout(Duration::from_secs(10), client.connect(url))
        .await
        .map_err(|_| anyhow!("the link at {} didn't answer (is this Mac on the tailnet?)", info.url))?
        .context("dialing the link")?;
    let (mut send, mut recv) = session.open_bi().await.context("opening the stream")?;
    let mut said = hello(shared);
    send.write_all(&encode(&Call::Computer(said.clone()))?).await?;
    shared.set_status(Status::Connected);

    let (done_tx, mut done_rx) = mpsc::unbounded_channel::<ComputerAnswer>();
    let mut reader = Reader::default();
    let mut buf = vec![0_u8; 64 * 1024];
    let mut check = tokio::time::interval(Duration::from_secs(2));
    loop {
        while let Some(reply) = reader.next_frame::<Reply>()? {
            match reply {
                Reply::ComputerAsk(ask) => {
                    if let Some(now) = start(shared, desktop, ask, done_tx.clone()) {
                        send.write_all(&encode(&ComputerSays::Answer(now))?).await?;
                    }
                }
                Reply::Failed(why) => bail!("the server refused: {why}"),
                other => bail!("the server sent {other:?}"),
            }
        }
        tokio::select! {
            read = recv.read(&mut buf) => match read? {
                Some(n) => reader.push(&buf[..n]),
                None => return Ok(()),
            },
            Some(answer) = done_rx.recv() => {
                send.write_all(&encode(&ComputerSays::Answer(answer))?).await?;
            }
            _ = check.tick() => {}
            () = shared.changed.notified() => {}
        }
        let now = hello(shared);
        if now != said {
            send.write_all(&encode(&ComputerSays::Status(now.clone()))?).await?;
            said = now;
        }
    }
}

/// Starts what the server asked; the answer when there is one at once (a ping, a refusal).
fn start(
    shared: &Shared,
    desktop: &std::sync::mpsc::Sender<Job>,
    ask: ComputerAsk,
    done: mpsc::UnboundedSender<ComputerAnswer>,
) -> Option<ComputerAnswer> {
    let refuse = |text: &str| Some(ComputerAnswer { id: ask.id, ok: false, text: text.to_owned(), jpeg: None });
    if ask.id == 0 {
        return Some(ComputerAnswer { id: 0, ok: true, text: String::new(), jpeg: None });
    }
    if shared.config().paused {
        return refuse("The owner paused agents on this computer from its menu bar. Ask him before using it.");
    }
    if !lifeos_desktop::OPS.contains(&ask.op.as_str()) {
        return refuse(&format!("This computer doesn't know {:?}; update LifeOS Computer.", ask.op));
    }
    let (tx, rx) = oneshot::channel();
    if desktop.send(Job { op: ask.op, input: ask.input, reply: tx }).is_err() {
        return refuse("The desktop stopped on this computer.");
    }
    let id = ask.id;
    tokio::spawn(async move {
        let answer = match rx.await {
            Ok(Ok(a)) => ComputerAnswer { id, ok: true, text: a.text, jpeg: a.jpeg },
            Ok(Err(why)) => ComputerAnswer { id, ok: false, text: why, jpeg: None },
            Err(_) => ComputerAnswer { id, ok: false, text: "The desktop stopped on this computer.".into(), jpeg: None },
        };
        let _ = done.send(answer);
    });
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn certificate_hashes_read_from_hex() {
        assert_eq!(unhex("00ff10").unwrap(), vec![0, 255, 16]);
        assert!(unhex("abc").is_err());
    }
}
