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
    /// The agent was told it took too long, so it must not be done after all.
    given_up: Arc<AtomicBool>,
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
        waiting(&self.rx)
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
            Ok(request) => match ask(tx, request, ANSWER_WITHIN, || ctx.request_repaint()) {
                Some(answer) => answer,
                None => break,
            },
            Err(e) => json!({ "error": format!("not JSON: {e}") }),
        };
        if writeln!(out, "{answer}").is_err() {
            break;
        }
    }
    agents.fetch_sub(1, Ordering::Relaxed);
    ctx.request_repaint();
}

/// Queues a request for the app (woken with `wake`) and waits for its answer,
/// at most `within`. None when the app is gone. A request the app didn't get
/// to in time is given up: the agent hears that it failed and may ask again,
/// so doing it later as well would do it twice.
fn ask(tx: &Sender<Pending>, request: Value, within: Duration, wake: impl Fn()) -> Option<Value> {
    let (reply, wait) = channel();
    let given_up = Arc::new(AtomicBool::new(false));
    tx.send(Pending { request, reply, given_up: given_up.clone() }).ok()?;
    wake();
    Some(wait.recv_timeout(within).unwrap_or_else(|_| {
        given_up.store(true, Ordering::Relaxed);
        json!({ "error": "XC Circuit didn't answer in time (is a dialog open?). It wasn't done: try again." })
    }))
}

/// The requests waiting for the app, without the ones given up meanwhile.
fn waiting(rx: &Receiver<Pending>) -> Vec<Pending> {
    rx.try_iter().filter(|p| !p.given_up.load(Ordering::Relaxed)).collect()
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A request the app answers in time gets its answer; one it didn't get
    /// to (a dialog was open) is told so and is not done later.
    #[test]
    fn a_request_given_up_is_not_done_later() {
        let (tx, rx) = channel();
        let app = std::thread::spawn(move || {
            let pending: Pending = rx.recv().unwrap();
            assert_eq!(pending.request["op"], "list");
            pending.answer(json!({ "tools": [] }));
            rx
        });
        let woken = AtomicBool::new(false);
        let answer = ask(&tx, json!({ "op": "list" }), Duration::from_secs(10), || woken.store(true, Ordering::Relaxed));
        assert_eq!(answer, Some(json!({ "tools": [] })));
        assert!(woken.load(Ordering::Relaxed), "the app is woken for it");
        let rx = app.join().unwrap();

        // Nobody answers: the agent hears it failed, and the app never sees it.
        let late = ask(&tx, json!({ "op": "call", "name": "add_block" }), Duration::from_millis(20), || {}).unwrap();
        assert!(late["error"].as_str().unwrap().contains("wasn't done"), "{late}");
        tx.send(Pending { request: json!({ "op": "list" }), reply: channel().0, given_up: Arc::default() }).unwrap();
        let left = waiting(&rx);
        assert_eq!(left.len(), 1, "only the one still waited for");
        assert_eq!(left[0].request["op"], "list");

        // The app closed: nothing to wait for.
        drop(rx);
        assert_eq!(ask(&tx, json!({}), Duration::from_secs(10), || {}), None);
    }
}
