//! Installing plugins from zips, reloading them and running native ones.

use super::*;

impl CircuitApp {
    /// Opens the Plugins tab, or focuses it.
    pub(super) fn open_plugins(&mut self) {
        match self.dock.find_tab(&Tab::Plugins) {
            Some(path) => {
                let _ = self.dock.set_active_tab(path);
            }
            None => self.plugins = Some(crate::plugins::manager::Draft::new(&self.native)),
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
        self.native.sync(&mut self.ed, self.bundled_plugins.as_deref());
    }

    /// Bot events, checks and queued commands for script and native plugins.
    pub(super) fn run_native(&mut self, ctx: &egui::Context) {
        let running = self.bot.state != BotState::Stopped;
        for ev in std::mem::take(&mut self.bot.events) {
            self.native.bot_event(&mut self.ed, &ev, running);
        }
        self.native.tick(&mut self.ed, running);
        for id in std::mem::take(&mut self.native.queued) {
            match id.as_str() {
                "bot.run" => self.run_command(Cmd::Run, ctx),
                "bot.stop" => self.run_command(Cmd::Stop, ctx),
                "file.save" => self.run_command(Cmd::Save, ctx),
                _ => {
                    if !self.native.run_command(&mut self.ed, &id, running) {
                        self.ed.log(Level::Warn, format!("No command `{id}`"));
                    }
                }
            }
        }
        if self.native.plugins.iter().any(|p| p.status == crate::plugins::native::Status::Loaded) {
            ctx.request_repaint_after(std::time::Duration::from_secs(1));
        }
        // Plugins' timers, and what they float over the editor (a desk pet …).
        if let Some(wake) = self.native.frame(ctx, &mut self.ed, running) {
            ctx.request_repaint_after(wake);
        }
    }
}
