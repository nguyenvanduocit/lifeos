//! The lifeos agent: a Claude Code session (via `claude-code-agent`) whose
//! working folder is the vault and whose custom tools are `lifeos_core::tools`,
//! served in-process.
//!
//! Every interface (CLI, web, Telegram) talks to the model through `LifeAgent`:
//! the Assistant, or one crew member (its own role, instructions, tools and
//! memory).

use std::sync::{Arc, Mutex};
use std::time::Instant;

use anyhow::{Context, Result};
use chrono::Local;
use claude_code_agent::protocol::{PermissionMode, ToolAnnotations, ToolDef};
use claude_code_agent::{
    Agent, AgentEvent, Initialize, SdkMcpServer, SdkTool, Session, SessionOptions, ToolResult,
};
use lifeos_core::life::memory::recall_note;
use lifeos_core::{Life, tools};
use lifeos_wire::{ASSISTANT, ChatDone, CrewMember, Step, ToolCall, ToolOutcome};
use tokio_stream::StreamExt;

pub use claude_code_agent::protocol::Effort;

/// The one `Life` shared by the agent's tools and the interface around it.
pub type SharedLife = Arc<Mutex<Life>>;

const SYSTEM_PROMPT: &str = include_str!("system_prompt.md");

/// Built-in Claude Code tools the agent keeps. No Bash: the agent manages a
/// life, not a computer. File tools are confined to the vault (`cwd`).
const BUILTIN_TOOLS: &[&str] = &["Read", "Glob", "Grep", "Edit", "Write", "WebSearch", "WebFetch"];

/// Every agent keeps its memory, whatever tools it was given.
const MEMORY_TOOLS: &[&str] = &["recall", "remember", "forget"];

/// Memories recalled in front of each message, at most.
const RECALL_PER_MESSAGE: u32 = 5;

/// A tool's output kept for the page, at most (in characters).
const OUTPUT_CHARS: usize = 4000;

#[derive(Debug, Clone, Default)]
pub struct AgentConfig {
    /// Claude Code model alias or id; `None` = the account default.
    pub model: Option<String>,
    pub effort: Option<Effort>,
    /// Continue an earlier conversation by session id.
    pub resume: Option<String>,
    /// Stop a single prompt after this many model turns.
    pub max_turns: Option<u32>,
    /// Called after every `write` tool the agent runs.
    pub on_change: OnChange,
    /// Run as this crew member instead of the Assistant.
    pub member: Option<CrewMember>,
}

/// A callback for "the data changed", e.g. to refresh open dashboards.
#[derive(Clone, Default)]
pub struct OnChange(Option<Arc<dyn Fn() + Send + Sync>>);

impl OnChange {
    pub fn new(f: impl Fn() + Send + Sync + 'static) -> OnChange {
        OnChange(Some(Arc::new(f)))
    }

    pub fn fire(&self) {
        if let Some(f) = &self.0 {
            f();
        }
    }
}

impl std::fmt::Debug for OnChange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(if self.0.is_some() { "OnChange(set)" } else { "OnChange(none)" })
    }
}

/// What the page sends when the user asks for the morning brief.
pub const BRIEF_PROMPT: &str = "Viết bản tin buổi sáng cho hôm nay.";

fn parse_effort(e: &str) -> Option<Effort> {
    Some(match e {
        "low" => Effort::Low,
        "medium" => Effort::Medium,
        "high" => Effort::High,
        "xhigh" => Effort::Xhigh,
        "max" => Effort::Max,
        _ => return None,
    })
}

/// What a crew member is told on top of lifeos's own prompt.
fn member_prompt(m: &CrewMember) -> String {
    format!(
        "\n\n# Bạn là {name}\n\nBạn là một thành viên trong crew của người dùng, không phải Assistant chính.{role}\n\n{instructions}\n\n\
         Memory của bạn là của riêng bạn; bạn cũng thấy những gì Assistant nhớ về người dùng. Việc ngoài phần của bạn thì gợi ý người dùng hỏi Assistant.",
        name = m.name,
        role = m.role.as_deref().map(|r| format!(" Vai trò: {r}.")).unwrap_or_default(),
        instructions = m.instructions.trim(),
    )
}

