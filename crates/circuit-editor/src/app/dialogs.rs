//! Dialogs over the editor: crash reports, recovering unsaved changes, updates, shortcuts, the new project chooser and the rest.

use crate::ui::kit;
use super::*;

impl CircuitApp {
    /// After a crash: what happened, with a ready bug report.
    pub(super) fn crash_dialog(&mut self, ctx: &egui::Context) {
        // Not over the first-start page (or the tour): it waits until they're done.
        if self.setup.is_some() || self.tour.is_some() {
            return;
        }
        let Some(crash) = self.crash_offer.clone() else { return };
        let theme = self.ed.theme.clone();
        let (mut report, mut open_log, mut later) = (false, false, false);
        let close = dialog::show(
            ctx,
            &theme,
            dialog::Spec { id: "crash", title: t!("XC Circuit stopped unexpectedly"), subtitle: Some(t!("Sorry about that. A report helps fix it.")), icon: None, width: 520.0 },
            |ui, part| match part {
                dialog::Part::Body => {
                    ui.label(RichText::new(t!("What went wrong:")).size(12.5).color(theme.text_muted));
                    ui.add(egui::Label::new(RichText::new(&crash.panic).monospace().size(12.0).color(theme.text)).wrap());
                    ui.add_space(6.0);
                    ui.label(RichText::new(t!("Report on GitHub opens a bug report with this filled in; you can read and change everything before sending it. Please attach the crash log too.")).size(12.0).color(theme.text_muted));
                }
                dialog::Part::Footer => {
                    report = kit::primary_button(ui, &theme, t!("Report on GitHub")).clicked();
                    open_log = kit::secondary_button(ui, &theme, t!("Show crash log")).clicked();
                    later = kit::secondary_button(ui, &theme, t!("Not now")).clicked();
                }
            },
        );
        if report {
            let _ = open::that(crate::app::crash::report_url(crate::app::crash::Report::Crash(&crash)));
        }
        if open_log {
            let _ = open::that(crash.path.parent().unwrap_or(&crash.path));
        }
        if report || later || close {
            self.ed.prefs.last_crash_seen = crash.at;
            self.crash_offer = None;
        }
    }

    /// After a crash: offers the unsaved changes that were left behind.
    pub(super) fn recovery_dialog(&mut self, ctx: &egui::Context, theme: &crate::ui::theme::Theme) {
        let Some(rec) = &self.ed.recovery_offer else { return };
        let Some(project) = &self.ed.project else { return };
        let changes = circuit_core::backup::compare(&rec.workspaces, &project.workspaces);
        let changed: Vec<String> = changes
            .iter()
            .filter_map(|(_, name, c)| match c {
                circuit_core::backup::Change::Same => None,
                circuit_core::backup::Change::Changed => Some(format!("{name} (changed)")),
                circuit_core::backup::Change::OnlyInBackup => Some(format!("{name} (new)")),
                circuit_core::backup::Change::OnlyNow => Some(format!("{name} (deleted)")),
            })
            .collect();
        let when = format!("{} ({})", crate::editor::local_time(rec.saved_at), crate::editor::ago(rec.saved_at));
        let icon = self.ed.reg.icons.get("clock").cloned();
        let (mut restore, mut discard) = (false, false);
        dialog::show(
            ctx,
            theme,
            dialog::Spec { id: "recovery", title: t!("Unsaved changes found"), subtitle: Some(t!("XC Circuit didn't close normally last time")), icon: icon.as_deref(), width: 500.0 },
            |ui, part| match part {
                dialog::Part::Body => {
                    ui.label(RichText::new(format!("Changes to {} from {when} weren't saved.", project.meta.name)).size(13.5).color(theme.text));
                    ui.add_space(6.0);
                    if changed.is_empty() {
                        ui.label(RichText::new(t!("They change the project's settings.")).size(12.5).color(theme.text_muted));
                    } else {
                        for c in &changed {
                            ui.label(RichText::new(format!("• {c}")).size(12.5).color(theme.text_muted));
                        }
                    }
                    ui.add_space(6.0);
                    ui.label(RichText::new(t!("Bring them back as unsaved changes, then save to keep them.")).size(12.5).color(theme.text_muted));
                }
                dialog::Part::Footer => {
                    restore = kit::primary_button(ui, theme, t!("Bring them back")).clicked();
                    discard = kit::secondary_button(ui, theme, t!("Discard")).clicked();
                }
            },
        );
        if restore && let Some(rec) = self.ed.recovery_offer.take() {
            self.ed.apply_recovery(rec);
        } else if discard {
            self.ed.recovery_offer = None;
            if let Some(p) = &self.ed.project {
                self.ed.backups.clear_recovery(&p.root);
            }
        }
    }

