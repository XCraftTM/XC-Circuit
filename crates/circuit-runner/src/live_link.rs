//! The wire of a live session: a room at a relay (an XC Circuit Host, or
//! the public relay), with everything in it sealed between the apps.
//!
//! The relay passes messages on and can't read them: each one is packed
//! (deflate) and sealed with the session's key, which only travels in the
//! invitation, from person to person. One too big for the relay goes in
//! sealed parts, put together again at the other end.
//!
//! When the host's connection drops, the relay keeps the room for a while
//! and the host's wire takes it back (with the room's own key, which only
//! the host has); the guests are told to say hello again, and catch up.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow, bail};
use base64::Engine;
use serde_json::{Value, json};

use crate::host_link::VaultKey;
use crate::hosting::calagopus::connect_socket;

/// The relay everyone can use, for people without a host of their own.
pub const PUBLIC_RELAY: &str = "https://xcrelay.xcrafttm.dev";
/// What an invitation starts with.
const SCHEME: &str = "xc-live:";
/// The most a relay passes on in one message, with room to spare.
const MOST: usize = 240 * 1024;
/// A message bigger than that goes in parts of this many packed bytes: a
/// part sealed stays under `MOST` even when nothing of it packs.
const PART: usize = 120 * 1024;
/// The most parts one message may have (about 30 MB packed), and how many
/// messages in parts may be on their way at once.
const MOST_PARTS: usize = 256;
const ON_THE_WAY: usize = 8;
/// Nothing at all from the relay for this long (it pings every 30 s and
/// answers ours): the connection is gone, whatever the system says.
const QUIET: Duration = Duration::from_secs(75);
/// How long the host tries to get its room back after a dropped
/// connection (the relay keeps it a minute).
const HOST_AWAY: Duration = Duration::from_secs(55);

/// What the wire says.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// The host's room is open: the invitation to pass on.
    Opened { invite: String },
    /// This guest is in the room; or, after the host was away, the host is
    /// back (and wants to hear hello again).
    Joined,
    /// A guest came (`on`) or went.
    Peer { id: String, on: bool },
    /// A message, opened: from the guest `id`, or from `host`.
    From { id: String, data: Value },
    /// The host's connection dropped: it is trying to get back (`why`).
    Away { why: String },
    /// The host is back in its room.
    Back,
    /// A guest: the host's connection dropped; the relay waits for them.
    HostAway,
    /// Something couldn't be sent, but the session goes on.
    Trouble { why: String },
    /// The session is over: the host ended it, or the connection is gone.
    Closed { why: String },
}

enum Out {
    /// To one guest, or to everyone (`None`). A guest's goes to the host.
    To(Option<String>, Value),
    Kick(String),
    Leave,
}

/// One end of a session. Dropping it leaves the room.
pub struct Wire {
    out: Sender<Out>,
    events: Receiver<Event>,
}

/// Called from the wire's thread whenever an event arrives: the window
/// draws again then, instead of looking every few milliseconds.
pub type Wake = Arc<dyn Fn() + Send + Sync>;

/// An invitation taken apart: the relay, the room, and the session's key.
pub fn read_invite(invite: &str) -> Result<(String, String, VaultKey)> {
    let rest = invite.trim().strip_prefix(SCHEME).ok_or_else(|| anyhow!("That isn't an invitation: one starts with {SCHEME}"))?;
    let (place, key) = rest.rsplit_once('#').ok_or_else(|| anyhow!("The invitation isn't complete: copy all of it."))?;
    let (relay, room) = place.rsplit_once('/').ok_or_else(|| anyhow!("The invitation isn't complete: copy all of it."))?;
    if !relay.starts_with("https://") && !relay.starts_with("http://") {
        bail!("The invitation isn't complete: copy all of it.");
    }
    Ok((relay.to_string(), room.to_string(), VaultKey::from_recovery_code(key).map_err(|_| anyhow!("The invitation isn't complete: copy all of it."))?))
}

fn write_invite(relay: &str, room: &str, key: &VaultKey) -> String {
    format!("{SCHEME}{}/{room}#{}", relay.trim_end_matches('/'), key.recovery_code().replace('-', ""))
}