/// Tool results as text: a string, or the text of content blocks.
fn output_text(content: &serde_json::Value) -> String {
    let text = match content {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(blocks) => blocks
            .iter()
            .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
            .collect::<Vec<_>>()
            .join("\n"),
        other => other.to_string(),
    };
    match text.char_indices().nth(OUTPUT_CHARS) {
        Some((i, _)) => format!("{}…", &text[..i]),
        None => text,
    }
}

/// Stops a running turn from outside `ask`.
#[derive(Clone)]
pub struct Interrupter(Session);

impl Interrupter {
    pub async fn interrupt(&self) -> Result<()> {
        Ok(self.0.interrupt_all().await?)
    }
}

pub struct LifeAgent {
    session: Session,
    agent: Agent,
    life: SharedLife,
    /// `ASSISTANT` or the crew member's id: who holds what it remembers.
    holder: String,
    /// Memories already put in front of a message in this conversation.
    shown: Mutex<Vec<i64>>,
}

impl LifeAgent {
    pub async fn spawn(life: SharedLife, config: AgentConfig) -> Result<LifeAgent> {
        let home = life.lock().expect("life lock").home().to_path_buf();
        let member = config.member.clone();
        let holder = member.as_ref().map(|m| m.id.clone()).unwrap_or_else(|| ASSISTANT.to_string());
        let only: Option<Vec<String>> = member.as_ref().filter(|m| !m.tools.is_empty()).map(|m| {
            m.tools.iter().cloned().chain(MEMORY_TOOLS.iter().map(|t| t.to_string())).collect()
        });
        let server = tool_server(life.clone(), holder.clone(), only.as_deref(), config.on_change.clone());
        let mut allowed: Vec<String> = BUILTIN_TOOLS.iter().map(|t| t.to_string()).collect();
        allowed.extend(server.qualified_tool_names());

        let opts = SessionOptions {
            cwd: Some(home),
            model: member.as_ref().and_then(|m| m.model.clone()).or(config.model),
            effort: member.as_ref().and_then(|m| m.effort.as_deref()).and_then(parse_effort).or(config.effort),
            max_turns: config.max_turns,
            // Never stop to ask: anything not pre-approved is denied.
            permission_mode: Some(PermissionMode::DontAsk),
            tools: Some(BUILTIN_TOOLS.iter().map(|t| t.to_string()).collect()),
            allowed_tools: allowed,
            mcp_servers: vec![server],
            strict_mcp_config: true,
            // Only the vault's own `.claude/` and `CLAUDE.md` apply, not the
            // user's global coding setup.
            setting_sources: Some(vec!["project".into()]),
            resume: config.resume,
            name: Some(format!("lifeos-{holder}")),
            ..Default::default()
        };
        let mut prompt = SYSTEM_PROMPT.to_string();
        if let Some(m) = &member {
            prompt.push_str(&member_prompt(m));
        }
        let init = Initialize { system_prompt: Some(prompt), ..Default::default() };
        let session = Session::spawn(opts, init)
            .await
            .context("cannot start Claude Code (is `claude` installed, logged in and up to date?)")?;
        let agent = Agent::new(session.clone());
        Ok(LifeAgent { session, agent, life, holder, shown: Mutex::new(Vec::new()) })
    }

