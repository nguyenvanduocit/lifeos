//! Per-vault MCP configuration. Only the runtime resolves `${NAME}` references, to keys in the Vault
//! (`vault_items`) the agent starting the session may use.
//! Custom servers are configured by the local operator, since stdio launches
//! a process and HTTP sends credentials to a remote destination.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};
use chrono::Local;
use lifeos_wire::{Connection, Connections, Preset};
use rusqlite::{OptionalExtension, params};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

use super::Life;

input! {
    pub struct AddConnection {
        pub preset: String,
    }
}

input! {
    pub struct SetConnection {
        pub name: String,
        pub enabled: bool,
    }
}

input! {
    pub struct ConnectionName {
        pub name: String,
    }
}

/// Configuration accepted from stdin by the local CLI, following the MCP config format.
#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct McpConfig {
    pub url: Option<String>,
    pub command: Option<String>,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub headers: BTreeMap<String, String>,
}

struct SavedConnection {
    name: String,
    preset: Option<String>,
    config: McpConfig,
    enabled: bool,
}

impl SavedConnection {
    fn belongs_to_github(&self) -> bool {
        self.preset.as_deref() == Some("github")
            || self.config.url.as_deref().and_then(|url| url::Url::parse(url).ok())
                .is_some_and(|url| url.scheme() == "https"
                    && url.host_str() == Some("api.githubcopilot.com")
                    && url.path().trim_end_matches('/') == "/mcp")
    }
}

struct PresetDefinition {
    id: &'static str,
    name: &'static str,
    description: &'static str,
    setup: &'static str,
    secrets: &'static [&'static str],
    config: fn() -> McpConfig,
}

/// Claude's own credentials must not be inherited by a stdio MCP server.
pub const PROVIDER_CREDENTIAL_ENV: &[&str] = &[
    "ANTHROPIC_API_KEY",
    "ANTHROPIC_AUTH_TOKEN",
    "CLAUDE_CODE_OAUTH_TOKEN",
];

const PRESETS: &[PresetDefinition] = &[
    PresetDefinition {
        id: "browser",
        name: "Browser",
        description: "Open pages, read content, fill forms, and take screenshots through agent-browser.",
        setup: "Install a version of `agent-browser` with MCP support on the LifeOS PATH. Run `agent-browser install` as the operating-system user that runs LifeOS. Add the connection, then choose which Crew members may use it. The Assistant and those members can access the same LifeOS browser session. [agent-browser documentation](https://github.com/vercel-labs/agent-browser).",
        secrets: &[],
        config: || McpConfig {
            command: Some("agent-browser".into()),
            args: vec!["mcp".into(), "--tools".into(), "core".into()],
            env: BTreeMap::from([("AGENT_BROWSER_SESSION".into(), "lifeos".into())]),
            ..Default::default()
        },
    },
    PresetDefinition {
        id: "x",
        name: "X",
        description: "Access the X API through the official xurl bridge.",
        setup: "Install `xurl` on the LifeOS PATH. Register an X app with callback `http://localhost:8080/callback` and the scopes you need. Register OAuth credentials with `xurl auth apps add`, set the default app with `xurl auth default`, and run `xurl auth oauth2 --headless` as the operating-system user that runs LifeOS. xurl reuses the session in `~/.xurl`. [X MCP documentation](https://docs.x.com/tools/mcp).",
        secrets: &[],
        config: || McpConfig {
            command: Some("xurl".into()),
            args: vec!["mcp".into(), "https://api.x.com/mcp".into()],
            ..Default::default()
        },
    },
];

fn secret_name(name: &str) -> bool {
    super::vault_items::key_name(name)
}

fn connection_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 48
        && name != "lifeos"
        && name != super::google::CONNECTION
        && name != super::desktop::CONNECTION
        && name.as_bytes()[0].is_ascii_lowercase()
        && name
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
}

