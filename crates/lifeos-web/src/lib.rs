//! The web app's server: the Vue app (embedded from `web/dist`) and
//! `/link.json` over HTTP, and the link — WebTransport over QUIC carrying
//! `lifeos_wire` frames — on its own UDP port.
//!
//! There is no login: keep both ports on loopback (the default) or on a
//! private network such as a tailnet, never on the open internet.

mod brief;
mod cert;
mod dist;
mod link;

use std::net::SocketAddr;
use std::sync::{Arc, RwLock};

use anyhow::{Context, Result};
use axum::Router;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::routing::get;
use axum::Json;
use lifeos_agent::{AgentConfig, SharedLife};
use lifeos_agent::OnChange;
use lifeos_wire::{LinkInfo, PATH};
use std::collections::HashMap;
use tokio::sync::watch;

#[derive(Debug, Clone, Copy)]
pub struct WebConfig {
    /// HTTP (TCP): the app and `/link.json`.
    pub http: SocketAddr,
    /// WebTransport (UDP).
    pub link: SocketAddr,
    /// Have the agent write the morning brief at this local time each day.
    pub brief_at: Option<chrono::NaiveTime>,
}

pub(crate) struct AppState {
    pub life: SharedLife,
    pub agent_config: AgentConfig,
    /// One seat per agent the page talks to: the Assistant and each crew
    /// member. A seat's agent starts on its first message, so the dashboard
    /// works without Claude Code.
    pub seats: std::sync::Mutex<HashMap<String, Arc<link::Seat>>>,
    /// Grows on every change; open pages watch it to read again.
    pub changes: watch::Sender<u64>,
}

impl AppState {
    pub fn bump(&self) {
        self.changes.send_modify(|n| *n += 1);
    }

    pub fn seat(&self, agent: &str) -> Arc<link::Seat> {
        self.seats.lock().expect("seats lock").entry(agent.to_string()).or_default().clone()
    }
}

struct HttpState {
    link: SocketAddr,
    certificate: Arc<RwLock<cert::Published>>,
}

/// A running server: where it listens and the hash a client pins.
pub struct Running {
    pub http: SocketAddr,
    pub link: SocketAddr,
    /// SHA-256 of the link's certificate, hex.
    pub certificate_hash: String,
    endpoint: web_transport_quinn::quinn::Endpoint,
    certificate: Arc<RwLock<cert::Published>>,
    http_task: tokio::task::JoinHandle<std::io::Result<()>>,
}

impl Running {
    /// Puts a fresh certificate on the link now (the server also does this
    /// on its own every few days); returns the new hash.
    pub fn rotate_certificate(&self) -> Result<String> {
        cert::rotate_once(&self.endpoint, &self.certificate)?;
        Ok(self.certificate.read().expect("certificate lock").hash.clone())
    }

    /// Serves until the HTTP server stops.
    pub async fn wait(self) -> Result<()> {
        self.http_task.await??;
        Ok(())
    }
}

pub async fn serve(life: SharedLife, agent_config: AgentConfig, config: WebConfig) -> Result<()> {
    start(life, agent_config, config).await?.wait().await
}

