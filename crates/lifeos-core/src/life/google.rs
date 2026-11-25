//! Where lifeos's Google app keeps the owner's OAuth client and sign-in: a private directory outside the vault,
//! hidden from every agent's session. `lifeos-google` reads and writes the files; the rest of lifeos only asks
//! whether someone is signed in.

use std::path::PathBuf;

use anyhow::Result;

use super::Life;

/// The connection name crew are granted Google by; its tools reach agents as `mcp__google__<tool>`.
pub const CONNECTION: &str = "google";
/// The OAuth client the owner pasted.
pub const CLIENT_FILE: &str = "client.json";
/// The refresh token and the account it belongs to.
pub const TOKEN_FILE: &str = "token.json";

impl Life {
    pub fn google_dir(&self) -> Result<Option<PathBuf>> {
        self.private_config_dir("LIFEOS_GOOGLE_DIR", "google")
    }

    /// The Google account signed in, if any.
    pub fn google_account(&self) -> Result<Option<String>> {
        let Some(dir) = self.google_dir()? else { return Ok(None) };
        let Ok(text) = std::fs::read_to_string(dir.join(TOKEN_FILE)) else { return Ok(None) };
        let saved: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
        Ok(saved.get("email").and_then(|e| e.as_str()).map(String::from))
    }
}
