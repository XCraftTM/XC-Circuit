//! The running app's end of the MCP server: a local connection that
//! `xc-circuit --mcp` passes agents' requests through. It listens on
//! 127.0.0.1 only, on a free port, and answers only who knows the token in
//! the app's data folder ([`super::info_file`]). Requests wait in a queue
//! the app answers between frames, with the plugins' tools.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::Arc;
use std::time::Duration;

use eframe::egui;
use serde_json::{Value, json};

/// How long an agent's request may wait for the app.
const ANSWER_WITHIN: Duration = Duration::from_secs(60);

/// A request from an agent, waiting for the app's answer.
pub struct Pending {
    pub request: Value,
    reply: Sender<Value>,
}

impl Pending {
    pub fn answer(self, value: Value) {
        let _ = self.reply.send(value);
    }
}

pub struct Bridge {
    rx: Receiver<Pending>,
    stop: Arc<AtomicBool>,
    agents: Arc<AtomicUsize>,
    token: String,
    file: PathBuf,
    pub port: u16,
}

impl Bridge {
    /// Listens on a free local port and writes how to reach it to the data
    /// folder. `ctx` is woken for each request.
    pub fn start(ctx: egui::Context) -> std::io::Result<Bridge> {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        listener.set_nonblocking(true)?;
        let port = listener.local_addr()?.port();
        let token = new_token();
        let file = super::info_file();
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&file, json!({ "port": port, "token": token, "pid": std::process::id() }).to_string())?;
        let (tx, rx) = channel();
        let stop = Arc::new(AtomicBool::new(false));
        let agents = Arc::new(AtomicUsize::new(0));
        let (stop2, agents2, token2) = (stop.clone(), agents.clone(), token.clone());
        std::thread::Builder::new().name("mcp-bridge".into()).spawn(move || {
            while !stop2.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let (tx, stop, agents, token, ctx) = (tx.clone(), stop2.clone(), agents2.clone(), token2.clone(), ctx.clone());
                        let _ = std::thread::Builder::new().name("mcp-agent".into()).spawn(move || serve(stream, &token, &tx, &stop, &agents, &ctx));
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(100)),
                    Err(_) => std::thread::sleep(Duration::from_millis(500)),
                }
            }
        })?;
        Ok(Bridge { rx, stop, agents, token, file, port })
    }

    /// The requests that came in since the last call.
    pub fn poll(&self) -> Vec<Pending> {
        self.rx.try_iter().collect()
    }

    /// How many agents are connected.
    pub fn agents(&self) -> usize {
        self.agents.load(Ordering::Relaxed)
    }
}

impl Drop for Bridge {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        // Only our own file: another copy of the app may have written its own since.
        let ours = std::fs::read_to_string(&self.file).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()).is_some_and(|v| v["token"] == self.token);
        if ours {
            let _ = std::fs::remove_file(&self.file);
        }
    }
}

/// One agent's connection: its token first, then a request per line.
fn serve(stream: TcpStream, token: &str, tx: &Sender<Pending>, stop: &AtomicBool, agents: &AtomicUsize, ctx: &egui::Context) {
    let _ = stream.set_nonblocking(false);
    let Ok(mut out) = stream.try_clone() else { return };
    let mut lines = BufReader::new(stream).lines();
    let hello: Option<Value> = lines.next().and_then(Result::ok).and_then(|l| serde_json::from_str(&l).ok());
    if hello.as_ref().and_then(|h| h["token"].as_str()) != Some(token) {
        let _ = writeln!(out, "{}", json!({ "error": "wrong token" }));
        return;
    }
    if writeln!(out, "{}", json!({ "ok": true, "version": env!("CARGO_PKG_VERSION") })).is_err() {
        return;
    }
    agents.fetch_add(1, Ordering::Relaxed);
    ctx.request_repaint();
    for line in lines {
        let Ok(line) = line else { break };
        if stop.load(Ordering::Relaxed) {
            break;
        }
        let answer = match serde_json::from_str::<Value>(&line) {
            Ok(request) => {
                let (reply, wait) = channel();
                if tx.send(Pending { request, reply }).is_err() {
                    break;
                }
                ctx.request_repaint();
                wait.recv_timeout(ANSWER_WITHIN).unwrap_or_else(|_| json!({ "error": "XC Circuit didn't answer in time (is a dialog open?)" }))
            }
            Err(e) => json!({ "error": format!("not JSON: {e}") }),
        };
        if writeln!(out, "{answer}").is_err() {
            break;
        }
    }
    agents.fetch_sub(1, Ordering::Relaxed);
    ctx.request_repaint();
}

/// A secret for the connection: 128 random bits as hex.
fn new_token() -> String {
    use std::hash::{BuildHasher, Hasher};
    let part = |salt: u64| {
        // RandomState is seeded from the system's randomness.
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u64(salt);
        h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos());
        h.finish()
    };
    format!("{:016x}{:016x}", part(1), part(2))
}
