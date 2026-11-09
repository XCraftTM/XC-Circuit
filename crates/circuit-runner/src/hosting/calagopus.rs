//! A Calagopus panel: its client API (with a client API key, `c7sp_…`), for
//! uploading the bot, starting, stopping and restarting its server, and its
//! live console (a websocket to the node, with a token from the panel).
//!
//! Signing in works as in Calagopus' VS Code extension: the panel's "create an
//! API key" page opens in the browser with the permissions the editor needs,
//! and once approved the panel hands the key to a one-time address on this
//! computer (127.0.0.1), checked with a random `state`.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use serde_json::{Value, json};

use super::{Hosting, Remote};

/// A panel, and the API key used with it.
#[derive(Clone)]
pub struct Panel {
    base: String,
    key: String,
    agent: ureq::Agent,
}

/// A server the key can reach.
#[derive(Clone, Debug, PartialEq)]
pub struct Server {
    pub uuid: String,
    pub short: String,
    pub name: String,
}

/// What a server is doing now.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Resources {
    /// "running", "starting", "stopping" or "offline".
    pub state: String,
    pub memory: u64,
    pub memory_limit: u64,
    /// Percent of one core.
    pub cpu: f64,
    /// Milliseconds since it started.
    pub uptime: u64,
}

/// The panel's address as typed, made whole: https:// added, no trailing /.
pub fn base_url(typed: &str) -> String {
    let t = typed.trim().trim_end_matches('/');
    if t.starts_with("http://") || t.starts_with("https://") { t.to_string() } else { format!("https://{t}") }
}

