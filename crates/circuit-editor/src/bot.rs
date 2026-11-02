//! Running the bot from the editor, its Settings dialog and Export.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, channel};
use std::thread;
use std::time::Instant;

use circuit_runner::{BotEvent, ExportOptions, Msg, Session, SessionConfig, build_bundle, secrets};
use eframe::egui::{self, RichText};
use serde_json::Value;

use crate::dialog::{self, Part};
use crate::editor::{Editor, Level};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BotState {
    Stopped,
    Starting,
    Running,
}

/// Something a block did recently, for highlighting on the canvas.
#[derive(Clone, Debug)]
pub struct Activity {
    pub at: Instant,
    pub error: Option<String>,
}

/// One row of the Settings dialog, read from the build mode's `[settings]`.
struct SettingField {
    key: String,
    group: String,
    label: String,
    description: Option<String>,
    placeholder: String,
    secret: bool,
    required: bool,
    value: String,
    /// Secrets only: whether one is already stored, and whether to show it while typing.
    saved: bool,
    reveal: bool,
    remove: bool,
}

struct SettingsDraft {
    fields: Vec<SettingField>,
    project_name: String,
}

struct ExportDraft {
    dir: PathBuf,
    include_token: bool,
    install: bool,
    token_saved: bool,
    name: String,
}

pub struct BotUi {
    pub state: BotState,
    session: Option<Session>,
    settings: Option<SettingsDraft>,
    export: Option<ExportDraft>,
    jobs: Vec<Receiver<(Level, String)>>,
    /// What the bot did since the last frame, for native plugins.
    pub events: Vec<xc_circuit_plugin::BotEvent>,
    /// A question from the bot, waiting for an answer.
    question: Option<(String, String, Vec<circuit_runner::protocol::Choice>)>,
}

impl BotUi {
    pub fn new() -> Self {
        BotUi { state: BotState::Stopped, session: None, settings: None, export: None, jobs: Vec::new(), events: Vec::new(), question: None }
    }

    pub fn start(&mut self, ed: &mut Editor, ctx: &egui::Context) {
        self.stop(ed);
        let Some(project) = &ed.project else { return };
        let mode = ed.mode();
        let bundle = match build_bundle(project, &ed.reg, mode) {
            Ok(b) => b,
            Err(e) => return ed.log(Level::Error, format!("Could not prepare the bot: {e:#}")),
        };
        let mut env = Vec::new();
        for (key, spec) in mode.map(|m| m.settings.iter().collect::<Vec<_>>()).unwrap_or_default() {
            if spec.get("type").and_then(Value::as_str) == Some("secret")
                && let Some(v) = secrets::get(&project.root, key)
            {
                env.push((format!("XC_SECRET_{}", key.to_uppercase()), v));
            }
        }
        let cfg = SessionConfig {
            bundle: bundle.value,
            dependencies: bundle.dependencies,
            packages_dir: app_data_dir().join("packages"),
            run_dir: project.root.join(".circuit").join("run"),
            data_dir: project.root.join(".circuit").join("data"),
            env,
        };
        let repaint = ctx.clone();
        self.session = Some(Session::start(cfg, move || repaint.request_repaint()));
        self.state = BotState::Starting;
        ed.activity.clear();
        ed.log(Level::Info, format!("Starting {} …", project.meta.name));
    }

    pub fn stop(&mut self, ed: &mut Editor) {
        if let Some(s) = self.session.take() {
            s.stop();
            ed.log(Level::Info, "Bot stopped");
        }
        self.state = BotState::Stopped;
    }

    /// Restarts a running bot, used after saving.
    /// After a save: restarts a running bot, unless that is turned off in Settings.
    pub fn restart_if_running(&mut self, ed: &mut Editor, ctx: &egui::Context) {
        if self.state != BotState::Stopped && ed.prefs.auto_restart {
            self.start(ed, ctx);
        }
    }

