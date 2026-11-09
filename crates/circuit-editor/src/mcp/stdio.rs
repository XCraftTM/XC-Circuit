//! `xc-circuit --mcp`: the MCP server AI clients start. It speaks MCP
//! (JSON-RPC, one message per line) on standard input and output, answers
//! the protocol itself and passes tool listings and calls to the running app
//! ([`super::bridge`]). While the app isn't open it offers one tool, which
//! opens it. When the app opens or closes, it tells the client that its
//! tools changed, so the client lists them again.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

/// MCP versions this server speaks, newest first.
const VERSIONS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];

/// The tool offered while XC Circuit isn't reachable.
const OPEN_TOOL: &str = "open_xc_circuit";

/// What agents are told about XC Circuit when they connect.
const INSTRUCTIONS: &str = "XC Circuit is a visual editor for Discord and Fluxer bots. A project has workspaces; a workspace has blocks wired together. \
Events (such as discord.on_slash_command) start a flow; a block's `next` (or another flow such as `failed`) runs the next blocks through their `in` pin. \
Values go from a block's outputs into other blocks' fields, which then become pins. Value blocks (Get Property, Math, Compare …) can be put in the flow too, so they run at that point, once; left out of it, they're worked out when read. Wires are written `block.pin > block.pin`, such as `1.next > 2.in` or `3.result > 4.text`. \
Text fields can hold `${…}` with JavaScript and values from earlier blocks. Start with get_project, then get_workspace; look up block types with find_block_types and \
get_block_type before adding blocks. Every change shows in the editor at once and can be undone there. The bot's token is never shown to you.";

/// How often the server looks whether XC Circuit opened or closed.
const WATCH_EVERY: Duration = Duration::from_secs(2);

/// Runs the server until the client closes its input.
pub fn run() -> i32 {
    let stdin = std::io::stdin();
    // Shared with the watcher, which writes its notifications between answers.
    let out = Arc::new(Mutex::new(std::io::stdout()));
    // Whether the app could be reached when the client last listed the tools;
    // None until it lists, and after a change was told. (Reachable rather than
    // which tools it got: a port that answers but isn't the app can't loop.)
    let listed: Arc<Mutex<Option<bool>>> = Arc::default();
    {
        let (out, listed) = (out.clone(), listed.clone());
        let _ = std::thread::Builder::new().name("mcp-watch".into()).spawn(move || watch(&out, &listed));
    }
    let mut app = AppLink::default();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(&line) {
            Ok(message) => {
                let reply = handle(&message, &mut app);
                if message["method"] == "tools/list" {
                    *listed.lock().unwrap_or_else(|e| e.into_inner()) = Some(app_reachable());
                }
                reply
            }
            Err(e) => Some(json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32700, "message": format!("Parse error: {e}") } })),
        };
        if let Some(reply) = reply
            && !send(&out, &reply)
        {
            break;
        }
    }
    0
}

/// Writes one message as a line. False when the client is gone.
fn send(out: &Mutex<std::io::Stdout>, message: &Value) -> bool {
    let mut out = out.lock().unwrap_or_else(|e| e.into_inner());
    writeln!(out, "{message}").is_ok() && out.flush().is_ok()
}

/// The notification that makes a client list the tools again.
fn list_changed() -> Value {
    json!({ "jsonrpc": "2.0", "method": "notifications/tools/list_changed" })
}

/// Every few seconds: when XC Circuit opened or closed since the client last
/// listed the tools, tell it so, once.
fn watch(out: &Mutex<std::io::Stdout>, listed: &Mutex<Option<bool>>) {
    loop {
        std::thread::sleep(WATCH_EVERY);
        let was = *listed.lock().unwrap_or_else(|e| e.into_inner());
        let Some(was) = was else { continue };
        if app_reachable() != was {
            // Once: listing again sets `listed` anew.
            *listed.lock().unwrap_or_else(|e| e.into_inner()) = None;
            if !send(out, &list_changed()) {
                return;
            }
        }
    }
}

/// Whether the running app answers on the port it wrote down. A connection
/// without its token is turned away quietly, so this doesn't count as an agent.
fn app_reachable() -> bool {
    let info: Option<Value> = std::fs::read_to_string(super::info_file()).ok().and_then(|t| serde_json::from_str(&t).ok());
    let Some(port) = info.and_then(|i| i["port"].as_u64()).and_then(|p| u16::try_from(p).ok()) else { return false };
    TcpStream::connect_timeout(&([127, 0, 0, 1], port).into(), Duration::from_millis(500)).is_ok()
}

