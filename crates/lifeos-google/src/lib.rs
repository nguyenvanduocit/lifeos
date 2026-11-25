//! lifeos's Google app: the owner signs in from the Google page (OAuth with PKCE, through an OAuth client of their own
//! they pasted once), and agents use Gmail, Calendar, Drive and Contacts through tools lifeos serves itself as the MCP
//! server `google`. The client and the refresh token live in a private directory outside the vault
//! (`Life::google_dir`), hidden from every session; access tokens stay in this process's memory.

mod api;
mod calendar;
mod contacts;
mod drive;
mod gmail;
mod store;
mod tools;
mod text;

use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail, ensure};
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::Local;
use lifeos_core::life::google::{CLIENT_FILE, TOKEN_FILE};
use lifeos_wire::{GoogleAccount, GoogleClient, GoogleSignin};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub use tools::{ToolSpec, specs};

/// Where Google sends the browser back to `lifeos web`.
pub const CALLBACK_PATH: &str = "/api/google/callback";

/// What the app asks Google for, by the service the owner sees.
pub const SERVICES: &[(&str, &str)] = &[
    ("gmail", "https://www.googleapis.com/auth/gmail.modify"),
    ("calendar", "https://www.googleapis.com/auth/calendar"),
    ("drive", "https://www.googleapis.com/auth/drive"),
    ("contacts", "https://www.googleapis.com/auth/contacts"),
];
const IDENTITY: &[&str] = &["openid", "https://www.googleapis.com/auth/userinfo.email"];

const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const REVOKE_URL: &str = "https://oauth2.googleapis.com/revoke";
const USERINFO_URL: &str = "https://openidconnect.googleapis.com/v1/userinfo";

/// How long a consent page stays good for.
const SIGNIN_LIMIT: Duration = Duration::from_secs(15 * 60);

/// The owner's OAuth client, as kept.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Client {
    client_id: String,
    client_secret: Option<String>,
    project: Option<String>,
    /// A web application client rather than a desktop one.
    #[serde(default)]
    web: bool,
}

/// The sign-in, as kept.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Saved {
    email: String,
    refresh_token: String,
    /// The client the refresh token was issued to.
    client_id: String,
    scopes: Vec<String>,
    since: String,
}

struct Pending {
    state: String,
    verifier: String,
    redirect: String,
    url: String,
    started: Instant,
    error: Option<String>,
}

struct Access {
    token: String,
    expires: Instant,
}

pub struct Google {
    dir: Option<PathBuf>,
    http: reqwest::Client,
    pending: Mutex<Option<Pending>>,
    access: tokio::sync::Mutex<Option<Access>>,
}

impl std::fmt::Debug for Google {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Google")
    }
}

impl Google {
    /// The app keeping its files in `dir`; with none, it cannot be signed in to.
    pub fn new(dir: Option<PathBuf>) -> Google {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .user_agent(concat!("lifeos/", env!("CARGO_PKG_VERSION")))
            .build()
            .unwrap_or_default();
        Google { dir, http, pending: Mutex::default(), access: tokio::sync::Mutex::default() }
    }

    fn dir(&self) -> Result<&PathBuf> {
        self.dir.as_ref().context("set LIFEOS_GOOGLE_DIR to a private absolute directory")
    }

    fn client(&self) -> Result<Option<Client>> {
        store::read(self.dir()?, CLIENT_FILE)
    }

    fn saved(&self) -> Result<Option<Saved>> {
        store::read(self.dir()?, TOKEN_FILE)
    }

    /// Whether agents can use Google now.
    pub fn signed_in(&self) -> bool {
        self.dir.is_some() && self.saved().ok().flatten().is_some()
    }

    /// What the Google page shows.
    pub fn view(&self) -> Result<(Option<GoogleClient>, Option<GoogleAccount>, Option<GoogleSignin>)> {
        if self.dir.is_none() {
            return Ok((None, None, None));
        }
        let client = self.client()?.map(|c| GoogleClient {
            client_id: c.client_id,
            kind: if c.web { "web" } else { "desktop" }.into(),
            project: c.project,
        });
        let account = self.saved()?.map(|s| GoogleAccount {
            email: s.email,
            services: services_of(&s.scopes),
            since: s.since,
        });
        let mut pending = self.lock_pending();
        if pending.as_ref().is_some_and(|p| p.error.is_none() && p.started.elapsed() > SIGNIN_LIMIT) {
            *pending = None;
        }
        let signin = pending.as_ref().map(|p| GoogleSignin {
            url: p.url.clone(),
            status: if p.error.is_some() { "failed" } else { "waiting" }.into(),
            error: p.error.clone(),
        });
        Ok((client, account, signin))
    }