    /// Moves bot output into the console and the canvas highlights.
    pub fn poll(&mut self, ed: &mut Editor) {
        let msgs = self.session.as_ref().map(|s| s.poll()).unwrap_or_default();
        for m in msgs {
            match m {
                Msg::Info(t) => ed.log(Level::Info, t),
                Msg::Error(t) => ed.log(Level::Error, t),
                Msg::Started => {
                    self.state = BotState::Running;
                    self.events.push(xc_circuit_plugin::BotEvent::Started);
                }
                Msg::Exited(code) => {
                    self.state = BotState::Stopped;
                    self.session = None;
                    self.events.push(xc_circuit_plugin::BotEvent::Stopped);
                    match code {
                        Some(0) | None => {}
                        Some(c) => ed.log(Level::Warn, format!("Bot process ended (exit code {c})")),
                    }
                }
                Msg::Bot(BotEvent::Ask { id, text, choices }) if !choices.is_empty() => {
                    ed.log(Level::Warn, format!("The bot asks: {text}"));
                    self.question = Some((id, text, choices));
                }
                Msg::Bot(ev) => {
                    if let Some(e) = plugin_event(&ev) {
                        self.events.push(e);
                    }
                    handle_event(ed, ev);
                }
            }
        }
        self.jobs.retain(|rx| {
            loop {
                match rx.try_recv() {
                    Ok((level, text)) => ed.log(level, text),
                    Err(std::sync::mpsc::TryRecvError::Empty) => return true,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => return false,
                }
            }
        });
    }

    pub fn open_settings(&mut self, ed: &Editor) {
        let (Some(project), Some(mode)) = (&ed.project, ed.mode()) else { return };
        let text = |spec: &Value, key: &str| spec.get(key).and_then(Value::as_str).map(str::to_string);
        let fields = mode
            .settings
            .iter()
            .map(|(key, spec)| {
                let secret = text(spec, "type").as_deref() == Some("secret");
                let value = if secret {
                    String::new()
                } else {
                    project.meta.settings.get(key).and_then(Value::as_str).map(str::to_string).or_else(|| text(spec, "default")).unwrap_or_default()
                };
                SettingField {
                    key: key.clone(),
                    group: text(spec, "group").unwrap_or_else(|| "General".into()),
                    label: text(spec, "label").unwrap_or_else(|| key.clone()),
                    description: text(spec, "description"),
                    placeholder: text(spec, "placeholder").unwrap_or_default(),
                    secret,
                    required: spec.get("required").and_then(Value::as_bool).unwrap_or(false),
                    value,
                    saved: secret && secrets::get(&project.root, key).is_some(),
                    reveal: false,
                    remove: false,
                }
            })
            .collect();
        self.settings = Some(SettingsDraft { fields, project_name: project.meta.name.clone() });
    }

    pub fn open_export(&mut self, ed: &Editor) {
        let Some(project) = &ed.project else { return };
        let folder = circuit_runner::export::folder_name(&project.meta.name);
        let base = dirs::document_dir().unwrap_or_else(|| project.root.clone()).join("XC Circuit Exports");
        let token_saved = secrets::get(&project.root, "token").is_some();
        self.export = Some(ExportDraft { dir: base.join(folder), include_token: token_saved, install: true, token_saved, name: project.meta.name.clone() });
    }

    pub fn windows(&mut self, ctx: &egui::Context, ed: &mut Editor) {
        self.settings_window(ctx, ed);
        self.export_window(ctx, ed);
        self.question_window(ctx, ed);
    }

