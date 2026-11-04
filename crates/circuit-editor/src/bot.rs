//! Running the bot from the editor, its Settings dialog and Export.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, channel};
use std::thread;
use std::time::{Duration, Instant};

use circuit_runner::{BotEvent, ExportOptions, ExportSecret, Msg, Session, SessionConfig, build_bundle, secrets};
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
    /// How long the block took the last time it ran, and when that was.
    pub ms: Option<(f64, Instant)>,
}

/// How long a block glows after it ran.
pub const GLOW: Duration = Duration::from_millis(2000);
/// How long a block shows how long it took.
pub const TIME_SHOWN: Duration = Duration::from_secs(30);

impl Activity {
    /// How strongly it glows now: 1 when it just ran, 0 after [`GLOW`].
    pub fn glow(&self) -> f32 {
        if self.error.is_some() { 0.0 } else { (1.0 - self.at.elapsed().as_secs_f32() / GLOW.as_secs_f32()).max(0.0) }
    }

    /// Its last run time while it's still shown, with how visible it is
    /// (fading out over the last second).
    pub fn time_shown(&self) -> Option<(f64, f32)> {
        let (ms, at) = self.ms?;
        let left = TIME_SHOWN.saturating_sub(at.elapsed()).as_secs_f32();
        (left > 0.0).then_some((ms, left.min(1.0)))
    }

    /// Whether the canvas still changes: a glow fading or a time to hide.
    pub fn live(&self) -> bool {
        self.error.is_none() && self.at.elapsed() < GLOW || self.time_shown().is_some()
    }
}

/// How long ago something happened: "just now", "12 s ago", "3 min ago".
pub fn ago(d: Duration) -> String {
    match d.as_secs() {
        0..=2 => t!("just now").to_string(),
        s @ 3..=59 => tf!("{seconds} s ago", seconds = s),
        s @ 60..=3599 => tf!("{minutes} min ago", minutes = s / 60),
        s => tf!("{hours} h ago", hours = s / 3600),
    }
}

