//! The Vault (after agentes.chat's ADR 0144): the owner's logins, keys and contact details, each shared with the
//! agents he picks. What an item is and who may use it sits in SQLite (`vault_items`); a password's or a key's value
//! sits in the private secrets directory outside the vault, one file per item, which every agent's jail hides.
//!
//! A saved value goes back to no one: not to an agent, not to the owner's pages. Lifeos alone opens one, to give a
//! key to a connection an agent it is shared with starts, and takes it out of whatever the agent's session shows.
//! Contacts are not secrets: the agents they are shared with read them.

use std::collections::BTreeMap;
use std::fs;
use std::io::{ErrorKind, Write};
use std::path::PathBuf;

use anyhow::{Context, Result, bail, ensure};
use chrono::Local;
use rusqlite::{OptionalExtension, params};

use super::Life;
use super::connections::PROVIDER_CREDENTIAL_ENV;
use lifeos_wire::{ASSISTANT, LIBRARIAN, VaultContact, VaultItem, VaultKind, VaultUse, VaultView};

/// Longest value a password or a key may be.
const VALUE_MAX: usize = 65536;
/// Uses the owner's page reads.
const USES_SHOWN: usize = 200;
/// Names a key may not take: what a process's environment needs, and the model's own credentials.
const RESERVED_KEYS: &[&str] = &["PATH", "HOME", "USER", "SHELL", "TMPDIR", "LANG", "PWD"];

choice! {
    /// What a Vault item is.
    pub enum ItemKind {
        Login => "login",
        Key => "key",
        Contact => "contact",
    }
}

input! {
    /// What the owner saves: a new item (no `id`) or a change to one. A password or a key is given only when it
    /// changes; leave `secret` out to keep the saved one.
    pub struct SaveVaultItem {
        pub id: Option<i64>,
        pub kind: ItemKind,
        pub title: String,
        #[serde(default)]
        pub description: String,
        /// Login: the website.
        pub website: Option<String>,
        /// Login: the username.
        pub username: Option<String>,
        /// Key: its UPPER_SNAKE name.
        pub key_name: Option<String>,
        /// Contact: its details.
        pub contact: Option<ContactInput>,
        /// A new password or key.
        pub secret: Option<String>,
        #[serde(default)]
        pub all_agents: bool,
        #[serde(default)]
        pub agents: Vec<String>,
    }
}

input! {
    pub struct ContactInput {
        #[serde(default)]
        pub name: String,
        #[serde(default)]
        pub company: String,
        #[serde(default)]
        pub tax_id: String,
        #[serde(default)]
        pub email: String,
        #[serde(default)]
        pub phone: String,
        #[serde(default)]
        pub address: String,
    }
}

input! {
    /// Keys from a `.env` file, read in the owner's browser: saved together or not at all.
    pub struct ImportVaultKeys {
        pub keys: Vec<KeyValue>,
        /// Names already saved whose value the new one replaces; the others are left as they are.
        #[serde(default)]
        pub replace: Vec<String>,
        #[serde(default)]
        pub all_agents: bool,
        #[serde(default)]
        pub agents: Vec<String>,
    }
}

input! {
    pub struct KeyValue {
        pub name: String,
        pub value: String,
    }
}

input! {
    /// What an agent looks for in the Vault.
    pub struct ListVault {
        /// Only logins, keys or contacts.
        pub kind: Option<ItemKind>,
        /// Words in the title, description, website, username, key name or contact.
        pub query: Option<String>,
    }
}

/// How many keys an import saved, replaced and left as they were.
#[derive(Debug, Default, serde::Serialize, PartialEq)]
pub struct Imported {
    pub added: usize,
    pub replaced: usize,
    pub kept: usize,
}

/// A key as the connections use it.
pub(super) struct KeyEntry {
    pub id: i64,
    pub name: String,
    pub value: String,
    all_agents: bool,
    agents: Vec<String>,
}

impl KeyEntry {
    pub fn usable_by(&self, agent: &str) -> bool {
        usable(self.all_agents, &self.agents, agent)
    }
}