impl Panel {
    pub fn new(base: &str, key: &str) -> Panel {
        let agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(60))).http_status_as_error(false).build().into();
        Panel { base: base_url(base), key: key.trim().to_string(), agent }
    }

    /// A request to the client API; its JSON answer, or what the panel said went wrong.
    fn call(&self, method: &str, path: &str, body: Option<Body>) -> Result<Value> {
        let url = format!("{}{path}", self.base);
        let auth = format!("Bearer {}", self.key);
        let result = match (method, body) {
            ("GET", _) => self.agent.get(&url).header("Authorization", &auth).header("Accept", "application/json").call(),
            ("POST", Some(Body::Json(v))) => self.agent.post(&url).header("Authorization", &auth).header("Accept", "application/json").header("Content-Type", "application/json").send(v.to_string()),
            ("POST", Some(Body::Raw(b))) => self.agent.post(&url).header("Authorization", &auth).header("Accept", "application/json").header("Content-Type", "text/plain").send(&b[..]),
            ("POST", None) => self.agent.post(&url).header("Authorization", &auth).header("Accept", "application/json").send_empty(),
            _ => bail!("unknown request {method}"),
        };
        let mut response = result.map_err(|e| anyhow!("couldn't reach {}: {e}", self.base))?;
        let status = response.status().as_u16();
        let text = response.body_mut().read_to_string().unwrap_or_default();
        if (200..300).contains(&status) {
            return Ok(serde_json::from_str(&text).unwrap_or(Value::Null));
        }
        // Calagopus says what's wrong as { errors: ["…"] }.
        let said = serde_json::from_str::<Value>(&text)
            .ok()
            .and_then(|v| v["errors"].as_array().map(|e| e.iter().filter_map(Value::as_str).collect::<Vec<_>>().join("; ")))
            .filter(|s| !s.is_empty());
        let hint = match status {
            401 => " The API key isn't valid (any more): sign in again.",
            403 => " The API key lacks a permission for this, or doesn't allow this computer's address: sign in again to grant it.",
            404 => " The panel doesn't know this server (or the key can't see it).",
            409 => " The server can't do that right now (it may be installing, suspended or being moved).",
            429 => " The panel asks to slow down: try again in a moment.",
            _ => "",
        };
        bail!("{} answered {status}{}.{hint}", self.base, said.map(|s| format!(": {s}")).unwrap_or_default())
    }

    /// Every server the key can reach.
    pub fn servers(&self) -> Result<Vec<Server>> {
        let mut out = Vec::new();
        for page in 1..=50 {
            let v = self.call("GET", &format!("/api/client/servers?page={page}&per_page=100"), None)?;
            let list = &v["servers"];
            let data = list["data"].as_array().cloned().unwrap_or_default();
            for s in &data {
                out.push(Server {
                    uuid: s["uuid"].as_str().unwrap_or_default().to_string(),
                    short: s["uuid_short"].as_str().map(str::to_string).or_else(|| s["uuid_short"].as_u64().map(|n| format!("{n:08x}"))).unwrap_or_default(),
                    name: s["name"].as_str().unwrap_or_default().to_string(),
                });
            }
            let total = list["total"].as_u64().unwrap_or(0) as usize;
            if data.is_empty() || out.len() >= total {
                break;
            }
        }
        Ok(out)
    }

    pub fn resources(&self, server: &str) -> Result<Resources> {
        let v = self.call("GET", &format!("/api/client/servers/{server}/resources"), None)?;
        let r = &v["resources"];
        Ok(Resources {
            state: r["state"].as_str().unwrap_or("unknown").to_string(),
            memory: r["memory_bytes"].as_u64().unwrap_or(0),
            memory_limit: r["memory_limit_bytes"].as_u64().unwrap_or(0),
            cpu: r["cpu_absolute"].as_f64().unwrap_or(0.0),
            uptime: r["uptime"].as_u64().unwrap_or(0),
        })
    }

    /// "start", "stop", "restart" or "kill".
    pub fn power(&self, server: &str, action: &str) -> Result<()> {
        self.call("POST", &format!("/api/client/servers/{server}/power"), Some(Body::Json(json!({ "action": action }))))?;
        Ok(())
    }

    /// Types a line into the server's console.
    pub fn command(&self, server: &str, command: &str) -> Result<()> {
        self.call("POST", &format!("/api/client/servers/{server}/command"), Some(Body::Json(json!({ "command": command }))))?;
        Ok(())
    }

    /// Writes a file (its path from the server's own folder, such as "/index.js").
    pub fn write_file(&self, server: &str, path: &str, data: &[u8]) -> Result<()> {
        self.call("POST", &format!("/api/client/servers/{server}/files/write?file={}", encode(path)), Some(Body::Raw(data.to_vec())))?;
        Ok(())
    }

    /// Makes folder `name` in `root` ("/" for the server's own folder).
    pub fn create_dir(&self, server: &str, root: &str, name: &str) -> Result<()> {
        self.call("POST", &format!("/api/client/servers/{server}/files/create-directory"), Some(Body::Json(json!({ "root": root, "name": name }))))?;
        Ok(())
    }

    /// The console's websocket address and a token for it.
    fn websocket(&self, server: &str) -> Result<(String, String)> {
        let v = self.call("GET", &format!("/api/client/servers/{server}/websocket"), None)?;
        let (url, token) = (v["url"].as_str().or(v["data"]["socket"].as_str()), v["token"].as_str().or(v["data"]["token"].as_str()));
        match (url, token) {
            (Some(u), Some(t)) => Ok((u.to_string(), t.to_string())),
            _ => bail!("the panel didn't give an address for the console"),
        }
    }
}

enum Body {
    Json(Value),
    Raw(Vec<u8>),
}

