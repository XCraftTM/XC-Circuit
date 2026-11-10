//! Secret settings (such as the bot token) in Windows Credential Manager or the
//! Linux Secret Service. Never written to project files.
//!
//! A project's secrets go by the project's id, so they stay with it when its
//! folder is moved or renamed. A second folder with the same id (a copy)
//! gets secrets of its own: the first place a project's secrets were saved
//! from is remembered, and while a project with that id is still there,
//! another folder with it doesn't get them. (A project file can be copied,
//! or made up with someone's id in it; their token mustn't come with it.)
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

/// The entry names' start for a project folder without an id (a project of
/// the layout before): from its path.
fn path_prefix(project_root: &Path) -> String {
    format!("p{:016x}", stable_hash(&canonical(project_root).to_string_lossy()))
}

/// The setting under which the place a project's secrets belong to is kept.
const PLACE: &str = "@place";

/// The start of the entry names for a project with `id` in folder `root`,
/// and whether `root` is to be remembered as the place of its secrets.
/// `recorded` is the place remembered so far; `still_there` says whether a
/// project with this id is still in that folder.
fn prefix_for(id: &str, root: &str, recorded: Option<&str>, still_there: impl FnOnce(&str) -> bool) -> (String, bool) {
    match recorded {
        // Nothing saved for it yet, or this is its place.
        None => (format!("i{id}"), false),
        Some(place) if place == root => (format!("i{id}"), false),
        // The project is still where its secrets were saved: this is a copy.
        Some(place) if still_there(place) => (format!("i{id}@{:016x}", stable_hash(root)), false),
        // It was moved or renamed: its secrets come along.
        Some(_) => (format!("i{id}"), true),
    }
}

/// The start of the entry names per project folder, worked out once (until
/// the project is opened again).
static PREFIXES: Mutex<Option<HashMap<PathBuf, String>>> = Mutex::new(None);

fn prefix(project_root: &Path) -> String {
    let known = PREFIXES.lock().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|m| m.get(project_root).cloned());
    if let Some(p) = known {
        return p;
    }
    let found = match circuit_core::project::id_of(project_root) {
        None => path_prefix(project_root),
        Some(id) => {
            let root = canonical(project_root).to_string_lossy().into_owned();
            let place = open(format!("i{id}:{PLACE}")).ok();
            let recorded = place.as_ref().and_then(|e| e.get_password().ok()).filter(|p| !p.is_empty());
            let (prefix, moved) = prefix_for(&id, &root, recorded.as_deref(), |there| circuit_core::project::id_of(Path::new(there)).as_deref() == Some(id.as_str()));
            if moved && let Some(e) = &place {
                let _ = e.set_password(&root);
            }
            prefix
        }
    };
    PREFIXES.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_default().insert(project_root.to_path_buf(), found.clone());
    found
}

/// One entry per project and setting.
fn entry(project_root: &Path, name: &str) -> Result<Entry> {
    open(format!("{}:{name}", prefix(project_root)))
}

/// Where it was kept while secrets went by the project's folder.
fn path_entry(project_root: &Path, name: &str) -> Result<Entry> {
    open(format!("{}:{name}", path_prefix(project_root)))
}

/// Remembers this folder as the place of the project's secrets, when none is
/// remembered yet (see the module's text).
fn remember_place(project_root: &Path) {
    let Some(id) = circuit_core::project::id_of(project_root) else { return };
    // A copy keeps its own secrets and never becomes the place.
    if prefix(project_root) != format!("i{id}") {
        return;
    }
    if let Ok(e) = open(format!("i{id}:{PLACE}"))
        && e.get_password().ok().filter(|p| !p.is_empty()).is_none()
    {
        let _ = e.set_password(&canonical(project_root).to_string_lossy());
    }
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
    // Its id may be new (a project just brought to the new layout).
    if let Some(p) = PREFIXES.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
        p.remove(project_root);
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
    // Moved over from where an earlier version kept it: by the project's
    // folder, or before that under another name for the folder.
    for old in [path_entry(project_root, name), legacy_entry(project_root, name)].into_iter().flatten() {
        let Some(v) = old.get_password().ok().filter(|s| !s.is_empty()) else { continue };
        if current.set_password(&v).is_ok() {
            let _ = old.delete_credential();
            remember_place(project_root);
        }
        return Some(v);
    }
    None
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
    // A stale copy at an old place must not come back later.
    let by_path = prefix(project_root) == path_prefix(project_root);
    for old in [(!by_path).then(|| path_entry(project_root, name)), Some(legacy_entry(project_root, name))].into_iter().flatten().flatten() {
        let _ = old.delete_credential();
    }
    if !value.is_empty() {
        remember_place(project_root);
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

    /// Secrets go by the project's id: they stay with a project that was
    /// moved, and a copy of it doesn't get them.
    #[test]
    fn secrets_follow_the_project_not_a_copy() {
        let id = "8cd7c5b5ae95edcb6b839ba00c48b6ae";
        let own = format!("i{id}");
        // Nothing saved yet, and at its own place: the project's entries.
        assert_eq!(prefix_for(id, "C:/bots/a", None, |_| unreachable!()), (own.clone(), false));
        assert_eq!(prefix_for(id, "C:/bots/a", Some("C:/bots/a"), |_| unreachable!()), (own.clone(), false));
        // Moved or renamed (nothing at the old place any more): they come along, and the new place is remembered.
        assert_eq!(prefix_for(id, "D:/moved/a", Some("C:/bots/a"), |_| false), (own.clone(), true));
        // The original is still there: a copy, with entries of its own, the same each time.
        let (copy, remember) = prefix_for(id, "C:/bots/copy", Some("C:/bots/a"), |place| place == "C:/bots/a");
        assert!(copy.starts_with(&format!("{own}@")) && copy != own && !remember, "{copy}");
        assert_eq!(prefix_for(id, "C:/bots/copy", Some("C:/bots/a"), |_| true).0, copy);
        assert_ne!(prefix_for(id, "C:/bots/other", Some("C:/bots/a"), |_| true).0, copy);
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
