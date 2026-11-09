//! The running app's end of the MCP server: a local connection that
//! `xc-circuit --mcp` passes agents' requests through. It listens on
//! 127.0.0.1 only, on a free port, and answers only who knows the token in
//! the app's data folder ([`super::info_file`]). Requests wait in a queue
//! the app answers between frames, with the plugins' tools.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::Arc;
use std::time::Duration;

use eframe::egui;
use serde_json::{Value, json};

/// How long an agent's request may wait for the app.
const ANSWER_WITHIN: Duration = Duration::from_secs(60);
/// How long a new connection has to say its token. Any program on this
/// computer can connect; one that then says nothing is let go.
const HELLO_WITHIN: Duration = Duration::from_secs(5);
/// The longest line taken: the token's, and a request's (a whole workspace
/// or a long script fits many times over).
const HELLO_MAX: usize = 4 * 1024;
const REQUEST_MAX: usize = 16 * 1024 * 1024;
/// Connections at once, with or without a token; more are turned away.
const CONNECTIONS_MAX: usize = 32;

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
        // 256 bits from the system's own generator.
        let token = circuit_runner::hosting::random_hex(32).map_err(std::io::Error::other)?;
        let file = super::info_file();
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir)?;
        }
        write_private(&file, &json!({ "port": port, "token": token, "pid": std::process::id() }).to_string())?;
        let (tx, rx) = channel();
        let stop = Arc::new(AtomicBool::new(false));
        let agents = Arc::new(AtomicUsize::new(0));
        let (stop2, agents2, token2) = (stop.clone(), agents.clone(), token.clone());
        // Connections open now, also the ones that haven't said their token.
        let open = Arc::new(AtomicUsize::new(0));
        std::thread::Builder::new().name("mcp-bridge".into()).spawn(move || {
            while !stop2.load(Ordering::Relaxed) {
                match listener.accept() {
                    // Too many at once: closed again, as each has a thread of its own.
                    Ok(_) if open.load(Ordering::Relaxed) >= CONNECTIONS_MAX => {}
                    Ok((stream, _)) => {
                        let (tx, stop, agents, token, ctx, open) = (tx.clone(), stop2.clone(), agents2.clone(), token2.clone(), ctx.clone(), open.clone());
                        open.fetch_add(1, Ordering::Relaxed);
                        let closed = open.clone();
                        let started = std::thread::Builder::new().name("mcp-agent".into()).spawn(move || {
                            serve(stream, &token, &tx, &stop, &agents, &ctx, HELLO_WITHIN);
                            open.fetch_sub(1, Ordering::Relaxed);
                        });
                        if started.is_err() {
                            closed.fetch_sub(1, Ordering::Relaxed);
                        }
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

/// One agent's connection: its token first (within `hello_within`), then a
/// request per line.
fn serve(stream: TcpStream, token: &str, tx: &Sender<Pending>, stop: &AtomicBool, agents: &AtomicUsize, ctx: &egui::Context, hello_within: Duration) {
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(hello_within));
    let Ok(mut out) = stream.try_clone() else { return };
    let mut reader = BufReader::new(stream);
    let hello: Option<Value> = read_line(&mut reader, HELLO_MAX).and_then(|l| serde_json::from_str(&l).ok());
    if !hello.as_ref().and_then(|h| h["token"].as_str()).is_some_and(|given| same(given, token)) {
        let _ = writeln!(out, "{}", json!({ "error": "wrong token" }));
        return;
    }
    if writeln!(out, "{}", json!({ "ok": true, "version": env!("CARGO_PKG_VERSION") })).is_err() {
        return;
    }
    // An agent may say nothing for hours between its requests.
    let _ = reader.get_ref().set_read_timeout(None);
    agents.fetch_add(1, Ordering::Relaxed);
    ctx.request_repaint();
    while let Some(line) = read_line(&mut reader, REQUEST_MAX) {
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

/// The next line, without its end. None at the end, on an error (the wait
/// for the token running out is one), and for a line longer than `max`
/// bytes: what follows such a line can't be told from its rest, so the
/// connection ends there.
fn read_line(reader: &mut impl BufRead, max: usize) -> Option<String> {
    let mut line = Vec::new();
    let read = reader.by_ref().take(max as u64 + 1).read_until(b'\n', &mut line).ok()?;
    let whole = line.last() == Some(&b'\n');
    if read == 0 || (!whole && line.len() > max) {
        return None;
    }
    while matches!(line.last(), Some(b'\n' | b'\r')) {
        line.pop();
    }
    String::from_utf8(line).ok()
}

/// Whether two secrets are the same, taking as long whichever byte differs.
fn same(given: &str, secret: &str) -> bool {
    given.len() == secret.len() && given.bytes().zip(secret.bytes()).fold(0, |differ, (a, b)| differ | (a ^ b)) == 0
}

/// Writes a file that only this user may read: it holds the token. (On
/// Windows the app's data folder is the user's own already.)
fn write_private(path: &Path, text: &str) -> std::io::Result<()> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        options.mode(0o600);
        // A file left by an earlier version keeps the rights it was made
        // with: they're changed before the new token goes in.
        if path.exists() {
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
        }
    }
    options.open(path)?.write_all(text.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Only who says the token in time gets in: a connection that says
    /// nothing, the wrong token or an endless line is let go.
    #[test]
    fn only_the_token_gets_a_connection_in() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let (tx, rx) = channel();
        let agents = Arc::new(AtomicUsize::new(0));
        let counted = agents.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let (tx, agents) = (tx.clone(), counted.clone());
                std::thread::spawn(move || serve(stream, "right-token", &tx, &AtomicBool::new(false), &agents, &egui::Context::default(), Duration::from_millis(300)));
            }
        });
        // What the app's side answers to a first line (None: it closed without a word).
        let said = |first: Option<&[u8]>| -> Option<Value> {
            let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
            stream.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
            if let Some(first) = first {
                // It may close while a long line is still being sent.
                let _ = stream.write_all(first);
            }
            let mut answer = String::new();
            let _ = BufReader::new(stream).read_line(&mut answer);
            serde_json::from_str(&answer).ok()
        };
        let started = std::time::Instant::now();
        assert_eq!(said(None).unwrap()["error"], "wrong token", "saying nothing ends it after a moment");
        assert!(started.elapsed() < Duration::from_secs(5), "it didn't wait for good");
        assert_eq!(said(Some(b"{\"token\":\"wrong-token\"}\n")).unwrap()["error"], "wrong token");
        assert_eq!(said(Some(b"GET / HTTP/1.1\r\n")).unwrap()["error"], "wrong token", "a browser's request isn't one");
        let endless = vec![b'x'; HELLO_MAX * 4];
        assert!(said(Some(&endless)).is_none_or(|v| v["error"] == "wrong token"), "a line without an end is cut off");
        assert_eq!(agents.load(Ordering::Relaxed), 0, "none of them counts as an agent");

        // The right token: in, and its requests reach the app.
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        stream.write_all(b"{\"token\":\"right-token\"}\n{\"op\":\"list\"}\n").unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut hello = String::new();
        reader.read_line(&mut hello).unwrap();
        assert_eq!(serde_json::from_str::<Value>(&hello).unwrap()["ok"], true);
        let pending = rx.recv_timeout(Duration::from_secs(10)).unwrap();
        assert_eq!(pending.request["op"], "list");
        pending.answer(json!({ "tools": [] }));
        let mut answer = String::new();
        reader.read_line(&mut answer).unwrap();
        assert_eq!(serde_json::from_str::<Value>(&answer).unwrap(), json!({ "tools": [] }));
        assert_eq!(agents.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn lines_have_a_longest_length_and_tokens_are_compared_whole() {
        let mut text: &[u8] = b"one\r\ntwo\n12345\nrest";
        assert_eq!(read_line(&mut text, 5).as_deref(), Some("one"));
        assert_eq!(read_line(&mut text, 5).as_deref(), Some("two"));
        assert_eq!(read_line(&mut text, 5).as_deref(), Some("12345"), "as long as allowed");
        assert_eq!(read_line(&mut text, 5).as_deref(), Some("rest"), "the last line needs no end");
        assert_eq!(read_line(&mut text, 5), None);
        assert_eq!(read_line(&mut &b"123456\nnext\n"[..], 5), None, "too long");
        assert!(same("abc123", "abc123"));
        assert!(!same("abc124", "abc123") && !same("abc12", "abc123") && !same("", "abc123"));
    }

    /// The file with the token is for this user's eyes only, also one that
    /// was there before with other rights.
    #[cfg(unix)]
    #[test]
    fn the_token_file_is_private() {
        use std::os::unix::fs::PermissionsExt;
        let file = std::env::temp_dir().join(format!("xc-mcp-private-{}.json", std::process::id()));
        std::fs::write(&file, "old").unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
        write_private(&file, "{}").unwrap();
        assert_eq!(std::fs::metadata(&file).unwrap().permissions().mode() & 0o777, 0o600);
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "{}");
        std::fs::remove_file(&file).unwrap();
        write_private(&file, "{}").unwrap();
        assert_eq!(std::fs::metadata(&file).unwrap().permissions().mode() & 0o777, 0o600);
        let _ = std::fs::remove_file(&file);
    }

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
