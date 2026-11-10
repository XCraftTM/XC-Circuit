//! The app's side of an XC Circuit Host (`host/server.js`): pairing this
//! device with it, and keeping a project's secrets in its vault.
//!
//! The host never sees a secret. Paired devices share one **vault key**;
//! secrets are sealed with it on the device before they are sent. The first
//! device makes the key. A further one gets it from a device that has it,
//! wrapped for it alone: the two agree on a key over the host (X25519), and
//! mix in half of the pairing code, which is typed from one device into the
//! other and never sent. So a host that swapped the keys in between still
//! couldn't unwrap it.

use std::time::{Duration, Instant};

use anyhow::{Result, anyhow, bail};
use base64::Engine;
use ring::aead::{self, Aad, LessSafeKey, Nonce, UnboundKey};
use ring::agreement::{self, EphemeralPrivateKey, UnparsedPublicKey};
use ring::hkdf;
use ring::rand::{SecureRandom, SystemRandom};
use serde_json::{Value, json};

use crate::hosting::calagopus::{Stream, connect_socket};

/// The protocol this app speaks ([`Status::protocol`]).
pub const PROTOCOL: u64 = 1;
/// The letters of a code: none that look alike.
const LETTERS: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn un64(text: &str) -> Result<Vec<u8>> {
    base64::engine::general_purpose::STANDARD.decode(text.trim()).map_err(|_| anyhow!("That isn't something this app sealed."))
}

fn random<const N: usize>() -> Result<[u8; N]> {
    let mut bytes = [0u8; N];
    SystemRandom::new().fill(&mut bytes).map_err(|_| anyhow!("The system gave no random numbers."))?;
    Ok(bytes)
}

/// The key the paired devices share. The host never has it.
#[derive(Clone, PartialEq, Eq)]
pub struct VaultKey([u8; 32]);

/// Never printed: not in a log, not in a test's message.
impl std::fmt::Debug for VaultKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("VaultKey(…)")
    }
}

impl VaultKey {
    pub fn new() -> Result<VaultKey> {
        Ok(VaultKey(random()?))
    }

    /// For the credential store.
    pub fn to_text(&self) -> String {
        b64(&self.0)
    }

    pub fn from_text(text: &str) -> Result<VaultKey> {
        Ok(VaultKey(un64(text)?.try_into().map_err(|_| anyhow!("That isn't a vault key."))?))
    }

    /// The key as a code to write down: with it the secrets can be read
    /// again when every paired device is gone. 52 letters in groups of 4.
    pub fn recovery_code(&self) -> String {
        // Base 32, five bits a letter.
        let mut out = String::new();
        let (mut bits, mut have) = (0u32, 0);
        for byte in self.0 {
            bits = (bits << 8) | byte as u32;
            have += 8;
            while have >= 5 {
                have -= 5;
                out.push(LETTERS[((bits >> have) & 31) as usize] as char);
            }
        }
        if have > 0 {
            out.push(LETTERS[((bits << (5 - have)) & 31) as usize] as char);
        }
        out.as_bytes().chunks(4).map(|c| std::str::from_utf8(c).unwrap_or_default()).collect::<Vec<_>>().join("-")
    }

    pub fn from_recovery_code(code: &str) -> Result<VaultKey> {
        let mut bytes = Vec::new();
        let (mut bits, mut have) = (0u32, 0);
        for c in code.bytes().filter(|c| c.is_ascii_alphanumeric()) {
            let value = LETTERS.iter().position(|l| *l == c.to_ascii_uppercase()).ok_or_else(|| anyhow!("That isn't a recovery code."))?;
            bits = (bits << 5) | value as u32;
            have += 5;
            if have >= 8 {
                have -= 8;
                bytes.push((bits >> have) as u8);
            }
        }
        Ok(VaultKey(bytes.try_into().map_err(|_| anyhow!("That isn't a recovery code: it has 52 letters and digits."))?))
    }

    /// Seals `plain` for the vault.
    pub fn seal(&self, plain: &[u8]) -> Result<String> {
        seal_with(&self.0, plain)
    }

    /// Opens what a device with this key sealed.
    pub fn open(&self, sealed: &str) -> Result<Vec<u8>> {
        open_with(&self.0, sealed).map_err(|_| anyhow!("These secrets were sealed with another key: this device isn't paired with the devices that stored them."))
    }
}

fn cipher(key: &[u8; 32]) -> Result<LessSafeKey> {
    Ok(LessSafeKey::new(UnboundKey::new(&aead::CHACHA20_POLY1305, key).map_err(|_| anyhow!("That key can't be used."))?))
}

