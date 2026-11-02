//! Finding Node.js on this computer, and installing it for the user: with
//! winget or the official installer on Windows, or as a private copy in XC
//! Circuit's own folder on Linux. Downloads come from nodejs.org and are
//! checked against its published SHA-256 sums before anything runs.

use std::collections::HashSet;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, anyhow, bail};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::node::{MIN_NODE_MAJOR, major_version, quiet, version_of};

const DIST: &str = "https://nodejs.org/dist";
/// Largest download accepted (the Windows installer is about 30 MB).
const MAX_DOWNLOAD: u64 = 150 * 1024 * 1024;

/// One Node.js found on this computer.
#[derive(Clone, Debug, PartialEq)]
pub struct NodeInstall {
    pub path: PathBuf,
    /// `None` when it couldn't be started.
    pub version: Option<String>,
    /// Where it was found: "PATH", "nvm", "Program Files", …
    pub source: String,
}

impl NodeInstall {
    /// New enough for bots.
    pub fn usable(&self) -> bool {
        self.version.as_deref().is_some_and(|v| major_version(v) >= MIN_NODE_MAJOR)
    }
}

fn exe() -> &'static str {
    if cfg!(windows) { "node.exe" } else { "node" }
}

/// Folders matching `base/*/rest` (one level of version folders, as nvm uses).
fn versions_under(base: &Path, rest: &[&str]) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(base) else { return Vec::new() };
    entries
        .flatten()
        .map(|e| rest.iter().fold(e.path(), |p, part| p.join(part)))
        .collect()
}

/// Places Node.js is commonly installed, with where they come from.
fn candidates(private_root: &Path) -> Vec<(PathBuf, &'static str)> {
    let mut list: Vec<(PathBuf, &'static str)> = Vec::new();
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            list.push((dir.join(exe()), "PATH"));
        }
    }
    let env = |k: &str| std::env::var_os(k).map(PathBuf::from);
    let home = dirs::home_dir();
    if cfg!(windows) {
        for (var, sub, source) in [
            ("ProgramFiles", "nodejs", "Program Files"),
            ("ProgramFiles(x86)", "nodejs", "Program Files (x86)"),
            ("LOCALAPPDATA", r"Programs\nodejs", "user install"),
            ("NVM_SYMLINK", "", "nvm"),
            ("LOCALAPPDATA", r"Volta\bin", "Volta"),
            ("ProgramData", r"chocolatey\bin", "Chocolatey"),
        ] {
            if let Some(base) = env(var) {
                list.push((base.join(sub).join(exe()), source));
            }
        }
        for (var, source) in [("NVM_HOME", "nvm"), ("APPDATA", "nvm")] {
            if let Some(base) = env(var) {
                let base = if var == "APPDATA" { base.join("nvm") } else { base };
                list.extend(versions_under(&base, &[exe()]).into_iter().map(|p| (p, source)));
            }
        }
        if let Some(appdata) = env("APPDATA") {
            list.extend(versions_under(&appdata.join(r"fnm\node-versions"), &["installation", exe()]).into_iter().map(|p| (p, "fnm")));
        }
        if let Some(h) = &home {
            for app in ["nodejs", "nodejs-lts"] {
                list.push((h.join("scoop").join("apps").join(app).join("current").join(exe()), "Scoop"));
            }
        }
    } else {
        for (dir, source) in [("/usr/bin", "system"), ("/usr/local/bin", "system"), ("/snap/bin", "Snap"), ("/home/linuxbrew/.linuxbrew/bin", "Homebrew")] {
            list.push((PathBuf::from(dir).join(exe()), source));
        }
        if let Some(h) = &home {
            list.extend(versions_under(&h.join(".nvm/versions/node"), &["bin", exe()]).into_iter().map(|p| (p, "nvm")));
            list.extend(versions_under(&h.join(".local/share/fnm/node-versions"), &["installation", "bin", exe()]).into_iter().map(|p| (p, "fnm")));
            list.push((h.join(".volta/bin").join(exe()), "Volta"));
        }
    }
    // Copies XC Circuit installed itself.
    let private_exe = |dir: PathBuf| if cfg!(windows) { dir.join(exe()) } else { dir.join("bin").join(exe()) };
    list.extend(fs::read_dir(private_root).into_iter().flatten().flatten().map(|e| (private_exe(e.path()), "XC Circuit")));
    list
}