/// Binds both ports (port 0 picks a free one) and serves in the background.
pub async fn start(life: SharedLife, mut agent_config: AgentConfig, config: WebConfig) -> Result<Running> {
    web_transport_quinn::quinn::rustls::crypto::ring::default_provider()
        .install_default()
        .ok();
    let cert = cert::Certificate::new(time::OffsetDateTime::now_utc())?;
    let published = Arc::new(RwLock::new(cert.published()?));
    let (changes, _) = watch::channel(0);
    let on_change = {
        let changes = changes.clone();
        OnChange::new(move || changes.send_modify(|n| *n += 1))
    };
    agent_config.on_change = on_change;
    let state = Arc::new(AppState {
        life,
        agent_config,
        seats: Default::default(),
        changes,
    });
    tokio::spawn(notice_outside_changes(state.clone()));
    if let Some(at) = config.brief_at {
        tokio::spawn(brief::daily(state.clone(), at));
    }

    let endpoint = web_transport_quinn::quinn::Endpoint::server(cert.quic_config()?, config.link)
        .with_context(|| format!("cannot listen for the link on udp {}", config.link))?;
    let link = endpoint.local_addr()?;
    tokio::spawn(cert::rotate(endpoint.clone(), published.clone()));
    tokio::spawn(link::accept(web_transport_quinn::Server::new(endpoint.clone()), state.clone()));

    let listener = tokio::net::TcpListener::bind(config.http)
        .await
        .with_context(|| format!("cannot listen on {}", config.http))?;
    let http = listener.local_addr()?;
    let app = Router::new()
        .route("/link.json", get(link_json))
        .fallback(dist::serve)
        .with_state(Arc::new(HttpState {
            link,
            certificate: published.clone(),
        }));
    let http_task = tokio::spawn(async move { axum::serve(listener, app).await });
    tracing::info!("lifeos web on http://{http} (link on udp {link})");
    let certificate_hash = published.read().expect("certificate lock").hash.clone();
    Ok(Running {
        http,
        link,
        certificate_hash,
        endpoint,
        certificate: published,
        http_task,
    })
}

/// How often to look for changes made outside this server: files edited by
/// hand, or the database written by `lifeos mcp`, the Telegram bot, the CLI.
const OUTSIDE_CHANGES_EVERY: std::time::Duration = std::time::Duration::from_secs(3);

async fn notice_outside_changes(state: Arc<AppState>) {
    let mut last_version = None;
    loop {
        tokio::time::sleep(OUTSIDE_CHANGES_EVERY).await;
        let changed = {
            let mut life = state.life.lock().expect("life lock");
            let files = match life.sync_index() {
                Ok(stats) => stats.indexed + stats.removed > 0,
                Err(err) => {
                    tracing::warn!("index sync failed: {err:#}");
                    false
                }
            };
            let version = life.data_version().ok();
            let db = last_version.is_some() && version != last_version;
            last_version = version;
            files || db
        };
        if changed {
            state.bump();
        }
    }
}

/// Where the page finds the link.
async fn link_json(State(state): State<Arc<HttpState>>, headers: HeaderMap) -> Json<LinkInfo> {
    let host = headers.get(axum::http::header::HOST).and_then(|h| h.to_str().ok());
    let cert = state.certificate.read().expect("certificate lock").clone();
    Json(LinkInfo {
        url: link_url(state.link, host),
        certificate_hashes: vec![cert.hash],
        certificate_expires: cert.expires,
    })
}

/// The address the link really listens on. A name like `localhost` may
/// resolve to the other IP family than the one bound, and QUIC then just
/// times out; the Host header is used only when the link listens on every
/// address.
fn link_url(link: SocketAddr, host_header: Option<&str>) -> String {
    let host = if link.ip().is_unspecified() {
        host_header.map(strip_port).unwrap_or("localhost").to_string()
    } else if link.is_ipv6() {
        format!("[{}]", link.ip())
    } else {
        link.ip().to_string()
    };
    format!("https://{host}:{}{PATH}", link.port())
}

/// `localhost:7420` -> `localhost`, `[::1]:7420` -> `[::1]`.
fn strip_port(host: &str) -> &str {
    match host.rfind(':') {
        Some(i) if !host[i..].contains(']') => &host[..i],
        _ => host,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn link_url_uses_the_bound_ip() {
        let v4 = "127.0.0.1:7421".parse().unwrap();
        assert_eq!(super::link_url(v4, Some("localhost:7420")), "https://127.0.0.1:7421/link");
        let v6 = "[::1]:7421".parse().unwrap();
        assert_eq!(super::link_url(v6, None), "https://[::1]:7421/link");
        let any = "0.0.0.0:7421".parse().unwrap();
        assert_eq!(super::link_url(any, Some("box.local:7420")), "https://box.local:7421/link");
    }

    #[test]
    fn strip_port_keeps_ipv6_brackets() {
        assert_eq!(super::strip_port("localhost:7420"), "localhost");
        assert_eq!(super::strip_port("[::1]:7420"), "[::1]");
        assert_eq!(super::strip_port("[::1]"), "[::1]");
        assert_eq!(super::strip_port("127.0.0.1"), "127.0.0.1");
    }
}
