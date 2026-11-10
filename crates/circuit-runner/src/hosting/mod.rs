//! Uploading an exported bot to the server it runs on: over SFTP, or through
//! a Calagopus panel's API. Only files that changed since
//! the last upload to the same place are sent, and nothing else on the server
//! is touched: its `data/` folder (the bot's saved data) and `node_modules/`
//! stay as they are.

pub mod calagopus;
pub use sftp::forget_key;
mod sftp;

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// How the bot gets to its server.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    #[default]
    None,
    Sftp,
    /// No longer offered (it sends the password and the token unprotected).
    /// Still read, so a project set to it says so instead of not opening.
    Ftp,
    Calagopus,
}

/// Where a project's bot is uploaded to, kept in the project (`data.hosting`).
/// Passwords and API keys are not in it: they're in the credential store
/// ([`PASSWORD`], [`API_KEY`]).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Hosting {
    pub kind: Kind,
    /// SFTP: the server's name or address, its port (0: the usual
    /// one) and the user to log in as.
    pub host: String,
    pub port: u16,
    pub user: String,
    /// SFTP: a private key file to log in with instead of the password (which
    /// then unlocks the key, when it needs one).
    pub key_file: String,
    /// Calagopus: the panel's address and the server (its uuid and name).
    pub panel: String,
    pub server: String,
    pub server_name: String,
    /// The folder on the server the bot goes in: empty for where the login
    /// starts (the home folder; Calagopus: the server's own folder).
    pub folder: String,
    /// Upload after every save.
    pub auto_upload: bool,
    /// Send the `.env` file with the bot token and secrets.
    pub include_token: bool,
    /// Calagopus: restart the server after an upload, so the new bot runs.
    pub restart: bool,
}

impl Default for Hosting {
    fn default() -> Self {
        Hosting {
            kind: Kind::None,
            host: String::new(),
            port: 0,
            user: String::new(),
            key_file: String::new(),
            panel: String::new(),
            server: String::new(),
            server_name: String::new(),
            folder: String::new(),
            auto_upload: false,
            include_token: true,
            restart: true,
        }
    }
}

/// What a project still set to FTP is told.
pub const FTP_GONE: &str = "FTP is no longer offered: it sends the password and the bot token unprotected. Choose SFTP or a Calagopus panel.";

/// The credential store's names for a project's hosting secrets.
pub const PASSWORD: &str = "hosting.password";
pub const API_KEY: &str = "hosting.apiKey";

/// The secrets an upload needs, read from the credential store.
#[derive(Default)]
pub struct Secrets {
    /// SFTP password, or the passphrase of the SFTP key file.
    pub password: Option<String>,
    pub api_key: Option<String>,
}

impl Hosting {
    /// The port used: the given one or the usual one.
    pub fn port(&self) -> u16 {
        if self.port == 0 { 22 } else { self.port }
    }

    /// What's missing before it can upload, in plain words; None when ready.
    pub fn missing(&self, secrets: &Secrets) -> Option<&'static str> {
        match self.kind {
            Kind::None => Some("Choose how the bot gets to its server."),
            Kind::Ftp => Some(FTP_GONE),
            Kind::Sftp if self.host.trim().is_empty() => Some("Fill in the server's address."),
            Kind::Sftp if self.user.trim().is_empty() => Some("Fill in the user name."),
            Kind::Sftp if self.key_file.trim().is_empty() && secrets.password.is_none() => Some("Fill in the password, or choose a key file."),
            Kind::Calagopus if self.panel.trim().is_empty() => Some("Fill in the panel's address."),
            Kind::Calagopus if secrets.api_key.is_none() => Some("Sign in to the panel, or paste an API key."),
            Kind::Calagopus if self.server.is_empty() => Some("Choose the server."),
            _ => None,
        }
    }

    /// Where the files go, as one line: a change of it uploads everything again.
    fn place(&self) -> String {
        match self.kind {
            Kind::Calagopus => format!("calagopus {} {} {}", self.panel.trim_end_matches('/'), self.server, self.folder),
            kind => format!("{kind:?} {}@{}:{} {}", self.user, self.host, self.port(), self.folder),
        }
    }

    /// The folder's names, one per level ("bots/my-bot" → ["bots", "my-bot"]),
    /// and whether it starts at the top ("/").
    fn folder_parts(&self) -> (bool, Vec<String>) {
        let f = self.folder.trim().replace('\\', "/");
        let parts = f.split('/').map(str::trim).filter(|p| !p.is_empty() && *p != ".").map(str::to_string).collect();
        (f.starts_with('/'), parts)
    }
}

