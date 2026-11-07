//! Block updates without an app update. The bundled block plugins (Circuit
//! Core, Discord Bot, Fluxer Bot) are published on the repository's "plugins"
//! release with an index (see tools/plugin_updates.py). When a plugin there
//! has a higher version than the app's copy, and the app is new enough for
//! it, the app offers it; an update goes into `plugin-updates` in the app's
//! data folder, and the higher version of a plugin is the one that loads.
//! Undo Block Updates deletes that folder, which brings back the bundled
//! versions. Checked with the app's own update check (at start, unless turned
//! off, and from Help). From Help, the Updates window shows what was found
//! for each plugin, and can download the published blocks again (Download
//! Blocks Again) even when they're no newer. Any trouble only means nothing
//! is offered; the Updates window says what it was.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

use circuit_plugins::manifest::{app_supported, compare_versions};
use serde::Deserialize;
use sha2::{Digest, Sha256};

const RELEASE: &str = "https://github.com/XCraftTM/XC-Circuit/releases/download/plugins/";
/// Larger downloads are refused.
const MAX_SIZE: u64 = 50 * 1024 * 1024;

/// A plugin version in the published index.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Entry {
    pub id: String,
    pub name: String,
    pub version: String,
    /// The oldest app version it works with.
    #[serde(default)]
    pub app: Option<String>,
    pub file: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Deserialize)]
struct Index {
    plugins: Vec<Entry>,
}

/// An update worth offering: the published version, and the one in use.
#[derive(Clone, Debug, PartialEq)]
pub struct Offer {
    pub entry: Entry,
    pub current: String,
}

/// Where updates go: next to the user's own plugins, in the app's data folder.
pub fn updates_dir() -> PathBuf {
    crate::bot::app_data_dir().join("plugin-updates")
}

/// Whether any update is installed (so Undo Block Updates has something to do).
pub fn any_installed() -> bool {
    any_in(&updates_dir())
}

fn any_in(dir: &std::path::Path) -> bool {
    std::fs::read_dir(dir).is_ok_and(|mut d| d.any(|e| e.is_ok_and(|e| e.path().join("plugin.toml").is_file())))
}

/// The published versions worth offering: plugins this app has (by id and
/// version), newer than its copy, that this app is new enough for.
pub fn offers(index: &[Entry], loaded: &[(String, String)]) -> Vec<Offer> {
    index
        .iter()
        .filter(|e| app_supported(e.app.as_deref()))
        .filter_map(|e| {
            let (_, current) = loaded.iter().find(|(id, _)| *id == e.id)?;
            (compare_versions(&e.version, current) == std::cmp::Ordering::Greater).then(|| Offer { entry: e.clone(), current: current.clone() })
        })
        .collect()
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(30))).build().into()
}

fn get(url: &str) -> anyhow::Result<Vec<u8>> {
    let mut response = agent().get(url).header("User-Agent", concat!("XC-Circuit/", env!("CARGO_PKG_VERSION"))).call()?;
    Ok(response.body_mut().with_config().limit(MAX_SIZE).read_to_vec()?)
}

/// The published index. Asked for with the time in the address, so a copy
/// GitHub still has from before the last publish isn't what comes back.
fn fetch_index() -> anyhow::Result<Vec<Entry>> {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let body = get(&format!("{RELEASE}index.json?t={now}"))?;
    Ok(serde_json::from_slice::<Index>(&body)?.plugins)
}

/// How a published plugin compares with the loaded one, for the Updates window.
#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    /// The same version.
    UpToDate,
    /// Newer, and this app can use it.
    Newer,
    /// Newer, but it needs this app version or a later one.
    NeedsApp(String),
    /// Older than the loaded one (which came with this app, or a test build).
    Older,
}

/// One row of the Updates window: a published plugin this app has.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub entry: Entry,
    pub current: String,
    pub status: Status,
}

/// The published plugins this app has, each compared with its loaded version.
pub fn rows(index: &[Entry], loaded: &[(String, String)]) -> Vec<Row> {
    index
        .iter()
        .filter_map(|e| {
            let (_, current) = loaded.iter().find(|(id, _)| *id == e.id)?;
            let status = match compare_versions(&e.version, current) {
                std::cmp::Ordering::Equal => Status::UpToDate,
                std::cmp::Ordering::Less => Status::Older,
                std::cmp::Ordering::Greater if app_supported(e.app.as_deref()) => Status::Newer,
                std::cmp::Ordering::Greater => Status::NeedsApp(e.app.clone().unwrap_or_default()),
            };
            Some(Row { entry: e.clone(), current: current.clone(), status })
        })
        .collect()
}

/// What Download Blocks Again downloads: the published plugins this app has
/// and can use, unless the loaded version is newer (it would win anyway).
pub fn again(index: &[Entry], loaded: &[(String, String)]) -> Vec<Entry> {
    rows(index, loaded).into_iter().filter(|r| matches!(r.status, Status::UpToDate | Status::Newer)).map(|r| r.entry).collect()
}

