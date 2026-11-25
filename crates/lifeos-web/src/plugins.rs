//! LifeOS bindings for Remit's plugin runner. The owner's local config is the
//! authority for executable pins and the functions and hooks exposed by LifeOS.
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
    time::Duration,
};

use axum::{
    Json,
    body::Bytes,
    extract::{FromRequest, Path as Route, Request, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use plugin::{RemotePlugin, Spec};
use plugin_sdk::{Firing, Manifest, Plugin, ProcessAnswer, Slot, Vars};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::sync::{Mutex, OnceCell, watch};

use crate::HttpState;
use lifeos_core::plugins::{Hook, Install, configs, identifier};

struct Installed {
    config: Install,
    remote: RemotePlugin,
    manifest: OnceCell<Manifest>,
}

pub(crate) struct Plugins {
    path: PathBuf,
    queue: crate::plugin_events::Queue,
    wake: tokio::sync::Notify,
    entries: RwLock<BTreeMap<String, Arc<Installed>>>,
    update: Mutex<()>,
    error: Option<String>,
}

#[derive(Serialize)]
struct Summary<'a> {
    id: &'a str,
    label: &'a str,
    functions: Vec<&'a plugin_wire::plugin::Tool>,
    slots: Vec<Slot>,
    enabled: bool,
    retrying_deliveries: i64,
    error: Option<String>,
}

impl Plugins {
    pub(crate) fn load(home: &Path) -> anyhow::Result<Self> {
        let directory = home.join(".lifeos");
        std::fs::create_dir_all(&directory)?;
        let path = directory.join("plugins.json");
        let (entries, error) = match Self::read(&path) {
            Ok(entries) => (entries, None),
            Err(error) => {
                tracing::warn!(%error, "plugin configuration failed");
                (BTreeMap::new(), Some(error.to_string()))
            }
        };
        Ok(Self {
            path,
            entries: RwLock::new(entries),
            error,
            update: Mutex::new(()),
            queue: crate::plugin_events::Queue::open(&directory.join("plugin-events.sqlite"))?,
            wake: Default::default(),
        })
    }

    fn read(path: &Path) -> anyhow::Result<BTreeMap<String, Arc<Installed>>> {
        Ok(configs(path)?
            .into_iter()
            .map(|(id, config)| {
                let plugin = Arc::new(Installed::new(&id, config));
                (id, plugin)
            })
            .collect())
    }

    fn snapshot(&self) -> BTreeMap<String, Arc<Installed>> {
        self.entries.read().expect("plugin registry lock").clone()
    }

    fn enabled(&self, id: &str) -> Result<Arc<Installed>, ApiError> {
        self.entries
            .read()
            .expect("plugin registry lock")
            .get(id)
            .filter(|plugin| !plugin.config.disabled)
            .cloned()
            .ok_or_else(|| not_found("Plugin is unavailable"))
    }

    fn packaged_github(&self) -> PathBuf {
        self.path
            .with_file_name("plugin-packages")
            .join("lifeos-github")
    }

    fn install_github(&self) -> anyhow::Result<Install> {
        let package = self.packaged_github();
        let body = std::fs::read(&package)?;
        let sha256 = hex::encode(ring::digest::digest(&ring::digest::SHA256, &body));
        let folder = self.path.with_file_name("plugins");
        std::fs::create_dir_all(&folder)?;
        let binary = folder.join(format!("lifeos-github-{sha256}"));
        let mut staged = tempfile::NamedTempFile::new_in(&folder)?;
        use std::io::Write;
        staged.write_all(&body)?;
        staged
            .as_file()
            .set_permissions(std::fs::metadata(package)?.permissions())?;
        staged.persist(&binary)?;
        Ok(Install {
            binary,
            sha256,
            disabled: false,
            args: vec![],
            functions: vec![],
            hooks: BTreeMap::new(),
            events: BTreeMap::new(),
            vars: Vars::new(),
        })
    }

    async fn change(&self, id: &str, action: Action) -> anyhow::Result<()> {
        let _update = self.update.lock().await;
        let running = self.snapshot();
        let mut configs = configs(&self.path)?;
        let config = match action {
            Action::Install => {
                anyhow::ensure!(
                    id == "github" && !configs.contains_key(id),
                    "Plugin is already installed or unavailable"
                );
                configs
                    .entry(id.to_owned())
                    .or_insert(self.install_github()?)
            }
            Action::Enable | Action::Disable => {
                let config = configs
                    .get_mut(id)
                    .ok_or_else(|| anyhow::anyhow!("Plugin is not installed"))?;
                config.disabled = matches!(action, Action::Disable);
                config
            }
        };
        let replacement = Arc::new(Installed::new(id, config.clone()));
        if !replacement.config.disabled {
            replacement
                .manifest()
                .await
                .map_err(|(_, error)| anyhow::anyhow!(error))?;
        }
        let folder = self.path.parent().expect("plugin config parent");
        let mut staged = tempfile::NamedTempFile::new_in(folder)?;
        serde_json::to_writer_pretty(&mut staged, &configs)?;
        staged.as_file().sync_all()?;
        staged.persist(&self.path)?;
        self.entries
            .write()
            .expect("plugin registry lock")
            .insert(id.to_owned(), replacement);
        if let Some(previous) = running.get(id) {
            previous.remote.retire().await;
        }
        Ok(())
    }

    pub(crate) async fn watch_changes(&self, mut changes: watch::Receiver<u64>) {
        let mut event = "lifeos.started";
        loop {
            let revision = *changes.borrow_and_update();
            let body = if event == "lifeos.started" {
                json!({})
            } else {
                json!({ "revision": revision })
            };
            let item = self.event(event.into(), body, String::new());
            let has_deliveries = !item.subscribers.is_empty();
            if let Err(error) = self.queue.commit(None, vec![item]) {
                tracing::error!(%error, %event, "could not enqueue plugin event");
            }
            if has_deliveries {
                self.wake.notify_one();
            }
            if changes.changed().await.is_err() {
                return;
            }
            event = "lifeos.changed";
        }
    }

    fn event(&self, key: String, body: Value, dedupe: String) -> crate::plugin_events::Event {
        let subscribers = self
            .snapshot()
            .into_iter()
            .filter(|(_, p)| !p.config.disabled)
            .filter_map(|(id, p)| Some((id, p.config.events.get(&key)?.clone())))
            .collect();
        crate::plugin_events::Event {
            key,
            body,
            dedupe,
            subscribers,
        }
    }

    fn readings(&self, id: &str, answer: ProcessAnswer) -> Vec<crate::plugin_events::Event> {
        answer
            .emits
            .into_iter()
            .take(64)
            .filter(|r| identifier(&r.key_suffix))
            .map(|r| self.event(format!("plugin.{id}.{}", r.key_suffix), r.value.0, r.dedupe))
            .collect()
    }

    async fn dispatch(&self) -> anyhow::Result<bool> {
        let Some(job) = self.queue.next()? else {
            return Ok(false);
        };
        let mut emits = vec![];
        if let Ok(plugin) = self.enabled(&job.plugin)
            && plugin.config.events.get(&job.key) == Some(&job.processor)
        {
            let reading = plugin_wire::plugin::ProcessedReading {
                key: job.key.clone(),
                rev: 0,
                seq: 0,
                at_ms: job.at,
                value: job.body.clone().into(),
                cause_kind: "event".into(),
                cause_who: "lifeos".into(),
                chain_id: String::new(),
            };
            match plugin
                .process(&job.processor, job.id.clone(), json!({}), Some(reading))
                .await
            {
                Ok(answer) => emits = self.readings(&job.plugin, answer),
                Err((status, _)) => {
                    tracing::warn!(plugin=%job.plugin, request_id=%job.id, %status, attempt=job.attempts+1, "plugin delivery failed");
                    self.queue.retry(&job)?;
                    return Ok(true);
                }
            }
        }
        self.queue.commit(Some(&job), emits)?;
        Ok(true)
    }

    pub(crate) async fn worker(&self) {
        loop {
            match self.dispatch().await {
                Ok(true) => continue,
                Ok(false) => {}
                Err(error) => tracing::error!(%error, "plugin delivery queue failed"),
            }
            tokio::select! {
                _ = self.wake.notified() => {},
                _ = tokio::time::sleep(Duration::from_secs(1)) => {},
            }
        }
    }
}

type ApiError = (StatusCode, String);

fn not_found(message: &str) -> ApiError {
    (StatusCode::NOT_FOUND, message.into())
}

fn failed(error: link::Failure) -> ApiError {
    let status = match error.code {
        link::ErrorCode::InvalidArgument => StatusCode::BAD_REQUEST,
        link::ErrorCode::NotFound => StatusCode::NOT_FOUND,
        link::ErrorCode::Forbidden => StatusCode::FORBIDDEN,
        _ => StatusCode::BAD_GATEWAY,
    };
    (status, error.message)
}

impl Installed {
    fn new(id: &str, config: Install) -> Self {
        let remote = RemotePlugin::new(Spec {
            id: id.into(),
            binary: config.binary.clone(),
            args: config.args.clone(),
            sha256: config.sha256.clone(),
        });
        Self {
            config,
            remote,
            manifest: OnceCell::new(),
        }
    }

    async fn manifest(&self) -> Result<&Manifest, ApiError> {
        self.manifest
            .get_or_try_init(|| async { self.remote.describe().await.map_err(failed) })
            .await
    }

    async fn process(
        &self,
        processor: &str,
        request_id: String,
        params: Value,
        reading: Option<plugin_wire::plugin::ProcessedReading>,
    ) -> Result<ProcessAnswer, ApiError> {
        if !self
            .manifest()
            .await?
            .processors
            .iter()
            .any(|p| p.name == processor)
        {
            return Err(not_found("Unknown plugin processor"));
        }
        tokio::time::timeout(
            Duration::from_secs(15),
            self.remote.process(Firing {
                processor: processor.into(),
                request_id,
                reading,
                params: params.into(),
                vars: self.config.vars.clone(),
            }),
        )
        .await
        .map_err(|_| {
            (
                StatusCode::GATEWAY_TIMEOUT,
                "Plugin processor timed out".into(),
            )
        })?
        .map_err(failed)
    }
}

pub(super) async fn signals(State(state): State<Arc<HttpState>>) -> Result<Json<Value>, ApiError> {
    let plugins = &state.app.plugins;
    let mut subscriptions = Vec::new();
    for (id, plugin) in plugins.snapshot() {
        for (event, processor) in &plugin.config.events {
            subscriptions.push(json!({
                "event": event, "plugin": id, "processor": processor,
                "enabled": !plugin.config.disabled,
            }));
        }
    }
    let events = plugins.queue.history().map_err(|error| {
        tracing::error!(%error, "could not read signal history");
        (
            StatusCode::SERVICE_UNAVAILABLE,
            "Signal history unavailable".into(),
        )
    })?;
    Ok(Json(
        json!({"subscriptions": subscriptions, "events": events, "error": plugins.error}),
    ))
}

pub(super) async fn catalog(State(state): State<Arc<HttpState>>) -> Result<Json<Value>, ApiError> {
    let plugins = &state.app.plugins;
    let mut entries = Vec::new();
    let snapshot = plugins.snapshot();
    let retries = plugins.queue.retry_counts().map_err(|e| {
        tracing::error!(%e, "cannot read plugin queue");
        (
            StatusCode::SERVICE_UNAVAILABLE,
            "Plugin queue unavailable".into(),
        )
    })?;
    for (id, plugin) in &snapshot {
        let retrying_deliveries = retries.get(id).copied().unwrap_or(0);
        let entry = if plugin.config.disabled {
            Summary {
                retrying_deliveries,
                id,
                label: if id == "github" { "GitHub" } else { id },
                functions: vec![],
                slots: vec![],
                enabled: false,
                error: None,
            }
        } else {
            match plugin.manifest().await {
                Ok(manifest) => Summary {
                    retrying_deliveries,
                    id,
                    label: &manifest.label,
                    functions: manifest
                        .tools
                        .iter()
                        .filter(|t| plugin.config.functions.contains(&t.name))
                        .collect(),
                    slots: manifest
                        .ui
                        .as_ref()
                        .into_iter()
                        .flat_map(|ui| {
                            ui.slots.iter().cloned().map(|mut slot| {
                                if slot.expose.is_empty() {
                                    slot.expose.clone_from(&ui.entry);
                                }
                                slot
                            })
                        })
                        .filter(|slot| {
                            matches!(slot.slot.as_str(), "page" | "dashboard")
                                && identifier(&slot.id)
                                && asset_path(&slot.expose)
                                && slot.expose.ends_with(".html")
                        })
                        .collect(),
                    enabled: true,
                    error: None,
                },
                Err((_, error)) => Summary {
                    retrying_deliveries,
                    id,
                    label: id,
                    functions: vec![],
                    slots: vec![],
                    enabled: true,
                    error: Some(error),
                },
            }
        };
        entries.push(entry);
    }
    let available = if !snapshot.contains_key("github") && plugins.packaged_github().is_file() {
        vec![json!({ "id": "github", "label": "GitHub" })]
    } else {
        vec![]
    };
    Ok(Json(
        json!({ "plugins": entries, "available": available, "error": plugins.error }),
    ))
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Action {
    Install,
    Enable,
    Disable,
}

pub(super) async fn manage(
    State(state): State<Arc<HttpState>>,
    Route(id): Route<String>,
    headers: HeaderMap,
    Json(action): Json<Action>,
) -> Result<StatusCode, ApiError> {
    if headers
        .get("x-lifeos-plugin")
        .is_none_or(|value| value != "1")
        || headers
            .get("sec-fetch-site")
            .is_some_and(|value| value != "same-origin" && value != "none")
    {
        return Err((
            StatusCode::FORBIDDEN,
            "Plugin changes require a LifeOS request".into(),
        ));
    }
    let revoked = if id == "github" && matches!(action, Action::Disable) {
        state
            .app
            .life
            .lock()
            .expect("life lock")
            .github_connection_names()
            .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?
    } else {
        Vec::new()
    };
    state
        .app
        .plugins
        .change(&id, action)
        .await
        .map_err(|error| (StatusCode::CONFLICT, error.to_string()))?;
    if !revoked.is_empty() {
        state.app.agents.revoke_connections(&revoked);
    }
    state.app.bump();
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub(super) struct Call {
    arguments: Value,
}

pub(super) async fn call(
    State(state): State<Arc<HttpState>>,
    Route((id, tool)): Route<(String, String)>,
    Json(input): Json<Call>,
) -> Result<Json<Value>, ApiError> {
    let plugin = state.app.plugins.enabled(&id)?;
    if !plugin.config.functions.contains(&tool)
        || !plugin
            .manifest()
            .await?
            .tools
            .iter()
            .any(|t| t.name == tool)
    {
        return Err(not_found("Unknown plugin function"));
    }
    let request = link::Request {
        id: uuid::Uuid::new_v4().to_string(),
    };
    let result = plugin
        .remote
        .call(request, tool, input.arguments, plugin.config.vars.clone())
        .await
        .map_err(failed)?;
    Ok(Json(json!({ "result": result })))
}

pub(super) async fn hook(
    State(state): State<Arc<HttpState>>,
    Route((id, name)): Route<(String, String)>,
    request: Request,
) -> Result<Json<Value>, Response> {
    let plugins = &state.app.plugins;
    let plugin = plugins.enabled(&id).map_err(IntoResponse::into_response)?;
    let binding = plugin
        .config
        .hooks
        .get(&name)
        .ok_or_else(|| not_found("Unknown plugin hook").into_response())?;
    let (processor, params) = match binding {
        Hook::Json(processor) => {
            let Json(params) = Json::<Value>::from_request(request, &state)
                .await
                .map_err(IntoResponse::into_response)?;
            (processor, params)
        }
        Hook::Request { processor, headers } => {
            let mut forwarded = BTreeMap::new();
            for name in headers {
                if let Some(value) = request
                    .headers()
                    .get(name.as_str())
                    .and_then(|v| v.to_str().ok())
                {
                    forwarded.insert(name, value.to_owned());
                }
            }
            let body = Bytes::from_request(request, &state)
                .await
                .map_err(IntoResponse::into_response)?;
            let body = std::str::from_utf8(&body).map_err(|_| {
                (StatusCode::BAD_REQUEST, "Expected a UTF-8 webhook body").into_response()
            })?;
            (processor, json!({ "body": body, "headers": forwarded }))
        }
    };
    let answer = plugin
        .process(processor, uuid::Uuid::new_v4().to_string(), params, None)
        .await
        .map_err(IntoResponse::into_response)?;
    let text = answer.text.clone();
    let events = plugins.readings(&id, answer);
    if !events.is_empty() {
        plugins.queue.commit(None, events).map_err(|error| {
            tracing::error!(%error, "could not enqueue webhook events");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                "Plugin event storage unavailable",
            )
                .into_response()
        })?;
        plugins.wake.notify_one();
    }
    Ok(Json(json!({ "result": text })))
}

pub(super) async fn asset(
    State(state): State<Arc<HttpState>>,
    Route((id, name)): Route<(String, String)>,
) -> Result<Response, ApiError> {
    let plugin = state.app.plugins.enabled(&id)?;
    if !asset_path(&name) {
        return Err(not_found("Unknown plugin asset"));
    }
    if plugin.manifest().await?.ui.is_none() {
        return Err(not_found("Plugin has no frontend"));
    }
    let asset = plugin.remote.asset(name.clone()).await.map_err(failed)?;
    let mime = asset_mime(&name, &asset.content_type)?;
    Ok(([
        (header::CONTENT_TYPE, mime.as_ref()),
        (header::CONTENT_SECURITY_POLICY, "sandbox allow-scripts; default-src 'none'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; base-uri 'none'; form-action 'none'; frame-ancestors 'self'"),
        (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
    ], asset.content).into_response())
}

fn asset_path(name: &str) -> bool {
    !name.split('/').any(|part| {
        part.is_empty()
            || matches!(part, "." | "..")
            || !part
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
    })
}

fn asset_mime(name: &str, declared: &str) -> Result<mime_guess::Mime, ApiError> {
    if declared.is_empty() {
        return Ok(mime_guess::from_path(name).first_or_octet_stream());
    }
    declared.parse().map_err(|_| {
        (
            StatusCode::BAD_GATEWAY,
            "Invalid plugin asset content type".into(),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mime_for_extensionless_assets() {
        assert_eq!(
            asset_mime("bundle", "application/javascript")
                .unwrap()
                .as_ref(),
            "application/javascript"
        );
        assert_eq!(asset_mime("styles.css", "").unwrap().as_ref(), "text/css");
        assert!(asset_mime("bundle", "invalid type").is_err());
    }
}
