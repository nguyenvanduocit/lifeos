//! What the agents' Claude Code sessions export as OpenTelemetry (remit's ADR-0085), kept for the owner: the door takes
//! it at `/otlp` with a live run's token (`door.rs`) and keeps it as the card the token names, never as the payload says;
//! the Session panel reads it summed up (`telemetry::summarize`). It is kept in a SQLite file of its own,
//! `<vault>/.lifeos/telemetry.db` (remit's `store::Telemetry`), out of the vault's documents and lifeos's records, which
//! sessions cannot read, for 90 days.

use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError, Weak};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::Result;
use lifeos_wire::CardTelemetry;
use rusqlite::{Connection, params};
use serde_json::{Map, Value};
use telemetry::{Record, SUMMARIZED, summarize};

use crate::Inner;

/// How long a record is kept.
const KEEP: Duration = Duration::from_secs(90 * 24 * 3600);
/// How often what is older than `KEEP` is removed.
const FORGET_EVERY: Duration = Duration::from_secs(24 * 3600);

/// The schema, created if it is not there. A record names the card its token belonged to.
const SCHEMA: &str = "
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;
CREATE TABLE IF NOT EXISTS records (
    id INTEGER PRIMARY KEY,
    at_ns INTEGER NOT NULL,
    card_id TEXT NOT NULL,
    session_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    name TEXT NOT NULL,
    value REAL,
    trace_id TEXT NOT NULL,
    span_id TEXT NOT NULL,
    parent_id TEXT NOT NULL,
    attrs TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS records_card ON records(card_id, kind, name, at_ns);
CREATE INDEX IF NOT EXISTS records_at ON records(at_ns);
";

/// The telemetry database. Its calls run off the async runtime's threads, one at a time.
#[derive(Clone)]
pub(crate) struct Store {
    conn: Arc<Mutex<Connection>>,
}

impl Store {
    /// Opens the database at path, making it if it is not there.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(SCHEMA)?;
        Ok(Self { conn: Arc::new(Mutex::new(conn)) })
    }

    /// Keeps records of card `card_id`, in one transaction.
    pub async fn write(&self, card_id: String, records: Vec<Record>) -> Result<()> {
        self.run(move |conn| {
            let tx = conn.transaction()?;
            {
                let mut put = tx.prepare_cached(
                    "INSERT INTO records(at_ns, card_id, session_id, kind, name, value, trace_id, span_id, parent_id, attrs)
                     VALUES(?,?,?,?,?,?,?,?,?,?)",
                )?;
                for r in records {
                    let session = match r.attrs.get("session.id") {
                        Some(Value::String(s)) => s.clone(),
                        _ => String::new(),
                    };
                    let attrs = Value::Object(r.attrs).to_string();
                    put.execute(params![
                        r.at_ns,
                        card_id,
                        session,
                        r.kind.as_str(),
                        r.name,
                        r.value,
                        r.trace_id,
                        r.span_id,
                        r.parent_id,
                        attrs
                    ])?;
                }
            }
            tx.commit()?;
            Ok(())
        })
        .await
    }

    /// Card `card_id`'s events a summary reads (`SUMMARIZED`), each as its name, when and its attributes.
    pub async fn card_events(&self, card_id: String) -> Result<Vec<(String, i64, Map<String, Value>)>> {
        self.run(move |conn| {
            let mut q = conn.prepare_cached(
                "SELECT name, at_ns, attrs FROM records WHERE card_id = ? AND kind = 'event' AND name IN (?, ?, ?)",
            )?;
            let [a, b, c] = SUMMARIZED;
            let rows = q.query_map(params![card_id, a, b, c], |row| {
                let attrs: String = row.get(2)?;
                let attrs = serde_json::from_str::<Map<String, Value>>(&attrs).unwrap_or_default();
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?, attrs))
            })?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
        .await
    }

    /// Removes the records from before `before_ns`; says how many.
    pub async fn forget_before(&self, before_ns: i64) -> Result<usize> {
        self.run(move |conn| Ok(conn.execute("DELETE FROM records WHERE at_ns < ?", [before_ns])?)).await
    }

    async fn run<T: Send + 'static>(
        &self,
        f: impl FnOnce(&mut Connection) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || f(&mut conn.lock().unwrap_or_else(PoisonError::into_inner))).await?
    }
}

