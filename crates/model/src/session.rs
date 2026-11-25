//! What Claude Code says of a session: what it loaded, what fills its context, its plan.

use serde::{Deserialize, Serialize};

use crate::Time;


#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PlanEntry {
    pub content: String,
    pub priority: String,
    pub status: String,
}

/// The owner's agent of the whole app: one conversation he holds with it from every screen, in no chain. It reads what
/// MCP servers.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SessionInit {
    pub session_id: String,
    /// Claude Code's.
    pub version: String,
    pub model: String,
    pub permission_mode: String,
    pub output_style: String,
    pub tools: Vec<String>,
    #[serde(rename = "mcpServers")]
    pub mcp_servers: Vec<MCPServerState>,
    pub skills: Vec<String>,
    pub plugins: Vec<Plugin>,
    pub plugin_errors: Vec<PluginError>,
    pub agents: Vec<String>,
    pub slash_commands: Vec<String>,
}

/// A card's session's newest [`SessionInit`], which replaces the one before, and when Remit heard it. The store keeps it
/// sent (`ContextPart::kind`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
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

/// before and the store keeps by card, or now, while it runs.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ContextCount {
    pub context: SessionContext,
    pub at: Time,
}

/// One part of a context. `kind`: `used` is sent with every request, `deferred` is listed by name until loaded,
/// `buffer` is kept free for compaction, `free` is left.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ContextPart {
    pub id: String,
    pub name: String,
    pub tokens: u64,
    pub kind: String,
}

/// One thing in a part of a context and its tokens; a `deferred` one is listed by name until loaded.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ContextItem {
    pub name: String,
    pub source: String,
    pub tokens: u64,
    pub deferred: bool,
}

/// An MCP server a session has and how it stands: connected, failed, needs-auth, pending.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MCPServerState {
    pub name: String,
    pub status: String,
    /// Where its definition came from: sdk (Remit), plugin, or a settings scope.
    pub source: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Plugin {
    pub name: String,
    pub path: String,
    pub version: String,
}

/// A plugin that did not load, or loaded without a part of it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PluginError {
    pub plugin: String,
    pub r#type: String,
    pub message: String,
}
