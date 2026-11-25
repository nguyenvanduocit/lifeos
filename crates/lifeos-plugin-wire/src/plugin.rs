//! The contract between Remit and a plugin it runs as a child process (ADR-0072 decision 7), from `sdk/pluginpb/plugin.proto`.
//! The plugin dials over its stdio: its `hello` offers `plugin` (the calls below, which Remit makes) and asks for `forge.read`
//! (the forge's one question, `forge`: the checks on a landed commit), which Remit's `welcome` grants only to a plugin the owner granted it; a plugin that asks
//! without the grant is answered `FORBIDDEN`. Its `bundle` is `PLUGIN_CONTRACT`. A sensor's readings come as pushes on the
//! stream of its sensor id, in order, and its last push is `ended`. JSON documents (schemas, settings, params, bodies, values)
//! travel as JSON, not as text.

use std::collections::BTreeMap;

use model::ForeignJson;
use serde::{Deserialize, Serialize};

use crate::Ack;

/// The contract's number, the plugin's `hello.bundle`. Additive changes keep it; any other change bumps it, and Remit refuses a
/// plugin built against another (`new_bundle`): it is built again.
pub const PLUGIN_CONTRACT: i64 = 4;

/// The credential variables of the connection a call came through, by name.
pub type Vars = BTreeMap<String, String>;

/// What a tool or processor does to the system it reaches, and so the grant an agent needs to call it: the grant ladder's
/// one `Effect`, which travels as [`EffectDef`] says.
pub use grant::Effect;

/// How an [`Effect`] is written in the contract and in a stored manifest: the variant, as serde's remote derive for a type
/// of another crate writes it (`#[serde(with = "EffectDef")]`). The compiler holds its variants to the ladder's.
#[derive(schemars::JsonSchema, Serialize, Deserialize)]
#[serde(remote = "Effect", rename_all = "snake_case")]
pub enum EffectDef {
    Read,
    Reversible,
    Compensable,
    Irreversible,
}

/// Whether `name` is a tool's: a lower case word of 2 to 48 letters, digits and underscores, starting with a letter.
#[must_use]
pub fn is_tool_name(name: &str) -> bool {
    word(
        name,
        |c| c.is_ascii_lowercase(),
        |c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_',
        2..=48,
    )
}

/// Whether `name` is a credential variable's: upper case words of 2 to 64 letters, digits and underscores that do not move
/// where a program is loaded from or what it runs (a credential never sets `PATH`, `LD_PRELOAD` and the like).
#[must_use]
pub fn is_var_name(name: &str) -> bool {
    let reserved = matches!(
        name,
        "PATH"
            | "HOME"
            | "TMPDIR"
            | "SHELL"
            | "IFS"
            | "ENV"
            | "BASH_ENV"
            | "PYTHONPATH"
            | "PYTHONHOME"
            | "NODE_OPTIONS"
            | "NODE_PATH"
            | "RUBYOPT"
            | "PERL5OPT"
            | "GOFLAGS"
    ) || name.starts_with("LD_")
        || name.starts_with("DYLD_");
    !reserved
        && word(
            name,
            |c| c.is_ascii_uppercase(),
            |c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'_',
            2..=64,
        )
}

/// Whether `s` is `len` bytes long, its first byte one `first` takes and every other one `rest` takes.
fn word(s: &str, first: fn(u8) -> bool, rest: fn(u8) -> bool, len: std::ops::RangeInclusive<usize>) -> bool {
    let b = s.as_bytes();
    len.contains(&b.len()) && b.first().is_some_and(|c| first(*c)) && b.iter().skip(1).all(|c| rest(*c))
}

/// One credential variable the plugin reads: `secret` is stored sealed and never shown, `address` says where the system is.
#[derive(schemars::JsonSchema, Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Var {
    /// Requests the Vault key of the same name at installation.
    #[serde(default)]
    pub vault: bool,
    pub name: String,
    pub what: String,
    pub secret: bool,
    pub optional: bool,
    pub address: bool,
}

/// One question or act an agent can ask of the plugin; `inputSchema` is a JSON Schema document.
#[derive(schemars::JsonSchema, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tool {
    pub name: String,
    pub description: String,
    #[schemars(with = "serde_json::Map<String, serde_json::Value>")]
    pub input_schema: ForeignJson,
    #[schemars(with = "EffectDef")]
    #[serde(with = "EffectDef")]
    pub effect: Effect,
}

/// An event signal the plugin ships, `event:<name>`, filled by its poll with `pollQuery` every `pollEveryMinutes` (zero: its
/// events come to its door); `paused` ships it waiting for the owner.
#[derive(schemars::JsonSchema, Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Signal {
    pub name: String,
    pub note: String,
    pub poll_query: String,
    pub poll_every_minutes: i32,
    pub paused: bool,
}