    /// Keeps the OAuth client the owner downloaded from Google Cloud (its JSON). A sign-in made through another
    /// client ends, since its refresh token only works with the client it was issued to.
    pub async fn set_client(&self, json: &str) -> Result<()> {
        let client = parse_client(json)?;
        let dir = self.dir()?;
        if self.saved()?.is_some_and(|s| s.client_id != client.client_id) {
            self.sign_out().await?;
        }
        store::write(dir, CLIENT_FILE, &client)?;
        self.cancel();
        Ok(())
    }

    /// Forgets the client and the sign-in made through it.
    pub async fn forget_client(&self) -> Result<()> {
        self.sign_out().await?;
        store::remove(self.dir()?, CLIENT_FILE)?;
        self.cancel();
        Ok(())
    }

    /// Starts a sign-in that comes back to `redirect`; the consent page's URL is in the view.
    pub fn begin_signin(&self, redirect: &str) -> Result<GoogleSignin> {
        let client = self.client()?.context("paste your Google OAuth client first")?;
        let state = random_token(24)?;
        let verifier = random_token(48)?;
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        let scopes: Vec<&str> = IDENTITY.iter().copied().chain(SERVICES.iter().map(|(_, s)| *s)).collect();
        let url = url::Url::parse_with_params(
            AUTH_URL,
            [
                ("client_id", client.client_id.as_str()),
                ("redirect_uri", redirect),
                ("response_type", "code"),
                ("scope", &scopes.join(" ")),
                ("code_challenge", &challenge),
                ("code_challenge_method", "S256"),
                ("state", &state),
                ("access_type", "offline"),
                ("prompt", "consent"),
                ("include_granted_scopes", "true"),
            ],
        )?
        .to_string();
        let view = GoogleSignin { url: url.clone(), status: "waiting".into(), error: None };
        *self.lock_pending() =
            Some(Pending { state, verifier, redirect: redirect.to_owned(), url, started: Instant::now(), error: None });
        Ok(view)
    }

    /// Ends the sign-in waiting with what Google sent the browser back with: the whole address the browser landed
    /// on, its query, or the bare code.
    pub async fn finish_signin(&self, returned: &str) -> Result<()> {
        let answer = parse_return(returned)?;
        let (verifier, redirect) = {
            let mut pending = self.lock_pending();
            let p = pending
                .as_mut()
                .filter(|p| p.error.is_none() && p.started.elapsed() <= SIGNIN_LIMIT)
                .context("no Google sign-in is waiting; start again")?;
            if let Some(state) = &answer.state {
                ensure!(*state == p.state, "this answer belongs to another sign-in; start again");
            }
            if let Some(error) = &answer.error {
                p.error = Some(match error.as_str() {
                    "access_denied" => "Google sign-in was cancelled.".to_owned(),
                    other => format!("Google refused the sign-in: {other}"),
                });
                return Ok(());
            }
            (p.verifier.clone(), p.redirect.clone())
        };
        match self.exchange(answer.code.as_deref().unwrap_or_default(), &verifier, &redirect).await {
            Ok(()) => {
                *self.lock_pending() = None;
                Ok(())
            }
            Err(err) => {
                if let Some(p) = self.lock_pending().as_mut() {
                    p.error = Some(format!("{err:#}"));
                }
                Err(err)
            }
        }
    }

    async fn exchange(&self, code: &str, verifier: &str, redirect: &str) -> Result<()> {
        ensure!(!code.is_empty(), "Google sent no code back");
        let client = self.client()?.context("the Google OAuth client was removed")?;
        let mut form = vec![
            ("grant_type", "authorization_code"),
            ("code", code),
            ("code_verifier", verifier),
            ("redirect_uri", redirect),
            ("client_id", &client.client_id),
        ];
        if let Some(secret) = &client.client_secret {
            form.push(("client_secret", secret));
        }
        let tokens = self.token_request(&form).await?;
        let access = tokens.access_token.context("Google sent no access token")?;
        let refresh = tokens.refresh_token.context(
            "Google sent no refresh token. Remove LifeOS's access at myaccount.google.com/permissions, then sign in again.",
        )?;
        let info: serde_json::Value = self
            .http
            .get(USERINFO_URL)
            .bearer_auth(&access)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let email = info.get("email").and_then(|e| e.as_str()).context("Google did not say which account this is")?;
        let saved = Saved {
            email: email.to_owned(),
            refresh_token: refresh,
            client_id: client.client_id,
            scopes: tokens.scope.unwrap_or_default().split_whitespace().map(String::from).collect(),
            since: Local::now().to_rfc3339(),
        };
        store::write(self.dir()?, TOKEN_FILE, &saved)?;
        *self.access.lock().await = Some(Access { token: access, expires: expiry(tokens.expires_in) });
        Ok(())
    }

