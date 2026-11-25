use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result, ensure};
use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use lifeos_core::life::widgets::WidgetPlacement;
use serde::Deserialize;
use serde_json::json;

use crate::accounts::{owner_request, private_response};
use crate::{AppState, HttpState};

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum Action {
    Arrange {
        page: String,
        layout: Vec<WidgetPlacement>,
    },
    Refresh {
        id: i32,
    },
    Remove {
        id: i32,
    },
}

pub(super) async fn act(
    State(state): State<Arc<HttpState>>,
    headers: HeaderMap,
    Json(action): Json<Action>,
) -> Response {
    if !owner_request(&headers) {
        return private_response(
            StatusCode::FORBIDDEN,
            json!({"error": "Use the LifeOS interface."}),
        );
    }
    let result = match action {
        Action::Arrange { page, layout } => {
            let result = state
                .app
                .life
                .lock()
                .expect("life lock")
                .arrange_widgets(&page, &layout);
            if result.is_ok() {
                state.app.bump();
            }
            result
        }
        Action::Refresh { id } => refresh(state.app.clone(), id).await,
        Action::Remove { id } => {
            let result = state.app.life.lock().expect("life lock").remove_widget(id);
            if result.is_ok() {
                state.app.bump();
            }
            result
        }
    };
    match result {
        Ok(()) => private_response(StatusCode::OK, json!({"ok": true})),
        Err(err) => private_response(
            StatusCode::BAD_REQUEST,
            json!({"error": format!("{err:#}")}),
        ),
    }
}

#[derive(Default)]
pub(super) struct Refreshes(Mutex<HashSet<i32>>);

struct RefreshGuard {
    state: Arc<AppState>,
    id: i32,
}

impl Drop for RefreshGuard {
    fn drop(&mut self) {
        self.state
            .widget_refreshes
            .0
            .lock()
            .expect("refresh lock")
            .remove(&self.id);
    }
}

async fn refresh(state: Arc<AppState>, id: i32) -> Result<()> {
    let widget = state
        .life
        .lock()
        .expect("life lock")
        .widget(id)
        .context("Widget not found")?;
    ensure!(
        state
            .widget_refreshes
            .0
            .lock()
            .expect("refresh lock")
            .insert(id),
        "This widget is already updating."
    );
    let guard = RefreshGuard {
        state: state.clone(),
        id,
    };
    // Keep the refresh running and reserved if the browser closes before completion.
    tokio::spawn(async move {
        let _guard = guard;
        let kind = widget.content.as_ref()
            .and_then(|content| serde_json::to_value(content).ok())
            .and_then(|value| value.get("kind")?.as_str().map(str::to_owned))
            .unwrap_or_else(|| "legacy Markdown".into());
        let request = format!(
            "Update now was pressed for LifeOS widget widget_{id} on page {page:?}, titled {title:?}. \
             Refresh just this widget using current source data. Read list_widgets first if you need its current fields. \
             Call save_widget with this exact ID and the updated content in {kind} format. Omit title, page and prompt. \
             If you cannot obtain current data, say why and leave the widget unchanged. \
             Standing instructions: {prompt}",
            page = widget.page,
            title = widget.title,
            prompt = if widget.prompt.is_empty() { &widget.title } else { &widget.prompt },
        );
        let outcome = tokio::time::timeout(
            Duration::from_secs(300),
            state.agents.ask(&widget.agent, &request, lifeos_agent::owner()),
        ).await.context("The agent has not finished updating this widget")??;
        if let Some(error) = outcome.error {
            anyhow::bail!("{error}");
        }
        let updated = state.life.lock().expect("life lock").widget(id)?;
        ensure!(updated.version > widget.version, "The agent finished without updating this widget. Check its conversation for details.");
        Ok(())
    }).await?
}
