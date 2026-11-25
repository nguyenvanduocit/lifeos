use std::collections::BTreeMap;
use std::process::ExitCode;

use link::{ErrorCode, Failure};
use plugin_sdk::{Call, Definition, Effect, ProcessAnswer, ProcessInput, Processor, Reading, Tool};
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
        let value = json!({ "delivery_id": delivery, "event": event, "payload": payload });
        let reading = |suffix: &str| Reading {
            key_suffix: suffix.into(),
            dedupe: delivery.into(),
            value: value.clone().into(),
        };
        let mut emits = vec![reading(event)];
        if event == "pull_request"
            && payload["action"] == "closed"
            && payload.pointer("/pull_request/merged") == Some(&Value::Bool(true))
        {
            emits.push(reading("pull_request_merged"));
        }
        Ok(ProcessAnswer {
            text: format!("Accepted GitHub {event}"),
            emits,
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

#[derive(Deserialize, JsonSchema)]
struct Recent {
    limit: Option<u32>,
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
    if std::env::args().nth(1).as_deref() == Some("--sign-notification") {
        use std::io::Read;
        let sign = || -> Result<String, plugin_sdk::Error> {
            let home = lifeos_core::default_home();
            let key = lifeos_core::Life::plugin_store(&home)?
                .integration_key(SECRET, None)?
                .ok_or("Webhook secret missing from Vault")?;
            let mut body = Vec::new();
            std::io::stdin().take(2_097_153).read_to_end(&mut body)?;
            if body.len() > 2_097_152 {
                return Err("Payload too large".into());
            }
            Ok(hex::encode(hmac::sign(
                &hmac::Key::new(hmac::HMAC_SHA256, key.as_bytes()),
                &body,
            )))
        };
        return match sign() {
            Ok(signature) => {
                println!("sha256={signature}");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("{error}");
                ExitCode::FAILURE
            }
        };
    }
    let history = std::env::args_os().nth(1).map(std::path::PathBuf::from);
    plugin_sdk::serve(Definition {
        processors: vec![Processor::new(
            "webhook",
            "Receive a signed GitHub event",
            webhook,
        )],
        tools: vec![Tool::handler("recent_events", Effect::Read,
            move |_, input: Recent| {
                let path = history.clone();
                async move {
                    let limit = input.limit.unwrap_or(20);
                    if !(1..=100).contains(&limit) { return Err("limit must be 1 to 100".into()); }
                    let path = path.ok_or("Configure the plugin argument {lifeos_db} to read signal history")?.with_file_name("plugin-events.sqlite");
                    let db = rusqlite::Connection::open_with_flags(path,rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
                    let mut query = db.prepare("SELECT event,at,subscribers FROM signal_history WHERE event GLOB 'plugin.github.*' ORDER BY id DESC LIMIT ?")?;
                    let events = query.query_map([limit], |r| Ok(json!({"event":r.get::<_,String>(0)?,"at_ms":r.get::<_,i64>(1)?,"subscribers":r.get::<_,i64>(2)?})))?
                        .collect::<Result<Vec<_>,_>>()?;
                    Ok(serde_json::to_string(&events)?)
                }
            })],
        ..Default::default()
    }.bind_manifest(include_str!("../manifest.json")).expect("package manifest"))
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

        for merged in [false, true] {
            let body = json!({"action": "closed", "pull_request": {"merged": merged}}).to_string();
            let mut signed = headers.clone();
            signed.insert(
                "x-hub-signature-256".into(),
                format!(
                    "sha256={}",
                    hex::encode(hmac::sign(&key, body.as_bytes()).as_ref())
                ),
            );
            let answer = Delivery {
                body,
                headers: signed,
            }
            .receive(secret)
            .unwrap();
            assert_eq!(answer.emits.len(), if merged { 2 } else { 1 });
            if merged {
                assert_eq!(answer.emits[1].key_suffix, "pull_request_merged");
                assert_eq!(answer.emits[1].dedupe, answer.emits[0].dedupe);
            }
        }

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