    /// Drops a sign-in that is waiting or failed.
    pub fn cancel(&self) {
        *self.lock_pending() = None;
    }

    /// Revokes the sign-in at Google (as far as Google answers) and forgets it here.
    pub async fn sign_out(&self) -> Result<()> {
        if let Some(saved) = self.saved()? {
            let revoked = self.http.post(REVOKE_URL).form(&[("token", saved.refresh_token.as_str())]).send().await;
            if let Err(err) = revoked {
                tracing::warn!("could not revoke the Google sign-in: {err}");
            }
        }
        store::remove(self.dir()?, TOKEN_FILE)?;
        *self.access.lock().await = None;
        Ok(())
    }

    /// An access token good for at least a minute.
    async fn access_token(&self) -> Result<String> {
        let mut access = self.access.lock().await;
        if let Some(a) = access.as_ref().filter(|a| a.expires > Instant::now() + Duration::from_secs(60)) {
            return Ok(a.token.clone());
        }
        let saved = self.saved()?.context("Google is not signed in; the owner signs in on the Google page in LifeOS")?;
        let client = self.client()?.context("the Google OAuth client was removed; the owner pastes it again")?;
        let mut form = vec![
            ("grant_type", "refresh_token"),
            ("refresh_token", saved.refresh_token.as_str()),
            ("client_id", client.client_id.as_str()),
        ];
        if let Some(secret) = &client.client_secret {
            form.push(("client_secret", secret));
        }
        let tokens = self.token_request(&form).await?;
        let token = tokens.access_token.context("Google sent no access token")?;
        *access = Some(Access { token: token.clone(), expires: expiry(tokens.expires_in) });
        Ok(token)
    }

    async fn invalidate(&self) {
        *self.access.lock().await = None;
    }

    async fn token_request(&self, form: &[(&str, &str)]) -> Result<Tokens> {
        let res = self.http.post(TOKEN_URL).form(form).send().await.context("could not reach Google")?;
        let status = res.status();
        let tokens: Tokens = res.json().await.context("Google answered with something unreadable")?;
        if let Some(error) = tokens.error {
            if error == "invalid_grant" {
                bail!("the Google sign-in expired or was revoked; the owner signs in again on the Google page");
            }
            bail!("Google: {}", tokens.error_description.unwrap_or(error));
        }
        ensure!(status.is_success(), "Google answered {status}");
        Ok(tokens)
    }

