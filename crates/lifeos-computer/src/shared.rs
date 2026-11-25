//! What the menu bar and the link share: the settings kept on disk, and how the link is doing.

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, PoisonError};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::sync::Notify;

/// The server this app is built for; the owner changes it from the menu bar.
pub const DEFAULT_SERVER: &str = "https://lifeos.aiocean.dev";

/// What the app keeps between launches.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Made on first launch and kept: the server knows this computer by it.
    pub id: String,
    pub server: String,
    #[serde(default)]
    pub paused: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    Connecting,
    Connected,
    Failed(String),
}

pub struct Shared {
    state: Mutex<(Config, Status)>,
    path: PathBuf,
    /// Rung when the server changes, so the link dials the new one now.
    pub redial: Notify,
    /// Rung when the owner pauses or resumes, so the server hears it now.
    pub changed: Notify,
}

impl Shared {
    pub fn load() -> Result<Shared> {
        let dir = std::env::var_os("HOME")
            .map(PathBuf::from)
            .context("no HOME")?
            .join("Library/Application Support/LifeOS Computer");
        std::fs::create_dir_all(&dir)?;
        let path = dir.join("config.json");
        let config = match std::fs::read_to_string(&path) {
            Ok(text) => serde_json::from_str(&text).context("config.json")?,
            Err(_) => {
                let mut bytes = [0_u8; 12];
                getrandom::fill(&mut bytes).map_err(|e| anyhow::anyhow!("{e}"))?;
                let id = format!("c_{}", bytes.iter().map(|b| format!("{b:02x}")).collect::<String>());
                Config { id, server: DEFAULT_SERVER.into(), paused: false }
            }
        };
        let shared = Shared { state: Mutex::new((config, Status::Connecting)), path, redial: Notify::new(), changed: Notify::new() };
        shared.save()?;
        Ok(shared)
    }

    fn lock(&self) -> MutexGuard<'_, (Config, Status)> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn save(&self) -> Result<()> {
        let text = serde_json::to_string_pretty(&self.lock().0)?;
        std::fs::write(&self.path, text)?;
        Ok(())
    }

    pub fn config(&self) -> Config {
        self.lock().0.clone()
    }

    pub fn status(&self) -> Status {
        self.lock().1.clone()
    }

    pub fn set_status(&self, s: Status) {
        self.lock().1 = s;
    }

    pub fn set_server(&self, server: &str) -> Result<()> {
        let server = server.trim().trim_end_matches('/');
        let url = url::Url::parse(server).context("the server is a URL, like https://lifeos.example.com")?;
        anyhow::ensure!(matches!(url.scheme(), "https" | "http"), "the server's URL starts with https://");
        self.lock().0.server = server.to_owned();
        self.save()?;
        self.redial.notify_one();
        Ok(())
    }

    pub fn set_paused(&self, paused: bool) -> Result<()> {
        self.lock().0.paused = paused;
        self.save()?;
        self.changed.notify_one();
        Ok(())
    }
}

/// The computer's name, as macOS shows it in Sharing.
pub fn computer_name() -> String {
    std::process::Command::new("/usr/sbin/scutil")
        .args(["--get", "ComputerName"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Mac".into())
}