/// Every Node.js found, newest first, each with its version (or none if it
/// wouldn't start). Starts each candidate once, so call it off the UI thread.
pub fn find_installs(private_root: &Path) -> Vec<NodeInstall> {
    let mut seen = HashSet::new();
    let mut found: Vec<NodeInstall> = Vec::new();
    for (path, source) in candidates(private_root) {
        if !path.is_file() {
            continue;
        }
        let key = path.canonicalize().unwrap_or_else(|_| path.clone());
        if !seen.insert(key) {
            continue;
        }
        let version = version_of(&path).ok();
        found.push(NodeInstall { path, version, source: source.to_string() });
    }
    found.sort_by_key(|n| std::cmp::Reverse(n.version.as_deref().map(version_key).unwrap_or_default()));
    found
}

fn version_key(v: &str) -> (u32, u32, u32) {
    let mut parts = v.trim_start_matches('v').split('.').map(|p| p.parse().unwrap_or(0));
    (parts.next().unwrap_or(0), parts.next().unwrap_or(0), parts.next().unwrap_or(0))
}

/// Whether winget (Windows Package Manager) is there to use.
pub fn winget_available() -> bool {
    cfg!(windows) && quiet(Command::new("winget").arg("--version")).output().is_ok_and(|o| o.status.success())
}

/// Runs a command, sending each output line to `log`.
fn run_logged(cmd: &mut Command, log: &mut dyn FnMut(String)) -> Result<std::process::ExitStatus> {
    let mut child = quiet(cmd).stdout(Stdio::piped()).stderr(Stdio::piped()).stdin(Stdio::null()).spawn()?;
    let err = child.stderr.take();
    if let Some(out) = child.stdout.take() {
        for line in BufReader::new(out).lines().map_while(|l| l.ok()) {
            let line = line.trim().trim_matches(|c: char| "\\|/-".contains(c)).trim().to_string();
            if !line.is_empty() {
                log(line);
            }
        }
    }
    if let Some(err) = err {
        for line in BufReader::new(err).lines().map_while(|l| l.ok()).filter(|l| !l.trim().is_empty()) {
            log(line);
        }
    }
    Ok(child.wait()?)
}

/// Where the official Windows installer puts Node.js.
pub fn program_files_node() -> Option<PathBuf> {
    let p = PathBuf::from(std::env::var_os("ProgramFiles")?).join("nodejs").join("node.exe");
    p.is_file().then_some(p)
}

/// Installs the current Node.js LTS with winget. Windows asks for permission.
pub fn install_with_winget(log: &mut dyn FnMut(String)) -> Result<PathBuf> {
    log("Installing Node.js LTS with winget. Windows may ask for permission.".into());
    let status = run_logged(
        Command::new("winget").args([
            "install",
            "--id",
            "OpenJS.NodeJS.LTS",
            "--exact",
            "--silent",
            "--accept-package-agreements",
            "--accept-source-agreements",
            "--disable-interactivity",
        ]),
        log,
    )
    .context("winget couldn't be started")?;
    // winget reports "already installed" with a non-zero code; a node.exe in
    // place counts as done.
    match program_files_node() {
        Some(p) => Ok(p),
        None if status.success() => Err(anyhow!("winget finished, but Node.js isn't in Program Files. Use Search again or Browse to find it.")),
        None => Err(anyhow!("winget couldn't install Node.js (exit code {}).", status.code().unwrap_or(-1))),
    }
}

#[derive(Deserialize)]
struct Release {
    version: String,
    lts: serde_json::Value,
    files: Vec<String>,
}

/// The newest LTS release new enough for bots that has `file_kind` (such as
/// `win-x64-msi`), from nodejs.org's release list.
fn pick_release(index: &str, file_kind: &str) -> Result<String> {
    let releases: Vec<Release> = serde_json::from_str(index).context("nodejs.org sent a release list that couldn't be read")?;
    releases
        .into_iter()
        .filter(|r| r.lts.is_string() && major_version(&r.version) >= MIN_NODE_MAJOR && r.files.iter().any(|f| f == file_kind))
        .max_by_key(|r| version_key(&r.version))
        .map(|r| r.version)
        .ok_or_else(|| anyhow!("nodejs.org lists no LTS release with {file_kind}"))
}

