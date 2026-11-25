//! What an agent's session holds and did, as its Session panel shows it (remit's `CardContext` and `CardTelemetry`):
//! what lifeos gave the session as it started it, what Claude Code says it loaded, what fills its context as Claude Code
//! counts it (`/context`), and what its model requests and tools did as its session exported them (remit's ADR-0085).
//! Each is read when the owner asks; a part with nothing recorded yet is absent, never filled in.

use serde::{Deserialize, Serialize};
#[cfg(feature = "ts")]
use ts_rs::TS;

/// What one agent has in its context, for the owner to understand it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct CardContext {
    pub card: String,
    /// What lifeos gave the card's session as it started its process; none before lifeos recorded a start of it.
    pub given: Option<ContextGiven>,
    /// The session's newest init; none until the session's first turn reports one.
    pub loaded: Option<InitReport>,
    /// Counted now while the session runs, else the count of when it last went to rest; none before either.
    pub context: Option<ContextCount>,
    /// Why the running session could not count it now; the stored count, when there is one, stands in. Empty when it was
    /// counted now, or no session runs.
    pub context_error: String,
}

/// What lifeos gave a session as it started it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct ContextGiven {
    /// The skills it holds, by name.
    pub skills: Vec<String>,
    /// Its Claude Code tools; none is every tool.
    pub claude_tools: Option<Vec<String>>,
    /// The settings layers it reads.
    pub setting_sources: Vec<String>,
    /// The MCP servers lifeos gave it by name: its own, Google's, the owner's connections.
    pub mcp_servers: Vec<String>,
    /// What lifeos's MCP server listed for it.
    pub tools: Vec<GivenTool>,
    /// Never asked for permission.
    pub allowed_tools: Vec<String>,
    pub deny_rules: Vec<String>,
    /// The session's first message as sent; none until it is sent, or when the session was opened before lifeos
    /// recorded it.
    pub opening: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct GivenTool {
    pub name: String,
    pub description: String,
}

/// A session's newest [`SessionInit`], which replaces the one before, and when lifeos heard it (RFC 3339).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct InitReport {
    pub init: SessionInit,
    pub at: String,
}

/// What Claude Code says a session loaded, from the init message it sends at the start of every turn, as it says it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct SessionInit {
    pub session_id: String,
    /// Claude Code's.
    pub version: String,
    pub model: String,
    pub permission_mode: String,
    pub output_style: String,
    pub tools: Vec<String>,
    pub mcp_servers: Vec<McpServerState>,
    pub skills: Vec<String>,
    pub plugins: Vec<PluginInfo>,
    pub plugin_errors: Vec<PluginError>,
    pub agents: Vec<String>,
    pub slash_commands: Vec<String>,
}

/// An MCP server a session has and how it stands: connected, failed, needs-auth, pending.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct McpServerState {
    pub name: String,
    pub status: String,
    /// Where its definition came from: sdk (lifeos), plugin, or a settings scope.
    pub source: String,
}

/// A Claude Code plugin a session loaded.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct PluginInfo {
    pub name: String,
    pub path: String,
    pub version: String,
}

/// A plugin that did not load, or loaded without a part of it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct PluginError {
    pub plugin: String,
    /// What failed, as Claude Code names it (its `type`).
    pub kind: String,
    pub message: String,
}

/// A count of a session's context and when it was counted (RFC 3339): as the session went to rest, which replaces the
/// count before, or now, while it runs.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct ContextCount {
    pub context: SessionContext,
    pub at: String,
}

/// What fills a session's context, as Claude Code counts it for `/context`. `parts` add up to `total` but for those not
/// sent (`ContextPart::kind`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct SessionContext {
    pub session_id: String,
    pub model: String,
    /// Tokens in the context, the window's size and where the conversation is compacted.
    pub total: u64,
    pub size: u64,
    pub compact_at: u64,
    pub parts: Vec<ContextPart>,
    /// The instruction files read (CLAUDE.md and its kin), `source` saying whose.
    pub memory_files: Vec<ContextItem>,
    pub skills: Vec<ContextItem>,
    pub agents: Vec<ContextItem>,
    /// `source` is the tool's server.
    pub mcp_tools: Vec<ContextItem>,
    /// The conversation's tool calls and their results, by tool.
    pub tool_calls: Vec<ContextItem>,
}

/// One part of a context. `kind`: `used` is sent with every request, `deferred` is listed by name until loaded,
/// `buffer` is kept free for compaction, `free` is left.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct ContextPart {
    pub id: String,
    pub name: String,
    pub tokens: u64,
    pub kind: String,
}

/// One thing in a part of a context and its tokens; a `deferred` one is listed by name until loaded.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct ContextItem {
    pub name: String,
    pub source: String,
    pub tokens: u64,
    pub deferred: bool,
}

/// An agent's telemetry, as its Claude Code sessions exported it. Latencies are milliseconds, zero with no request;
/// times are milliseconds since the Unix epoch.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct CardTelemetry {
    pub requests: u64,
    pub errors: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
    pub cost_usd: f64,
    pub latency_p50_ms: u64,
    pub latency_p95_ms: u64,
    /// Time to the first token.
    pub ttft_p50_ms: u64,
    /// By model and effort as each request reported them, most requests first.
    pub models: Vec<ModelUse>,
    /// By time spent, most first.
    pub tools: Vec<ToolUse>,
    /// Newest first.
    pub recent_errors: Vec<ApiError>,
    pub first_at_ms: i64,
    pub last_at_ms: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct ModelUse {
    pub model: String,
    /// Empty when the requests reported none.
    pub effort: String,
    pub requests: u64,
    pub cost_usd: f64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct ToolUse {
    pub name: String,
    pub runs: u64,
    pub failed: u64,
    pub total_ms: u64,
    pub p95_ms: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct ApiError {
    pub at_ms: i64,
    pub model: String,
    pub status: String,
    pub error: String,
}