fn template(value: &str, mut resolve: impl FnMut(&str) -> String) -> Result<String> {
    let mut tail = value;
    let mut result = String::new();
    while let Some((prefix, rest)) = tail.split_once("${") {
        let (name, suffix) = rest.split_once('}').context("unclosed secret reference")?;
        ensure!(secret_name(name), "invalid secret reference name");
        result.push_str(prefix);
        result.push_str(&resolve(name));
        tail = suffix;
    }
    result.push_str(tail);
    Ok(result)
}

impl McpConfig {
    fn validate(&self) -> Result<()> {
        match (&self.url, &self.command) {
            (Some(url), None) => {
                let url = url::Url::parse(url).context("invalid MCP URL")?;
                ensure!(
                    matches!(url.scheme(), "https" | "http") && url.has_host(),
                    "MCP URL must use HTTP or HTTPS"
                );
                ensure!(
                    url.username().is_empty()
                        && url.password().is_none()
                        && url.fragment().is_none(),
                    "put credentials in header secret references"
                );
                ensure!(
                    self.args.is_empty() && self.env.is_empty(),
                    "HTTP connections use headers"
                );
            }
            (None, Some(command)) => {
                ensure!(
                    !command.trim().is_empty() && !command.contains("${"),
                    "provide a literal stdio command"
                );
                ensure!(self.headers.is_empty(), "stdio connections use env");
            }
            _ => anyhow::bail!("provide exactly one of url or command"),
        }
        for value in self.env.values().chain(self.headers.values()) {
            template(value, |_| String::new())?;
        }
        for (key, value) in &self.headers {
            if key.eq_ignore_ascii_case("authorization")
                || key.eq_ignore_ascii_case("cookie")
                || key.eq_ignore_ascii_case("x-api-key")
            {
                ensure!(
                    value.contains("${"),
                    "authentication headers require a secret reference"
                );
            }
        }
        Ok(())
    }

    /// Every secret it names.
    fn referenced(&self) -> Result<Vec<String>> {
        let mut names = BTreeSet::new();
        for value in self.env.values().chain(self.headers.values()) {
            template(value, |name| {
                names.insert(name.to_string());
                String::new()
            })?;
        }
        Ok(names.into_iter().collect())
    }

    fn missing(&self, secrets: &BTreeMap<String, String>) -> Result<Vec<String>> {
        Ok(self.referenced()?.into_iter().filter(|name| !secrets.contains_key(name)).collect())
    }

    fn resolve(&self, secrets: &BTreeMap<String, String>) -> Result<Value> {
        let expand = |fields: &BTreeMap<String, String>| -> Result<BTreeMap<String, String>> {
            fields
                .iter()
                .map(|(key, value)| {
                    Ok((
                        key.clone(),
                        template(value, |name| secrets.get(name).cloned().unwrap_or_default())?,
                    ))
                })
                .collect()
        };
        Ok(match &self.url {
            Some(url) => json!({ "type": "http", "url": url, "headers": expand(&self.headers)? }),
            None => {
                let mut env = expand(&self.env)?;
                for key in PROVIDER_CREDENTIAL_ENV {
                    env.entry((*key).into()).or_default();
                }
                json!({ "type": "stdio", "command": self.command, "args": self.args, "env": env })
            }
        })
    }

    fn target(&self) -> String {
        if let Some(url) = &self.url {
            return url::Url::parse(url)
                .map(|url| format!("{}{}", url.origin().ascii_serialization(), url.path()))
                .unwrap_or_else(|_| "Invalid URL".into());
        }
        self.command
            .as_deref()
            .and_then(|c| Path::new(c).file_name())
            .and_then(|c| c.to_str())
            .unwrap_or("stdio")
            .into()
    }
}

impl Life {
    /// What no agent's session may read or change: lifeos's own state in the vault (its database), the secrets of
    /// the owner's connections and the Google sign-in. The jail hides them from every session.
    pub fn private_paths(&self) -> Result<Vec<std::path::PathBuf>> {
        let mut paths = vec![self.home().canonicalize()?.join(".lifeos")];
        paths.extend(self.secrets_dir()?);
        paths.extend(self.google_dir()?);
        Ok(paths)
    }

