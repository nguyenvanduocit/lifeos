//! An applied extension as a `Plugin`: every call reaches the plugin's process over `link`, starting it first when none runs.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use async_trait::async_trait;
use link::{ErrorCode, Failure, Outgoing, Request};
use plugin_sdk::{Asset, Forge, Plugin, Senses};
use serde::de::DeserializeOwned;
use serde_json::Value;
use tokio::sync::{Mutex, mpsc};
use wire::plugin::{
    Call, Firing, Manifest, PollAnswer, PollAsk, ProcessAnswer, SenseOpen, Vars, client,
};

pub use crate::start::Stopped;
use crate::start::{Process, sensors, start};

/// The program of one plugin and the hash it must have: a file with another hash does not start.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Spec {
    pub id: String,
    pub binary: PathBuf,
    /// What the program is started with; none for a plugin that is a program of its own.
    pub args: Vec<String>,
    pub sha256: String,
}

/// One applied extension. Its process starts on the first call and again on the first call after it died; a call that found
/// it dead fails `DISCONNECTED` and is not repeated, as a tool that ran half way may have acted. Dropping it kills the process.
pub struct RemotePlugin {
    spec: Spec,
    forge: Option<Arc<dyn Forge>>,
    retired: AtomicBool,
    running: Mutex<Option<Arc<Process>>>,
    next_sensor: AtomicU64,
}

impl RemotePlugin {
    pub fn new(spec: Spec) -> RemotePlugin {
        RemotePlugin {
            spec,
            forge: None,
            retired: AtomicBool::new(false),
            running: Mutex::new(None),
            next_sensor: AtomicU64::new(0),
        }
    }

    /// Ends the process, if it runs: a plugin removed or replaced is not left running.
    pub async fn stop(&self) -> Option<Stopped> {
        let process = self.running.lock().await.take()?;
        Some(process.shutdown().await)
    }

    /// Disable this instance permanently, including callers holding an old reference.
    pub async fn retire(&self) {
        self.retired.store(true, Ordering::Release);
        self.stop().await;
    }

    /// The running process, started when none runs or the one that ran has ended.
    async fn process(&self) -> Result<Arc<Process>, Failure> {
        let mut running = self.running.lock().await;
        if self.retired.load(Ordering::Acquire) {
            return Err(Failure::new(ErrorCode::Unavailable, "Plugin is disabled"));
        }
        if let Some(p) = running.as_ref().filter(|p| !p.exited()) {
            return Ok(Arc::clone(p));
        }
        if let Some(old) = running.take() {
            old.kill().await;
        }
        let p = start(&self.spec, self.forge.clone())
            .await
            .map(Arc::new)
            .map_err(|reason| {
                Failure::new(
                    ErrorCode::Unavailable,
                    format!("plugin {}: {reason}", self.spec.id),
                )
            })?;
        *running = Some(Arc::clone(&p));
        Ok(p)
    }

    async fn ask<A: DeserializeOwned + Send + 'static>(
        &self,
        out: Outgoing<Call, A>,
    ) -> Result<A, Failure> {
        let p = self.process().await?;
        match p.peer.call(out).await {
            Ok(answer) => Ok(answer),
            Err(failure) => Err(self.fail(&p, failure).await),
        }
    }

    /// How a call to `p` that answered `failure` failed. A session that ended is a process that went: it is dropped, unless
    /// it was replaced already, so the next call starts a fresh one. Any other failure is the plugin's own answer.
    async fn fail(&self, p: &Arc<Process>, failure: Failure) -> Failure {
        if failure.code != ErrorCode::Disconnected && !p.exited() {
            return failure;
        }
        let mut running = self.running.lock().await;
        if running.as_ref().is_some_and(|q| Arc::ptr_eq(q, p)) {
            running.take();
            p.kill().await;
        }
        Failure::new(
            ErrorCode::Disconnected,
            format!(
                "plugin {} could not be reached: {}",
                self.spec.id, failure.message
            ),
        )
    }
}

#[async_trait]
impl Plugin for RemotePlugin {
    /// The grant reaches the plugin in the `welcome` of its next process.
    fn grant_forge(&mut self, forge: Arc<dyn Forge>) {
        self.forge = Some(forge);
    }

    async fn describe(&self) -> Result<Manifest, Failure> {
        let document: model::ForeignJson = self.ask(client::describe()).await?;
        let manifest: Manifest = serde_json::from_value(document.0)
            .map_err(|error| Failure::new(ErrorCode::InvalidArgument,format!("Invalid manifest: {error}")))?;
        manifest.check().map_err(|msg| Failure::new(ErrorCode::InvalidArgument, msg))?;
        Ok(manifest)
    }

    async fn call(
        &self,
        request: Request,
        tool: String,
        arguments: Value,
        vars: Vars,
    ) -> Result<String, Failure> {
        self.ask(client::call(request, tool, arguments.into(), vars))
            .await
    }

    async fn poll(&self, ask: PollAsk) -> Result<PollAnswer, Failure> {
        self.ask(client::poll(ask)).await
    }

