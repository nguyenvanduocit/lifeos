//! The door every agent's session reaches lifeos's tools through (remit's `agentmcp`): MCP over streamable HTTP on
//! loopback, at `/mcp`. A request comes in only with the bearer token of a live run, which names its card, and is served
//! that card's tools (`Toolbox`). Stateless: each request is checked and served by its own token's server.
//!
//! Beside it, at `/otlp` (remit's `otlp.rs`, ADR-0085), sessions export their OpenTelemetry as OTLP/HTTP JSON
//! (`/otlp/v1/logs`, `/otlp/v1/metrics`, `/otlp/v1/traces`) with the same token: what comes in is kept as the card the
//! token names, never the card the payload names.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex, PoisonError};

use anyhow::Result;
use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::http::header::{AUTHORIZATION, CONTENT_TYPE};
use axum::response::{IntoResponse, Response};
use axum::routing::{any, post};
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, Implementation, ListToolsResult,
    PaginatedRequestParams, ServerCapabilities, ServerConfig,
};
use rmcp::service::{RequestContext, RoleServer};
use rmcp::transport::streamable_http_server::session::never::NeverSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use rmcp::{ErrorData, ServerHandler};
use telemetry::{Record, otlp};
use tower_service::Service as _;

use crate::otel::Store;
use crate::toolbox::Toolbox;

/// Where the door is mounted.
pub const PATTERN: &str = "/mcp";

/// The name sessions know lifeos's server by: its tools are `mcp__lifeos__<tool>`.
pub const SERVER_NAME: &str = "lifeos";

/// Where the Google app's server is mounted, beside lifeos's; its tools are `mcp__google__<tool>`.
pub const GOOGLE_PATTERN: &str = "/google";

/// Where the desktop tools' server is mounted; its tools are `mcp__desktop__<tool>`.
pub const DESKTOP_PATTERN: &str = "/desktop";

/// Where sessions export their telemetry: OTLP's `/v1/logs`, `/v1/metrics` and `/v1/traces` are under it.
pub const OTLP_PATTERN: &str = "/otlp";

/// The most one telemetry export may carry.
const OTLP_BODY_LIMIT: usize = 8_388_608;

/// The servers behind the door.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Server {
    Lifeos,
    Google,
    Desktop,
}

impl Server {
    fn name(self) -> &'static str {
        match self {
            Server::Lifeos => SERVER_NAME,
            Server::Google => lifeos_core::life::google::CONNECTION,
            Server::Desktop => lifeos_core::life::desktop::CONNECTION,
        }
    }
}

/// What a token opens: the toolbox of the card whose run holds it, on one of the door's servers.
pub trait Opens: Send + Sync + 'static {
    fn toolbox(&self, token: &str, server: Server) -> Option<Toolbox>;
    /// The card a live run's token names and where its telemetry is kept; none for a token no live run holds.
    fn telemetry(&self, token: &str) -> Option<(String, Store)>;
}

/// The run tokens alive now, each naming its card.
#[derive(Default)]
pub struct Tokens(Mutex<HashMap<String, String>>);

impl Tokens {
    /// A new token for card's run.
    pub fn mint(&self, card: &str) -> String {
        let mut bytes = [0_u8; 32];
        getrandom::fill(&mut bytes).unwrap_or_else(|_| bytes.fill(0));
        let token: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        self.lock().insert(token.clone(), card.to_owned());
        token
    }

    pub fn card_of(&self, token: &str) -> Option<String> {
        self.lock().get(token).cloned()
    }

    pub fn revoke(&self, token: &str) {
        self.lock().remove(token);
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, String>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

struct Door {
    opens: Arc<dyn Opens>,
    sessions: Arc<NeverSessionManager>,
    config: StreamableHttpServerConfig,
}

/// Starts the door on a loopback port of its own; the URL sessions are given.
pub async fn open(opens: Arc<dyn Opens>) -> Result<String> {
    let door = Door {
        opens,
        sessions: Arc::new(NeverSessionManager::default()),
        config: StreamableHttpServerConfig::default().with_legacy_session_mode(false).with_json_response(true),
    };
    let router = Router::new()
        .route(PATTERN, any(serve_lifeos))
        .route(GOOGLE_PATTERN, any(serve_google))
        .route(DESKTOP_PATTERN, any(serve_desktop))
        .route(&format!("{OTLP_PATTERN}/v1/logs"), post(export_logs))
        .route(&format!("{OTLP_PATTERN}/v1/metrics"), post(export_metrics))
        .route(&format!("{OTLP_PATTERN}/v1/traces"), post(export_traces))
        .with_state(Arc::new(door));
    let listener = tokio::net::TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0))).await?;
    let addr = listener.local_addr()?;
    tokio::spawn(async move {
        if let Err(err) = axum::serve(listener, router).await {
            tracing::error!("the agents' MCP door closed: {err}");
        }
    });
    Ok(format!("http://{addr}{PATTERN}"))
}

async fn serve_lifeos(state: State<Arc<Door>>, req: Request) -> Response {
    serve(state, req, Server::Lifeos).await
}

async fn serve_google(state: State<Arc<Door>>, req: Request) -> Response {
    serve(state, req, Server::Google).await
}

async fn serve_desktop(state: State<Arc<Door>>, req: Request) -> Response {
    serve(state, req, Server::Desktop).await
}

async fn serve(State(door): State<Arc<Door>>, req: Request, server: Server) -> Response {
    let Some(toolbox) = bearer(&req).and_then(|t| door.opens.toolbox(t, server)) else {
        return (StatusCode::UNAUTHORIZED, "no live run holds this token, or it may not use this server\n").into_response();
    };
    let one = Mutex::new(Some(CardServer(Arc::new(toolbox), server)));
    let mut mcp = StreamableHttpService::new(move || take(&one), Arc::clone(&door.sessions), door.config.clone());
    match mcp.call(req).await {
        Ok(res) => res.map(Body::new),
        Err(never) => match never {},
    }
}

