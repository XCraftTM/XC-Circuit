//! Running the bot from the editor, its Settings dialog and Export.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, channel};
use std::thread;
use std::time::{Duration, Instant};

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
    /// Kept in the system keyring instead of the project.
    secret: bool,
    /// Typed like a password, with an eye button to show it (every secret is).
    masked: bool,
    required: bool,
    /// The value must match this, when not empty (`pattern` in the build mode).
    pattern: Option<regex::Regex>,
    /// Shown when the value doesn't match (`patternHint`).
    pattern_hint: Option<String>,
    value: String,
    /// Secrets only: whether one is already stored.
    saved: bool,
    reveal: bool,
    remove: bool,
    /// When the Copy button was last used, to show that it worked.
    copied: Option<Instant>,
}

/// How long the Copy button shows a check mark.
const COPIED_FOR: Duration = Duration::from_millis(1500);

/// A field pattern must match the whole value.
fn compile_pattern(pattern: &str) -> Result<regex::Regex, regex::Error> {
    regex::Regex::new(&format!("^(?:{pattern})$"))
}

/// Why a typed value isn't accepted, if it isn't. Empty values are left to `required`.
fn pattern_problem(pattern: Option<&regex::Regex>, hint: Option<&str>, value: &str) -> Option<String> {
    let value = value.trim();
    match pattern {
        Some(re) if !value.is_empty() && !re.is_match(value) => Some(hint.unwrap_or(t!("This doesn't look right.")).to_string()),
        _ => None,
    }
}

impl SettingField {
    fn problem(&self) -> Option<String> {
        pattern_problem(self.pattern.as_ref(), self.pattern_hint.as_deref(), &self.value)
    }
}

pub struct SettingsDraft {
    fields: Vec<SettingField>,
    project_name: String,
}

pub struct ExportDraft {
    dir: PathBuf,
    include_token: bool,
    install: bool,
    token_saved: bool,
    name: String,
}

pub struct BotUi {
    pub state: BotState,
    session: Option<Session>,
    pub settings: Option<SettingsDraft>,
    pub export: Option<ExportDraft>,
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
            ed.log(Level::Info, t!("Bot stopped"));
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

    pub fn open_settings(&mut self, ed: &mut Editor) {
        let (Some(project), Some(mode)) = (&ed.project, ed.mode()) else { return };
        let text = |spec: &Value, key: &str| spec.get(key).and_then(Value::as_str).map(str::to_string);
        let tr = |s: &str| crate::i18n::tr(s).into_owned();
        let mut bad_patterns = Vec::new();
        let fields = mode
            .settings
            .iter()
            .map(|(key, spec)| {
                let secret = text(spec, "type").as_deref() == Some("secret");
                let pattern = text(spec, "pattern").and_then(|p| match compile_pattern(&p) {
                    Ok(re) => Some(re),
                    Err(e) => {
                        bad_patterns.push(format!("The build mode's pattern for setting {key} doesn't work, so it isn't checked: {e}"));
                        None
                    }
                });
                let value = if secret {
                    String::new()
                } else {
                    project.meta.settings.get(key).and_then(Value::as_str).map(str::to_string).or_else(|| text(spec, "default")).unwrap_or_default()
                };
                SettingField {
                    key: key.clone(),
                    group: text(spec, "group").map(|g| tr(&g)).unwrap_or_else(|| t!("General").into()),
                    label: text(spec, "label").map(|l| tr(&l)).unwrap_or_else(|| key.clone()),
                    description: text(spec, "description").map(|d| tr(&d)),
                    placeholder: text(spec, "placeholder").map(|p| tr(&p)).unwrap_or_default(),
                    secret,
                    masked: secret || spec.get("sensitive").and_then(Value::as_bool).unwrap_or(false),
                    required: spec.get("required").and_then(Value::as_bool).unwrap_or(false),
                    pattern,
                    pattern_hint: text(spec, "patternHint").map(|h| tr(&h)),
                    value,
                    saved: secret && secrets::get(&project.root, key).is_some(),
                    reveal: false,
                    remove: false,
                    copied: None,
                }
            })
            .collect();
        self.settings = Some(SettingsDraft { fields, project_name: project.meta.name.clone() });
        for line in bad_patterns {
            ed.log(Level::Warn, line);
        }
    }

    pub fn open_export(&mut self, ed: &Editor) {
        let Some(project) = &ed.project else { return };
        let folder = circuit_runner::export::folder_name(&project.meta.name);
        let base = dirs::document_dir().unwrap_or_else(|| project.root.clone()).join("XC Circuit Exports");
        let token_saved = secrets::get(&project.root, "token").is_some();
        self.export = Some(ExportDraft { dir: base.join(folder), include_token: token_saved, install: true, token_saved, name: project.meta.name.clone() });
    }

