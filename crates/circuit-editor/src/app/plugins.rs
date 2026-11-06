//! Installing plugins from zips, reloading them and running their scripts.

use super::*;

impl CircuitApp {
    /// Shows the Plugins view (in the side bar, or its tab).
    pub(super) fn open_plugins(&mut self) {
        self.show_panel(Tab::Side(SideView::Plugins));
    }

    /// Removes a plugin: its folder is deleted. Never one that comes with the
    /// app, nor one outside the plugin folders.
    pub(super) fn remove_plugin(&mut self, id: &str, ctx: &egui::Context) {
        if self.ed.bundled_plugins.iter().any(|b| b == id) {
            return;
        }
        let Some(lp) = self.ed.reg.plugins.iter().chain(&self.ed.reg.disabled).find(|p| p.manifest.id == id).cloned() else { return };
        let mut folders = self.ed.plugin_dirs.clone();
        if let Some(p) = &self.ed.project {
            folders.push(p.root.join("plugins"));
        }
        if !folders.iter().any(|f| lp.dir.parent() == Some(f.as_path())) {
            self.ed.log(Level::Warn, format!("{} isn't in a plugin folder, so it wasn't removed.", lp.manifest.name));
            return;
        }
        match std::fs::remove_dir_all(&lp.dir) {
            Ok(()) => {
                self.ed.prefs.disabled_plugins.retain(|p| p != id);
                self.ed.log(Level::Success, tf!("Removed {name}", name = lp.manifest.name));
                self.reload_plugins(ctx);
            }
            Err(e) => self.ed.log(Level::Error, format!("Couldn't remove {}: {e}", lp.manifest.name)),
        }
    }

    /// Plugins › Install from Zip…: asks for the file.
    pub(super) fn pick_plugin_zip(&mut self, ctx: &egui::Context) {
        if let Some(path) = rfd::FileDialog::new().set_title(t!("Install a plugin")).add_filter(t!("Plugin (.zip)"), &["zip"]).pick_file() {
            self.install_plugin_zip(&path, ctx);
        }
    }

    /// Installs the plugin in a .zip into the user's plugins folder (asking
    /// before replacing one) and loads it.
    pub(super) fn install_plugin_zip(&mut self, path: &Path, ctx: &egui::Context) {
        use crate::plugins::install::{InstallError, install_zip};
        let dir = crate::bot::app_data_dir().join("plugins");
        let result = match install_zip(path, &dir, false) {
            Err(InstallError::Exists { name, .. }) => {
                let replace = rfd::MessageDialog::new()
                    .set_title(t!("Install a plugin"))
                    .set_description(tf!("{name} is installed already. Replace it with the version in this file?", name = name))
                    .set_buttons(rfd::MessageButtons::YesNo)
                    .show();
                if replace != rfd::MessageDialogResult::Yes {
                    return;
                }
                install_zip(path, &dir, true)
            }
            other => other,
        };
        match result {
            Ok(done) => {
                self.ed.log(Level::Success, tf!("Installed {name} into {folder}", name = done.name, folder = done.dir.display()));
                self.reload_plugins(ctx);
            }
            Err(e) => self.ed.log(Level::Error, tf!("Couldn't install {file}: {error}", file = path.display(), error = e)),
        }
    }

    pub(super) fn reload_plugins(&mut self, ctx: &egui::Context) {
        self.ed.reload_plugins();
        self.ed.refresh_define_ctx();
        self.ed.theme.apply(ctx);
        self.plugin_host.sync(&mut self.ed, self.bundled_plugins.as_deref());
    }

    /// Bot events, checks and queued commands for plugins' scripts.
    pub(super) fn run_plugins(&mut self, ctx: &egui::Context) {
        let running = self.bot.state != BotState::Stopped;
        for ev in std::mem::take(&mut self.bot.events) {
            self.plugin_host.bot_event(&mut self.ed, &ev, running);
        }
        self.plugin_host.tick(&mut self.ed, running);
        for id in std::mem::take(&mut self.plugin_host.queued) {
            match id.as_str() {
                "bot.run" => self.run_command(Cmd::Run, ctx),
                "bot.stop" => self.run_command(Cmd::Stop, ctx),
                "file.save" => self.run_command(Cmd::Save, ctx),
                _ => {
                    if !self.plugin_host.run_command(&mut self.ed, &id, running) {
                        self.ed.log(Level::Warn, format!("No command `{id}`"));
                    }
                }
            }
        }
        if self.plugin_host.plugins.iter().any(|p| p.status == crate::plugins::host::Status::Loaded) {
            ctx.request_repaint_after(std::time::Duration::from_secs(1));
        }
        // Plugins' timers, and what they float over the editor (a desk pet …).
        if let Some(wake) = self.plugin_host.frame(ctx, &mut self.ed, running) {
            ctx.request_repaint_after(wake);
        }
    }
}