/// An agent template the plugin ships; `group` is the kind of work it is for, empty for general.
#[derive(schemars::JsonSchema, Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Agent {
    /// Plugin functions this agent may call, also subject to the installed allowlist.
    #[serde(default)]
    pub plugin_tools: Vec<String>,
    /// Native LifeOS tool names requested by this crew member.
    #[serde(default,skip_serializing_if="Vec::is_empty")]
    pub lifeos_tools: Vec<String>,
    pub id: String,
    pub name: String,
    pub description: String,
    pub group: String,
    pub instructions: String,
}

/// A native LifeOS routine targeting an agent contributed by this package.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct Automation {
    pub id:String, pub title:String, pub instructions:String, pub agent:String,
    /// Optional five-field cron expression in the LifeOS timezone. Omit for manual runs.
    #[serde(default)] pub cron: Option<String>,
    /// LifeOS write tools which trigger this routine.
    #[serde(default)] pub on: Vec<String>,
    #[serde(default)] pub paused:bool,
}

/// A kind of sensor the plugin adds (ADR-0049); `settingsSchema` is a JSON Schema document of a sensor's settings.
#[derive(schemars::JsonSchema, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SensorKind {
    pub name: String,
    pub label: String,
    #[schemars(with = "serde_json::Map<String, serde_json::Value>")]
    pub settings_schema: ForeignJson,
}

/// Something a firing can do that the plugin adds, named by a subscription as `plugin:<id>.<name>`.
#[derive(schemars::JsonSchema, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Processor {
    pub name: String,
    pub label: String,
    #[schemars(with = "serde_json::Map<String, serde_json::Value>")]
    pub params_schema: ForeignJson,
    #[schemars(with = "EffectDef")]
    #[serde(with = "EffectDef")]
    pub effect: Effect,
}

/// A subscription the plugin ships: exactly one of `processor` (with `params`, a JSON object) and `action` (`tell` or `run`,
/// with `job` and the expressions of `with`) is set.
#[derive(schemars::JsonSchema, Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Subscription {
    pub name: String,
    pub note: String,
    pub r#match: String,
    pub expr: String,
    pub processor: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<serde_json::Value>")]
    pub params: Option<ForeignJson>,
    pub action: String,
    pub job: String,
    pub with: BTreeMap<String, String>,
    pub paused: bool,
}

/// How the injecting proxy reaches the plugin's API and adds the connection's credential (ADR-0018, form T1).
#[derive(schemars::JsonSchema, Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Auth {
    pub base: String,
    pub scheme: String,
    pub header: String,
    pub param: String,
    pub hash: String,
    pub vars: Vec<String>,
    pub read_posts: Vec<String>,
    pub base_var: String,
    pub headers: BTreeMap<String, String>,
}

/// One place in the app the plugin fills with a component its remote exposes.
#[derive(schemars::JsonSchema, Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Slot {
    pub slot: String,
    pub id: String,
    pub title: String,
    pub expose: String,
}

/// The frontend a plugin brings: the entry of its Module Federation remote, one of its assets, and the slots it fills.
#[derive(schemars::JsonSchema, Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Ui {
    pub entry: String,
    pub slots: Vec<Slot>,
}

#[derive(Debug, PartialEq, schemars::JsonSchema, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Text,
    Secret,
    Select,
}

#[derive(Debug, PartialEq, schemars::JsonSchema, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Field {
    pub key: String,
    pub label: String,
    #[serde(rename = "type")]
    pub kind: Kind,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub options: Vec<String>,
    #[serde(default)]
    pub default: String,
}

#[derive(Debug, PartialEq, schemars::JsonSchema, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SettingsSchema {
    pub version: u32,
    #[serde(default)]
    pub description: String,
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, schemars::JsonSchema, Serialize, Deserialize)]
#[serde(rename_all="snake_case")]
pub enum Storage { VaultDatabase, SignalHistory }

#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize,schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Webhook {
    pub processor:String,
    pub headers:Vec<String>,
}

/// Everything the owner reads before applying a plugin; `poll` makes it a source of events, `accounts` makes each of its
/// connections a Claude account (ADR-0071).
#[derive(schemars::JsonSchema, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase",deny_unknown_fields)]
pub struct Manifest {
    #[serde(default = "manifest_version")]
    #[schemars(range(min=1,max=1))]
    pub schema_version: u32,
    #[serde(default)]
    pub automations: Vec<Automation>,
    #[serde(default)]
    pub emits:BTreeMap<String,String>,
    #[serde(default)]
    pub webhooks:BTreeMap<String,Webhook>,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub settings: Option<SettingsSchema>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub storage: Option<Storage>,
    pub label: String,
    pub capability: String,
    pub vars: Vec<Var>,
    pub tools: Vec<Tool>,
    pub poll: bool,
    pub accounts: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ui: Option<Ui>,
    pub signals: Vec<Signal>,
    pub agents: Vec<Agent>,
    pub sensor_kinds: Vec<SensorKind>,
    pub processors: Vec<Processor>,
    pub subscriptions: Vec<Subscription>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth: Option<Auth>,
}

