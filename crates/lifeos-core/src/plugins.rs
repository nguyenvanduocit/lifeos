use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Install {
    pub binary: PathBuf,
    pub sha256: String,
    #[serde(default)]
    pub disabled: bool,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub functions: Vec<String>,
    #[serde(default)]
    pub vault_keys: Vec<String>,
    #[serde(default)]
    pub agents: Vec<plugin_wire::plugin::Agent>,
    #[serde(default)]
    pub automations: Vec<plugin_wire::plugin::Automation>,
    #[serde(default)]
    pub hooks: BTreeMap<String, Hook>,
    #[serde(default)]
    pub events: BTreeMap<String, String>,
    #[serde(default)]
    pub vars: BTreeMap<String, String>,
}

pub fn contribution_id(plugin:&str, local:&str) -> String {format!("plugin.{plugin}.{local}")}

impl Install {
    pub fn agent(&self, plugin:&str, card:&str) -> Option<&plugin_wire::plugin::Agent> {
        self.agents.iter().find(|a| card == contribution_id(plugin,&a.id))
    }
    pub fn resolved_args(&self, registry: &Path) -> Vec<String> {
        self.args
            .iter()
            .map(|arg| match arg.as_str() {
                "{lifeos_db}" => registry.with_file_name("lifeos.db").display().to_string(),
                "{signal_history}" => registry
                    .with_file_name("plugin-events.sqlite")
                    .display()
                    .to_string(),
                _ => arg.clone(),
            })
            .collect()
    }
}

/// String bindings keep the JSON hook API. Request bindings forward the
/// unchanged UTF-8 body and explicitly selected headers for signed webhooks.
#[derive(Clone, Deserialize, Serialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum Hook {
    Json(String),
    Request {
        processor: String,
        headers: Vec<String>,
    },
}

pub fn legacy_credentials(id: &str) -> &'static [&'static str] {
    match id {
        "paypal" => &["PAYPAL_CLIENT_ID", "PAYPAL_CLIENT_SECRET"],
        "github" => &["GITHUB_WEBHOOK_SECRET"],
        "telegram" => &["TELEGRAM_BOT_TOKEN"],
        _ => &[],
    }
}

pub fn runtime_vars(
    home: &Path,
    config: &Install,
    agent: Option<&str>,
) -> anyhow::Result<BTreeMap<String, String>> {
    let mut vars = config.vars.clone();
    let keys = &config.vault_keys;
    if !keys.is_empty() {
        let store = crate::Life::plugin_store(home)?;
        for key in keys {
            vars.remove(key);
            if let Some(value) = store.integration_key(key, agent)? {
                vars.insert(key.to_owned(), value);
            }
        }
    }
    Ok(vars)
}

pub fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
}

pub fn configs(path: &Path) -> anyhow::Result<BTreeMap<String, Install>> {
    let raw = match std::fs::read(path) {
        Ok(raw) => raw,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(e) => return Err(e.into()),
    };
    let configs: BTreeMap<String, Install> = serde_json::from_slice(&raw).map_err(|e| {
        anyhow::anyhow!(
            "Invalid plugins.json at line {}, column {}",
            e.line(),
            e.column()
        )
    })?;
    for (id, config) in &configs {
        anyhow::ensure!(
            identifier(id) && config.binary.is_absolute(),
            "Invalid plugin ID or relative binary path"
        );
        anyhow::ensure!(
            config.sha256.len() == 64 && config.sha256.bytes().all(|c| c.is_ascii_hexdigit()),
            "Invalid SHA-256 for {id}"
        );
        for (name, hook) in &config.hooks {
            anyhow::ensure!(identifier(name), "Invalid hook name for {id}");
            if let Hook::Request { headers, .. } = hook {
                for name in headers {
                    anyhow::ensure!(
                        http::HeaderName::from_lowercase(name.as_bytes()).is_ok(),
                        "Invalid request hook header for {id}"
                    );
                }
            }
        }
    }
    Ok(configs)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_entire_registry_including_disabled_entries() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("plugins.json");
        let mut github = serde_json::json!({"binary":"/opt/github","sha256":"0".repeat(64)});
        std::fs::write(&path, serde_json::json!({"github":github}).to_string()).unwrap();
        assert!(!configs(&path).unwrap()["github"].disabled);
        github["binary"] = "relative".into();
        std::fs::write(&path, serde_json::json!({"github":github}).to_string()).unwrap();
        assert!(configs(&path).is_err());
        github["binary"] = "/opt/github".into();
        github["sha256"] = "bad".into();
        github["disabled"] = true.into();
        std::fs::write(&path, serde_json::json!({"github":github}).to_string()).unwrap();
        assert!(configs(&path).is_err());
    }
}
