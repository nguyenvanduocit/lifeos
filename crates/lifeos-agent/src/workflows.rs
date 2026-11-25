//! The shell around the `workflow` engine (remit's `chains::workflow_run`, ADR-0073): a run's script is evaluated
//! against the journal of its steps' results; each call it makes that has none yet is recorded and issued; when one is
//! settled, the run is evaluated again. lifeos's verbs:
//!
//! - `agent(prompt, {agentType, schema, label})` asks the Assistant or the crew member `agentType` on its own card, as a
//!   routine does, and gives its answer: its text, or with a schema its last JSON value, checked.
//! - `askOwner(question)` puts a question to the owner, which the Assistant decides for them first (ADR-0087).
//! - `wait({seconds | minutes | hours | until})` comes due at its time.
//! - `log`, `phase` are recorded. `hire`, `accept`, `sendBack` and `workflow` fail: lifeos hires no one and runs no
//!   script as another's step.
//!
//! A step an agent was working on when lifeos stopped is failed on start, with why: the script decides what follows.

use std::sync::{Arc, PoisonError, Weak};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use chrono::{Local, NaiveDateTime};
use lifeos_wire::{ASSISTANT, Actor, ActorKind, Delivery};
use model::{ForeignJson, WorkflowVerb};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use workflow::{Entry, ScriptState};

use crate::toolbox::Toolbox;
use crate::{Agents, Inner};

/// How many evaluations one advance makes at most, while calls settle at once.
const MAX_PASSES: usize = 32;

/// How long the waits' ticker sleeps when none is due sooner.
const TICK: Duration = Duration::from_secs(60);

/// The time format of a wait's due time: local, sortable as text.
const DUE: &str = "%Y-%m-%dT%H:%M:%S";

/// Where a question a workflow asked the owner comes from: `workflow:<run>:<seq>`.
const ASKER: &str = "workflow:";

/// What agent() takes.
#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct AgentCall {
    prompt: String,
    schema: Option<Value>,
    agent_type: String,
}

/// What wait() takes.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct WaitCall {
    seconds: Option<f64>,
    minutes: Option<f64>,
    hours: Option<f64>,
    until: Option<String>,
}

/// When a wait comes due, from now. Pure but for now.
fn due_of(w: &WaitCall, now: NaiveDateTime) -> Result<NaiveDateTime> {
    if let Some(until) = w.until.as_deref() {
        if let Ok(t) = chrono::DateTime::parse_from_rfc3339(until) {
            return Ok(t.with_timezone(&Local).naive_local());
        }
        for f in ["%Y-%m-%dT%H:%M:%S", "%Y-%m-%dT%H:%M", "%Y-%m-%d %H:%M"] {
            if let Ok(t) = NaiveDateTime::parse_from_str(until, f) {
                return Ok(t);
            }
        }
        bail!("wait()'s until {until:?} is no time: write 2026-11-25T18:30 or RFC 3339");
    }
    let secs = w.seconds.unwrap_or(0.0) + w.minutes.unwrap_or(0.0) * 60.0 + w.hours.unwrap_or(0.0) * 3600.0;
    if !secs.is_finite() || secs <= 0.0 {
        bail!("wait() takes seconds, minutes, hours or until");
    }
    Ok(now + chrono::Duration::milliseconds((secs * 1000.0) as i64))
}

/// The last whole JSON object or array in s, compacted (remit's `last_json`). Pure.
pub(crate) fn last_json(s: &str) -> Option<String> {
    let mut last = None;
    let mut from = 0;
    while let Some(i) = s.get(from..).and_then(|rest| rest.find(['{', '['])).map(|i| from + i) {
        let rest = s.get(i..).unwrap_or_default();
        let mut values = serde_json::Deserializer::from_str(rest).into_iter::<Value>();
        match values.next() {
            Some(Ok(v)) => {
                last = Some(v.to_string());
                from = i + values.byte_offset().max(1);
            }
            _ => from = i + 1,
        }
    }
    last
}