/// A run time as people read it: 0.4 ms, 12 ms, 1.25 s.
pub fn format_ms(ms: f64) -> String {
    if ms < 10.0 {
        format!("{ms:.1} ms")
    } else if ms < 1000.0 {
        format!("{ms:.0} ms")
    } else {
        format!("{:.2} s", ms / 1000.0)
    }
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
    /// The bot before a restart, still stopping. The new one starts once it
    /// has exited, so two never run at once (both answering commands, both
    /// writing the saved data).
    old: Option<Session>,
    /// Starts the bot when `old` has exited.
    restart: Option<SessionConfig>,
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
        BotUi { state: BotState::Stopped, session: None, old: None, restart: None, settings: None, export: None, jobs: Vec::new(), events: Vec::new(), question: None }
    }

    /// Starts the bot, or restarts it: a running bot is stopped first, and the
    /// new one starts once it has exited.
    pub fn start(&mut self, ed: &mut Editor, ctx: &egui::Context) {
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
        let name = project.meta.name.clone();
        self.state = BotState::Starting;
        self.question = None;
        ed.activity.clear();
        ed.wire_values.clear();
        if let Some(s) = self.session.take() {
            s.stop();
            // A bot still stopping from an earlier restart is waited for instead.
            if self.old.is_none() {
                self.old = Some(s);
            }
        }
        if self.old.is_some() {
            ed.log(Level::Info, format!("Restarting {name} …"));
            self.restart = Some(cfg);
        } else {
            ed.log(Level::Info, format!("Starting {name} …"));
            self.launch(cfg, ctx);
        }
    }

    fn launch(&mut self, cfg: SessionConfig, ctx: &egui::Context) {
        let repaint = ctx.clone();
        self.session = Some(Session::start(cfg, move || repaint.request_repaint()));
    }

    pub fn stop(&mut self, ed: &mut Editor) {
        self.restart = None;
        let stopped = self.session.take().or_else(|| self.old.take());
        if let Some(s) = stopped {
            s.stop();
            ed.log(Level::Info, t!("Bot stopped"));
        }
        // Dropping a stopping bot doesn't wait for it; it still ends.
        self.old = None;
        self.question = None;
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
    pub fn poll(&mut self, ed: &mut Editor, ctx: &egui::Context) {
        // The bot before a restart: its last lines, then the new one starts.
        let old = self.old.as_ref().map(|s| s.poll()).unwrap_or_default();
        for m in old {
            match m {
                Msg::Exited(_) => {
                    self.old = None;
                    if let Some(cfg) = self.restart.take() {
                        self.launch(cfg, ctx);
                    }
                }
                Msg::Error(t) => ed.log(Level::Error, t),
                Msg::Bot(ev) => handle_event(ed, ev),
                _ => {}
            }
        }
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
            let mut token_changed = false;
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
                        } else if f.key == "token" {
                            token_changed = true;
                        }
                    } else if project.meta.settings.get(&f.key).and_then(Value::as_str) != Some(f.value.as_str()) {
                        project.meta.settings.insert(f.key.clone(), Value::String(f.value.trim().to_string()));
                        ed.edited = true;
                    }
                }
                for e in errors {
                    ed.log(Level::Error, e);
                }
            }
            ed.log(Level::Success, t!("Settings saved"));
            // The previews show the bot of the token saved now.
            if token_changed {
                ed.fetch_bot_profile();
            }
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
                    let token_env = ed.mode().map(|m| m.token_env().to_string()).unwrap_or_else(|| "DISCORD_TOKEN".into());
                    let token_text = if draft.token_saved {
                        tf!("Writes {variable} and the other saved secrets to the folder's .env file. Never share or upload that file.", variable = token_env)
                    } else {
                        t!("No token saved yet (Bot › Settings). An empty .env is created for you to fill in.").to_string()
                    };
                    ui.add_enabled_ui(draft.token_saved, |ui| {
                        dialog::toggle_row(ui, &theme, &mut draft.include_token, t!("Include the bot token and secrets"), &token_text);
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
        // The mode's other secrets, as lines in .env: with their values when the token goes too.
        let secrets_out = ed
            .mode()
            .map(|m| {
                m.settings
                    .iter()
                    .filter(|(key, spec)| *key != "token" && spec.get("type").and_then(Value::as_str) == Some("secret"))
                    .map(|(key, spec)| ExportSecret {
                        env: format!("XC_SECRET_{}", key.to_uppercase()),
                        label: spec.get("label").and_then(Value::as_str).unwrap_or(key).to_string(),
                        value: if draft.include_token { secrets::get(&project.root, key) } else { None },
                    })
                    .collect()
            })
            .unwrap_or_default();
        let token_env = ed.mode().map(|m| m.token_env().to_string()).unwrap_or_else(|| "DISCORD_TOKEN".into());
        let opts = ExportOptions { name: project.meta.name.clone(), token, token_env, secrets: secrets_out, install: draft.install };
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
        BotEvent::Node { ws, node, phase, ms, error, .. } => P::Block { workspace: ws.clone(), node: node.clone(), phase: phase.clone(), ms: *ms, error: error.clone() },
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
            ed.push_log(line);
        }
        BotEvent::Node { ws, node, phase, error, ms, outputs } => {
            let now = Instant::now();
            for (pin, value) in outputs.into_iter().flatten() {
                ed.wire_values.insert((ws.clone(), node.clone(), pin), (value, now));
            }
            let key = (ws, node);
            // The last run time stays until the block finishes again.
            let last = ed.activity.get(&key).and_then(|a| a.ms);
            let activity = match phase.as_str() {
                "error" => Activity { at: now, error, ms: last },
                "exit" => Activity { at: now, error: None, ms: ms.map(|m| (m, now)).or(last) },
                _ => Activity { at: now, error: None, ms: last },
            };
            ed.activity.insert(key, activity);
        }
        BotEvent::Zone { ws, zone, phase, error, outputs } => {
            for (pin, value) in outputs.into_iter().flatten() {
                ed.wire_values.insert((ws.clone(), zone.clone(), pin), (value, Instant::now()));
            }
            let error = if phase == "error" { error.or(Some("failed".into())) } else { None };
            ed.activity.insert((ws, zone), Activity { at: Instant::now(), error, ms: None });
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
    fn blocks_show_how_long_they_took() {
        assert_eq!((format_ms(0.42), format_ms(12.4), format_ms(1250.0)), ("0.4 ms".into(), "12 ms".into(), "1.25 s".into()));
        let mut ed = Editor::new(Vec::new(), None).unwrap();
        let node = |phase: &str, ms: Option<f64>| BotEvent::Node { ws: "w".into(), node: "1".into(), phase: phase.into(), ms, error: None, outputs: None };
        handle_event(&mut ed, node("enter", None));
        assert!(ed.activity[&("w".into(), "1".into())].time_shown().is_none(), "no time before it finished");
        handle_event(&mut ed, node("exit", Some(12.0)));
        let a = &ed.activity[&("w".into(), "1".into())];
        assert_eq!(a.time_shown(), Some((12.0, 1.0)));
        assert!(a.glow() > 0.9 && a.live());
        // Running again keeps the last time until it finishes.
        handle_event(&mut ed, node("enter", None));
        assert_eq!(ed.activity[&("w".into(), "1".into())].time_shown().map(|t| t.0), Some(12.0));
        // After 30 seconds it's gone.
        let old = Instant::now() - TIME_SHOWN - Duration::from_millis(1);
        let a = Activity { at: old, error: None, ms: Some((12.0, old)) };
        assert!(a.time_shown().is_none() && a.glow() == 0.0 && !a.live());
    }

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
        // Put together here, so secret scanners don't take it for a real token.
        let fake_token = ["MTIzNDU2Nzg5MDEyMzQ1Njc4OQ", "GhIjKl", "abcdefghijklmnopqrstuvwxyz0123456789AB"].join(".");
        assert_eq!(check("token", &fake_token), None);
        assert!(check("token", "not a token").is_some());
        assert!(check("token", "abc.def").is_some());
        assert_eq!(check("devServer", "123456789012345678"), None);
        assert!(check("devServer", "12345").is_some());
        assert!(check("devServer", "my server").is_some());
        assert_eq!(check("prefix", "!"), None);
        assert!(check("prefix", "a b").is_some());
    }
}
