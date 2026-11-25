//! Claude account selection, shared conversation transcripts, and sign-in
//! flows for the owner. Authorization material never enters the tool surface.

use std::collections::HashMap;
use std::fs;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::Duration;

use anyhow::{Context, Result, bail, ensure};
use chrono::{Datelike, NaiveDate, NaiveTime, TimeZone, Utc};
use agent::{Handler, Launcher as _, Update};
use model::runtime::{Failure, FailureCategory, Limit, LimitStatus, PermissionAnswer, PermissionRequest, Task};
use lifeos_core::life::accounts::DEFAULT_ACCOUNT;
use lifeos_core::life::connections::PROVIDER_CREDENTIAL_ENV;
use lifeos_wire::{AccountSignin, AgentAccount};
use regex::Regex;
use serde::Deserialize;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{ChildStdin, Command};
use tokio::sync::{Notify, mpsc};
use tokio_util::sync::CancellationToken;

use crate::{OnChange, SharedLife};

const LOGIN_LIMIT: Duration = Duration::from_secs(15 * 60);
const PROBE_LIMIT: Duration = Duration::from_secs(35);
const RETRY_AFTER: i64 = 5 * 60;
const PROVIDER_ROUTING_ENV: &[&str] = &[
    "ANTHROPIC_BASE_URL",
    "CLAUDE_CODE_USE_BEDROCK",
    "CLAUDE_CODE_USE_VERTEX",
    "CLAUDE_CODE_USE_FOUNDRY",
];

pub struct AccountPool {
    life: SharedLife,
    changes: OnChange,
    loads: Mutex<HashMap<String, usize>>,
    wake: Notify,
    signin: Mutex<Option<Signin>>,
    checking: tokio::sync::Mutex<()>,
    command: PathBuf,
}

impl std::fmt::Debug for AccountPool {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("AccountPool")
    }
}

struct Signin {
    view: AccountSignin,
    code: mpsc::Sender<String>,
    cancel: CancellationToken,
}

#[derive(Default)]
pub(crate) struct AccountEnvironment {
    pub token: Option<String>,
}

pub(crate) struct Lease {
    pool: Arc<AccountPool>,
    pub account: AgentAccount,
}

impl Drop for Lease {
    fn drop(&mut self) {
        let mut loads = self.pool.loads.lock().expect("account loads lock");
        if let Some(load) = loads.get_mut(&self.account.id) {
            *load -= 1;
            if *load == 0 {
                loads.remove(&self.account.id);
            }
        }
    }
}

#[derive(Default)]
pub(crate) struct AccountObservation {
    pub status: Option<&'static str>,
    pub quota: Option<String>,
    pub utilization: Option<f64>,
    pub resets_at: Option<i64>,
}