    /// The bot's question, with one button per answer. The bot waits for it.
    fn question_window(&mut self, ctx: &egui::Context, ed: &mut Editor) {
        if self.state == BotState::Stopped {
            self.question = None;
        }
        let Some((id, text, choices)) = self.question.clone() else { return };
        let theme = ed.theme.clone();
        let icon = ed.reg.icons.get("database").cloned();
        let mut picked: Option<(String, String)> = None;
        dialog::show(
            ctx,
            &theme,
            dialog::Spec { id: "bot-question", title: "Your bot needs a decision", subtitle: Some("It waits until you choose"), icon: icon.as_deref(), width: 520.0 },
            |ui, part| match part {
                Part::Body => {
                    ui.label(RichText::new(&text).size(13.5).color(theme.text));
                    ui.add_space(10.0);
                    for (i, c) in choices.iter().enumerate() {
                        let b = if i == 0 { dialog::primary_button(ui, &theme, &c.label) } else { dialog::secondary_button(ui, &theme, &c.label) };
                        if b.clicked() {
                            picked = Some((c.value.clone(), c.label.clone()));
                        }
                        ui.add_space(4.0);
                    }
                }
                Part::Footer => {
                    if dialog::secondary_button(ui, &theme, "Stop the bot").clicked() {
                        picked = Some((String::new(), String::new()));
                    }
                }
            },
        );
        match picked {
            Some((value, _)) if value.is_empty() => self.stop(ed),
            Some((value, label)) => {
                if let Some(s) = &self.session {
                    s.answer(&id, &value);
                }
                ed.log(Level::Info, format!("You chose: {label}"));
                self.question = None;
            }
            None => {}
        }
    }