fn deflate(bytes: &[u8]) -> Result<Vec<u8>> {
    let mut packed = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::fast());
    packed.write_all(bytes)?;
    Ok(packed.finish()?)
}

/// A message packed and sealed: one piece, or several parts when it is
/// too big for one message at the relay. `next` numbers messages in parts.
fn pack(key: &VaultKey, data: &Value, next: &mut u64) -> Result<Vec<String>> {
    let packed = deflate(data.to_string().as_bytes())?;
    let sealed = key.seal(&packed)?;
    if sealed.len() <= MOST {
        return Ok(vec![sealed]);
    }
    let n = packed.len().div_ceil(PART);
    if n > MOST_PARTS {
        bail!("That is too much for a session (a workspace of many megabytes).");
    }
    *next += 1;
    let id = *next;
    packed
        .chunks(PART)
        .enumerate()
        .map(|(i, chunk)| {
            let part = json!({ "xcPart": { "id": id, "i": i, "n": n, "d": base64::engine::general_purpose::STANDARD.encode(chunk) } });
            let sealed = key.seal(&deflate(part.to_string().as_bytes())?)?;
            if sealed.len() > MOST {
                bail!("A part of a message came out too big.");
            }
            Ok(sealed)
        })
        .collect()
}

fn inflate(packed: &[u8]) -> Result<Value> {
    let mut text = String::new();
    flate2::read::DeflateDecoder::new(packed).take(64 * 1024 * 1024).read_to_string(&mut text)?;
    Ok(serde_json::from_str(&text)?)
}

fn unpack(key: &VaultKey, sealed: &str) -> Result<Value> {
    let packed = key.open(sealed).map_err(|_| anyhow!("A message couldn't be opened: it isn't from this session."))?;
    inflate(&packed)
}

/// Messages in parts on their way: by who sent them and their number.
#[derive(Default)]
struct Parts {
    waiting: HashMap<(String, u64), Vec<Option<Vec<u8>>>>,
}

impl Parts {
    /// Takes an opened message. A whole one comes back as it is; a part
    /// comes back as the whole message once its last part is here.
    fn take(&mut self, from: &str, data: Value) -> Option<Value> {
        let Some(part) = data.get("xcPart") else { return Some(data) };
        let (Some(id), Some(i), Some(n), Some(d)) = (part["id"].as_u64(), part["i"].as_u64(), part["n"].as_u64(), part["d"].as_str()) else { return None };
        let (i, n) = (i as usize, n as usize);
        if n == 0 || n > MOST_PARTS || i >= n {
            return None;
        }
        let bytes = base64::engine::general_purpose::STANDARD.decode(d).ok()?;
        let key = (from.to_string(), id);
        if !self.waiting.contains_key(&key) && self.waiting.len() >= ON_THE_WAY {
            // More at once than anyone sends: the oldest are dropped.
            self.waiting.clear();
        }
        let pieces = self.waiting.entry(key.clone()).or_insert_with(|| vec![None; n]);
        if pieces.len() != n {
            *pieces = vec![None; n];
        }
        pieces[i] = Some(bytes);
        if pieces.iter().any(Option::is_none) {
            return None;
        }
        let whole: Vec<u8> = self.waiting.remove(&key)?.into_iter().flatten().flatten().collect();
        inflate(&whole).ok()
    }
}

impl Wire {
    /// Opens a room at `relay` as its host. [`Event::Opened`] brings the
    /// invitation.
    pub fn host(relay: &str, wake: Wake) -> Result<Wire> {
        let key = VaultKey::new()?;
        Wire::start(relay.trim().trim_end_matches('/').to_string(), None, key, wake)
    }

    /// Joins the session an invitation is for.
    pub fn join(invite: &str, wake: Wake) -> Result<Wire> {
        let (relay, room, key) = read_invite(invite)?;
        Wire::start(relay, Some(room), key, wake)
    }

