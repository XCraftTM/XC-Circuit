//! The project folder (layout 2):
//!
//! ```text
//! My Bot/
//!   circuit.project.json    id, layout, name, mode, plugins, shared settings, folders
//!   commands/<name>.json    one slash command each, with its subcommands
//!   workspaces/<key>.json   one workspace each
//!   .circuit/local.json     what stays on this computer: where the bot is
//!                           uploaded to, database connections, private settings
//! ```
//!
//! In memory a project is whole: [`ProjectMeta::settings`] and
//! [`ProjectMeta::data`] hold the shared and the local parts together, as the
//! editor and the runtime use them. Only [`Project::load`] and
//! [`Project::save`] know which file each part is kept in.
//!
//! Layout 1 (`circuit.json` with everything in it) is read and becomes layout
//! 2 with the next save; its `circuit.json` is then kept in `.circuit/`. Apps
//! from before layout 2 don't check a project's version, so the project file
//! has another name: they find no project instead of damaging one.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::graph::Workspace;

pub const PROJECT_FILE: &str = "circuit.project.json";
/// The project file of layout 1.
pub const OLD_PROJECT_FILE: &str = "circuit.json";
pub const WORKSPACE_DIR: &str = "workspaces";
pub const COMMANDS_DIR: &str = "commands";
/// The folder for what stays on this computer, and its file of settings.
pub const LOCAL_DIR: &str = ".circuit";
pub const LOCAL_FILE: &str = "local.json";
/// The layout this app writes, and the newest it reads.
pub const LAYOUT: u32 = 2;

/// Keys of `data` that aren't kept in the project file.
const SLASH_COMMANDS: &str = "slashCommands";
const HOSTING: &str = "hosting";
/// What of a database connection is shared: which databases there are, by
/// the names blocks use. Where each one is stays on this computer.
const DATABASE_SHARED: [&str; 3] = ["id", "name", "kind"];
const DATABASES: &str = "databases";

/// Whether `dir` holds a project (of this layout or the one before).
pub fn is_project(dir: &Path) -> bool {
    dir.join(PROJECT_FILE).is_file() || dir.join(OLD_PROJECT_FILE).is_file()
}

/// A new project's id: 128 random bits, as hex. Not a secret, only never the
/// same twice.
pub fn new_id() -> String {
    use std::hash::{BuildHasher, Hasher};
    // Each RandomState is seeded by the system; time and process are added
    // for good measure.
    let part = |salt: u64| {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u64(salt);
        h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
        h.write_u32(std::process::id());
        h.finish()
    };
    format!("{:016x}{:016x}", part(1), part(2))
}

/// The id of a project that has none in its file yet (layout 1): made from
/// its folder's path, so it is the same each time until a save writes it.
fn path_id(root: &Path) -> String {
    let path = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let fnv = |seed: u64| path.to_string_lossy().bytes().fold(seed, |h, b| (h ^ b as u64).wrapping_mul(0x0100_0000_01b3));
    format!("{:016x}{:016x}", fnv(0xcbf2_9ce4_8422_2325), fnv(0x8422_2325_cbf2_9ce4))
}

/// The project as a whole (see the module's text). Secrets never go in here.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectMeta {
    /// The version of the project folder's layout (`format` in layout 1).
    #[serde(alias = "format")]
    pub layout: u32,
    /// What tells this project from every other, wherever its folder is:
    /// secrets, backups and what was allowed for it go by this.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub id: String,
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
    /// Slash commands (their first words) in their order, in the project
    /// file only, when it isn't their files' order. In memory the order is
    /// that of `data.slashCommands`.
    #[serde(rename = "commandOrder", skip_serializing_if = "Vec::is_empty")]
    pub command_order: Vec<String>,
}