impl AccountObservation {
    /// What the provider said of the account's limit at a turn.
    pub fn limit(&mut self, l: &Limit) {
        self.status = Some(match l.status {
            LimitStatus::Allowed => "ready",
            LimitStatus::Warning => "near",
            LimitStatus::Rejected => "limited",
        });
        self.quota = (!l.window.is_empty()
            && l.window.len() <= 40
            && l.window.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'))
        .then(|| l.window.clone());
        self.utilization = l.utilization.filter(|n| (0.0..=1.0).contains(n));
        self.resets_at = l.resets_at.map(|t| t.0.unix_timestamp()).filter(|n| *n > Utc::now().timestamp());
    }

    /// A turn the provider failed.
    pub fn failed(&mut self, f: &Failure) {
        match f.category {
            FailureCategory::Limit => {
                self.status = Some("limited");
                self.resets_at = f.resets_at.map(|t| t.0.unix_timestamp()).or_else(|| reported_reset(&f.title, Utc::now()));
            }
            FailureCategory::Access => {
                self.status = Some("access");
                self.resets_at = None;
            }
            FailureCategory::Service => self.failure(&f.title),
        }
    }

    pub fn failure(&mut self, error: &str) {
        let text = error.to_ascii_lowercase();
        if self.status == Some("limited")
            || [
                "rate limit",
                "rate_limit",
                "billing_error",
                "account_on_hold",
                "usage limit",
                "hit your limit",
                "out of extra usage",
                "out of credits",
                "quota exceeded",
                "429",
                "5-hour limit",
                "weekly limit",
            ]
            .iter()
            .any(|s| text.contains(s))
        {
            self.status = Some("limited");
            self.resets_at = self.resets_at.or_else(|| reported_reset(error, Utc::now()));
        } else if [
            "authentication_error",
            "authentication_failed",
            "oauth_org_not_allowed",
            "verification_required",
            "cloud_credential_error",
            "not logged in",
            "not signed in",
            "invalid api key",
            "please run /login",
            "oauth token has expired",
            "unauthorized",
            "401",
        ]
        .iter()
        .any(|s| text.contains(s))
        {
            self.status = Some("access");
            self.resets_at = None;
        }
    }

    pub fn blocked(&self) -> bool {
        matches!(self.status, Some("access" | "limited"))
    }
}

/// Some CLI versions report the reset only in the provider error text.
fn reported_reset(text: &str, now: chrono::DateTime<Utc>) -> Option<i64> {
    static RESET: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)resets (?:([a-z]{3}) (\d{1,2}),? (?:at )?)?(\d{1,2})(?::(\d{2}))?(am|pm) \(([^)]+)\)")
            .expect("valid provider reset pattern")
    });
    let words = RESET.captures(text)?;
    let zone: chrono_tz::Tz = words.get(6)?.as_str().parse().ok()?;
    let hour: u32 = words.get(3)?.as_str().parse().ok()?;
    if !(1..=12).contains(&hour) {
        return None;
    }
    let minute = words
        .get(4)
        .map_or(Some(0), |word| word.as_str().parse().ok())?;
    let afternoon = words.get(5)?.as_str().eq_ignore_ascii_case("pm");
    let time = NaiveTime::from_hms_opt(hour % 12 + if afternoon { 12 } else { 0 }, minute, 0)?;
    let local = now.with_timezone(&zone);
    let at = |date: NaiveDate| {
        zone.from_local_datetime(&date.and_time(time))
            .earliest()
            .map(|t| t.timestamp())
    };
    if let Some(month) = words.get(1) {
        let number = [
            "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ]
        .iter()
        .position(|name| name.eq_ignore_ascii_case(month.as_str()))? as u32
            + 1;
        let day = words.get(2)?.as_str().parse().ok()?;
        return [local.year(), local.year() + 1]
            .into_iter()
            .filter_map(|year| at(NaiveDate::from_ymd_opt(year, number, day)?))
            .find(|candidate| *candidate > now.timestamp());
    }
    [Some(local.date_naive()), local.date_naive().succ_opt()]
        .into_iter()
        .flatten()
        .filter_map(at)
        .find(|candidate| *candidate > now.timestamp())
}

fn standing(account: &AgentAccount, now: i64) -> u8 {
    match account.status.as_str() {
        "access" | "checking" => 0,
        "limited" => u8::from(
            account
                .resets_at
                .or(account.retry_at)
                .is_some_and(|reset| reset <= now),
        ),
        "near" if account.resets_at.is_none_or(|reset| reset > now) => 2,
        "ready" | "near" => 3,
        _ => 1,
    }
}

impl AccountPool {
    pub fn new(life: SharedLife, changes: OnChange) -> Arc<Self> {
        Arc::new(Self {
            life,
            changes,
            loads: Mutex::new(HashMap::new()),
            wake: Notify::new(),
            signin: Mutex::new(None),
            checking: tokio::sync::Mutex::new(()),
            command: claude_binary(),
        })
    }

    pub fn accounts(&self) -> Result<Vec<AgentAccount>> {
        self.life.lock().expect("life lock").agent_accounts()
    }

