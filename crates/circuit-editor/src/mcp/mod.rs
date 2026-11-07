//! XC Circuit as an MCP server, for AI agents such as Claude.
//!
//! An AI client starts `xc-circuit --mcp` ([`stdio`]), which speaks MCP on
//! its standard input and output. It passes the agent's tool calls to the
//! running app over a local connection ([`bridge`]), where plugins answer
//! them with their `xc.tool`s (the bundled AI agents plugin has the usual
//! ones). [`install`] adds that command to AI clients' settings.
//!
//! Between the two, one JSON object per line: first `{ "token" }` (from the
//! file the app writes, [`info_file`]), then `{ "op": "list" }` (answered
//! with `{ "tools" }`) or `{ "op": "call", "name", "args" }` (answered with
//! `{ "result" }`, an MCP tool result).

pub mod bridge;
pub mod install;
pub mod stdio;

use std::path::PathBuf;

use serde_json::{Value, json};

/// The name the server has in AI clients' settings.
pub const SERVER_NAME: &str = "xc-circuit";

/// Where the running app says how to reach it: `{ port, token, pid }`.
pub fn info_file() -> PathBuf {
    crate::bot::app_data_dir().join("mcp.json")
}

/// A tool's answer as MCP wants it: text content, and whether it failed.
pub fn tool_result(answer: Result<Value, String>) -> Value {
    match answer {
        Ok(Value::String(text)) => json!({ "content": [{ "type": "text", "text": text }] }),
        Ok(Value::Null) => json!({ "content": [{ "type": "text", "text": "Done." }] }),
        Ok(other) => json!({ "content": [{ "type": "text", "text": serde_json::to_string_pretty(&other).unwrap_or_default() }] }),
        Err(e) => json!({ "content": [{ "type": "text", "text": e }], "isError": true }),
    }
}
