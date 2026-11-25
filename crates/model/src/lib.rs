//! The records an agent's session speaks in, taken from remit's `model` crate: string enums (`text_enum!`), times and
//! foreign JSON as every document holds them, what a session loaded and reports (`session`, `runtime`), and a workflow
//! run's steps. Only what lifeos's agent backbone needs is here; lifeos's own domain records live in `lifeos-wire`.

mod account;
mod event_status;
mod image;
mod invalid;
pub mod runtime;
mod session;
pub mod stored_json;
pub mod tagged;
mod text_enum;
mod workflow;

pub use account::*;
pub use event_status::*;
pub use image::*;
pub use invalid::Invalid;
pub use session::*;
pub use stored_json::{Duration, ForeignJson, Time};
pub use text_enum::UnknownValue;
pub use workflow::*;

// What `text_enum!` and `json_tagged!` expand to names through this crate, so a crate that declares one needs none of
// these as a dependency.
#[doc(hidden)]
pub use {schemars as __schemars, serde as __serde, serde_json as __serde_json};