    fn lock_pending(&self) -> std::sync::MutexGuard<'_, Option<Pending>> {
        self.pending.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[derive(Deserialize)]
struct Tokens {
    access_token: Option<String>,
    refresh_token: Option<String>,
    expires_in: Option<u64>,
    scope: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

fn expiry(seconds: Option<u64>) -> Instant {
    Instant::now() + Duration::from_secs(seconds.unwrap_or(3600))
}

fn services_of(scopes: &[String]) -> Vec<String> {
    SERVICES.iter().filter(|(_, s)| scopes.iter().any(|g| g == s)).map(|(name, _)| (*name).to_owned()).collect()
}

fn random_token(bytes: usize) -> Result<String> {
    let mut buf = vec![0_u8; bytes];
    getrandom::fill(&mut buf).map_err(|e| anyhow::anyhow!("no randomness: {e}"))?;
    Ok(URL_SAFE_NO_PAD.encode(buf))
}

/// The client JSON Google Cloud gives for a Desktop app (`installed`) or a web app (`web`), or its two fields.
fn parse_client(json: &str) -> Result<Client> {
    #[derive(Deserialize)]
    struct Fields {
        client_id: String,
        client_secret: Option<String>,
        project_id: Option<String>,
    }
    #[derive(Deserialize)]
    struct Downloaded {
        installed: Option<Fields>,
        web: Option<Fields>,
    }
    let json = json.trim();
    ensure!(json.len() <= 16 * 1024, "that is too long to be a Google OAuth client");
    let (fields, web) = match serde_json::from_str::<Downloaded>(json) {
        Ok(Downloaded { web: Some(f), .. }) => (f, true),
        Ok(Downloaded { installed: Some(f), .. }) => (f, false),
        _ => (
            serde_json::from_str::<Fields>(json)
                .context("paste the JSON Google Cloud gives for an OAuth client")?,
            true,
        ),
    };
    let id = fields.client_id.trim();
    ensure!(
        id.ends_with(".apps.googleusercontent.com") && id.len() < 256 && !id.contains(char::is_whitespace),
        "that is not a Google OAuth client ID"
    );
    Ok(Client {
        client_id: id.to_owned(),
        client_secret: fields.client_secret.map(|s| s.trim().to_owned()).filter(|s| !s.is_empty()),
        project: fields.project_id.filter(|p| !p.is_empty()),
        web,
    })
}

#[derive(Debug, PartialEq)]
struct Return {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

/// What the browser came back with: `http://127.0.0.1:…/api/google/callback?code=…&state=…`, its query, or a code.
fn parse_return(returned: &str) -> Result<Return> {
    let returned = returned.trim();
    ensure!(!returned.is_empty() && returned.len() <= 8 * 1024, "paste the address Google sent you back to");
    let query = if let Ok(url) = url::Url::parse(returned) {
        url.query().unwrap_or_default().to_owned()
    } else if returned.contains("code=") || returned.contains("error=") {
        returned.trim_start_matches('?').to_owned()
    } else {
        ensure!(!returned.contains(char::is_whitespace), "paste the address Google sent you back to");
        return Ok(Return { code: Some(returned.to_owned()), state: None, error: None });
    };
    let mut out = Return { code: None, state: None, error: None };
    for (key, value) in url::form_urlencoded::parse(query.as_bytes()) {
        match key.as_ref() {
            "code" => out.code = Some(value.into_owned()),
            "state" => out.state = Some(value.into_owned()),
            "error" => out.error = Some(value.into_owned()),
            _ => {}
        }
    }
    ensure!(out.code.is_some() || out.error.is_some(), "that address has no sign-in code");
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_client_json_google_cloud_gives() {
        let json = r#"{"installed":{"client_id":"123-abc.apps.googleusercontent.com","project_id":"lifeos-1",
            "auth_uri":"https://accounts.google.com/o/oauth2/auth","client_secret":"GOCSPX-x",
            "redirect_uris":["http://localhost"]}}"#;
        let client = parse_client(json).unwrap();
        assert_eq!(client.client_id, "123-abc.apps.googleusercontent.com");
        assert_eq!(client.client_secret.as_deref(), Some("GOCSPX-x"));
        assert_eq!(client.project.as_deref(), Some("lifeos-1"));
        assert!(!client.web);
        let web = r#"{"web":{"client_id":"9-z.apps.googleusercontent.com","client_secret":"x",
            "redirect_uris":["https://lifeos.example.com/api/google/callback"]}}"#;
        assert!(parse_client(web).unwrap().web);
        assert!(parse_client(r#"{"client_id":"nope"}"#).is_err());
        assert!(parse_client("not json").is_err());
    }

    #[test]
    fn reads_what_the_browser_came_back_with() {
        let full = parse_return("http://127.0.0.1:7420/api/google/callback?state=s1&code=4%2F0A&scope=x").unwrap();
        assert_eq!(full, Return { code: Some("4/0A".into()), state: Some("s1".into()), error: None });
        let query = parse_return("?error=access_denied&state=s1").unwrap();
        assert_eq!(query.error.as_deref(), Some("access_denied"));
        assert_eq!(parse_return("4/0Abc").unwrap().code.as_deref(), Some("4/0Abc"));
        assert!(parse_return("http://127.0.0.1/api/google/callback").is_err());
        assert!(parse_return("two words").is_err());
    }

    #[tokio::test]
    async fn a_sign_in_takes_only_its_own_answer() {
        let dir = tempfile::tempdir().unwrap();
        let google = Google::new(Some(dir.path().join("google")));
        assert!(google.begin_signin("http://127.0.0.1:1/cb").is_err());
        google
            .set_client(r#"{"installed":{"client_id":"1-a.apps.googleusercontent.com","client_secret":"s"}}"#)
            .await
            .unwrap();
        let signin = google.begin_signin("http://127.0.0.1:1/cb").unwrap();
        let url = url::Url::parse(&signin.url).unwrap();
        let params: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(params["code_challenge_method"], "S256");
        assert!(params["scope"].contains("gmail.modify"));
        assert!(google.finish_signin("http://127.0.0.1:1/cb?code=c&state=other").await.is_err());
        let state = &params["state"];
        google.finish_signin(&format!("?error=access_denied&state={state}")).await.unwrap();
        let (_, account, signin) = google.view().unwrap();
        assert!(account.is_none());
        assert_eq!(signin.unwrap().status, "failed");
        assert!(!google.signed_in());
    }

    #[test]
    fn the_pkce_challenge_is_rfc_7636s() {
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(b"dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"));
        assert_eq!(challenge, "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
    }
}