/// Percent-encodes a query value.
fn encode(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => match u8::from_str_radix(&text[i + 1..i + 3], 16) {
                Ok(b) => {
                    out.push(b);
                    i += 3;
                    continue;
                }
                Err(_) => out.push(b'%'),
            },
            b'+' => out.push(b' '),
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Uploading through the panel's file API.
pub(super) struct Upload {
    panel: Panel,
    server: String,
    base: String,
    restart: bool,
}

impl Upload {
    pub(super) fn open(h: &Hosting, key: &str, log: &mut dyn FnMut(String)) -> Result<Upload> {
        let panel = Panel::new(&h.panel, key);
        log(format!("Connecting to {} …", panel.base));
        // Also checks the key and that it reaches the server.
        panel.call("GET", &format!("/api/client/servers/{}", h.server), None)?;
        let (_, parts) = h.folder_parts();
        let mut root = "/".to_string();
        for part in parts {
            // Already there is fine: writing into it says if it isn't.
            let _ = panel.create_dir(&h.server, &root, &part);
            root = format!("{}{part}/", if root == "/" { "/" } else { root.as_str() });
        }
        log(format!("Connected to {}.", if h.server_name.is_empty() { &h.server } else { &h.server_name }));
        Ok(Upload { panel, server: h.server.clone(), base: root, restart: h.restart })
    }
}

impl Remote for Upload {
    fn put(&mut self, name: &str, data: &[u8]) -> Result<()> {
        self.panel.write_file(&self.server, &format!("{}{name}", self.base), data)
    }

    fn finish(&mut self, log: &mut dyn FnMut(String)) -> Result<()> {
        if self.restart {
            self.panel.power(&self.server, "restart").context("restarting the server")?;
            log("Restarting the server, so the new bot runs.".into());
        }
        Ok(())
    }
}

/// The permissions the editor asks for: reading the servers, the console,
/// start, stop and restart, and writing files.
const SERVER_PERMISSIONS: &str = "control.read-console,control.console,control.start,control.stop,control.restart,files.create,files.read,files.update";

/// A sign-in under way: open `url` in the browser; the key arrives on `key`.
pub struct SignIn {
    pub url: String,
    pub key: Receiver<String>,
    stop: Arc<AtomicBool>,
}

impl Drop for SignIn {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// Starts signing in to `panel`: a one-time address on this computer waits
/// (up to 15 minutes) for the panel to hand over the new key.
pub fn sign_in(panel: &str, name: &str) -> Result<SignIn> {
    let listener = TcpListener::bind("127.0.0.1:0").context("couldn't wait for the panel's answer on this computer")?;
    listener.set_nonblocking(true)?;
    let port = listener.local_addr()?.port();
    let state = super::random_hex(16)?;
    let callback = format!("http://127.0.0.1:{port}/?state={state}");
    let url = format!(
        "{}/account/api-keys/create?name={}&user_permissions=servers.read&server_permissions={}&callback_url={}",
        base_url(panel),
        encode(name),
        encode(SERVER_PERMISSIONS),
        encode(&callback)
    );
    let (tx, rx) = channel();
    let stop = Arc::new(AtomicBool::new(false));
    let stopped = stop.clone();
    thread::spawn(move || {
        let until = Instant::now() + Duration::from_secs(15 * 60);
        while Instant::now() < until && !stopped.load(Ordering::Relaxed) {
            match listener.accept() {
                Ok((stream, _)) => {
                    if let Some(key) = answer(stream, &state) {
                        let _ = tx.send(key);
                        return;
                    }
                }
                Err(_) => thread::sleep(Duration::from_millis(100)),
            }
        }
    });
    Ok(SignIn { url, key: rx, stop })
}

/// Reads the panel's redirect: the key, when the state is ours. Answers with a page.
fn answer(mut stream: TcpStream, state: &str) -> Option<String> {
    stream.set_nonblocking(false).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
    let mut line = String::new();
    BufReader::new(&mut stream).read_line(&mut line).ok()?;
    let target = line.split_whitespace().nth(1).unwrap_or("");
    let query = target.split_once('?').map(|(_, q)| q).unwrap_or("");
    let value = |name: &str| query.split('&').filter_map(|p| p.split_once('=')).find(|(k, _)| *k == name).map(|(_, v)| decode(v));
    let ours = value("state").is_some_and(|s| s == state);
    let key = value("key").filter(|k| ours && !k.trim().is_empty());
    let (status, heading, text) = match &key {
        Some(_) => ("200 OK", "Signed in", "XC Circuit has the new API key. You can close this tab."),
        None if ours => ("400 Bad Request", "No API key", "The panel didn't hand over a key. Go back to XC Circuit and try again."),
        None => ("404 Not Found", "Not found", "This address only waits for the panel."),
    };
    let page = format!(
        "<!doctype html><meta charset=utf-8><title>XC Circuit</title><body style=\"font-family:system-ui;background:#1e1f22;color:#dbdee1;display:grid;place-items:center;height:90vh\"><div><h2>{heading}</h2><p>{text}</p></div>"
    );
    let _ = write!(stream, "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{page}", page.len());
    key.map(|k| k.trim().to_string())
}

/// A name for the API key the panel makes at sign-in: with the bot's name,
/// and a short tag, as a panel takes each name once per account (signing in
/// again, or for a second bot, would be refused). At most 31 characters,
/// Calagopus's limit.
pub fn key_name(bot: &str) -> String {
    key_name_with(bot, &super::random_hex(2).unwrap_or_else(|_| "0000".into()))
}

const KEY_NAME_MAX: usize = 31;

fn key_name_with(bot: &str, tag: &str) -> String {
    let bot = bot.split_whitespace().collect::<Vec<_>>().join(" ");
    if bot.is_empty() {
        return format!("XC Circuit #{tag}");
    }
    for prefix in ["XC Circuit · ", "XCC · "] {
        let name = format!("{prefix}{bot} #{tag}");
        if name.chars().count() <= KEY_NAME_MAX {
            return name;
        }
    }
    // Still too long: the bot's name shortened.
    let end = format!("… #{tag}");
    let room = KEY_NAME_MAX - "XCC · ".chars().count() - end.chars().count();
    let short: String = bot.chars().take(room).collect();
    format!("XCC · {}{end}", short.trim_end())
}

/// Something from a server's live console.
#[derive(Clone, Debug, PartialEq)]
pub enum ConsoleEvent {
    Connected,
    /// A line the server printed, in pieces of one colour each.
    Output(Vec<Piece>),
    /// Its power state: "running", "starting", "stopping", "offline".
    Status(String),
    /// Memory in bytes, CPU in percent of a core.
    Stats { memory: u64, memory_limit: u64, cpu: f64 },
    /// Couldn't connect, or the connection broke (it tries again).
    Problem(String),
}

/// A server's live console, kept connected on its own thread (reconnecting
/// when the connection breaks). Dropping it closes it.
pub struct Console {
    pub events: Receiver<ConsoleEvent>,
    commands: Sender<String>,
    stop: Arc<AtomicBool>,
}

impl Console {
    pub fn open(panel: Panel, server: String) -> Console {
        let (tx, events) = channel();
        let (commands, rx) = channel();
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        thread::spawn(move || {
            let mut wait = 1;
            while !stopped.load(Ordering::Relaxed) {
                match run_console(&panel, &server, &tx, &rx, &stopped) {
                    Ok(()) => return,
                    Err(e) => {
                        if tx.send(ConsoleEvent::Problem(format!("{e:#}"))).is_err() {
                            return;
                        }
                    }
                }
                // Tries again, waiting longer each time (up to 30 s).
                for _ in 0..wait * 10 {
                    if stopped.load(Ordering::Relaxed) {
                        return;
                    }
                    thread::sleep(Duration::from_millis(100));
                }
                wait = (wait * 2).min(30);
            }
        });
        Console { events, commands, stop }
    }

    /// Types a line into the console.
    pub fn send(&self, command: &str) {
        let _ = self.commands.send(command.to_string());
    }
}

impl Drop for Console {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

trait Stream: Read + Write {}
impl<T: Read + Write> Stream for T {}

/// One connection to the console, until it closes (Err) or `stop` (Ok).
fn run_console(panel: &Panel, server: &str, tx: &Sender<ConsoleEvent>, commands: &Receiver<String>, stop: &AtomicBool) -> Result<()> {
    let (url, token) = panel.websocket(server)?;
    let mut socket = connect_socket(&url)?;
    let send = |socket: &mut tungstenite::WebSocket<Box<dyn Stream>>, event: &str, args: &[&str]| -> Result<()> {
        socket.send(tungstenite::Message::text(json!({ "event": event, "args": args }).to_string()))?;
        Ok(())
    };
    send(&mut socket, "auth", &[&token])?;
    loop {
        if stop.load(Ordering::Relaxed) {
            let _ = socket.close(None);
            return Ok(());
        }
        loop {
            match commands.try_recv() {
                Ok(line) => send(&mut socket, "send command", &[&line])?,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return Ok(()),
            }
        }
        let message = match socket.read() {
            Ok(m) => m,
            Err(tungstenite::Error::Io(e)) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => continue,
            Err(e) => bail!("the console connection closed: {e}"),
        };
        let tungstenite::Message::Text(text) = message else { continue };
        let Ok(v) = serde_json::from_str::<Value>(&text) else { continue };
        let arg = |i: usize| v["args"][i].as_str().unwrap_or_default().to_string();
        let event = match v["event"].as_str().unwrap_or_default() {
            "auth success" => {
                send(&mut socket, "send logs", &[])?;
                Some(ConsoleEvent::Connected)
            }
            "console output" | "install output" | "daemon message" => Some(ConsoleEvent::Output(read_line(&arg(0)))),
            "status" => Some(ConsoleEvent::Status(arg(0))),
            "stats" => serde_json::from_str::<Value>(&arg(0)).ok().map(|s| ConsoleEvent::Stats {
                memory: s["memory_bytes"].as_u64().unwrap_or(0),
                memory_limit: s["memory_limit_bytes"].as_u64().unwrap_or(0),
                cpu: s["cpu_absolute"].as_f64().unwrap_or(0.0),
            }),
            "token expiring" | "token expired" => {
                let (_, token) = panel.websocket(server)?;
                send(&mut socket, "auth", &[&token])?;
                None
            }
            "jwt error" | "daemon error" => Some(ConsoleEvent::Problem(arg(0))),
            _ => None,
        };
        if let Some(e) = event
            && tx.send(e).is_err()
        {
            return Ok(());
        }
    }
}

/// Opens the websocket (ws:// or wss://, the panel may give http(s)://),
/// reading with a short timeout so commands go out while it waits.
fn connect_socket(url: &str) -> Result<tungstenite::WebSocket<Box<dyn Stream>>> {
    use tungstenite::client::IntoClientRequest;
    let url = url.replacen("https://", "wss://", 1).replacen("http://", "ws://", 1);
    let secure = url.starts_with("wss://");
    let rest = url.split_once("://").map(|(_, r)| r).unwrap_or(&url);
    let authority = rest.split('/').next().unwrap_or(rest);
    let host = authority.rsplit_once(':').filter(|(_, p)| p.parse::<u16>().is_ok()).map(|(h, _)| h).unwrap_or(authority).trim_matches(['[', ']']).to_string();
    let port = authority.rsplit_once(':').and_then(|(_, p)| p.parse::<u16>().ok()).unwrap_or(if secure { 443 } else { 80 });
    let tcp = TcpStream::connect((host.as_str(), port)).with_context(|| format!("couldn't reach the console at {host}:{port}"))?;
    tcp.set_read_timeout(Some(Duration::from_millis(250)))?;
    let stream: Box<dyn Stream> = if secure {
        let name = rustls::pki_types::ServerName::try_from(host.clone()).map_err(|_| anyhow!("{host} isn't a valid server name"))?;
        let conn = rustls::ClientConnection::new(super::tls_config(), name)?;
        Box::new(rustls::StreamOwned::new(conn, tcp))
    } else {
        Box::new(tcp)
    };
    let mut request = url.as_str().into_client_request()?;
    // The node checks where the connection comes from: its own origin, as the extension sends.
    let origin = format!("{}://{authority}", if secure { "https" } else { "http" });
    request.headers_mut().insert("Origin", origin.parse()?);
    // The handshake waits for its answer, through the short read timeout.
    let mut handshake = tungstenite::client(request, stream);
    let started = Instant::now();
    loop {
        match handshake {
            Ok((socket, _)) => return Ok(socket),
            Err(tungstenite::HandshakeError::Interrupted(mid)) if started.elapsed() < Duration::from_secs(20) => handshake = mid.handshake(),
            Err(tungstenite::HandshakeError::Interrupted(_)) => bail!("the console at {host} didn't answer"),
            Err(tungstenite::HandshakeError::Failure(e)) => bail!("couldn't open the console: {e}"),
        }
    }
}

/// A colour from a terminal's colour codes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Color {
    /// One of the 16 terminal colours: 0–7 (black, red, green, yellow, blue,
    /// magenta, cyan, white) and 8–15 their bright versions.
    Ansi(u8),
    Rgb(u8, u8, u8),
}

/// How a piece of a line looks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Style {
    /// None: the console's own text colour.
    pub color: Option<Color>,
    pub bold: bool,
    pub dim: bool,
}

/// A piece of a console line in one style.
#[derive(Clone, Debug, PartialEq)]
pub struct Piece {
    pub text: String,
    pub style: Style,
}

/// A line's text without its colours.
pub fn plain(pieces: &[Piece]) -> String {
    pieces.iter().map(|p| p.text.as_str()).collect()
}

/// Reads a line a server printed as a terminal shows it: its colour codes
/// (ESC [ … m: 16 and 256 colours, RGB, bold, dim) become the pieces'
/// styles, a carriage return goes back to the start and writes over what's
/// there (npm's spinner), and ESC [ K erases the rest of the line.
pub fn read_line(text: &str) -> Vec<Piece> {
    let text = text.trim_end_matches(['\r', '\n']);
    let mut cells: Vec<(char, Style)> = Vec::new();
    let mut col = 0;
    let mut style = Style::default();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\r' => col = 0,
            '\u{1b}' => {
                if chars.peek() != Some(&'[') {
                    chars.next();
                    continue;
                }
                chars.next();
                let mut params = String::new();
                let mut end = None;
                for d in chars.by_ref() {
                    if d.is_ascii_alphabetic() || d == '@' || d == '~' {
                        end = Some(d);
                        break;
                    }
                    params.push(d);
                }
                match end {
                    Some('m') => apply_sgr(&mut style, &params),
                    // Erase in line: to its end (0), to the cursor (1), all of it (2).
                    Some('K') => match params.as_str() {
                        "2" => {
                            cells.clear();
                            col = 0;
                        }
                        "1" => cells.iter_mut().take(col).for_each(|c| c.0 = ' '),
                        _ => cells.truncate(col),
                    },
                    // To a column (counted from 1).
                    Some('G') => col = params.parse::<usize>().unwrap_or(1).saturating_sub(1).min(cells.len()),
                    _ => {}
                }
            }
            '\t' => {
                for _ in 0..(4 - col % 4) {
                    put(&mut cells, &mut col, ' ', style);
                }
            }
            c if c.is_control() => {}
            c => put(&mut cells, &mut col, c, style),
        }
    }
    let mut pieces: Vec<Piece> = Vec::new();
    for (c, st) in cells {
        match pieces.last_mut() {
            Some(p) if p.style == st => p.text.push(c),
            _ => pieces.push(Piece { text: c.to_string(), style: st }),
        }
    }
    pieces
}

