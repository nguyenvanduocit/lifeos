//! What crosses the wire between the lifeos server and its clients — the one
//! definition every side compiles (Rust) or generates (TypeScript, via ts-rs)
//! from. Never copy one of these types by hand.
//!
//! The link: one WebTransport session per page at [`PATH`]. Each call opens
//! its own bidirectional stream: the client writes one [`Call`] frame and
//! finishes; the server writes exactly one reply, and finishes.
//! [`Call::Watch`] is the exception: its stream stays open and carries a
//! [`Reply::Changed`] whenever the data changes, and every event, delta and
//! standing of the agents as they work.
//!
//! The page only looks and talks. It reads through `read` tools and never
//! changes a life itself: every change is the agent's, asked for in a chat.
//! What the page does write is the owner's own acts on the agents: his
//! message, a pick in a block, an overturn, a mark on a reply, a stop.
//!
//! Frames are a 4-byte big-endian length, then postcard. Postcard is not
//! self-describing, so these types must not use `skip_serializing_if`,
//! `flatten`, untagged or internally tagged enums, or `serde_json::Value`.

pub mod computer;
pub mod frame;
pub mod session;
pub mod stream;
pub mod view;

use serde::{Deserialize, Serialize};
#[cfg(feature = "ts")]
use ts_rs::TS;

pub use computer::*;
pub use frame::{FrameError, Reader, encode};
pub use session::*;
pub use stream::*;
pub use view::*;

/// The path every WebTransport session is opened at.
pub const PATH: &str = "/link";

/// The Assistant's agent id, which is also its card's.
pub const ASSISTANT: &str = "assistant";

/// The librarian's agent id: the one agent that writes memory (remit's ADR-0117), from what the owner taught the others.
pub const LIBRARIAN: &str = "librarian";

/// A request from a client.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub enum Call {
    /// Runs a `read` tool; `input` and the reply are JSON (the types are in
    /// `ReadTools`, generated from `tools!`). Replies `Json`.
    Read { tool: String, input: String },
    /// One agent as its drawer shows it (`card` is the agent's id); replies `Agent`.
    Agent { card: String },
    /// The newest events of an agent's stream, oldest first, or those before
    /// event `before`; replies `Events`.
    Events { card: String, before: Option<i64> },
    /// The owner's message to an agent, with the images put for it, sent as
    /// `delivery` says; replies `Done` once it is queued (what the agent does
    /// arrives on `Watch`).
    Say { card: String, text: String, images: Vec<ImageRef>, delivery: Delivery },
    /// Asks the Assistant to write today's morning brief; like `Say`.
    Brief,
    /// Ends the turn an agent is in; replies `Done`.
    Interrupt { card: String },
    /// Opens a new session for an agent (Start over); replies `Done`.
    Restart { card: String },
    /// Keeps what the owner is typing to an agent; replies `Done`.
    Draft { card: String, text: String, images: Vec<ImageRef> },
    /// Keeps an image's bytes as `id` (an id the page makes), for a draft to name; replies `Done`. An id put again
    /// keeps what it was first put with. Refused over `IMAGE_MAX_BYTES`.
    ImagePut { id: String, image: Image },
    /// The image kept as `id`; replies `Image`.
    ImageGet { id: String },
    /// What fills an agent's context and what it was given; replies `Context`.
    Context { card: String },
    /// What an agent's model requests and tools did, as its sessions exported it; replies `Telemetry`.
    Telemetry { card: String },
    /// The owner picks `picked` (option ids) in the choice `part` of the block event `event` (remit's ADR-0091): the
    /// block holds his pick and its agent is told in one queued message from him, which replaces one from the same block
    /// it has not read yet. Replies `Done`.
    BlockPick { event: i64, part: String, picked: Vec<String> },
    /// The owner overturns the answer given to question `question` with `word`, or answers one left to him: the asker
    /// gets his word and an overturn teaches the Assistant. Replies `Done`.
    Overturn { question: i64, word: String },
    /// The owner marks agent reply `event` (remit's ADR-0093): helpful, not helpful (`words` why, may be empty) or
    /// correct (`words` the correct reply). The mark stays on the reply and teaches its agent. Replies `Done`.
    Teach { event: i64, mark: TurnMark, words: String },
    /// The owner stops agent `card`: its turn ends, the messages waiting for it are dropped, and `reason` stays in its
    /// stream for it to read with the next message. Replies `Done`.
    Stop { card: String, reason: String },
    /// Replies `Changed` now and after every change of a life's data, and
    /// `Event`, `Delta` and `Agent` as agents work, until the session ends.
    Watch,
    /// LifeOS Computer offering its screen to the agents (see [`computer`]). Its stream stays open both ways: the
    /// server writes `ComputerAsk` replies, the app writes `ComputerSays` frames, until either goes away.
    Computer(ComputerHello),
}

/// What the server answers on a call's stream.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub enum Reply {
    Json(String),
    Agent(AgentView),
    Events(Vec<EventRead>),
    Context(CardContext),
    Telemetry(CardTelemetry),
    Image(Image),
    /// An event added to a stream, or changed.
    Event(EventRead),
    /// A chunk of a growing message or thought.
    Delta(EventDelta),
    /// The data changed; read again. Carries a counter that only grows.
    Changed(u64),
    Done,
    Failed(String),
    /// A desktop tool for a computer to run, on a `Computer` stream.
    ComputerAsk(ComputerAsk),
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