    pub(crate) async fn acquire(
        self: &Arc<Self>,
        preferred: Option<&str>,
        stop: &CancellationToken,
        on_wait: impl FnOnce(),
    ) -> Result<Lease> {
        let mut on_wait = Some(on_wait);
        loop {
            ensure!(!stop.is_cancelled(), "Claude turn stopped.");
            let wake = self.wake.notified();
            let accounts = self.accounts()?;
            {
                let now = Utc::now().timestamp();
                let mut loads = self.loads.lock().expect("account loads lock");
                let best = accounts
                    .iter()
                    .filter(|account| standing(account, now) > 0)
                    .min_by_key(|account| {
                        (
                            std::cmp::Reverse(standing(account, now)),
                            loads.get(&account.id).copied().unwrap_or(0),
                            Some(account.id.as_str()) != preferred,
                        )
                    });
                if let Some(account) = best {
                    *loads.entry(account.id.clone()).or_default() += 1;
                    return Ok(Lease {
                        pool: self.clone(),
                        account: account.clone(),
                    });
                }
            }
            if let Some(notify) = on_wait.take() {
                notify();
            }
            tokio::select! {
                _ = stop.cancelled() => bail!("Stopped while waiting for a Claude account."),
                _ = wake => {}
                _ = tokio::time::sleep(Duration::from_secs(3)) => {}
            }
        }
    }

    pub(crate) fn observe(&self, original: &AgentAccount, observation: &AccountObservation) {
        let Some(status) = observation.status else {
            return;
        };
        let mut account = original.clone();
        account.status = status.into();
        account.quota = observation.quota.clone();
        account.utilization = observation.utilization;
        account.resets_at = observation.resets_at;
        account.retry_at = (status == "limited" && account.resets_at.is_none())
            .then(|| Utc::now().timestamp() + RETRY_AFTER);
        account.reason = match status {
            "limited" => Some("Claude reached its usage limit.".into()),
            "access" => Some(
                "Claude rejected authentication. Sign in again or check your credentials.".into(),
            ),
            _ => None,
        };
        self.save_health(&account);
    }

    fn save_health(&self, account: &AgentAccount) {
        if self
            .life
            .lock()
            .expect("life lock")
            .save_agent_account_health(account)
            .is_ok()
        {
            self.changed();
        }
    }

    fn changed(&self) {
        self.wake.notify_waiters();
        self.changes.fire();
    }

    fn default_profile() -> Result<PathBuf> {
        std::env::var_os("CLAUDE_CONFIG_DIR")
            .map(PathBuf::from)
            .or_else(|| dirs::home_dir().map(|path| path.join(".claude")))
            .context("could not find the default Claude configuration directory")
    }

    fn profile(&self, id: &str) -> Result<PathBuf> {
        if id == DEFAULT_ACCOUNT {
            return Self::default_profile();
        }
        ensure!(valid_id(id), "invalid account");
        Ok(self
            .life
            .lock()
            .expect("life lock")
            .account_profiles_dir()?
            .join(id))
    }

    fn prepare(&self, id: &str) -> Result<PathBuf> {
        let profile = self.profile(id)?;
        if id == DEFAULT_ACCOUNT {
            return Ok(profile);
        }
        let projects = Self::default_profile()?.join("projects");
        private_dir(&projects)?;
        private_dir(&profile)?;
        link_projects(&profile, &projects.canonicalize()?)?;
        Ok(profile)
    }

    /// What a session on account needs beside its config dir, which is made ready here: the OAuth token of an account
    /// added by token.
    pub(crate) fn environment(&self, account: &AgentAccount) -> Result<AccountEnvironment> {
        if account.id == DEFAULT_ACCOUNT {
            return Ok(AccountEnvironment::default());
        }
        let dir = self.prepare(&account.id)?;
        let token = if account.kind == "connection" {
            Some(fs::read_to_string(dir.join("oauth-token")).context("missing Claude credentials")?)
        } else {
            None
        };
        Ok(AccountEnvironment { token })
    }

