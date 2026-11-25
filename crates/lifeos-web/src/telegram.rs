use super::{HttpState, plugins::Action};
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
};
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tokio::sync::watch;

type Failure = (StatusCode, String);

#[derive(Clone, Default, Serialize, Deserialize)]
pub(crate) struct Config {
    pub installed: bool,
    pub enabled: bool,
    #[serde(default, skip_serializing)]
    token: String,
    pub chats: Vec<i64>,
}

pub(crate) struct Channel {
    life: lifeos_agent::SharedLife,
    path: PathBuf,
    config: watch::Sender<Config>,
    writes: Mutex<()>,
}

impl Channel {
    pub fn load(life: lifeos_agent::SharedLife) -> anyhow::Result<Self> {
        let path = life
            .lock()
            .expect("life lock")
            .home()
            .join(".lifeos/telegram.json");
        let mut config: Config = match std::fs::read(&path) {
            Ok(data) => serde_json::from_slice(&data)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Config::default(),
            Err(e) => return Err(e.into()),
        };
        let migrated = !config.token.is_empty();
        if migrated {
            life.lock()
                .expect("life lock")
                .migrate_integration_key("TELEGRAM_BOT_TOKEN", &config.token)?;
            config.token.clear();
        }
        let channel = Self {
            life,
            path,
            config: watch::channel(config).0,
            writes: Mutex::new(()),
        };
        if migrated {
            channel
                .update(|_| Ok(()))
                .map_err(|(_, e)| anyhow::anyhow!(e))?;
        }
        Ok(channel)
    }

    fn token(&self) -> Option<String> {
        self.life
            .lock()
            .expect("life lock")
            .integration_key("TELEGRAM_BOT_TOKEN", None)
            .ok()
            .flatten()
    }

    pub fn configured(&self) -> bool {
        self.token().is_some()
    }

    pub fn current(&self) -> Config {
        self.config.borrow().clone()
    }

    fn update(&self, edit: impl FnOnce(&mut Config) -> Result<(), Failure>) -> Result<(), Failure> {
        let _lock = self.writes.lock().expect("telegram settings lock");
        let mut config = self.current();
        edit(&mut config)?;
        let persist = || -> anyhow::Result<()> {
            let mut file = tempfile::NamedTempFile::new_in(self.path.parent().unwrap())?;
            serde_json::to_writer(&mut file, &config)?;
            file.persist(&self.path)?;
            Ok(())
        };
        persist().map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Could not save Telegram settings".into(),
            )
        })?;
        self.config.send_replace(config);
        Ok(())
    }

    pub fn change(&self, action: Action) -> Result<(), Failure> {
        self.update(|c| {
            match action {
                Action::Install if !c.installed => {
                    c.installed = true;
                    c.enabled = true;
                }
                Action::Disable if c.installed => c.enabled = false,
                Action::Enable if c.installed => c.enabled = true,
                _ => {
                    return Err((
                        StatusCode::CONFLICT,
                        "Invalid Telegram plugin action".into(),
                    ));
                }
            }
            Ok(())
        })
    }

    pub async fn worker(&self, agents: &lifeos_agent::Agents) {
        let mut updates = self.config.subscribe();
        loop {
            let c = updates.borrow_and_update().clone();
            if c.installed
                && c.enabled
                && let Some(token) = self.token()
            {
                let poll = lifeos_telegram::poll(agents, &token, &c.chats);
                tokio::pin!(poll);
                loop {
                    tokio::select! {
                        _ = updates.changed() => break,
                        result = &mut poll => {
                            if result.is_err() { tracing::warn!("Telegram polling stopped"); }
                            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                            break;
                        },
                        _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => {
                            if self.token().as_ref() != Some(&token) { break; }
                        }
                    }
                }
            } else {
                tokio::select! {
                    _ = updates.changed() => (),
                    _ = tokio::time::sleep(std::time::Duration::from_secs(5)), if c.installed && c.enabled => (),
                }
            }
        }
    }
}

#[derive(Deserialize)]
pub(super) struct Settings {
    pub(super) token: String,
    pub(super) chats: Vec<i64>,
}

pub(super) async fn configure(
    State(state): State<Arc<HttpState>>,
    headers: HeaderMap,
    Json(input): Json<Settings>,
) -> Result<StatusCode, Failure> {
    if headers.get("x-lifeos-plugin").is_none_or(|v| v != "1")
        || headers
            .get("sec-fetch-site")
            .is_some_and(|v| v != "same-origin" && v != "none")
    {
        return Err((StatusCode::FORBIDDEN, "Requires a LifeOS request".into()));
    }
    state.app.telegram.save(input)?;
    Ok(StatusCode::NO_CONTENT)
}

impl Channel {
    pub(super) fn save(&self, input: Settings) -> Result<(), Failure> {
        self.update(|c| {
            if !c.installed || !c.enabled {
                return Err((StatusCode::CONFLICT, "Install Telegram first".into()));
            }
            let token = input.token.trim();
            if !token.is_empty() {
                if !token.split_once(':').is_some_and(|(id, key)| {
                    !id.is_empty()
                        && id.bytes().all(|b| b.is_ascii_digit())
                        && !key.is_empty()
                        && key
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
                }) {
                    return Err((StatusCode::BAD_REQUEST, "Invalid bot token format".into()));
                }
                self.life
                    .lock()
                    .expect("life lock")
                    .set_vault_key("TELEGRAM_BOT_TOKEN", token)
                    .map_err(|_| {
                        (
                            StatusCode::BAD_REQUEST,
                            "Cannot save bot token in Vault".into(),
                        )
                    })?;
            }

            c.chats = input.chats;
            Ok(())
        })
    }
}