    pub(crate) fn private_config_dir(&self, env: &str, folder: &str) -> Result<Option<PathBuf>> {
        let base = std::env::var_os(env)
            .map(PathBuf::from)
            .or_else(|| dirs::config_dir().map(|dir| dir.join("lifeos").join(folder)));
        let Some(base) = base else { return Ok(None) };
        ensure!(base.is_absolute(), "{env} must be absolute");
        let home = self.home().canonicalize()?;
        let vault_id = Sha256::digest(home.as_os_str().as_encoded_bytes());
        let dir = base.join(
            vault_id
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>(),
        );
        // Check through the nearest existing ancestor before creating anything.
        let ancestor = dir
            .ancestors()
            .find(|p| p.symlink_metadata().is_ok())
            .context("no secrets directory ancestor")?;
        let resolved = ancestor.canonicalize()?.join(dir.strip_prefix(ancestor)?);
        ensure!(
            !resolved.starts_with(&home),
            "{folder} directory must be outside the vault"
        );
        Ok(Some(dir))
    }

    /// Where the Vault keeps passwords' and keys' values (`vault_items`).
    pub(super) fn secrets_dir(&self) -> Result<Option<PathBuf>> {
        #[cfg(test)]
        {
            // Tests never write to the owner's own directory: one folder per test vault, under the temp dir.
            let home = self.home().canonicalize()?;
            let id = Sha256::digest(home.as_os_str().as_encoded_bytes());
            return Ok(Some(std::env::temp_dir().join("lifeos-test-secrets").join(id.iter().map(|b| format!("{b:02x}")).collect::<String>())));
        }
        #[allow(unreachable_code)]
        self.private_config_dir("LIFEOS_SECRETS_DIR", "secrets")
    }

    pub fn add_connection(&self, preset: &str) -> Result<Connection> {
        let preset = PRESETS
            .iter()
            .find(|p| p.id == preset)
            .context("unknown connection preset")?;
        let config = (preset.config)();
        self.save_connection(preset.id, Some(preset.id), &config, false)?;
        self.connections()?
            .connections
            .into_iter()
            .find(|c| c.name == preset.id)
            .context("connection missing")
    }

    pub fn set_custom_connection(&self, name: &str, config: &McpConfig) -> Result<()> {
        self.save_connection(name, None, config, true)
    }

    fn save_connection(
        &self,
        name: &str,
        preset: Option<&str>,
        config: &McpConfig,
        replace: bool,
    ) -> Result<()> {
        ensure!(
            connection_name(name),
            "use a lowercase MCP name with digits or hyphens; lifeos and google are reserved"
        );
        config.validate()?;
        let changed = self.conn.execute(
            "INSERT INTO connections(name,preset,url,command,args,env,headers,created)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8)
             ON CONFLICT(name) DO UPDATE SET preset=excluded.preset,url=excluded.url,
                command=excluded.command,args=excluded.args,env=excluded.env,headers=excluded.headers
             WHERE ?9",
            params![name, preset, config.url, config.command, serde_json::to_string(&config.args)?,
                serde_json::to_string(&config.env)?, serde_json::to_string(&config.headers)?,
                Local::now().to_rfc3339(), replace],
        )?;
        ensure!(changed != 0, "connection '{name}' already exists");
        Ok(())
    }

    pub fn enable_connection(&self, name: &str, enabled: bool) -> Result<()> {
        ensure!(name != super::google::CONNECTION, "Google is turned on and off by signing in and out on the Google page");
        ensure!(name != super::desktop::CONNECTION, "The desktop is on while a computer runs LifeOS Computer; pause it from that Mac's menu bar");
        let changed = self.conn.execute(
            "UPDATE connections SET enabled=?2 WHERE name=?1",
            params![name, enabled],
        )?;
        ensure!(changed != 0, "connection '{name}' does not exist");
        Ok(())
    }