    /// What's new in an available update, with its download.
    /// Newer bundled blocks: what they are, with Update and Later.
    pub(super) fn block_updates_window(&mut self, ctx: &egui::Context) {
        if !self.block_updates.show || self.block_updates.offers.is_empty() {
            return;
        }
        let theme = self.ed.theme.clone();
        let offers = self.block_updates.offers.clone();
        let installing = self.block_updates.installing();
        let (mut update, mut later) = (false, false);
        let close = dialog::show(
            ctx,
            &theme,
            dialog::Spec {
                id: "block-updates",
                title: t!("Block updates are available"),
                subtitle: Some(t!("New and improved blocks, without updating the app.")),
                icon: None,
                width: 480.0,
            },
            |ui, part| match part {
                dialog::Part::Body => {
                    for o in &offers {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&o.entry.name).strong().color(theme.text));
                            ui.label(RichText::new(format!("{} → {}", o.current, o.entry.version)).color(theme.text_muted));
                        });
                    }
                    ui.add_space(6.0);
                    ui.label(RichText::new(t!("Your projects keep working: blocks that changed are upgraded when a project opens. Help › Undo Block Updates goes back.")).size(12.0).color(theme.text_muted));
                }
                dialog::Part::Footer => {
                    let label = if installing { t!("Updating…") } else { t!("Update") };
                    update = ui.add_enabled_ui(!installing, |ui| kit::primary_button(ui, &theme, label)).inner.clicked();
                    later = kit::secondary_button(ui, &theme, t!("Later")).clicked();
                }
            },
        );
        if update {
            self.block_updates.install_all();
        }
        if (later || close) && !installing {
            self.block_updates.show = false;
        }
    }

    pub(super) fn update_window(&mut self, ctx: &egui::Context) {
        let Some(r) = self.updater.available.clone().filter(|_| self.updater.show) else { return };
        let theme = self.ed.theme.clone();
        // Installed with the Setup, or an AppImage: it updates itself.
        let install = crate::app::updater::Install::current();
        let asset = install.asset(&r).cloned();
        let progress = self.updater.progress();
        let (mut download, mut update, mut skip, mut later) = (false, false, false, false);
        let close = dialog::show(
            ctx,
            &theme,
            dialog::Spec {
                id: "update",
                title: &tf!("XC™ Circuit {version} is available", version = r.version),
                subtitle: Some(&tf!("You have {version}.", version = env!("CARGO_PKG_VERSION"))),
                icon: None,
                width: 520.0,
            },
            |ui, part| match part {
                dialog::Part::Body => {
                    kit::section(ui, &theme, t!("What's new"));
                    egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                        if r.notes.trim().is_empty() {
                            ui.label(RichText::new(t!("The release has no notes.")).size(12.5).color(theme.text_muted));
                        } else {
                            crate::ui::notes::show(ui, &theme, &r.notes, 12.5);
                        }
                    });
                }
                dialog::Part::Footer => {
                    if let Some(a) = &asset {
                        let label = match progress {
                            Some(p) => tf!("Downloading… {percent} %", percent = (p * 100.0).round() as u32),
                            None => t!("Update and restart").to_string(),
                        };
                        let tip = tf!("Downloads {size} MB and installs it when XC Circuit closes, then opens it again. You're asked about unsaved changes first.", size = (a.size as f64 / 1e6).round());
                        update = ui.add_enabled_ui(progress.is_none(), |ui| kit::primary_button(ui, &theme, &label)).inner.on_hover_text(tip).clicked();
                    } else {
                        download = kit::primary_button(ui, &theme, t!("Download")).on_hover_text(&r.url).clicked();
                    }
                    later = kit::secondary_button(ui, &theme, t!("Later")).clicked();
                    skip = kit::secondary_button(ui, &theme, t!("Skip this version")).on_hover_text(t!("Not mentioned at start any more; Help › Check for Updates still finds it")).clicked();
                }
            },
        );
        if download {
            let _ = open::that(&r.url);
        }
        if update && let Some(a) = &asset {
            self.updater.download(&install, a);
        }
        if skip {
            self.ed.prefs.skipped_update = Some(r.version.clone());
            self.updater.available = None;
        }
        // Downloading, the window stays: when the download is done, the app closes.
        if download || skip || later || close {
            self.updater.show = false;
        }
    }

    /// Undo, clipboard and selection shortcuts, unless a text box has focus.
    pub(super) fn edit_shortcuts(&mut self, ctx: &egui::Context) {
        // The settings and data tabs undo changes to them; others a workspace.
        self.ed.undo_project = self.dock.find_active_focused().is_some_and(|(_, t)| matches!(t, Tab::SlashCommands | Tab::BotSettings));
        if ctx.text_edit_focused() || self.ed.picker.is_some() {
            return;
        }
        if self.ed.recording_shortcut(ctx) {
            return;
        }
        let keys = self.ed.prefs.shortcuts.clone();
        let hit = |action| keymap::consume(ctx, &keys, action);
        if hit(Hotkey::Redo) {
            self.redo();
        } else if hit(Hotkey::Undo) {
            self.undo();
        }
        if hit(Hotkey::Duplicate) {
            self.ed.duplicate();
        }
        if hit(Hotkey::SelectAll) {
            self.ed.select_all();
        }
        if hit(Hotkey::Delete) {
            self.ed.delete_selection();
        }
        let events = ctx.input(|i| i.events.clone());
        for e in events {
            match e {
                egui::Event::Copy => {
                    self.ed.copy(ctx);
                }
                egui::Event::Cut => {
                    if self.ed.copy(ctx) {
                        self.ed.delete_selection();
                    }
                }
                egui::Event::Paste(text) => {
                    let at = self.ed.active_workspace.as_ref().and_then(|k| self.ed.views.get(k)).and_then(|v| v.pointer);
                    self.ed.paste(&text, at);
                }
                _ => {}
            }
        }
    }

    pub(super) fn new_project_chooser(&mut self, ctx: &egui::Context, theme: &crate::ui::theme::Theme) {
        use crate::pages::new_project::Outcome;
        let Some(mut state) = self.new_project.take() else { return };
        let templates = crate::pages::new_project::templates(self.ed.examples_dir.as_deref(), &self.ed.reg);
        match crate::pages::new_project::show(ctx, theme, &mut state, &self.ed.reg, &templates) {
            None => self.new_project = Some(state),
            Some(Outcome::Cancel) => {}
            Some(Outcome::MorePlugins) => self.open_plugins(),
            Some(Outcome::Create { dir, name, mode, template }) => {
                self.ed.prefs.new_project_location = Some(state.location.trim().to_string());
                let made = match template {
                    Some(from) => create_from_template(&from, &dir, &name),
                    None => Project::create(&dir, &name, &mode).map(|_| ()),
                };
                match made {
                    Ok(()) => self.switch_project(&dir),
                    Err(e) => {
                        self.ed.log(Level::Error, format!("Could not create the project: {e:#}"));
                        self.new_project = Some(state);
                    }
                }
            }
        }
    }

    pub(super) fn dialogs(&mut self, ctx: &egui::Context) {
        self.update_window(ctx);
        self.block_updates_window(ctx);
        self.crash_dialog(ctx);
        let theme = self.ed.theme.clone();
        if self.setup.is_none() && self.tour.is_none() {
            self.recovery_dialog(ctx, &theme);
        }
        self.new_project_chooser(ctx, &theme);
        if self.show_about {
            let app_root = self.bundled_plugins.as_ref().and_then(|p| p.parent()).map(Path::to_path_buf);
            let info = tf!("{plugins} plugins · {blocks} blocks · {themes} themes loaded", plugins = self.ed.reg.plugins.len(), blocks = self.ed.reg.blocks.len(), themes = self.ed.reg.themes.len());
            let mut ok = false;
            let close = dialog::show(
                ctx,
                &theme,
                dialog::Spec { id: "about", title: t!("About XC™ Circuit"), subtitle: None, icon: None, width: 420.0 },
                |ui, part| match part {
                    dialog::Part::Body => {
                        ui.vertical_centered(|ui| {
                            ui.add_space(6.0);
                            ui.add(egui::Image::new(egui::include_image!("../../assets/icon.svg")).fit_to_exact_size(egui::vec2(76.0, 76.0)));
                            ui.add_space(10.0);
                            ui.label(RichText::new(t!("XC™ Circuit")).size(22.0).strong().color(theme.text));
                            ui.label(RichText::new(tf!("Version {version} · {channel}", version = env!("CARGO_PKG_VERSION"), channel = crate::app::crash::CHANNEL)).size(12.5).color(theme.text_muted));
                            ui.add_space(10.0);
                            ui.label(RichText::new(t!("Build bots by connecting blocks.")).size(13.5).color(theme.text));
                            ui.add_space(4.0);
                            kit::hint(ui, &theme, &info);
                            ui.add_space(12.0);
                            ui.label(RichText::new(tf!("Made by {name}", name = CREATOR)).size(13.0).color(theme.text));
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                // Centred: the links' width, then them.
                                let links = [(t!("GitHub profile"), CREATOR_URL.to_string()), (t!("XC Circuit on GitHub"), crate::app::crash::REPO.to_string())];
                                let width: f32 = links.iter().map(|(l, _)| ui.painter().layout_no_wrap(l.to_string(), egui::FontId::proportional(13.0), theme.accent).size().x).sum::<f32>() + 24.0;
                                ui.add_space(((ui.available_width() - width) / 2.0).max(0.0));
                                ui.spacing_mut().item_spacing.x = 24.0;
                                for (label, url) in links {
                                    ui.hyperlink_to(RichText::new(label).size(13.0).color(theme.accent), url);
                                }
                            });
                            ui.add_space(10.0);
                            kit::hint(ui, &theme, t!("Designed and directed by XCraftTM, built with AI assistance (Claude)."));
                            ui.hyperlink_to(RichText::new(t!("How XC Circuit is made")).size(11.5).color(theme.accent), "https://xcrafttm.github.io/XC-Circuit/made-with-ai");
                            ui.add_space(6.0);
                            kit::hint(ui, &theme, t!("Copyright © 2026 XCraftTM. All rights reserved."));
                            ui.horizontal(|ui| {
                                let licence = t!("Licence");
                                let notices = t!("Third-party licences");
                                let width = ui.painter().layout_no_wrap(format!("{licence}{notices}"), egui::FontId::proportional(11.5), theme.accent).size().x + 18.0;
                                ui.add_space(((ui.available_width() - width) / 2.0).max(0.0));
                                ui.spacing_mut().item_spacing.x = 18.0;
                                for (label, file) in [(licence, "LICENSE"), (notices, "THIRD-PARTY-NOTICES.txt")] {
                                    if ui.link(RichText::new(label).size(11.5).color(theme.accent)).clicked() {
                                        open_doc(app_root.as_deref(), file);
                                    }
                                }
                            });
                            ui.add_space(6.0);
                        });
                    }
                    dialog::Part::Footer => ok = kit::primary_button(ui, &theme, t!("Close")).clicked(),
                },
            );
            if ok || close {
                self.show_about = false;
            }
        }
        if self.confirm_close {
            let name = self.ed.project.as_ref().map(|p| p.meta.name.clone()).unwrap_or_default();
            let title = format!("Save changes to {name}?");
            let icon = self.ed.reg.icons.get("alert").cloned();
            let (mut save, mut discard, mut cancel) = (false, false, false);
            let close = dialog::show(
                ctx,
                &theme,
                dialog::Spec { id: "confirm-close", title: &title, subtitle: None, icon: icon.as_deref(), width: 460.0 },
                |ui, part| match part {
                    dialog::Part::Body => {
                        ui.label(RichText::new(t!("Your changes are lost if you close without saving.")).size(13.5).color(theme.text));
                        if self.bot.state != BotState::Stopped {
                            ui.add_space(4.0);
                            ui.label(RichText::new(t!("The running bot will be stopped.")).size(12.0).color(theme.text_muted));
                        }
                    }
                    dialog::Part::Footer => {
                        save = kit::primary_button(ui, &theme, t!("Save and close")).clicked();
                        discard = kit::secondary_button(ui, &theme, t!("Don't save")).clicked();
                        cancel = kit::secondary_button(ui, &theme, t!("Cancel")).clicked();
                    }
                },
            );
            if save || discard {
                if save {
                    self.ed.save();
                } else if let Some(p) = &self.ed.project {
                    // Chosen on purpose: no recovery offer next time.
                    self.ed.backups.clear_recovery(&p.root);
                }
                self.allow_close = true;
                ctx.send_viewport_cmd(ViewportCommand::Close);
            } else if cancel || close {
                self.confirm_close = false;
            }
        }
    }

    /// A finished update check: an update shows in the status bar (and its
    /// window, when asked for by hand); by hand, "the latest" goes to the console.
    pub(super) fn poll_updates(&mut self, ctx: &egui::Context) {
        use crate::app::updater::Found;
        if self.updater.busy() {
            ctx.request_repaint_after(std::time::Duration::from_millis(500));
        }
        match self.updater.poll(self.ed.prefs.skipped_update.as_deref()) {
            Some(Found::UpToDate) => self.ed.log(Level::Info, tf!("XC Circuit {version} is the latest version (or GitHub couldn't be asked)", version = env!("CARGO_PKG_VERSION"))),
            Some(Found::Newer) | None => {}
        }
        match self.updater.poll_download() {
            // Waiting in the update window: close now (asking about unsaved
            // changes as always); the update is installed as the app closes.
            Some(Ok(_)) if self.updater.show => {
                self.updater.show = false;
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
            Some(Ok(_)) => {
                let version = self.updater.available.as_ref().map(|r| r.version.clone()).unwrap_or_default();
                self.ed.log(Level::Success, tf!("XC Circuit {version} is downloaded: it's installed when you close XC Circuit", version = version));
            }
            Some(Err(e)) => self.ed.log(Level::Error, tf!("Couldn't download the update: {error}. The release page has it too (Help › Check for Updates).", error = e)),
            None => {}
        }
        // Block updates: what's offered for the plugins loaded now.
        if self.block_updates.busy() {
            ctx.request_repaint_after(std::time::Duration::from_millis(300));
        }
        let loaded: Vec<(String, String)> = self.ed.reg.plugins.iter().map(|p| (p.manifest.id.clone(), p.manifest.version.clone())).collect();
        if let Some(crate::plugins::updates::Found::None) = self.block_updates.poll(&loaded) {
            self.ed.log(Level::Info, t!("The blocks are up to date (or GitHub couldn't be asked)"));
        }
        match self.block_updates.poll_install() {
            Some(Ok(done)) => {
                self.reload_plugins(ctx);
                self.ed.log(Level::Success, tf!("Blocks updated: {plugins}", plugins = done.join(", ")));
            }
            Some(Err(e)) => self.ed.log(Level::Error, tf!("Couldn't update the blocks: {error}", error = e)),
            None => {}
        }
    }
}
