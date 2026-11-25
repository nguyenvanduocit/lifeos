//! The owner's Google page: paste the OAuth client, sign in, sign out. Like the account controls, these requests
//! stay outside the agent tools and the link; the client, codes and tokens never reach an agent.

use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::Json;
use lifeos_wire::GoogleView;
use serde::Deserialize;
use serde_json::json;

use crate::HttpState;
use crate::accounts::{owner_request, private_response};

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum Action {
    /// Keep the OAuth client JSON from Google Cloud.
    Client { json: String },
    /// Forget the client and the sign-in.
    ForgetClient,
    /// Start a sign-in; the consent page's URL comes back in the view.
    Login,
    /// Finish a sign-in with the address the browser landed on, when it could not reach this server itself.
    Code { returned: String },
    Cancel,
    Logout,
}

/// Where Google sends the browser back: the address the owner opened LifeOS at (a domain behind a reverse proxy such
/// as `https://lifeos.example.com`, which a web application client registers), or this server on loopback when
/// opened there (which a desktop client may use without registering).
pub(super) fn redirect(state: &HttpState, headers: &HeaderMap) -> String {
    redirect_to(state.http, headers)
}

fn redirect_to(http: std::net::SocketAddr, headers: &HeaderMap) -> String {
    let text = |name: &str| headers.get(name).and_then(|v| v.to_str().ok()).map(str::trim).filter(|v| !v.is_empty());
    let host = text("x-forwarded-host").or_else(|| text(header::HOST.as_str())).and_then(|h| h.split(',').next());
    let loopback = |h: &str| {
        let name = crate::strip_port(h).trim_start_matches('[').trim_end_matches(']');
        name == "localhost" || name.parse::<std::net::IpAddr>().is_ok_and(|ip| ip.is_loopback())
    };
    match host.map(str::trim).filter(|h| !loopback(h) && valid_host(h)) {
        Some(host) => {
            let scheme = match text("x-forwarded-proto").and_then(|p| p.split(',').next()).map(str::trim) {
                Some("https") => "https",
                Some("http") => "http",
                _ => "http",
            };
            format!("{scheme}://{host}{}", lifeos_google::CALLBACK_PATH)
        }
        None => {
            let ip = if http.ip() == std::net::Ipv6Addr::LOCALHOST { "[::1]" } else { "127.0.0.1" };
            format!("http://{ip}:{}{}", http.port(), lifeos_google::CALLBACK_PATH)
        }
    }
}

/// A host[:port] fit to put in a URL.
fn valid_host(h: &str) -> bool {
    !h.is_empty()
        && h.len() <= 255
        && h.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b':' | b'[' | b']'))
}

fn snapshot(state: &HttpState, headers: &HeaderMap) -> Response {
    let google = state.app.agents.google();
    let members = {
        let life = state.app.life.lock().expect("life lock");
        life.connections().map(|c| {
            c.connections
                .into_iter()
                .find(|c| c.name == lifeos_core::life::google::CONNECTION)
                .map(|c| c.members)
                .unwrap_or_default()
        })
    };
    match (google.view(), members) {
        (Ok((client, account, signin)), Ok(members)) => private_response(
            StatusCode::OK,
            GoogleView { client, account, signin, redirect: redirect(state, headers), members },
        ),
        (Err(err), _) | (_, Err(err)) => {
            tracing::warn!("could not read the Google app: {err:#}");
            private_response(StatusCode::INTERNAL_SERVER_ERROR, json!({ "error": "Could not read the Google app." }))
        }
    }
}

fn forbidden() -> Response {
    private_response(
        StatusCode::FORBIDDEN,
        json!({ "error": "Google requests must come from the LifeOS interface." }),
    )
}

pub(super) async fn view(State(state): State<Arc<HttpState>>, headers: HeaderMap) -> Response {
    if !owner_request(&headers) {
        return forbidden();
    }
    snapshot(&state, &headers)
}

