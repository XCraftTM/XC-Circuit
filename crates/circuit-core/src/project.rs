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
#[derive(Clone, Debug, Serialize, Deserialize)]
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
        }
    }
}

#[derive(Clone, Debug)]
pub struct Project {
    pub root: PathBuf,
    pub meta: ProjectMeta,
    /// Keyed by file stem, so `workspaces/rank-command.json` is `rank-command`.
    pub workspaces: IndexMap<String, Workspace>,
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
        let dir = root.join(WORKSPACE_DIR);
        if dir.is_dir() {
            let mut files: Vec<_> = fs::read_dir(&dir)?
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().is_some_and(|e| e == "json"))
                .collect();
            files.sort();
            for path in files {
                let stem = path.file_stem().unwrap_or_default().to_string_lossy().into_owned();
                let ws: Workspace = serde_json::from_str(&fs::read_to_string(&path)?)
                    .with_context(|| format!("parsing {}", path.display()))?;
                workspaces.insert(stem, ws);
            }
        }
        Ok(Project { root, meta, workspaces })
    }

    /// Creates a new project folder with one empty workspace.
    pub fn create(root: impl AsRef<Path>, name: &str, mode: &str) -> Result<Project> {
        let root = root.as_ref().to_path_buf();
        fs::create_dir_all(root.join(WORKSPACE_DIR))?;
        let meta = ProjectMeta { name: name.into(), mode: mode.into(), ..Default::default() };
        let mut workspaces = IndexMap::new();
        workspaces.insert("main".to_string(), Workspace::new("Main"));
        let project = Project { root, meta, workspaces };
        project.save()?;
        // Run output and test data stay out of version control.
        let ignore = project.root.join(".gitignore");
        if !ignore.exists() {
            fs::write(ignore, ".circuit/\n")?;
        }
        Ok(project)
    }

    pub fn save(&self) -> Result<()> {
        fs::create_dir_all(self.root.join(WORKSPACE_DIR))?;
        write_json(&self.root.join(PROJECT_FILE), &self.meta)?;
        for (stem, ws) in &self.workspaces {
            write_json(&self.workspace_path(stem), ws)?;
        }
        Ok(())
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
        if !self.workspaces.contains_key(&base) {
            return base;
        }
        (2..).map(|i| format!("{base}-{i}")).find(|k| !self.workspaces.contains_key(k)).expect("unbounded range")
    }
}

/// Writes pretty JSON through a temporary file, so a crash never leaves half a file.
fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let mut text = serde_json::to_string_pretty(value)?;
    text.push('\n');
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, text).with_context(|| format!("writing {}", tmp.display()))?;
    fs::rename(&tmp, path).with_context(|| format!("replacing {}", path.display()))?;
    Ok(())
}
