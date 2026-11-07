//! Looks for a newer release on GitHub: at start (unless turned off in
//! Settings) and from Help › Check for Updates. Anything going wrong (no
//! internet, the repository still private and answering 404, GitHub's limits)
//! is ignored: the app just doesn't offer an update then.
//!
//! Where it can, the app updates itself: installed with the Windows Setup, it
//! downloads the new Setup and runs it silently when XC Circuit closes, which
//! opens XC Circuit again; as an AppImage, it swaps in the new AppImage and
//! starts it. Every download is checked against the size and SHA-256 that
//! GitHub gives for it. Other kinds (.deb, .rpm, .zip, .tar.gz) open the
//! release page, as the package manager or the user installs those.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

use sha2::{Digest, Sha256};

const LATEST: &str = "https://api.github.com/repos/XCraftTM/XC-Circuit/releases/latest";
/// Larger downloads are refused.
const MAX_SIZE: u64 = 300 * 1024 * 1024;

/// A file of a release.
#[derive(Clone, Debug, PartialEq)]
pub struct Asset {
    pub name: String,
    pub url: String,
    pub size: u64,
    /// Its SHA-256 in hex, when GitHub gives one.
    pub sha256: Option<String>,
}

/// A release newer than this app.
#[derive(Clone, Debug, PartialEq)]
pub struct Release {
    /// Its version without the v: 1.2.0.
    pub version: String,
    /// Its page, with the downloads.
    pub url: String,
    /// What changed, as written on the release.
    pub notes: String,
    pub assets: Vec<Asset>,
}

/// How this copy of XC Circuit was installed, as far as updating goes.
#[derive(Clone, Debug, PartialEq)]
pub enum Install {
    /// With the Windows Setup; `all_users` when it went into Program Files.
    WindowsSetup { all_users: bool },
    /// An AppImage, at this path.
    AppImage(PathBuf),
    /// Anything else: the release page.
    Other,
}

impl Install {
    /// This copy's kind.
    pub fn current() -> Install {
        let exe = std::env::current_exe().ok();
        if cfg!(windows)
            && let Some(dir) = exe.as_deref().and_then(Path::parent)
            && dir.join("unins000.exe").is_file()
        {
            let program_files = ["ProgramW6432", "ProgramFiles"].iter().filter_map(std::env::var_os).map(PathBuf::from);
            let lower = |p: &Path| p.to_string_lossy().to_lowercase();
            let all_users = program_files.into_iter().any(|pf| lower(dir).starts_with(&lower(&pf)));
            return Install::WindowsSetup { all_users };
        }
        if cfg!(target_os = "linux")
            && let Some(path) = std::env::var_os("APPIMAGE").map(PathBuf::from)
            && path.is_file()
        {
            return Install::AppImage(path);
        }
        Install::Other
    }

    /// The end of the release file's name that updates this kind, on this machine.
    fn suffix(&self) -> Option<&'static str> {
        let arm = cfg!(target_arch = "aarch64");
        match self {
            Install::WindowsSetup { .. } => Some(if arm { "-windows-arm64-setup.exe" } else { "-windows-x64-setup.exe" }),
            Install::AppImage(_) => Some(if arm { "-linux-aarch64.AppImage" } else { "-linux-x86_64.AppImage" }),
            Install::Other => None,
        }
    }

    /// The release's file that updates this copy, if it can update itself.
    pub fn asset<'a>(&self, release: &'a Release) -> Option<&'a Asset> {
        let suffix = self.suffix()?;
        release.assets.iter().find(|a| a.name.ends_with(suffix))
    }

    /// Where a download goes: the AppImage's own folder (so it can replace it),
    /// or else a folder for updates in the temporary folder.
    fn download_dir(&self) -> PathBuf {
        match self {
            Install::AppImage(path) => path.parent().map(Path::to_path_buf).unwrap_or_else(std::env::temp_dir),
            _ => std::env::temp_dir().join("xc-circuit-update"),
        }
    }

    /// Installs a downloaded update and starts the new version. Called as the
    /// app closes.
    pub fn install(&self, file: &Path) -> std::io::Result<()> {
        match self {
            Install::WindowsSetup { all_users } => {
                // Silent: only a progress window. /restart=1 opens XC Circuit
                // when it's done (see packaging/windows/xc-circuit.iss).
                // Its log goes next to it, to see why an update failed.
                let log = file.with_file_name("setup.log");
                std::process::Command::new(file).args(setup_args(*all_users, &log)).spawn().map(|_| ())
            }
            Install::AppImage(path) => {
                std::fs::rename(file, path)?;
                std::process::Command::new(path).spawn().map(|_| ())
            }
            Install::Other => Ok(()),
        }
    }
}

