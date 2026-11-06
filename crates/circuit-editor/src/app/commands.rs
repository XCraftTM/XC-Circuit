//! The command palette's entries and running a command (from the palette, a menu or a shortcut).

use super::*;

impl CircuitApp {
    /// Everything the command palette offers right now.
    pub(super) fn palette_items(&self) -> Vec<Item> {
        let has_project = self.ed.project.is_some();
        let running = self.bot.state != BotState::Stopped;
        let key = self.ed.undo_key();
        let has_sel = !self.ed.selection(&key).is_empty();
        let mut items = Vec::new();
        if has_project {
            items.push(Item::new(t!("Bot"), if running { t!("Restart bot") } else { t!("Run bot") }, Cmd::Run).shortcut(self.key_label(Hotkey::Run)));
        }
        if running {
            items.push(Item::new(t!("Bot"), t!("Stop bot"), Cmd::Stop).shortcut(self.key_label(Hotkey::Stop)));
        }
        if has_project {
            if self.ed.has_slash_commands() {
                items.push(Item::new(t!("Bot"), t!("Slash Commands…"), Cmd::SlashCommands));
            }
            items.push(Item::new(t!("Bot"), t!("Bot Settings…"), Cmd::Settings));
            items.push(Item::new(t!("Bot"), "Export…", Cmd::Export));
            items.push(Item::new(t!("File"), "Save", Cmd::Save).shortcut(self.key_label(Hotkey::Save)));
            items.push(Item::new(t!("File"), t!("Import workspaces…"), Cmd::ImportWorkspaces));
            items.push(Item::new(t!("File"), t!("Export workspaces…"), Cmd::ExportWorkspaces));
        }
        items.push(Item::new(t!("File"), t!("New Project…"), Cmd::NewProject).shortcut(self.key_label(Hotkey::NewProject)));
        items.push(Item::new(t!("File"), t!("Open Project…"), Cmd::OpenProject).shortcut(self.key_label(Hotkey::OpenProject)));
        let open = self.ed.project.as_ref().map(|p| p.root.display().to_string());
        for path in self.ed.prefs.recent_projects.iter().filter(|p| Some(*p) != open.as_ref()) {
            let name = Path::new(path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.clone());
            items.push(Item::new(t!("Recent"), name, Cmd::OpenRecent(PathBuf::from(path))).detail(path.clone()));
        }
        if self.ed.project.is_some() {
            items.push(Item::new(t!("File"), t!("Close Project"), Cmd::CloseProject));
        }
        if self.ed.examples_dir.is_some() {
            items.push(Item::new(t!("Help"), t!("Open Example: DemoBot"), Cmd::OpenExample));
        }
        items.push(Item::new(t!("File"), t!("Reload Plugins"), Cmd::ReloadPlugins));
        if self.ed.history.can_undo(&key) {
            items.push(Item::new(t!("Edit"), "Undo", Cmd::Undo).shortcut(self.key_label(Hotkey::Undo)));
        }
        if self.ed.history.can_redo(&key) {
            items.push(Item::new(t!("Edit"), "Redo", Cmd::Redo).shortcut(self.key_label(Hotkey::Redo)));
        }
        if has_sel {
            items.push(Item::new(t!("Edit"), "Duplicate", Cmd::Duplicate).shortcut(self.key_label(Hotkey::Duplicate)));
            items.push(Item::new(t!("Edit"), "Delete", Cmd::Delete).shortcut(self.key_label(Hotkey::Delete)));
        }
        if has_project {
            items.push(Item::new(t!("Edit"), t!("Select all"), Cmd::SelectAll).shortcut(self.key_label(Hotkey::SelectAll)));
        }
        for view in SideView::ALL {
            let tab = Tab::Side(view);
            let label = if self.panel_open(&tab) { tf!("Close {name}", name = view.name()) } else { tf!("Show {name}", name = view.name()) };
            items.push(Item::new(t!("View"), label, Cmd::TogglePanel(tab)));
        }
        for (tab, name) in Tab::PANELS {
            let label = if self.panel_open(&tab) { tf!("Close {name} panel", name = crate::i18n::t(name)) } else { tf!("Show {name} panel", name = crate::i18n::t(name)) };
            items.push(Item::new(t!("View"), label, Cmd::TogglePanel(tab)));
        }
        for t in &self.ed.reg.themes {
            if t.id != self.ed.theme.id {
                items.push(Item::new(t!("Theme"), format!("Use {} theme", t.name), Cmd::Theme(t.id.clone())));
            }
        }
        items.push(Item::new(t!("View"), t!("Reset layout"), Cmd::ResetLayout));
        items.push(Item::new(t!("Help"), t!("About XC™ Circuit"), Cmd::About));
        items.push(Item::new(t!("File"), "Settings", Cmd::OpenSettings).shortcut(self.key_label(Hotkey::Settings)));
        items.push(Item::new(t!("Help"), t!("Take the tour"), Cmd::Tour));
        if has_project {
            items.push(Item::new(t!("File"), t!("Restore from backup…"), Cmd::Backups));
        }
        items.push(Item::new(t!("Plugins"), t!("Manage Plugins…"), Cmd::ManagePlugins));
        for p in &self.native.plugins {
            for c in p.commands() {
                items.push(Item::new("Plugin", c.title.clone(), Cmd::PluginCommand(c.id.clone())).detail(p.name.clone()));
            }
            for panel in p.panels() {
                let tab = Tab::Plugin(p.id.clone(), panel.id.clone());
                let verb = if self.panel_open(&tab) { "Close" } else { "Show" };
                items.push(Item::new("Plugin", format!("{verb} {} panel", panel.title), Cmd::TogglePluginPanel(p.id.clone(), panel.id.clone())).detail(p.name.clone()));
            }
        }
        if let Some(p) = &self.ed.project {
            for (wk, ws) in &p.workspaces {
                items.push(Item::new(t!("Workspace"), ws.name.clone(), Cmd::OpenWorkspace(wk.clone())));
            }
            for (wk, ws) in &p.workspaces {
                for (id, node) in &ws.nodes {
                    let block = node.block.split('@').next().unwrap_or(&node.block);
                    let def = self.ed.reg.block(block);
                    let name = def.map(|d| d.name.clone()).unwrap_or_else(|| block.to_string());
                    items.push(Item::new(t!("Block"), name, Cmd::RevealBlock(wk.clone(), id.clone())).detail(format!("{} · #{id}", ws.name)).content(block_texts(def, &node.values)));
                }
            }
        }
        items
    }