fn put(cells: &mut Vec<(char, Style)>, col: &mut usize, c: char, style: Style) {
    if *col < cells.len() {
        cells[*col] = (c, style);
    } else {
        cells.push((c, style));
    }
    *col += 1;
}

/// Changes `style` by the numbers of a colour code ("1;33", "38;5;208", "0").
fn apply_sgr(style: &mut Style, params: &str) {
    let nums: Vec<u16> = if params.is_empty() { vec![0] } else { params.split([';', ':']).map(|n| n.parse().unwrap_or(0)).collect() };
    let mut i = 0;
    while i < nums.len() {
        match nums[i] {
            0 => *style = Style::default(),
            1 => style.bold = true,
            2 => style.dim = true,
            22 => (style.bold, style.dim) = (false, false),
            n @ 30..=37 => style.color = Some(Color::Ansi((n - 30) as u8)),
            n @ 90..=97 => style.color = Some(Color::Ansi((n - 90 + 8) as u8)),
            39 => style.color = None,
            // 38;5;n (256 colours) or 38;2;r;g;b.
            38 => match nums.get(i + 1) {
                Some(5) => {
                    style.color = nums.get(i + 2).map(|&n| color_256(n.min(255) as u8));
                    i += 2;
                }
                Some(2) => {
                    let c = |k: usize| nums.get(i + k).copied().unwrap_or(0).min(255) as u8;
                    style.color = Some(Color::Rgb(c(2), c(3), c(4)));
                    i += 4;
                }
                _ => {}
            },
            // Backgrounds are left out (the console keeps its own), with their numbers.
            48 => match nums.get(i + 1) {
                Some(5) => i += 2,
                Some(2) => i += 4,
                _ => {}
            },
            _ => {}
        }
        i += 1;
    }
}