    fn start(relay: String, room: Option<String>, key: VaultKey, wake: Wake) -> Result<Wire> {
        if !relay.starts_with("https://") && !relay.starts_with("http://") {
            bail!("A relay's address starts with https://");
        }
        let (out, outgoing) = channel::<Out>();
        let (tell, events) = channel::<Event>();
        thread::spawn(move || {
            let say = |e: Event| {
                let _ = tell.send(e);
                wake();
            };
            let why = match run(&relay, room, &key, &outgoing, &say) {
                Ok(()) => String::new(),
                Err(e) => format!("{e:#}"),
            };
            say(Event::Closed { why });
        });
        Ok(Wire { out, events })
    }

    /// Sends to one guest, or to all (`None`). A guest's goes to the host.
    pub fn send(&self, to: Option<&str>, data: Value) {
        let _ = self.out.send(Out::To(to.map(str::to_string), data));
    }

    /// The host sends a guest away.
    pub fn kick(&self, id: &str) {
        let _ = self.out.send(Out::Kick(id.to_string()));
    }

    /// What arrived since the last call.
    pub fn events(&self) -> Vec<Event> {
        let mut all = Vec::new();
        loop {
            match self.events.try_recv() {
                Ok(e) => all.push(e),
                Err(TryRecvError::Empty) => return all,
                Err(TryRecvError::Disconnected) => return all,
            }
        }
    }
}

impl Drop for Wire {
    fn drop(&mut self) {
        let _ = self.out.send(Out::Leave);
    }
}

/// How one connection to the relay ended.
enum End {
    /// The window left the session, or the relay closed the room.
    Done,
    /// The connection broke: worth trying again (the host).
    Cut(anyhow::Error),
    /// The relay refused: trying again won't help.
    Over(anyhow::Error),
}

/// What a connection keeps across connections: the room, and the host's
/// key to get it back.
struct Room {
    hosting: bool,
    id: Option<String>,
    reclaim: Option<String>,
}

