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

/// How much a plugin's zip may unpack to. A small zip can say it holds very
/// much (a "zip bomb"), which would fill the memory and the disk; what is
/// really unpacked is counted, not what the zip says.
struct Limits {
    /// The biggest file, and all files together, in bytes.
    file: u64,
    total: u64,
    /// How many files and folders.
    entries: usize,
}

/// Far above any plugin: the bundled ones are a few megabytes each.
const USUAL: Limits = Limits { file: 64 * 1024 * 1024, total: 256 * 1024 * 1024, entries: 20_000 };

fn megabytes(bytes: u64) -> u64 {
    bytes.div_ceil(1024 * 1024)
}

fn install_from(reader: impl Read + Seek, plugins_dir: &Path, replace: bool) -> Result<Installed, InstallError> {
    install_with(reader, plugins_dir, replace, &USUAL)
}

fn install_with(reader: impl Read + Seek, plugins_dir: &Path, replace: bool, limits: &Limits) -> Result<Installed, InstallError> {
    let mut zip = zip::ZipArchive::new(reader).map_err(|e| other(format!("not a zip file: {e}")))?;
    if zip.len() > limits.entries {
        return Err(other(format!("the zip holds {} files, more than a plugin may have ({})", zip.len(), limits.entries)));
    }
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
    zip.by_name(&manifest_at).map_err(other)?.take(1024 * 1024).read_to_string(&mut text).map_err(other)?;
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
        let mut total = 0u64;
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
            // One byte more than allowed is read, to tell a file of that size from a bigger one.
            let mut bytes = Vec::new();
            entry.by_ref().take(limits.file + 1).read_to_end(&mut bytes).map_err(other)?;
            if bytes.len() as u64 > limits.file {
                return Err(other(format!("{} in the zip is bigger than a plugin's file may be ({} MB)", rel.display(), megabytes(limits.file))));
            }
            total += bytes.len() as u64;
            if total > limits.total {
                return Err(other(format!("the zip unpacks to more than a plugin may be ({} MB)", megabytes(limits.total))));
            }
            std::fs::write(&out, bytes).map_err(other)?;
        }
        Ok(())
    })();
    if let Err(e) = unpacked {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(e);
    }
    swap_in(&staging, &dir, &plugins_dir.join(format!(".replaced-{folder}")))?;
    Ok(Installed { name, dir })
}

/// Puts the unpacked plugin in `staging` at `dir`. A version already there
/// steps aside (to `aside`) first and is only deleted once the new one is in
/// its place: if that fails, the old one comes back, so the plugin is never
/// gone.
fn swap_in(staging: &Path, dir: &Path, aside: &Path) -> Result<(), InstallError> {
    let _ = std::fs::remove_dir_all(aside);
    let had_old = dir.exists();
    if had_old && let Err(e) = std::fs::rename(dir, aside) {
        let _ = std::fs::remove_dir_all(staging);
        return Err(other(format!("could not replace the old version (is one of its files open?): {e}")));
    }
    if let Err(e) = std::fs::rename(staging, dir) {
        if had_old {
            let _ = std::fs::rename(aside, dir);
        }
        let _ = std::fs::remove_dir_all(staging);
        return Err(other(format!("could not put the new version in place: {e}")));
    }
    let _ = std::fs::remove_dir_all(aside);
    Ok(())
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
            out.start_file(*name, zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated)).unwrap();
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
        assert!(!tmp.join(".replaced-acme.tools").exists() && !tmp.join(".installing-acme.tools").exists(), "nothing is left behind");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// A zip that unpacks to more than a plugin may be isn't installed, and
    /// leaves nothing behind: neither one huge file nor many files, nor more
    /// files than allowed.
    #[test]
    fn a_zip_that_unpacks_to_too_much_is_refused() {
        let tmp = std::env::temp_dir().join(format!("xc-install-bomb-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let toml = "id = 'acme.big'";
        let big = "0".repeat(100_000);
        let limits = Limits { file: 50_000, total: 120_000, entries: 5 };
        // 100 kB of zeros are a few hundred bytes in the zip.
        let z = zip_of(&[("plugin.toml", toml), ("blocks/big.js", &big)]);
        assert!(z.get_ref().len() < 2_000);
        let err = install_with(z, &tmp, false, &limits).unwrap_err();
        assert!(err.to_string().contains("big.js") && err.to_string().contains("bigger than"), "{err}");
        let part = "0".repeat(45_000);
        let z = zip_of(&[("plugin.toml", toml), ("a.js", &part), ("b.js", &part), ("c.js", &part)]);
        let err = install_with(z, &tmp, false, &limits).unwrap_err();
        assert!(err.to_string().contains("unpacks to more"), "{err}");
        let z = zip_of(&[("plugin.toml", toml), ("1.js", ""), ("2.js", ""), ("3.js", ""), ("4.js", ""), ("5.js", "")]);
        let err = install_with(z, &tmp, false, &limits).unwrap_err();
        assert!(err.to_string().contains("more than a plugin may have"), "{err}");
        assert!(!tmp.join("acme.big").exists() && !tmp.join(".installing-acme.big").exists(), "nothing is left behind");
        // Within the limits it installs.
        let z = zip_of(&[("plugin.toml", toml), ("a.js", &part), ("b.js", &part)]);
        install_with(z, &tmp, false, &limits).unwrap();
        assert_eq!(std::fs::metadata(tmp.join("acme.big").join("a.js")).unwrap().len(), 45_000);
        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// When the new version can't take the old one's place, the old one stays.
    #[test]
    fn a_failed_replace_keeps_the_old_version() {
        let tmp = std::env::temp_dir().join(format!("xc-install-swap-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let (dir, aside) = (tmp.join("acme.tools"), tmp.join(".replaced-acme.tools"));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("plugin.toml"), "id = \"acme.tools\"\nversion = \"1.0.0\"\n").unwrap();
        // Nothing was unpacked: there is no new version to move in.
        let err = swap_in(&tmp.join(".installing-acme.tools"), &dir, &aside).unwrap_err();
        assert!(err.to_string().contains("new version"), "{err}");
        assert!(std::fs::read_to_string(dir.join("plugin.toml")).unwrap().contains("1.0.0"), "the old version is back");
        assert!(!aside.exists());

        // With a new version, it takes the old one's place and the old one goes.
        let staging = tmp.join(".installing-acme.tools");
        std::fs::create_dir_all(&staging).unwrap();
        std::fs::write(staging.join("plugin.toml"), "id = \"acme.tools\"\nversion = \"2.0.0\"\n").unwrap();
        swap_in(&staging, &dir, &aside).unwrap();
        assert!(std::fs::read_to_string(dir.join("plugin.toml")).unwrap().contains("2.0.0"));
        assert!(!aside.exists() && !staging.exists());
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