impl Default for ProjectMeta {
    fn default() -> Self {
        Self {
            layout: LAYOUT,
            id: String::new(),
            name: "New Bot".into(),
            mode: "discord-bot".into(),
            plugins: IndexMap::new(),
            settings: Map::new(),
            data: Map::new(),
            folders: Vec::new(),
            order: Vec::new(),
            command_order: Vec::new(),
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
    /// Settings that are private: kept in `.circuit/local.json`, not in the
    /// project file. The editor names them (plugins say which of theirs are);
    /// those read from local.json stay there whatever this says.
    pub private_settings: Vec<String>,
    /// The layout the project was read from, when it was an older one: it is
    /// brought to the current layout with the next save.
    pub converted_from: Option<u32>,
    /// Command files that couldn't be read: never overwritten or deleted.
    pub unreadable_commands: Vec<(String, String)>,
    /// Settings that came from local.json.
    local_settings: Vec<String>,
}

/// `.circuit/local.json`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
struct Local {
    /// Where Bot › Host… uploads the bot.
    #[serde(skip_serializing_if = "Option::is_none")]
    hosting: Option<Value>,
    /// Private settings, by key.
    #[serde(skip_serializing_if = "Map::is_empty")]
    settings: Map<String, Value>,
    /// Where each database is, by its id: everything but its id, name and kind.
    #[serde(skip_serializing_if = "Map::is_empty")]
    databases: Map<String, Value>,
}

/// The first word of a command's name: `ticket` of `ticket open`.
fn command_root(entry: &Value) -> String {
    entry.get("name").and_then(Value::as_str).unwrap_or_default().split_whitespace().next().unwrap_or_default().to_string()
}

/// A file name (without its ending) for a command: its name, as far as a
/// file can be called that.
fn command_stem(root: &str) -> String {
    let stem: String = root.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' }).collect();
    let stem = stem.trim_matches('-').to_string();
    if stem.is_empty() {
        "command".to_string()
    } else if is_device_name(&stem) {
        format!("{stem}-command")
    } else {
        stem
    }
}

fn json_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(dir).into_iter().flatten().filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|e| e == "json")).collect();
    files.sort();
    files
}

impl Project {
    pub fn load(root: impl AsRef<Path>) -> Result<Project> {
        let root = root.as_ref().to_path_buf();
        // This layout's file; without one, the file of the layout before.
        let old = !root.join(PROJECT_FILE).is_file() && root.join(OLD_PROJECT_FILE).is_file();
        let meta_path = root.join(if old { OLD_PROJECT_FILE } else { PROJECT_FILE });
        let mut meta: ProjectMeta = serde_json::from_str(
            &fs::read_to_string(&meta_path).with_context(|| format!("reading {}", meta_path.display()))?,
        )
        .with_context(|| format!("parsing {}", meta_path.display()))?;
        anyhow::ensure!(
            meta.layout <= LAYOUT,
            "this project was saved by a newer XC Circuit (project layout {}; this version reads up to {LAYOUT}). Update XC Circuit to open it.",
            meta.layout
        );
        let mut converted_from = None;
        let mut local_settings = Vec::new();
        let mut unreadable_commands = Vec::new();
        if old {
            // Layout 1 had everything in its one file: it is whole already.
            converted_from = Some(meta.layout.max(1));
            meta.layout = LAYOUT;
        } else {
            unreadable_commands = read_commands(&root, &mut meta);
            local_settings = read_local(&root, &mut meta);
        }
        if meta.id.is_empty() {
            meta.id = path_id(&root);
        }
        // No commands is the same as none listed.
        if meta.data.get(SLASH_COMMANDS).and_then(Value::as_array).is_some_and(Vec::is_empty) {
            meta.data.remove(SLASH_COMMANDS);
        }

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
        Ok(Project { root, meta, workspaces: ordered, unreadable, removed: Vec::new(), private_settings: Vec::new(), converted_from, unreadable_commands, local_settings })
    }

    /// Whether setting `key` is kept on this computer only.
    fn is_private(&self, key: &str) -> bool {
        self.private_settings.iter().chain(&self.local_settings).any(|k| k == key)
    }

