//! Secret settings (such as the bot token) in Windows Credential Manager or the
//! Linux Secret Service. Never written to project files.

use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::Path;

use anyhow::{Result, anyhow};
use keyring::Entry;

const SERVICE: &str = "xc-circuit";

/// One entry per project folder and setting.
fn entry(project_root: &Path, name: &str) -> Result<Entry> {
    let mut h = DefaultHasher::new();
    project_root.canonicalize().unwrap_or_else(|_| project_root.to_path_buf()).hash(&mut h);
    Entry::new(SERVICE, &format!("{:016x}:{name}", h.finish())).map_err(|e| anyhow!("the system credential store is not available: {e}"))
}

pub fn get(project_root: &Path, name: &str) -> Option<String> {
    entry(project_root, name).ok()?.get_password().ok().filter(|s| !s.is_empty())
}

pub fn set(project_root: &Path, name: &str, value: &str) -> Result<()> {
    let e = entry(project_root, name)?;
    if value.is_empty() {
        let _ = e.delete_credential();
        return Ok(());
    }
    e.set_password(value).map_err(|e| anyhow!("could not save the secret: {e}"))
}
