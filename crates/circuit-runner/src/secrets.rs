//! Secret settings (such as the bot token) in Windows Credential Manager or the
//! Linux Secret Service. Never written to project files.

use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use keyring::Entry;

const SERVICE: &str = "xc-circuit";

fn canonical(project_root: &Path) -> PathBuf {
    project_root.canonicalize().unwrap_or_else(|_| project_root.to_path_buf())
}

/// FNV-1a: a fixed algorithm, so an entry's name stays the same across Rust
/// versions (std's `DefaultHasher` may change and would lose saved tokens).
fn stable_hash(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| (h ^ b as u64).wrapping_mul(0x0100_0000_01b3))
}

fn open(account: String) -> Result<Entry> {
    Entry::new(SERVICE, &account).map_err(|e| anyhow!("the system credential store is not available: {e}"))
}

/// One entry per project folder and setting.
fn entry(project_root: &Path, name: &str) -> Result<Entry> {
    open(format!("p{:016x}:{name}", stable_hash(&canonical(project_root).to_string_lossy())))
}

/// Where earlier versions kept it.
fn legacy_entry(project_root: &Path, name: &str) -> Result<Entry> {
    let mut h = DefaultHasher::new();
    canonical(project_root).hash(&mut h);
    open(format!("{:016x}:{name}", h.finish()))
}

pub fn get(project_root: &Path, name: &str) -> Option<String> {
    let current = entry(project_root, name).ok()?;
    if let Some(v) = current.get_password().ok().filter(|s| !s.is_empty()) {
        return Some(v);
    }
    // Moved over from where an earlier version kept it.
    let old = legacy_entry(project_root, name).ok()?;
    let v = old.get_password().ok().filter(|s| !s.is_empty())?;
    if current.set_password(&v).is_ok() {
        let _ = old.delete_credential();
    }
    Some(v)
}

pub fn set(project_root: &Path, name: &str, value: &str) -> Result<()> {
    let e = entry(project_root, name)?;
    // A stale copy at the old place must not come back later.
    if let Ok(old) = legacy_entry(project_root, name) {
        let _ = old.delete_credential();
    }
    if value.is_empty() {
        let _ = e.delete_credential();
        return Ok(());
    }
    e.set_password(value).map_err(|e| anyhow!("could not save the secret: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_names_are_stable() {
        // Published FNV-1a 64-bit test vectors.
        assert_eq!(stable_hash(""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(stable_hash("a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(stable_hash("foobar"), 0x8594_4171_f739_67e8);
    }
}
