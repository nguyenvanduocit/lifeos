//! What crosses between the server and LifeOS Computer, the Mac app that lets agents see and work a computer's
//! screen (desktop use). The app dials the link like a page does, opens one stream with [`Call::Computer`] and keeps
//! it open: the server writes [`Reply::ComputerAsk`] frames (each a desktop tool to run), the app writes
//! [`ComputerSays`] frames (each answer, and its status whenever that changes).
//!
//! [`Call::Computer`]: crate::Call::Computer
//! [`Reply::ComputerAsk`]: crate::Reply::ComputerAsk

use serde::{Deserialize, Serialize};
#[cfg(feature = "ts")]
use ts_rs::TS;

/// A computer saying who it is and what it can do, first on its stream and again whenever that changes.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct ComputerHello {
    /// Made once by the app and kept: the same computer under a new name is still the same computer.
    pub id: String,
    /// The computer's name as macOS gives it ("Duoc's MacBook Pro").
    pub name: String,
    /// The app's version.
    pub version: String,
    /// macOS lets the app read and drive other apps (Privacy & Security → Accessibility).
    pub accessibility: bool,
    /// macOS lets the app see the screen (Privacy & Security → Screen Recording).
    pub screen_recording: bool,
    /// The main display, in points.
    pub width: u32,
    pub height: u32,
    /// The owner paused agents on this computer from its menu bar.
    pub paused: bool,
}

/// A desktop tool for the computer to run: `op` is the tool's name, `input` its JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct ComputerAsk {
    /// Names the answer; 0 is a ping the app answers at once.
    pub id: u64,
    pub op: String,
    pub input: String,
}

/// The computer's answer to the ask `id`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct ComputerAnswer {
    pub id: u64,
    /// Whether it did it; when not, `text` says why.
    pub ok: bool,
    pub text: String,
    /// A screenshot, JPEG, base64.
    pub jpeg: Option<String>,
}

/// What the app writes on its stream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub enum ComputerSays {
    Answer(ComputerAnswer),
    Status(ComputerHello),
}

/// A computer as the Integrations page shows it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct ComputerView {
    pub id: String,
    pub name: String,
    pub online: bool,
    pub version: String,
    pub accessibility: bool,
    pub screen_recording: bool,
    pub width: u32,
    pub height: u32,
    pub paused: bool,
    /// When it last connected or went away, RFC 3339.
    pub seen: String,
}
