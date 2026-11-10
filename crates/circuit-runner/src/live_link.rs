//! The wire of a live session: a room at a relay (an XC Circuit Host, or
//! the public relay), with everything in it sealed between the apps.
//!
//! The relay passes messages on and can't read them: each one is packed
//! (deflate) and sealed with the session's key, which only travels in the
//! invitation, from person to person.

use std::io::{Read, Write};
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow, bail};
use serde_json::{Value, json};

use crate::host_link::VaultKey;
use crate::hosting::calagopus::connect_socket;

/// The relay everyone can use, for people without a host of their own.
pub const PUBLIC_RELAY: &str = "https://xcrelay.xcrafttm.dev";
/// What an invitation starts with.
const SCHEME: &str = "xc-live:";
/// The most a relay passes on in one message, with room to spare.
const MOST: usize = 240 * 1024;

/// What the wire says.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// The host's room is open: the invitation to pass on.
    Opened { invite: String },
    /// This guest is in the room.
    Joined,
    /// A guest came (`on`) or went.
    Peer { id: String, on: bool },
    /// A message, opened: from the guest `id`, or from `host`.
    From { id: String, data: Value },
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

fn pack(key: &VaultKey, data: &Value) -> Result<String> {
    let mut packed = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::fast());
    packed.write_all(data.to_string().as_bytes())?;
    let sealed = key.seal(&packed.finish()?)?;
    if sealed.len() > MOST {
        bail!("That is too much for one message of a session (a very big workspace).");
    }
    Ok(sealed)
}

fn unpack(key: &VaultKey, sealed: &str) -> Result<Value> {
    let packed = key.open(sealed).map_err(|_| anyhow!("A message couldn't be opened: it isn't from this session."))?;
    let mut text = String::new();
    flate2::read::DeflateDecoder::new(packed.as_slice()).take(64 * 1024 * 1024).read_to_string(&mut text)?;
    Ok(serde_json::from_str(&text)?)
}

impl Wire {
    /// Opens a room at `relay` as its host. [`Event::Opened`] brings the
    /// invitation.
    pub fn host(relay: &str) -> Result<Wire> {
        let key = VaultKey::new()?;
        Wire::start(relay.trim().trim_end_matches('/').to_string(), None, key)
    }

    /// Joins the session an invitation is for.
    pub fn join(invite: &str) -> Result<Wire> {
        let (relay, room, key) = read_invite(invite)?;
        Wire::start(relay, Some(room), key)
    }

    fn start(relay: String, room: Option<String>, key: VaultKey) -> Result<Wire> {
        if !relay.starts_with("https://") && !relay.starts_with("http://") {
            bail!("A relay's address starts with https://");
        }
        let (out, outgoing) = channel::<Out>();
        let (tell, events) = channel::<Event>();
        thread::spawn(move || {
            let why = match run(&relay, room, &key, &outgoing, &tell) {
                Ok(()) => String::new(),
                Err(e) => format!("{e:#}"),
            };
            let _ = tell.send(Event::Closed { why });
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

fn run(relay: &str, room: Option<String>, key: &VaultKey, outgoing: &Receiver<Out>, tell: &Sender<Event>) -> Result<()> {
    let mut socket = connect_socket(&format!("{relay}/ws")).map_err(|e| anyhow!("Couldn't reach the relay at {relay}: {e:#}"))?;
    let say = |socket: &mut tungstenite::WebSocket<_>, message: Value| socket.send(tungstenite::Message::text(message.to_string())).map_err(|e| anyhow!("The connection to the relay broke: {e}"));
    let mut asked = false;
    let mut last_ping = Instant::now();
    loop {
        // What the window wants sent.
        loop {
            match outgoing.try_recv() {
                Ok(Out::To(to, data)) => {
                    let sealed = pack(key, &data)?;
                    say(&mut socket, match to {
                        Some(id) => json!({ "t": "to", "id": id, "data": sealed }),
                        None => json!({ "t": "to", "data": sealed }),
                    })?;
                }
                Ok(Out::Kick(id)) => say(&mut socket, json!({ "t": "kick", "id": id }))?,
                Ok(Out::Leave) | Err(TryRecvError::Disconnected) => {
                    let _ = say(&mut socket, json!({ "t": "leave" }));
                    let _ = socket.close(None);
                    return Ok(());
                }
                Err(TryRecvError::Empty) => break,
            }
        }
        // A sign of life: proxies cut connections that say nothing.
        if last_ping.elapsed() > Duration::from_secs(25) {
            say(&mut socket, json!({ "t": "ping" }))?;
            last_ping = Instant::now();
        }
        let text = match socket.read() {
            Ok(tungstenite::Message::Text(text)) => text,
            Ok(tungstenite::Message::Close(_)) => bail!("The relay closed the connection."),
            Ok(_) => continue,
            Err(tungstenite::Error::Io(e)) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => continue,
            Err(e) => bail!("The connection to the relay broke: {e}"),
        };
        let v: Value = serde_json::from_str(&text).unwrap_or_default();
        match v["t"].as_str().unwrap_or_default() {
            // The relay says what it is first: then the room is asked for.
            "status" if !asked => {
                if v["name"] != "xc-circuit-host" {
                    bail!("That address answers, but not as an XC Circuit relay.");
                }
                asked = true;
                say(&mut socket, match &room {
                    Some(room) => json!({ "t": "join", "room": room }),
                    None => json!({ "t": "open" }),
                })?;
            }
            "opened" => {
                let _ = tell.send(Event::Opened { invite: write_invite(relay, v["room"].as_str().unwrap_or_default(), key) });
            }
            "joined" => {
                let _ = tell.send(Event::Joined);
            }
            "peer" => {
                let _ = tell.send(Event::Peer { id: v["id"].as_str().unwrap_or_default().into(), on: v["on"].as_bool().unwrap_or(false) });
            }
            "from" => {
                // One that can't be opened isn't from this session: let go.
                if let Some(data) = v["data"].as_str().and_then(|sealed| unpack(key, sealed).ok()) {
                    let _ = tell.send(Event::From { id: v["id"].as_str().unwrap_or_default().into(), data });
                }
            }
            "closed" => return Ok(()),
            "error" => bail!("{}", v["why"].as_str().unwrap_or("The relay refused.")),
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
        let data = json!({ "m": "update", "state": { "nodes": "x".repeat(50_000) } });
        let sealed = pack(&key, &data).unwrap();
        assert!(sealed.len() < 2000 && !sealed.contains("update"), "{}", sealed.len());
        assert_eq!(unpack(&key, &sealed).unwrap(), data);
        assert!(unpack(&VaultKey::new().unwrap(), &sealed).is_err());
        // Something that doesn't pack: random letters, more than fits.
        let noise: String = (0..10_000).map(|_| VaultKey::new().unwrap().to_text()).collect();
        assert!(pack(&key, &json!(noise)).unwrap_err().to_string().contains("too much"));
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
        let host = Wire::host(&relay).unwrap();
        let Event::Opened { invite } = next(&host) else { panic!("not opened") };
        let guest = Wire::join(&invite).unwrap();
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
        assert!(matches!(next(&Wire::join(&invite).unwrap()), Event::Closed { why } if why.contains("isn't open")));
    }
}