fn run(relay: &str, room: Option<String>, key: &VaultKey, outgoing: &Receiver<Out>, say: &dyn Fn(Event)) -> Result<()> {
    let mut room = Room { hosting: room.is_none(), id: room, reclaim: None };
    let mut parts = Parts::default();
    let mut next = 0u64;
    let mut away_since: Option<Instant> = None;
    loop {
        let cut = match serve(relay, &mut room, key, outgoing, say, &mut parts, &mut next, &mut away_since) {
            End::Done => return Ok(()),
            End::Over(e) => return Err(e),
            End::Cut(e) => e,
        };
        // Only the host gets its room back, once it had one.
        if !room.hosting || room.reclaim.is_none() {
            return Err(cut);
        }
        let since = *away_since.get_or_insert_with(|| {
            say(Event::Away { why: format!("{cut:#}") });
            Instant::now()
        });
        if since.elapsed() > HOST_AWAY {
            return Err(cut);
        }
        // A moment between tries; meanwhile what the window sends is
        // dropped (the guests catch up when the host is back), unless it
        // leaves.
        for _ in 0..20 {
            loop {
                match outgoing.try_recv() {
                    Ok(Out::Leave) | Err(TryRecvError::Disconnected) => return Ok(()),
                    Ok(_) => {}
                    Err(TryRecvError::Empty) => break,
                }
            }
            thread::sleep(Duration::from_millis(100));
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn serve(relay: &str, room: &mut Room, key: &VaultKey, outgoing: &Receiver<Out>, say: &dyn Fn(Event), parts: &mut Parts, next: &mut u64, away_since: &mut Option<Instant>) -> End {
    let mut socket = match connect_socket(&format!("{relay}/ws")) {
        Ok(s) => s,
        Err(e) => return End::Cut(anyhow!("Couldn't reach the relay at {relay}: {e:#}")),
    };
    let send = |socket: &mut tungstenite::WebSocket<_>, message: Value| socket.send(tungstenite::Message::text(message.to_string())).map_err(|e| anyhow!("The connection to the relay broke: {e}"));
    macro_rules! cut {
        ($e:expr) => {
            match $e {
                Ok(v) => v,
                Err(e) => return End::Cut(e),
            }
        };
    }
    let mut asked = false;
    let mut last_ping = Instant::now();
    let mut heard = Instant::now();
    loop {
        // What the window wants sent.
        loop {
            match outgoing.try_recv() {
                Ok(Out::To(to, data)) => match pack(key, &data, next) {
                    Ok(sealed) => {
                        for piece in sealed {
                            cut!(send(&mut socket, match &to {
                                Some(id) => json!({ "t": "to", "id": id, "data": piece }),
                                None => json!({ "t": "to", "data": piece }),
                            }));
                        }
                    }
                    // The session goes on without this one message.
                    Err(e) => say(Event::Trouble { why: format!("{e:#}") }),
                },
                Ok(Out::Kick(id)) => cut!(send(&mut socket, json!({ "t": "kick", "id": id }))),
                Ok(Out::Leave) | Err(TryRecvError::Disconnected) => {
                    let _ = send(&mut socket, json!({ "t": "leave" }));
                    let _ = socket.close(None);
                    return End::Done;
                }
                Err(TryRecvError::Empty) => break,
            }
        }
        // A sign of life: proxies cut connections that say nothing.
        if last_ping.elapsed() > Duration::from_secs(25) {
            cut!(send(&mut socket, json!({ "t": "ping" })));
            last_ping = Instant::now();
        }
        if heard.elapsed() > QUIET {
            return End::Cut(anyhow!("The relay stopped answering."));
        }
        let text = match socket.read() {
            Ok(message) => {
                heard = Instant::now();
                match message {
                    tungstenite::Message::Text(text) => text,
                    tungstenite::Message::Close(_) => return End::Cut(anyhow!("The relay closed the connection.")),
                    _ => continue,
                }
            }
            Err(tungstenite::Error::Io(e)) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => continue,
            Err(e) => return End::Cut(anyhow!("The connection to the relay broke: {e}")),
        };
        let v: Value = serde_json::from_str(&text).unwrap_or_default();
        match v["t"].as_str().unwrap_or_default() {
            // The relay says what it is first: then the room is asked for
            // (or, for a host back after a dropped connection, asked back).
            "status" if !asked => {
                if v["name"] != "xc-circuit-host" {
                    return End::Over(anyhow!("That address answers, but not as an XC Circuit relay."));
                }
                asked = true;
                let ask = match (&room.id, &room.reclaim) {
                    (Some(id), Some(reclaim)) if room.hosting => json!({ "t": "reopen", "room": id, "key": reclaim }),
                    (Some(id), _) if !room.hosting => json!({ "t": "join", "room": id }),
                    _ => json!({ "t": "open" }),
                };
                cut!(send(&mut socket, ask));
            }
            "opened" => {
                let id = v["room"].as_str().unwrap_or_default().to_string();
                if away_since.take().is_some() {
                    say(Event::Back);
                }
                room.reclaim = v["key"].as_str().map(str::to_string).filter(|k| !k.is_empty());
                say(Event::Opened { invite: write_invite(relay, &id, key) });
                room.id = Some(id);
            }
            // A guest: in the room, or the host is back in it.
            "joined" | "back" => say(Event::Joined),
            "away" => say(Event::HostAway),
            "peer" => say(Event::Peer { id: v["id"].as_str().unwrap_or_default().into(), on: v["on"].as_bool().unwrap_or(false) }),
            "from" => {
                let from = v["id"].as_str().unwrap_or_default().to_string();
                // One that can't be opened isn't from this session: let go.
                if let Some(data) = v["data"].as_str().and_then(|sealed| unpack(key, sealed).ok())
                    && let Some(data) = parts.take(&from, data)
                {
                    say(Event::From { id: from, data });
                }
            }
            "closed" => return End::Done,
            "error" => return End::Over(anyhow!("{}", v["why"].as_str().unwrap_or("The relay refused."))),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invitations_and_sealed_messages() {
        let key = VaultKey::new().unwrap();
        let invite = write_invite("https://xcrelay.xcrafttm.dev/", "0123abcd", &key);
        assert!(invite.starts_with("xc-live:https://xcrelay.xcrafttm.dev/0123abcd#") && !invite.contains(' '), "{invite}");
        let (relay, room, read) = read_invite(&format!("  {invite}\n")).unwrap();
        assert!(relay == "https://xcrelay.xcrafttm.dev" && room == "0123abcd" && read == key);
        for bad in ["", "https://xcrelay.xcrafttm.dev/room#key", "xc-live:room#key", &invite[..invite.len() - 5], "xc-live:https://relay/room"] {
            assert!(read_invite(bad).is_err(), "{bad}");
        }
        // Packed and sealed: small, unreadable, and only this session's key opens it.
        let mut next = 0;
        let data = json!({ "m": "update", "state": { "nodes": "x".repeat(50_000) } });
        let sealed = pack(&key, &data, &mut next).unwrap();
        assert!(sealed.len() == 1 && sealed[0].len() < 2000 && !sealed[0].contains("update"), "{}", sealed[0].len());
        assert_eq!(unpack(&key, &sealed[0]).unwrap(), data);
        assert!(unpack(&VaultKey::new().unwrap(), &sealed[0]).is_err());
    }

    /// A message bigger than the relay passes on goes in parts, each small
    /// enough, and comes out whole once the last part is in, whatever the
    /// order; parts from two senders don't mix.
    #[test]
    fn big_messages_go_in_parts() {
        let key = VaultKey::new().unwrap();
        let mut next = 0;
        // Random letters: nothing to pack, about a megabyte.
        let noise: String = (0..12_000).map(|_| VaultKey::new().unwrap().to_text()).collect();
        let data = json!({ "m": "welcome", "docs": { "main": noise } });
        let sealed = pack(&key, &data, &mut next).unwrap();
        assert!(sealed.len() > 2 && sealed.iter().all(|s| s.len() <= MOST), "{} parts", sealed.len());
        let mut parts = Parts::default();
        let opened: Vec<Value> = sealed.iter().map(|s| unpack(&key, s).unwrap()).collect();
        let mut order: Vec<usize> = (0..opened.len()).collect();
        order.reverse();
        let (last, rest) = order.split_last().unwrap();
        for i in rest {
            assert_eq!(parts.take("a", opened[*i].clone()), None, "not whole yet");
            assert_eq!(parts.take("b", opened[*i].clone()), None);
        }
        assert_eq!(parts.take("a", opened[*last].clone()), Some(data.clone()));
        assert_eq!(parts.take("b", opened[*last].clone()), Some(data));
        assert!(parts.waiting.is_empty());
        // A whole message passes straight through.
        assert_eq!(parts.take("a", json!({ "m": "hello" })), Some(json!({ "m": "hello" })));
    }

    /// Through a running relay (`XC_RELAY_TEST` is its address): a host, a
    /// guest, and a stranger with another key. Skipped without one.
    #[test]
    fn a_session_over_a_relay() {
        let Ok(relay) = std::env::var("XC_RELAY_TEST") else { return };
        let next = |wire: &Wire| -> Event {
            for _ in 0..200 {
                if let Some(e) = wire.events().into_iter().next() {
                    return e;
                }
                thread::sleep(Duration::from_millis(50));
            }
            panic!("nothing came");
        };
        let wake: Wake = Arc::new(|| {});
        let host = Wire::host(&relay, wake.clone()).unwrap();
        let Event::Opened { invite } = next(&host) else { panic!("not opened") };
        let guest = Wire::join(&invite, wake.clone()).unwrap();
        assert_eq!(next(&guest), Event::Joined);
        let Event::Peer { id, on: true } = next(&host) else { panic!("no guest") };
        guest.send(None, json!({ "m": "hello", "name": "Grace" }));
        assert_eq!(next(&host), Event::From { id: id.clone(), data: json!({ "m": "hello", "name": "Grace" }) });
        host.send(Some(&id), json!({ "big": "y".repeat(300_000) }));
        let Event::From { id: from, data } = next(&guest) else { panic!("nothing from the host") };
        assert!(from == "host" && data["big"].as_str().unwrap().len() == 300_000);
        host.send(None, json!("to all"));
        assert_eq!(next(&guest), Event::From { id: "host".into(), data: json!("to all") });
        // The host leaves: over for the guest.
        drop(host);
        assert!(matches!(next(&guest), Event::Closed { .. }));
        assert!(matches!(next(&Wire::join(&invite, wake).unwrap()), Event::Closed { why } if why.contains("isn't open")));
    }
}