    /// What goes into the project file, and what into local.json.
    fn split(&self) -> (ProjectMeta, Vec<Value>, Local) {
        let mut shared = self.meta.clone();
        shared.layout = LAYOUT;
        let mut local = Local::default();
        for (key, value) in &self.meta.settings {
            if self.is_private(key) {
                shared.settings.remove(key);
                local.settings.insert(key.clone(), value.clone());
            }
        }
        // Databases: which there are is shared, where they are is not.
        if let Some(Value::Array(list)) = self.meta.settings.get(DATABASES) {
            let mut names = Vec::new();
            for db in list {
                let Some(fields) = db.as_object() else { continue };
                let id = fields.get("id").and_then(Value::as_str).unwrap_or_default().to_string();
                let (keep, here): (Map<String, Value>, Map<String, Value>) = fields.clone().into_iter().partition(|(k, _)| DATABASE_SHARED.contains(&k.as_str()));
                names.push(Value::Object(keep));
                if !id.is_empty() && !here.is_empty() {
                    local.databases.insert(id, Value::Object(here));
                }
            }
            shared.settings.insert(DATABASES.into(), Value::Array(names));
        }
        local.hosting = shared.data.remove(HOSTING);
        let commands = match shared.data.remove(SLASH_COMMANDS) {
            Some(Value::Array(list)) => list,
            _ => Vec::new(),
        };
        // Their order, when it isn't the files' own (by name).
        let roots = command_roots(&commands);
        let mut by_file = roots.clone();
        by_file.sort_by_key(|r| command_stem(r));
        shared.command_order = if roots == by_file { Vec::new() } else { roots };
        (shared, commands, local)
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
        let meta = ProjectMeta { id: new_id(), ..meta };
        let mut project = Project { root, meta, workspaces, unreadable: Vec::new(), removed: Vec::new(), private_settings: Vec::new(), converted_from: None, unreadable_commands: Vec::new(), local_settings: Vec::new() };
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
        self.meta.layout = LAYOUT;
        let (shared, commands, local) = self.split();
        write_json(&self.root.join(PROJECT_FILE), &shared)?;
        write_commands(&self.root, &commands, &self.unreadable_commands)?;
        write_local(&self.root, &local)?;
        // A project of the layout before: its file is kept, out of the way,
        // so an older app no longer opens (and damages) the folder.
        if self.root.join(OLD_PROJECT_FILE).is_file() {
            let keep = self.root.join(LOCAL_DIR).join("layout-1");
            fs::create_dir_all(&keep)?;
            fs::rename(self.root.join(OLD_PROJECT_FILE), keep.join(OLD_PROJECT_FILE)).with_context(|| format!("moving {OLD_PROJECT_FILE} to {}", keep.display()))?;
        }
        self.converted_from = None;
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

/// The commands' first words, in their order, each once.
fn command_roots(commands: &[Value]) -> Vec<String> {
    let mut roots: Vec<String> = Vec::new();
    for entry in commands {
        let root = command_root(entry);
        if !roots.contains(&root) {
            roots.push(root);
        }
    }
    roots
}

/// Writes each command (its entries: itself, or its subcommands and groups)
/// to a file of its own, and removes the files of commands that are gone.
fn write_commands(root: &Path, commands: &[Value], unreadable: &[(String, String)]) -> Result<()> {
    let dir = root.join(COMMANDS_DIR);
    let mut written: Vec<String> = Vec::new();
    for name in command_roots(commands) {
        let entries: Vec<&Value> = commands.iter().filter(|e| command_root(e) == name).collect();
        // Two names that make the same file name (or one a damaged file has) get a number.
        let base = command_stem(&name);
        let taken = |stem: &str, written: &[String]| written.iter().chain(unreadable.iter().map(|(s, _)| s)).any(|w| w.eq_ignore_ascii_case(stem));
        let stem = if taken(&base, &written) { (2..).map(|i| format!("{base}-{i}")).find(|s| !taken(s, &written)).expect("unbounded range") } else { base };
        fs::create_dir_all(&dir)?;
        write_json(&dir.join(format!("{stem}.json")), &entries)?;
        written.push(stem);
    }
    for file in json_files(&dir) {
        let stem = file.file_stem().unwrap_or_default().to_string_lossy().into_owned();
        let keep = written.iter().chain(unreadable.iter().map(|(s, _)| s)).any(|w| w.eq_ignore_ascii_case(&stem));
        if !keep {
            fs::remove_file(&file).with_context(|| format!("deleting {}", file.display()))?;
        }
    }
    Ok(())
}

/// Reads `commands/*.json` into `data.slashCommands`, in the project file's
/// order (then by file name). Gives the files that couldn't be read.
fn read_commands(root: &Path, meta: &mut ProjectMeta) -> Vec<(String, String)> {
    let order = std::mem::take(&mut meta.command_order);
    let mut found: Vec<(String, Vec<Value>)> = Vec::new();
    let mut unreadable = Vec::new();
    for file in json_files(&root.join(COMMANDS_DIR)) {
        let stem = file.file_stem().unwrap_or_default().to_string_lossy().into_owned();
        let read = fs::read_to_string(&file).map_err(anyhow::Error::from).and_then(|text| Ok(serde_json::from_str::<Vec<Value>>(&text)?));
        match read {
            Ok(entries) if !entries.is_empty() => found.push((command_root(&entries[0]), entries)),
            Ok(_) => {}
            Err(e) => unreadable.push((stem, format!("{e:#}"))),
        }
    }
    let mut commands: Vec<Value> = Vec::new();
    for name in &order {
        if let Some(i) = found.iter().position(|(root, _)| root == name) {
            commands.extend(found.remove(i).1);
        }
    }
    commands.extend(found.into_iter().flat_map(|(_, entries)| entries));
    if !commands.is_empty() {
        meta.data.insert(SLASH_COMMANDS.into(), Value::Array(commands));
    }
    unreadable
}

/// Writes `.circuit/local.json`; with nothing to keep there, the file goes.
fn write_local(root: &Path, local: &Local) -> Result<()> {
    let path = root.join(LOCAL_DIR).join(LOCAL_FILE);
    if *local == Local::default() {
        return match fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e).with_context(|| format!("deleting {}", path.display())),
            _ => Ok(()),
        };
    }
    fs::create_dir_all(root.join(LOCAL_DIR))?;
    write_json(&path, local)
}