/// The Windows Setup's arguments for an update from inside the app.
fn setup_args(all_users: bool, log: &Path) -> Vec<String> {
    let scope = if all_users { "/ALLUSERS" } else { "/CURRENTUSER" };
    let mut args: Vec<String> = ["/SILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/CLOSEAPPLICATIONS", "/restart=1", scope].map(String::from).into();
    args.push(format!("/LOG={}", log.display()));
    args
}

#[derive(Default)]
pub struct Updater {
    pending: Option<Receiver<Result<Option<Release>, String>>>,
    /// How the last check went: fine, or why GitHub couldn't be asked.
    pub last: Option<Result<(), String>>,
    /// Asked for from the Help menu: then "you have the latest" is said too.
    by_hand: bool,
    /// A newer release, once found.
    pub available: Option<Release>,
    /// Its window is open.
    pub show: bool,
    downloading: Option<Receiver<Result<PathBuf, String>>>,
    /// Bytes downloaded so far, of `size`.
    received: Arc<AtomicU64>,
    size: u64,
    /// A checked download, installed when the app closes.
    pub ready: Option<PathBuf>,
}

/// What a finished check tells the app.
pub enum Found {
    /// A newer release (`available` holds it).
    Newer,
    /// Checked by hand, and this is the latest version (or it couldn't tell).
    UpToDate,
}

impl Updater {
    /// Starts a check in the background, unless one is running.
    pub fn check(&mut self, by_hand: bool) {
        if self.pending.is_some() {
            return;
        }
        self.by_hand = by_hand;
        let (tx, rx) = channel();
        self.pending = Some(rx);
        std::thread::spawn(move || {
            let _ = tx.send(latest().map(|found| found.filter(|r| newer(&r.version, env!("CARGO_PKG_VERSION")))));
        });
    }

    /// A finished check, once. `skipped` is a version the user chose to skip
    /// (only a check by hand still shows it).
    pub fn poll(&mut self, skipped: Option<&str>) -> Option<Found> {
        let found = self.pending.as_ref()?.try_recv().ok()?;
        self.pending = None;
        self.last = Some(found.as_ref().map(|_| ()).map_err(Clone::clone));
        match found.unwrap_or_default() {
            Some(r) if self.by_hand || skipped != Some(r.version.as_str()) => {
                self.show = self.by_hand;
                self.available = Some(r);
                Some(Found::Newer)
            }
            _ if self.by_hand => Some(Found::UpToDate),
            _ => None,
        }
    }

    pub fn busy(&self) -> bool {
        self.pending.is_some() || self.downloading.is_some()
    }

    /// A check is running.
    pub fn checking(&self) -> bool {
        self.pending.is_some()
    }

    /// Starts downloading `asset` for `install` in the background.
    pub fn download(&mut self, install: &Install, asset: &Asset) {
        if self.downloading.is_some() {
            return;
        }
        let (tx, rx) = channel();
        self.downloading = Some(rx);
        self.received = Arc::new(AtomicU64::new(0));
        self.size = asset.size;
        let (asset, dir, received) = (asset.clone(), install.download_dir(), self.received.clone());
        std::thread::spawn(move || {
            let _ = tx.send(fetch(&asset, &dir, &received).map_err(|e| format!("{e:#}")));
        });
    }

