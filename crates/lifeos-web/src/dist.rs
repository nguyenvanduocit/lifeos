//! The built Vue app (`web/dist`, from `make web`), embedded in release
//! builds and read from disk in debug builds: every file as it is, and
//! `index.html` for any other path.

use axum::http::header::{CONTENT_SECURITY_POLICY, CONTENT_TYPE};
use axum::http::{HeaderValue, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "../../web/dist"]
#[allow_missing = true]
struct Dist;

/// Scripts and wasm from the app itself only. `connect-src https:` lets the
/// page dial the link wherever `/link.json` says it is (loopback, a tailnet).
const CSP: &str = "default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; \
                   img-src 'self' data: blob:; connect-src 'self' https:";

pub async fn serve(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let (name, file) = match Dist::get(path).filter(|_| !path.is_empty()) {
        Some(f) => (path, f),
        None => match Dist::get("index.html") {
            Some(f) => ("index.html", f),
            None => {
                return (
                    StatusCode::NOT_FOUND,
                    "this binary holds no web app: build it with `make web`\n",
                )
                    .into_response();
            }
        },
    };
    let mime = mime_guess::from_path(name).first_or_octet_stream();
    let mut res = (StatusCode::OK, file.data.into_owned()).into_response();
    let h = res.headers_mut();
    h.insert(CONTENT_SECURITY_POLICY, HeaderValue::from_static(CSP));
    if let Ok(v) = HeaderValue::from_str(mime.as_ref()) {
        h.insert(CONTENT_TYPE, v);
    }
    res
}