/// What was uploaded last time to which place: name → SHA-256.
#[derive(Default, Serialize, Deserialize)]
struct Record {
    place: String,
    files: BTreeMap<String, String>,
}

/// What an upload did.
#[derive(Debug, Default, PartialEq)]
pub struct Uploaded {
    /// The files sent.
    pub sent: Vec<String>,
    /// How many were the same as last time.
    pub same: usize,
}

/// A place files go to, opened and logged in.
trait Remote {
    /// Writes `name` (in the bot's folder) with `data`, replacing it.
    fn put(&mut self, name: &str, data: &[u8]) -> Result<()>;
    /// After the files: logs out and closes the connection (Calagopus:
    /// restarts the server first, when asked to).
    fn finish(&mut self, log: &mut dyn FnMut(String)) -> Result<()>;
}

/// The files of an exported bot that are uploaded, in this order: `index.js`
/// last, so a bot started meanwhile finds what it needs. `.env` only with
/// `include_token`.
pub fn files_to_send(include_token: bool) -> Vec<&'static str> {
    let mut list = vec!["package.json", ".gitignore", "README.txt"];
    if include_token {
        list.push(".env");
    }
    list.push("index.js");
    list
}

/// Uploads the exported bot in `dir`: the files of [`files_to_send`] that
/// changed since the last upload to this place (as `record` says, which is
/// updated). `known_hosts` keeps the SFTP servers' keys (see `sftp`).
pub fn upload(dir: &Path, hosting: &Hosting, secrets: &Secrets, record: &Path, known_hosts: &Path, log: &mut dyn FnMut(String)) -> Result<Uploaded> {
    if let Some(what) = hosting.missing(secrets) {
        bail!("{what}");
    }
    send(dir, hosting, record, &mut |log| open(hosting, secrets, known_hosts, log), log)
}

/// Opens the place files go to, logging what it does.
type Connect<'a> = dyn FnMut(&mut dyn FnMut(String)) -> Result<Box<dyn Remote>> + 'a;

/// [`upload`]'s work, with `connect` opening the place (only when something changed).
fn send(dir: &Path, hosting: &Hosting, record: &Path, connect: &mut Connect, log: &mut dyn FnMut(String)) -> Result<Uploaded> {
    let mut last: Record = fs::read_to_string(record).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    let place = hosting.place();
    if last.place != place {
        last = Record { place, files: BTreeMap::new() };
    }
    let mut changed = Vec::new();
    let mut same = 0;
    for name in files_to_send(hosting.include_token) {
        let data = fs::read(dir.join(name)).with_context(|| format!("reading {name}"))?;
        let hash = hex(&Sha256::digest(&data));
        if last.files.get(name) == Some(&hash) {
            same += 1;
        } else {
            changed.push((name, data, hash));
        }
    }
    if changed.is_empty() {
        return Ok(Uploaded { sent: Vec::new(), same });
    }
    let mut remote = connect(log)?;
    let mut sent = Vec::new();
    for (name, data, hash) in changed {
        log(format!("Uploading {name} ({})", size(data.len())));
        remote.put(name, &data).with_context(|| format!("uploading {name}"))?;
        last.files.insert(name.to_string(), hash);
        sent.push(name.to_string());
        // Kept after each file, so a failed upload sends only the rest next time.
        save_record(record, &last)?;
    }
    remote.finish(log)?;
    Ok(Uploaded { sent, same })
}

/// Forgets what was uploaded, so the next upload sends every file.
pub fn forget(record: &Path) {
    let _ = fs::remove_file(record);
}

fn save_record(path: &Path, record: &Record) -> Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(path, serde_json::to_string_pretty(record)?)?;
    Ok(())
}