fn get_text(url: &str) -> Result<String> {
    let mut resp = ureq::get(url).call().with_context(|| format!("couldn't reach {url}"))?;
    Ok(resp.body_mut().read_to_string()?)
}

/// The SHA-256 nodejs.org publishes for one file of a release.
fn published_sum(sums: &str, file: &str) -> Result<String> {
    sums.lines()
        .filter_map(|l| l.split_once("  "))
        .find(|(_, name)| name.trim() == file)
        .map(|(sum, _)| sum.trim().to_lowercase())
        .ok_or_else(|| anyhow!("nodejs.org lists no checksum for {file}"))
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// Downloads one file of a release into `dir`, checked against its published
/// SHA-256. Nothing unchecked is kept.
fn download_release_file(version: &str, file: &str, dir: &Path, log: &mut dyn FnMut(String)) -> Result<PathBuf> {
    let sums = get_text(&format!("{DIST}/{version}/SHASUMS256.txt"))?;
    let expected = published_sum(&sums, file)?;
    log(format!("Downloading {file} …"));
    let url = format!("{DIST}/{version}/{file}");
    let mut resp = ureq::get(&url).call().with_context(|| format!("couldn't download {url}"))?;
    let total = resp.body().content_length();
    let mut reader = resp.body_mut().as_reader().take(MAX_DOWNLOAD);
    let mut bytes = Vec::with_capacity(total.unwrap_or(0) as usize);
    let mut chunk = vec![0u8; 256 * 1024];
    let mut last_report = 0u64;
    loop {
        let n = reader.read(&mut chunk)?;
        if n == 0 {
            break;
        }
        bytes.extend_from_slice(&chunk[..n]);
        let got = bytes.len() as u64;
        if got - last_report >= 5 * 1024 * 1024 {
            last_report = got;
            match total {
                Some(t) if t > 0 => log(format!("{} of {} MB", got / 1_048_576, t / 1_048_576)),
                _ => log(format!("{} MB", got / 1_048_576)),
            }
        }
    }
    let actual = sha256_hex(&bytes);
    if actual != expected {
        bail!("The download didn't match nodejs.org's checksum, so it was thrown away. Try again.");
    }
    log("Checksum verified".into());
    fs::create_dir_all(dir)?;
    let path = dir.join(file);
    let mut f = fs::File::create(&path)?;
    f.write_all(&bytes)?;
    Ok(path)
}

/// Downloads the official Windows installer and runs it in unattended mode
/// (only a progress bar; Windows asks for permission).
pub fn install_official_windows(downloads: &Path, log: &mut dyn FnMut(String)) -> Result<PathBuf> {
    let arch = if cfg!(target_arch = "aarch64") { "arm64" } else { "x64" };
    log("Looking up the current Node.js LTS …".into());
    let version = pick_release(&get_text(&format!("{DIST}/index.json"))?, &format!("win-{arch}-msi"))?;
    let file = format!("node-{version}-{arch}.msi");
    let msi = download_release_file(&version, &file, downloads, log)?;
    log(format!("Installing Node.js {version}. Windows will ask for permission …"));
    let status = run_logged(Command::new("msiexec").arg("/i").arg(&msi).args(["/passive", "/norestart"]), log).context("the installer couldn't be started")?;
    let _ = fs::remove_file(&msi);
    // 3010: installed, a restart is suggested.
    if !status.success() && status.code() != Some(3010) {
        bail!("The installer stopped (exit code {}). If you said no to the permission prompt, try again.", status.code().unwrap_or(-1));
    }
    program_files_node().ok_or_else(|| anyhow!("The installer finished, but node.exe isn't in Program Files."))
}

/// Downloads Node.js into `root` (XC Circuit's own folder; no admin rights
/// needed). Linux.
pub fn install_private(root: &Path, log: &mut dyn FnMut(String)) -> Result<PathBuf> {
    let arch = match std::env::consts::ARCH {
        "x86_64" => "x64",
        "aarch64" => "arm64",
        other => bail!("There is no Node.js download for this processor ({other})."),
    };
    log("Looking up the current Node.js LTS …".into());
    let version = pick_release(&get_text(&format!("{DIST}/index.json"))?, &format!("linux-{arch}"))?;
    let file = format!("node-{version}-linux-{arch}.tar.gz");
    let archive = download_release_file(&version, &file, &root.join("downloads"), log)?;
    log("Unpacking …".into());
    let folder = root.join(format!("node-{version}-linux-{arch}"));
    let _ = fs::remove_dir_all(&folder);
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(fs::File::open(&archive)?));
    // `unpack` refuses entries that would land outside the folder.
    tar.unpack(root).context("couldn't unpack Node.js")?;
    let _ = fs::remove_file(&archive);
    let node = folder.join("bin").join("node");
    let version_found = version_of(&node)?;
    log(format!("Node.js {version_found} is ready in {}", folder.display()));
    Ok(node)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_the_newest_lts_with_the_file() {
        let index = r#"[
            {"version":"v25.1.0","lts":false,"files":["win-x64-msi","linux-x64"]},
            {"version":"v24.11.0","lts":"Krypton","files":["win-x64-msi","linux-x64"]},
            {"version":"v24.10.0","lts":"Krypton","files":["win-x64-msi"]},
            {"version":"v22.20.0","lts":"Jod","files":["win-x64-msi","linux-x64"]},
            {"version":"v20.19.0","lts":"Iron","files":["win-x64-msi","linux-x64"]}
        ]"#;
        assert_eq!(pick_release(index, "win-x64-msi").unwrap(), "v24.11.0", "LTS only, the newest");
        assert_eq!(pick_release(index, "linux-arm64").ok(), None);
    }

    #[test]
    fn checksums_are_matched_by_file_name() {
        let sums = "aa11  node-v24.11.0-x64.msi\nbb22  node-v24.11.0-linux-x64.tar.gz\n";
        assert_eq!(published_sum(sums, "node-v24.11.0-linux-x64.tar.gz").unwrap(), "bb22");
        assert!(published_sum(sums, "node-v24.11.0-arm64.msi").is_err());
        assert_eq!(sha256_hex(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }

    /// Talks to nodejs.org: run with `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn real_release_list_and_checksums() {
        let index = get_text(&format!("{DIST}/index.json")).unwrap();
        for kind in ["win-x64-msi", "linux-x64"] {
            let version = pick_release(&index, kind).unwrap();
            assert!(major_version(&version) >= MIN_NODE_MAJOR, "{version}");
            let sums = get_text(&format!("{DIST}/{version}/SHASUMS256.txt")).unwrap();
            let file = if kind == "win-x64-msi" { format!("node-{version}-x64.msi") } else { format!("node-{version}-linux-x64.tar.gz") };
            let sum = published_sum(&sums, &file).unwrap();
            assert_eq!(sum.len(), 64, "{file}: {sum}");
            println!("{kind}: {version} {file} {sum}");
        }
    }

    /// Downloads the real Windows installer (about 30 MB), checks it, deletes it.
    #[test]
    #[ignore]
    fn real_download_is_verified() {
        let version = pick_release(&get_text(&format!("{DIST}/index.json")).unwrap(), "win-x64-msi").unwrap();
        let dir = std::env::temp_dir().join(format!("xc-node-dl-{}", std::process::id()));
        let mut lines = Vec::new();
        let file = download_release_file(&version, &format!("node-{version}-x64.msi"), &dir, &mut |l| lines.push(l)).unwrap();
        assert!(lines.iter().any(|l| l == "Checksum verified"), "{lines:?}");
        assert!(fs::metadata(&file).unwrap().len() > 10_000_000);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn finds_the_node_running_these_tests() {
        // CI and development machines have Node.js on the PATH.
        let found = find_installs(&std::env::temp_dir().join("xc-no-private-node"));
        assert!(found.iter().any(|n| n.source == "PATH" && n.version.is_some()), "{found:?}");
        let mut seen = HashSet::new();
        assert!(found.iter().all(|n| seen.insert(n.path.canonicalize().unwrap_or(n.path.clone()))), "no duplicates");
    }
}