/// Puts what `.circuit/local.json` holds back into the project: private
/// settings, where the databases are, and where the bot is uploaded to.
/// Gives the keys of the settings it held. A file that can't be read is left
/// as it is: the project opens without its local part.
fn read_local(root: &Path, meta: &mut ProjectMeta) -> Vec<String> {
    let path = root.join(LOCAL_DIR).join(LOCAL_FILE);
    let Some(local) = fs::read_to_string(&path).ok().and_then(|text| serde_json::from_str::<Local>(&text).ok()) else { return Vec::new() };
    let keys: Vec<String> = local.settings.keys().cloned().collect();
    meta.settings.extend(local.settings);
    if let Some(Value::Array(list)) = meta.settings.get_mut(DATABASES) {
        for db in list {
            let id = db.get("id").and_then(Value::as_str).unwrap_or_default().to_string();
            if let (Some(fields), Some(Value::Object(here))) = (db.as_object_mut(), local.databases.get(&id)) {
                fields.extend(here.clone());
            }
        }
    }
    if let Some(hosting) = local.hosting {
        meta.data.insert(HOSTING.into(), hosting);
    }
    keys
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

    /// A project of layout 2: what is shared is in circuit.project.json and
    /// commands/, what is this computer's in .circuit/local.json; read back,
    /// it is whole again.
    #[test]
    fn each_part_is_kept_in_its_file() {
        use serde_json::json;
        let dir = temp("layout");
        let mut p = Project::create(&dir, "Test", "discord-bot").unwrap();
        let id = p.meta.id.clone();
        assert_eq!(id.len(), 32);
        assert!(dir.join(PROJECT_FILE).is_file() && !dir.join(OLD_PROJECT_FILE).exists() && is_project(&dir));

        p.meta.settings.insert("prefix".into(), json!("?"));
        p.meta.settings.insert("devServer".into(), json!("123456789012345678"));
        p.private_settings = vec!["devServer".into()];
        p.meta.settings.insert("storeIn".into(), json!("main"));
        p.meta.settings.insert("databases".into(), json!([{ "id": "main", "name": "Main", "kind": "postgres", "host": "db.example", "port": 5432, "user": "bot" }, { "id": "files", "name": "Files", "kind": "sqlite", "file": "bot.sqlite" }]));
        p.meta.data.insert("hosting".into(), json!({ "kind": "sftp", "host": "203.0.113.7", "user": "erik" }));
        p.meta.data.insert("slashFolders".into(), json!(["Fun"]));
        let commands = json!([
            { "name": "ticket open", "description": "Open" },
            { "name": "rank", "description": "Rank", "folder": "Fun" },
            { "name": "ticket close", "description": "Close" },
            { "name": "Con", "description": "odd" }
        ]);
        p.meta.data.insert("slashCommands".into(), commands);
        p.save().unwrap();

        let file = fs::read_to_string(dir.join(PROJECT_FILE)).unwrap();
        for hidden in ["203.0.113.7", "db.example", "123456789012345678", "ticket open", "bot.sqlite"] {
            assert!(!file.contains(hidden), "{hidden} isn't in the project file:\n{file}");
        }
        let shared: Value = serde_json::from_str(&file).unwrap();
        assert_eq!((shared["layout"].clone(), shared["id"].clone()), (json!(2), json!(id)));
        assert_eq!(shared["settings"], json!({ "prefix": "?", "storeIn": "main", "databases": [{ "id": "main", "name": "Main", "kind": "postgres" }, { "id": "files", "name": "Files", "kind": "sqlite" }] }));
        assert_eq!(shared["data"], json!({ "slashFolders": ["Fun"] }));
        assert_eq!(shared["commandOrder"], json!(["ticket", "rank", "Con"]));
        let names: Vec<String> = json_files(&dir.join(COMMANDS_DIR)).iter().map(|f| f.file_name().unwrap().to_string_lossy().into_owned()).collect();
        assert_eq!(names, ["con-command.json", "rank.json", "ticket.json"]);
        let ticket: Value = serde_json::from_str(&fs::read_to_string(dir.join(COMMANDS_DIR).join("ticket.json")).unwrap()).unwrap();
        assert_eq!(ticket, json!([{ "name": "ticket open", "description": "Open" }, { "name": "ticket close", "description": "Close" }]));
        let local: Value = serde_json::from_str(&fs::read_to_string(dir.join(LOCAL_DIR).join(LOCAL_FILE)).unwrap()).unwrap();
        assert_eq!(local["hosting"]["host"], "203.0.113.7");
        assert_eq!(local["settings"], json!({ "devServer": "123456789012345678" }));
        assert_eq!(local["databases"], json!({ "main": { "host": "db.example", "port": 5432, "user": "bot" }, "files": { "file": "bot.sqlite" } }));

        // Whole again when read; a command's entries stand together, in the saved order.
        let mut again = Project::load(&dir).unwrap();
        assert_eq!(again.meta.id, id);
        assert_eq!(again.meta.settings, p.meta.settings);
        assert_eq!(again.meta.data["hosting"], p.meta.data["hosting"]);
        let read: Vec<&str> = again.meta.data["slashCommands"].as_array().unwrap().iter().map(|c| c["name"].as_str().unwrap()).collect();
        assert_eq!(read, ["ticket open", "ticket close", "rank", "Con"]);
        // What came from local.json stays there, also when nobody says it is private.
        again.save().unwrap();
        assert!(!fs::read_to_string(dir.join(PROJECT_FILE)).unwrap().contains("123456789012345678"));

        // A command that is gone loses its file; one that can't be read keeps it.
        fs::write(dir.join(COMMANDS_DIR).join("broken.json"), "{ nope").unwrap();
        let mut again = Project::load(&dir).unwrap();
        assert_eq!(again.unreadable_commands.len(), 1);
        again.meta.data.insert("slashCommands".into(), json!([{ "name": "rank", "description": "Rank" }]));
        again.meta.data.remove("hosting");
        again.save().unwrap();
        let names: Vec<String> = json_files(&dir.join(COMMANDS_DIR)).iter().map(|f| f.file_name().unwrap().to_string_lossy().into_owned()).collect();
        assert_eq!(names, ["broken.json", "rank.json"]);
        assert!(!fs::read_to_string(dir.join(LOCAL_DIR).join(LOCAL_FILE)).unwrap().contains("hosting"));
    }

    /// A project of layout 1 opens as it is and becomes layout 2 with the
    /// next save, its old file kept out of an older app's way. A project of a
    /// newer layout than this app knows isn't opened.
    #[test]
    fn layout_1_is_converted_and_newer_ones_refused() {
        use serde_json::json;
        let dir = temp("convert");
        fs::create_dir_all(dir.join(WORKSPACE_DIR)).unwrap();
        let old = json!({
            "format": 1, "name": "Old Bot", "mode": "discord-bot", "plugins": { "xc.core": "1.0.0" },
            "settings": { "prefix": "!", "databases": [{ "id": "main", "name": "Main", "kind": "mariadb", "host": "10.0.0.5" }] },
            "data": { "slashCommands": [{ "name": "ping", "description": "Pong" }], "hosting": { "kind": "ftp", "host": "ftp.example" } },
            "folders": ["Events"]
        });
        fs::write(dir.join(OLD_PROJECT_FILE), serde_json::to_string_pretty(&old).unwrap()).unwrap();
        fs::write(dir.join(WORKSPACE_DIR).join("main.json"), r#"{ "name": "Main", "nodes": {}, "links": [] }"#).unwrap();

        let mut p = Project::load(&dir).unwrap();
        assert_eq!((p.converted_from, p.meta.layout, p.meta.name.as_str()), (Some(1), 2, "Old Bot"));
        assert_eq!(p.meta.id.len(), 32);
        assert_eq!(Project::load(&dir).unwrap().meta.id, p.meta.id, "the same id each time, before it is saved");
        assert_eq!(p.meta.data["slashCommands"][0]["name"], "ping");
        assert!(dir.join(OLD_PROJECT_FILE).is_file() && !dir.join(PROJECT_FILE).exists(), "nothing is written by opening it");

        p.save().unwrap();
        assert!(p.converted_from.is_none());
        assert!(!dir.join(OLD_PROJECT_FILE).exists(), "an older app finds no project here now");
        assert_eq!(serde_json::from_str::<Value>(&fs::read_to_string(dir.join(LOCAL_DIR).join("layout-1").join(OLD_PROJECT_FILE)).unwrap()).unwrap(), old, "the old file is kept");
        assert!(dir.join(COMMANDS_DIR).join("ping.json").is_file());
        let again = Project::load(&dir).unwrap();
        assert_eq!(again.converted_from, None);
        assert_eq!(again.meta, p.meta, "the same project, with the same id");
        assert_eq!(again.meta.folders, ["Events"]);
        assert_eq!(again.meta.settings["databases"][0]["host"], "10.0.0.5");

        // A layout from the future.
        let file = dir.join(PROJECT_FILE);
        let newer = fs::read_to_string(&file).unwrap().replace("\"layout\": 2", "\"layout\": 3");
        fs::write(&file, &newer).unwrap();
        let refused = Project::load(&dir).unwrap_err().to_string();
        assert!(refused.contains("newer XC Circuit") && refused.contains("layout 3"), "{refused}");
        assert_eq!(fs::read_to_string(&file).unwrap(), newer, "and left as it is");
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
