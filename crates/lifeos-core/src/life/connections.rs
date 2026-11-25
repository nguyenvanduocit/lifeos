//! Per-vault MCP configuration. Only the runtime resolves secret references.
//! Custom servers are configured by the local operator, since stdio launches
//! a process and HTTP sends credentials to a remote destination.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{ErrorKind, Write};
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

struct PresetDefinition {
    id: &'static str,
    name: &'static str,
    description: &'static str,
    setup: &'static str,
    secrets: &'static [&'static str],
    config: fn() -> McpConfig,
}

const PRESETS: &[PresetDefinition] = &[
    PresetDefinition {
        id: "github",
        name: "GitHub",
        description: "Repositories, issues và pull requests qua MCP chính thức.",
        setup: "Tạo GitHub Personal Access Token với quyền trên các repository cần dùng. Đưa token qua stdin cho `lifeos secret set GITHUB_TOKEN`. [Tài liệu GitHub MCP](https://github.com/github/github-mcp-server/blob/main/docs/server-configuration.md).",
        secrets: &["GITHUB_TOKEN"],
        config: || McpConfig {
            url: Some("https://api.githubcopilot.com/mcp/".into()),
            headers: BTreeMap::from([("Authorization".into(), "Bearer ${GITHUB_TOKEN}".into())]),
            ..Default::default()
        },
    },
    PresetDefinition {
        id: "gmail",
        name: "Gmail",
        description: "Đọc và quản lý thư theo phạm vi OAuth đã cấp cho Gmail MCP.",
        setup: "Bật Gmail API, tạo OAuth client loại Desktop app, rồi lưu JSON thành `~/.gmail-mcp/gcp-oauth.keys.json`. Chạy `npx @artymclabin/gmail-mcp auth --scopes=gmail.readonly` dưới tài khoản hệ điều hành chạy LifeOS. [Tài liệu Gmail MCP](https://github.com/ArtyMcLabin/Gmail-MCP-Server#installation--authentication).",
        secrets: &[],
        config: || McpConfig {
            command: Some("npx".into()),
            args: vec!["-y".into(), "@artymclabin/gmail-mcp".into()],
            ..Default::default()
        },
    },
    PresetDefinition {
        id: "x",
        name: "X",
        description: "Truy cập X API qua cầu nối xurl chính thức.",
        setup: "Cài `xurl` trên PATH của LifeOS. Đăng ký ứng dụng X với callback `http://localhost:8080/callback` và scope theo nhu cầu. Dùng `xurl auth apps add` để đăng ký ứng dụng, rồi `xurl auth oauth2 --headless` dưới tài khoản hệ điều hành chạy LifeOS. xurl dùng lại phiên trong `~/.xurl`. [Tài liệu X MCP](https://docs.x.com/tools/mcp).",
        secrets: &[],
        config: || McpConfig {
            command: Some("xurl".into()),
            args: vec!["mcp".into(), "https://api.x.com/mcp".into()],
            ..Default::default()
        },
    },
];

fn secret_name(name: &str) -> bool {
    !name.is_empty() && name.len() <= 128 && name.bytes().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'_')
}

