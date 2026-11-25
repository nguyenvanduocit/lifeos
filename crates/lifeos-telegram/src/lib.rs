//! Telegram bot over the Bot API, by long polling (no public URL needed).
//!
//! Only chats listed in `allowed_chats` are served: the bot reads and writes
//! someone's whole life, so an unknown chat gets only its own id back (to
//! help setup) and nothing else.

use std::time::Duration;

use anyhow::{Context, Result, bail};
use lifeos_agent::{Agents, AgentsConfig, SharedLife, owner};
use lifeos_wire::ASSISTANT;
use serde::Deserialize;
use serde_json::json;

const MAX_MESSAGE_CHARS: usize = 4000; // Telegram's limit is 4096

pub struct BotConfig {
    pub token: String,
    pub allowed_chats: Vec<i64>,
    pub agent: AgentsConfig,
}

#[derive(Deserialize)]
struct ApiResponse<T> {
    ok: bool,
    result: Option<T>,
    description: Option<String>,
}

#[derive(Deserialize)]
struct Update {
    update_id: i64,
    message: Option<Message>,
}

#[derive(Deserialize)]
struct Message {
    chat: Chat,
    text: Option<String>,
}

#[derive(Deserialize)]
struct Chat {
    id: i64,
}

struct Bot {
    http: reqwest::Client,
    base: String,
}

impl Bot {
    async fn call<T: for<'de> Deserialize<'de>>(&self, method: &str, body: serde_json::Value) -> Result<T> {
        let res: ApiResponse<T> = self
            .http
            .post(format!("{}/{method}", self.base))
            .json(&body)
            .send()
            .await.map_err(reqwest::Error::without_url)?
            .json()
            .await.map_err(reqwest::Error::without_url)?;
        if !res.ok {
            bail!("telegram {method}: {}", res.description.unwrap_or_default());
        }
        res.result.context("telegram returned no result")
    }

    async fn send(&self, chat_id: i64, text: &str) -> Result<()> {
        for chunk in split(text, MAX_MESSAGE_CHARS) {
            let _: serde_json::Value = self
                .call("sendMessage", json!({ "chat_id": chat_id, "text": chunk }))
                .await?;
        }
        Ok(())
    }

    async fn typing(&self, chat_id: i64) {
        let _ = self
            .call::<serde_json::Value>("sendChatAction", json!({ "chat_id": chat_id, "action": "typing" }))
            .await;
    }
}

pub async fn run(life: SharedLife, config: BotConfig) -> Result<()> {
    let agents = Agents::start(life.clone(), config.agent.clone()).await?;
    poll(&agents, &config.token, &config.allowed_chats).await
}

pub async fn poll(agents: &Agents, token: &str, allowed_chats: &[i64]) -> Result<()> {
    if allowed_chats.is_empty() {
        tracing::warn!("no allowed chats: the bot will only tell each chat its id");
    }
    let bot = Bot {
        http: reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()?,
        base: format!("https://api.telegram.org/bot{token}"),
    };
    let mut offset = 0i64;
    tracing::info!("lifeos telegram bot is polling");

    loop {
        let updates: Vec<Update> = match bot
            .call("getUpdates", json!({ "offset": offset, "timeout": 50, "allowed_updates": ["message"] }))
            .await
        {
            Ok(u) => u,
            Err(err) => {
                tracing::warn!("getUpdates failed: {err:#}");
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
        };
        for update in updates {
            offset = update.update_id + 1;
            let Some(Message { chat, text: Some(text) }) = update.message else { continue };
            if let Err(err) = handle(&bot, agents, allowed_chats, chat.id, text.trim()).await {
                tracing::warn!("chat {}: {err:#}", chat.id);
                let _ = bot.send(chat.id, &format!("Lỗi: {err:#}")).await;
            }
        }
    }
}

/// One message from an allowed chat, to the Assistant: the same conversation the web app shows.
async fn handle(bot: &Bot, agents: &Agents, allowed_chats: &[i64], chat_id: i64, text: &str) -> Result<()> {
    if !allowed_chats.contains(&chat_id) {
        return bot
            .send(
                chat_id,
                &format!("Chat này chưa được phép dùng lifeos. Chat id: {chat_id}\nThêm ID này vào danh sách chat được phép trong cấu hình Telegram."),
            )
            .await;
    }
    match text {
        "/new" => {
            agents.restart(ASSISTANT).await?;
            return bot.send(chat_id, "Đã bắt đầu phiên mới.").await;
        }
        "/start" => {
            return bot
                .send(chat_id, "Chào bạn, mình là lifeos. Cứ kể cho mình nghe hôm nay của bạn, hoặc hỏi nên làm gì tiếp theo.\n/new: bắt đầu phiên mới")
                .await;
        }
        _ => {}
    }
    bot.typing(chat_id).await;
    let ask = agents.ask(ASSISTANT, text, owner());
    tokio::pin!(ask);
    let reply = loop {
        // Telegram's typing indicator lasts ~5 s; keep it up while the agent works.
        tokio::select! {
            r = &mut ask => break r?,
            _ = tokio::time::sleep(Duration::from_secs(4)) => bot.typing(chat_id).await,
        }
    };
    let text = reply.shown();
    let text = if text.trim().is_empty() { "(không có trả lời)" } else { text.as_str() };
    bot.send(chat_id, text).await
}

/// Splits on line breaks where possible, never inside a UTF-8 character.
fn split(text: &str, max_chars: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current = String::new();
    for line in text.split_inclusive('\n') {
        if current.chars().count() + line.chars().count() > max_chars && !current.is_empty() {
            chunks.push(std::mem::take(&mut current));
        }
        if line.chars().count() > max_chars {
            let chars: Vec<char> = line.chars().collect();
            for part in chars.chunks(max_chars) {
                chunks.push(part.iter().collect());
            }
        } else {
            current.push_str(line);
        }
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    chunks
}

#[cfg(test)]
mod tests {
    use super::split;

    #[test]
    fn split_respects_lines_and_limit() {
        assert_eq!(split("ngắn", 10), vec!["ngắn"]);
        assert_eq!(split("dòng một\ndòng hai\n", 10), vec!["dòng một\n", "dòng hai\n"]);
        let long = "ồ".repeat(25);
        let parts = split(&long, 10);
        assert_eq!(parts.len(), 3);
        assert!(parts.iter().all(|p| p.chars().count() <= 10));
    }
}