pub(super) async fn act(State(state): State<Arc<HttpState>>, headers: HeaderMap, Json(action): Json<Action>) -> Response {
    if !owner_request(&headers) {
        return forbidden();
    }
    let google = state.app.agents.google();
    let result = match action {
        Action::Client { json } => google.set_client(&json).await,
        Action::ForgetClient => google.forget_client().await,
        Action::Login => google.begin_signin(&redirect(&state, &headers)).map(|_| ()),
        Action::Code { returned } => google.finish_signin(&returned).await,
        Action::Cancel => {
            google.cancel();
            Ok(())
        }
        Action::Logout => google.sign_out().await,
    };
    // The Connections page shows whether Google is on.
    state.app.bump();
    match result {
        Ok(()) => snapshot(&state, &headers),
        Err(err) => private_response(StatusCode::BAD_REQUEST, json!({ "error": format!("{err:#}") })),
    }
}

#[derive(Deserialize)]
pub(super) struct Returned {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

/// Where Google sends the browser after consent. Google's navigation is cross-site, so there is no owner header;
/// the sign-in's own `state` is what ties the answer to the sign-in the owner started.
pub(super) async fn callback(State(state): State<Arc<HttpState>>, Query(q): Query<Returned>) -> Response {
    let query = url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(q.code.iter().map(|c| ("code", c)))
        .extend_pairs(q.state.iter().map(|s| ("state", s)))
        .extend_pairs(q.error.iter().map(|e| ("error", e)))
        .finish();
    let google = state.app.agents.google();
    let outcome = google.finish_signin(&format!("?{query}")).await;
    state.app.bump();
    let (title, words) = match (&outcome, google.view().ok().and_then(|v| v.1)) {
        (Ok(()), Some(account)) => ("Signed in to Google", format!("LifeOS can now use Google as {}.", account.email)),
        (Ok(()), None) => ("Google sign-in cancelled", "Nothing was connected.".to_owned()),
        (Err(err), _) => ("Google sign-in failed", format!("{err:#}")),
    };
    let page = format!(
        "<!doctype html><meta charset=utf-8><meta name=viewport content='width=device-width'>\
         <title>{title}</title><body style='font:16px system-ui;max-width:32rem;margin:4rem auto;padding:0 1rem'>\
         <h1 style='font-size:1.25rem'>{title}</h1><p>{}</p>\
         <p><a href='/connections/google'>Back to LifeOS</a></p>",
        escape(&words)
    );
    let status = if outcome.is_ok() { StatusCode::OK } else { StatusCode::BAD_REQUEST };
    (status, [(header::CACHE_CONTROL, "no-store"), (header::REFERRER_POLICY, "no-referrer")], Html(page)).into_response()
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use axum::http::HeaderMap;

    fn redirect_for(pairs: &[(&'static str, &str)]) -> String {
        let mut headers = HeaderMap::new();
        for (k, v) in pairs {
            headers.insert(*k, v.parse().unwrap());
        }
        super::redirect_to("127.0.0.1:7420".parse().unwrap(), &headers)
    }

    #[test]
    fn sign_in_comes_back_to_the_address_the_owner_opened() {
        assert_eq!(
            redirect_for(&[("host", "lifeos.aiocean.dev"), ("x-forwarded-proto", "https")]),
            "https://lifeos.aiocean.dev/api/google/callback"
        );
        assert_eq!(
            redirect_for(&[("host", "box.tail.ts.net:8443"), ("x-forwarded-proto", "https")]),
            "https://box.tail.ts.net:8443/api/google/callback"
        );
        assert_eq!(redirect_for(&[("host", "localhost:5173")]), "http://127.0.0.1:7420/api/google/callback");
        assert_eq!(redirect_for(&[("host", "127.0.0.1:7420")]), "http://127.0.0.1:7420/api/google/callback");
        assert_eq!(redirect_for(&[("host", "evil.com/x?")]), "http://127.0.0.1:7420/api/google/callback");
        assert_eq!(redirect_for(&[]), "http://127.0.0.1:7420/api/google/callback");
    }
}