/// What agent() returns for an answer: its text as a string, or, when the call names a schema, its last JSON value
/// checked against the schema (remit's `report_value`). The error says what is wrong with it. Pure.
pub(crate) fn report_value(answer: &str, schema: Option<&Value>) -> std::result::Result<String, String> {
    let Some(schema) = schema.filter(|s| !s.is_null()) else {
        return serde_json::to_string(answer).map_err(|e| e.to_string());
    };
    let validator = jsonschema::validator_for(schema).map_err(|e| format!("the call's schema is no JSON schema: {e}"))?;
    let raw = last_json(answer).ok_or_else(|| "the answer holds no JSON value".to_owned())?;
    let value: Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    validator.validate(&value).map_err(|e| format!("its last JSON value does not match the schema: {e}"))?;
    Ok(raw)
}

fn now_due() -> String {
    Local::now().naive_local().format(DUE).to_string()
}

fn entry_of(s: lifeos_core::life::workflows::Settled) -> Result<Entry> {
    Ok(Entry {
        seq: s.seq,
        verb: s.verb.parse().map_err(|_| anyhow::anyhow!("unknown verb {}", s.verb))?,
        hash: s.hash,
        result: if s.result.is_empty() { None } else { Some(ForeignJson::parse(&s.result)?) },
        err: s.error,
    })
}

/// The run and step a question a workflow asked names, when it is one.
pub(crate) fn asked_by(asker: &str) -> Option<(i64, i64)> {
    let (run, seq) = asker.strip_prefix(ASKER)?.split_once(':')?;
    Some((run.parse().ok()?, seq.parse().ok()?))
}

impl Agents {
    /// Starts a run of the script `workflows/<name>.js`, or of `script` given inline, with `args`. Its id.
    pub async fn run_workflow(&self, name: Option<&str>, script: Option<&str>, args: Option<Value>, by: &str) -> Result<i64> {
        self.inner.start_workflow(name, script, args, by)
    }

    /// Stops run `id`: its pending steps fail, and the script is not evaluated again.
    pub fn stop_workflow(&self, id: i64) -> Result<bool> {
        let stopped = self.inner.life().end_workflow_run(id, "stopped", "", "stopped by the owner")?;
        self.inner.config.on_change.fire();
        Ok(stopped)
    }
}

impl Inner {
    fn start_workflow(self: &Arc<Self>, name: Option<&str>, script: Option<&str>, args: Option<Value>, by: &str) -> Result<i64> {
        let (name, script) = match (name.filter(|n| !n.trim().is_empty()), script.filter(|s| !s.trim().is_empty())) {
            (_, Some(s)) => (name.unwrap_or("inline").to_owned(), s.to_owned()),
            (Some(n), None) => (n.trim().to_owned(), self.life().workflow_script(n.trim())?),
            (None, None) => bail!("say which workflow (name: its file in workflows/) or give its script"),
        };
        let args = args.filter(|a| !a.is_null()).map(|a| a.to_string()).unwrap_or_default();
        let id = self.life().new_workflow_run(&name, "", &script, &args, by)?;
        self.config.on_change.fire();
        self.advance_soon(id);
        Ok(id)
    }

    /// Evaluates run `id` again on a task of its own.
    pub(crate) fn advance_soon(self: &Arc<Self>, id: i64) {
        let i = Arc::clone(self);
        tokio::spawn(async move {
            if let Err(err) = i.advance_run(id).await {
                tracing::warn!("workflow run {id}: {err:#}");
                let _ended = i.life().end_workflow_run(id, "failed", "", &format!("{err:#}"));
                i.config.on_change.fire();
            }
        });
    }

    fn run_lock(&self, id: i64) -> Arc<tokio::sync::Mutex<()>> {
        Arc::clone(self.workflow_locks.lock().unwrap_or_else(PoisonError::into_inner).entry(id).or_default())
    }