/// What the server needs of XC Circuit: its tools, and opening it.
pub trait Backend {
    fn list(&mut self) -> Result<Vec<Value>, String>;
    fn call(&mut self, name: &str, args: &Value) -> Result<Value, String>;
    fn open(&mut self) -> Result<String, String>;
}

/// Answers one MCP message (None for notifications).
pub fn handle(message: &Value, app: &mut dyn Backend) -> Option<Value> {
    let id = message.get("id").cloned()?;
    let method = message["method"].as_str().unwrap_or("");
    let params = &message["params"];
    let result = match method {
        "initialize" => {
            let asked = params["protocolVersion"].as_str().unwrap_or("");
            let version = VERSIONS.iter().find(|v| **v == asked).copied().unwrap_or(VERSIONS[0]);
            Ok(json!({
                "protocolVersion": version,
                // Tools come and go with the app: clients are told (see `watch`).
                "capabilities": { "tools": { "listChanged": true } },
                "serverInfo": { "name": super::SERVER_NAME, "title": "XC Circuit", "version": env!("CARGO_PKG_VERSION") },
                "instructions": INSTRUCTIONS,
            }))
        }
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": app.list().unwrap_or_else(|_| vec![open_tool()]) })),
        "tools/call" => {
            let name = params["name"].as_str().unwrap_or("");
            let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
            let answer = if name == OPEN_TOOL { app.open().map(Value::String) } else { app.call(name, &args) };
            Ok(match answer {
                // The app's answer is a tool result already.
                Ok(Value::Object(result)) if result.contains_key("content") => Value::Object(result),
                other => super::tool_result(other),
            })
        }
        "resources/list" => Ok(json!({ "resources": [] })),
        "prompts/list" => Ok(json!({ "prompts": [] })),
        other => Err(json!({ "code": -32601, "message": format!("Method not found: {other}") })),
    };
    Some(match result {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err(error) => json!({ "jsonrpc": "2.0", "id": id, "error": error }),
    })
}

fn open_tool() -> Value {
    json!({
        "name": OPEN_TOOL,
        "description": "XC Circuit isn't open (or its AI agents plugin is turned off), so its tools aren't available. This opens XC Circuit and waits for it; then list the tools again.",
        "inputSchema": { "type": "object", "properties": {} },
    })
}

/// The connection to the running app, made when first needed and again after it closed.
#[derive(Default)]
struct AppLink {
    stream: Option<(TcpStream, BufReader<TcpStream>)>,
}

impl AppLink {
    fn connect() -> Result<(TcpStream, BufReader<TcpStream>), String> {
        let not_open = || "XC Circuit isn't open, or its AI agents plugin is turned off (Plugins in the side bar). Use open_xc_circuit to open it.".to_string();
        let info: Value = std::fs::read_to_string(super::info_file()).ok().and_then(|t| serde_json::from_str(&t).ok()).ok_or_else(not_open)?;
        let port = info["port"].as_u64().and_then(|p| u16::try_from(p).ok()).ok_or_else(not_open)?;
        let mut stream = TcpStream::connect_timeout(&([127, 0, 0, 1], port).into(), Duration::from_secs(2)).map_err(|_| not_open())?;
        // The app greets at once; something else on its old port may never answer.
        let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
        writeln!(stream, "{}", json!({ "token": info["token"] })).map_err(|e| e.to_string())?;
        let mut reader = BufReader::new(stream.try_clone().map_err(|e| e.to_string())?);
        let mut hello = String::new();
        reader.read_line(&mut hello).map_err(|_| not_open())?;
        let hello: Value = serde_json::from_str(&hello).map_err(|_| not_open())?;
        if hello["ok"] != true {
            return Err(not_open());
        }
        // A tool may take a while (a dialog in the app): calls get longer.
        let _ = stream.set_read_timeout(Some(Duration::from_secs(90)));
        Ok((stream, reader))
    }

    /// Sends a request and reads the answer, connecting again once if the app went away.
    fn ask(&mut self, request: Value) -> Result<Value, String> {
        for attempt in 0..2 {
            if self.stream.is_none() {
                self.stream = Some(Self::connect()?);
            }
            let (stream, reader) = self.stream.as_mut().expect("connected");
            let mut line = String::new();
            let sent = writeln!(stream, "{request}").is_ok() && reader.read_line(&mut line).is_ok_and(|n| n > 0);
            if sent {
                let answer: Value = serde_json::from_str(&line).map_err(|e| e.to_string())?;
                if let Some(e) = answer["error"].as_str() {
                    return Err(e.to_string());
                }
                return Ok(answer);
            }
            self.stream = None;
            if attempt == 1 {
                break;
            }
        }
        Err("The connection to XC Circuit was lost.".into())
    }
}