/// Downloads one update, checks it against the index and unpacks it into
/// `updates_dir`, replacing an older update of the same plugin.
pub fn install(entry: &Entry) -> anyhow::Result<()> {
    anyhow::ensure!(!entry.file.contains(['/', '\\']) && entry.file.ends_with(".zip"), "the index names an odd file: {}", entry.file);
    let bytes = get(&format!("{RELEASE}{}", entry.file))?;
    install_bytes(entry, &bytes, &updates_dir())
}

fn install_bytes(entry: &Entry, bytes: &[u8], dir: &std::path::Path) -> anyhow::Result<()> {
    anyhow::ensure!(bytes.len() as u64 == entry.size, "the download of {} is incomplete", entry.name);
    let sum: String = Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect();
    anyhow::ensure!(sum.eq_ignore_ascii_case(&entry.sha256), "the download of {} doesn't match its checksum", entry.name);
    std::fs::create_dir_all(dir)?;
    let zip = dir.join(format!(".download-{}", entry.file));
    std::fs::write(&zip, bytes)?;
    let result = crate::plugins::install::install_zip(&zip, dir, true);
    let _ = std::fs::remove_file(&zip);
    let installed = result.map_err(|e| anyhow::anyhow!("{e}"))?;
    anyhow::ensure!(installed.dir.file_name().is_some_and(|n| n == entry.id.as_str()), "the zip holds another plugin than {}", entry.id);
    Ok(())
}

