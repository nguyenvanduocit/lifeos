//! Remit's plugin protocol, shared by the LifeOS host and out-of-process plugins.
pub mod plugin;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Ack {}
