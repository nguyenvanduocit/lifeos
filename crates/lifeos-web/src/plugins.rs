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

const STARTED: &str = "lifeos.started";
const CHANGED: &str = "lifeos.changed";

struct Installed {
    home: PathBuf,
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
    pub(crate) fn load(life: &lifeos_core::Life) -> anyhow::Result<Self> {
        let directory = life.home().join(".lifeos");
        std::fs::create_dir_all(&directory)?;
        let path = directory.join("plugins.json");
        let (entries, error) = match Self::read(&path, life) {
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

    fn read(
        path: &Path,
        life: &lifeos_core::Life,
    ) -> anyhow::Result<BTreeMap<String, Arc<Installed>>> {
        let mut entries = configs(path)?;
        let original: Value = if path.exists() {
            serde_json::from_slice(&std::fs::read(path)?)?
        } else {
            Value::Null
        };
        let mut migrated = false;
        for (id, config) in &mut entries {
            if original[id].get("vault_keys").is_none() {
                for &key in lifeos_core::plugins::legacy_credentials(id) {
                    if !config.vault_keys.iter().any(|k| k == key) {
                        config.vault_keys.push(key.into());
                        migrated = true;
                    }
                }
            }
            for key in &config.vault_keys {
                if let Some(value) = config.vars.remove(key) {
                    if !value.is_empty() {
                        life.migrate_integration_key(key, &value)?;
                    }
                    migrated = true;
                }
            }
        }
        if migrated {
            Self::write_configs(path, &entries)?;
        }
        Ok(entries
            .into_iter()
            .map(|(id, config)| {
                let plugin = Arc::new(Installed::new(&id, config, path));
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

    fn package(&self, id: &str) -> PathBuf {
        self.path
            .with_file_name("plugin-packages")
            .join(format!("lifeos-{id}"))
    }

    fn package_manifest(&self, id: &str) -> anyhow::Result<Manifest> {
        anyhow::ensure!(identifier(id), "Invalid package identifier");
        let source = std::fs::read_to_string(self.package(id).with_extension("manifest.json"))?;
        let manifest = Manifest::parse_package(&source).map_err(anyhow::Error::msg)?;
        anyhow::ensure!(
            manifest.capability == id && !manifest.version.is_empty(),
            "Package identity or version missing"
        );
        Ok(manifest)
    }

    fn available(&self, installed: &BTreeMap<String, Arc<Installed>>) -> Vec<Value> {
        let mut packages = Vec::new();
        if let Ok(files) = std::fs::read_dir(self.path.with_file_name("plugin-packages")) {
            for file in files.flatten() {
                let name = file.file_name();
                let Some(id) = name
                    .to_str()
                    .and_then(|n| n.strip_prefix("lifeos-"))
                    .and_then(|n| n.strip_suffix(".manifest.json"))
                else {
                    continue;
                };
                if installed.contains_key(id) || !self.package(id).is_file() {
                    continue;
                }
                if let Ok(manifest) = self.package_manifest(id) {
                    packages.push(json!({"id":id,"label":manifest.label,"manifest":manifest}));
                }
            }
        }
        packages.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
        packages
    }

    fn install_package(&self, id: &str) -> anyhow::Result<Install> {
        let manifest = self.package_manifest(id)?;
        let package = self.package(id);
        let body = std::fs::read(&package)?;
        let sha256 = hex::encode(ring::digest::digest(&ring::digest::SHA256, &body));
        let folder = self.path.with_file_name("plugins");
        std::fs::create_dir_all(&folder)?;
        let binary = folder.join(format!("lifeos-{id}-{sha256}"));
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
            args: manifest
                .storage
                .into_iter()
                .map(|s| match s {
                    plugin_wire::plugin::Storage::VaultDatabase => "{lifeos_db}".into(),
                    plugin_wire::plugin::Storage::SignalHistory => "{signal_history}".into(),
                })
                .collect(),
            functions: vec![],
            vault_keys: vec![],
            agents:vec![],automations:vec![],
            hooks: BTreeMap::new(),
            events: BTreeMap::new(),
            vars: Vars::new(),
        })
    }

    async fn change(&self, id: &str, action: Action) -> anyhow::Result<()> {
        let _update = self.update.lock().await;
        let running = self.snapshot();
        let mut configs = configs(&self.path)?;
        let installing = matches!(action, Action::Install);
        let config = match action {
            Action::Install => {
                anyhow::ensure!(
                    identifier(id) && id != "telegram" && !configs.contains_key(id),
                    "Plugin is already installed or unavailable"
                );
                configs
                    .entry(id.to_owned())
                    .or_insert(self.install_package(id)?)
            }
            Action::Enable | Action::Disable => {
                let config = configs
                    .get_mut(id)
                    .ok_or_else(|| anyhow::anyhow!("Plugin is not installed"))?;
                config.disabled = matches!(action, Action::Disable);
                config
            }
        };
        if installing {
            let staged = Installed::new(id, config.clone(), &self.path);
            let manifest = staged
                .manifest()
                .await
                .map_err(|(_, e)| anyhow::anyhow!(e))?;
            anyhow::ensure!(
                manifest == &self.package_manifest(id)?,
                "Package declaration differs from executable"
            );
            for agent in &manifest.agents {
                anyhow::ensure!(agent.lifeos_tools.iter().all(|name|lifeos_core::tools::spec(name).is_some()), "Unknown LifeOS tool requested by {}", agent.id);
            }
            for routine in &manifest.automations {
                lifeos_core::life::routines::validate_plugin_trigger(routine.cron.as_deref(), &routine.on)?;
            }
            config.hooks = manifest.webhooks.iter().map(|(id,hook)| (id.clone(),Hook::Request{processor:hook.processor.clone(),headers:hook.headers.clone()})).collect();
            config.agents.clone_from(&manifest.agents);
            config.automations.clone_from(&manifest.automations);
            config.functions = manifest.tools.iter().map(|t| t.name.clone()).collect();
            config.vault_keys = manifest
                .vars
                .iter()
                .filter(|v| v.vault)
                .map(|v| v.name.clone())
                .collect();
        }
        let replacement = Arc::new(Installed::new(id, config.clone(), &self.path));
        if !replacement.config.disabled {
            replacement
                .manifest()
                .await
                .map_err(|(_, error)| anyhow::anyhow!(error))?;
        }
        self.persist_configs(&configs)?;
        self.entries
            .write()
            .expect("plugin registry lock")
            .insert(id.to_owned(), replacement);
        if let Some(previous) = running.get(id) {
            previous.remote.retire().await;
        }
        Ok(())
    }

    fn persist_configs(&self, configs: &BTreeMap<String, Install>) -> anyhow::Result<()> {
        Self::write_configs(&self.path, configs)
    }

    fn write_configs(path: &Path, configs: &BTreeMap<String, Install>) -> anyhow::Result<()> {
        let folder = path.parent().expect("plugin config parent");
        let mut staged = tempfile::NamedTempFile::new_in(folder)?;
        serde_json::to_writer_pretty(&mut staged, &configs)?;
        staged.as_file().sync_all()?;
        staged.persist(path)?;
        Ok(())
    }

    pub(crate) async fn watch_changes(&self, mut changes: watch::Receiver<u64>) {
        let mut event = STARTED;
        loop {
            let revision = *changes.borrow_and_update();
            let body = if event == STARTED {
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
            event = CHANGED;
        }
    }

    fn event(&self, key: String, body: Value, dedupe: String) -> crate::plugin_events::Event {
        let mut subscribers=Vec::new();
        for (id,plugin) in self.snapshot() {
            if plugin.config.disabled {continue;}
            if let Some(processor)=plugin.config.events.get(&key) {subscribers.push((id.clone(),processor.clone()));}
            for routine in &plugin.config.automations {
                if routine.on.contains(&key) {subscribers.push((id.clone(),format!("routine:{}",routine.id)));}
            }
        }
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

    async fn dispatch(&self,life:&lifeos_agent::SharedLife) -> anyhow::Result<bool> {
        let Some(job) = self.queue.next()? else {
            return Ok(false);
        };
        let mut emits = vec![];
        if let Some(local)=job.processor.strip_prefix("routine:") {
            if self.enabled(&job.plugin).is_ok_and(|p|p.config.automations.iter().any(|r|r.id==local && r.on.contains(&job.key))) {
                let id=lifeos_core::plugins::contribution_id(&job.plugin,local);
                let result=life.lock().expect("life lock").queue_plugin_delivery(&id,&job.key,&job.id,&job.body);
                if let Err(error)=result {
                    tracing::warn!(%id,%error,"could not queue plugin routine");
                    self.queue.retry(&job)?;
                    return Ok(true);
                }
            }
        } else if let Ok(plugin) = self.enabled(&job.plugin)
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

    pub(crate) async fn worker(&self,life:&lifeos_agent::SharedLife) {
        loop {
            match self.dispatch(life).await {
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
    fn new(id: &str, config: Install, registry: &Path) -> Self {
        let remote = RemotePlugin::new(Spec {
            id: id.into(),
            binary: config.binary.clone(),
            args: config.resolved_args(registry),
            sha256: config.sha256.clone(),
        });
        Self {
            home: registry.parent().unwrap().parent().unwrap().to_owned(),
            config,
            remote,
            manifest: OnceCell::new(),
        }
    }

    fn vars(&self) -> Result<Vars, ApiError> {
        lifeos_core::plugins::runtime_vars(&self.home, &self.config, None).map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Cannot access Vault credentials".into(),
            )
        })
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
                vars: self.vars()?,
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
    let history_error = |error: anyhow::Error| {
        tracing::error!(%error, "could not read signal history");
        (
            StatusCode::SERVICE_UNAVAILABLE,
            "Signal history unavailable".into(),
        )
    };
    let mut signals = BTreeMap::from([
        (STARTED.to_owned(), "When LifeOS starts".to_owned()),
        (CHANGED.to_owned(), "When the LifeOS data revision changes".to_owned()),
    ]);
    let mut subscriptions = Vec::new();
    for (id, plugin) in plugins.snapshot() {
        if !plugin.config.disabled && let Ok(manifest) = plugin.manifest().await {
            for (event,note) in &manifest.emits { signals.insert(format!("plugin.{id}.{event}"),note.clone()); }
        }
        for routine in &plugin.config.automations {
            for event in &routine.on {
                subscriptions.push(json!({"event":event,"plugin":id,"processor":routine.title,"enabled":!plugin.config.disabled}));
            }
        }
        for (event, processor) in &plugin.config.events {
            signals
                .entry(event.clone())
                .or_insert_with(|| "Referenced by a subscription".into());
            subscriptions.push(json!({
                "event": event, "plugin": id, "processor": processor,
                "enabled": !plugin.config.disabled,
            }));
        }
    }
    for event in plugins.queue.observed_keys().map_err(history_error)? {
        signals.entry(event).or_insert_with(|| "Seen in event history".into());
    }
    let signals: Vec<_> = signals
        .into_iter()
        .map(|(event, note)| json!({ "event": event, "note": note }))
        .collect();
    let events = plugins.queue.history().map_err(history_error)?;
    Ok(Json(
        json!({"routineEvents": lifeos_core::tools::routine_event_names(), "signals": signals, "subscriptions": subscriptions, "events": events, "error": plugins.error}),
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
                            matches!(slot.slot.as_str(), "page" | "dashboard" | "sidebar")
                                && identifier(&slot.id)
                                && asset_path(&slot.expose)
                                && (slot.expose.ends_with(".html")
                                    || slot.expose.ends_with(".json"))
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
        let mut details = json!(entry);
        if let Some(manifest) = plugin.manifest.get().filter(|_| !plugin.config.disabled) {
            details["agents"] = json!(plugin.config.agents);
            details["automations"] = json!(plugin.config.automations);
            details["vault_keys"] = json!(manifest.vars.iter()
                .filter(|var| var.vault)
                .map(|var| &var.name)
                .collect::<Vec<_>>());
            details["emits"] = json!(manifest.emits);
            details["description"] = json!(manifest.description);
            details["capability"] = json!(manifest.capability);
            details["processors"] = json!(
                manifest
                    .processors
                    .iter()
                    .map(|p| &p.label)
                    .collect::<Vec<_>>()
            );
            details["has_settings"] = json!(
                !manifest.vars.is_empty()
                    || manifest
                        .ui
                        .iter()
                        .flat_map(|ui| &ui.slots)
                        .any(|s| s.slot == "settings")
            );
        }
        entries.push(details);
    }
    let mut available = plugins.available(&snapshot);
    let telegram = state.app.telegram.current();
    if telegram.installed {
        let manifest = lifeos_mcp::telegram::definition().manifest();
        entries.push(json!({
            "id": "telegram",
            "label": manifest.label,
            "description": manifest.description,
            "capability": manifest.capability,
            "has_settings": true,
            "vault_keys": manifest.vars.iter().filter(|var| var.vault).map(|var| &var.name).collect::<Vec<_>>(),
            "configured": state.app.telegram.configured(),
            "chats": telegram.chats,
            "enabled": telegram.enabled,
            "functions": manifest.tools,
            "slots": [],
            "retrying_deliveries": 0,
            "error": null,
        }));
    } else {
        available.push(json!({"id":"telegram","label":"Telegram","manifest":lifeos_mcp::telegram::definition().manifest()}));
    }
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
    if id == "telegram" {
        state.app.telegram.change(action)?;
        return Ok(StatusCode::NO_CONTENT);
    }
    let disabling = matches!(action,Action::Disable);
    let revoked = if id == "github" && disabling {
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
    if disabling { state.app.agents.stop_plugin_agents(&id); }
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
        .call(request, tool, input.arguments, plugin.vars()?)
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

#[path = "plugin_settings.rs"]
mod settings;
pub(crate) use settings::{settings_save, settings_view};

pub(super) async fn manifest_view(
    State(state): State<Arc<HttpState>>,
    Route(id): Route<String>,
) -> Result<Json<Value>, ApiError> {
    if id == "telegram" && state.app.telegram.current().enabled {
        return Ok(Json(json!(lifeos_mcp::telegram::definition().manifest())));
    }
    Ok(Json(json!(
        state.app.plugins.enabled(&id)?.manifest().await?
    )))
}