    /// Evaluates run `id` against its journal and issues each call it makes that is not recorded yet, again while
    /// calls settle at once; then says where it stands.
    async fn advance_run(self: &Arc<Self>, id: i64) -> Result<()> {
        let lock = self.run_lock(id);
        let _held = lock.lock().await;
        for pass in 0..MAX_PASSES {
            let run = self.life().workflow_run(id)?;
            if !matches!(run.status.as_str(), "running" | "waiting") {
                return Ok(());
            }
            let (script, args) = self.life().workflow_source(id)?;
            let args = if args.is_empty() { None } else { Some(ForeignJson::parse(&args)?) };
            let journal: Vec<Entry> =
                self.life().workflow_journal(id)?.into_iter().map(entry_of).collect::<Result<_>>()?;
            let out = tokio::task::spawn_blocking(move || workflow::advance(&script, args.as_ref(), &journal)).await?;
            let mut run = run;
            if pass == 0 && (!out.meta.name.is_empty() || !out.meta.description.is_empty()) {
                let named = if run.name == "inline" { out.meta.name.as_str() } else { "" };
                self.life().set_workflow_meta(id, named, &out.meta.description)?;
                if !named.is_empty() {
                    run.name = named.to_owned();
                }
            }
            if out.state == ScriptState::Failed {
                self.life().end_workflow_run(id, "failed", "", &out.why())?;
                break;
            }
            let mut again = false;
            for call in &out.calls {
                let due = if call.verb == WorkflowVerb::Wait {
                    match serde_json::from_value::<WaitCall>(call.input.0.clone()).map_err(anyhow::Error::from).and_then(|w| due_of(&w, Local::now().naive_local())) {
                        Ok(at) => at.format(DUE).to_string(),
                        Err(err) => {
                            let added = self.life().add_workflow_step(id, call.seq, call.verb.as_str(), &call.label, &call.phase, &call.input.text(), &call.hash, "")?;
                            if added {
                                self.life().settle_workflow_step(id, call.seq, Err(format!("{err:#}")))?;
                                again = true;
                            }
                            continue;
                        }
                    }
                } else {
                    String::new()
                };
                let added = self.life().add_workflow_step(
                    id,
                    call.seq,
                    call.verb.as_str(),
                    &call.label,
                    &call.phase,
                    &call.input.text(),
                    &call.hash,
                    &due,
                )?;
                if !added {
                    continue;
                }
                again |= self.issue(id, &run.name, call.seq, call.verb, &call.input.0).await?;
            }
            if out.state == ScriptState::Done {
                let result = out.result.as_ref().map(ForeignJson::text).unwrap_or_default();
                self.life().end_workflow_run(id, "done", &result, "")?;
                break;
            }
            if !again {
                break;
            }
        }
        let pending = self.life().pending_workflow_steps(id)?;
        let working = pending.iter().any(|s| s.verb != "wait" && s.verb != "ask_owner");
        self.life().set_workflow_status(id, if working || pending.is_empty() { "running" } else { "waiting" })?;
        self.config.on_change.fire();
        self.workflow_tick.notify_one();
        Ok(())
    }

    /// Issues step `seq` of run `id`; true when it settled at once.
    async fn issue(self: &Arc<Self>, id: i64, name: &str, seq: i64, verb: WorkflowVerb, input: &Value) -> Result<bool> {
        let settle = |outcome: std::result::Result<String, String>| -> Result<bool> {
            self.life().settle_workflow_step(id, seq, outcome)?;
            Ok(true)
        };
        match verb {
            WorkflowVerb::Log | WorkflowVerb::Phase => settle(Ok(String::new())),
            WorkflowVerb::Wait => {
                self.workflow_tick.notify_one();
                Ok(false)
            }
            WorkflowVerb::Agent => {
                let call: AgentCall = match serde_json::from_value(input.clone()) {
                    Ok(c) => c,
                    Err(err) => return settle(Err(format!("agent()'s input: {err}"))),
                };
                let i = Arc::clone(self);
                let by = Actor { kind: ActorKind::Workflow, id: id.to_string(), name: format!("{name} · step {seq}") };
                tokio::spawn(async move {
                    let outcome = i.agent_step(&call, by).await;
                    let settled = i.life().settle_workflow_step(id, seq, outcome);
                    match settled {
                        Ok(true) => i.advance_soon(id),
                        Ok(false) => {}
                        Err(err) => tracing::warn!("workflow run {id} step {seq}: {err:#}"),
                    }
                });
                Ok(false)
            }
            WorkflowVerb::AskOwner => {
                let question = input.get("question").and_then(Value::as_str).unwrap_or_default();
                let (q, open) = {
                    let life = self.life();
                    let q = life.ask_owner(&format!("{ASKER}{id}:{seq}"), &format!("Workflow {name}"), question)?;
                    (q, life.questions(true)?)
                };
                let lifeos = Actor { kind: ActorKind::Lifeos, id: String::new(), name: "lifeos".into() };
                let agents = Agents { inner: Arc::clone(self) };
                agents.say(ASSISTANT, &crate::messages::decide_text(&q, &open), lifeos, Delivery::Queue).await?;
                Ok(false)
            }
            WorkflowVerb::Hire | WorkflowVerb::Accept | WorkflowVerb::SendBack => settle(Err(format!(
                "{}() is remit's: lifeos hires no one. Ask an agent with agent(prompt, {{agentType}}) instead",
                verb.as_str()
            ))),
            WorkflowVerb::Workflow => settle(Err(
                "a workflow does not run another as its step yet: run_workflow starts it on its own".to_owned()
            )),
        }
    }