    #[expect(clippy::disallowed_methods, reason = "a short `claude auth` command that ends by itself, killed on drop")]
    fn auth(&self, id: &str, command: &str) -> Result<Command> {
        let mut child = Command::new(&self.command);
        child.args(["auth", command]).kill_on_drop(true);
        child.env("CLAUDE_CONFIG_DIR", self.profile(id)?);
        if id != DEFAULT_ACCOUNT {
            for key in PROVIDER_CREDENTIAL_ENV.iter().chain(PROVIDER_ROUTING_ENV) {
                child.env_remove(key);
            }
        }
        Ok(child)
    }

    async fn identity(&self, id: &str) -> Result<Option<String>> {
        let output =
            tokio::time::timeout(PROBE_LIMIT, self.auth(id, "status")?.arg("--json").output())
                .await
                .map_err(|_| anyhow::anyhow!("The Claude sign-in check timed out."))?
                .map_err(|_| anyhow::anyhow!("Could not start Claude Code."))?;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Status {
            logged_in: bool,
            email: Option<String>,
        }
        let status: Status = serde_json::from_slice(&output.stdout)
            .context("Claude did not return a sign-in status")?;
        Ok(status
            .logged_in
            .then_some(status.email)
            .flatten()
            .filter(|email| email.contains('@') && email.len() < 256))
    }

    pub async fn check(self: &Arc<Self>) -> Result<()> {
        let Ok(_check) = self.checking.try_lock() else {
            return Ok(());
        };
        for account in self.accounts()? {
            self.probe(account).await;
        }
        Ok(())
    }

    async fn probe(self: &Arc<Self>, mut account: AgentAccount) {
        if self
            .loads
            .lock()
            .expect("account loads lock")
            .contains_key(&account.id)
        {
            return; // Its current turn already reports its status.
        }
        // Leave the persisted status intact while checking so a server stop
        // cannot strand an account in a temporary "checking" state.
        if account.id == DEFAULT_ACCOUNT
            && let Ok(Some(email)) = self.identity(DEFAULT_ACCOUNT).await
        {
            let _ = self
                .life
                .lock()
                .expect("life lock")
                .label_default_account(&email);
        }
        let mut observation = AccountObservation::default();
        let probe =
            tokio::time::timeout(PROBE_LIMIT, self.probe_turn(&account, &mut observation)).await;
        match probe {
            Ok(Ok(())) => {
                observation.status.get_or_insert("ready");
                self.observe(&account, &observation);
            }
            Ok(Err(_)) | Err(_) if observation.blocked() => self.observe(&account, &observation),
            _ => {
                account.status = "limited".into();
                account.reason = Some("Could not confirm that Claude is ready.".into());
                account.resets_at = None;
                account.retry_at = Some(Utc::now().timestamp() + RETRY_AFTER);
                self.save_health(&account);
            }
        }
    }

    async fn probe_turn(
        self: &Arc<Self>,
        account: &AgentAccount,
        observation: &mut AccountObservation,
    ) -> Result<()> {
        let env = self.environment(account)?;
        let directory = tempfile::tempdir()?;
        let account_env = match &env.token {
            Some(t) => agent::parse_account_env(std::collections::BTreeMap::from([(
                "CLAUDE_CODE_OAUTH_TOKEN".to_owned(),
                t.clone(),
            )]))?,
            None => agent::AccountEnv::default(),
        };
        let spec = agent::Spec {
            card_id: "account-check".into(),
            cwd: directory.path().display().to_string(),
            account: if account.id == DEFAULT_ACCOUNT { String::new() } else { account.id.clone() },
            account_env,
            mode: "dontAsk".into(),
            tools: Some(Vec::new()),
            builtin_agents: Some(Vec::new()),
            ..agent::Spec::default()
        };
        let heard = Arc::new(Probe::default());
        let launcher = self.launcher(vec![directory.path().join(".none").display().to_string()]);
        let session = launcher.start(spec, heard.clone()).await.map_err(|e| anyhow::anyhow!("{e}"))?;
        let answered = session.prompt(model::Prompt::from("Reply with OK.")).await;
        session.close();
        if let Some(l) = heard.limit.lock().expect("probe lock").take() {
            observation.limit(&l);
        }
        match answered {
            Ok(_) => Ok(()),
            Err(agent::Error::Failure(f)) => {
                observation.failed(&f);
                bail!("Claude did not accept the check")
            }
            Err(e) => {
                observation.failure(&e.to_string());
                bail!("Claude did not accept the check")
            }
        }
    }