type Decode = fn(&[u8]) -> Result<Vec<Record>, otlp::DecodeError>;

async fn export_logs(state: State<Arc<Door>>, req: Request) -> Response {
    export(state, req, otlp::logs).await
}

async fn export_metrics(state: State<Arc<Door>>, req: Request) -> Response {
    export(state, req, otlp::metrics).await
}

async fn export_traces(state: State<Arc<Door>>, req: Request) -> Response {
    export(state, req, otlp::traces).await
}

/// Keeps one OTLP export as the card the request's token names.
async fn export(State(door): State<Arc<Door>>, req: Request, decode: Decode) -> Response {
    let Some((card, store)) = bearer(&req).and_then(|t| door.opens.telemetry(t)) else {
        return (StatusCode::UNAUTHORIZED, "no live run holds this token\n").into_response();
    };
    let body: Bytes = match axum::body::to_bytes(req.into_body(), OTLP_BODY_LIMIT).await {
        Ok(b) => b,
        Err(err) => return (StatusCode::PAYLOAD_TOO_LARGE, format!("{err}\n")).into_response(),
    };
    let records = match decode(&body) {
        Ok(r) => r,
        Err(err) => return (StatusCode::BAD_REQUEST, format!("{err}\n")).into_response(),
    };
    if let Err(err) = store.write(card.clone(), records).await {
        tracing::error!("telemetry of {card}: {err:#}");
        return (StatusCode::SERVICE_UNAVAILABLE, "the telemetry could not be kept\n").into_response();
    }
    // An empty ExportServiceResponse: everything was taken.
    ([(CONTENT_TYPE, "application/json")], "{}").into_response()
}

fn take(one: &Mutex<Option<CardServer>>) -> Result<CardServer, std::io::Error> {
    one.lock()
        .unwrap_or_else(PoisonError::into_inner)
        .take()
        .ok_or_else(|| std::io::Error::other("a request is served by one server"))
}

fn bearer(req: &Request) -> Option<&str> {
    let value = req.headers().get(AUTHORIZATION)?.to_str().ok()?;
    value.strip_prefix("Bearer ").filter(|t| !t.is_empty())
}

/// One card's MCP server: the tools its toolbox holds.
struct CardServer(Arc<Toolbox>, Server);

impl ServerHandler for CardServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(self.1.name(), env!("CARGO_PKG_VERSION")))
    }

    async fn list_tools(
        &self,
        _: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        Ok(ListToolsResult::with_all_items(self.0.listed()))
    }

    async fn call_tool(
        &self,
        req: CallToolRequestParams,
        _: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let answer = self.0.call(req.name.as_ref(), req.arguments).await?;
        Ok(match answer {
            Ok(out) => CallToolResult::success(
                std::iter::once(ContentBlock::text(out.text))
                    .chain(out.images.into_iter().map(|(media, data)| ContentBlock::image(data, media)))
                    .collect(),
            ),
            Err(err) => CallToolResult::error(vec![ContentBlock::text(format!("{err:#}"))]),
        }
        .into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    struct One;

    impl Opens for One {
        fn toolbox(&self, token: &str, server: Server) -> Option<Toolbox> {
            (token == "good" && server == Server::Lifeos).then(|| {
                let mut t = Toolbox::default();
                t.add("echo", "Says it back", serde_json::Map::new(), true, |args| {
                    Box::pin(async move { Ok(serde_json::Value::Object(args).to_string()) })
                });
                t
            })
        }

        fn telemetry(&self, _: &str) -> Option<(String, Store)> {
            None
        }
    }

    async fn post(url: &str, token: &str, body: &str) -> (u16, String) {
        post_to(url, PATTERN, token, body).await
    }

    async fn post_to(url: &str, path: &str, token: &str, body: &str) -> (u16, String) {
        let addr = url.trim_start_matches("http://").trim_end_matches(PATTERN);
        let mut s = tokio::net::TcpStream::connect(addr).await.unwrap();
        let req = format!(
            "POST {path} HTTP/1.1\r\nHost: {addr}\r\nAuthorization: Bearer {token}\r\nContent-Type: application/json\r\n\
             Accept: application/json, text/event-stream\r\nMCP-Protocol-Version: 2025-06-18\r\nContent-Length: {}\r\n\
             Connection: close\r\n\r\n{body}",
            body.len()
        );
        s.write_all(req.as_bytes()).await.unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).await.unwrap();
        let status = out.split(' ').nth(1).and_then(|c| c.parse().ok()).unwrap_or(0);
        (status, out)
    }

    #[tokio::test]
    async fn a_live_token_is_served_its_cards_tools_and_no_other_is() {
        let url = open(Arc::new(One)).await.unwrap();
        let list = r#"{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}"#;
        let (status, _) = post(&url, "stale", list).await;
        assert_eq!(status, 401);
        let (status, body) = post(&url, "good", list).await;
        assert_eq!(status, 200, "{body}");
        assert!(body.contains(r#""name":"echo""#), "{body}");
        let call = r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"echo","arguments":{"x":1}}}"#;
        let (status, body) = post(&url, "good", call).await;
        assert_eq!(status, 200, "{body}");
        assert!(body.contains(r#"{\"x\":1}"#), "{body}");
        let (status, _) = post_to(&url, GOOGLE_PATTERN, "good", list).await;
        assert_eq!(status, 401, "a card not allowed Google is not served its server");
    }
}
