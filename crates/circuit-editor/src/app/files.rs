//! Files on disk: new projects from templates, the bundled plugins and examples, and the guides.

use super::*;

/// A new project at `dir` from a template folder: its workspaces, slash
/// commands and settings, under the folder's name.
pub(super) fn create_from_template(template: &Path, dir: &Path, name: &str) -> anyhow::Result<()> {
    anyhow::ensure!(!dir.join(circuit_core::project::PROJECT_FILE).exists(), "there is already a project in that folder");
    let mut project = Project::load(template)?;
    project.root = dir.to_path_buf();
    project.meta.name = name.to_string();
    project.save()?;
    let ignore = dir.join(".gitignore");
    if !ignore.exists() {
        std::fs::write(ignore, ".circuit/\n")?;
    }
    Ok(())
}

/// Plugin folders: `XC_CIRCUIT_PLUGINS`, the bundled `plugins/` next to the program
/// (or in a parent folder while developing), and the user's plugin folder.
/// Also returns the folder that holds the bundled `plugins/`.
pub(super) fn find_plugin_dirs() -> (Vec<PathBuf>, Option<PathBuf>) {
    let mut dirs = Vec::new();
    if let Ok(list) = env::var("XC_CIRCUIT_PLUGINS") {
        dirs.extend(env::split_paths(&list));
    }
    let starts = [env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf)), env::current_dir().ok()];
    let bundled_root = starts
        .into_iter()
        .flatten()
        .find_map(|start| start.ancestors().find(|a| a.join("plugins").join("xc.core").join("plugin.toml").is_file()).map(Path::to_path_buf));
    if let Some(root) = &bundled_root {
        dirs.push(root.join("plugins"));
    }
    if let Some(data) = dirs::data_dir() {
        let name = if cfg!(windows) { "XC Circuit" } else { "xc-circuit" };
        dirs.push(data.join(name).join("plugins"));
    }
    dirs.dedup();
    (dirs, bundled_root)
}

/// DemoBot as the user's own copy (in the app's data folder), so it can be
/// saved and run also when XC Circuit is installed in a read-only place. It's
/// copied the first time; later the copy is opened, with the user's changes.
pub(super) fn writable_example(examples: &Path) -> Option<PathBuf> {
    let from = examples.join("DemoBot");
    if !from.is_dir() {
        return None;
    }
    let to = crate::bot::app_data_dir().join("examples").join("DemoBot");
    if to.join(circuit_core::project::PROJECT_FILE).is_file() {
        return Some(to);
    }
    fn copy(from: &Path, to: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(to)?;
        for e in std::fs::read_dir(from)? {
            let e = e?;
            let name = e.file_name();
            if name == ".circuit" {
                continue;
            }
            if e.file_type()?.is_dir() {
                copy(&e.path(), &to.join(&name))?;
            } else {
                std::fs::copy(e.path(), to.join(&name))?;
            }
        }
        Ok(())
    }
    match copy(&from, &to) {
        Ok(()) => Some(to),
        // Can't copy: open it where it is.
        Err(_) => Some(from),
    }
}

/// Opens a document that releases have next to the program (LICENSE,
/// THIRD-PARTY-NOTICES.txt); its page on GitHub when it isn't there.
pub(super) fn open_doc(app_root: Option<&Path>, file: &str) {
    match app_root.map(|r| r.join(file)).filter(|p| p.is_file()) {
        Some(path) => {
            let _ = open::that(path);
        }
        None if file == "LICENSE" => {
            let _ = open::that(format!("{}/blob/main/LICENSE", crate::app::crash::REPO));
        }
        // The notices are made for each release.
        None => {
            let _ = open::that(format!("{}/releases/latest", crate::app::crash::REPO));
        }
    }
}