/// Deletes every installed update: the bundled versions load again.
pub fn undo() -> std::io::Result<()> {
    match std::fs::remove_dir_all(updates_dir()) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

/// What a finished check found.
pub enum Found {
    /// Updates to offer (in `offers`).
    Some,
    /// Checked by hand and nothing newer (or the index couldn't be read).
    None,
}

/// The check and the installing, run in the background.
#[derive(Default)]
pub struct BlockUpdates {
    checking: Option<Receiver<Result<Vec<Entry>, String>>>,
    /// What the last check found: the published plugins, or why it couldn't tell.
    pub last: Option<Result<Vec<Entry>, String>>,
    /// How the last installing went: the plugins updated, or what went wrong.
    pub done: Option<Result<Vec<String>, String>>,
    by_hand: bool,
    installing: Option<Receiver<Result<Vec<String>, String>>>,
    /// Updates found, until installed or put off.
    pub offers: Vec<Offer>,
    /// Their window is open.
    pub show: bool,
}

impl BlockUpdates {
    /// Starts a check, unless one is running.
    pub fn check(&mut self, by_hand: bool) {
        if self.checking.is_some() {
            return;
        }
        self.by_hand = by_hand;
        let (tx, rx) = channel();
        self.checking = Some(rx);
        std::thread::spawn(move || {
            let _ = tx.send(fetch_index().map_err(|e| format!("{e:#}")));
        });
    }

    /// A check is running.
    pub fn checking(&self) -> bool {
        self.checking.is_some()
    }

    /// A finished check, once: what's offered for the plugins loaded now.
    pub fn poll(&mut self, loaded: &[(String, String)]) -> Option<Found> {
        let index = self.checking.as_ref()?.try_recv().ok()?;
        self.checking = None;
        self.offers = offers(index.as_deref().unwrap_or_default(), loaded);
        self.last = Some(index);
        if self.offers.is_empty() {
            return self.by_hand.then_some(Found::None);
        }
        // From Help, the Updates window shows them; at start, their own window.
        self.show = !self.by_hand;
        Some(Found::Some)
    }

    /// Installs every offered update in the background.
    pub fn install_all(&mut self) {
        let entries: Vec<Entry> = self.offers.iter().map(|o| o.entry.clone()).collect();
        self.install_entries(entries);
    }

    /// Downloads and installs these published plugins in the background,
    /// whatever their versions (Download Blocks Again).
    pub fn install_entries(&mut self, entries: Vec<Entry>) {
        if self.installing.is_some() || entries.is_empty() {
            return;
        }
        self.done = None;
        let (tx, rx) = channel();
        self.installing = Some(rx);
        std::thread::spawn(move || {
            let mut done = Vec::new();
            for e in &entries {
                if let Err(err) = install(e) {
                    let _ = tx.send(Err(format!("{}: {err:#}", e.name)));
                    return;
                }
                done.push(format!("{} {}", e.name, e.version));
            }
            let _ = tx.send(Ok(done));
        });
    }

    /// Finished installing: the plugins updated (to reload them), or what went wrong.
    pub fn poll_install(&mut self) -> Option<Result<Vec<String>, String>> {
        let result = self.installing.as_ref()?.try_recv().ok()?;
        self.installing = None;
        self.offers.clear();
        self.show = false;
        self.done = Some(result.clone());
        Some(result)
    }

    pub fn busy(&self) -> bool {
        self.checking.is_some() || self.installing.is_some()
    }

    pub fn installing(&self) -> bool {
        self.installing.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, version: &str, app: Option<&str>) -> Entry {
        Entry { id: id.into(), name: id.into(), version: version.into(), app: app.map(str::to_string), file: format!("{id}-{version}.zip"), size: 0, sha256: String::new() }
    }

    /// The Updates window compares each published plugin this app has with
    /// its loaded version; Download Blocks Again takes the ones this app can
    /// use that aren't older than what it has.
    #[test]
    fn rows_and_downloading_again() {
        let loaded = vec![
            ("xc.discord".to_string(), "1.2.4".to_string()),
            ("xc.core".to_string(), "1.0.35".to_string()),
            ("xc.fluxer".to_string(), "1.3.0".to_string()),
            ("xc.server".to_string(), "1.0.0".to_string()),
        ];
        let index = vec![entry("xc.discord", "1.2.5", Some("0.0.1")), entry("xc.core", "1.0.35", None), entry("xc.fluxer", "1.2.4", None), entry("xc.server", "2.0.0", Some("99.0.0")), entry("xc.agents", "1.0.0", None)];
        let status: Vec<(String, Status)> = rows(&index, &loaded).into_iter().map(|r| (r.entry.id, r.status)).collect();
        assert_eq!(
            status,
            [
                ("xc.discord".to_string(), Status::Newer),
                ("xc.core".to_string(), Status::UpToDate),
                ("xc.fluxer".to_string(), Status::Older),
                ("xc.server".to_string(), Status::NeedsApp("99.0.0".into())),
            ]
        );
        let ids: Vec<String> = again(&index, &loaded).into_iter().map(|e| e.id).collect();
        assert_eq!(ids, ["xc.discord", "xc.core"]);
    }

    /// Only newer versions of plugins this app has, and only ones this app is new enough for.
    #[test]
    fn offers_newer_versions_this_app_can_use() {
        let loaded = vec![("xc.discord".to_string(), "1.0.0".to_string()), ("xc.core".to_string(), "1.2.0".to_string())];
        let index = vec![entry("xc.discord", "1.1.0", None), entry("xc.core", "1.2.0", None), entry("xc.fluxer", "2.0.0", None), entry("xc.discord", "9.0.0", Some("99.0.0"))];
        let found = offers(&index, &loaded);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!((found[0].entry.version.as_str(), found[0].current.as_str()), ("1.1.0", "1.0.0"));
    }

    /// A download is checked before it's unpacked, and lands in the updates folder.
    #[test]
    fn checks_and_unpacks_a_download() {
        use std::io::Write;
        let dir = std::env::temp_dir().join(format!("xc-block-updates-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        zip.start_file("acme.tools/plugin.toml", zip::write::SimpleFileOptions::default()).unwrap();
        zip.write_all(b"id = \"acme.tools\"\nname = \"Tools\"\nversion = \"1.1.0\"\n").unwrap();
        let bytes = zip.finish().unwrap().into_inner();
        let sum: String = Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect();
        let mut e = entry("acme.tools", "1.1.0", None);
        e.size = bytes.len() as u64;
        e.sha256 = "0".repeat(64);
        assert!(install_bytes(&e, &bytes, &dir).unwrap_err().to_string().contains("checksum"));
        assert!(!any_in(&dir), "nothing unpacked from a bad download");
        e.sha256 = sum;
        install_bytes(&e, &bytes, &dir).unwrap();
        assert!(dir.join("acme.tools").join("plugin.toml").is_file());
        assert!(any_in(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Against the real "plugins" release: an app with older blocks is offered
    /// every published plugin, and each one downloads, checks out and unpacks.
    /// Needs the internet: `cargo test -p circuit-editor live_ -- --ignored`.
    #[test]
    #[ignore]
    fn live_block_updates_download_and_install() {
        let index = fetch_index().expect("the published index");
        assert!(!index.is_empty());
        let loaded: Vec<(String, String)> = index.iter().map(|e| (e.id.clone(), "0.0.1".to_string())).collect();
        let found = offers(&index, &loaded);
        assert_eq!(found.len(), index.len(), "{found:?}");
        let dir = std::env::temp_dir().join(format!("xc-block-updates-live-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for o in &found {
            let bytes = get(&format!("{RELEASE}{}", o.entry.file)).unwrap();
            install_bytes(&o.entry, &bytes, &dir).unwrap();
            let toml = std::fs::read_to_string(dir.join(&o.entry.id).join("plugin.toml")).unwrap();
            assert!(toml.contains(&format!("\"{}\"", o.entry.version)), "{} has its version", o.entry.id);
            println!("{} {}: {} bytes, checksum ok", o.entry.id, o.entry.version, bytes.len());
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