    pub fn remove_connection(&self, name: &str) -> Result<()> {
        ensure!(name != super::google::CONNECTION, "Google is removed by signing out on the Google page");
        ensure!(name != super::desktop::CONNECTION, "The desktop is built in; take crew's access away instead");
        // Remove grants as well so a later server using this name does not inherit them.
        for member in self
            .crew()?
            .into_iter()
            .filter(|m| m.connections.iter().any(|n| n == name))
        {
            let mut doc = self.find(lifeos_wire::Kind::Crew, &member.id)?;
            doc.meta.connections.retain(|n| n != name);
            self.save(&doc)?;
        }
        let changed = self
            .conn
            .execute("DELETE FROM connections WHERE name=?1", [name])?;
        ensure!(changed != 0, "connection '{name}' does not exist");
        Ok(())
    }

    pub(super) fn check_connections(&self, names: &[String]) -> Result<()> {
        for name in names.iter().filter(|n| *n != super::google::CONNECTION && *n != super::desktop::CONNECTION) {
            let found = self
                .conn
                .query_row(
                    "SELECT 1 FROM connections WHERE name=?1",
                    [name],
                    |_| Ok(()),
                )
                .optional()?;
            ensure!(found.is_some(), "connection '{name}' does not exist");
        }
        Ok(())
    }

    fn saved_connections(&self) -> Result<Vec<SavedConnection>> {
        let mut stmt = self.conn.prepare("SELECT name,preset,url,command,args,env,headers,enabled FROM connections ORDER BY name")?;
        let mut rows = stmt.query([])?;
        let mut saved = Vec::new();
        while let Some(row) = rows.next()? {
            saved.push(SavedConnection {
                name: row.get(0)?,
                preset: row.get(1)?,
                enabled: row.get(7)?,
                config: McpConfig {
                    url: row.get(2)?,
                    command: row.get(3)?,
                    args: serde_json::from_str(&row.get::<_, String>(4)?)?,
                    env: serde_json::from_str(&row.get::<_, String>(5)?)?,
                    headers: serde_json::from_str(&row.get::<_, String>(6)?)?,
                },
            });
        }
        Ok(saved)
    }

    /// Plugin state is shared by web, Telegram and standalone MCP/CLI sessions.
    /// Existing saved connection rows and crew grants survive disabling GitHub.
    pub fn github_plugin_enabled(&self) -> bool {
        let path = self.home().join(".lifeos").join("plugins.json");
        crate::plugins::configs(&path).is_ok_and(|entries| entries.get("github").is_some_and(|p| !p.disabled))
    }

    pub fn github_connection_names(&self) -> Result<Vec<String>> {
        Ok(self.saved_connections()?.into_iter()
            .filter(SavedConnection::belongs_to_github).map(|server| server.name).collect())
    }

