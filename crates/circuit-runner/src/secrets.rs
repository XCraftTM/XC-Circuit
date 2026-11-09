//! Secret settings (such as the bot token) in Windows Credential Manager or the
//! Linux Secret Service. Never written to project files.
//!
//! What's read is kept in memory, so the window doesn't wait on the credential
//! store, which can take seconds (or wait for the keyring to be unlocked).
//! [`prefetch`] reads a project's secrets in the background when it opens.

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

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

/// Secrets read or saved so far, per project folder and name; `None` when
/// none is saved.
type Kept = HashMap<(PathBuf, String), Option<String>>;
static CACHE: Mutex<Option<Kept>> = Mutex::new(None);

fn cache() -> MutexGuard<'static, Option<Kept>> {
    CACHE.lock().unwrap_or_else(|e| e.into_inner())
}

fn cached(project_root: &Path, name: &str) -> Option<Option<String>> {
    cache().as_ref()?.get(&(project_root.to_path_buf(), name.to_string())).cloned()
}

/// Keeps what was read, unless something newer is kept already (saved meanwhile).
fn keep_read(project_root: &Path, name: &str, value: Option<String>) {
    cache().get_or_insert_default().entry((project_root.to_path_buf(), name.to_string())).or_insert(value);
}

fn keep_saved(project_root: &Path, name: &str, value: Option<String>) {
    cache().get_or_insert_default().insert((project_root.to_path_buf(), name.to_string()), value);
}

/// Reads a project's secrets in the background, so [`get`] has them at once.
/// What was kept for the project before is read again (it may have been
/// changed elsewhere).
pub fn prefetch(project_root: &Path, names: Vec<String>) {
    if let Some(c) = cache().as_mut() {
        c.retain(|(root, _), _| root != project_root);
    }
    let root = project_root.to_path_buf();
    let _ = std::thread::Builder::new().name("reading secrets".into()).spawn(move || {
        for name in names {
            if cached(&root, &name).is_none() {
                keep_read(&root, &name, read(&root, &name));
            }
        }
    });
}

/// The secret, if one is saved: kept in memory once read.
pub fn get(project_root: &Path, name: &str) -> Option<String> {
    if let Some(v) = cached(project_root, name) {
        return v;
    }
    let v = read(project_root, name);
    keep_read(project_root, name, v.clone());
    v
}

/// Whether a secret is kept in memory already, so [`get`] answers at once.
pub fn is_known(project_root: &Path, name: &str) -> bool {
    cached(project_root, name).is_some()
}

fn read(project_root: &Path, name: &str) -> Option<String> {
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
    let result = store(project_root, name, value);
    if result.is_ok() {
        keep_saved(project_root, name, (!value.is_empty()).then(|| value.to_string()));
    } else if let Some(c) = cache().as_mut() {
        // Not known any more: read again next time.
        c.remove(&(project_root.to_path_buf(), name.to_string()));
    }
    result
}

fn store(project_root: &Path, name: &str, value: &str) -> Result<()> {
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

    #[test]
    fn what_is_read_is_kept_but_never_over_what_was_saved_meanwhile() {
        let root = Path::new("/xc-test/secrets-cache");
        assert!(!is_known(root, "token"));
        keep_read(root, "token", None);
        assert!(is_known(root, "token"), "that none is saved is known too");
        keep_saved(root, "token", Some("new".into()));
        // A read started before the save finishes after it.
        keep_read(root, "token", None);
        assert_eq!(cached(root, "token"), Some(Some("new".into())));
        // Opening the project again reads it again.
        prefetch(root, Vec::new());
        assert!(!is_known(root, "token"));
    }
}
