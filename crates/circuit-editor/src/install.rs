//! Installs a plugin from a `.zip` file into the user's plugins folder: from
//! Plugins › Install from zip…, or by dropping the file on the window.
//!
//! The zip holds a plugin folder: `plugin.toml` at its top, or inside one
//! folder at its top (as GitHub's "Download ZIP" makes them).

use std::io::{Read, Seek};
use std::path::{Component, Path, PathBuf};

use circuit_plugins::PluginManifest;

#[derive(Debug, PartialEq)]
pub enum InstallError {
    /// A plugin with this id is installed already; install again with `replace`.
    Exists { name: String, dir: PathBuf },
    Other(String),
}

impl std::fmt::Display for InstallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InstallError::Exists { name, .. } => write!(f, "{name} is installed already"),
            InstallError::Other(e) => f.write_str(e),
        }
    }
}

fn other(e: impl std::fmt::Display) -> InstallError {
    InstallError::Other(e.to_string())
}

/// What was installed.
#[derive(Debug)]
pub struct Installed {
    pub name: String,
    pub dir: PathBuf,
}

/// Installs the plugin in the zip at `zip_path` into `plugins_dir`, in a folder
/// named after its id. An installed plugin with the same id is only replaced
/// when `replace` is set.
pub fn install_zip(zip_path: &Path, plugins_dir: &Path, replace: bool) -> Result<Installed, InstallError> {
    let file = std::fs::File::open(zip_path).map_err(|e| other(format!("could not open {}: {e}", zip_path.display())))?;
    install_from(file, plugins_dir, replace)
}

fn install_from(reader: impl Read + Seek, plugins_dir: &Path, replace: bool) -> Result<Installed, InstallError> {
    let mut zip = zip::ZipArchive::new(reader).map_err(|e| other(format!("not a zip file: {e}")))?;
    // Where plugin.toml is: at the top, or in one folder at the top.
    let names: Vec<String> = zip.file_names().filter_map(|n| n.ok()).map(|n| n.into_owned()).collect();
    let manifest_at = names
        .iter()
        .filter(|n| {
            let n = n.trim_start_matches("./");
            n == "plugin.toml" || (n.ends_with("/plugin.toml") && n.matches('/').count() == 1)
        })
        .min_by_key(|n| n.len())
        .cloned()
        .ok_or_else(|| other("the zip has no plugin.toml (at its top, or in one folder at its top)"))?;
    let prefix = manifest_at.trim_end_matches("plugin.toml").to_string();

    let mut text = String::new();
    zip.by_name(&manifest_at).map_err(other)?.read_to_string(&mut text).map_err(other)?;
    let manifest: PluginManifest = toml::from_str(&text).map_err(|e| other(format!("plugin.toml can't be read: {e}")))?;
    let folder = safe_folder_name(&manifest.id).ok_or_else(|| other("plugin.toml has no usable id"))?;
    let name = if manifest.name.is_empty() { manifest.id.clone() } else { manifest.name.clone() };
    let dir = plugins_dir.join(&folder);
    if dir.exists() && !replace {
        return Err(InstallError::Exists { name, dir });
    }

    // Unpacked next to the target first, so a broken zip leaves the old version alone.
    std::fs::create_dir_all(plugins_dir).map_err(other)?;
    let staging = plugins_dir.join(format!(".installing-{folder}"));
    let _ = std::fs::remove_dir_all(&staging);
    let unpacked = (|| -> Result<(), InstallError> {
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i).map_err(other)?;
            // `enclosed_name` refuses paths that would leave the folder (../, absolute).
            let Some(path) = entry.enclosed_name() else { continue };
            let Ok(rel) = path.strip_prefix(&prefix) else { continue };
            if rel.as_os_str().is_empty() || rel.components().any(|c| !matches!(c, Component::Normal(_))) {
                continue;
            }
            let out = staging.join(rel);
            if entry.is_dir() {
                std::fs::create_dir_all(&out).map_err(other)?;
                continue;
            }
            if let Some(parent) = out.parent() {
                std::fs::create_dir_all(parent).map_err(other)?;
            }
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).map_err(other)?;
            std::fs::write(&out, bytes).map_err(other)?;
        }
        Ok(())
    })();
    if let Err(e) = unpacked {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(e);
    }
    if dir.exists() {
        std::fs::remove_dir_all(&dir).map_err(|e| other(format!("could not remove the old version: {e}")))?;
    }
    std::fs::rename(&staging, &dir).map_err(other)?;
    Ok(Installed { name, dir })
}

/// A plugin id as a folder name: letters, digits, `.`, `-` and `_`.
fn safe_folder_name(id: &str) -> Option<String> {
    let name: String = id.trim().chars().map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') { c } else { '-' }).collect();
    let name = name.trim_matches('.').to_string();
    (!name.is_empty()).then_some(name)
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Write};

    use super::*;

    fn zip_of(files: &[(&str, &str)]) -> Cursor<Vec<u8>> {
        let mut out = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, text) in files {
            out.start_file(*name, zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored)).unwrap();
            out.write_all(text.as_bytes()).unwrap();
        }
        let mut c = out.finish().unwrap();
        c.set_position(0);
        c
    }

    #[test]
    fn installs_a_plugin_zip() {
        let tmp = std::env::temp_dir().join(format!("xc-install-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let toml = "id = \"acme.tools\"\nname = \"Acme Tools\"\n";
        // In a folder at the top, as GitHub's Download ZIP makes them.
        let z = zip_of(&[("tools-main/plugin.toml", toml), ("tools-main/blocks/a.js", "module.exports = {}"), ("tools-main/../evil.txt", "no")]);
        let done = install_from(z, &tmp, false).unwrap();
        assert_eq!((done.name.as_str(), done.dir.clone()), ("Acme Tools", tmp.join("acme.tools")));
        assert!(tmp.join("acme.tools/blocks/a.js").is_file());
        assert!(!tmp.join("evil.txt").exists(), "nothing lands outside the plugin");

        // Again: asks first, then replaces.
        let z = zip_of(&[("plugin.toml", toml), ("editor.js", "xc.log('v2')")]);
        assert!(matches!(install_from(z.clone(), &tmp, false), Err(InstallError::Exists { .. })));
        install_from(z, &tmp, true).unwrap();
        assert!(tmp.join("acme.tools/editor.js").is_file() && !tmp.join("acme.tools/blocks").exists());

        let err = install_from(zip_of(&[("readme.md", "hi")]), &tmp, false).unwrap_err();
        assert!(err.to_string().contains("no plugin.toml"), "{err}");
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