/// nonce ‖ sealed, as text.
fn seal_with(key: &[u8; 32], plain: &[u8]) -> Result<String> {
    let nonce: [u8; 12] = random()?;
    let mut data = plain.to_vec();
    cipher(key)?.seal_in_place_append_tag(Nonce::assume_unique_for_key(nonce), Aad::empty(), &mut data).map_err(|_| anyhow!("Sealing didn't work."))?;
    Ok(b64(&[nonce.as_slice(), &data].concat()))
}

fn open_with(key: &[u8; 32], sealed: &str) -> Result<Vec<u8>> {
    let mut data = un64(sealed)?;
    if data.len() < 12 + 16 {
        bail!("too short");
    }
    let nonce: [u8; 12] = data[..12].try_into().expect("12 bytes");
    let mut body = data.split_off(12);
    let plain = cipher(key)?.open_in_place(Nonce::assume_unique_for_key(nonce), Aad::empty(), &mut body).map_err(|_| anyhow!("wrong key"))?.len();
    body.truncate(plain);
    Ok(body)
}

/// The key two devices agree on: what X25519 gives them, with the half of
/// the pairing code only they know mixed in.
fn agreed(shared: &[u8], secret: &str) -> Result<[u8; 32]> {
    let tidy: String = secret.chars().filter(|c| c.is_ascii_alphanumeric()).map(|c| c.to_ascii_uppercase()).collect();
    let salt = hkdf::Salt::new(hkdf::HKDF_SHA256, b"xc-circuit pairing v1");
    let mut key = [0u8; 32];
    salt.extract(&[shared, tidy.as_bytes()].concat()).expand(&[b"vault key"], hkdf::HKDF_SHA256).and_then(|okm| okm.fill(&mut key)).map_err(|_| anyhow!("The keys couldn't be worked out."))?;
    Ok(key)
}

/// The new device's half of the exchange, alive while it waits for the key.
pub struct Half {
    private: EphemeralPrivateKey,
    /// What the other device needs of it: goes to the host with `pair`.
    pub public: String,
}

impl Half {
    pub fn new() -> Result<Half> {
        let private = EphemeralPrivateKey::generate(&agreement::X25519, &SystemRandom::new()).map_err(|_| anyhow!("The system gave no random numbers."))?;
        let public = b64(private.compute_public_key().map_err(|_| anyhow!("The key couldn't be made."))?.as_ref());
        Ok(Half { private, public })
    }

    /// The vault key another device wrapped for this one with [`wrap`].
    pub fn unwrap(self, wrapped: &str, secret: &str) -> Result<VaultKey> {
        let data = un64(wrapped)?;
        if data.len() < 32 {
            bail!("That isn't a wrapped key.");
        }
        let (theirs, sealed) = data.split_at(32);
        let key = agreement::agree_ephemeral(self.private, &UnparsedPublicKey::new(&agreement::X25519, theirs), |shared| agreed(shared, secret)).map_err(|_| anyhow!("The keys couldn't be worked out."))??;
        let plain = open_with(&key, &b64(sealed)).map_err(|_| anyhow!("The key didn't open: is the code typed right? Pair again with a new code."))?;
        Ok(VaultKey(plain.try_into().map_err(|_| anyhow!("That isn't a vault key."))?))
    }
}

/// Wraps the vault key for the device whose half is `their_public`, with
/// the part of the pairing code that was never sent.
pub fn wrap(key: &VaultKey, their_public: &str, secret: &str) -> Result<String> {
    let mine = EphemeralPrivateKey::generate(&agreement::X25519, &SystemRandom::new()).map_err(|_| anyhow!("The system gave no random numbers."))?;
    let public = mine.compute_public_key().map_err(|_| anyhow!("The key couldn't be made."))?;
    let theirs = un64(their_public)?;
    let wrap_key = agreement::agree_ephemeral(mine, &UnparsedPublicKey::new(&agreement::X25519, &theirs), |shared| agreed(shared, secret)).map_err(|_| anyhow!("The other device's key isn't one."))??;
    let sealed = un64(&seal_with(&wrap_key, &key.0)?)?;
    Ok(b64(&[public.as_ref(), &sealed].concat()))
}

/// The half of a pairing code that stays between the two devices:
/// `ABCD-EFGH`.
pub fn code_secret() -> Result<String> {
    let bytes: [u8; 8] = random()?;
    let text: String = bytes.iter().map(|b| LETTERS[(b % 32) as usize] as char).collect();
    Ok(format!("{}-{}", &text[..4], &text[4..]))
}