    pub fn connections(&self) -> Result<Connections> {
        let secrets: BTreeMap<String, String> =
            self.vault_keys()?.into_iter().map(|k| (k.name, k.value)).collect();
        let crew = self.crew()?;
        let google = self.google_account()?;
        let built_in = Connection {
            name: super::google::CONNECTION.into(),
            preset: Some(super::google::CONNECTION.into()),
            transport: "built-in".into(),
            target: google.clone().unwrap_or_else(|| "Not signed in".into()),
            enabled: google.is_some(),
            secrets: Vec::new(),
            missing: Vec::new(),
            members: crew
                .iter()
                .filter(|m| m.status == "active" && m.connections.iter().any(|c| c == super::google::CONNECTION))
                .map(|m| m.id.clone())
                .collect(),
        };
        let computers = self.computers()?;
        let online = computers.iter().filter(|c| c.online).count();
        let desktop = Connection {
            name: super::desktop::CONNECTION.into(),
            preset: Some(super::desktop::CONNECTION.into()),
            transport: "built-in".into(),
            target: match (computers.len(), online) {
                (0, _) => "No computer yet".into(),
                (n, o) => format!("{o} of {n} computer{} online", if n == 1 { "" } else { "s" }),
            },
            enabled: online > 0,
            secrets: Vec::new(),
            missing: Vec::new(),
            members: crew
                .iter()
                .filter(|m| m.status == "active" && m.connections.iter().any(|c| c == super::desktop::CONNECTION))
                .map(|m| m.id.clone())
                .collect(),
        };
        let github_enabled = self.github_plugin_enabled();
        let connections = [Ok(built_in), Ok(desktop)].into_iter().chain(self
            .saved_connections()?
            .into_iter()
            .filter(|saved| github_enabled || !saved.belongs_to_github())
            .map(|saved| {
                let missing = saved.config.missing(&secrets)?;
                let members = crew
                    .iter()
                    .filter(|m| m.status == "active" && m.connections.contains(&saved.name))
                    .map(|m| m.id.clone())
                    .collect();
                Ok(Connection {
                    name: saved.name,
                    preset: saved.preset,
                    enabled: saved.enabled,
                    secrets: saved.config.referenced()?,
                    missing,
                    members,
                    transport: if saved.config.url.is_some() {
                        "http"
                    } else {
                        "stdio"
                    }
                    .into(),
                    target: saved.config.target(),
                })
            }))
            .collect::<Result<Vec<_>>>()?;
        let presets = PRESETS
            .iter()
            .map(|p| Preset {
                id: p.id.into(),
                name: p.name.into(),
                description: p.description.into(),
                setup: p.setup.into(),
                secrets: p.secrets.iter().map(|s| (*s).into()).collect(),
                added: connections
                    .iter()
                    .any(|c| c.preset.as_deref() == Some(p.id)),
            })
            .collect();
        Ok(Connections { connections, presets })
    }

    /// For each key name, the saved connections that name it.
    pub(super) fn keys_used_in(&self) -> Result<BTreeMap<String, Vec<String>>> {
        let mut used: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for server in self.saved_connections()? {
            for name in server.config.referenced()? {
                used.entry(name).or_default().push(server.name.clone());
            }
        }
        Ok(used)
    }

