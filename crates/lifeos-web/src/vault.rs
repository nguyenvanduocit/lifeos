//! The owner's Vault page (Settings › Vault). Like the account controls, this stays outside the agent tools and the
//! link: a password or a key comes in with the request that saves it, is written to the private secrets directory,
//! and goes back to no one; the page reads what the items are and who may use them, never a value.

use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use lifeos_core::life::vault_items::{ImportVaultKeys, SaveVaultItem};
use serde::Deserialize;
use serde_json::json;

use crate::HttpState;
use crate::accounts::{owner_request, private_response};

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum Action {
    Save { item: SaveVaultItem },
    Remove { id: i64 },
    Import { keys: ImportVaultKeys },
}

fn refused() -> Response {
    private_response(StatusCode::FORBIDDEN, json!({ "error": "Vault requests must come from the LifeOS interface." }))
}

pub(super) async fn view(State(state): State<Arc<HttpState>>, headers: HeaderMap) -> Response {
    if !owner_request(&headers) {
        return refused();
    }
    let view = state.app.life.lock().expect("life lock").vault_view();
    match view {
        Ok(view) => private_response(StatusCode::OK, view),
        Err(err) => private_response(StatusCode::INTERNAL_SERVER_ERROR, json!({ "error": format!("{err:#}") })),
    }
}

pub(super) async fn act(State(state): State<Arc<HttpState>>, headers: HeaderMap, Json(action): Json<Action>) -> Response {
    if !owner_request(&headers) {
        return refused();
    }
    // What the request carried that must never come back, even inside an error.
    let values: Vec<String> = match &action {
        Action::Save { item } => item.secret.iter().cloned().collect(),
        Action::Import { keys } => keys.keys.iter().map(|k| k.value.clone()).collect(),
        Action::Remove { .. } => Vec::new(),
    };
    let result = {
        let life = state.app.life.lock().expect("life lock");
        match action {
            Action::Save { item } => life.save_vault_item(item).map(|item| json!({ "ok": true, "id": item.id })),
            Action::Remove { id } => life.remove_vault_item(id).map(|()| json!({ "ok": true })),
            Action::Import { keys } => life.import_vault_keys(keys).map(|done| json!({ "ok": true, "imported": done })),
        }
    };
    match result {
        Ok(body) => {
            state.app.bump();
            private_response(StatusCode::OK, body)
        }
        Err(err) => {
            let mut message = format!("{err:#}");
            for value in values.iter().filter(|v| !v.is_empty()) {
                message = message.replace(value.as_str(), "…");
            }
            private_response(StatusCode::BAD_REQUEST, json!({ "error": message }))
        }
    }
}
