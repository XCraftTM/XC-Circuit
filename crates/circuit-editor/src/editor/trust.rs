//! A project's own plugins (its `plugins/` folder), and whether they load.
//!
//! Plugins hold code: their blocks run in the bot with the same access as
//! any program, and their editor scripts run as soon as they load. A project
//! from someone else brings its plugins along, so they only load once the
//! user said yes, for this folder and each plugin (kept in the app's
//! preferences, not in the project). The app's own examples came with the
//! app and need no asking. And a project's plugin never takes the place of
//! one that comes with the app.

use std::path::{Path, PathBuf};

use super::Editor;

/// One of a project's own plugins.
#[derive(Clone, Debug, PartialEq)]
pub struct OwnPlugin {
    pub id: String,
    pub name: String,
    pub dir: PathBuf,
}

/// The plugins in a project's `plugins/` folder, by id.
pub fn own_plugins(root: &Path) -> Vec<OwnPlugin> {
    let Ok(entries) = std::fs::read_dir(root.join("plugins")) else { return Vec::new() };
    let mut found: Vec<OwnPlugin> = entries
        .flatten()
        .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
        .filter_map(|e| {
            let dir = e.path();
            let text = std::fs::read_to_string(dir.join("plugin.toml")).ok()?;
            // One whose plugin.toml can't be read still counts, by its
            // folder's name; loading it says what's wrong with it.
            let folder = e.file_name().to_string_lossy().into_owned();
            let (id, name) = match toml::from_str::<circuit_plugins::PluginManifest>(&text) {
                Ok(m) if !m.id.is_empty() => (m.id.clone(), if m.name.is_empty() { m.id } else { m.name }),
                _ => (folder.clone(), folder),
            };
            Some(OwnPlugin { id, name, dir })
        })
        .collect();
    found.sort_by(|a, b| a.id.cmp(&b.id));
    found
}

impl Editor {
    /// Whether a project is one of the app's own examples: in an `examples`
    /// folder next to a folder the app's plugins are in (the bundled ones,
    /// or the user's copy in the app's data folder).
    fn is_app_example(&self, root: &Path) -> bool {
        let real = |p: &Path| p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
        let root = real(root);
        self.plugin_dirs.iter().filter_map(|d| d.parent()).any(|base| root.starts_with(real(&base.join("examples"))))
    }

    /// What becomes of the open project's own plugins: the ones waiting for
    /// the user's yes, and the ones refused for having the id of a plugin
    /// that comes with the app. The rest load.
    pub(super) fn sort_own_plugins(&self) -> (Vec<OwnPlugin>, Vec<OwnPlugin>) {
        let Some(p) = &self.project else { return Default::default() };
        let example = self.is_app_example(&p.root);
        let yes = self.prefs.trusted_plugins.get(&p.root.display().to_string());
        let comes_with_the_app = |id: &str| self.bundled_plugins.iter().any(|b| b == id) || self.optional_plugins.iter().any(|(o, _)| o == id);
        let (refused, rest): (Vec<_>, Vec<_>) = own_plugins(&p.root).into_iter().partition(|o| comes_with_the_app(&o.id));
        let waiting = rest.into_iter().filter(|o| !example && !yes.is_some_and(|y| y.contains(&o.id))).collect();
        (waiting, refused)
    }

    /// The user's yes to the open project's own plugins: kept for this
    /// folder. They load with the next [`Editor::reload_plugins`].
    pub fn trust_own_plugins(&mut self) {
        let Some(p) = &self.project else { return };
        let ids = own_plugins(&p.root).into_iter().map(|o| o.id).collect();
        self.prefs.trusted_plugins.insert(p.root.display().to_string(), ids);
        self.plugin_ask = false;
    }
}