fn open(hosting: &Hosting, secrets: &Secrets, known_hosts: &Path, log: &mut dyn FnMut(String)) -> Result<Box<dyn Remote>> {
    Ok(match hosting.kind {
        Kind::Sftp => Box::new(sftp::Sftp::open(hosting, secrets.password.as_deref(), known_hosts, log)?),
        Kind::Ftp => bail!(FTP_GONE),
        Kind::Calagopus => Box::new(calagopus::Upload::open(hosting, secrets.api_key.as_deref().unwrap_or_default(), log)?),
        Kind::None => bail!("Choose how the bot gets to its server."),
    })
}

/// Logs in and out again: whether the settings work. Says what it found.
pub fn test(hosting: &Hosting, secrets: &Secrets, known_hosts: &Path, log: &mut dyn FnMut(String)) -> Result<()> {
    if let Some(what) = hosting.missing(secrets) {
        bail!("{what}");
    }
    let mut remote = open(hosting, secrets, known_hosts, log)?;
    // Logging in was the test: nothing is restarted.
    if hosting.kind == Kind::Calagopus {
        return Ok(());
    }
    remote.finish(log)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// `bytes` random bytes from the system's own generator, as hex: for secrets
/// that must not be guessed (a sign-in's `state`, the token AI clients'
/// connections show).
pub fn random_hex(bytes: usize) -> Result<String> {
    let mut buf = vec![0u8; bytes];
    rustls::crypto::ring::default_provider().secure_random.fill(&mut buf).map_err(|_| anyhow::anyhow!("no random numbers"))?;
    Ok(hex(&buf))
}

fn size(bytes: usize) -> String {
    match bytes {
        0..1024 => format!("{bytes} B"),
        1024..1_048_576 => format!("{:.0} KB", bytes as f64 / 1024.0),
        _ => format!("{:.1} MB", bytes as f64 / 1_048_576.0),
    }
}

/// TLS settings for the Calagopus console: the usual web roots, ring.
pub(crate) fn tls_config() -> std::sync::Arc<rustls::ClientConfig> {
    let roots = rustls::RootCertStore { roots: webpki_roots::TLS_SERVER_ROOTS.to_vec() };
    let provider = std::sync::Arc::new(rustls::crypto::ring::default_provider());
    let config = rustls::ClientConfig::builder_with_provider(provider).with_safe_default_protocol_versions().expect("ring supports the default TLS versions").with_root_certificates(roots).with_no_client_auth();
    std::sync::Arc::new(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A place that keeps what it was sent.
    struct Fake(std::rc::Rc<std::cell::RefCell<Vec<String>>>);
    impl Remote for Fake {
        fn put(&mut self, name: &str, _: &[u8]) -> Result<()> {
            self.0.borrow_mut().push(name.to_string());
            Ok(())
        }
        fn finish(&mut self, _: &mut dyn FnMut(String)) -> Result<()> {
            Ok(())
        }
    }

    #[test]
    fn what_is_missing_is_said() {
        let mut h = Hosting { kind: Kind::Sftp, host: "example.com".into(), user: "bot".into(), ..Default::default() };
        assert_eq!(h.missing(&Secrets::default()), Some("Fill in the password, or choose a key file."));
        h.key_file = "C:/keys/id_ed25519".into();
        assert_eq!(h.missing(&Secrets::default()), None, "a key file is enough");
        assert_eq!(h.port(), 22);
        // A project still set to FTP is told that it's gone, and nothing is sent.
        h.kind = Kind::Ftp;
        assert_eq!(h.missing(&Secrets { password: Some("p".into()), ..Default::default() }), Some(FTP_GONE));
        assert!(serde_json::from_value::<Hosting>(serde_json::json!({ "kind": "ftp", "host": "a", "ftps": false })).is_ok(), "an older project's settings are still read");
        let c = Hosting { kind: Kind::Calagopus, panel: "https://panel.example.com".into(), ..Default::default() };
        assert_eq!(c.missing(&Secrets { api_key: Some("c7sp_x".into()), ..Default::default() }), Some("Choose the server."));
    }

    #[test]
    fn folders_are_split_into_levels() {
        let h = Hosting { folder: " /home/bot\\\\my-bot/ ".into(), ..Default::default() };
        assert_eq!(h.folder_parts(), (true, vec!["home".to_string(), "bot".into(), "my-bot".into()]));
        let h = Hosting { folder: "bots/./x".into(), ..Default::default() };
        assert_eq!(h.folder_parts(), (false, vec!["bots".to_string(), "x".into()]));
    }

    /// Only what changed is sent again; another place gets everything.
    #[test]
    fn only_changed_files_are_sent() {
        let dir = std::env::temp_dir().join(format!("xc-upload-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        for name in files_to_send(true) {
            fs::write(dir.join(name), name).unwrap();
        }
        let record = dir.join("record.json");
        let sent = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let h = Hosting { kind: Kind::Sftp, host: "a".into(), user: "u".into(), include_token: false, ..Default::default() };
        let run = |h: &Hosting| {
            sent.borrow_mut().clear();
            let shared = sent.clone();
            send(&dir, h, &record, &mut |_| Ok(Box::new(Fake(shared.clone()))), &mut |_| {}).unwrap();
            sent.borrow().clone()
        };
        assert_eq!(run(&h), ["package.json", ".gitignore", "README.txt", "index.js"], "no .env without the token, index.js last");
        assert!(run(&h).is_empty(), "nothing changed");
        fs::write(dir.join("index.js"), "new").unwrap();
        assert_eq!(run(&h), ["index.js"]);
        let other = Hosting { folder: "elsewhere".into(), include_token: true, ..h.clone() };
        assert_eq!(run(&other).len(), 5, "another folder gets every file, .env too");
        let _ = fs::remove_dir_all(&dir);
    }

    /// A real upload, to a test SFTP server: XC_TEST_SFTP=host:port, user
    /// "bot", password "test-password", its folder in XC_TEST_SFTP_HOME.
    #[test]
    #[ignore]
    fn real_sftp_upload() {
        let (Ok(server), Ok(home)) = (std::env::var("XC_TEST_SFTP"), std::env::var("XC_TEST_SFTP_HOME")) else { return };
        let (host, port) = server.split_once(':').unwrap();
        let dir = std::env::temp_dir().join(format!("xc-sftp-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        for name in files_to_send(true) {
            fs::write(dir.join(name), format!("{name} v1")).unwrap();
        }
        let h = Hosting { kind: Kind::Sftp, host: host.into(), port: port.parse().unwrap(), user: "bot".into(), folder: "bots/my-bot".into(), ..Default::default() };
        let secrets = Secrets { password: Some("test-password".into()), ..Default::default() };
        let (record, known) = (dir.join("record.json"), dir.join("known.json"));
        let mut lines = Vec::new();
        let up = upload(&dir, &h, &secrets, &record, &known, &mut |l| lines.push(l)).unwrap();
        assert_eq!(up.sent.len(), 5, "{lines:?}");
        assert!(lines.iter().any(|l| l.contains("its key SHA256:")), "the key is trusted the first time: {lines:?}");
        let there = Path::new(&home).join("bots/my-bot");
        assert_eq!(fs::read_to_string(there.join("index.js")).unwrap(), "index.js v1");
        fs::write(dir.join("index.js"), "v2").unwrap();
        lines.clear();
        let up = upload(&dir, &h, &secrets, &record, &known, &mut |l| lines.push(l)).unwrap();
        assert_eq!(up.sent, ["index.js"]);
        assert!(!lines.iter().any(|l| l.contains("First connection")), "the same key is known");
        assert_eq!(fs::read_to_string(there.join("index.js")).unwrap(), "v2", "replaced, not appended");
        assert!(Path::new(&home).join("data/keep.json").is_file());
        // Another key for the same server is refused, until forgotten.
        let mut kept: BTreeMap<String, String> = serde_json::from_str(&fs::read_to_string(&known).unwrap()).unwrap();
        kept.insert(server.clone(), "SHA256:somethingelse".into());
        fs::write(&known, serde_json::to_string(&kept).unwrap()).unwrap();
        let err = test(&h, &secrets, &known, &mut |_| {}).unwrap_err().to_string();
        assert!(err.contains("shows another key than before"), "{err}");
        forget_key(&known, host, h.port()).unwrap();
        test(&h, &secrets, &known, &mut |_| {}).unwrap();
        let wrong = Secrets { password: Some("nope".into()), ..Default::default() };
        let err = test(&h, &wrong, &known, &mut |_| {}).unwrap_err().to_string();
        assert!(err.contains("didn't accept the user name and password"), "{err}");
        let _ = fs::remove_dir_all(&dir);
    }
}