    /// What agent() gives: the answer of the agent the call names to its prompt.
    async fn agent_step(self: &Arc<Self>, call: &AgentCall, by: Actor) -> std::result::Result<String, String> {
        let agent = if call.agent_type.trim().is_empty() { ASSISTANT } else { call.agent_type.trim() };
        let mut prompt = call.prompt.clone();
        if let Some(schema) = call.schema.as_ref().filter(|s| !s.is_null()) {
            prompt.push_str(&format!(
                "\n\nEnd your answer with one JSON value that matches this JSON schema:\n```json\n{schema}\n```"
            ));
        }
        let agents = Agents { inner: Arc::clone(self) };
        let answer = agents.ask(agent, &prompt, by).await.map_err(|e| format!("{e:#}"))?;
        if let Some(err) = answer.error {
            return Err(err);
        }
        report_value(&answer.text, call.schema.as_ref())
    }

    /// Gives a question a workflow asked its answer: the step it was asked at settles with it.
    pub(crate) fn answer_workflow(self: &Arc<Self>, asker: &str, answer: &str) -> bool {
        let Some((id, seq)) = asked_by(asker) else { return false };
        let answer = serde_json::to_string(answer).unwrap_or_default();
        match self.life().settle_workflow_step(id, seq, Ok(answer)) {
            Ok(true) => self.advance_soon(id),
            Ok(false) => {}
            Err(err) => tracing::warn!("workflow run {id} step {seq}: {err:#}"),
        }
        true
    }

    /// The Assistant's tools that start and stop workflows.
    pub(crate) fn add_workflow_tools(self: &Arc<Self>, toolbox: &mut Toolbox) {
        let weak: Weak<Inner> = Arc::downgrade(self);
        let schema = |props: Value, required: &[&str]| -> Map<String, Value> {
            match json!({ "type": "object", "properties": props, "required": required }) {
                Value::Object(m) => m,
                _ => Map::new(),
            }
        };
        let w = weak.clone();
        toolbox.add(
            "run_workflow",
            "Start a workflow: a script that puts several agents to work in order or side by side, waits, asks the owner, \
             and returns a result, run durably by lifeos. name runs workflows/<name>.js (list_workflows shows them); \
             script runs one given here. The script is JavaScript: `export const meta = {name, description}` first, \
             then a body that may await: agent(prompt, {agentType: a crew member's id, schema, label}) gives that \
             agent's answer (the Assistant's when no agentType; its last JSON value, checked, with a schema); \
             parallel([() => agent(…), …]); askOwner(question); wait({minutes: 30} | {until: '2026-11-26T07:00'}); \
             phase(title); log(text); return a value. Returns the run's id at once; get_workflow_run shows where it \
             stands.",
            schema(
                json!({
                    "name": { "type": "string", "description": "A script in workflows/, without .js." },
                    "script": { "type": "string", "description": "A script given inline, in place of name." },
                    "args": { "description": "What the script reads as args." }
                }),
                &[],
            ),
            false,
            move |args| {
                let w = w.clone();
                Box::pin(async move {
                    let i = w.upgrade().context("lifeos stopped")?;
                    let name = args.get("name").and_then(Value::as_str);
                    let script = args.get("script").and_then(Value::as_str);
                    let id = i.start_workflow(name, script, args.get("args").cloned(), "the Assistant")?;
                    Ok(format!("Started workflow run {id}. get_workflow_run shows where it stands."))
                })
            },
        );
        toolbox.add(
            "stop_workflow",
            "Stop a workflow run: its steps that wait fail, and its script goes no further.",
            schema(json!({ "id": { "type": "integer" } }), &["id"]),
            false,
            move |args| {
                let w = weak.clone();
                Box::pin(async move {
                    let i = w.upgrade().context("lifeos stopped")?;
                    let id = args.get("id").and_then(Value::as_i64).context("say which run (id)")?;
                    let stopped = Agents { inner: i }.stop_workflow(id)?;
                    Ok(if stopped { format!("Stopped run {id}.") } else { format!("Run {id} had ended.") })
                })
            },
        );
    }
}

