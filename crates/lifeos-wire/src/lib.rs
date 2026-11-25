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

/// A request from a client.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub enum Call {
    /// Runs a `read` tool; `input` and the reply are JSON (the types are in
    /// `ReadTools`, generated from `tools!`). Replies `Json`.
    Read { tool: String, input: String },
    /// One message to the agent; replies `Progress`… then `ChatDone`.
    Chat { message: String },
    /// Asks the agent to write today's morning brief; replies `Progress`… then `ChatDone`.
    Brief,
    /// The conversation so far; replies `History`.
    History,
    /// Forget the conversation; replies `Done`.
    ResetChat,
    /// Replies `Changed` now and after every change, until the session ends.
    Watch,
}

/// What the server answers on a call's stream.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub enum Reply {
    Json(String),
    /// The agent is still working; more frames follow.
    Progress(Progress),
    ChatDone(ChatDone),
    History(Vec<Turn>),
    /// The data changed; read again. Carries a counter that only grows.
    Changed(u64),
    Done,
    Failed(String),
}

/// One exchange with the agent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct Turn {
    /// What the user (or the page, for a brief) asked.
    pub ask: String,
    pub reply: ChatDone,
}

/// What the agent is doing while it works on a message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub enum Progress {
    Text(String),
    /// A tool call, by tool name.
    Tool(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct ChatDone {
    pub text: String,
    /// Every tool the agent called for this message, in order.
    pub tools: Vec<String>,
    pub cost_usd: Option<f64>,
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
