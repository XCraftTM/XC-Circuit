//! Running the bot from the editor, its Settings dialog and Export.

pub mod profile;
pub mod servers;
pub(crate) mod databases;
pub mod git;
pub mod host;
pub mod hosting;
pub mod live;
pub mod remote;
mod settings;

pub use settings::{SettingsDraft, secret_names};

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, channel};
use std::thread;
use std::time::{Duration, Instant};

use circuit_runner::{BotEvent, ExportOptions, ExportSecret, Msg, Session, SessionConfig, build_bundle, secrets};
use eframe::egui::{self, RichText};

use crate::ui::kit;
use crate::ui::dialog::{self, Part};
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

/// How long ago something happened: "just now", "12 s ago", "3 min ago", "2 d ago".
pub fn ago(d: Duration) -> String {
    match d.as_secs() {
        0..=2 => t!("just now").to_string(),
        s @ 3..=59 => tf!("{seconds} s ago", seconds = s),
        s @ 60..=3599 => tf!("{minutes} min ago", minutes = s / 60),
        s @ 3600..=86_399 => tf!("{hours} h ago", hours = s / 3600),
        s => tf!("{days} d ago", days = s / 86_400),
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
    /// What the bot did since the last frame, for plugins' scripts.
    pub events: Vec<crate::plugins::api::BotEvent>,
    /// A question from the bot, waiting for an answer.
    question: Option<(String, String, Vec<circuit_runner::protocol::Choice>)>,
    /// Uploading to the bot's server, and a Calagopus server's console.
    pub hosting: hosting::HostingUi,
    /// File › Git…: the project in a Git repository.
    pub git: git::GitUi,
    /// File › Host…: this computer and an XC Circuit Host.
    pub host: host::HostUi,
    /// File › Live session…: working in one project together.
    pub live: live::LiveUi,
}

impl BotUi {
    pub fn new() -> Self {
        BotUi { state: BotState::Stopped, session: None, old: None, restart: None, settings: None, export: None, jobs: Vec::new(), events: Vec::new(), question: None, hosting: hosting::HostingUi::new(), git: git::GitUi::new(), host: host::HostUi::new(), live: live::LiveUi::new() }
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
        for (key, _) in settings::secret_keys(ed) {
            if let Some(v) = secrets::get(&project.root, &key) {
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
    /// Run (or Restart), as the Run button does it: here, or with Server
    /// picked beside it, on the bot's server.
    pub fn run_pressed(&mut self, ed: &mut Editor, ctx: &egui::Context) {
        // A guest's copy has no token, and two bots must not answer.
        if self.live.is_guest() {
            self.live.ask_power(ed, circuit_sync::live::Power::Start);
            return;
        }
        if self.hosting.remote {
            self.hosting.run_remote(ed);
        } else {
            self.start(ed, ctx);
        }
    }

    /// Stop, as the Stop button does it (see [`BotUi::run_pressed`]).
    pub fn stop_pressed(&mut self, ed: &mut Editor) {
        if self.live.is_guest() {
            self.live.ask_power(ed, circuit_sync::live::Power::Stop);
            return;
        }
        if self.hosting.remote {
            self.hosting.stop_remote(ed);
        } else {
            self.stop(ed);
        }
    }

    /// Start, but only when it's stopped (a shortcut for Start alone).
    pub fn start_pressed(&mut self, ed: &mut Editor, ctx: &egui::Context) {
        if self.shown_state() == BotState::Stopped {
            self.run_pressed(ed, ctx);
        }
    }

    /// Restart, but only when it runs (a shortcut for Restart alone).
    pub fn restart_pressed(&mut self, ed: &mut Editor, ctx: &egui::Context) {
        if self.shown_state() == BotState::Running {
            self.run_pressed(ed, ctx);
        }
    }

    /// The state Run and the status bar show: the server's with Server picked.
    pub fn shown_state(&self) -> BotState {
        match (&self.hosting.watch, self.hosting.remote) {
            (Some(w), true) => w.bot_state(),
            _ => self.state,
        }
    }

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
                    self.events.push(crate::plugins::api::BotEvent::Started);
                }
                Msg::Exited(code) => {
                    self.state = BotState::Stopped;
                    self.session = None;
                    self.events.push(crate::plugins::api::BotEvent::Stopped);
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
                        let b = if i == 0 { kit::primary_button(ui, &theme, &c.label) } else { kit::secondary_button(ui, &theme, &c.label) };
                        if b.clicked() {
                            picked = Some((c.value.clone(), c.label.clone()));
                        }
                        ui.add_space(4.0);
                    }
                }
                Part::Footer => {
                    if kit::secondary_button(ui, &theme, t!("Stop the bot")).clicked() {
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
                    kit::section(ui, &theme, t!("Destination"));
                    kit::field(ui, &theme, t!("Folder"), Some(t!("Created if it doesn't exist. It must be empty, or a folder you exported a bot to before.")), |ui| {
                        ui.horizontal(|ui| {
                            ui.allocate_ui(egui::vec2(ui.available_width() - 104.0, 34.0), |ui| kit::readout(ui, &theme, &draft.dir.display().to_string()));
                            if kit::secondary_button(ui, &theme, t!("Change…")).clicked()
                                && let Some(parent) = rfd::FileDialog::new().set_title(t!("Choose where to export the bot")).pick_folder()
                            {
                                draft.dir = parent.join(circuit_runner::export::folder_name(&draft.name));
                            }
                        });
                    });

                    kit::section(ui, &theme, t!("Options"));
                    let token_env = ed.mode().map(|m| m.token_env().to_string()).unwrap_or_else(|| "DISCORD_TOKEN".into());
                    let token_text = if draft.token_saved {
                        tf!("Writes {variable} and the other saved secrets to the folder's .env file. Never share or upload that file.", variable = token_env)
                    } else {
                        t!("No token saved yet (Bot › Settings). An empty .env is created for you to fill in.").to_string()
                    };
                    ui.add_enabled_ui(draft.token_saved, |ui| {
                        kit::toggle_row(ui, &theme, &mut draft.include_token, t!("Include the bot token and secrets"), &token_text);
                    });
                    kit::toggle_row(ui, &theme, &mut draft.install, t!("Install npm packages now"), t!("Runs npm install in the folder so it's ready to start. Needs internet."));

                    kit::section(ui, &theme, t!("What you get"));
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
                                kit::hint(ui, &theme, what);
                            });
                        }
                    });
                }
                Part::Footer => {
                    go = kit::primary_button(ui, &theme, t!("Export")).clicked();
                    cancel = kit::secondary_button(ui, &theme, t!("Cancel")).clicked();
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
        let opts = export_options(ed, draft.include_token, draft.install);
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

/// What Export writes for the open project: `.env` with the token and the
/// mode's other secrets (their values only with `include_secrets`).
pub(crate) fn export_options(ed: &Editor, include_secrets: bool, install: bool) -> ExportOptions {
    let project = ed.project.as_ref().expect("a project is open");
    let token = if include_secrets { secrets::get(&project.root, "token") } else { None };
    // The mode's other secrets, as lines in .env: with their values when the token goes too.
    let secrets_out = settings::secret_keys(ed)
        .into_iter()
        .filter(|(key, _)| key != "token")
        .map(|(key, label)| ExportSecret {
            env: format!("XC_SECRET_{}", key.to_uppercase()),
            value: if include_secrets { secrets::get(&project.root, &key) } else { None },
            label,
        })
        .collect();
    let token_env = ed.mode().map(|m| m.token_env().to_string()).unwrap_or_else(|| "DISCORD_TOKEN".into());
    ExportOptions { name: project.meta.name.clone(), token, token_env, secrets: secrets_out, install }
}

/// The bot's events in the shape plugins' scripts get them.
fn plugin_event(ev: &BotEvent) -> Option<crate::plugins::api::BotEvent> {
    use crate::plugins::api::{BotEvent as P, Level as L};
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
/// Where projects live that are kept in a Git repository and have no folder
/// of the user's choosing: complete copies, to work on without a network.
/// On Windows in the local app data, not the roaming one that follows the
/// user to other computers (the repository does that).
pub fn remotes_dir() -> PathBuf {
    if std::env::var_os("XC_CIRCUIT_DATA").is_some_and(|d| !d.is_empty()) {
        return app_data_dir().join("remotes");
    }
    let name = if cfg!(windows) { "XC Circuit" } else { "xc-circuit" };
    dirs::data_local_dir().unwrap_or_else(app_data_dir).join(name).join("remotes")
}

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
}