    pub(super) fn run_command(&mut self, cmd: Cmd, ctx: &egui::Context) {
        match cmd {
            Cmd::NewProject => self.new_project_dialog(),
            Cmd::OpenProject => self.open_project_dialog(),
            Cmd::OpenExample => {
                if let Some(example) = self.ed.examples_dir.as_deref().and_then(writable_example) {
                    self.switch_project(&example);
                }
            }
            Cmd::OpenRecent(path) => self.open_recent(&path),
            Cmd::CloseProject => self.close_project(),
            Cmd::Save => {
                self.ed.save();
                self.bot.restart_if_running(&mut self.ed, ctx);
            }
            Cmd::ReloadPlugins => self.reload_plugins(ctx),
            Cmd::Undo => self.undo(),
            Cmd::Redo => self.redo(),
            Cmd::Duplicate => self.ed.duplicate(),
            Cmd::Delete => self.ed.delete_selection(),
            Cmd::SelectAll => self.ed.select_all(),
            Cmd::TogglePanel(tab) => self.toggle_panel(tab),
            Cmd::Theme(id) => {
                self.ed.set_theme(&id);
                self.ed.theme.apply(ctx);
            }
            Cmd::ResetLayout => self.reset_layout(),
            Cmd::Run => self.bot.start(&mut self.ed, ctx),
            Cmd::Stop => self.bot.stop(&mut self.ed),
            Cmd::SlashCommands => crate::builders::slash::open(&mut self.ed),
            Cmd::Settings => self.bot.open_settings(&mut self.ed),
            Cmd::Export => self.bot.open_export(&self.ed),
            Cmd::ImportWorkspaces => crate::editor::exchange::pick_import(&mut self.ed),
            Cmd::ExportWorkspaces => crate::editor::exchange::open_export(&mut self.ed, crate::editor::exchange::Preselect::All),
            Cmd::About => self.show_about = true,
            Cmd::OpenSettings => self.open_settings(),
            Cmd::Tour => self.start_tour(),
            Cmd::Backups => open_center(&mut self.dock, Tab::Backups),
            Cmd::ManagePlugins => self.open_plugins(),
            Cmd::PluginCommand(id) => self.native.queued.push(id),
            Cmd::TogglePluginPanel(plugin, panel) => self.toggle_panel(Tab::Plugin(plugin, panel)),
            Cmd::OpenWorkspace(key) => self.ed.actions.push(Action::OpenWorkspace(key)),
            Cmd::RevealBlock(ws, id) => self.ed.reveal(&ws, &id),
        }
    }

    pub(super) fn undo(&mut self) {
        if self.ed.undo() {
            self.reopen_bot_settings();
        }
    }

    pub(super) fn redo(&mut self) {
        if self.ed.redo() {
            self.reopen_bot_settings();
        }
    }

    /// An action's first shortcut, for menus and the palette.
    pub(super) fn key_label(&self, action: Hotkey) -> String {
        keymap::label(&self.ed.prefs.shortcuts, action)
    }
}