fn connection_name(name: &str) -> bool {
    !name.is_empty() && name.len() <= 48 && name != "lifeos"
        && name.as_bytes()[0].is_ascii_lowercase()
        && name.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
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
                ensure!(matches!(url.scheme(), "https" | "http") && url.has_host(), "MCP URL must use HTTP or HTTPS");
                ensure!(url.username().is_empty() && url.password().is_none() && url.fragment().is_none(),
                    "put credentials in header secret references");
                ensure!(self.args.is_empty() && self.env.is_empty(), "HTTP connections use headers");
            }
            (None, Some(command)) => {
                ensure!(!command.trim().is_empty() && !command.contains("${"), "provide a literal stdio command");
                ensure!(self.headers.is_empty(), "stdio connections use env");
            }
            _ => anyhow::bail!("provide exactly one of url or command"),
        }
        for value in self.env.values().chain(self.headers.values()) {
            template(value, |_| String::new())?;
        }
        for (key, value) in &self.headers {
            if key.eq_ignore_ascii_case("authorization") || key.eq_ignore_ascii_case("cookie") || key.eq_ignore_ascii_case("x-api-key") {
                ensure!(value.contains("${"), "authentication headers require a secret reference");
            }
        }
        Ok(())
    }

    fn missing(&self, secrets: &BTreeMap<String, String>) -> Result<Vec<String>> {
        let mut missing = BTreeSet::new();
        for value in self.env.values().chain(self.headers.values()) {
            template(value, |name| {
                if !secrets.contains_key(name) { missing.insert(name.to_string()); }
                String::new()
            })?;
        }
        Ok(missing.into_iter().collect())
    }

    fn resolve(&self, secrets: &BTreeMap<String, String>) -> Result<Value> {
        let expand = |fields: &BTreeMap<String, String>| -> Result<BTreeMap<String, String>> {
            fields.iter().map(|(key, value)| Ok((
                key.clone(), template(value, |name| secrets.get(name).cloned().unwrap_or_default())?,
            ))).collect()
        };
        Ok(match &self.url {
            Some(url) => json!({ "type": "http", "url": url, "headers": expand(&self.headers)? }),
            None => json!({ "type": "stdio", "command": self.command, "args": self.args, "env": expand(&self.env)? }),
        })
    }

    fn target(&self) -> String {
        if let Some(url) = &self.url {
            return url::Url::parse(url).map(|url| format!("{}{}", url.origin().ascii_serialization(), url.path()))
                .unwrap_or_else(|_| "URL không hợp lệ".into());
        }
        self.command.as_deref().and_then(|c| Path::new(c).file_name())
            .and_then(|c| c.to_str()).unwrap_or("stdio").into()
    }
}

impl Life {
    fn secrets_dir(&self) -> Result<Option<PathBuf>> {
        let base = std::env::var_os("LIFEOS_SECRETS_DIR").map(PathBuf::from)
            .or_else(|| dirs::config_dir().map(|dir| dir.join("lifeos").join("secrets")));
        let Some(base) = base else { return Ok(None) };
        ensure!(base.is_absolute(), "LIFEOS_SECRETS_DIR must be absolute");
        let home = self.home().canonicalize()?;
        let vault_id = Sha256::digest(home.as_os_str().as_encoded_bytes());
        let dir = base.join(vault_id.iter().map(|b| format!("{b:02x}")).collect::<String>());
        // Check through the nearest existing ancestor before creating anything.
        let ancestor = dir.ancestors().find(|p| p.symlink_metadata().is_ok()).context("no secrets directory ancestor")?;
        let resolved = ancestor.canonicalize()?.join(dir.strip_prefix(ancestor)?);
        ensure!(!resolved.starts_with(&home), "secrets directory must be outside the vault");
        Ok(Some(dir))
    }

    pub fn set_secret(&self, name: &str, value: &str) -> Result<()> {
        ensure!(secret_name(name), "secret names use A–Z, 0–9 and underscore");
        ensure!(!value.is_empty() && value.len() <= 65536 && !value.contains(['\r', '\n', '\0']), "expected a nonempty single-line secret");
        let dir = self.secrets_dir()?.context("set LIFEOS_SECRETS_DIR to a private absolute directory")?;
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(&dir)?;
        let mut file = tempfile::NamedTempFile::new_in(&dir)?;
        file.write_all(value.as_bytes())?;
        file.as_file().sync_all()?;
        file.persist(dir.join(name)).map_err(|err| err.error)?;
        Ok(())
    }

    pub fn remove_secret(&self, name: &str) -> Result<()> {
        ensure!(secret_name(name), "invalid secret name");
        if let Some(dir) = self.secrets_dir()? {
            match fs::remove_file(dir.join(name)) {
                Ok(()) => {}
                Err(err) if err.kind() == ErrorKind::NotFound => {}
                Err(err) => return Err(err.into()),
            }
        }
        Ok(())
    }