/// A pairing code taken apart: the part the host checks, and the part only
/// the devices know (empty for the first device's code from the log).
pub fn code_parts(code: &str) -> (String, String) {
    let tidy: String = code.chars().filter(|c| c.is_ascii_alphanumeric()).map(|c| c.to_ascii_uppercase()).collect();
    let group = |s: &str| if s.len() == 8 { format!("{}-{}", &s[..4], &s[4..]) } else { s.to_string() };
    if tidy.len() > 8 { (group(&tidy[..8]), group(&tidy[8..])) } else { (group(&tidy), String::new()) }
}

/// What a host says about itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Status {
    pub mode: String,
    pub version: String,
    pub protocol: u64,
    /// What it can do: `relay`, `projects`, `vault`.
    pub can: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Device {
    pub id: String,
    pub name: String,
    /// Its half of the key exchange, from when it was paired.
    pub key: String,
    pub created: i64,
}

/// A connection to a host.
pub struct Link {
    socket: tungstenite::WebSocket<Box<dyn Stream>>,
    pub status: Status,
}

/// The address to store a project at on a host: `https://host/git/name.git`.
pub fn project_address(host: &str, name: &str) -> String {
    let mut slug: String = name.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    while slug.contains("--") {
        slug = slug.replace("--", "-");
    }
    let slug = slug.trim_matches('-');
    format!("{}/git/{}.git", host.trim().trim_end_matches('/'), if slug.is_empty() { "project" } else { slug })
}

impl Link {
    /// Connects to the host at `address` (`https://circuit.example.com`).
    pub fn open(address: &str) -> Result<Link> {
        let address = address.trim().trim_end_matches('/');
        if !address.starts_with("https://") && !address.starts_with("http://") {
            bail!("A host's address starts with https://");
        }
        let socket = connect_socket(&format!("{address}/ws")).map_err(|e| anyhow!("Couldn't reach the host at {address}: {e:#}"))?;
        let mut link = Link { socket, status: Status { mode: String::new(), version: String::new(), protocol: 0, can: Vec::new() } };
        let v = link.wait("status", Duration::from_secs(20))?;
        if v["name"] != "xc-circuit-host" {
            bail!("That address answers, but not as an XC Circuit Host.");
        }
        link.status = Status {
            mode: v["mode"].as_str().unwrap_or_default().into(),
            version: v["version"].as_str().unwrap_or_default().into(),
            protocol: v["protocol"].as_u64().unwrap_or(0),
            can: v["can"].as_array().into_iter().flatten().filter_map(|c| c.as_str().map(str::to_string)).collect(),
        };
        if link.status.protocol > PROTOCOL {
            bail!("The host is newer than this app (it speaks protocol {}, this app {PROTOCOL}): update XC Circuit.", link.status.protocol);
        }
        if link.status.protocol < PROTOCOL {
            bail!("The host is older than this app (it speaks protocol {}, this app {PROTOCOL}): update the host.", link.status.protocol);
        }
        Ok(link)
    }

    /// Whether it keeps things (full mode), or only passes live sessions on.
    pub fn keeps(&self) -> bool {
        self.status.can.iter().any(|c| c == "vault")
    }

    fn send(&mut self, message: Value) -> Result<()> {
        self.socket.send(tungstenite::Message::text(message.to_string())).map_err(|e| anyhow!("The connection to the host broke: {e}"))
    }

    /// The next message of the kind `want`. An `error` from the host ends
    /// the wait with its words.
    fn wait(&mut self, want: &str, within: Duration) -> Result<Value> {
        let until = Instant::now() + within;
        loop {
            match self.socket.read() {
                Ok(tungstenite::Message::Text(text)) => {
                    let v: Value = serde_json::from_str(&text).unwrap_or_default();
                    if v["t"] == want {
                        return Ok(v);
                    }
                    if v["t"] == "error" {
                        bail!("{}", v["why"].as_str().unwrap_or("The host refused."));
                    }
                }
                Ok(tungstenite::Message::Close(_)) => bail!("The host closed the connection."),
                Ok(_) => {}
                Err(tungstenite::Error::Io(e)) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {}
                Err(e) => bail!("The connection to the host broke: {e}"),
            }
            if Instant::now() > until {
                bail!("The host didn't answer.");
            }
        }
    }

    fn ask(&mut self, message: Value, want: &str) -> Result<Value> {
        self.send(message)?;
        self.wait(want, Duration::from_secs(20))
    }