fn usable(all_agents: bool, agents: &[String], agent: &str) -> bool {
    agent != LIBRARIAN && (all_agents || agents.iter().any(|a| a == agent))
}

pub(super) fn key_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name.as_bytes()[0].is_ascii_uppercase()
        && name.bytes().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'_')
}

fn kind_of(word: &str) -> Result<VaultKind> {
    Ok(match word {
        "login" => VaultKind::Login,
        "key" => VaultKind::Key,
        "contact" => VaultKind::Contact,
        _ => bail!("a Vault item is a login, a key or a contact"),
    })
}

fn wire_kind(kind: ItemKind) -> VaultKind {
    match kind {
        ItemKind::Login => VaultKind::Login,
        ItemKind::Key => VaultKind::Key,
        ItemKind::Contact => VaultKind::Contact,
    }
}

fn kind_word(kind: VaultKind) -> &'static str {
    match kind {
        VaultKind::Login => "login",
        VaultKind::Key => "key",
        VaultKind::Contact => "contact",
    }
}

fn hint(value: &str) -> String {
    let chars: Vec<char> = value.chars().collect();
    chars[chars.len().saturating_sub(4)..].iter().collect()
}

fn check_value(value: &str) -> Result<()> {
    ensure!(
        !value.is_empty() && value.len() <= VALUE_MAX && !value.contains(['\r', '\n', '\0']),
        "a password or a key is one nonempty line"
    );
    Ok(())
}

/// A login's website as `https://host[:port]`, from what the owner typed (`github.com`, a page's address).
fn website(raw: &str) -> Result<String> {
    let raw = raw.trim();
    let with_scheme = if raw.contains("://") { raw.to_owned() } else { format!("https://{raw}") };
    let url = url::Url::parse(&with_scheme).context("the website is not an address")?;
    ensure!(matches!(url.scheme(), "https" | "http") && url.has_host(), "the website is an http or https address");
    Ok(url.origin().ascii_serialization())
}

fn now() -> String {
    Local::now().to_rfc3339()
}

impl Life {
    fn vault_values_dir(&self) -> Result<PathBuf> {
        self.secrets_dir()?.context("set LIFEOS_SECRETS_DIR to a private absolute directory")
    }

    fn value_path(&self, id: i64) -> Result<PathBuf> {
        Ok(self.vault_values_dir()?.join(format!("item-{id}")))
    }

    fn write_value(&self, id: i64, value: &str) -> Result<()> {
        let dir = self.vault_values_dir()?;
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(&dir)?;
        // NamedTempFile is created 0600.
        let mut file = tempfile::NamedTempFile::new_in(&dir)?;
        file.write_all(value.as_bytes())?;
        file.as_file().sync_all()?;
        file.persist(dir.join(format!("item-{id}"))).map_err(|err| err.error)?;
        Ok(())
    }

