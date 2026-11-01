//! Records what this build was made with, so the editor can refuse a native
//! plugin whose compiler or egui crates differ from its own.

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let rustc = env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
    let version = Command::new(&rustc)
        .arg("-V")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=XC_PLUGIN_RUSTC={version}");
    println!("cargo:rustc-env=XC_PLUGIN_TARGET={}", env::var("TARGET").unwrap_or_default());
    println!("cargo:rerun-if-env-changed=RUSTC");

    let deps = match find_lockfile() {
        Some(lock) => {
            println!("cargo:rerun-if-changed={}", lock.display());
            egui_closure(&lock).unwrap_or_else(|| "unknown".into())
        }
        None => "unknown".into(),
    };
    println!("cargo:rustc-env=XC_PLUGIN_DEPS={deps}");
}

/// The Cargo.lock of the workspace being built: the build output folder
/// normally lives inside it.
fn find_lockfile() -> Option<PathBuf> {
    let out = PathBuf::from(env::var("OUT_DIR").ok()?);
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").ok()?);
    out.ancestors().chain(manifest.ancestors()).map(|d| d.join("Cargo.lock")).find(|p| p.is_file())
}

/// `name@version` of egui and everything it depends on, sorted.
fn egui_closure(lock: &Path) -> Option<String> {
    let text = std::fs::read_to_string(lock).ok()?;
    let doc: toml::Table = text.parse().ok()?;
    let packages = doc.get("package")?.as_array()?;
    let entries: Vec<(String, String, Vec<String>)> = packages
        .iter()
        .filter_map(|p| {
            let name = p.get("name")?.as_str()?.to_string();
            let version = p.get("version")?.as_str()?.to_string();
            let deps = p
                .get("dependencies")
                .and_then(|d| d.as_array())
                .map(|d| d.iter().filter_map(|x| x.as_str().map(String::from)).collect())
                .unwrap_or_default();
            Some((name, version, deps))
        })
        .collect();
    // A dependency entry is "name", or "name version" when several versions exist.
    let find = |dep: &str| {
        let mut parts = dep.split(' ');
        let name = parts.next().unwrap_or_default();
        let version = parts.next();
        entries.iter().position(|(n, v, _)| n == name && version.is_none_or(|ver| ver == v))
    };
    let mut seen = vec![false; entries.len()];
    let mut stack = vec![find("egui")?];
    while let Some(i) = stack.pop() {
        if std::mem::replace(&mut seen[i], true) {
            continue;
        }
        stack.extend(entries[i].2.iter().filter_map(|d| find(d)));
    }
    let mut list: Vec<String> = entries.iter().zip(&seen).filter(|(_, s)| **s).map(|((n, v, _), _)| format!("{n}@{v}")).collect();
    list.sort();
    Some(list.join(","))
}