    /// Pairs this device: the host's part of the code, a name for the
    /// device, and its half of the key exchange. Gives the device's id and
    /// its token.
    pub fn pair(&mut self, code: &str, name: &str, half: &str) -> Result<(String, String)> {
        let v = self.ask(json!({ "t": "pair", "code": code, "name": name, "key": half }), "paired")?;
        Ok((v["device"].as_str().unwrap_or_default().into(), v["token"].as_str().unwrap_or_default().into()))
    }

    /// Says who this is, with the token from pairing. Gives the device's name.
    pub fn hello(&mut self, token: &str) -> Result<String> {
        Ok(self.ask(json!({ "t": "hello", "token": token }), "welcome")?["name"].as_str().unwrap_or_default().into())
    }

    /// A code that lets one more device in (the host's part of it).
    pub fn invite(&mut self) -> Result<String> {
        Ok(self.ask(json!({ "t": "invite" }), "invited")?["code"].as_str().unwrap_or_default().into())
    }

    pub fn devices(&mut self) -> Result<Vec<Device>> {
        let v = self.ask(json!({ "t": "devices" }), "devices")?;
        Ok(v["devices"].as_array().into_iter().flatten().map(|d| Device { id: d["id"].as_str().unwrap_or_default().into(), name: d["name"].as_str().unwrap_or_default().into(), key: d["key"].as_str().unwrap_or_default().into(), created: d["created"].as_i64().unwrap_or(0) }).collect())
    }

    pub fn remove_device(&mut self, id: &str) -> Result<()> {
        self.ask(json!({ "t": "device.remove", "id": id }), "device.removed").map(|_| ())
    }

    /// A project's sealed secrets and their count (0: none yet).
    pub fn vault_get(&mut self, project: &str) -> Result<(Option<String>, u64)> {
        let v = self.ask(json!({ "t": "vault.get", "project": project }), "vault")?;
        Ok((v["blob"].as_str().map(str::to_string), v["rev"].as_u64().unwrap_or(0)))
    }

    /// Keeps a project's sealed secrets; `rev` is the count last seen.
    pub fn vault_put(&mut self, project: &str, sealed: &str, rev: u64) -> Result<u64> {
        Ok(self.ask(json!({ "t": "vault.put", "project": project, "blob": sealed, "rev": rev }), "vault.kept")?["rev"].as_u64().unwrap_or(0))
    }

    /// Leaves a wrapped key for another device.
    pub fn key_put(&mut self, device: &str, wrapped: &str) -> Result<()> {
        self.ask(json!({ "t": "key.put", "for": device, "blob": wrapped }), "key.kept").map(|_| ())
    }