    async fn sense(&self, open: SenseOpen) -> Result<Senses, Failure> {
        let p = self.process().await?;
        let sensor = self.next_sensor.fetch_add(1, Ordering::Relaxed).to_string();
        let (tx, rx) = mpsc::unbounded_channel();
        let registered = sensors(&p.sensors)
            .as_mut()
            .map(|open| open.insert(sensor.clone(), tx))
            .is_some();
        if !registered {
            let failure = Failure::new(ErrorCode::Disconnected, "the session ended");
            return Err(self.fail(&p, failure).await);
        }
        let handshake = open_sensor(p.peer.clone(), Arc::clone(&p.sensors), sensor, rx, open);
        match handshake
            .await
            .map_err(|e| Failure::new(ErrorCode::Aborted, e.to_string()))?
        {
            Ok(stream) => Ok(stream),
            Err(failure) => Err(self.fail(&p, failure).await),
        }
    }

    async fn process(&self, firing: Firing) -> Result<ProcessAnswer, Failure> {
        self.ask(client::process(firing)).await
    }

    async fn asset(&self, name: String) -> Result<Asset, Failure> {
        Asset::from_wire(self.ask(client::asset(name)).await?)
    }

    async fn account(&self, vars: Vars) -> Result<Vars, Failure> {
        self.ask(client::account(vars)).await
    }
}

// Dropping the waiter detaches this handshake. Once open is acknowledged,
// its unclaimed stream is dropped and sends the matching close.
fn open_sensor(
    peer: plugin_sdk::pipe::Peer,
    registry: crate::start::Sensors,
    sensor: String,
    rx: mpsc::UnboundedReceiver<Result<plugin_sdk::Sensed, Failure>>,
    open: SenseOpen,
) -> tokio::task::JoinHandle<Result<Senses, Failure>> {
    tokio::spawn(async move {
        let close_id = sensor.clone();
        let close_peer = peer.clone();
        let stream = Senses::new(rx, move || {
            if let Some(open) = sensors(&registry).as_mut() {
                open.remove(&close_id);
            }
            close_peer.call_detached(client::sense_close(close_id));
        });
        peer.call(client::sense_open(sensor, open)).await?;
        Ok(stream)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use link::{Config, Conn, Hello, Node, Payload};
    use plugin_sdk::pipe::{Note, Peer};
    use tokio::sync::Notify;

    #[tokio::test]
    async fn cancelled_open_closes_after_ack() {
        tokio::time::timeout(std::time::Duration::from_secs(3), async {
            let (host, idle) = Peer::new();
            let (child, remote) = Peer::new();
            let (a, b) = tokio::io::duplex(4096);
            let (hr, hw) = tokio::io::split(a);
            let (cr, cw) = tokio::io::split(b);
            let node = Node {
                id: "test".into(),
                public_key: String::new(),
            };
            let mut host_notes = idle.run(
                Conn::accept(Config::default(), node.clone()),
                hr,
                hw,
                Arc::new(|_, _, _| Box::pin(async { unreachable!() })),
            );
            let opened = Arc::new(Notify::new());
            let ack = Arc::new(Notify::new());
            let closed = Arc::new(Notify::new());
            let hello = Hello {
                node,
                credential: String::new(),
                capabilities: vec![],
                offers: vec!["plugin".into()],
                bundle: None,
            };
            let mut child_notes = remote.run(Conn::dial(Config::default(), hello), cr, cw, {
                let (opened, ack, closed) = (opened.clone(), ack.clone(), closed.clone());
                Arc::new(move |_, method, _| {
                    let (opened, ack, closed) = (opened.clone(), ack.clone(), closed.clone());
                    Box::pin(async move {
                        match method.as_str() {
                            "sense_open" => {
                                opened.notify_one();
                                ack.notified().await;
                            }
                            "sense_close" => closed.notify_one(),
                            _ => unreachable!(),
                        }
                        Payload::encode(&wire::Ack {})
                    })
                })
            });
            assert!(matches!(host_notes.recv().await, Some(Note::Hello(_))));
            host.admit(vec![]);
            assert!(matches!(child_notes.recv().await, Some(Note::Welcomed(_))));
            let (tx, rx) = mpsc::unbounded_channel();
            let registry = Arc::new(std::sync::Mutex::new(Some(
                std::collections::HashMap::from([("s".into(), tx)]),
            )));
            let open = SenseOpen {
                kind: "probe".into(),
                name: "p".into(),
                settings: serde_json::json!({}).into(),
                vars: Vars::new(),
                cursor: String::new(),
                last_at_ms: 0,
            };
            let waiter = open_sensor(host.clone(), registry.clone(), "s".into(), rx, open);
            opened.notified().await;
            drop(waiter);
            ack.notify_one();
            closed.notified().await;
            assert!(sensors(&registry).as_ref().unwrap().is_empty());
            child.close();
        })
        .await
        .unwrap();
    }
}
