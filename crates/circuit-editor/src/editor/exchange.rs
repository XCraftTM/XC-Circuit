//! Workspaces as files to share: some or all of a project's workspaces, with
//! the slash commands they use, exported to an .xcws.json file and imported
//! into another project. Both show a summary first, where parts can be left out.

use std::collections::HashSet;
use std::path::PathBuf;

use circuit_core::{Project, Workspace};
use eframe::egui::{self, RichText, Ui};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::ui::dialog::{self, Part};
use crate::editor::{Action, Editor, Level};
use crate::ui::theme::Theme;

/// What the file says it is; files of another major format are refused.
pub const FORMAT: &str = "xc-circuit-workspaces/1";
/// File names end in .xcws.json: JSON to every program, workspaces to us.
pub const EXTENSION: &str = "xcws.json";

/// The block that runs a slash command, whose `command` names it.
const SLASH_EVENT: &str = "discord.on_slash_command";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspacesFile {
    pub format: String,
    /// The project they come from.
    #[serde(default)]
    pub project: String,
    #[serde(default)]
    pub app_version: String,
    /// When, as an ISO date.
    #[serde(default)]
    pub exported_at: String,
    pub workspaces: Vec<WorkspaceEntry>,
    /// Slash Command Builder entries the workspaces use.
    #[serde(default)]
    pub slash_commands: Vec<Value>,
    /// The plugins their blocks come from: id → version.
    #[serde(default)]
    pub plugins: IndexMap<String, String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkspaceEntry {
    /// Its file name in the project it came from.
    pub key: String,
    pub workspace: Workspace,
}

/// The project's Slash Command Builder entries.
fn slash_commands(project: &Project) -> Vec<Value> {
    project.meta.data.get(crate::builders::slash::DATA_KEY).and_then(Value::as_array).cloned().unwrap_or_default()
}

/// A command entry's first word: `ticket open` is part of `ticket`.
fn command_root(entry: &Value) -> String {
    entry.get("name").and_then(Value::as_str).unwrap_or("").split_whitespace().next().unwrap_or("").to_lowercase()
}

/// The commands (first words) a workspace's Slash Command blocks run.
fn commands_used(ws: &Workspace) -> HashSet<String> {
    ws.nodes
        .values()
        .filter(|n| n.block_id() == SLASH_EVENT)
        .filter_map(|n| n.values.get("command").and_then(Value::as_str))
        .filter_map(|c| c.trim().trim_start_matches('/').split_whitespace().next().map(str::to_lowercase))
        .filter(|c| !c.is_empty())
        .collect()
}

/// The file for the workspaces `keys` of a project, with the slash commands
/// (by first word) in `commands`.
pub fn build_file(project: &Project, reg: &circuit_plugins::Registry, keys: &[String], commands: &HashSet<String>) -> WorkspacesFile {
    let workspaces: Vec<WorkspaceEntry> = keys.iter().filter_map(|k| project.workspaces.get(k).map(|ws| WorkspaceEntry { key: k.clone(), workspace: ws.clone() })).collect();
    let mut plugins = IndexMap::new();
    for e in &workspaces {
        for node in e.workspace.nodes.values() {
            if let Some(def) = reg.block(&node.block)
                && let Some(p) = reg.plugins.iter().find(|p| p.manifest.id == def.plugin)
            {
                plugins.insert(p.manifest.id.clone(), p.manifest.version.clone());
            }
        }
    }
    plugins.sort_keys();
    WorkspacesFile {
        format: FORMAT.into(),
        project: project.meta.name.clone(),
        app_version: env!("CARGO_PKG_VERSION").into(),
        exported_at: chrono::Local::now().to_rfc3339(),
        workspaces,
        slash_commands: slash_commands(project).into_iter().filter(|c| commands.contains(&command_root(c))).collect(),
        plugins,
    }
}

/// Reads a workspaces file.
pub fn read_file(text: &str) -> anyhow::Result<WorkspacesFile> {
    let file: WorkspacesFile = serde_json::from_str(text).map_err(|e| anyhow::anyhow!("not a workspaces file: {e}"))?;
    let (kind, major) = file.format.split_once('/').unwrap_or((&file.format, ""));
    anyhow::ensure!(kind == "xc-circuit-workspaces", "not a workspaces file");
    anyhow::ensure!(major == "1", "made by a newer XC Circuit (format {}); update to import it", file.format);
    Ok(file)
}

// ---- Export --------------------------------------------------------------

/// The Export Workspaces tab: which workspaces and slash commands go in.
pub struct ExportDraft {
    /// Every workspace (in sidebar order) and whether it goes in.
    pub chosen: IndexMap<String, bool>,
    /// Slash commands (first words) left out, though a chosen workspace uses them.
    pub skipped_commands: HashSet<String>,
}

/// What the export starts with chosen.
pub enum Preselect {
    All,
    Folder(String),
    Workspace(String),
}

/// Opens the Export Workspaces tab.
pub fn open_export(ed: &mut Editor, preselect: Preselect) {
    let Some(project) = &ed.project else { return };
    let chosen = project
        .workspaces
        .iter()
        .map(|(k, ws)| {
            let on = match &preselect {
                Preselect::All => true,
                Preselect::Folder(f) => ws.folder.as_ref() == Some(f),
                Preselect::Workspace(key) => key == k,
            };
            (k.clone(), on)
        })
        .collect();
    ed.ws_export = Some(ExportDraft { chosen, skipped_commands: HashSet::new() });
}

/// The slash command entries the chosen workspaces use: (first word, entry).
fn used_commands(project: &Project, draft: &ExportDraft) -> Vec<(String, Value)> {
    let used: HashSet<String> = draft.chosen.iter().filter(|(_, on)| **on).filter_map(|(k, _)| project.workspaces.get(k)).flat_map(commands_used).collect();
    let mut out: Vec<(String, Value)> = slash_commands(project).into_iter().map(|c| (command_root(&c), c)).filter(|(root, _)| used.contains(root)).collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

pub fn export_tab(ui: &mut Ui, ed: &mut Editor) {
    let theme = ed.theme.clone();
    let icon = ed.reg.icons.get("workspace").cloned();
    let (Some(project), Some(draft)) = (ed.project.as_ref(), ed.ws_export.as_mut()) else { return };
    let (mut go, mut cancel) = (false, false);
    let commands = used_commands(project, draft);
    dialog::tab(
        ui,
        &theme,
        dialog::Spec {
            id: "export-workspaces",
            title: t!("Export workspaces"),
            subtitle: Some(t!("As one .xcws.json file, to share or to import into another project")),
            icon: icon.as_deref(),
            width: 620.0,
        },
        |ui, part| match part {
            Part::Body => {
                dialog::section(ui, &theme, t!("Workspaces"));
                ui.horizontal(|ui| {
                    if dialog::button(ui, &theme, t!("All")).clicked() {
                        draft.chosen.values_mut().for_each(|on| *on = true);
                    }
                    if dialog::button(ui, &theme, t!("None")).clicked() {
                        draft.chosen.values_mut().for_each(|on| *on = false);
                    }
                });
                workspace_checklist(ui, &theme, project, &mut draft.chosen);

                if !commands.is_empty() {
                    dialog::section(ui, &theme, t!("Slash commands"));
                    ui.label(RichText::new(t!("The chosen workspaces use these. They come along, so the commands work in the other project too.")).size(11.5).color(theme.text_muted));
                    for (root, entry) in &commands {
                        let mut on = !draft.skipped_commands.contains(root);
                        let text = format!("/{}", entry.get("name").and_then(Value::as_str).unwrap_or(root));
                        if ui.checkbox(&mut on, RichText::new(text).monospace()).changed() {
                            if on {
                                draft.skipped_commands.remove(root);
                            } else {
                                draft.skipped_commands.insert(root.clone());
                            }
                        }
                    }
                }

                dialog::section(ui, &theme, t!("Summary"));
                let keys: Vec<&String> = draft.chosen.iter().filter(|(_, on)| **on).map(|(k, _)| k).collect();
                let wss: Vec<&Workspace> = keys.iter().filter_map(|k| project.workspaces.get(*k)).collect();
                let blocks: usize = wss.iter().map(|w| w.nodes.len()).sum();
                let mut folders: Vec<&str> = wss.iter().filter_map(|w| w.folder.as_deref()).collect();
                folders.sort();
                folders.dedup();
                let sending = commands.iter().filter(|(r, _)| !draft.skipped_commands.contains(r)).count();
                let mut plugins: Vec<String> = Vec::new();
                for w in &wss {
                    for n in w.nodes.values() {
                        if let Some(p) = ed.reg.block(&n.block).and_then(|d| ed.reg.plugins.iter().find(|p| p.manifest.id == d.plugin))
                            && !plugins.contains(&p.manifest.name)
                        {
                            plugins.push(p.manifest.name.clone());
                        }
                    }
                }
                summary(ui, &theme, |ui| {
                    summary_line(ui, &theme, &tf!("{workspaces} with {blocks}", workspaces = workspaces_text(wss.len()), blocks = blocks_text(blocks)));
                    if !folders.is_empty() {
                        summary_line(ui, &theme, &tf!("In the folders {names}", names = folders.join(", ")));
                    }
                    summary_line(ui, &theme, &commands_text(sending));
                    if !plugins.is_empty() {
                        summary_line(ui, &theme, &tf!("Needs the plugins {names}", names = plugins.join(", ")));
                    }
                    ui.add_space(4.0);
                    ui.label(RichText::new(t!("Not included: the bot token and other secrets, the bot settings and the bot's saved data.")).size(11.5).color(theme.text_muted));
                });
            }
            Part::Footer => {
                let any = draft.chosen.values().any(|on| *on);
                ui.add_enabled_ui(any, |ui| go = dialog::primary_button(ui, &theme, t!("Export…")).clicked());
                cancel = dialog::secondary_button(ui, &theme, t!("Cancel")).clicked();
            }
        },
    );
    if cancel {
        ed.ws_export = None;
    } else if go {
        run_export(ed);
    }
}

/// Asks where to save, then writes the file.
fn run_export(ed: &mut Editor) {
    let (Some(project), Some(draft)) = (ed.project.as_ref(), ed.ws_export.as_ref()) else { return };
    let keys: Vec<String> = draft.chosen.iter().filter(|(_, on)| **on).map(|(k, _)| k.clone()).collect();
    let commands: HashSet<String> = used_commands(project, draft).into_iter().map(|(r, _)| r).filter(|r| !draft.skipped_commands.contains(r)).collect();
    let base = if keys.len() == 1 { project.workspaces[&keys[0]].name.clone() } else { project.meta.name.clone() };
    let name: String = base.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' { c } else { '-' }).collect();
    let Some(mut path) = rfd::FileDialog::new()
        .set_title(t!("Export workspaces"))
        .set_file_name(format!("{}.{EXTENSION}", name.trim()))
        .add_filter(t!("XC Circuit workspaces"), &["json", "xcws"])
        .save_file()
    else {
        return;
    };
    if !path.to_string_lossy().ends_with(".json") && !path.to_string_lossy().ends_with(".xcws") {
        path.set_extension(EXTENSION);
    }
    let file = build_file(project, &ed.reg, &keys, &commands);
    let result = serde_json::to_string_pretty(&file).map_err(anyhow::Error::from).and_then(|text| Ok(std::fs::write(&path, text + "\n")?));
    match result {
        Ok(()) => {
            ed.log(Level::Success, tf!("Exported {workspaces} to {path}", workspaces = workspaces_text(keys.len()), path = path.display()));
            ed.ws_export = None;
        }
        Err(e) => ed.log(Level::Error, format!("Couldn't export to {}: {e:#}", path.display())),
    }
}

/// Checkboxes for workspaces, under their folders (a folder's box ticks all of it).
fn workspace_checklist(ui: &mut Ui, theme: &Theme, project: &Project, chosen: &mut IndexMap<String, bool>) {
    let mut folders: Vec<Option<String>> = project.meta.folders.iter().cloned().map(Some).collect();
    for ws in project.workspaces.values() {
        if !folders.contains(&ws.folder) {
            folders.push(ws.folder.clone());
        }
    }
    folders.sort_by_key(|f| f.is_none());
    egui::Frame::new().fill(theme.field).corner_radius(8).inner_margin(egui::Margin::symmetric(12, 8)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        for folder in folders {
            let keys: Vec<String> = project.workspaces.iter().filter(|(_, w)| w.folder == folder).map(|(k, _)| k.clone()).collect();
            if keys.is_empty() {
                continue;
            }
            let indent = if let Some(name) = &folder {
                let on = keys.iter().filter(|k| chosen.get(*k).copied().unwrap_or(false)).count();
                let mut all = on == keys.len();
                if ui.add(egui::Checkbox::new(&mut all, RichText::new(name).strong()).indeterminate(on > 0 && on < keys.len())).changed() {
                    for k in &keys {
                        chosen.insert(k.clone(), all);
                    }
                }
                20.0
            } else {
                0.0
            };
            for k in &keys {
                let ws = &project.workspaces[k];
                ui.horizontal(|ui| {
                    ui.add_space(indent);
                    let on = chosen.entry(k.clone()).or_insert(false);
                    ui.checkbox(on, &ws.name);
                    ui.label(RichText::new(blocks_text(ws.nodes.len())).size(11.5).color(theme.text_muted));
                });
            }
        }
    });
}

fn workspaces_text(n: usize) -> String {
    if n == 1 { t!("1 workspace").to_string() } else { tf!("{count} workspaces", count = n) }
}

fn blocks_text(n: usize) -> String {
    if n == 1 { t!("1 block").to_string() } else { tf!("{count} blocks", count = n) }
}

fn commands_text(n: usize) -> String {
    if n == 1 { t!("1 slash command").to_string() } else { tf!("{count} slash commands", count = n) }
}

fn summary(ui: &mut Ui, theme: &Theme, add: impl FnOnce(&mut Ui)) {
    egui::Frame::new().fill(theme.field).corner_radius(8).inner_margin(egui::Margin::symmetric(12, 10)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        add(ui);
    });
}