impl Backend for AppLink {
    fn list(&mut self) -> Result<Vec<Value>, String> {
        let answer = self.ask(json!({ "op": "list" }))?;
        Ok(answer["tools"].as_array().cloned().unwrap_or_default())
    }

    fn call(&mut self, name: &str, args: &Value) -> Result<Value, String> {
        Ok(self.ask(json!({ "op": "call", "name": name, "args": args }))?["result"].clone())
    }

    fn open(&mut self) -> Result<String, String> {
        if self.list().is_ok() {
            return Ok("XC Circuit is open: list the tools again to use them.".into());
        }
        let exe = super::install::program()?;
        std::process::Command::new(&exe).spawn().map_err(|e| format!("Couldn't start {}: {e}", exe.display()))?;
        let until = Instant::now() + Duration::from_secs(45);
        while Instant::now() < until {
            std::thread::sleep(Duration::from_millis(500));
            if self.list().is_ok() {
                return Ok("XC Circuit is open: list the tools again to use them.".into());
            }
        }
        Err("XC Circuit started, but its tools didn't come up. Is the AI agents plugin turned on (Plugins in the side bar)?".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// XC Circuit as the tests want it: open with one tool, or not.
    struct Fake {
        open: bool,
    }

    impl Backend for Fake {
        fn list(&mut self) -> Result<Vec<Value>, String> {
            if self.open { Ok(vec![json!({ "name": "get_project", "description": "", "inputSchema": { "type": "object" } })]) } else { Err("not open".into()) }
        }
        fn call(&mut self, name: &str, args: &Value) -> Result<Value, String> {
            if !self.open {
                return Err("not open".into());
            }
            Ok(json!({ "content": [{ "type": "text", "text": format!("{name} {args}") }] }))
        }
        fn open(&mut self) -> Result<String, String> {
            self.open = true;
            Ok("opened".into())
        }
    }

    fn ask(app: &mut Fake, method: &str, params: Value) -> Value {
        handle(&json!({ "jsonrpc": "2.0", "id": 7, "method": method, "params": params }), app).expect("an answer")
    }

    #[test]
    fn speaks_mcp() {
        let mut app = Fake { open: false };
        let init = ask(&mut app, "initialize", json!({ "protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": { "name": "test" } }));
        assert_eq!(init["id"], 7);
        assert_eq!(init["result"]["protocolVersion"], "2025-03-26", "the client's version when it's one we speak");
        assert_eq!(init["result"]["capabilities"]["tools"]["listChanged"], true, "clients hear when the app opens or closes");
        assert_eq!(ask(&mut app, "initialize", json!({ "protocolVersion": "1999-01-01" }))["result"]["protocolVersion"], VERSIONS[0]);
        assert!(handle(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }), &mut app).is_none(), "notifications get no answer");
        assert_eq!(ask(&mut app, "ping", json!({}))["result"], json!({}));
        assert_eq!(ask(&mut app, "nope", json!({}))["error"]["code"], -32601);

        // Not open: only the tool that opens it.
        let tools = ask(&mut app, "tools/list", json!({}));
        assert_eq!(tools["result"]["tools"][0]["name"], OPEN_TOOL);
        let failed = ask(&mut app, "tools/call", json!({ "name": "get_project" }));
        assert_eq!(failed["result"]["isError"], true, "a failed call is a tool error, not a protocol error");
        let opened = ask(&mut app, "tools/call", json!({ "name": OPEN_TOOL }));
        assert_eq!(opened["result"]["content"][0]["text"], "opened");

        // Open: the app's tools, and its answers as they are.
        assert_eq!(ask(&mut app, "tools/list", json!({}))["result"]["tools"][0]["name"], "get_project");
        let called = ask(&mut app, "tools/call", json!({ "name": "get_project", "arguments": { "a": 1 } }));
        assert_eq!(called["result"]["content"][0]["text"], "get_project {\"a\":1}");
    }

    /// The notification that makes a client list the tools again.
    #[test]
    fn tells_of_changed_tools() {
        assert_eq!(list_changed(), json!({ "jsonrpc": "2.0", "method": "notifications/tools/list_changed" }));
        assert!(list_changed().get("id").is_none(), "a notification, which gets no answer");
    }

    #[test]
    fn tool_answers_become_text() {
        assert_eq!(super::super::tool_result(Ok(json!("hi")))["content"][0]["text"], "hi");
        assert!(super::super::tool_result(Ok(json!({ "a": 1 })))["content"][0]["text"].as_str().unwrap().contains("\"a\": 1"));
        assert_eq!(super::super::tool_result(Err("no".into()))["isError"], true);
    }
}