/// Removes what is older than `KEEP`, now and every `FORGET_EVERY`, for as long as lifeos runs.
pub(crate) fn keep_forgetting(inner: Weak<Inner>) {
    tokio::spawn(async move {
        let mut every = tokio::time::interval(FORGET_EVERY);
        loop {
            every.tick().await;
            let Some(store) = inner.upgrade().map(|i| i.telemetry.clone()) else { return };
            let before = SystemTime::now()
                .checked_sub(KEEP)
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .and_then(|d| i64::try_from(d.as_nanos()).ok())
                .unwrap_or_default();
            match store.forget_before(before).await {
                Ok(n) => tracing::info!(removed = n, "telemetry kept 90 days"),
                Err(err) => tracing::error!("telemetry: remove old records: {err:#}"),
            }
        }
    });
}

impl Inner {
    /// Card's telemetry, summed up.
    pub(crate) async fn card_telemetry(&self, card: &str) -> Result<CardTelemetry> {
        Ok(summarize(&self.telemetry.card_events(card.to_owned()).await?))
    }
}

/// Sessions that need no model: one exports its telemetry to the door with its run's token, reports its init and
/// context as Claude Code does, and the owner reads them back.
#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use agent::{BoxFuture, Handler, Launcher, Session, Spec, Update};
    use lifeos_core::Life;
    use lifeos_wire::ASSISTANT;
    use model::{Prompt, StopReason};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    use crate::{Agents, AgentsConfig, Launch, owner};

    /// The telemetry each session was started with.
    #[derive(Default, Clone)]
    struct Fake(Arc<Mutex<Vec<agent::Telemetry>>>);

    struct FakeSession {
        spec: Spec,
        handler: Arc<dyn Handler>,
    }

    impl Launcher for Fake {
        fn start(&self, spec: Spec, handler: Arc<dyn Handler>) -> BoxFuture<'_, Result<Box<dyn Session>, agent::Error>> {
            self.0.lock().unwrap().extend(spec.telemetry.clone());
            Box::pin(async move { Ok(Box::new(FakeSession { spec, handler }) as Box<dyn Session>) })
        }
    }

    const LOGS: &str = r#"{"resourceLogs":[{"resource":{"attributes":[
        {"key":"session.id","value":{"stringValue":"fake"}},{"key":"user.email","value":{"stringValue":"a@b.c"}}]},
      "scopeLogs":[{"scope":{"name":"com.anthropic.claude_code.events"},"logRecords":[
        {"timeUnixNano":"1764000000000000000","body":{"stringValue":"claude_code.api_request"},"attributes":[
          {"key":"model","value":{"stringValue":"claude-fable"}},{"key":"effort","value":{"stringValue":"high"}},
          {"key":"cost_usd","value":{"doubleValue":0.25}},{"key":"duration_ms","value":{"intValue":"1200"}},
          {"key":"ttft_ms","value":{"intValue":"300"}},{"key":"input_tokens","value":{"intValue":"10"}},
          {"key":"output_tokens","value":{"intValue":"20"}},{"key":"cache_read_tokens","value":{"intValue":"30"}}]},
        {"timeUnixNano":"1764000001000000000","body":{"stringValue":"claude_code.tool_result"},"attributes":[
          {"key":"tool_name","value":{"stringValue":"Read"}},{"key":"success","value":{"stringValue":"false"}},
          {"key":"duration_ms","value":{"intValue":"40"}}]},
        {"timeUnixNano":"1764000002000000000","body":{"stringValue":"claude_code.api_error"},"attributes":[
          {"key":"model","value":{"stringValue":"claude-fable"}},{"key":"status_code","value":{"stringValue":"529"}},
          {"key":"error","value":{"stringValue":"Overloaded"}}]}]}]}]}"#;

    /// Posts body to the telemetry door's `signal` with token; its status and response.
    async fn export(t: &agent::Telemetry, token: &str, signal: &str, body: &str) -> (u16, String) {
        let rest = t.endpoint.trim_start_matches("http://");
        let (addr, base) = rest.split_once('/').unwrap();
        let mut s = tokio::net::TcpStream::connect(addr).await.unwrap();
        let req = format!(
            "POST /{base}/v1/{signal} HTTP/1.1\r\nHost: {addr}\r\nAuthorization: Bearer {token}\r\n\
             Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        s.write_all(req.as_bytes()).await.unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).await.unwrap();
        (out.split(' ').nth(1).and_then(|c| c.parse().ok()).unwrap_or(0), out)
    }

    fn counted() -> model::SessionContext {
        model::SessionContext {
            session_id: "fake".into(),
            total: 15_000,
            size: 200_000,
            parts: vec![model::ContextPart { id: "messages".into(), name: "Messages".into(), tokens: 15_000, kind: "used".into() }],
            ..model::SessionContext::default()
        }
    }

    impl Session for FakeSession {
        fn id(&self) -> &str {
            "fake"
        }

        fn prompt(&self, _: Prompt) -> BoxFuture<'_, Result<StopReason, agent::Error>> {
            Box::pin(async move {
                self.handler.on_init(model::SessionInit {
                    session_id: "fake".into(),
                    version: "2.1.0".into(),
                    model: "claude-fable".into(),
                    mcp_servers: vec![model::MCPServerState { name: "lifeos".into(), status: "connected".into(), source: "sdk".into() }],
                    ..model::SessionInit::default()
                });
                let t = self.spec.telemetry.as_ref().unwrap();
                let (status, out) = export(t, &t.token, "logs", LOGS).await;
                assert_eq!(status, 200, "{out}");
                self.handler.on_update(Update::MessageChunk("Xong.".into()));
                self.handler.on_context(model::SessionContext { total: 9_000, ..counted() });
                Ok(StopReason::EndTurn)
            })
        }

        fn steer(&self, _: Prompt) -> BoxFuture<'_, Result<bool, agent::Error>> {
            Box::pin(async { Ok(false) })
        }

        fn cancel(&self) -> BoxFuture<'_, Result<(), agent::Error>> {
            Box::pin(async { Ok(()) })
        }

        fn close(&self) {}

        fn context_lost(&self) -> bool {
            false
        }

        fn move_to(&self, _: String) -> BoxFuture<'_, Result<(), agent::Error>> {
            Box::pin(async { Ok(()) })
        }

        fn context(&self) -> BoxFuture<'_, Result<model::SessionContext, agent::Error>> {
            Box::pin(async { Ok(counted()) })
        }
    }

    #[tokio::test]
    async fn a_session_s_telemetry_init_and_context_are_kept_and_read_back() {
        let dir = tempfile::tempdir().unwrap();
        let life = Life::open_with_memory_db(dir.path()).unwrap();
        let fake = Fake::default();
        let config = AgentsConfig { launcher: Some(Launch(Arc::new(fake.clone()))), ..Default::default() };
        let agents = Agents::start(Arc::new(Mutex::new(life)), config).await.unwrap();
        assert_eq!(agents.telemetry(ASSISTANT).await.unwrap(), lifeos_wire::CardTelemetry::default());

        let answer = agents.ask(ASSISTANT, "Chào", owner()).await.unwrap();
        assert_eq!((answer.text.as_str(), answer.error), ("Xong.", None));
        let t = fake.0.lock().unwrap()[0].clone();
        assert!(t.endpoint.ends_with("/otlp"), "{}", t.endpoint);

        let summary = agents.telemetry(ASSISTANT).await.unwrap();
        assert_eq!((summary.requests, summary.errors), (1, 1));
        assert_eq!((summary.input_tokens, summary.output_tokens, summary.cache_read_tokens), (10, 20, 30));
        assert!((summary.cost_usd - 0.25).abs() < 1e-9);
        assert_eq!((summary.latency_p50_ms, summary.ttft_p50_ms), (1200, 300));
        assert_eq!(summary.models[0].effort, "high");
        assert_eq!((summary.tools[0].name.as_str(), summary.tools[0].runs, summary.tools[0].failed), ("Read", 1, 1));
        assert_eq!(summary.recent_errors[0].status, "529");
        assert_eq!((summary.first_at_ms, summary.last_at_ms), (1_764_000_000_000, 1_764_000_002_000));
        assert_eq!(agents.telemetry("librarian").await.unwrap().requests, 0, "kept as the token's card only");

        let ctx = agents.context(ASSISTANT).await.unwrap();
        let given = ctx.given.expect("the launch was recorded");
        assert!(given.opening.as_deref().is_some_and(|o| o.ends_with("Chào")), "{:?}", given.opening);
        assert_eq!(given.mcp_servers, ["lifeos", "desktop"], "Google is not signed in; the desktop is always offered");
        assert!(given.tools.iter().any(|t| t.name == "dashboard"));
        assert_eq!(given.setting_sources, ["project"]);
        assert_eq!(ctx.loaded.expect("the init was kept").init.version, "2.1.0");
        // The live session counted afresh.
        assert_eq!((ctx.context.unwrap().context.total, ctx.context_error.as_str()), (15_000, ""));

        // A token no live run holds is refused; so is a run's once its session closed.
        assert_eq!(export(&t, "stale", "logs", LOGS).await.0, 401);
        agents.restart(ASSISTANT).await.unwrap();
        assert_eq!(export(&t, &t.token, "traces", r#"{"resourceSpans":[]}"#).await.0, 401);
        let ctx = agents.context(ASSISTANT).await.unwrap();
        assert_eq!((ctx.given, ctx.loaded, ctx.context), (None, None, None), "of a session no longer the card's");
        assert_eq!(agents.telemetry(ASSISTANT).await.unwrap().requests, 1, "kept across sessions");
    }

    #[tokio::test]
    async fn the_stored_count_stands_in_when_no_session_runs() {
        let dir = tempfile::tempdir().unwrap();
        let life = Life::open_with_memory_db(dir.path()).unwrap();
        let fake = Fake::default();
        let config = AgentsConfig { launcher: Some(Launch(Arc::new(fake))), ..Default::default() };
        let agents = Agents::start(Arc::new(Mutex::new(life)), config).await.unwrap();
        agents.ask(ASSISTANT, "Chào", owner()).await.unwrap();
        agents.close();
        let ctx = agents.context(ASSISTANT).await.unwrap();
        assert_eq!((ctx.context.unwrap().context.total, ctx.context_error.as_str()), (9_000, ""));
    }

    #[tokio::test]
    async fn records_older_than_kept_are_forgotten() {
        let dir = tempfile::tempdir().unwrap();
        let store = super::Store::open(&dir.path().join("t.db")).unwrap();
        let record = |at_ns| telemetry::Record {
            at_ns,
            kind: telemetry::Kind::Event,
            name: telemetry::TOOL_RESULT.into(),
            value: None,
            trace_id: String::new(),
            span_id: String::new(),
            parent_id: String::new(),
            attrs: serde_json::Map::new(),
        };
        store.write("a".into(), vec![record(1), record(5)]).await.unwrap();
        assert_eq!(store.forget_before(3).await.unwrap(), 1);
        assert_eq!(store.card_events("a".into()).await.unwrap().len(), 1);
    }
}