fn summary_line(ui: &mut Ui, theme: &Theme, text: &str) {
    ui.label(RichText::new(format!("•  {text}")).size(12.5).color(theme.text));
}

// ---- Import --------------------------------------------------------------

/// The Import Workspaces tab: what the chosen files hold and what goes in.
pub struct ImportDraft {
    /// The files read: path and contents.
    pub files: Vec<(PathBuf, WorkspacesFile)>,
    pub items: Vec<ImportItem>,
    pub commands: Vec<ImportCommand>,
    /// Plugins the files need that aren't installed: name and version.
    pub missing_plugins: Vec<(String, String)>,
}

pub struct ImportItem {
    /// Which file, and which workspace in it.
    pub file: usize,
    pub index: usize,
    pub chosen: bool,
}

pub struct ImportCommand {
    pub entry: Value,
    /// The project has a command with this first word, which is kept.
    pub exists: bool,
    pub chosen: bool,
}

/// Lets the user pick files, reads them and opens the Import Workspaces tab.
pub fn pick_import(ed: &mut Editor) {
    let Some(paths) = rfd::FileDialog::new().set_title(t!("Import workspaces")).add_filter(t!("XC Circuit workspaces"), &["json", "xcws"]).pick_files() else { return };
    let mut files = Vec::new();
    for path in paths {
        match std::fs::read_to_string(&path).map_err(anyhow::Error::from).and_then(|t| read_file(&t)) {
            Ok(f) => files.push((path, f)),
            Err(e) => ed.log(Level::Error, format!("Couldn't import {}: {e:#}", path.display())),
        }
    }
    if !files.is_empty() {
        open_import(ed, files);
    }
}

