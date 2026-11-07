//! Opening, creating, switching and closing projects, and the start page.

use super::*;

impl CircuitApp {
    pub(super) fn open_project_dialog(&mut self) {
        if let Some(dir) = rfd::FileDialog::new().set_title(t!("Open a Circuit project folder")).pick_folder() {
            self.switch_project(&dir);
        }
    }

    /// File › New Project: the kind of project, its name and place, and what
    /// it starts from.
    pub(super) fn new_project_dialog(&mut self) {
        let location = self
            .ed
            .prefs
            .new_project_location
            .as_ref()
            .map(PathBuf::from)
            .filter(|p| p.is_dir())
            .unwrap_or_else(|| {
                // Next to the open project, unless that is one of the app's examples.
                let open = self.ed.project.as_ref().map(|p| p.root.as_path()).filter(|r| self.ed.examples_dir.as_ref().is_none_or(|e| !r.starts_with(e)));
                crate::pages::new_project::default_location(open)
            });
        self.new_project = Some(crate::pages::new_project::NewProject::new(&self.ed.reg, location));
    }

    /// Opens a project and puts it first among the recent ones.
    pub(super) fn switch_project(&mut self, dir: &Path) {
        self.load_project(dir);
        if let Some(root) = self.ed.project.as_ref().map(|p| p.root.clone()) {
            self.ed.prefs.remember_project(&root);
        }
    }

    /// A recent project: opened, or taken off the list when it is gone.
    pub(super) fn open_recent(&mut self, dir: &Path) {
        if dir.join(circuit_core::project::PROJECT_FILE).is_file() {
            self.switch_project(dir);
        } else {
            self.ed.log(Level::Warn, format!("{} isn't there any more, so it's off the recent list", dir.display()));
            self.ed.prefs.forget_project(&dir.display().to_string());
        }
    }

    /// Saves and closes the project; the start page shows.
    pub(super) fn close_project(&mut self) {
        if self.ed.project.is_none() {
            return;
        }
        if self.ed.unsaved_now() {
            self.ed.save();
        }
        self.bot.stop(&mut self.ed);
        clear_canvases(&mut self.dock);
        self.ed.close_project();
        self.plugin_host.sync(&mut self.ed, self.bundled_plugins.as_deref());
        self.watcher.watch(&self.ed.watch_dirs());
        self.stored = Default::default();
    }

    pub(super) fn start_page_pick(&mut self, pick: crate::pages::start_page::Pick) {
        use crate::pages::start_page::Pick;
        match pick {
            Pick::New => self.new_project_dialog(),
            Pick::Open => self.open_project_dialog(),
            Pick::Recent(dir) => self.open_recent(&dir),
            Pick::Forget(path) => self.ed.prefs.forget_project(&path),
            Pick::Example => {
                if let Some(example) = self.ed.examples_dir.as_deref().and_then(writable_example) {
                    self.switch_project(&example);
                }
            }
        }
    }

    /// Opens a project without listing it (the tour's).
    pub(super) fn load_project(&mut self, dir: &Path) {
        if self.ed.unsaved_now() {
            self.ed.save();
        }
        self.bot.stop(&mut self.ed);
        clear_canvases(&mut self.dock);
        self.ed.open_project(dir);
        // It couldn't be opened: the project before stays, and so does its canvas.
        if let Some(p) = self.ed.project.as_ref().filter(|p| p.root != dir)
            && let Some(first) = p.workspaces.keys().next().cloned()
        {
            self.ed.actions.push(Action::OpenWorkspace(first));
        }
        self.plugin_host.sync(&mut self.ed, self.bundled_plugins.as_deref());
        self.watcher.watch(&self.ed.watch_dirs());
        self.stored = Default::default();
        // Its first canvas right away, where the old ones were.
        self.apply_actions();
    }

    /// Without a project only the start page shows: the panels and tabs are
    /// put aside, and come back as they were when a project opens.
    pub(super) fn swap_start_page(&mut self) {
        let showing = self.dock_before_start.is_some();
        if self.ed.project.is_none() && !showing && self.tour.is_none() {
            let start = Dock::new(vec![Tab::Start]);
            self.dock_before_start = Some(std::mem::replace(&mut self.dock, start));
        } else if self.ed.project.is_some() && showing {
            let mut dock = self.dock_before_start.take().unwrap_or_else(default_layout);
            // Where its canvases were, a placeholder keeps the place for the new ones.
            clear_canvases(&mut dock);
            if dock.is_empty() {
                dock = default_layout();
            }
            self.dock = dock;
        }
        self.drop_placeholder();
    }

    /// Once a canvas is in its place, the placeholder that kept it goes. With
    /// a project open and no canvas, there is always one (left of the
    /// Inspector, if no canvas left one in its place).
    pub(super) fn drop_placeholder(&mut self) {
        if self.dock_before_start.is_some() {
            return;
        }
        let canvas = self.dock.find(|t| matches!(t, Tab::Canvas(_))).is_some();
        if canvas {
            close_tabs(&mut self.dock, |t| matches!(t, Tab::Start));
        } else if self.ed.project.is_some() && !self.dock.contains(&Tab::Start) {
            let focused = self.dock.focused().cloned();
            open_center(&mut self.dock, Tab::Start);
            if let Some(tab) = focused {
                self.dock.focus(&tab);
            }
        }
    }

    /// The placeholder's buttons: open a workspace, or make one and open it.
    pub(super) fn no_workspace_ask(&mut self, ask: super::dock::NoWorkspace) {
        use super::dock::NoWorkspace;
        let key = match ask {
            NoWorkspace::Open(key) => key,
            NoWorkspace::New => {
                let Some(project) = self.ed.project.as_mut() else { return };
                let name = (1..).map(|i| if i == 1 { t!("Workspace").to_string() } else { format!("{} {i}", t!("Workspace")) }).find(|n| !project.workspaces.values().any(|w| w.name == *n)).expect("unbounded");
                let key = project.new_workspace_key(&name);
                project.workspaces.insert(key.clone(), circuit_core::Workspace::new(name));
                self.ed.edited = true;
                key
            }
        };
        self.ed.actions.push(Action::OpenWorkspace(key));
    }
}
