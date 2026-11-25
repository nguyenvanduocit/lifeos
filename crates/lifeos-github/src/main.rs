use std::collections::BTreeMap;
use std::process::ExitCode;

use link::{ErrorCode, Failure};
use plugin_sdk::{
    Call, Definition, ProcessAnswer, ProcessInput, Processor, Reading, Slot, Ui, Var,
};
use ring::hmac;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

const SECRET: &str = "GITHUB_WEBHOOK_SECRET";

#[derive(Deserialize, JsonSchema)]
struct Delivery {
    body: String,
    headers: BTreeMap<String, String>,
}

impl Delivery {
    fn header(&self, name: &str) -> &str {
        self.headers.get(name).map_or("", String::as_str)
    }

    fn receive(self, secret: &str) -> Result<ProcessAnswer, Failure> {
        if secret.is_empty() {
            return Err(Failure::new(
                ErrorCode::Forbidden,
                "GitHub webhooks are disabled",
            ));
        }
        let signature = self
            .header("x-hub-signature-256")
            .strip_prefix("sha256=")
            .and_then(|tag| hex::decode(tag).ok())
            .ok_or_else(invalid_signature)?;
        let key = hmac::Key::new(hmac::HMAC_SHA256, secret.as_bytes());
        hmac::verify(&key, self.body.as_bytes(), &signature).map_err(|_| invalid_signature())?;

        let event = self.header("x-github-event");
        let delivery = self.header("x-github-delivery");
        if event.is_empty()
            || event.len() > 64
            || !event
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
            || delivery.is_empty()
            || delivery.len() > 64
            || !delivery.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-')
        {
            return Err(Failure::new(
                ErrorCode::InvalidArgument,
                "Invalid GitHub delivery headers",
            ));
        }
        let payload = serde_json::from_str::<Value>(&self.body)
            .ok()
            .filter(Value::is_object)
            .ok_or_else(|| {
                Failure::new(ErrorCode::InvalidArgument, "Expected a GitHub JSON object")
            })?;
        Ok(ProcessAnswer {
            text: format!("Accepted GitHub {event}"),
            emits: vec![Reading {
                key_suffix: event.into(),
                dedupe: delivery.into(),
                value: json!({ "delivery_id": delivery, "event": event, "payload": payload })
                    .into(),
            }],
        })
    }
}

fn invalid_signature() -> Failure {
    Failure::new(ErrorCode::Forbidden, "Invalid GitHub webhook signature")
}

async fn webhook(
    call: Call,
    input: ProcessInput<Delivery>,
) -> Result<ProcessAnswer, plugin_sdk::Error> {
    Ok(input.params.receive(call.var(SECRET))?)
}

fn main() -> ExitCode {
    if std::env::args().nth(1).as_deref() == Some("--mcp-config") {
        println!(
            "{}",
            json!({
                "url": "https://api.githubcopilot.com/mcp/",
                "headers": { "Authorization": "Bearer ${GITHUB_TOKEN}" }
            })
        );
        return ExitCode::SUCCESS;
    }
    plugin_sdk::serve(Definition {
        label: "GitHub".into(),
        capability: "github".into(),
        vars: vec![Var {
            name: SECRET.into(),
            what: "Shared secret for GitHub webhook signatures".into(),
            secret: true,
            optional: true,
            ..Default::default()
        }],
        processors: vec![Processor::new(
            "webhook",
            "Receive a signed GitHub event",
            webhook,
        )],
        ui: Some(Ui {
            entry: "github.html".into(),
            slots: vec![Slot {
                slot: "page".into(),
                id: "setup".into(),
                title: "GitHub setup".into(),
                expose: String::new(),
            }],
        }),
        assets: [("github.html".into(), include_bytes!("github.html").to_vec())].into(),
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn webhook_authenticates_original_bytes_before_emitting_an_event() {
        let secret = "fixture-secret";
        let body = "{ \"action\": \"opened\", \"title\": \"Cà phê\" }";
        let key = hmac::Key::new(hmac::HMAC_SHA256, secret.as_bytes());
        let signature = hex::encode(hmac::sign(&key, body.as_bytes()).as_ref());
        let headers = BTreeMap::from([
            ("x-hub-signature-256".into(), format!("sha256={signature}")),
            ("x-github-event".into(), "pull_request".into()),
            (
                "x-github-delivery".into(),
                "ef671af2-7264-40af-88cd-1ccdc5138f40".into(),
            ),
        ]);
        let accepted = Delivery {
            body: body.into(),
            headers: headers.clone(),
        }
        .receive(secret)
        .unwrap();
        assert_eq!(accepted.emits[0].key_suffix, "pull_request");
        assert_eq!(accepted.emits[0].value.0["payload"]["title"], "Cà phê");

        let tampered = Delivery {
            body: format!("{body}\n"),
            headers,
        };
        assert_eq!(
            tampered.receive(secret).unwrap_err().code,
            ErrorCode::Forbidden
        );
    }
}