pub const fn manifest_version() -> u32 { 1 }

impl Manifest {
    pub fn parse_package(source: &str) -> Result<Self,String> {
        let mut document: serde_json::Value = serde_json::from_str(source).map_err(|e| e.to_string())?;
        if document.get("schemaVersion").and_then(|v|v.as_u64()) != Some(1) {
            return Err("Package requires schemaVersion 1".into());
        }
        if let Some(object)=document.as_object_mut() { object.remove("$schema"); }
        let manifest: Self = serde_json::from_value(document).map_err(|e| e.to_string())?;
        manifest.check().map_err(str::to_owned)?;
        if manifest.version.trim().is_empty() || manifest.description.trim().is_empty() || manifest.vars.iter().any(|v|v.secret && !v.vault) {
            return Err("Package requires version, description and Vault bindings for secret fields".into());
        }
        Ok(manifest)
    }

    /// Validate the common contract at the host boundary, including non-SDK binaries.
    pub fn check(&self) -> Result<(), &'static str> {
        if self.schema_version != manifest_version() || self.label.trim().is_empty() || self.capability.trim().is_empty() {
            return Err("Unsupported or incomplete plugin manifest");
        }
        let mut names = std::collections::BTreeSet::new();
        for var in &self.vars {
            if !is_var_name(&var.name) || !names.insert(&var.name) { return Err("Invalid or duplicate variable"); }
        }
        if let Some(spec) = &self.settings {
            let mut fields = std::collections::BTreeSet::new();
            if spec.version != 1 || spec.fields.iter().any(|f| {
                let var = self.vars.iter().find(|v| v.name == f.key);
                !fields.insert(&f.key) || f.label.trim().is_empty() || var.is_none()
                    || (var.is_some_and(|v| v.secret) && !matches!(f.kind,Kind::Secret))
                    || (matches!(f.kind,Kind::Secret) && !f.default.is_empty())
                    || (matches!(f.kind,Kind::Select) && (f.options.is_empty() || (!f.default.is_empty() && !f.options.contains(&f.default))))
            }) { return Err("Invalid settings declaration"); }
        }
        names.clear();
        for tool in &self.tools {
            if !is_tool_name(&tool.name) || !names.insert(&tool.name) || tool.description.trim().is_empty()
                || tool.input_schema.0.get("type").and_then(|v| v.as_str()) != Some("object") {
                return Err("Invalid tool declaration");
            }
        }
        let valid_id = |id:&str| !id.is_empty() && id.len() <= 40 && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
        let mut agents = std::collections::BTreeSet::new();
        for a in &self.agents {
            if !valid_id(&a.id) || a.name.trim().is_empty() || a.instructions.trim().is_empty() || !agents.insert(&a.id)
                || a.plugin_tools.iter().any(|name| !names.contains(name)) {return Err("Invalid agent declaration");}
        }
        let mut routines = std::collections::BTreeSet::new();
        for r in &self.automations {
            if !valid_id(&r.id) || !routines.insert(&r.id) || !agents.contains(&r.agent) || r.title.trim().is_empty() || r.instructions.trim().is_empty() {
                return Err("Invalid automation declaration or agent reference");
            }
        }
        let event_name = |s:&str| !s.is_empty() && s.len()<=64 && s.bytes().all(|b|b.is_ascii_lowercase() || b.is_ascii_digit() || b==b'-' || b==b'_');
        if self.emits.iter().any(|(name,note)| !event_name(name) || note.trim().is_empty()) {return Err("Invalid event declaration");}
        if self.webhooks.iter().any(|(name,hook)|!event_name(name) || !self.processors.iter().any(|p|p.name==hook.processor)
            || hook.headers.iter().any(|s|!event_name(s))) {return Err("Invalid webhook declaration");}
        if let Some(ui) = &self.ui {
            let mut slots = std::collections::BTreeSet::new();
            for slot in &ui.slots {
                if slot.id.is_empty() || slot.slot.is_empty() || !slots.insert((&slot.slot,&slot.id)) || (slot.expose.is_empty() && ui.entry.is_empty()) {
                    return Err("Invalid UI slot");
                }
            }
        }
        Ok(())
    }
}

pub fn manifest_schema() -> schemars::Schema {
    let mut schema = schemars::schema_for!(Manifest);
    let obj = schema.ensure_object();
    obj["properties"]["$schema"] = serde_json::json!({"type":"string"});
    if let Some(fields) = obj["required"].as_array_mut() {
        fields.extend(["schemaVersion","version","description"].map(|s|serde_json::Value::String(s.into())));
    }
    schema
}