    /// Starts sessions with Claude Code, each in its account's config dir (none for the machine's own login), with
    /// `hide` kept from it.
    pub(crate) fn launcher(self: &Arc<Self>, hide: Vec<String>) -> agent::ClaudeLauncher {
        let pool = Arc::downgrade(self);
        agent::ClaudeLauncher {
            command: vec![self.command.display().to_string()],
            home: Some(Arc::new(move |id: &str| {
                if id.is_empty() || id == DEFAULT_ACCOUNT {
                    return std::env::var("CLAUDE_CONFIG_DIR").unwrap_or_default();
                }
                pool.upgrade().and_then(|p| p.profile(id).ok()).map(|d| d.display().to_string()).unwrap_or_default()
            })),
            env: Vec::new(),
            hide,
            read_only: Vec::new(),
            stderr: None,
            now: Arc::new(jiff::Timestamp::now),
            store: Arc::new(claudecode::Store::of),
        }
    }

    pub fn signin(&self) -> Option<AccountSignin> {
        self.signin
            .lock()
            .expect("sign-in lock")
            .as_ref()
            .map(|signin| signin.view.clone())
    }

    pub fn begin_signin(self: &Arc<Self>, replace: Option<String>) -> Result<AccountSignin> {
        if let Some(id) = &replace {
            ensure!(
                self.accounts()?
                    .iter()
                    .any(|account| account.id == *id && account.kind == "login"),
                "select an existing sign-in account"
            );
        }
        let mut slot = self.signin.lock().expect("sign-in lock");
        if let Some(active) = slot.as_ref().filter(|s| s.view.status == "waiting") {
            return Ok(active.view.clone());
        }
        let id = new_id()?;
        self.prepare(&id)?;
        let (code, rx) = mpsc::channel(1);
        let cancel = CancellationToken::new();
        let view = AccountSignin {
            id: id.clone(),
            url: None,
            status: "waiting".into(),
            error: None,
        };
        *slot = Some(Signin {
            view: view.clone(),
            code,
            cancel: cancel.clone(),
        });
        let pool = self.clone();
        tokio::spawn(async move {
            let operation = pool.login(&id, rx, replace.as_deref());
            let login = tokio::select! {
                result = tokio::time::timeout(LOGIN_LIMIT, operation) => {
                    result.unwrap_or_else(|_| Err(anyhow::anyhow!("The sign-in request expired.")))
                }
                _ = cancel.cancelled() => Err(anyhow::anyhow!("Sign-in canceled.")),
            };
            {
                let mut slot = pool.signin.lock().expect("sign-in lock");
                if let Some(signin) = slot.as_mut().filter(|s| s.view.id == id) {
                    signin.view.url = None;
                    signin.view.error = login.as_ref().err().map(ToString::to_string);
                    signin.view.status = if login.is_ok() { "complete" } else { "failed" }.into();
                }
            }
            pool.changed();
            match login {
                Ok(account) => pool.probe(account).await,
                Err(_) => {
                    let _ = pool.discard(&id, "login").await;
                }
            }
        });
        Ok(view)
    }

    pub fn sign_in_code(&self, id: &str, code: String) -> Result<()> {
        ensure!(
            !code.trim().is_empty() && code.len() <= 2048 && !code.contains(['\r', '\n']),
            "invalid sign-in code"
        );
        let slot = self.signin.lock().expect("sign-in lock");
        let signin = slot
            .as_ref()
            .filter(|s| s.view.id == id && s.view.url.is_some() && s.view.status == "waiting")
            .context("the sign-in request has ended")?;
        signin
            .code
            .try_send(code)
            .map_err(|_| anyhow::anyhow!("could not accept the sign-in code"))
    }