/// Opens the Import Workspaces tab for files already read.
pub fn open_import(ed: &mut Editor, files: Vec<(PathBuf, WorkspacesFile)>) {
    let Some(project) = &ed.project else { return };
    let existing: HashSet<String> = slash_commands(project).iter().map(command_root).collect();
    let mut items = Vec::new();
    let mut commands: Vec<ImportCommand> = Vec::new();
    let mut missing_plugins: Vec<(String, String)> = Vec::new();
    for (i, (_, f)) in files.iter().enumerate() {
        items.extend((0..f.workspaces.len()).map(|index| ImportItem { file: i, index, chosen: true }));
        for c in &f.slash_commands {
            let root = command_root(c);
            if commands.iter().any(|x| command_root(&x.entry) == root) {
                continue;
            }
            let exists = existing.contains(&root);
            commands.push(ImportCommand { entry: c.clone(), exists, chosen: !exists });
        }
        for (id, version) in &f.plugins {
            if !ed.reg.plugins.iter().any(|p| p.manifest.id == *id) && !missing_plugins.iter().any(|(m, _)| m == id) {
                missing_plugins.push((id.clone(), version.clone()));
            }
        }
    }
    ed.ws_import = Some(ImportDraft { files, items, commands, missing_plugins });
}

/// Names for the chosen items: their own, or with (2), (3) … when the project
/// or an earlier item has the name. None for items left out.
fn planned_names(project: &Project, draft: &ImportDraft) -> Vec<Option<String>> {
    let mut taken: HashSet<String> = project.workspaces.values().map(|w| w.name.to_lowercase()).collect();
    draft
        .items
        .iter()
        .map(|item| {
            if !item.chosen {
                return None;
            }
            let name = draft.files[item.file].1.workspaces[item.index].workspace.name.clone();
            let mut candidate = name.clone();
            let mut n = 2;
            while taken.contains(&candidate.to_lowercase()) {
                candidate = format!("{name} ({n})");
                n += 1;
            }
            taken.insert(candidate.to_lowercase());
            Some(candidate)
        })
        .collect()
}