/// What a poll is asked: the owner's query, the cursor of the last poll (empty on the first) and the most items to answer.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PollAsk {
    pub query: String,
    pub cursor: String,
    pub limit: i64,
    pub vars: Vars,
}

/// One thing that happened; `body` is a JSON object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PollItem {
    pub id: String,
    pub version: String,
    pub body: ForeignJson,
}

/// The items oldest first and the cursor to ask from next; the first poll answers none and the cursor that marks now.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PollAnswer {
    pub items: Vec<PollItem>,
    pub cursor: String,
}

/// One sensor to open: its kind and name, its settings (a JSON object), the credential variables it reads through, and what
/// Remit kept of its last run: the cursor the plugin last gave and the instant of its last reading (ms since the epoch, 0 on
/// the first).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SenseOpen {
    pub kind: String,
    pub name: String,
    pub settings: ForeignJson,
    pub vars: Vars,
    pub cursor: String,
    pub last_at_ms: i64,
}

/// One reading a sensor writes, or a processor asks Remit to write. Remit decides the key: `keySuffix`, when not empty, is a
/// word added to the sensor's signal after a dot; `dedupe`, when not empty, makes a retry of the same reading one reading.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reading {
    pub key_suffix: String,
    pub value: ForeignJson,
    pub dedupe: String,
}

/// A reading as a processor reads it: `rev` is its revision of its key, `seq` its place in the log; `causeKind` and
/// `causeWho` say who made it, `chainId` the chain whose agent did, when one did.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessedReading {
    pub key: String,
    pub rev: i64,
    pub seq: i64,
    pub at_ms: i64,
    pub value: ForeignJson,
    pub cause_kind: String,
    pub cause_who: String,
    pub chain_id: String,
}

/// One firing handed to a processor: `requestId` names it and is the same on every delivery of it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Firing {
    pub processor: String,
    pub request_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reading: Option<ProcessedReading>,
    pub params: ForeignJson,
    pub vars: Vars,
}

/// What a processor did, in words, and the readings it asks Remit to write as a result.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessAnswer {
    pub text: String,
    pub emits: Vec<Reading>,
}

/// One file of the plugin's frontend: `content` is its bytes in base64; `contentType` is empty when unknown.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    pub content: String,
    pub content_type: String,
}

link::contract! {
    capability: "plugin";
    reads {
        describe() -> ForeignJson, why = "reads what the plugin says it is";
        poll(ask: PollAsk) -> PollAnswer, why = "reads the events after a cursor; only the host keeps the cursor it answers";
        asset(name: String) -> Asset, why = "reads one file of the plugin's frontend";
        /// The environment a Claude Code session runs with on the account the connection of `vars` is (ADR-0071).
        account(vars: Vars) -> Vars, why = "answers the environment of an account; asked again, it answers again";
    }
    changes {
        /// Runs one tool; `arguments` is a JSON object valid against its input schema. Remit never sends one again after its
        /// session ended: a tool that ran half way may have acted.
        call(tool: String, arguments: ForeignJson, vars: Vars) -> String, timeout_ms = 600000;
    }
    idempotent {
        process(firing: Firing) -> ProcessAnswer, why = "a firing delivered again acts once by its requestId";
        /// Starts sensing and returns at once: what it senses comes as pushes on the stream of `sensor`, the last `ended`.
        sense_open(sensor: String, open: SenseOpen) -> Ack, why = "a sensor already sensing is not opened again";
        /// Ends a sensor's run; no `ended` follows.
        sense_close(sensor: String) -> Ack, why = "a sensor closed twice is closed";
    }
    pushes {
        reading(sensor: String, reading: Reading);
        /// The cursor Remit keeps for the sensor and hands back when it opens it again.
        cursor(sensor: String, cursor: String);
        /// The run stopped by itself: `error` is why it failed, empty when it did not.
        ended(sensor: String, error: String);
    }
}

/// The one question a plugin granted `forge.read` asks Remit (ADR-0057, ADR-0114): the checks a forge's pipeline ran on a
/// commit Remit landed. The forge's login stays Remit's; nothing here merges, opens or pushes.
pub mod forge {
    use model::ForeignJson;

    link::contract! {
        capability: "forge.read";
        reads {
            /// The checks the pipeline of `project` on `host` ran on commit `sha`: a `model::Checks` as JSON, or JSON null
            /// while the pipeline has run none.
            checks(host: String, project: String, sha: String) -> ForeignJson, why = "reads the checks a forge's pipeline ran on a commit";
        }
        changes {}
        idempotent {}
        pushes {}
    }
}