    /// The effective SDK configuration for `agent`'s session and the values to redact from its output. A server
    /// whose `${NAME}` keys are not all in the Vault, or not all shared with `agent`, stays out; each key a
    /// server takes is one use of it.
    pub fn runtime_connections(
        &self,
        allowed: Option<&[String]>,
        agent: &str,
    ) -> Result<(Map<String, Value>, Vec<String>)> {
        let keys = self.vault_keys()?;
        let usable: BTreeMap<String, String> =
            keys.iter().filter(|k| k.usable_by(agent)).map(|k| (k.name.clone(), k.value.clone())).collect();
        let mut config = Map::new();
        let github_enabled = self.github_plugin_enabled();
        for server in self.saved_connections()? {
            if (github_enabled || !server.belongs_to_github())
                && server.enabled
                && allowed.is_none_or(|names| names.contains(&server.name))
                && server.config.missing(&usable)?.is_empty()
            {
                for name in server.config.referenced()? {
                    if let Some(key) = keys.iter().find(|k| k.name == name) {
                        self.use_vault_item(key.id, "key", Some(agent), &server.name)?;
                    }
                }
                config.insert(server.name.clone(), server.config.resolve(&usable)?);
            }
        }
        let mut redact = self.vault_values()?;
        redact.sort_unstable_by_key(|value| std::cmp::Reverse(value.len()));
        Ok((config, redact))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lifeos_wire::{ASSISTANT, LIBRARIAN};

    fn add_fixture_connection(life: &Life) {
        life.set_custom_connection("github", &McpConfig {
            url: Some("https://example.com/mcp/".into()),
            headers: BTreeMap::from([("Authorization".into(), "Bearer ${GITHUB_TOKEN}".into())]),
            ..Default::default()
        }).unwrap();
    }

    #[test]
    fn github_connections_follow_plugin_state_without_deleting_saved_access() {
        let (_dir, life) = crate::life::test_support::life();
        life.set_custom_connection("my-github", &McpConfig {
            url: Some("https://api.githubcopilot.com/mcp/".into()),
            headers: BTreeMap::from([("Authorization".into(), "Bearer ${GITHUB_TOKEN}".into())]),
            ..Default::default()
        }).unwrap();
        life.set_vault_key("GITHUB_TOKEN", "fixture-only-token").unwrap();
        let config = life.home().join(".lifeos/plugins.json");
        std::fs::create_dir_all(config.parent().unwrap()).unwrap();
        let visible = || life.connections().unwrap().connections.iter().any(|c| c.name == "my-github");
        let usable = || life.runtime_connections(None, ASSISTANT).unwrap().0.contains_key("my-github");
        assert!(!visible() && !usable());
        for disabled in [false, true, false] {
            std::fs::write(&config, json!({
                "github": { "binary": "/fixture/plugin", "sha256": "0".repeat(64), "disabled": disabled }
            }).to_string()).unwrap();
            assert_eq!(visible(), !disabled);
            assert_eq!(usable(), !disabled);
        }
        std::fs::write(&config, json!({
            "github": { "binary": "relative", "sha256": "0".repeat(64) }
        }).to_string()).unwrap();
        assert!(!visible() && !usable());
        assert_eq!(life.github_connection_names().unwrap(), ["my-github"]);
    }

    #[test]
    fn mcp_resolution_keeps_secrets_out_of_the_connection_view() {
        let (_dir, life) = crate::life::test_support::life();
        add_fixture_connection(&life);
        let github = life.connections().unwrap().connections.into_iter()
            .find(|connection| connection.name == "github").unwrap();
        assert_eq!(github.missing, ["GITHUB_TOKEN"]);
        assert!(life.runtime_connections(None, ASSISTANT).unwrap().0.is_empty());
        let saved = life.saved_connections().unwrap();
        let secrets = BTreeMap::from([("GITHUB_TOKEN".into(), "fixture-only-token".into())]);
        let resolved = saved[0].config.resolve(&secrets).unwrap();
        assert_eq!(resolved["type"], "http");
        assert_eq!(
            resolved["headers"]["Authorization"],
            "Bearer fixture-only-token"
        );
        assert!(
            !serde_json::to_string(&life.connections().unwrap())
                .unwrap()
                .contains("fixture-only-token")
        );

        let stdio = McpConfig {
            command: Some("example-mcp".into()),
            args: vec!["serve".into()],
            ..Default::default()
        };
        life.set_custom_connection("local-mcp", &stdio).unwrap();
        let config = life
            .runtime_connections(Some(&["local-mcp".into()]), ASSISTANT)
            .unwrap()
            .0;
        assert_eq!(config["local-mcp"]["type"], "stdio");
        assert!(life.runtime_connections(Some(&[]), ASSISTANT).unwrap().0.is_empty());
        life.enable_connection("local-mcp", false).unwrap();
        assert!(life.runtime_connections(None, ASSISTANT).unwrap().0.is_empty());
    }

    #[test]
    fn a_connection_reaches_only_agents_its_keys_are_shared_with() {
        let (_dir, life) = crate::life::test_support::life();
        add_fixture_connection(&life);
        life.enable_connection("github", true).unwrap();
        life.set_vault_key("GITHUB_TOKEN", "fixture-only-token").unwrap();
        // A new key from the CLI is the Assistant's.
        let (config, redact) = life.runtime_connections(None, ASSISTANT).unwrap();
        assert_eq!(config["github"]["headers"]["Authorization"], "Bearer fixture-only-token");
        assert_eq!(redact, ["fixture-only-token"]);
        assert!(life.runtime_connections(None, "coach").unwrap().0.is_empty());
        assert!(life.runtime_connections(None, LIBRARIAN).unwrap().0.is_empty());
        let view = life.vault_view().unwrap();
        assert_eq!(view.items[0].used_in, ["github"]);
        assert!(view.uses.iter().any(|u| u.action == "key" && u.detail == "github"));
        assert!(life.connections().unwrap().connections.iter().all(|c| c.missing.is_empty()));
    }
}