    /// The wrapped keys waiting for this device (who from, the key): asked
    /// for until one is there or `within` is over.
    pub fn wait_keys(&mut self, within: Duration) -> Result<Vec<(String, String)>> {
        let until = Instant::now() + within;
        loop {
            let v = self.ask(json!({ "t": "key.get" }), "keys")?;
            let keys: Vec<(String, String)> = v["keys"].as_array().into_iter().flatten().map(|k| (k["from"].as_str().unwrap_or_default().to_string(), k["blob"].as_str().unwrap_or_default().to_string())).collect();
            if !keys.is_empty() || Instant::now() > until {
                return Ok(keys);
            }
            std::thread::sleep(Duration::from_millis(700));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secrets_are_sealed_and_the_key_is_passed_on() {
        let key = VaultKey::new().unwrap();
        let sealed = key.seal(br#"{"DISCORD_TOKEN":"abc"}"#).unwrap();
        assert!(!sealed.contains("abc") && key.seal(b"x").unwrap() != key.seal(b"x").unwrap(), "sealed, and never the same twice");
        assert_eq!(key.open(&sealed).unwrap(), br#"{"DISCORD_TOKEN":"abc"}"#);
        assert!(VaultKey::new().unwrap().open(&sealed).unwrap_err().to_string().contains("another key"));
        assert!(key.open("not sealed").is_err() && key.open("").is_err());
        assert!(VaultKey::from_text(&key.to_text()).unwrap() == key);

        // To write down, and read back however it is typed.
        let code = key.recovery_code();
        assert_eq!((code.len(), code.split('-').count()), (52 + 12, 13), "{code}");
        assert!(VaultKey::from_recovery_code(&code.to_lowercase().replace('-', " ")).unwrap() == key);
        assert!(VaultKey::from_recovery_code("ABCD-EFGH").is_err() && VaultKey::from_recovery_code("0000").is_err());

        // A new device gets the key from one that has it: only with the
        // part of the code that was typed, not sent.
        let secret = code_secret().unwrap();
        assert_eq!((secret.len(), &secret[4..5]), (9, "-"));
        let half = Half::new().unwrap();
        let wrapped = wrap(&key, &half.public, &secret).unwrap();
        assert!(half.unwrap(&wrapped, &secret.to_lowercase()).unwrap() == key);
        let half = Half::new().unwrap();
        let wrapped = wrap(&key, &half.public, &secret).unwrap();
        assert!(half.unwrap(&wrapped, "AAAA-AAAA").unwrap_err().to_string().contains("typed right"));
        // Wrapped for another device: of no use to this one.
        let (one, other) = (Half::new().unwrap(), Half::new().unwrap());
        let wrapped = wrap(&key, &one.public, &secret).unwrap();
        assert!(other.unwrap(&wrapped, &secret).is_err());

        assert_eq!(code_parts("abcd-efgh-jklm-npqr"), ("ABCD-EFGH".into(), "JKLM-NPQR".into()));
        assert_eq!(code_parts(" ABCD EFGH "), ("ABCD-EFGH".into(), String::new()));
        assert_eq!(project_address("https://circuit.example.com/", "My Bot 2!"), "https://circuit.example.com/git/my-bot-2.git");
        assert_eq!(project_address("https://h", "???"), "https://h/git/project.git");
    }

    /// Against a running host (`XC_HOST_TEST` names a file with its Git
    /// address and a paired device's token, and a third line: a setup or
    /// invite code is not needed, this pairs with an invite of its own).
    #[test]
    fn pairs_and_keeps_secrets_at_a_host() {
        let Some(file) = std::env::var_os("XC_HOST_TEST") else { return };
        let text = std::fs::read_to_string(file).unwrap();
        let mut lines = text.lines();
        let (git, token) = (lines.next().unwrap(), lines.next().unwrap());
        let address = git.split("/git/").next().unwrap();

        // The first device: paired already, makes the vault key.
        let mut first = Link::open(address).unwrap();
        assert!(first.keeps() && first.status.mode == "full");
        assert!(first.vault_get("abc").unwrap_err().to_string().contains("Pair this device first"));
        assert_eq!(first.hello(token).unwrap(), "L");
        let key = VaultKey::new().unwrap();

        // It lets a second one in: the host's part of the code, and a part
        // that goes from screen to keyboard only.
        let (host_part, secret) = (first.invite().unwrap(), code_secret().unwrap());
        let (for_host, for_us) = code_parts(&format!("{host_part}-{secret}"));
        assert_eq!((for_host.as_str(), for_us.as_str()), (host_part.as_str(), secret.as_str()));
        let mut second = Link::open(address).unwrap();
        let half = Half::new().unwrap();
        let (device, second_token) = second.pair(&for_host, "Tablet", &half.public).unwrap();
        assert!(second.pair(&for_host, "Again", "x").is_err(), "a code is good once");
        let new = first.devices().unwrap().into_iter().find(|d| d.id == device).unwrap();
        assert_eq!((new.name.as_str(), new.key.as_str()), ("Tablet", half.public.as_str()));
        first.key_put(&device, &wrap(&key, &new.key, &secret).unwrap()).unwrap();
        let keys = second.wait_keys(Duration::from_secs(5)).unwrap();
        let got = half.unwrap(&keys[0].1, &for_us).unwrap();
        assert!(got == key);

        // Secrets: sealed here, kept there, read on the other device.
        let project = format!("p{}", std::process::id());
        assert_eq!(first.vault_get(&project).unwrap(), (None, 0));
        let rev = first.vault_put(&project, &key.seal(b"{\"TOKEN\":\"t\"}").unwrap(), 0).unwrap();
        assert_eq!(rev, 1);
        assert!(first.vault_put(&project, "x", 0).unwrap_err().to_string().contains("newer secrets"));
        let (sealed, rev) = second.vault_get(&project).unwrap();
        assert_eq!((got.open(&sealed.unwrap()).unwrap(), rev), (b"{\"TOKEN\":\"t\"}".to_vec(), 1));
        drop(second);
        let mut again = Link::open(address).unwrap();
        assert_eq!(again.hello(&second_token).unwrap(), "Tablet");
        first.remove_device(&device).unwrap();
        assert!(Link::open(address).unwrap().hello(&second_token).is_err());
        assert!(Link::open("ftp://x").is_err() && Link::open("http://127.0.0.1:9").is_err());
    }
}