    pub fn cancel_pending_signin(&self) {
        if let Some(signin) = self.signin.lock().expect("sign-in lock").as_ref() {
            signin.cancel.cancel();
        }
    }

    pub fn cancel_signin(&self, id: &str) -> Result<()> {
        let slot = self.signin.lock().expect("sign-in lock");
        let signin = slot
            .as_ref()
            .filter(|s| s.view.id == id && s.view.status == "waiting")
            .context("the sign-in request has ended")?;
        signin.cancel.cancel();
        Ok(())
    }

    async fn login(
        &self,
        id: &str,
        mut codes: mpsc::Receiver<String>,
        replace: Option<&str>,
    ) -> Result<AgentAccount> {
        let mut child = self
            .auth(id, "login")?
            .arg("--claudeai")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|_| anyhow::anyhow!("Could not start Claude Code sign-in."))?;
        let mut stdin = child
            .stdin
            .take()
            .context("Claude did not accept the code on stdin")?;
        let stdout = child.stdout.take().context("Claude stdout is unavailable")?;
        let stderr = child.stderr.take().context("Claude stderr is unavailable")?;
        let (tx, mut urls) = mpsc::channel(2);
        let scan = async {
            tokio::join!(scan_links(stdout, tx.clone()), scan_links(stderr, tx));
        };
        let interact = async {
            loop {
                tokio::select! {
                    Some(url) = urls.recv() => {
                        if let Some(state) = self.signin.lock().expect("sign-in lock").as_mut().filter(|s| s.view.id == id) {
                            state.view.url.get_or_insert(url);
                        }
                    }
                    Some(code) = codes.recv() => { let _ = send_code(&mut stdin, &code).await; }
                    else => break,
                }
            }
        };
        tokio::select! { _ = scan => {}, _ = interact => {} }
        // If the user could reach the local callback, the child can exit
        // without receiving a pasted code.
        let status = child
            .wait()
            .await
            .map_err(|_| anyhow::anyhow!("Claude sign-in ended unexpectedly."))?;
        ensure!(status.success(), "Claude did not complete sign-in.");
        let email = self
            .identity(id)
            .await?
            .context("Claude did not confirm the signed-in account")?;
        if let Some(expected) = replace {
            let accounts = self.accounts()?;
            ensure!(
                accounts
                    .iter()
                    .any(|a| a.id == expected && a.label.eq_ignore_ascii_case(&email)),
                "Sign in again with the email address of the selected account."
            );
        }
        let account = self.keep(id, &email.to_ascii_lowercase(), "login")?;
        if let Some(old) = account.1 {
            let _ = self.discard(&old, "login").await;
        }
        Ok(account.0)
    }

    pub async fn add_token(self: &Arc<Self>, label: &str, token: &str, replace: Option<&str>) -> Result<()> {
        ensure!(
            !label.trim().is_empty()
                && label.chars().count() <= 80
                && !label.contains(['\r', '\n']),
            "invalid connection name"
        );
        ensure!(
            !token.is_empty() && token.len() <= 4096 && !token.chars().any(char::is_whitespace),
            "invalid Claude token"
        );
        if let Some(id) = replace {
            ensure!(
                self.accounts()?
                    .iter()
                    .any(|a| a.id == id && a.kind == "connection"),
                "connection does not exist"
            );
        }
        let id = new_id()?;
        let dir = self.prepare(&id)?;
        let saved = (|| -> Result<AgentAccount> {
            let mut file = tempfile::NamedTempFile::new_in(&dir)?;
            file.write_all(token.as_bytes())?;
            file.as_file().sync_all()?;
            file.persist(dir.join("oauth-token"))
                .map_err(|err| err.error)?;
            if let Some(previous) = replace {
                let life = self.life.lock().expect("life lock");
                life.replace_token_account(previous, &id, label.trim())?;
                drop(life);
                self.accounts()?
                    .into_iter()
                    .find(|a| a.id == id)
                    .context("Claude connection not found")
            } else {
                Ok(self.keep(&id, label.trim(), "connection")?.0)
            }
        })();
        let account = match saved {
            Ok(account) => account,
            Err(err) => {
                let _ = self.discard(&id, "connection").await;
                return Err(err);
            }
        };
        if let Some(id) = replace {
            let _ = self.discard(id, "connection").await;
        }
        self.changed();
        self.probe(account).await;
        Ok(())
    }

    fn keep(&self, id: &str, label: &str, kind: &str) -> Result<(AgentAccount, Option<String>)> {
        let old = self
            .life
            .lock()
            .expect("life lock")
            .keep_agent_account(id, label, kind)?;
        let account = self
            .accounts()?
            .into_iter()
            .find(|a| a.id == id)
            .context("could not save the account")?;
        self.changed();
        Ok((account, old))
    }

    pub async fn remove(&self, id: &str) -> Result<()> {
        let account = self
            .accounts()?
            .into_iter()
            .find(|a| a.id == id)
            .context("account does not exist")?;
        self.life
            .lock()
            .expect("life lock")
            .remove_agent_account(id)?;
        self.changed();
        self.discard(id, &account.kind).await
    }

    async fn discard(&self, id: &str, kind: &str) -> Result<()> {
        ensure!(
            id != DEFAULT_ACCOUNT && valid_id(id),
            "the host account cannot be removed"
        );
        if kind == "login" {
            let _ = tokio::time::timeout(PROBE_LIMIT, self.auth(id, "logout")?.output()).await;
        }
        match fs::remove_dir_all(self.profile(id)?) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == ErrorKind::NotFound => Ok(()),
            Err(_) => bail!("The account was removed. Its Claude profile could not be deleted from the host."),
        }
    }
}

