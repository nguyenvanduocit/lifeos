//! Where the iPhone app (`apps/ios`) sends what it reads from Apple Health: a day's value per measure, the last
//! days again on every sync. The Mac has no Health store of its own, so the phone is the only way in.

use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use lifeos_core::life::health::{APPLE_HEALTH, Reading};
use serde::Deserialize;
use serde_json::json;

use crate::HttpState;
use crate::accounts::{owner_request, private_response};

/// About a year of days with every measure.
pub(super) const MAX_READINGS: usize = 4000;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Upload {
    readings: Vec<Reading>,
}

pub(super) async fn apple(State(state): State<Arc<HttpState>>, headers: HeaderMap, Json(upload): Json<Upload>) -> Response {
    if !owner_request(&headers) {
        return private_response(StatusCode::FORBIDDEN, json!({ "error": "not the owner's app" }));
    }
    if upload.readings.len() > MAX_READINGS {
        return private_response(StatusCode::PAYLOAD_TOO_LARGE, json!({ "error": "send fewer days at once" }));
    }
    let kept = state.app.life.lock().expect("life lock").import_readings(APPLE_HEALTH, &upload.readings);
    match kept {
        Ok(kept) => {
            state.app.bump();
            private_response(StatusCode::OK, json!({ "kept": kept }))
        }
        Err(err) => private_response(StatusCode::UNPROCESSABLE_ENTITY, json!({ "error": format!("{err:#}") })),
    }
}