    fn read_value(&self, id: i64) -> Result<Option<String>> {
        let Some(dir) = self.secrets_dir()? else { return Ok(None) };
        let path = dir.join(format!("item-{id}"));
        let metadata = match path.symlink_metadata() {
            Ok(m) => m,
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(err.into()),
        };
        ensure!(metadata.is_file() && metadata.len() <= VALUE_MAX as u64, "Vault value {id} is not a small file");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            ensure!(metadata.permissions().mode() & 0o077 == 0, "Vault value {id} must be private");
        }
        let value = fs::read_to_string(path)?;
        Ok((!value.is_empty()).then_some(value))
    }

    fn remove_value(&self, id: i64) -> Result<()> {
        match fs::remove_file(self.value_path(id)?) {
            Err(err) if err.kind() != ErrorKind::NotFound => Err(err.into()),
            _ => Ok(()),
        }
    }

    /// Secrets saved before the Vault (one file per name in the secrets directory) become keys every agent may
    /// use, as they could before, and their old files go.
    fn adopt_old_secrets(&self) -> Result<()> {
        let Some(dir) = self.secrets_dir()? else { return Ok(()) };
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(()),
            Err(err) => return Err(err.into()),
        };
        for entry in entries {
            let entry = entry?;
            let Some(name) = entry.file_name().into_string().ok().filter(|n| key_name(n)) else { continue };
            let metadata = entry.path().symlink_metadata()?;
            if !metadata.is_file() || metadata.len() as usize > VALUE_MAX {
                continue;
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                ensure!(metadata.permissions().mode() & 0o077 == 0, "secret file {name} must be private");
            }
            let value = fs::read_to_string(entry.path())?;
            let value = value.trim_end_matches(['\r', '\n']);
            let taken = self
                .conn
                .query_row("SELECT 1 FROM vault_items WHERE key_name=?1", [&name], |_| Ok(()))
                .optional()?
                .is_some();
            if !taken && !value.is_empty() {
                let at = now();
                self.conn.execute(
                    "INSERT INTO vault_items(id, kind,title,key_name,key_hint,all_agents,created,updated)
                 VALUES ((SELECT MAX(COALESCE(MAX(id), 0),
                    COALESCE((SELECT highest FROM retired_row_ids WHERE table_name = 'vault_items'), 0)) + 1 FROM vault_items), 'key',?1,?1,?2,1,?3,?3)",
                    params![name, hint(value), at],
                )?;
                let id = self.conn.last_insert_rowid();
                self.write_value(id, value)?;
                self.use_vault_item(id, "create", None, "kept from before the Vault")?;
            }
            fs::remove_file(entry.path())?;
        }
        Ok(())
    }

    fn agents_valid(&self, agents: &[String]) -> Result<Vec<String>> {
        let crew = self.crew()?;
        let mut out = Vec::new();
        for agent in agents {
            ensure!(
                agent == ASSISTANT || crew.iter().any(|m| &m.id == agent && m.status == "active"),
                "no agent '{agent}'"
            );
            if !out.contains(agent) {
                out.push(agent.clone());
            }
        }
        Ok(out)
    }

    fn vault_rows(&self, filter: &str, args: &[&dyn rusqlite::ToSql]) -> Result<Vec<VaultItem>> {
        let used_in = self.keys_used_in()?;
        let mut stmt = self.conn.prepare(&format!(
            "SELECT id,kind,title,description,website,username,key_name,key_hint,contact,all_agents,agents,last_used,
                    created,updated FROM vault_items WHERE {filter} ORDER BY kind, title COLLATE NOCASE"
        ))?;
        let mut rows = stmt.query(args)?;
        let mut items = Vec::new();
        while let Some(r) = rows.next()? {
            let id: i64 = r.get(0)?;
            let kind = kind_of(&r.get::<_, String>(1)?)?;
            let key_name: Option<String> = r.get(6)?;
            let contact: Option<String> = r.get(8)?;
            let has_secret = kind != VaultKind::Contact && self.value_path(id).is_ok_and(|p| p.is_file());
            items.push(VaultItem {
                id,
                kind,
                title: r.get(2)?,
                description: r.get(3)?,
                website: r.get(4)?,
                username: r.get(5)?,
                used_in: key_name.as_ref().and_then(|k| used_in.get(k)).cloned().unwrap_or_default(),
                key_name,
                key_hint: r.get(7)?,
                has_secret,
                contact: contact.map(|c| serde_json::from_str(&c)).transpose()?,
                all_agents: r.get(9)?,
                agents: serde_json::from_str(&r.get::<_, String>(10)?)?,
                last_used: r.get(11)?,
                created: r.get(12)?,
                updated: r.get(13)?,
            });
        }
        Ok(items)
    }

    /// The Vault as the owner's page reads it: every item and its latest uses, never a value.
    pub fn vault_view(&self) -> Result<VaultView> {
        self.adopt_old_secrets()?;
        let items = self.vault_rows("1", &[])?;
        let mut stmt = self.conn.prepare("SELECT item,action,agent,detail,at FROM vault_uses ORDER BY id DESC LIMIT ?1")?;
        let uses = stmt
            .query_map([USES_SHOWN as i64], |r| {
                Ok(VaultUse { item: r.get(0)?, action: r.get(1)?, agent: r.get(2)?, detail: r.get(3)?, at: r.get(4)? })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(VaultView { items, uses })
    }

    /// What the Vault shares with `agent`: never a password or a key, contact details in full (each read recorded).
    pub fn vault_list(&self, agent: &str, input: &ListVault) -> Result<Vec<VaultItem>> {
        self.adopt_old_secrets()?;
        let kind = input.kind.map(wire_kind);
        let words: Vec<String> =
            input.query.as_deref().unwrap_or("").split_whitespace().map(|w| w.to_lowercase()).collect();
        let items: Vec<VaultItem> = self
            .vault_rows("1", &[])?
            .into_iter()
            .filter(|i| usable(i.all_agents, &i.agents, agent))
            .filter(|i| kind.is_none_or(|k| i.kind == k))
            .filter(|i| {
                let c = i.contact.clone().unwrap_or_default();
                let text = [
                    i.title.as_str(),
                    &i.description,
                    i.website.as_deref().unwrap_or(""),
                    i.username.as_deref().unwrap_or(""),
                    i.key_name.as_deref().unwrap_or(""),
                    &c.name,
                    &c.company,
                    &c.email,
                ]
                .join(" ")
                .to_lowercase();
                words.iter().all(|w| text.contains(w))
            })
            .collect();
        for item in items.iter().filter(|i| i.kind == VaultKind::Contact) {
            self.use_vault_item(item.id, "read", Some(agent), "")?;
        }
        Ok(items)
    }

    /// Writes down one use of item (never a value) and, for an agent's, when it was last used.
    pub(super) fn use_vault_item(&self, item: i64, action: &str, agent: Option<&str>, detail: &str) -> Result<()> {
        let at = now();
        self.conn.execute(
            "INSERT INTO vault_uses(item,action,agent,detail,at) VALUES (?1,?2,?3,?4,?5)",
            params![item, action, agent, detail, at],
        )?;
        if agent.is_some() {
            self.conn.execute("UPDATE vault_items SET last_used=?2 WHERE id=?1", params![item, at])?;
        }
        Ok(())
    }

    /// Saves an item the owner typed: a new one, or a change to one (its kind stays).
    pub fn save_vault_item(&self, input: SaveVaultItem) -> Result<VaultItem> {
        self.adopt_old_secrets()?;
        let kind = wire_kind(input.kind);
        let title = input.title.trim();
        ensure!(!title.is_empty() && title.chars().count() <= 200, "give the item a title (200 characters at most)");
        let description = input.description.trim();
        ensure!(description.chars().count() <= 2000, "the description is 2000 characters at most");
        let agents = self.agents_valid(&input.agents)?;
        if let Some(secret) = &input.secret {
            ensure!(kind != VaultKind::Contact, "a contact has no password");
            check_value(secret)?;
        }
        let (mut site, mut user, mut name, mut contact) = (None, None, None, None);
        match kind {
            VaultKind::Login => {
                site = Some(website(input.website.as_deref().context("a login needs its website")?)?);
                user = input.username.as_deref().map(str::trim).filter(|u| !u.is_empty()).map(str::to_owned);
            }
            VaultKind::Key => {
                let n = input.key_name.as_deref().map(str::trim).context("a key needs its name")?;
                ensure!(key_name(n), "a key's name is UPPER_SNAKE: A–Z, 0–9 and underscore, starting with a letter");
                ensure!(
                    !RESERVED_KEYS.contains(&n) && !PROVIDER_CREDENTIAL_ENV.contains(&n),
                    "{n} is a name lifeos keeps for itself"
                );
                name = Some(n.to_owned());
            }
            VaultKind::Contact => {
                let c = input.contact.context("a contact needs its details")?;
                let c = VaultContact {
                    name: c.name.trim().into(),
                    company: c.company.trim().into(),
                    tax_id: c.tax_id.trim().into(),
                    email: c.email.trim().into(),
                    phone: c.phone.trim().into(),
                    address: c.address.trim().into(),
                };
                ensure!(
                    [&c.name, &c.company, &c.tax_id, &c.email, &c.phone, &c.address].iter().any(|f| !f.is_empty()),
                    "a contact needs at least one detail"
                );
                ensure!(
                    [&c.name, &c.company, &c.tax_id, &c.email, &c.phone, &c.address].iter().all(|f| f.chars().count() <= 500),
                    "a contact's detail is 500 characters at most"
                );
                contact = Some(serde_json::to_string(&c)?);
            }
        }
        let at = now();
        let hint_of = |s: &Option<String>| s.as_deref().filter(|_| kind == VaultKind::Key).map(hint);
        let id = match input.id {
            None => {
                ensure!(kind == VaultKind::Contact || input.secret.is_some(), "type the password or the key to save");
                if let Some(n) = &name {
                    let taken = self.conn.query_row("SELECT 1 FROM vault_items WHERE key_name=?1", [n], |_| Ok(())).optional()?;
                    ensure!(taken.is_none(), "a key named {n} is already in the Vault");
                }
                self.conn.execute(
                    "INSERT INTO vault_items(id, kind,title,description,website,username,key_name,key_hint,contact,all_agents,
                        agents,created,updated)
                 VALUES ((SELECT MAX(COALESCE(MAX(id), 0),
                    COALESCE((SELECT highest FROM retired_row_ids WHERE table_name = 'vault_items'), 0)) + 1 FROM vault_items), ?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?11)",
                    params![kind_word(kind), title, description, site, user, name, hint_of(&input.secret), contact,
                        input.all_agents, serde_json::to_string(&agents)?, at],
                )?;
                let id = self.conn.last_insert_rowid();
                if let Some(secret) = &input.secret {
                    if let Err(err) = self.write_value(id, secret) {
                        self.conn.execute("DELETE FROM vault_items WHERE id=?1", [id])?;
                        return Err(err);
                    }
                }
                self.use_vault_item(id, "create", None, "")?;
                id
            }
            Some(id) => {
                let was: String = self
                    .conn
                    .query_row("SELECT kind FROM vault_items WHERE id=?1", [id], |r| r.get(0))
                    .optional()?
                    .context("no such Vault item")?;
                ensure!(was == kind_word(kind), "an item keeps its kind; save a new one instead");
                if let Some(n) = &name {
                    let taken = self
                        .conn
                        .query_row("SELECT 1 FROM vault_items WHERE key_name=?1 AND id<>?2", params![n, id], |_| Ok(()))
                        .optional()?;
                    ensure!(taken.is_none(), "a key named {n} is already in the Vault");
                }
                if let Some(secret) = &input.secret {
                    self.write_value(id, secret)?;
                }
                self.conn.execute(
                    "UPDATE vault_items SET title=?2,description=?3,website=?4,username=?5,key_name=?6,
                        key_hint=COALESCE(?7,key_hint),contact=?8,all_agents=?9,agents=?10,updated=?11 WHERE id=?1",
                    params![id, title, description, site, user, name, hint_of(&input.secret), contact,
                        input.all_agents, serde_json::to_string(&agents)?, at],
                )?;
                self.use_vault_item(id, "edit", None, if input.secret.is_some() { "new value" } else { "" })?;
                id
            }
        };
        self.vault_changed()?;
        self.vault_rows("id=?1", &[&id])?.pop().context("item missing")
    }

    pub fn remove_vault_item(&self, id: i64) -> Result<()> {
        let changed = self.conn.execute("DELETE FROM vault_items WHERE id=?1", [id])?;
        ensure!(changed != 0, "no such Vault item");
        self.remove_value(id)?;
        self.vault_changed()
    }

    /// Keys from a `.env` file, all saved or none. A key already saved takes the new value only when its name is in
    /// `replace`, and keeps its access and description.
    pub fn import_vault_keys(&self, input: ImportVaultKeys) -> Result<Imported> {
        self.adopt_old_secrets()?;
        ensure!(!input.keys.is_empty() && input.keys.len() <= 200, "import 1 to 200 keys at a time");
        let agents = self.agents_valid(&input.agents)?;
        let mut seen = BTreeMap::new();
        for k in &input.keys {
            ensure!(key_name(&k.name), "{} is not an UPPER_SNAKE name", k.name);
            ensure!(!RESERVED_KEYS.contains(&k.name.as_str()) && !PROVIDER_CREDENTIAL_ENV.contains(&k.name.as_str()),
                "{} is a name lifeos keeps for itself", k.name);
            check_value(&k.value).with_context(|| format!("{} has no single-line value", k.name))?;
            seen.insert(k.name.clone(), k.value.clone());
        }
        let mut done = Imported::default();
        let mut writes = Vec::new();
        let tx = self.conn.unchecked_transaction()?;
        let at = now();
        for (name, value) in &seen {
            let id: Option<i64> =
                tx.query_row("SELECT id FROM vault_items WHERE key_name=?1", [name], |r| r.get(0)).optional()?;
            match id {
                Some(id) if input.replace.contains(name) => {
                    tx.execute("UPDATE vault_items SET key_hint=?2,updated=?3 WHERE id=?1", params![id, hint(value), at])?;
                    writes.push((id, value.clone(), "edit"));
                    done.replaced += 1;
                }
                Some(_) => done.kept += 1,
                None => {
                    tx.execute(
                        "INSERT INTO vault_items(id, kind,title,key_name,key_hint,all_agents,agents,created,updated)
                 VALUES ((SELECT MAX(COALESCE(MAX(id), 0),
                    COALESCE((SELECT highest FROM retired_row_ids WHERE table_name = 'vault_items'), 0)) + 1 FROM vault_items), 'key',?1,?1,?2,?3,?4,?5,?5)",
                        params![name, hint(value), input.all_agents, serde_json::to_string(&agents)?, at],
                    )?;
                    writes.push((tx.last_insert_rowid(), value.clone(), "create"));
                    done.added += 1;
                }
            }
        }
        // Values first: a failed write leaves the database as it was.
        for (id, value, _) in &writes {
            self.write_value(*id, value)?;
        }
        tx.commit()?;
        for (id, _, action) in &writes {
            self.use_vault_item(*id, action, None, "imported from .env")?;
        }
        self.vault_changed()?;
        Ok(done)
    }

    /// `lifeos vault set NAME`: a key's new value, or a new key the Assistant may use.
    pub fn set_vault_key(&self, name: &str, value: &str) -> Result<()> {
        self.adopt_old_secrets()?;
        let id: Option<i64> =
            self.conn.query_row("SELECT id FROM vault_items WHERE key_name=?1", [name], |r| r.get(0)).optional()?;
        self.save_vault_item(SaveVaultItem {
            id,
            kind: ItemKind::Key,
            title: match id {
                Some(id) => self.conn.query_row("SELECT title FROM vault_items WHERE id=?1", [id], |r| r.get(0))?,
                None => name.into(),
            },
            description: match id {
                Some(id) => self.conn.query_row("SELECT description FROM vault_items WHERE id=?1", [id], |r| r.get(0))?,
                None => String::new(),
            },
            website: None,
            username: None,
            key_name: Some(name.into()),
            contact: None,
            secret: Some(value.into()),
            all_agents: match id {
                Some(id) => self.conn.query_row("SELECT all_agents FROM vault_items WHERE id=?1", [id], |r| r.get(0))?,
                None => false,
            },
            agents: match id {
                Some(id) => serde_json::from_str(
                    &self.conn.query_row::<String, _, _>("SELECT agents FROM vault_items WHERE id=?1", [id], |r| r.get(0))?,
                )?,
                None => vec![ASSISTANT.into()],
            },
        })
        .map(|_| ())
    }

    /// `lifeos vault remove NAME`.
    pub fn remove_vault_key(&self, name: &str) -> Result<()> {
        self.adopt_old_secrets()?;
        let id: i64 = self
            .conn
            .query_row("SELECT id FROM vault_items WHERE key_name=?1", [name], |r| r.get(0))
            .optional()?
            .with_context(|| format!("no key named {name} in the Vault"))?;
        self.remove_vault_item(id)
    }

    /// Resolve one named credential for a host integration. Agent calls must satisfy its Vault grant.
    pub fn integration_key(&self, name: &str, agent: Option<&str>) -> Result<Option<String>> {
        let row = self.conn.query_row("SELECT id,all_agents,agents FROM vault_items WHERE kind='key' AND key_name=?1", [name],
            |r| Ok((r.get::<_,i64>(0)?, r.get::<_,bool>(1)?, r.get::<_,String>(2)?))).optional()?;
        let Some((id, all, agents)) = row else { return Ok(None) };
        if let Some(agent) = agent && !usable(all, &serde_json::from_str::<Vec<String>>(&agents)?, agent) {
            return Ok(None);
        }
        self.read_value(id)
    }

    pub fn migrate_integration_key(&self, name: &str, value: &str) -> Result<()> {
        if let Some(existing) = self.integration_key(name, None)? {
            ensure!(existing == value, "Existing Vault value differs for {name}");
        } else {
            self.set_vault_key(name, value)?;
        }
        Ok(())
    }

    /// Every saved key with its value, for the connections lifeos starts.
    pub(super) fn vault_keys(&self) -> Result<Vec<KeyEntry>> {
        self.adopt_old_secrets()?;
        let mut stmt = self.conn.prepare("SELECT id,key_name,all_agents,agents FROM vault_items WHERE kind='key'")?;
        let rows = stmt
            .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, bool>(2)?, r.get::<_, String>(3)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut keys = Vec::new();
        for (id, name, all_agents, agents) in rows {
            if let Some(value) = self.read_value(id)? {
                keys.push(KeyEntry { id, name, value, all_agents, agents: serde_json::from_str(&agents)? });
            }
        }
        Ok(keys)
    }

    /// Every password and key saved, to take out of what agents' sessions show.
    pub fn vault_values(&self) -> Result<Vec<String>> {
        let ids = self
            .conn
            .prepare("SELECT id FROM vault_items WHERE kind<>'contact'")?
            .query_map([], |r| r.get::<_, i64>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut values = Vec::new();
        for id in ids {
            values.extend(self.read_value(id)?);
        }
        Ok(values)
    }

    fn vault_changed(&self) -> Result<()> {
        // The connections an agent's next session starts with read it again.
        self.kv_set("connections.credentials_changed", &now())
    }

    #[cfg(test)]
    pub(super) fn vault_value_for_test(&self, id: i64) -> Option<String> {
        self.read_value(id).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(name: &str, secret: &str, agents: &[&str]) -> SaveVaultItem {
        SaveVaultItem {
            id: None,
            kind: ItemKind::Key,
            title: name.into(),
            description: String::new(),
            website: None,
            username: None,
            key_name: Some(name.into()),
            contact: None,
            secret: Some(secret.into()),
            all_agents: false,
            agents: agents.iter().map(|a| (*a).into()).collect(),
        }
    }

    #[test]
    fn a_saved_value_goes_back_to_no_one() {
        let (_dir, life) = crate::life::test_support::life();
        let item = life.save_vault_item(key("GITHUB_TOKEN", "fixture-token-a1b2", &[ASSISTANT])).unwrap();
        assert_eq!(item.key_hint.as_deref(), Some("a1b2"));
        assert!(item.has_secret);
        let shown = serde_json::to_string(&life.vault_view().unwrap()).unwrap();
        assert!(!shown.contains("fixture-token"));
        let listed = serde_json::to_string(&life.vault_list(ASSISTANT, &ListVault { kind: None, query: None }).unwrap()).unwrap();
        assert!(listed.contains("GITHUB_TOKEN") && !listed.contains("fixture-token"));
        assert_eq!(life.vault_value_for_test(item.id).as_deref(), Some("fixture-token-a1b2"));

        // A change without a value keeps the saved one.
        let mut change = key("GITHUB_TOKEN", "", &[]);
        change.id = Some(item.id);
        change.secret = None;
        change.title = "GitHub".into();
        life.save_vault_item(change).unwrap();
        assert_eq!(life.vault_value_for_test(item.id).as_deref(), Some("fixture-token-a1b2"));
        // ... and no agent now.
        assert!(life.vault_list(ASSISTANT, &ListVault { kind: None, query: None }).unwrap().is_empty());

        life.remove_vault_item(item.id).unwrap();
        assert_eq!(life.vault_value_for_test(item.id), None);
    }

    #[test]
    fn items_go_only_to_the_agents_they_are_shared_with() {
        let (_dir, life) = crate::life::test_support::life();
        life.save_vault_item(SaveVaultItem {
            id: None,
            kind: ItemKind::Contact,
            title: "Home".into(),
            description: String::new(),
            website: None,
            username: None,
            key_name: None,
            contact: Some(ContactInput {
                name: "Owner".into(),
                company: String::new(),
                tax_id: String::new(),
                email: String::new(),
                phone: String::new(),
                address: "Hà Nội".into(),
            }),
            secret: None,
            all_agents: true,
            agents: vec![],
        })
        .unwrap();
        life.save_vault_item(SaveVaultItem {
            website: Some("github.com/login".into()),
            username: Some("owner".into()),
            kind: ItemKind::Login,
            key_name: None,
            ..key("GITHUB", "pw-fixture", &[ASSISTANT])
        })
        .unwrap();
        let all = ListVault { kind: None, query: None };
        assert_eq!(life.vault_list(ASSISTANT, &all).unwrap().len(), 2);
        assert_eq!(life.vault_list(LIBRARIAN, &all).unwrap().len(), 0);
        let login = &life.vault_list(ASSISTANT, &ListVault { kind: Some(ItemKind::Login), query: None }).unwrap()[0];
        assert_eq!(login.website.as_deref(), Some("https://github.com"));
        assert!(!serde_json::to_string(login).unwrap().contains("pw-fixture"));
        // Reading a contact is a use.
        assert!(life.vault_view().unwrap().uses.iter().any(|u| u.action == "read" && u.agent.as_deref() == Some(ASSISTANT)));
        assert!(life.save_vault_item(key("PATH", "x", &[])).is_err());
        assert!(life.save_vault_item(key("lower", "x", &[])).is_err());
        assert!(life.save_vault_item(key("SOME_KEY", "x", &["nobody"])).is_err());
    }

    #[test]
    fn an_env_import_saves_all_or_nothing() {
        let (_dir, life) = crate::life::test_support::life();
        life.save_vault_item(key("A_KEY", "old-a", &[ASSISTANT])).unwrap();
        let bad = ImportVaultKeys {
            keys: vec![KeyValue { name: "B_KEY".into(), value: "b".into() }, KeyValue { name: "bad".into(), value: "c".into() }],
            replace: vec![],
            all_agents: true,
            agents: vec![],
        };
        assert!(life.import_vault_keys(bad).is_err());
        assert_eq!(life.vault_view().unwrap().items.len(), 1);
        let done = life
            .import_vault_keys(ImportVaultKeys {
                keys: vec![KeyValue { name: "A_KEY".into(), value: "new-a".into() }, KeyValue { name: "B_KEY".into(), value: "b".into() }],
                replace: vec![],
                all_agents: true,
                agents: vec![],
            })
            .unwrap();
        assert_eq!(done, Imported { added: 1, replaced: 0, kept: 1 });
        let a = life.vault_view().unwrap().items.into_iter().find(|i| i.key_name.as_deref() == Some("A_KEY")).unwrap();
        assert_eq!(life.vault_value_for_test(a.id).as_deref(), Some("old-a"));
        assert_eq!(a.agents, [ASSISTANT]);
    }
}