async fn scan_links(reader: impl tokio::io::AsyncRead + Unpin, links: mpsc::Sender<String>) {
    let mut lines = BufReader::new(reader).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        if let Some(url) = line.split_whitespace().find_map(signin_url) {
            let _ = links.try_send(url);
        }
    }
}

fn signin_url(word: &str) -> Option<String> {
    let word = word.trim_matches(['(', ')', '<', '>', '"']);
    let url = url::Url::parse(word).ok()?;
    if url.scheme() == "https"
        && matches!(
            url.host_str(),
            Some("claude.ai" | "claude.com" | "console.anthropic.com" | "platform.claude.com")
        )
        && (url.path().contains("oauth") || url.path().contains("authorize"))
        && url.username().is_empty()
        && url.password().is_none()
    {
        return Some(url.to_string());
    }
    None
}

async fn send_code(stdin: &mut ChildStdin, code: &str) -> std::io::Result<()> {
    stdin.write_all(code.trim().as_bytes()).await?;
    stdin.write_all(b"\n").await?;
    stdin.flush().await
}

fn valid_id(id: &str) -> bool {
    id.len() == 34 && id.starts_with("a-") && id[2..].bytes().all(|c| c.is_ascii_hexdigit())
}

fn new_id() -> Result<String> {
    let mut random = [0; 16];
    getrandom::fill(&mut random)
        .map_err(|_| anyhow::anyhow!("could not create a sign-in request ID"))?;
    Ok(format!(
        "a-{}",
        random
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    ))
}

fn private_dir(path: &Path) -> Result<()> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path)?;
    Ok(())
}

fn link_projects(profile: &Path, projects: &Path) -> Result<()> {
    let link = profile.join("projects");
    if let Ok(destination) = link.read_link() {
        ensure!(destination == projects, "the Claude profile points to a different history");
        return Ok(());
    }
    #[cfg(unix)]
    let result = std::os::unix::fs::symlink(projects, &link);
    #[cfg(windows)]
    let result = std::os::windows::fs::symlink_dir(projects, &link);
    match result {
        Ok(()) => Ok(()),
        Err(err)
            if err.kind() == ErrorKind::AlreadyExists
                && link.read_link().is_ok_and(|p| p == projects) =>
        {
            Ok(())
        }
        Err(err) => Err(err.into()),
    }
}