/// Resumes the runs lifeos stopped in, and settles waits as they come due, while lifeos runs.
pub(crate) fn keep_workflows(inner: Weak<Inner>) {
    tokio::spawn(async move {
        if let Some(i) = inner.upgrade() {
            let open = i.life().open_workflow_runs().unwrap_or_default();
            for id in open {
                let pending = i.life().pending_workflow_steps(id).unwrap_or_default();
                for s in pending.iter().filter(|s| s.verb == "agent") {
                    let _settled = i.life().settle_workflow_step(id, s.seq, Err("lifeos stopped while the agent worked on it".into()));
                }
                i.advance_soon(id);
            }
        }
        loop {
            let Some(i) = inner.upgrade() else { return };
            let (due, next) = i.life().due_workflow_waits(&now_due()).unwrap_or_default();
            for (id, seq) in due {
                if i.life().settle_workflow_step(id, seq, Ok(String::new())).unwrap_or(false) {
                    i.advance_soon(id);
                }
            }
            let sleep = next
                .and_then(|n| NaiveDateTime::parse_from_str(&n, DUE).ok())
                .and_then(|n| (n - Local::now().naive_local()).to_std().ok())
                .map_or(TICK, |d| d.min(TICK))
                .max(Duration::from_millis(50));
            let tick = Arc::clone(&i.workflow_tick);
            drop(i);
            tokio::select! {
                () = tick.notified() => {}
                () = tokio::time::sleep(sleep) => {}
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_answer_with_a_schema_gives_its_last_json_value_checked() {
        let schema = json!({"type": "object", "required": ["ok"], "properties": {"ok": {"type": "boolean"}}});
        assert_eq!(report_value("Xong.", None).unwrap(), "\"Xong.\"");
        assert_eq!(report_value("Trước {\"ok\": false} rồi {\"ok\": true}", Some(&schema)).unwrap(), "{\"ok\":true}");
        assert!(report_value("{\"ok\": 1}", Some(&schema)).unwrap_err().contains("does not match"));
        assert!(report_value("không có", Some(&schema)).is_err());
    }

    #[test]
    fn a_wait_comes_due_at_its_time() {
        let now = NaiveDateTime::parse_from_str("2026-11-25T08:00:00", DUE).unwrap();
        let at = |w: WaitCall| due_of(&w, now).map(|t| t.format(DUE).to_string());
        assert_eq!(at(WaitCall { minutes: Some(30.0), ..Default::default() }).unwrap(), "2026-11-25T08:30:00");
        assert_eq!(at(WaitCall { until: Some("2026-11-26T07:00".into()), ..Default::default() }).unwrap(), "2026-11-26T07:00:00");
        assert!(at(WaitCall::default()).is_err());
        assert_eq!(asked_by("workflow:3:7"), Some((3, 7)));
        assert_eq!(asked_by("fin"), None);
    }
}