    /// Sends one message and waits for the full reply, reporting each step.
    /// A turn that fails or is stopped still returns, with `error` set.
    pub async fn ask(&self, message: &str, mut on_step: impl FnMut(Step)) -> Result<ChatDone> {
        let started = Instant::now();
        let now = Local::now().format("%Y-%m-%d %A %H:%M");
        let recalled = {
            let life = self.life.lock().expect("life lock");
            let shown = self.shown.lock().expect("shown lock").clone();
            life.recall_for(&self.holder, message, &shown, RECALL_PER_MESSAGE).unwrap_or_default()
        };
        self.shown.lock().expect("shown lock").extend(recalled.iter().map(|m| m.id));
        let prompt = match recall_note(&recalled) {
            Some(note) => format!("[{now}]\n{note}\n\n{message}"),
            None => format!("[{now}]\n{message}"),
        };

        let mut steps: Vec<Step> = Vec::new();
        let mut push = |step: Step, steps: &mut Vec<Step>| {
            on_step(step.clone());
            match (steps.last_mut(), step) {
                (Some(Step::Text(t)), Step::Text(more)) => t.push_str(&more),
                (_, step) => steps.push(step),
            }
        };
        let mut stream = self.agent.prompt(prompt.as_str()).await?;
        let mut cost_usd = None;
        let mut error = None;
        let mut finished = false;
        while let Some(event) = stream.next().await {
            match event {
                AgentEvent::Text(t) => push(Step::Text(t), &mut steps),
                AgentEvent::Thinking(t) if !t.trim().is_empty() => push(Step::Thinking(t), &mut steps),
                AgentEvent::ToolUse { id, name, input, .. } => push(
                    Step::Tool(ToolCall { id, name: short_tool_name(&name), input: input.to_string() }),
                    &mut steps,
                ),
                AgentEvent::ToolResult { tool_use_id, content, is_error, .. } => push(
                    Step::ToolDone(ToolOutcome { id: tool_use_id, output: output_text(&content), failed: is_error }),
                    &mut steps,
                ),
                AgentEvent::Done(result) => {
                    let has_text = steps.iter().any(|s| matches!(s, Step::Text(_)));
                    if let (false, Some(text)) = (has_text, result.result.filter(|t| !t.is_empty())) {
                        push(Step::Text(text), &mut steps);
                    }
                    cost_usd = result.total_cost_usd;
                    finished = true;
                    break;
                }
                AgentEvent::Error(err) => {
                    error = Some(err.to_string());
                    break;
                }
                _ => {}
            }
        }
        if !finished && error.is_none() {
            error = Some("the agent stopped without finishing its reply".into());
        }
        Ok(ChatDone { steps, ms: started.elapsed().as_millis() as u64, cost_usd, error })
    }

    pub fn interrupter(&self) -> Interrupter {
        Interrupter(self.session.clone())
    }

    pub fn session_id(&self) -> Option<String> {
        self.session.session_id()
    }

    pub async fn close(self) {
        self.session.close().await;
    }
}

/// The words of a finished reply.
pub fn reply_text(done: &ChatDone) -> String {
    let text: String = done
        .steps
        .iter()
        .filter_map(|s| if let Step::Text(t) = s { Some(t.as_str()) } else { None })
        .collect::<Vec<_>>()
        .join("\n\n");
    match (&done.error, text.trim().is_empty()) {
        (Some(err), true) => format!("⚠️ {err}"),
        _ => text,
    }
}

/// `mcp__lifeos__add_task` → `add_task`; built-in tools keep their name.
pub fn short_tool_name(name: &str) -> String {
    name.strip_prefix("mcp__lifeos__").unwrap_or(name).to_string()
}

/// `lifeos_core::tools` as an in-process MCP server named `lifeos`.
fn tool_server(life: SharedLife, holder: String, only: Option<&[String]>, on_change: OnChange) -> SdkMcpServer {
    let specs = tools::specs().into_iter().filter(|s| only.is_none_or(|o| o.iter().any(|t| t == s.name)));
    specs.fold(SdkMcpServer::new("lifeos"), |server, spec| {
        let def = ToolDef {
            name: spec.name.to_string(),
            description: spec.description.to_string(),
            input_schema: Some(spec.input_schema),
            title: None,
            output_schema: None,
            annotations: Some(ToolAnnotations {
                read_only_hint: Some(spec.read_only),
                ..Default::default()
            }),
        };
        let life = life.clone();
        let on_change = on_change.clone();
        let holder = holder.clone();
        let (name, writes) = (spec.name, !spec.read_only);
        server.tool(SdkTool::new(def, move |args| {
            let life = life.clone();
            let on_change = on_change.clone();
            let holder = holder.clone();
            async move {
                let result = tools::call(&mut life.lock().expect("life lock"), &holder, name, args);
                match result {
                    Ok(value) => {
                        if writes {
                            on_change.fire();
                        }
                        ToolResult::text(value.to_string())
                    }
                    Err(err) => ToolResult::error(format!("{err:#}")),
                }
            }
        }))
    })
}