/// The `claude` command: `LIFEOS_CLAUDE` when set, else `claude` on the PATH.
fn claude_binary() -> PathBuf {
    std::env::var_os("LIFEOS_CLAUDE").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("claude"))
}

/// What an account check hears from its one turn.
#[derive(Default)]
struct Probe {
    limit: Mutex<Option<Limit>>,
}

impl Handler for Probe {
    fn on_update(&self, _u: Update) {}

    fn on_permission(&self, _req: PermissionRequest) -> agent::BoxFuture<'_, PermissionAnswer> {
        Box::pin(async { PermissionAnswer::Deny { message: "an account check uses no tool".into() } })
    }

    fn on_init(&self, _init: model::SessionInit) {}

    fn on_task(&self, _t: Task) {}

    fn on_working(&self, _working: bool) {}

    fn on_exit(&self, _end: Option<agent::Exit>) {}

    fn on_limit(&self, l: Limit) {
        *self.limit.lock().expect("probe lock") = Some(l);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn quota_rejection_moves_work_to_another_account() {
        let dir = tempfile::tempdir().unwrap();
        let life = Arc::new(Mutex::new(
            lifeos_core::Life::open_with_memory_db(dir.path()).unwrap(),
        ));
        let pool = AccountPool::new(life.clone(), OnChange::default());
        life.lock()
            .unwrap()
            .keep_agent_account("second", "test@example.test", "login")
            .unwrap();
        let stop = CancellationToken::new();
        let primary = pool.acquire(None, &stop, || panic!("ready account must not wait")).await.unwrap();
        assert_eq!(primary.account.id, DEFAULT_ACCOUNT);
        let reset = Utc::now().timestamp() + 60;
        let mut observation = AccountObservation::default();
        observation.limit(&Limit {
            status: LimitStatus::Rejected,
            window: "five_hour".into(),
            utilization: Some(1.0),
            resets_at: Some(model::Time(time::OffsetDateTime::from_unix_timestamp(reset).unwrap())),
        });
        pool.observe(&primary.account, &observation);
        let fallback = pool.acquire(Some(DEFAULT_ACCOUNT), &stop, || panic!("fallback must not wait")).await.unwrap();
        assert_eq!(fallback.account.id, "second");
        let accounts = pool.accounts().unwrap();
        assert_eq!(accounts[0].resets_at, Some(reset));
        assert_eq!(accounts[0].quota.as_deref(), Some("five_hour"));
        pool.observe(
            &fallback.account,
            &AccountObservation {
                status: Some("access"),
                ..Default::default()
            },
        );
        let waiting = pool.clone();
        let cancellation = stop.clone();
        let (entered, waiting_for_account) = tokio::sync::oneshot::channel();
        let wait = tokio::spawn(async move {
            waiting.acquire(None, &cancellation, || entered.send(()).unwrap()).await.is_err()
        });
        waiting_for_account.await.unwrap();
        assert!(!wait.is_finished());
        stop.cancel();
        assert!(wait.await.unwrap());
    }

    #[test]
    fn uses_the_provider_timezone_when_reset_is_text_only() {
        let now = "2026-11-25T09:00:00Z".parse().unwrap();
        let reset: chrono::DateTime<Utc> = "2026-11-27T01:00:00Z".parse().unwrap();
        assert_eq!(
            reported_reset("Weekly limit · resets Nov 27 at 8am (Asia/Saigon)", now),
            Some(reset.timestamp())
        );
    }

    #[test]
    fn sign_in_only_exposes_provider_oauth_urls() {
        assert!(signin_url("https://claude.ai/oauth/authorize?state=example").is_some());
        assert!(signin_url("https://example.test/oauth/authorize").is_none());
        assert!(signin_url("https://claude.ai@host.test/oauth/authorize").is_none());
        assert!(signin_url("https://claude.ai/help").is_none());
    }
}
