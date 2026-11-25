//! Owner controls for the agent's accounts. These requests stay outside of
//! the agent tool and chat protocols, especially login codes and tokens.

use std::sync::Arc;

use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use lifeos_wire::AgentAccounts;
use serde::Deserialize;
use serde_json::json;

use crate::HttpState;

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum Action {
    Login {
        replace: Option<String>,
    },
    Code {
        id: String,
        code: String,
    },
    Cancel {
        id: String,
    },
    Check,
    Remove {
        id: String,
    },
    Token {
        label: String,
        token: String,
        replace: Option<String>,
    },
}

pub(super) fn owner_request(headers: &HeaderMap) -> bool {
    // The app is served on loopback or its private network. Require a header
    // a cross-origin webpage cannot send without a CORS preflight, and reject
    // cross-origin browser requests before any action.
    headers
        .get("x-lifeos-account")
        .is_some_and(|value| value == "1")
        && headers
            .get("sec-fetch-site")
            .is_none_or(|value| value == "same-origin" || value == "none")
}

pub(super) fn private_response(status: StatusCode, value: impl serde::Serialize) -> Response {
    (
        status,
        [
            (header::CACHE_CONTROL, "no-store"),
            (header::REFERRER_POLICY, "no-referrer"),
        ],
        Json(value),
    )
        .into_response()
}

async fn snapshot(state: &HttpState) -> Response {
    let pool = &state.app.accounts;
    match pool.accounts() {
        Ok(accounts) => private_response(
            StatusCode::OK,
            AgentAccounts {
                accounts,
                signin: pool.signin(),
            },
        ),
        Err(_) => private_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            json!({ "error": "Could not load accounts." }),
        ),
    }
}

pub(super) async fn view(State(state): State<Arc<HttpState>>, headers: HeaderMap) -> Response {
    if !owner_request(&headers) {
        return private_response(
            StatusCode::FORBIDDEN,
            json!({ "error": "Account requests must come from the LifeOS interface." }),
        );
    }
    snapshot(&state).await
}

pub(super) async fn act(
    State(state): State<Arc<HttpState>>,
    headers: HeaderMap,
    Json(action): Json<Action>,
) -> Response {
    if !owner_request(&headers) {
        return private_response(
            StatusCode::FORBIDDEN,
            json!({ "error": "Account requests must come from the LifeOS interface." }),
        );
    }
    let pool = &state.app.accounts;
    let result = match action {
        Action::Login { replace } => pool.begin_signin(replace).map(|_| ()),
        Action::Code { id, code } => pool.sign_in_code(&id, code),
        Action::Cancel { id } => pool.cancel_signin(&id),
        Action::Check => pool.check().await,
        Action::Remove { id } => pool.remove(&id).await,
        Action::Token {
            label,
            token,
            replace,
        } => pool.add_token(&label, &token, replace.as_deref()).await,
    };
    match result {
        Ok(()) => snapshot(&state).await,
        Err(err) => private_response(StatusCode::BAD_REQUEST, json!({ "error": err.to_string() })),
    }
}