    fn load_secrets(&self) -> Result<BTreeMap<String, String>> {
        let mut secrets = BTreeMap::new();
        let Some(dir) = self.secrets_dir()? else { return Ok(secrets) };
        let entries = match fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(secrets),
            Err(err) => return Err(err.into()),
        };
        for entry in entries {
            let entry = entry?;
            let Some(name) = entry.file_name().into_string().ok().filter(|name| secret_name(name)) else { continue };
            let metadata = entry.path().symlink_metadata()?;
            if metadata.is_file() && metadata.len() <= 65536 {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    ensure!(metadata.permissions().mode() & 0o077 == 0, "secret file {name} must be private");
                }
                let value = fs::read_to_string(entry.path())?;
                if !value.is_empty() { secrets.insert(name, value); }
            }
        }
        Ok(secrets)
    }

    pub fn add_connection(&self, preset: &str) -> Result<Connection> {
        let preset = PRESETS.iter().find(|p| p.id == preset).context("unknown connection preset")?;
        let config = (preset.config)();
        self.save_connection(preset.id, Some(preset.id), &config, false)?;
        self.connections()?.connections.into_iter().find(|c| c.name == preset.id).context("connection missing")
    }

    pub fn set_custom_connection(&self, name: &str, config: &McpConfig) -> Result<()> {
        self.save_connection(name, None, config, true)
    }

    fn save_connection(&self, name: &str, preset: Option<&str>, config: &McpConfig, replace: bool) -> Result<()> {
        ensure!(connection_name(name), "use a lowercase MCP name with digits or hyphens; lifeos is reserved");
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
        let changed = self.conn.execute("UPDATE connections SET enabled=?2 WHERE name=?1", params![name, enabled])?;
        ensure!(changed != 0, "connection '{name}' does not exist");
        Ok(())
    }

    pub fn remove_connection(&self, name: &str) -> Result<()> {
        // Remove grants as well so a later server using this name does not inherit them.
        for member in self.crew()?.into_iter().filter(|m| m.connections.iter().any(|n| n == name)) {
            let mut doc = self.find(lifeos_wire::Kind::Crew, &member.id)?;
            doc.meta.connections.retain(|n| n != name);
            self.save(&doc)?;
        }
        let changed = self.conn.execute("DELETE FROM connections WHERE name=?1", [name])?;
        ensure!(changed != 0, "connection '{name}' does not exist");
        Ok(())
    }

    pub(super) fn check_connections(&self, names: &[String]) -> Result<()> {
        for name in names {
            let found = self.conn.query_row("SELECT 1 FROM connections WHERE name=?1", [name], |_| Ok(())).optional()?;
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
                name: row.get(0)?, preset: row.get(1)?, enabled: row.get(7)?,
                config: McpConfig {
                    url: row.get(2)?, command: row.get(3)?,
                    args: serde_json::from_str(&row.get::<_, String>(4)?)?,
                    env: serde_json::from_str(&row.get::<_, String>(5)?)?,
                    headers: serde_json::from_str(&row.get::<_, String>(6)?)?,
                },
            });
        }
        Ok(saved)
    }

    pub fn connections(&self) -> Result<Connections> {
        let secrets = self.load_secrets()?;
        let crew = self.crew()?;
        let connections = self.saved_connections()?.into_iter().map(|saved| {
            let missing = saved.config.missing(&secrets)?;
            let members = crew.iter().filter(|m| m.status == "active" && m.connections.contains(&saved.name))
                .map(|m| m.id.clone()).collect();
            Ok(Connection {
                name: saved.name, preset: saved.preset, enabled: saved.enabled, missing, members,
                transport: if saved.config.url.is_some() { "http" } else { "stdio" }.into(),
                target: saved.config.target(),
            })
        }).collect::<Result<Vec<_>>>()?;
        let presets = PRESETS.iter().map(|p| Preset {
            id: p.id.into(), name: p.name.into(), description: p.description.into(),
            setup: p.setup.into(), secrets: p.secrets.iter().map(|s| (*s).into()).collect(),
            added: connections.iter().any(|c| c.preset.as_deref() == Some(p.id)),
        }).collect();
        Ok(Connections { connections, presets, secrets: secrets.into_keys().collect() })
    }

    /// The effective SDK configuration and values to redact from agent output.
    pub fn runtime_connections(&self, allowed: Option<&[String]>) -> Result<(Map<String, Value>, Vec<String>)> {
        let secrets = self.load_secrets()?;
        let mut config = Map::new();
        for server in self.saved_connections()? {
            if server.enabled && allowed.is_none_or(|names| names.contains(&server.name))
                && server.config.missing(&secrets)?.is_empty() {
                config.insert(server.name, server.config.resolve(&secrets)?);
            }
        }
        let mut redact: Vec<_> = secrets.into_values().collect();
        redact.sort_unstable_by_key(|value| std::cmp::Reverse(value.len()));
        Ok((config, redact))
    }
}
