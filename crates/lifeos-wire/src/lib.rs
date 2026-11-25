//! What crosses the wire between the lifeos server and its clients — the one
//! definition every side compiles (Rust) or generates (TypeScript, via ts-rs)
//! from. Never copy one of these types by hand.
//!
//! The link: one WebTransport session per page at [`PATH`]. Each call opens
//! its own bidirectional stream: the client writes one [`Call`] frame and
//! finishes; the server writes zero or more [`Reply::Progress`] frames, then
//! exactly one final reply, and finishes.
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
    /// The day's overview; replies `Reply::Today`.
    Today,
    /// One message to the agent; replies `Progress`… then `ChatDone`.
    Chat { message: String },
    /// Forget the conversation; replies `Done`.
    ResetChat,
}

/// What the server answers on a call's stream.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub enum Reply {
    Today(Today),
    /// The agent is still working; more frames follow.
    Progress(Progress),
    ChatDone(ChatDone),
    Done,
    Failed(String),
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
