//! What crosses the wire between the lifeos server and its clients — the one
//! definition every side compiles (Rust) or generates (TypeScript, via ts-rs)
//! from. Never copy one of these types by hand.
//!
//! The link: one WebTransport session per page at [`PATH`]. Each call opens
//! its own bidirectional stream: the client writes one [`Call`] frame and
//! finishes; the server writes zero or more [`Reply::Progress`] frames, then
//! exactly one final reply, and finishes. [`Call::Watch`] is the exception:
//! its stream stays open and carries a [`Reply::Changed`] whenever the data
//! changes.
//!
//! The page only looks and talks. It reads through `read` tools and never
//! changes a life itself: every change is the agent's, asked for in a chat.
//!
//! Frames are a 4-byte big-endian length, then postcard. Postcard is not
//! self-describing, so these types must not use `skip_serializing_if`,
//! `flatten`, untagged or internally tagged enums, or `serde_json::Value`.

pub mod frame;
pub mod view;

use serde::{Deserialize, Serialize};
#[cfg(feature = "ts")]
use ts_rs::TS;

pub use frame::{FrameError, Reader, encode};
pub use view::*;

/// The path every WebTransport session is opened at.
pub const PATH: &str = "/link";

/// Who a chat is with: the Assistant, or a crew member by id.
pub const ASSISTANT: &str = "assistant";

/// A request from a client.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub enum Call {
    /// Runs a `read` tool; `input` and the reply are JSON (the types are in
    /// `ReadTools`, generated from `tools!`). Replies `Json`.
    Read { tool: String, input: String },
    /// One message to an agent (`ASSISTANT` or a crew member id); replies
    /// `Progress`… then `ChatDone`.
    Chat { agent: String, message: String },
    /// Asks the Assistant to write today's morning brief; like `Chat`.
    Brief,
    /// Stops what an agent is doing now; replies `Done`.
    Stop { agent: String },
    /// An agent's conversation so far, oldest first; replies `History`.
    History { agent: String },
    /// Starts an agent's conversation over; replies `Done`.
    ResetChat { agent: String },
    /// Replies `Changed` now and after every change, until the session ends.
    Watch,
}

/// What the server answers on a call's stream.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub enum Reply {
    Json(String),
    /// The agent is still working; more frames follow.
    Progress(Step),
    ChatDone(ChatDone),
    History(Vec<Turn>),
    /// The data changed; read again. Carries a counter that only grows.
    Changed(u64),
    Done,
    Failed(String),
}

/// One thing an agent did while answering, in order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub enum Step {
    /// Words to the user (streamed: consecutive `Text` steps join up).
    Text(String),
    /// The model thinking aloud.
    Thinking(String),
    Tool(ToolCall),
    ToolDone(ToolOutcome),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct ToolCall {
    pub id: String,
    /// `add_task` for lifeos tools, `Read`, `WebSearch`… for built-in ones.
    pub name: String,
    /// The input as JSON text.
    pub input: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct ToolOutcome {
    /// The `ToolCall` this answers.
    pub id: String,
    /// What the tool gave back, as text (cut to a few KB).
    pub output: String,
    pub failed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct ChatDone {
    /// Everything the agent did, in order, the final words included.
    pub steps: Vec<Step>,
    /// How long it worked, in milliseconds.
    pub ms: u64,
    pub cost_usd: Option<f64>,
    /// Set when the turn did not finish (stopped, or an error).
    pub error: Option<String>,
}

/// One exchange with an agent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct Turn {
    /// What the user (or lifeos, for a brief) asked.
    pub ask: String,
    /// When it was asked, `YYYY-MM-DD HH:MM`.
    pub at: String,
    pub reply: ChatDone,
}

/// How a page finds the link: served as JSON at `/link.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct LinkInfo {
    pub url: String,
    /// SHA-256 of the server's self-signed certificate, hex.
    pub certificate_hashes: Vec<String>,
    /// RFC 3339.
    pub certificate_expires: String,
}
