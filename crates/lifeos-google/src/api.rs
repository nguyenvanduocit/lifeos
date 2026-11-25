//! Calls to Google's APIs as the signed-in owner.

use anyhow::{Result, bail};
use reqwest::{RequestBuilder, Response, StatusCode};
use serde_json::Value;

use crate::Google;

impl Google {
    /// Sends what `build` makes with a fresh access token, once more with a new token if Google rejects it.
    pub(crate) async fn send(&self, build: impl Fn(&reqwest::Client) -> RequestBuilder) -> Result<Response> {
        for attempt in 0..2 {
            let token = self.access_token().await?;
            let res = build(&self.http).bearer_auth(&token).send().await?;
            if res.status() == StatusCode::UNAUTHORIZED && attempt == 0 {
                self.invalidate().await;
                continue;
            }
            if !res.status().is_success() {
                bail!(failure(res).await);
            }
            return Ok(res);
        }
        bail!("Google rejected the sign-in; the owner signs in again on the Google page")
    }

    /// Like `send`, reading the answer as JSON (`null` for an empty one).
    pub(crate) async fn json(&self, build: impl Fn(&reqwest::Client) -> RequestBuilder) -> Result<Value> {
        let text = self.send(build).await?.text().await?;
        Ok(if text.trim().is_empty() { Value::Null } else { serde_json::from_str(&text)? })
    }
}

/// Google's own words for why a call failed.
async fn failure(res: Response) -> String {
    let status = res.status();
    let body: Value = res.json().await.unwrap_or_default();
    let message = body
        .pointer("/error/message")
        .or_else(|| body.get("error_description"))
        .and_then(|m| m.as_str())
        .unwrap_or_default();
    match (status, message) {
        (StatusCode::FORBIDDEN, m) if m.contains("insufficient") || m.contains("scope") => {
            "Google did not grant this service; the owner signs in again on the Google page and allows it".into()
        }
        (_, "") => format!("Google answered {status}"),
        (_, m) => format!("Google answered {status}: {m}"),
    }
}

/// Puts `segments` on `base` as path segments, escaped.
pub(crate) fn url(base: &str, segments: &[&str]) -> String {
    let mut url = url::Url::parse(base).expect("a Google API base URL");
    // A base ends in `/`: its empty last segment goes, or the path gets `//` and Google answers 404.
    url.path_segments_mut().expect("a base URL").pop_if_empty().extend(segments);
    url.to_string()
}

/// A count asked for, kept within what Google allows.
pub(crate) fn count(asked: Option<u32>, default: u32, most: u32) -> u32 {
    asked.unwrap_or(default).clamp(1, most)
}

#[cfg(test)]
mod tests {
    #[test]
    fn segments_join_the_base_with_one_slash_and_are_escaped() {
        assert_eq!(
            super::url("https://gmail.googleapis.com/gmail/v1/users/me/", &["messages", "18c/x"]),
            "https://gmail.googleapis.com/gmail/v1/users/me/messages/18c%2Fx"
        );
        assert_eq!(
            super::url("https://www.googleapis.com/calendar/v3/", &["calendars", "a@b.c", "events"]),
            "https://www.googleapis.com/calendar/v3/calendars/a@b.c/events"
        );
    }
}
