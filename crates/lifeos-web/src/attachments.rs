//! Original bytes of files attached to an agent message.

use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use lifeos_core::life::images::attachment_path;

use super::HttpState;

pub(super) const MAX_BYTES: usize = 25 * 1024 * 1024;

fn path(state: &HttpState, key: &str, name: &str) -> Result<PathBuf, StatusCode> {
    let relative = attachment_path(&format!("{key}/{name}")).map_err(|_| StatusCode::BAD_REQUEST)?;
    Ok(state.app.life.lock().expect("life lock").home().join(relative))
}

pub(super) async fn put(
    State(state): State<Arc<HttpState>>,
    Path((key, name)): Path<(String, String)>,
    bytes: Bytes,
) -> Result<StatusCode, StatusCode> {
    let path = path(&state, &key, &name)?;
    tokio::task::spawn_blocking(move || {
        let dir = path.parent().expect("attachment directory");
        std::fs::create_dir_all(dir)?;
        let mut file = tempfile::NamedTempFile::new_in(dir)?;
        file.write_all(&bytes)?;
        match file.persist_noclobber(path) {
            Ok(_) => Ok(StatusCode::CREATED),
            Err(e) if e.error.kind() == std::io::ErrorKind::AlreadyExists => Ok(StatusCode::CONFLICT),
            Err(e) => Err(e.error),
        }
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .map_err(|e| {
        tracing::error!(%e, "could not store attachment");
        StatusCode::INTERNAL_SERVER_ERROR
    })
}

pub(super) async fn get(
    State(state): State<Arc<HttpState>>,
    Path((key, name)): Path<(String, String)>,
) -> Result<Response, StatusCode> {
    let path = path(&state, &key, &name)?;
    let bytes = tokio::fs::read(path).await.map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => StatusCode::NOT_FOUND,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    })?;
    Ok((
        [
            (header::CONTENT_TYPE, "application/octet-stream"),
            (header::CONTENT_DISPOSITION, "attachment"),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
        ],
        bytes,
    )
        .into_response())
}