    pub fn windows(&mut self, ctx: &egui::Context, ed: &mut Editor) {
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
            dialog::Spec { id: "bot-question", title: t!("Your bot needs a decision"), subtitle: Some(t!("It waits until you choose")), icon: icon.as_deref(), width: 520.0 },
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
                    if dialog::secondary_button(ui, &theme, t!("Stop the bot")).clicked() {
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

    /// The Bot settings tab.
    pub fn settings_tab(&mut self, ui: &mut egui::Ui, ed: &mut Editor) {
        let Some(draft) = &mut self.settings else { return };
        let theme = ed.theme.clone();
        let icon = ed.reg.icons.get("settings").cloned();
        let subtitle = tf!("{project} · stored per project", project = draft.project_name);
        let store = if cfg!(windows) { t!("Windows Credential Manager") } else { t!("the system keyring") };
        let root = ed.project.as_ref().map(|p| p.root.clone());
        let icon_of = |name: &str| ed.reg.icons.get(name).cloned();
        let (eye, eye_off, copy, check) = (icon_of("eye"), icon_of("eye-off"), icon_of("copy"), icon_of("check"));
        let problems = draft.fields.iter().filter(|f| f.problem().is_some()).count();
        let (mut save, mut close, mut cancel) = (false, false, false);
        dialog::tab(
            ui,
            &theme,
            dialog::Spec { id: "bot-settings", title: t!("Bot settings"), subtitle: Some(&subtitle), icon: icon.as_deref(), width: 520.0 },
            |ui, part| match part {
                Part::Body => {
                    let mut group: Option<String> = None;
                    for f in &mut draft.fields {
                        if group.as_ref() != Some(&f.group) {
                            dialog::section(ui, &theme, &f.group);
                            group = Some(f.group.clone());
                        }
                        let label = if f.required { format!("{} *", f.label) } else { f.label.clone() };
                        let description = f.description.clone();
                        dialog::field(ui, &theme, &label, description.as_deref(), |ui| {
                            if !f.masked {
                                dialog::text_input(ui, &mut f.value, &f.placeholder, false);
                            } else {
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 6.0;
                                    let hint = if f.saved && !f.remove { t!("•••••••• saved · type to replace") } else { f.placeholder.as_str() };
                                    ui.allocate_ui(egui::vec2(ui.available_width() - 80.0, 34.0), |ui| {
                                        dialog::text_input(ui, &mut f.value, hint, !f.reveal);
                                    });
                                    let (icon, tip) = if f.reveal { (eye_off.as_deref(), t!("Hide")) } else { (eye.as_deref(), t!("Show what you typed")) };
                                    if dialog::icon_button(ui, &theme, icon, if f.reveal { t!("Hide") } else { "Show" }, tip).clicked() {
                                        f.reveal = !f.reveal;
                                    }
                                    // Copies what was typed, or else the saved value, without showing it.
                                    let saved_value = || if f.secret && f.saved { root.as_deref().and_then(|r| secrets::get(r, &f.key)) } else { None };
                                    let can_copy = !f.value.is_empty() || (f.saved && !f.remove);
                                    let copied = f.copied.is_some_and(|t| t.elapsed() < COPIED_FOR);
                                    let (icon, tip) = if copied { (check.as_deref(), t!("Copied")) } else { (copy.as_deref(), t!("Copy without showing it")) };
                                    if ui.add_enabled_ui(can_copy, |ui| dialog::icon_button(ui, &theme, icon, "Copy", tip)).inner.clicked() {
                                        let value = if f.value.is_empty() { saved_value() } else { Some(f.value.trim().to_string()) };
                                        if let Some(v) = value {
                                            ui.ctx().copy_text(v);
                                            f.copied = Some(Instant::now());
                                            ui.ctx().request_repaint_after(COPIED_FOR);
                                        }
                                    }
                                });
                            }
                            if let Some(p) = f.problem() {
                                ui.add_space(2.0);
                                ui.label(RichText::new(p).size(11.5).color(theme.error));
                            }
                            if !f.secret {
                                return;
                            }
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                if f.remove {
                                    dialog::pill(ui, t!("Will be removed"), theme.error);
                                } else if f.saved {
                                    dialog::pill(ui, t!("Saved"), theme.action);
                                } else {
                                    dialog::pill(ui, t!("Not set"), theme.warning);
                                }
                                if f.saved && dialog::button(ui, &theme, if f.remove { t!("Keep it") } else { t!("Remove saved value") }).clicked() {
                                    f.remove = !f.remove;
                                }
                            });
                            ui.add_space(2.0);
                            ui.label(RichText::new(tf!("Kept in {store}, never in project files.", store = store)).size(11.5).color(theme.text_muted));
                        });
                    }
                }
                Part::Footer => {
                    ui.add_enabled_ui(problems == 0, |ui| {
                        if dialog::primary_button(ui, &theme, t!("Save and close")).clicked() {
                            (save, close) = (true, true);
                        }
                        if dialog::secondary_button(ui, &theme, t!("Save")).clicked() {
                            save = true;
                        }
                    });
                    cancel = dialog::secondary_button(ui, &theme, t!("Cancel")).clicked();
                    if problems > 0 {
                        ui.label(RichText::new(tf!("{count} to fix", count = problems)).size(12.0).color(theme.warning));
                    }
                }
            },
        );
        if save && problems == 0 {
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
            ed.log(Level::Success, t!("Settings saved"));
            if close {
                self.settings = None;
            } else {
                // Stay open, showing what is stored now (a typed token becomes "Saved").
                self.open_settings(ed);
            }
        } else if cancel {
            self.settings = None;
        }
    }

    /// The Export bot tab.
    pub fn export_tab(&mut self, ui: &mut egui::Ui, ed: &mut Editor) {
        let Some(draft) = &mut self.export else { return };
        let theme = ed.theme.clone();
        let icon = ed.reg.icons.get("package").cloned();
        let (mut go, mut cancel) = (false, false);
        dialog::tab(
            ui,
            &theme,
            dialog::Spec {
                id: "export-bot",
                title: t!("Export bot"),
                subtitle: Some(t!("A folder that runs anywhere with Node.js 22+")),
                icon: icon.as_deref(),
                width: 540.0,
            },
            |ui, part| match part {
                Part::Body => {
                    dialog::section(ui, &theme, t!("Destination"));
                    dialog::field(ui, &theme, t!("Folder"), Some(t!("Created if it doesn't exist. It must be empty, or a folder you exported a bot to before.")), |ui| {
                        ui.horizontal(|ui| {
                            ui.allocate_ui(egui::vec2(ui.available_width() - 104.0, 34.0), |ui| dialog::readout(ui, &theme, &draft.dir.display().to_string()));
                            if dialog::secondary_button(ui, &theme, t!("Change…")).clicked()
                                && let Some(parent) = rfd::FileDialog::new().set_title(t!("Choose where to export the bot")).pick_folder()
                            {
                                draft.dir = parent.join(circuit_runner::export::folder_name(&draft.name));
                            }
                        });
                    });

                    dialog::section(ui, &theme, t!("Options"));
                    let token_text = if draft.token_saved {
                        t!("Writes DISCORD_TOKEN to the folder's .env file. Never share or upload that file.")
                    } else {
                        t!("No token saved yet (Bot › Settings). An empty .env is created for you to fill in.")
                    };
                    ui.add_enabled_ui(draft.token_saved, |ui| {
                        dialog::toggle_row(ui, &theme, &mut draft.include_token, t!("Include the bot token"), token_text);
                    });
                    dialog::toggle_row(ui, &theme, &mut draft.install, t!("Install npm packages now"), t!("Runs npm install in the folder so it's ready to start. Needs internet."));

                    dialog::section(ui, &theme, t!("What you get"));
                    egui::Frame::new().fill(theme.field).corner_radius(8).inner_margin(egui::Margin::symmetric(12, 10)).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        for (file, what) in [
                            ("index.js", t!("your workspaces, blocks and the Circuit runtime")),
                            ("package.json", t!("npm packages the bot needs")),
                            (".env", t!("the bot token")),
                            ("README.txt", t!("how to start it: node index.js")),
                        ] {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(file).monospace().size(12.0).color(theme.text));
                                ui.label(RichText::new(what).size(11.5).color(theme.text_muted));
                            });
                        }
                    });
                }
                Part::Footer => {
                    go = dialog::primary_button(ui, &theme, t!("Export")).clicked();
                    cancel = dialog::secondary_button(ui, &theme, t!("Cancel")).clicked();
                }
            },
        );
        if go {
            let draft = self.export.take().unwrap();
            self.run_export(ed, draft);
        } else if cancel {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patterns_match_the_whole_value() {
        let re = compile_pattern(r"\d{3}").unwrap();
        assert_eq!(pattern_problem(Some(&re), None, "123"), None);
        assert_eq!(pattern_problem(Some(&re), None, " 123 "), None, "surrounding spaces are trimmed, as on save");
        assert!(pattern_problem(Some(&re), None, "1234").is_some(), "anchored at both ends");
        assert_eq!(pattern_problem(Some(&re), Some("Three digits"), "12").as_deref(), Some("Three digits"));
        assert_eq!(pattern_problem(Some(&re), None, ""), None, "empty is left to required");
        assert!(compile_pattern("(").is_err());
    }

    #[test]
    fn discord_settings_check_their_values() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../plugins/xc.discord/modes/discord.toml");
        let mode: circuit_plugins::BuildMode = toml::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let check = |key: &str, value: &str| {
            let spec = &mode.settings[key];
            let re = compile_pattern(spec["pattern"].as_str().unwrap()).unwrap();
            pattern_problem(Some(&re), spec.get("patternHint").and_then(Value::as_str), value)
        };
        // Made-up values in the shape Discord uses.
        assert_eq!(check("token", "MTIzNDU2Nzg5MDEyMzQ1Njc4OQ.GhIjKl.abcdefghijklmnopqrstuvwxyz0123456789AB"), None);
        assert!(check("token", "not a token").is_some());
        assert!(check("token", "abc.def").is_some());
        assert_eq!(check("devServer", "123456789012345678"), None);
        assert!(check("devServer", "12345").is_some());
        assert!(check("devServer", "my server").is_some());
        assert_eq!(check("prefix", "!"), None);
        assert!(check("prefix", "a b").is_some());
    }
}