/// Puts the chosen workspaces and commands into the project. Returns the new
/// workspaces' keys.
pub fn apply_import(project: &mut Project, draft: &ImportDraft) -> Vec<String> {
    let names = planned_names(project, draft);
    let mut added = Vec::new();
    for (item, name) in draft.items.iter().zip(names) {
        let Some(name) = name else { continue };
        let mut ws = draft.files[item.file].1.workspaces[item.index].workspace.clone();
        ws.name = name.clone();
        if let Some(f) = &ws.folder
            && !project.meta.folders.contains(f)
        {
            project.meta.folders.push(f.clone());
        }
        let key = project.new_workspace_key(&name);
        project.removed.retain(|r| *r != key);
        project.workspaces.insert(key.clone(), ws);
        added.push(key);
    }
    let new_commands: Vec<Value> = draft.commands.iter().filter(|c| c.chosen && !c.exists).map(|c| c.entry.clone()).collect();
    if !new_commands.is_empty() {
        let list = project.meta.data.entry(crate::builders::slash::DATA_KEY).or_insert_with(|| Value::Array(Vec::new()));
        if let Value::Array(items) = list {
            items.extend(new_commands);
        }
    }
    added
}

pub fn import_tab(ui: &mut Ui, ed: &mut Editor) {
    let theme = ed.theme.clone();
    let icon = ed.reg.icons.get("workspace").cloned();
    let (Some(project), Some(draft)) = (ed.project.as_ref(), ed.ws_import.as_mut()) else { return };
    let (mut go, mut cancel) = (false, false);
    let names = planned_names(project, draft);
    dialog::tab(
        ui,
        &theme,
        dialog::Spec { id: "import-workspaces", title: t!("Import workspaces"), subtitle: Some(t!("Into this project, as unsaved changes")), icon: icon.as_deref(), width: 620.0 },
        |ui, part| match part {
            Part::Body => {
                dialog::section(ui, &theme, t!("From"));
                for (path, f) in &draft.files {
                    let file = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                    let when = chrono::DateTime::parse_from_rfc3339(&f.exported_at).map(|d| d.format("%Y-%m-%d %H:%M").to_string()).unwrap_or_default();
                    ui.label(RichText::new(tf!("{file}: from the project {project}, exported {when}", file = file, project = f.project, when = when)).size(12.0).color(theme.text_muted));
                }

                dialog::section(ui, &theme, t!("Workspaces"));
                egui::Frame::new().fill(theme.field).corner_radius(8).inner_margin(egui::Margin::symmetric(12, 8)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    for (item, name) in draft.items.iter_mut().zip(&names) {
                        let ws = &draft.files[item.file].1.workspaces[item.index].workspace;
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut item.chosen, &ws.name);
                            if let Some(n) = name
                                && *n != ws.name
                            {
                                ui.label(RichText::new(tf!("becomes {name}, the name is taken", name = n)).size(11.5).color(theme.warning));
                            }
                            if let Some(f) = &ws.folder {
                                let new = !project.meta.folders.contains(f) && !project.workspaces.values().any(|w| w.folder.as_ref() == Some(f));
                                let text = if new { tf!("new folder {name}", name = f) } else { tf!("in {name}", name = f) };
                                ui.label(RichText::new(text).size(11.5).color(theme.text_muted));
                            }
                            ui.label(RichText::new(blocks_text(ws.nodes.len())).size(11.5).color(theme.text_muted));
                        });
                    }
                });

                if !draft.commands.is_empty() {
                    dialog::section(ui, &theme, t!("Slash commands"));
                    for c in &mut draft.commands {
                        let text = format!("/{}", c.entry.get("name").and_then(Value::as_str).unwrap_or(""));
                        if c.exists {
                            ui.horizontal(|ui| {
                                ui.add_enabled(false, egui::Checkbox::new(&mut false, RichText::new(&text).monospace()));
                                ui.label(RichText::new(t!("already in this project, kept as it is")).size(11.5).color(theme.text_muted));
                            });
                        } else {
                            ui.checkbox(&mut c.chosen, RichText::new(&text).monospace());
                        }
                    }
                }

                if !draft.missing_plugins.is_empty() {
                    dialog::section(ui, &theme, t!("Missing plugins"));
                    for (id, version) in &draft.missing_plugins {
                        ui.label(RichText::new(tf!("{id} {version} isn't installed: its blocks show as missing until it is.", id = id, version = version)).size(12.0).color(theme.warning));
                    }
                }

                dialog::section(ui, &theme, t!("Summary"));
                let count = names.iter().flatten().count();
                let blocks: usize = draft.items.iter().filter(|i| i.chosen).map(|i| draft.files[i.file].1.workspaces[i.index].workspace.nodes.len()).sum();
                let renamed = draft.items.iter().zip(&names).filter(|(i, n)| n.as_ref().is_some_and(|n| *n != draft.files[i.file].1.workspaces[i.index].workspace.name)).count();
                let mut new_folders: Vec<&str> = draft
                    .items
                    .iter()
                    .filter(|i| i.chosen)
                    .filter_map(|i| draft.files[i.file].1.workspaces[i.index].workspace.folder.as_deref())
                    .filter(|f| !project.meta.folders.iter().any(|x| x == f) && !project.workspaces.values().any(|w| w.folder.as_deref() == Some(*f)))
                    .collect();
                new_folders.sort();
                new_folders.dedup();
                let commands = draft.commands.iter().filter(|c| c.chosen && !c.exists).count();
                summary(ui, &theme, |ui| {
                    summary_line(ui, &theme, &tf!("{workspaces} with {blocks}", workspaces = workspaces_text(count), blocks = blocks_text(blocks)));
                    if renamed == 1 {
                        summary_line(ui, &theme, t!("1 gets a new name, because the project has a workspace with its name"));
                    } else if renamed > 1 {
                        summary_line(ui, &theme, &tf!("{count} get a new name, because the project has workspaces with theirs", count = renamed));
                    }
                    if !new_folders.is_empty() {
                        summary_line(ui, &theme, &tf!("New folders: {names}", names = new_folders.join(", ")));
                    }
                    summary_line(ui, &theme, &commands_text(commands));
                    ui.add_space(4.0);
                    ui.label(RichText::new(t!("Nothing is replaced. The project only changes on disk when you save.")).size(11.5).color(theme.text_muted));
                });
            }
            Part::Footer => {
                let any = draft.items.iter().any(|i| i.chosen) || draft.commands.iter().any(|c| c.chosen && !c.exists);
                ui.add_enabled_ui(any, |ui| go = dialog::primary_button(ui, &theme, t!("Import")).clicked());
                cancel = dialog::secondary_button(ui, &theme, t!("Cancel")).clicked();
            }
        },
    );
    if cancel {
        ed.ws_import = None;
    } else if go
        && let Some(draft) = ed.ws_import.take()
        && let Some(project) = ed.project.as_mut()
    {
        let added = apply_import(project, &draft);
        ed.edited = true;
        ed.refresh_define_ctx();
        ed.log(Level::Success, tf!("Imported {workspaces}", workspaces = workspaces_text(added.len())));
        if let Some(first) = added.into_iter().next() {
            ed.actions.push(Action::OpenWorkspace(first));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn demo() -> (Editor, PathBuf) {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut ed = Editor::new(vec![root.join("plugins")], None).unwrap();
        let dir = std::env::temp_dir().join(format!("xc-exchange-{}-{}", std::process::id(), rand_suffix()));
        let _ = std::fs::remove_dir_all(&dir);
        // A copy of DemoBot, so the test can change it.
        copy_dir(&root.join("examples/DemoBot"), &dir);
        ed.open_project(&dir);
        (ed, dir)
    }

    fn rand_suffix() -> u128 {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    }

    fn copy_dir(from: &std::path::Path, to: &std::path::Path) {
        std::fs::create_dir_all(to).unwrap();
        for e in std::fs::read_dir(from).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                copy_dir(&p, &to.join(e.file_name()));
            } else {
                std::fs::copy(&p, to.join(e.file_name())).unwrap();
            }
        }
    }

    #[test]
    fn export_then_import_into_another_project() {
        let (ed, dir) = demo();
        let project = ed.project.as_ref().unwrap();
        // The rank command workspace uses /rank, which the builder defines.
        let rank = &project.workspaces["rank-command"];
        assert!(commands_used(rank).contains("rank"));
        let file = build_file(project, &ed.reg, &["rank-command".to_string(), "welcome".to_string()], &["rank".to_string()].into());
        assert_eq!(file.workspaces.len(), 2);
        assert_eq!(file.slash_commands.iter().map(command_root).collect::<Vec<_>>(), ["rank"]);
        assert!(file.plugins.contains_key("xc.discord") && file.plugins.contains_key("xc.core"));
        let text = serde_json::to_string_pretty(&file).unwrap();
        let read = read_file(&text).unwrap();
        assert!(read_file(&text.replace(FORMAT, "xc-circuit-workspaces/2")).is_err(), "a newer format is refused");
        assert!(read_file("{\"format\":\"other\",\"workspaces\":[]}").is_err());

        // Into the same project: names are taken, so they get (2); /rank exists and is kept.
        let (mut other, other_dir) = demo();
        open_import(&mut other, vec![(PathBuf::from("x.xcws.json"), read)]);
        let draft = other.ws_import.take().unwrap();
        assert!(draft.commands[0].exists && !draft.commands[0].chosen);
        let project = other.project.as_mut().unwrap();
        let before = project.workspaces.len();
        let added = apply_import(project, &draft);
        assert_eq!(project.workspaces.len(), before + 2);
        let names: Vec<&str> = added.iter().map(|k| project.workspaces[k].name.as_str()).collect();
        assert_eq!(names, ["Rank command (2)", "Welcome (2)"]);
        assert_eq!(slash_commands(project).len(), 1, "the existing /rank is kept, not doubled");
        let _ = std::fs::remove_dir_all(dir);
        let _ = std::fs::remove_dir_all(other_dir);
    }

    #[test]
    fn importing_adds_new_commands_and_folders() {
        let (mut ed, dir) = demo();
        let mut ws = Workspace::new("Shop");
        ws.folder = Some("Economy".into());
        let file = WorkspacesFile {
            format: FORMAT.into(),
            project: "Other".into(),
            app_version: "0.1.0".into(),
            exported_at: String::new(),
            workspaces: vec![WorkspaceEntry { key: "shop".into(), workspace: ws }],
            slash_commands: vec![serde_json::json!({ "name": "buy", "description": "Buy something", "options": [] })],
            plugins: [("xc.music".to_string(), "1.0.0".to_string())].into_iter().collect(),
        };
        open_import(&mut ed, vec![(PathBuf::from("shop.xcws.json"), file)]);
        let draft = ed.ws_import.take().unwrap();
        assert_eq!(draft.missing_plugins, [("xc.music".to_string(), "1.0.0".to_string())]);
        let project = ed.project.as_mut().unwrap();
        let added = apply_import(project, &draft);
        assert_eq!(project.workspaces[&added[0]].name, "Shop");
        assert!(project.meta.folders.contains(&"Economy".to_string()));
        assert!(slash_commands(project).iter().any(|c| command_root(c) == "buy"));
        let _ = std::fs::remove_dir_all(dir);
    }
}
