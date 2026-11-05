//! Looks for a newer release on GitHub: at start (unless turned off in
//! Settings) and from Help › Check for Updates. Anything going wrong (no
//! internet, the repository still private and answering 404, GitHub's limits)
//! is ignored: the app just doesn't offer an update then.

use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

const LATEST: &str = "https://api.github.com/repos/XCraftTM/XC-Circuit/releases/latest";

/// A release newer than this app.
#[derive(Clone, Debug, PartialEq)]
pub struct Release {
    /// Its version without the v: 1.2.0.
    pub version: String,
    /// Its page, with the downloads.
    pub url: String,
    /// What changed, as written on the release.
    pub notes: String,
}

#[derive(Default)]
pub struct Updater {
    pending: Option<Receiver<Option<Release>>>,
    /// Asked for from the Help menu: then "you have the latest" is said too.
    by_hand: bool,
    /// A newer release, once found.
    pub available: Option<Release>,
    /// Its window is open.
    pub show: bool,
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
            let _ = tx.send(latest().filter(|r| newer(&r.version, env!("CARGO_PKG_VERSION"))));
        });
    }

    /// A finished check, once. `skipped` is a version the user chose to skip
    /// (only a check by hand still shows it).
    pub fn poll(&mut self, skipped: Option<&str>) -> Option<Found> {
        let found = self.pending.as_ref()?.try_recv().ok()?;
        self.pending = None;
        match found {
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
        self.pending.is_some()
    }
}

/// The latest release on GitHub, or None for any trouble.
fn latest() -> Option<Release> {
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(10))).build().into();
    let mut response = agent
        .get(LATEST)
        .header("User-Agent", concat!("XC-Circuit/", env!("CARGO_PKG_VERSION")))
        .header("Accept", "application/vnd.github+json")
        .call()
        .ok()?;
    let body: serde_json::Value = serde_json::from_slice(&response.body_mut().read_to_vec().ok()?).ok()?;
    parse(&body)
}

fn parse(body: &serde_json::Value) -> Option<Release> {
    let text = |k: &str| body.get(k).and_then(|v| v.as_str()).map(str::to_string);
    if body.get("draft").and_then(|v| v.as_bool()).unwrap_or(false) || body.get("prerelease").and_then(|v| v.as_bool()).unwrap_or(false) {
        return None;
    }
    let version = text("tag_name")?.trim_start_matches(['v', 'V']).to_string();
    Some(Release { version, url: text("html_url")?, notes: text("body").unwrap_or_default() })
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
        let r = parse(&serde_json::json!({ "tag_name": "v1.2.0", "html_url": "https://github.com/x/y/releases/tag/v1.2.0", "body": "Fixes", "draft": false, "prerelease": false })).unwrap();
        assert_eq!((r.version.as_str(), r.notes.as_str()), ("1.2.0", "Fixes"));
        assert!(parse(&serde_json::json!({ "tag_name": "v2.0.0", "html_url": "u", "prerelease": true })).is_none(), "previews aren't offered");
        assert!(parse(&serde_json::json!({ "message": "Not Found" })).is_none(), "a private repository answers 404");
    }

    #[test]
    fn checks_quietly() {
        // A check by hand that finds nothing says so; one at start says nothing.
        let mut u = Updater::default();
        let (tx, rx) = channel();
        u.pending = Some(rx);
        tx.send(None).unwrap();
        assert!(u.poll(None).is_none());
        let (tx, rx) = channel();
        u.pending = Some(rx);
        u.by_hand = true;
        tx.send(None).unwrap();
        assert!(matches!(u.poll(None), Some(Found::UpToDate)));
        // A skipped version stays quiet at start.
        let (tx, rx) = channel();
        u.pending = Some(rx);
        u.by_hand = false;
        tx.send(Some(Release { version: "9.0.0".into(), url: String::new(), notes: String::new() })).unwrap();
        assert!(u.poll(Some("9.0.0")).is_none());
    }
}