    fn settings_window(&mut self, ctx: &egui::Context, ed: &mut Editor) {
        let Some(draft) = &mut self.settings else { return };
        let theme = ed.theme.clone();
        let icon = ed.reg.icons.get("settings").cloned();
        let subtitle = format!("{} · stored per project", draft.project_name);
        let store = if cfg!(windows) { "Windows Credential Manager" } else { "the system keyring" };
        let (mut save, mut cancel) = (false, false);
        let close = dialog::show(
            ctx,
            &theme,
            dialog::Spec { id: "bot-settings", title: "Bot settings", subtitle: Some(&subtitle), icon: icon.as_deref(), width: 520.0 },
            |ui, part| match part {
                Part::Body => {
                    let mut group: Option<String> = None;
                    for f in &mut draft.fields {
                        if group.as_ref() != Some(&f.group) {
                            dialog::section(ui, &theme, &f.group);
                            group = Some(f.group.clone());
                        }
                        let label = if f.required { format!("{} *", f.label) } else { f.label.clone() };
                        dialog::field(ui, &theme, &label, f.description.as_deref(), |ui| {
                            if !f.secret {
                                dialog::text_input(ui, &mut f.value, &f.placeholder, false);
                                return;
                            }
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 8.0;
                                let hint = if f.saved && !f.remove { "•••••••• saved · type to replace" } else { f.placeholder.as_str() };
                                ui.allocate_ui(egui::vec2(ui.available_width() - 92.0, 34.0), |ui| {
                                    dialog::text_input(ui, &mut f.value, hint, !f.reveal);
                                });
                                let eye = if f.reveal { "Hide" } else { "Show" };
                                if dialog::secondary_button(ui, &theme, eye).clicked() {
                                    f.reveal = !f.reveal;
                                }
                            });
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                if f.remove {
                                    dialog::pill(ui, "Will be removed", theme.error);
                                } else if f.saved {
                                    dialog::pill(ui, "Saved", theme.action);
                                } else {
                                    dialog::pill(ui, "Not set", theme.warning);
                                }
                                if f.saved && ui.link(RichText::new(if f.remove { "Keep it" } else { "Remove saved value" }).size(11.5)).clicked() {
                                    f.remove = !f.remove;
                                }
                            });
                            ui.add_space(2.0);
                            ui.label(RichText::new(format!("Kept in {store}, never in project files.")).size(11.5).color(theme.text_muted));
                        });
                    }
                }
                Part::Footer => {
                    save = dialog::primary_button(ui, &theme, "Save").clicked();
                    cancel = dialog::secondary_button(ui, &theme, "Cancel").clicked();
                }
            },
        );
        if save {
            if let Some(project) = ed.project.as_mut() {
                let mut errors = Vec::new();
                for f in &draft.fields {
                    if f.secret {
                        let result = if f.remove && f.value.is_empty() {
                            secrets::set(&project.root, &f.key, "")
                        } else if !f.value.is_empty() {
                            secrets::set(&project.root, &f.key, f.value.trim())
                        } else {
                            Ok(())
                        };
                        if let Err(e) = result {
                            errors.push(format!("{}: {e:#}", f.label));
                        }
                    } else if project.meta.settings.get(&f.key).and_then(Value::as_str) != Some(f.value.as_str()) {
                        project.meta.settings.insert(f.key.clone(), Value::String(f.value.trim().to_string()));
                        ed.dirty = true;
                    }
                }
                for e in errors {
                    ed.log.push(crate::editor::LogLine::new(Level::Error, e));
                }
            }
            ed.log(Level::Success, "Settings saved");
            self.settings = None;
        } else if cancel || close {
            self.settings = None;
        }
    }

    fn export_window(&mut self, ctx: &egui::Context, ed: &mut Editor) {
        let Some(draft) = &mut self.export else { return };
        let theme = ed.theme.clone();
        let icon = ed.reg.icons.get("package").cloned();
        let (mut go, mut cancel) = (false, false);
        let close = dialog::show(
            ctx,
            &theme,
            dialog::Spec {
                id: "export-bot",
                title: "Export bot",
                subtitle: Some("A folder that runs anywhere with Node.js 22+"),
                icon: icon.as_deref(),
                width: 540.0,
            },
            |ui, part| match part {
                Part::Body => {
                    dialog::section(ui, &theme, "Destination");
                    dialog::field(ui, &theme, "Folder", Some("Created if it doesn't exist. It must be empty, or a folder you exported a bot to before."), |ui| {
                        ui.horizontal(|ui| {
                            ui.allocate_ui(egui::vec2(ui.available_width() - 104.0, 34.0), |ui| dialog::readout(ui, &theme, &draft.dir.display().to_string()));
                            if dialog::secondary_button(ui, &theme, "Change…").clicked()
                                && let Some(parent) = rfd::FileDialog::new().set_title("Choose where to export the bot").pick_folder()
                            {
                                draft.dir = parent.join(circuit_runner::export::folder_name(&draft.name));
                            }
                        });
                    });

                    dialog::section(ui, &theme, "Options");
                    let token_text = if draft.token_saved {
                        "Writes DISCORD_TOKEN to the folder's .env file. Never share or upload that file."
                    } else {
                        "No token saved yet (Bot › Settings). An empty .env is created for you to fill in."
                    };
                    ui.add_enabled_ui(draft.token_saved, |ui| {
                        dialog::toggle_row(ui, &theme, &mut draft.include_token, "Include the bot token", token_text);
                    });
                    dialog::toggle_row(ui, &theme, &mut draft.install, "Install npm packages now", "Runs npm install in the folder so it's ready to start. Needs internet.");

                    dialog::section(ui, &theme, "What you get");
                    egui::Frame::new().fill(theme.field).corner_radius(8).inner_margin(egui::Margin::symmetric(12, 10)).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        for (file, what) in [
                            ("index.js", "your workspaces, blocks and the Circuit runtime"),
                            ("package.json", "npm packages the bot needs"),
                            (".env", "the bot token"),
                            ("README.txt", "how to start it: node index.js"),
                        ] {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(file).monospace().size(12.0).color(theme.text));
                                ui.label(RichText::new(what).size(11.5).color(theme.text_muted));
                            });
                        }
                    });
                }
                Part::Footer => {
                    go = dialog::primary_button(ui, &theme, "Export").clicked();
                    cancel = dialog::secondary_button(ui, &theme, "Cancel").clicked();
                }
            },
        );
        if go {
            let draft = self.export.take().unwrap();
            self.run_export(ed, draft);
        } else if cancel || close {
            self.export = None;
        }
    }

    fn run_export(&mut self, ed: &mut Editor, draft: ExportDraft) {
        let Some(project) = &ed.project else { return };
        let bundle = match build_bundle(project, &ed.reg, ed.mode()) {
            Ok(b) => b,
            Err(e) => return ed.log(Level::Error, format!("Export failed: {e:#}")),
        };
        let token = if draft.include_token { secrets::get(&project.root, "token") } else { None };
        let opts = ExportOptions { name: project.meta.name.clone(), token, install: draft.install };
        let dir = draft.dir.clone();
        let (tx, rx) = channel();
        self.jobs.push(rx);
        ed.log(Level::Info, format!("Exporting to {} …", dir.display()));
        thread::spawn(move || {
            let mut log = |line: String| {
                let _ = tx.send((Level::Info, line));
            };
            match circuit_runner::export(&dir, &bundle, &opts, &mut log) {
                Ok(()) => {
                    let _ = tx.send((Level::Success, format!("Exported to {} · start it with: node index.js", dir.display())));
                }
                Err(e) => {
                    let _ = tx.send((Level::Error, format!("Export failed: {e:#}")));
                }
            }
        });
    }
}

