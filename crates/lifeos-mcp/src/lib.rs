//! lifeos as an MCP server over stdio (newline-delimited JSON-RPC 2.0), so any
//! MCP client — Claude Code, Paseo, Claude Desktop — can use the same tools
//! the lifeos agent uses.
//!
//! ```sh
//! claude mcp add lifeos -- lifeos mcp
//! ```

pub mod plugin;
pub mod telegram;

use std::io::{BufRead, Write};

use anyhow::Result;
use lifeos_core::{Life, tools};
use serde_json::{Value, json};

const PROTOCOL_VERSION: &str = "2025-06-18";

pub fn serve_stdio(life: &mut Life) -> Result<()> {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        if let Some(response) = handle_line(life, &line) {
            writeln!(stdout, "{response}")?;
            stdout.flush()?;
        }
    }
    Ok(())
}

/// One JSON-RPC message in, at most one response out (none for notifications).
pub fn handle_line(life: &mut Life, line: &str) -> Option<Value> {
    let msg: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(e) => return Some(error(Value::Null, -32700, &format!("parse error: {e}"))),
    };
    let id = msg.get("id").cloned();
    let method = msg.get("method").and_then(Value::as_str).unwrap_or_default();
    let params = msg.get("params").cloned().unwrap_or(Value::Null);
    let id = id?; // notifications get no reply

    let result = match method {
        "initialize" => {
            let version = params
                .get("protocolVersion")
                .and_then(Value::as_str)
                .unwrap_or(PROTOCOL_VERSION);
            json!({
                "protocolVersion": version,
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "lifeos", "version": env!("CARGO_PKG_VERSION") },
                "instructions": "Tools over the user's lifeos: tasks, goals, habits, people, money, health, journal, notes, reviews and an inbox. Call `dashboard` for an overview."
            })
        }
        "ping" => json!({}),
        "tools/list" => json!({
            "tools": tools::specs().into_iter().map(|s| json!({
                "name": s.name,
                "description": s.description,
                "inputSchema": s.input_schema,
                "annotations": { "readOnlyHint": s.read_only },
            })).collect::<Vec<_>>()
        }),
        "tools/call" => {
            let name = params.get("name").and_then(Value::as_str).unwrap_or_default();
            let args = params.get("arguments").cloned().unwrap_or(Value::Null);
            match tools::call(life, lifeos_core::life::ASSISTANT, name, args) {
                Ok(value) => json!({
                    "content": [{ "type": "text", "text": value.to_string() }],
                }),
                Err(err) => json!({
                    "content": [{ "type": "text", "text": format!("{err:#}") }],
                    "isError": true,
                }),
            }
        }
        _ => return Some(error(id, -32601, &format!("method not found: {method}"))),
    };
    Some(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
}

fn error(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initialize_list_and_call() {
        let dir = tempfile::tempdir().unwrap();
        let mut life = Life::open_with_memory_db(dir.path()).unwrap();

        let init = handle_line(&mut life, r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}"#).unwrap();
        assert_eq!(init["result"]["serverInfo"]["name"], "lifeos");
        assert!(handle_line(&mut life, r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#).is_none());

        let list = handle_line(&mut life, r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#).unwrap();
        assert_eq!(list["result"]["tools"].as_array().unwrap().len(), tools::specs().len());

        let call = handle_line(&mut life, r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"log_habit","arguments":{"habit":"thiền"}}}"#).unwrap();
        assert!(call["result"].get("isError").is_none(), "{call}");
        let bad = handle_line(&mut life, r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"log_habit","arguments":{}}}"#).unwrap();
        assert_eq!(bad["result"]["isError"], true);

        let unknown = handle_line(&mut life, r#"{"jsonrpc":"2.0","id":5,"method":"nope"}"#).unwrap();
        assert_eq!(unknown["error"]["code"], -32601);
    }
}
