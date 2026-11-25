//! The lifeos agent: a Claude Code session (via `claude-code-agent`) whose
//! working folder is the vault and whose custom tools are `lifeos_core::tools`,
//! served in-process.
//!
//! Every interface (CLI, web, Telegram) talks to the model through `LifeAgent`.

use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use chrono::Local;
use claude_code_agent::protocol::{PermissionMode, ToolAnnotations, ToolDef};
use claude_code_agent::{
    Agent, AgentEvent, Initialize, SdkMcpServer, SdkTool, Session, SessionOptions, ToolResult,
};
use lifeos_core::{Life, tools};
use tokio_stream::StreamExt;

pub use claude_code_agent::protocol::Effort;

/// The one `Life` shared by the agent's tools and the interface around it.
pub type SharedLife = Arc<Mutex<Life>>;

const SYSTEM_PROMPT: &str = include_str!("system_prompt.md");

/// Built-in Claude Code tools the agent keeps. No Bash: the agent manages a
/// life, not a computer. File tools are confined to the vault (`cwd`).
const BUILTIN_TOOLS: &[&str] = &["Read", "Glob", "Grep", "Edit", "Write", "WebSearch", "WebFetch"];

#[derive(Debug, Clone, Default)]
pub struct AgentConfig {
    /// Claude Code model alias or id; `None` = the account default.
    pub model: Option<String>,
    pub effort: Option<Effort>,
    /// Continue an earlier conversation by session id.
    pub resume: Option<String>,
    /// Stop a single prompt after this many model turns.
    pub max_turns: Option<u32>,
}

pub use lifeos_wire::Progress;

#[derive(Debug, Clone)]
pub struct Reply {
    pub text: String,
    pub session_id: Option<String>,
    pub cost_usd: Option<f64>,
}

pub struct LifeAgent {
    session: Session,
    agent: Agent,
}

impl LifeAgent {
    pub async fn spawn(life: SharedLife, config: AgentConfig) -> Result<LifeAgent> {
        let home = life.lock().expect("life lock").home().to_path_buf();
        let server = tool_server(life);
        let mut allowed: Vec<String> = BUILTIN_TOOLS.iter().map(|t| t.to_string()).collect();
        allowed.extend(server.qualified_tool_names());

        let opts = SessionOptions {
            cwd: Some(home),
            model: config.model,
            effort: config.effort,
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
            name: Some("lifeos".into()),
            ..Default::default()
        };
        let init = Initialize {
            system_prompt: Some(SYSTEM_PROMPT.to_string()),
            ..Default::default()
        };
        let session = Session::spawn(opts, init)
            .await
            .context("cannot start Claude Code (is `claude` installed, logged in and up to date?)")?;
        let agent = Agent::new(session.clone());
        Ok(LifeAgent { session, agent })
    }

    /// Sends one message and waits for the full reply, reporting progress.
    pub async fn ask(&self, message: &str, mut on_progress: impl FnMut(Progress)) -> Result<Reply> {
        let now = Local::now().format("%Y-%m-%d %A %H:%M");
        let prompt = format!("[{now}]\n{message}");
        let mut stream = self.agent.prompt(prompt.as_str()).await?;
        let mut text = String::new();
        while let Some(event) = stream.next().await {
            match event {
                AgentEvent::Text(t) => {
                    on_progress(Progress::Text(t.clone()));
                    text.push_str(&t);
                }
                AgentEvent::ToolUse { name, .. } => on_progress(Progress::Tool(name)),
                AgentEvent::Done(result) => {
                    return Ok(Reply {
                        text: if text.is_empty() { result.result.unwrap_or_default() } else { text },
                        session_id: result.session_id,
                        cost_usd: result.total_cost_usd,
                    });
                }
                AgentEvent::Error(err) => return Err(anyhow::anyhow!("{err}")),
                _ => {}
            }
        }
        anyhow::bail!("the agent stopped without finishing its reply")
    }

    pub fn session_id(&self) -> Option<String> {
        self.session.session_id()
    }

    pub async fn close(self) {
        self.session.close().await;
    }
}

/// `lifeos_core::tools` as an in-process MCP server named `lifeos`.
fn tool_server(life: SharedLife) -> SdkMcpServer {
    tools::specs().into_iter().fold(SdkMcpServer::new("lifeos"), |server, spec| {
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
        let name = spec.name;
        server.tool(SdkTool::new(def, move |args| {
            let life = life.clone();
            async move {
                let mut life = life.lock().expect("life lock");
                // Pick up files the model or the user changed by hand.
                if let Err(err) = life.sync_index() {
                    tracing::warn!("index sync failed: {err:#}");
                }
                match tools::call(&mut life, name, args) {
                    Ok(value) => ToolResult::text(value.to_string()),
                    Err(err) => ToolResult::error(format!("{err:#}")),
                }
            }
        }))
    })
}