/// The bot's events in the shape native plugins get them.
fn plugin_event(ev: &BotEvent) -> Option<xc_circuit_plugin::BotEvent> {
    use xc_circuit_plugin::{BotEvent as P, Level as L};
    Some(match ev {
        BotEvent::Log { level, text, .. } => P::Log {
            level: match level.as_str() {
                "error" => L::Error,
                "warn" => L::Warn,
                "success" => L::Success,
                _ => L::Info,
            },
            text: text.clone(),
        },
        BotEvent::Node { ws, node, phase, ms, error } => P::Block { workspace: ws.clone(), node: node.clone(), phase: phase.clone(), ms: *ms, error: error.clone() },
        _ => return None,
    })
}

fn handle_event(ed: &mut Editor, ev: BotEvent) {
    match ev {
        BotEvent::Log { level, text, ws, node } => {
            let level = match level.as_str() {
                "success" => Level::Success,
                "warn" => Level::Warn,
                "error" => Level::Error,
                _ => Level::Info,
            };
            let mut line = crate::editor::LogLine::new(level, text);
            line.target = ws.zip(node);
            ed.log.push(line);
        }
        BotEvent::Node { ws, node, phase, error, .. } => {
            let key = (ws, node);
            match phase.as_str() {
                "error" => {
                    ed.activity.insert(key, Activity { at: Instant::now(), error });
                }
                _ => {
                    ed.activity.insert(key, Activity { at: Instant::now(), error: None });
                }
            }
        }
        BotEvent::Zone { ws, zone, phase, error } => {
            let error = if phase == "error" { error.or(Some("failed".into())) } else { None };
            ed.activity.insert((ws, zone), Activity { at: Instant::now(), error });
        }
        // Questions are taken in `poll`; one without choices has nothing to show.
        BotEvent::Ready {} | BotEvent::Ask { .. } => {}
        BotEvent::Plain(text) => {
            if !text.trim().is_empty() {
                ed.log(Level::Info, text);
            }
        }
    }
}

/// `%APPDATA%\XC Circuit` on Windows, `~/.local/share/xc-circuit` on Linux.
/// XC Circuit's own data: packages, backups, downloaded Node.js. The
/// `XC_CIRCUIT_DATA` environment variable moves it (portable installs, tests).
pub fn app_data_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("XC_CIRCUIT_DATA").filter(|d| !d.is_empty()) {
        return PathBuf::from(dir);
    }
    let name = if cfg!(windows) { "XC Circuit" } else { "xc-circuit" };
    dirs::data_dir().unwrap_or_else(std::env::temp_dir).join(name)
}
