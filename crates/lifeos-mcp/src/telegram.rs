//! Native Telegram channel exposed through the same SDK manifest as packaged plugins.
use anyhow::Result;
use plugin_sdk::{Call, Definition, Effect, Error, Tool};
use schemars::JsonSchema;
use serde::Deserialize;
use std::path::Path;

pub const TOKEN: &str = "TELEGRAM_BOT_TOKEN";
pub const CHATS: &str = "TELEGRAM_ALLOWED_CHATS";

#[derive(Deserialize, Default)]
pub struct Channel {
    pub installed: bool,
    pub enabled: bool,
    pub chats: Vec<i64>,
}

pub fn channel(home: &Path) -> Result<Channel> {
    match std::fs::read(home.join(".lifeos/telegram.json")) {
        Ok(data) => Ok(serde_json::from_slice(&data)?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Channel::default()),
        Err(e) => Err(e.into()),
    }
}

#[derive(Deserialize, JsonSchema)]
struct Empty {}
#[derive(Deserialize, JsonSchema)]
struct Send {
    chat_id: i64,
    text: String,
}

fn chats(call: &Call) -> Result<Vec<i64>, Error> {
    Ok(call
        .var(CHATS)
        .split(',')
        .filter(|s| !s.is_empty())
        .map(str::parse)
        .collect::<Result<_, _>>()?)
}

pub fn definition() -> Definition {
    Definition {
        tools:vec![
            Tool::handler("channel_status", Effect::Read, |call, _:Empty| async move {
                Ok(serde_json::json!({"has_token":!call.var(TOKEN).is_empty(),"allowed_chats":chats(&call)?}).to_string())
            }),
            Tool::handler("send_message", Effect::Irreversible, |call,input:Send| async move {
                if !chats(&call)?.contains(&input.chat_id) { return Err("Chat is not allowed".into()); }
                if input.text.trim().is_empty() || input.text.encode_utf16().count()>4096 {return Err("Expected 1–4096 characters".into());}
                let token=call.var(TOKEN);
                if token.is_empty() { return Err("Share TELEGRAM_BOT_TOKEN with Assistant in Vault".into()); }
                let reply=reqwest::Client::builder().redirect(reqwest::redirect::Policy::none()).timeout(std::time::Duration::from_secs(15)).build()?
                    .post(format!("https://api.telegram.org/bot{token}/sendMessage"))
                    .json(&serde_json::json!({"chat_id":input.chat_id,"text":input.text})).send().await.map_err(reqwest::Error::without_url)?
                    .error_for_status().map_err(reqwest::Error::without_url)?.json::<serde_json::Value>().await.map_err(reqwest::Error::without_url)?;
                let id=reply.pointer("/result/message_id").and_then(|v|v.as_i64()).ok_or("Telegram returned no message ID")?;
                Ok(serde_json::json!({"message_id":id,"chat_id":input.chat_id}).to_string())
            })],
        ..Default::default()
    }.bind_manifest(include_str!("telegram-manifest.json")).expect("Telegram manifest")
}
