//! The project folder: `circuit.json` plus one JSON file per workspace.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::graph::Workspace;

pub const PROJECT_FILE: &str = "circuit.json";
pub const WORKSPACE_DIR: &str = "workspaces";
pub const FORMAT: u32 = 1;

/// `circuit.json`. Secrets never go in here.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectMeta {
    pub format: u32,
    pub name: String,
    pub mode: String,
    pub plugins: IndexMap<String, String>,
    pub settings: Map<String, Value>,
    /// Project-wide data owned by project editors, such as `slashCommands`.
    #[serde(skip_serializing_if = "Map::is_empty")]
    pub data: Map<String, Value>,
    /// Workspace folders in sidebar order, kept even while empty.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub folders: Vec<String>,
    /// Workspace keys in sidebar order, when it isn't their files' order (by
    /// name). Workspaces it doesn't name come after these, by name.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub order: Vec<String>,
}

impl Default for ProjectMeta {
    fn default() -> Self {
        Self {
            format: FORMAT,
            name: "New Bot".into(),
            mode: "discord-bot".into(),
            plugins: IndexMap::new(),
            settings: Map::new(),
            data: Map::new(),
            folders: Vec::new(),
            order: Vec::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Project {
    pub root: PathBuf,
    pub meta: ProjectMeta,
    /// Keyed by file stem, so `workspaces/rank-command.json` is `rank-command`.
    pub workspaces: IndexMap<String, Workspace>,
    /// Workspace files that couldn't be read, with the reason. They are never
    /// overwritten or deleted, so nothing in them is lost.
    pub unreadable: Vec<(String, String)>,
    /// Workspaces deleted since the last save; their files go when saving.
    pub removed: Vec<String>,
}

impl Project {
    pub fn load(root: impl AsRef<Path>) -> Result<Project> {
        let root = root.as_ref().to_path_buf();
        let meta_path = root.join(PROJECT_FILE);
        let meta: ProjectMeta = serde_json::from_str(
            &fs::read_to_string(&meta_path).with_context(|| format!("reading {}", meta_path.display()))?,
        )
        .with_context(|| format!("parsing {}", meta_path.display()))?;

        let mut workspaces = IndexMap::new();
        let mut unreadable = Vec::new();
        let dir = root.join(WORKSPACE_DIR);
        if dir.is_dir() {
            let mut files: Vec<_> = fs::read_dir(&dir)?
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().is_some_and(|e| e == "json"))
                .collect();
            files.sort();
            for path in files {
                let stem = path.file_stem().unwrap_or_default().to_string_lossy().into_owned();
                // One damaged file shouldn't keep the whole project from opening.
                let read = fs::read_to_string(&path).map_err(anyhow::Error::from).and_then(|text| Ok(serde_json::from_str::<Workspace>(&text)?));
                match read {
                    Ok(mut ws) => {
                        ws.pin_wired_fields();
                        workspaces.insert(stem, ws);
                    }
                    Err(e) => unreadable.push((stem, format!("{e:#}"))),
                }
            }
        }
        // The sidebar order: the saved one first, then the rest as their files came.
        let mut ordered = IndexMap::new();
        for key in &meta.order {
            if let Some(ws) = workspaces.shift_remove(key) {
                ordered.insert(key.clone(), ws);
            }
        }
        ordered.extend(workspaces);
        Ok(Project { root, meta, workspaces: ordered, unreadable, removed: Vec::new() })
    }

    /// The order to save: empty while it's the files' own order (by name).
    fn order_to_save(&self) -> Vec<String> {
        let keys: Vec<String> = self.workspaces.keys().cloned().collect();
        let mut by_file = keys.clone();
        by_file.sort_by_key(|k| format!("{k}.json"));
        if keys == by_file { Vec::new() } else { keys }
    }

    /// Creates a new project folder with one empty workspace.
    pub fn create(root: impl AsRef<Path>, name: &str, mode: &str) -> Result<Project> {
        let root = root.as_ref().to_path_buf();
        fs::create_dir_all(root.join(WORKSPACE_DIR))?;
        let meta = ProjectMeta { name: name.into(), mode: mode.into(), ..Default::default() };
        let mut workspaces = IndexMap::new();
        workspaces.insert("main".to_string(), Workspace::new("Main"));
        let mut project = Project { root, meta, workspaces, unreadable: Vec::new(), removed: Vec::new() };
        project.save()?;
        // Run output and test data stay out of version control.
        let ignore = project.root.join(".gitignore");
        if !ignore.exists() {
            fs::write(ignore, ".circuit/\n")?;
        }
        Ok(project)
    }

    /// A copy to save elsewhere (on another thread): as `save` would write
    /// it, with the workspaces to delete. Those are no longer this one's to
    /// delete; give them back with [`Project::removed`] if that save fails.
    pub fn take_save(&mut self) -> Project {
        self.meta.order = self.order_to_save();
        let copy = self.clone();
        self.removed.clear();
        copy
    }

    pub fn save(&mut self) -> Result<()> {
        fs::create_dir_all(self.root.join(WORKSPACE_DIR))?;
        self.meta.order = self.order_to_save();
        write_json(&self.root.join(PROJECT_FILE), &self.meta)?;
        for (stem, ws) in &self.workspaces {
            write_json(&self.workspace_path(stem), ws)?;
        }
        for stem in std::mem::take(&mut self.removed) {
            let unsafe_to_delete = self.workspaces.contains_key(&stem) || self.unreadable.iter().any(|(s, _)| *s == stem);
            if !unsafe_to_delete {
                match fs::remove_file(self.workspace_path(&stem)) {
                    Ok(()) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => return Err(e).with_context(|| format!("deleting {}", self.workspace_path(&stem).display())),
                }
            }
        }
        Ok(())
    }

    /// Takes a workspace out of the project; its file is deleted on the next save.
    pub fn remove_workspace(&mut self, key: &str) {
        if self.workspaces.shift_remove(key).is_some() {
            self.removed.push(key.to_string());
        }
    }

    pub fn workspace_path(&self, stem: &str) -> PathBuf {
        self.root.join(WORKSPACE_DIR).join(format!("{stem}.json"))
    }

    /// A file stem not used yet, derived from a display name.
    pub fn new_workspace_key(&self, name: &str) -> String {
        let base: String = name
            .to_lowercase()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect::<String>()
            .split('-')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("-");
        let base = if base.is_empty() { "workspace".to_string() } else { base };
        // Windows keeps some names for devices, with any ending: con.json
        // can't be written there (and what's written to nul.json is gone).
        let base = if is_device_name(&base) { format!("{base}-workspace") } else { base };
        // Case-insensitive, like file names on Windows, and never the file of a
        // workspace that couldn't be read or is about to be deleted.
        let taken = |k: &str| {
            self.workspaces.keys().chain(self.unreadable.iter().map(|(s, _)| s)).chain(self.removed.iter()).any(|t| t.eq_ignore_ascii_case(k))
        };
        if !taken(&base) {
            return base;
        }
        (2..).map(|i| format!("{base}-{i}")).find(|k| !taken(k)).expect("unbounded range")
    }
}

/// Whether Windows keeps this file name (without its ending) for a device.
fn is_device_name(stem: &str) -> bool {
    let s = stem.to_ascii_lowercase();
    matches!(s.as_str(), "con" | "prn" | "aux" | "nul") || (s.len() == 4 && (s.starts_with("com") || s.starts_with("lpt")) && s.as_bytes()[3].is_ascii_digit())
}

/// Writes pretty JSON through a temporary file, so a crash never leaves half
/// a file. A file that already says the same is left alone: a save writes
/// only what changed (the other workspaces keep their dates, and aren't
/// waited for to reach the disk).
pub(crate) fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let mut text = serde_json::to_string_pretty(value)?;
    text.push('\n');
    if fs::read_to_string(path).is_ok_and(|old| old == text) {
        return Ok(());
    }
    let tmp = path.with_extension("json.tmp");
    // On the disk before it takes the old file's place: after a power cut
    // the file is the old one or the new one, never an empty one.
    let written = fs::File::create(&tmp).and_then(|mut f| {
        std::io::Write::write_all(&mut f, text.as_bytes())?;
        f.sync_all()
    });
    written.with_context(|| format!("writing {}", tmp.display()))?;
    fs::rename(&tmp, path).with_context(|| format!("replacing {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("xc-project-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn a_damaged_workspace_is_kept_and_the_rest_opens() {
        let dir = temp("damaged");
        let mut p = Project::create(&dir, "Test", "none").unwrap();
        p.workspaces.insert("other".into(), Workspace::new("Other"));
        p.save().unwrap();
        let broken = dir.join(WORKSPACE_DIR).join("broken.json");
        fs::write(&broken, "{ this is not json").unwrap();

        let mut p = Project::load(&dir).unwrap();
        assert_eq!(p.workspaces.len(), 2, "the readable ones open");
        assert_eq!(p.unreadable.len(), 1);
        assert_eq!(p.unreadable[0].0, "broken");
        // Saving never touches it, and a new workspace can't take its file name.
        assert_eq!(p.new_workspace_key("Broken"), "broken-2");
        p.save().unwrap();
        assert_eq!(fs::read_to_string(&broken).unwrap(), "{ this is not json");
    }

    #[test]
    fn deleted_workspaces_lose_their_file_only_on_save() {
        let dir = temp("delete");
        let mut p = Project::create(&dir, "Test", "none").unwrap();
        let file = p.workspace_path("main");
        p.remove_workspace("main");
        assert!(file.is_file(), "still there until saved, so Don't save keeps it");
        p.save().unwrap();
        assert!(!file.is_file());
        assert!(Project::load(&dir).unwrap().workspaces.is_empty());
    }

    #[test]
    fn workspace_keys_ignore_case() {
        let dir = temp("case");
        let mut p = Project::create(&dir, "Test", "none").unwrap();
        p.workspaces.insert("Rank".into(), Workspace::new("Rank"));
        assert_eq!(p.new_workspace_key("rank"), "rank-2", "rank.json would overwrite Rank.json on Windows");
        assert_eq!(p.new_workspace_key("../../etc/passwd"), "etc-passwd");
    }

    /// Saving writes the files that changed and leaves the others alone.
    #[test]
    fn saving_leaves_unchanged_files_alone() {
        let dir = temp("unchanged");
        let mut p = Project::create(&dir, "Test", "none").unwrap();
        p.workspaces.insert("other".into(), Workspace::new("Other"));
        p.save().unwrap();
        let (main, other) = (p.workspace_path("main"), p.workspace_path("other"));
        let written = |file: &Path| fs::metadata(file).unwrap().modified().unwrap();
        let (main_before, other_before) = (written(&main), written(&other));
        std::thread::sleep(std::time::Duration::from_millis(50));
        p.workspaces.get_mut("other").unwrap().name = "Renamed".into();
        p.save().unwrap();
        assert_eq!(written(&main), main_before, "the same as before: not written again");
        assert!(written(&other) > other_before, "the changed one is");
        assert!(fs::read_to_string(&other).unwrap().contains("Renamed"));
        // A file that went missing is written again.
        fs::remove_file(&main).unwrap();
        p.save().unwrap();
        assert!(main.is_file());
    }

    /// A workspace named after one of Windows' devices gets a file name that
    /// can be written there, and saves.
    #[test]
    fn workspace_keys_avoid_device_names() {
        let dir = temp("devices");
        let mut p = Project::create(&dir, "Test", "none").unwrap();
        for (name, key) in [("Con", "con-workspace"), ("NUL", "nul-workspace"), ("Aux", "aux-workspace"), ("COM1", "com1-workspace"), ("lpt9", "lpt9-workspace")] {
            assert_eq!(p.new_workspace_key(name), key);
        }
        // Names that only look alike stay as they are.
        for name in ["console", "com", "com10", "comx", "null"] {
            assert_eq!(p.new_workspace_key(name), name);
        }
        let key = p.new_workspace_key("Nul");
        p.workspaces.insert(key.clone(), Workspace::new("Nul"));
        p.save().unwrap();
        assert_eq!(Project::load(&dir).unwrap().workspaces[&key].name, "Nul");
        assert_eq!(p.new_workspace_key("nul"), "nul-workspace-2");
    }
}