    /// How far a download is, from 0 to 1, while one runs.
    pub fn progress(&self) -> Option<f32> {
        self.downloading.as_ref().map(|_| (self.received.load(Ordering::Relaxed) as f32 / self.size.max(1) as f32).min(1.0))
    }

    /// A finished download, once: the checked file, or what went wrong.
    pub fn poll_download(&mut self) -> Option<Result<PathBuf, String>> {
        let result = self.downloading.as_ref()?.try_recv().ok()?;
        self.downloading = None;
        if let Ok(path) = &result {
            self.ready = Some(path.clone());
        }
        Some(result)
    }
}

fn agent(timeout: u64) -> ureq::Agent {
    ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(timeout))).build().into()
}

/// The latest release on GitHub, or None for any trouble.
/// The latest release, if it's one to offer; or why GitHub couldn't be asked.
fn latest() -> Result<Option<Release>, String> {
    let mut response = agent(10)
        .get(LATEST)
        .header("User-Agent", concat!("XC-Circuit/", env!("CARGO_PKG_VERSION")))
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| e.to_string())?;
    let bytes = response.body_mut().read_to_vec().map_err(|e| e.to_string())?;
    let body: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    Ok(parse(&body))
}

fn parse(body: &serde_json::Value) -> Option<Release> {
    let text = |k: &str| body.get(k).and_then(|v| v.as_str()).map(str::to_string);
    if body.get("draft").and_then(|v| v.as_bool()).unwrap_or(false) || body.get("prerelease").and_then(|v| v.as_bool()).unwrap_or(false) {
        return None;
    }
    let version = text("tag_name")?.trim_start_matches(['v', 'V']).to_string();
    let assets = body
        .get("assets")
        .and_then(|a| a.as_array())
        .map(|list| {
            list.iter()
                .filter_map(|a| {
                    Some(Asset {
                        name: a.get("name")?.as_str()?.to_string(),
                        url: a.get("browser_download_url")?.as_str()?.to_string(),
                        size: a.get("size")?.as_u64()?,
                        sha256: a.get("digest").and_then(|d| d.as_str()).and_then(|d| d.strip_prefix("sha256:")).map(str::to_string),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    Some(Release { version, url: text("html_url")?, notes: text("body").unwrap_or_default(), assets })
}

/// Downloads `asset` into `dir`, counting bytes in `received`, and checks it.
fn fetch(asset: &Asset, dir: &Path, received: &AtomicU64) -> anyhow::Result<PathBuf> {
    use std::io::{Read, Write};
    anyhow::ensure!(!asset.name.contains(['/', '\\']) && asset.size <= MAX_SIZE, "the release names an odd file: {}", asset.name);
    std::fs::create_dir_all(dir)?;
    let mut response = agent(600).get(&asset.url).header("User-Agent", concat!("XC-Circuit/", env!("CARGO_PKG_VERSION"))).call()?;
    let mut body = response.body_mut().with_config().limit(MAX_SIZE).reader();
    let mut bytes = Vec::with_capacity(asset.size as usize);
    let mut chunk = vec![0; 64 * 1024];
    loop {
        let n = body.read(&mut chunk)?;
        if n == 0 {
            break;
        }
        bytes.extend_from_slice(&chunk[..n]);
        received.store(bytes.len() as u64, Ordering::Relaxed);
    }
    check(asset, &bytes)?;
    // A name of its own beside the target, never the target itself.
    let path = dir.join(format!(".update-{}", asset.name));
    let mut file = std::fs::File::create(&path)?;
    file.write_all(&bytes)?;
    drop(file);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))?;
    }
    Ok(path)
}

/// The download is whole and is the file GitHub describes.
fn check(asset: &Asset, bytes: &[u8]) -> anyhow::Result<()> {
    anyhow::ensure!(bytes.len() as u64 == asset.size, "the download of {} is incomplete", asset.name);
    if let Some(want) = &asset.sha256 {
        let sum: String = Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect();
        anyhow::ensure!(sum.eq_ignore_ascii_case(want), "the download of {} doesn't match its checksum", asset.name);
    }
    Ok(())
}

/// Whether version `a` comes after `b` (1.10.0 after 1.9.2; 1.2 equals 1.2.0).
pub fn newer(a: &str, b: &str) -> bool {
    let parts = |v: &str| -> Vec<u64> { v.split(['.', '-', '+']).take(3).map(|p| p.parse().unwrap_or(0)).collect() };
    let (mut a, mut b) = (parts(a), parts(b));
    a.resize(3, 0);
    b.resize(3, 0);
    a > b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_versions() {
        assert!(newer("1.10.0", "1.9.2"));
        assert!(newer("0.2.0", "0.1.9"));
        assert!(!newer("1.2", "1.2.0"));
        assert!(!newer("0.1.0", "0.1.0"));
        assert!(!newer("0.0.9", "0.1.0"));
        assert!(newer("2.0.0-beta", "1.9.9"));
    }

    #[test]
    fn reads_github_s_answer() {
        let r = parse(&serde_json::json!({ "tag_name": "v1.2.0", "html_url": "https://github.com/x/y/releases/tag/v1.2.0", "body": "Fixes", "draft": false, "prerelease": false, "assets": [
            { "name": "XC-Circuit-1.2.0-windows-x64-setup.exe", "browser_download_url": "https://x/setup.exe", "size": 12, "digest": "sha256:ab12" },
            { "name": "XC-Circuit-1.2.0-linux-x86_64.AppImage", "browser_download_url": "https://x/app", "size": 34 }
        ] }))
        .unwrap();
        assert_eq!((r.version.as_str(), r.notes.as_str()), ("1.2.0", "Fixes"));
        assert_eq!(r.assets[0], Asset { name: "XC-Circuit-1.2.0-windows-x64-setup.exe".into(), url: "https://x/setup.exe".into(), size: 12, sha256: Some("ab12".into()) });
        assert_eq!(r.assets[1].sha256, None, "older uploads have no digest");
        assert!(parse(&serde_json::json!({ "tag_name": "v2.0.0", "html_url": "u", "prerelease": true })).is_none(), "previews aren't offered");
        assert!(parse(&serde_json::json!({ "message": "Not Found" })).is_none(), "a private repository answers 404");
    }

    /// Each kind of install picks its own file; the others open the page.
    #[test]
    fn picks_the_file_for_this_install() {
        let file = |name: &str| Asset { name: name.into(), url: String::new(), size: 1, sha256: None };
        let r = Release {
            version: "1.2.0".into(),
            url: String::new(),
            notes: String::new(),
            assets: ["windows-arm64-setup.exe", "windows-x64-setup.exe", "windows-x64.zip", "linux-aarch64.AppImage", "linux-x86_64.AppImage", "linux-x86_64.deb"].iter().map(|s| file(&format!("XC-Circuit-1.2.0-{s}"))).collect(),
        };
        let arm = cfg!(target_arch = "aarch64");
        let setup = Install::WindowsSetup { all_users: false }.asset(&r).unwrap();
        assert!(setup.name.ends_with(if arm { "windows-arm64-setup.exe" } else { "windows-x64-setup.exe" }), "{}", setup.name);
        let image = Install::AppImage(PathBuf::from("/opt/xc.AppImage")).asset(&r).unwrap();
        assert!(image.name.ends_with(if arm { "aarch64.AppImage" } else { "x86_64.AppImage" }), "{}", image.name);
        assert!(Install::Other.asset(&r).is_none());
        let log = Path::new("C:\\Temp\\xc-circuit-update\\setup.log");
        assert!(setup_args(true, log).contains(&"/ALLUSERS".into()) && setup_args(false, log).contains(&"/CURRENTUSER".into()));
        assert!(setup_args(false, log).contains(&"/restart=1".into()), "the Setup opens XC Circuit again");
        assert!(setup_args(false, log).contains(&"/LOG=C:\\Temp\\xc-circuit-update\\setup.log".into()), "a failed update leaves its log");
    }

    /// A download must have its size and, when GitHub gives one, its checksum.
    #[test]
    fn checks_downloads() {
        let bytes = b"new version";
        let sum: String = Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect();
        let asset = |size, sha256: Option<&str>| Asset { name: "a".into(), url: String::new(), size, sha256: sha256.map(str::to_string) };
        assert!(check(&asset(11, Some(&sum)), bytes).is_ok());
        assert!(check(&asset(11, None), bytes).is_ok());
        assert!(check(&asset(12, Some(&sum)), bytes).unwrap_err().to_string().contains("incomplete"));
        assert!(check(&asset(11, Some(&"0".repeat(64))), bytes).unwrap_err().to_string().contains("checksum"));
    }

    /// Against the real repository: the latest release is found and read.
    /// Needs the internet: `cargo test -p circuit-editor live_ -- --ignored`.
    #[test]
    #[ignore]
    fn live_latest_release() {
        let r = latest().expect("GitHub answers").expect("GitHub's latest release");
        println!("latest: {} ({}), {} characters of notes, {} files", r.version, r.url, r.notes.len(), r.assets.len());
        assert!(r.url.contains(&format!("/v{}", r.version)));
        assert!(!newer(env!("CARGO_PKG_VERSION"), &r.version), "this app isn't newer than the latest release");
        assert!(newer(&r.version, "0.0.9"), "an older app would be offered it");
        let setup = Install::WindowsSetup { all_users: false }.asset(&r).expect("a Windows Setup");
        assert!(setup.sha256.is_some(), "GitHub gives its checksum");
        assert!(Install::AppImage(PathBuf::from("x")).asset(&r).is_some(), "an AppImage");
    }

    /// Downloads the real Windows Setup (about 12 MB) and checks it, without running it.
    #[test]
    #[ignore]
    fn live_download_is_checked() {
        let r = latest().expect("GitHub answers").expect("GitHub's latest release");
        let setup = Install::WindowsSetup { all_users: false }.asset(&r).unwrap().clone();
        let dir = std::env::temp_dir().join(format!("xc-update-live-{}", std::process::id()));
        let received = AtomicU64::new(0);
        let path = fetch(&setup, &dir, &received).unwrap();
        assert_eq!(std::fs::metadata(&path).unwrap().len(), setup.size);
        assert_eq!(received.load(Ordering::Relaxed), setup.size);
        println!("{}: {} bytes, checksum ok", setup.name, setup.size);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn checks_quietly() {
        // A check by hand that finds nothing says so; one at start says nothing.
        let mut u = Updater::default();
        let (tx, rx) = channel();
        u.pending = Some(rx);
        tx.send(Ok(None)).unwrap();
        assert!(u.poll(None).is_none());
        let (tx, rx) = channel();
        u.pending = Some(rx);
        u.by_hand = true;
        tx.send(Ok(None)).unwrap();
        assert!(matches!(u.poll(None), Some(Found::UpToDate)));
        assert_eq!(u.last, Some(Ok(())));
        // GitHub couldn't be asked: the Updates window says why.
        let (tx, rx) = channel();
        u.pending = Some(rx);
        tx.send(Err("connection refused".into())).unwrap();
        assert!(matches!(u.poll(None), Some(Found::UpToDate)));
        assert_eq!(u.last, Some(Err("connection refused".to_string())));
        // A skipped version stays quiet at start.
        let (tx, rx) = channel();
        u.pending = Some(rx);
        u.by_hand = false;
        tx.send(Ok(Some(Release { version: "9.0.0".into(), url: String::new(), notes: String::new(), assets: Vec::new() }))).unwrap();
        assert!(u.poll(Some("9.0.0")).is_none());
    }
}
