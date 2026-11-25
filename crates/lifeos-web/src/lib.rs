//! The web app's server: the Vue app (embedded from `web/dist`) and
//! `/link.json` over HTTP, and the link — WebTransport over QUIC carrying
//! `lifeos_wire` frames — on its own UDP port.
//!
//! There is no login: keep both ports on loopback (the default).

mod cert;
mod dist;
mod link;

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::{Context, Result};
use axum::Router;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::routing::get;
use axum::Json;
use lifeos_agent::{AgentConfig, LifeAgent, SharedLife};
use lifeos_wire::{LinkInfo, PATH};
use tokio::sync::Mutex;

#[derive(Debug, Clone, Copy)]
pub struct WebConfig {
    /// HTTP (TCP): the app and `/link.json`.
    pub http: SocketAddr,
    /// WebTransport (UDP).
    pub link: SocketAddr,
}

pub(crate) struct AppState {
    pub life: SharedLife,
    pub agent_config: AgentConfig,
    /// One conversation for the web app, started on the first message so the
    /// dashboard works without Claude Code.
    pub agent: Mutex<Option<LifeAgent>>,
}

struct HttpState {
    link: SocketAddr,
    certificate_hash: String,
    certificate_expires: String,
}

/// A running server: where it listens and the hash a client pins.
pub struct Running {
    pub http: SocketAddr,
    pub link: SocketAddr,
    /// SHA-256 of the link's certificate, hex.
    pub certificate_hash: String,
    http_task: tokio::task::JoinHandle<std::io::Result<()>>,
}

impl Running {
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
pub async fn start(life: SharedLife, agent_config: AgentConfig, config: WebConfig) -> Result<Running> {
    web_transport_quinn::quinn::rustls::crypto::ring::default_provider()
        .install_default()
        .ok();
    let cert = cert::Certificate::new(time::OffsetDateTime::now_utc())?;
    let certificate_hash = hex(&cert.hash);
    let certificate_expires = cert
        .not_after
        .format(&time::format_description::well_known::Rfc3339)?;
    let state = Arc::new(AppState {
        life,
        agent_config,
        agent: Mutex::new(None),
    });

    let server = web_transport_quinn::ServerBuilder::new()
        .with_addr(config.link)
        .with_certificate(cert.chain, cert.key)
        .context("cannot start the WebTransport listener")?;
    let link = server.local_addr()?;
    tokio::spawn(link::accept(server, state));

    let listener = tokio::net::TcpListener::bind(config.http)
        .await
        .with_context(|| format!("cannot listen on {}", config.http))?;
    let http = listener.local_addr()?;
    let app = Router::new()
        .route("/link.json", get(link_json))
        .fallback(dist::serve)
        .with_state(Arc::new(HttpState {
            link,
            certificate_hash: certificate_hash.clone(),
            certificate_expires,
        }));
    let http_task = tokio::spawn(async move { axum::serve(listener, app).await });
    tracing::info!("lifeos web on http://{http} (link on udp {link})");
    Ok(Running { http, link, certificate_hash, http_task })
}

/// Where the page finds the link.
async fn link_json(State(state): State<Arc<HttpState>>, headers: HeaderMap) -> Json<LinkInfo> {
    let host = headers.get(axum::http::header::HOST).and_then(|h| h.to_str().ok());
    Json(LinkInfo {
        url: link_url(state.link, host),
        certificate_hashes: vec![state.certificate_hash.clone()],
        certificate_expires: state.certificate_expires.clone(),
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

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
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