/// One of xterm's 256 colours.
fn color_256(n: u8) -> Color {
    match n {
        0..=15 => Color::Ansi(n),
        16..=231 => {
            let n = n - 16;
            let level = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
            Color::Rgb(level(n / 36), level(n / 6 % 6), level(n % 6))
        }
        _ => {
            let g = 8 + (n - 232) * 10;
            Color::Rgb(g, g, g)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The key's name carries the bot's name, a tag to keep it apart from
    /// earlier ones, and fits the panel's 31 characters.
    #[test]
    fn key_names_fit_the_panel() {
        assert_eq!(key_name_with("Test Bot", "a1b2"), "XC Circuit · Test Bot #a1b2");
        assert_eq!(key_name_with("Test XCC Bot", "a1b2"), "XC Circuit · Test XCC Bot #a1b2");
        assert_eq!(key_name_with("My Moderation Bot", "a1b2"), "XCC · My Moderation Bot #a1b2");
        assert_eq!(key_name_with("  ", "a1b2"), "XC Circuit #a1b2");
        let long = key_name_with("The Most Amazing Ticket Bot Ever Made", "a1b2");
        assert_eq!(long.chars().count(), 31, "{long}");
        assert!(long.starts_with("XCC · The Most") && long.ends_with("… #a1b2"), "{long}");
        assert_ne!(key_name("Bot"), key_name("Bot"), "a new tag each time");
    }
    use std::sync::Mutex;

    #[test]
    fn addresses_and_text() {
        assert_eq!(base_url(" panel.example.com/ "), "https://panel.example.com");
        assert_eq!(base_url("http://10.0.0.2:8080"), "http://10.0.0.2:8080");
        assert_eq!(encode("/bots/my bot.js"), "/bots/my%20bot.js");
        assert_eq!(decode("c7sp_ab%2Bc+d"), "c7sp_ab+c d");
    }

    /// Colour codes become styles, as a terminal shows them.
    #[test]
    fn console_lines_keep_their_colours() {
        let yellow = Style { color: Some(Color::Ansi(3)), ..Default::default() };
        assert_eq!(
            read_line("\u{1b}[33m[Bot]\u{1b}[0m ready\r\n"),
            [Piece { text: "[Bot]".into(), style: yellow }, Piece { text: " ready".into(), style: Style::default() }]
        );
        let pieces = read_line("\u{1b}[90m10:53:29\u{1b}[39m \u{1b}[1;32mSUCCESS\u{1b}[22;39m Logged in");
        assert_eq!(plain(&pieces), "10:53:29 SUCCESS Logged in");
        assert_eq!(pieces[0].style.color, Some(Color::Ansi(8)), "bright black: grey");
        assert_eq!(pieces[2].style, Style { color: Some(Color::Ansi(2)), bold: true, dim: false });
        assert_eq!(pieces[3].style, Style::default());
        assert_eq!(read_line("\u{1b}[38;5;208mx")[0].style.color, Some(Color::Rgb(255, 135, 0)), "256 colours");
        assert_eq!(read_line("\u{1b}[38;2;1;2;3;48;2;9;9;9mx")[0].style.color, Some(Color::Rgb(1, 2, 3)), "RGB, background left out");
        // npm's spinner: a character, back to the start, the text over it.
        assert_eq!(plain(&read_line("/\rnpm warn install-scripts")), "npm warn install-scripts");
        assert_eq!(plain(&read_line("loading 50%\rdone\u{1b}[K")), "done", "erased after the cursor");
        assert_eq!(plain(&read_line("a\tb")), "a   b");
    }


    /// The sign-in hands over the key the panel sends to the one-time
    /// address, and only with the right state.
    #[test]
    fn sign_in_takes_the_key_from_the_panel() {
        let s = sign_in("panel.example.com", "XC Circuit").unwrap();
        assert!(s.url.starts_with("https://panel.example.com/account/api-keys/create?name=XC%20Circuit&user_permissions=servers.read&server_permissions=control.read-console"));
        let callback = decode(s.url.split("callback_url=").nth(1).unwrap());
        let (address, state) = callback.split_once("/?state=").unwrap();
        let get = |path: &str| {
            let mut c = TcpStream::connect(address.trim_start_matches("http://")).unwrap();
            write!(c, "GET {path} HTTP/1.1\r\nHost: x\r\n\r\n").unwrap();
            let mut reply = String::new();
            let _ = c.read_to_string(&mut reply);
            reply
        };
        assert!(get("/?state=wrong&key=c7sp_bad").starts_with("HTTP/1.1 404"));
        assert!(get(&format!("/?state={state}&key=c7sp_good")).starts_with("HTTP/1.1 200"));
        assert_eq!(s.key.recv_timeout(Duration::from_secs(5)).unwrap(), "c7sp_good");
    }

    /// Requests carry the key, and the panel's errors come back in words.
    #[test]
    fn the_api_is_called_with_the_key() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let seen = Arc::new(Mutex::new(Vec::<String>::new()));
        let log = seen.clone();
        thread::spawn(move || {
            for stream in listener.incoming().take(3) {
                let mut stream = stream.unwrap();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut head = String::new();
                let mut length = 0;
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    if line.to_lowercase().starts_with("content-length:") {
                        length = line[15..].trim().parse().unwrap();
                    }
                    if line == "\r\n" {
                        break;
                    }
                    head.push_str(&line);
                }
                let mut body = vec![0; length];
                reader.read_exact(&mut body).unwrap();
                log.lock().unwrap().push(format!("{head}{}", String::from_utf8_lossy(&body)));
                let (status, reply) = if head.starts_with("GET /api/client/servers?") {
                    ("200 OK", r#"{"servers":{"data":[{"uuid":"u-1","uuid_short":"1a2b3c4d","name":"My bot"}],"total":1,"page":1,"per_page":100}}"#)
                } else if head.contains("/power") {
                    ("409 Conflict", r#"{"errors":["server is installing"]}"#)
                } else {
                    ("200 OK", "{}")
                };
                write!(stream, "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}", reply.len()).unwrap();
            }
        });
        let panel = Panel::new(&format!("http://{address}"), "c7sp_secret");
        assert_eq!(panel.servers().unwrap(), [Server { uuid: "u-1".into(), short: "1a2b3c4d".into(), name: "My bot".into() }]);
        panel.write_file("u-1", "/bot/index.js", b"// bot").unwrap();
        let err = panel.power("u-1", "restart").unwrap_err().to_string();
        assert!(err.contains("409: server is installing") && err.contains("can't do that right now"), "{err}");
        let seen = seen.lock().unwrap();
        assert!(seen.iter().all(|r| r.contains("Bearer c7sp_secret")), "every request has the key");
        assert!(seen[1].starts_with("POST /api/client/servers/u-1/files/write?file=/bot/index.js") && seen[1].ends_with("// bot"));
        assert!(seen[2].contains(r#"{"action":"restart"}"#));
    }

    /// The console: the panel gives the node's address and a token; the
    /// node gets the token, sends lines and the state, and takes commands.
    #[test]
    fn the_console_streams_and_takes_commands() {
        // The node's websocket.
        let node = TcpListener::bind("127.0.0.1:0").unwrap();
        let node_address = node.local_addr().unwrap();
        let got = Arc::new(Mutex::new(Vec::<String>::new()));
        let seen = got.clone();
        thread::spawn(move || {
            let (stream, _) = node.accept().unwrap();
            let mut ws = tungstenite::accept(stream).unwrap();
            let said = |ws: &mut tungstenite::WebSocket<TcpStream>| -> String {
                let text = ws.read().unwrap().into_text().unwrap().to_string();
                seen.lock().unwrap().push(text.clone());
                text
            };
            assert_eq!(said(&mut ws), r#"{"event":"auth","args":["token-1"]}"#);
            ws.send(tungstenite::Message::text(r#"{"event":"auth success","args":[]}"#)).unwrap();
            assert_eq!(said(&mut ws), r#"{"event":"send logs","args":[]}"#);
            ws.send(tungstenite::Message::text(r#"{"event":"console output","args":["\u001b[32mBot ready\u001b[0m"]}"#)).unwrap();
            ws.send(tungstenite::Message::text(r#"{"event":"status","args":["running"]}"#)).unwrap();
            said(&mut ws);
            ws.send(tungstenite::Message::text(r#"{"event":"console output","args":["> say hi"]}"#)).unwrap();
            let _ = ws.read();
        });
        // The panel: the websocket's address and token.
        let panel = TcpListener::bind("127.0.0.1:0").unwrap();
        let panel_address = panel.local_addr().unwrap();
        thread::spawn(move || {
            let (mut stream, _) = panel.accept().unwrap();
            let mut line = String::new();
            BufReader::new(stream.try_clone().unwrap()).read_line(&mut line).unwrap();
            assert!(line.starts_with("GET /api/client/servers/u-1/websocket"), "{line}");
            let reply = format!(r#"{{"token":"token-1","url":"ws://{node_address}/api/servers/u-1/ws"}}"#);
            write!(stream, "HTTP/1.1 200 OK
Content-Type: application/json
Content-Length: {}
Connection: close

{reply}", reply.len()).unwrap();
        });
        let console = Console::open(Panel::new(&format!("http://{panel_address}"), "c7sp_x"), "u-1".into());
        let next = || console.events.recv_timeout(Duration::from_secs(10)).unwrap();
        assert_eq!(next(), ConsoleEvent::Connected);
        let ConsoleEvent::Output(ready) = next() else { panic!("a line") };
        assert_eq!((plain(&ready), ready[0].style.color), ("Bot ready".to_string(), Some(Color::Ansi(2))), "with its colour");
        assert_eq!(next(), ConsoleEvent::Status("running".into()));
        console.send("say hi");
        assert!(matches!(next(), ConsoleEvent::Output(p) if plain(&p) == "> say hi"));
        assert_eq!(got.lock().unwrap()[2], r#"{"event":"send command","args":["say hi"]}"#);
    }
}
