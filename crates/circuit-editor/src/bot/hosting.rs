//! Putting the bot on its server: Bot › Remote… sets where it goes (SFTP
//! or a Calagopus panel) and uploads it, by hand or after every save.
//! For a Calagopus server there's also Bot › Remote console: its state,
//! Start / Restart / Stop, and its live console.
//!
//! The settings are kept in the project (`data.hosting`); passwords and API
//! keys in the credential store, like the bot token.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::thread;
use std::time::Instant;

use circuit_runner::hosting::{self, Hosting, Kind, Secrets, calagopus};
use circuit_runner::{build_bundle, secrets};
use eframe::egui::{self, RichText};

use crate::editor::{Editor, Level};
use crate::ui::dialog::{self, Part};
use crate::ui::kit;

const DATA_KEY: &str = "hosting";

/// A project's hosting settings (the defaults when it has none).
pub fn settings_of(ed: &Editor) -> Hosting {
    ed.project.as_ref().and_then(|p| p.meta.data.get(DATA_KEY)).and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default()
}

/// The project's hosting secrets, to use: none while its server isn't the one
/// saved on this computer (see [`approved`]), so they can't be sent elsewhere.
fn secrets_of(ed: &Editor) -> Secrets {
    if approved(ed) { saved_secrets(ed) } else { Secrets::default() }
}

/// The project's hosting secrets as they are saved, whatever its server is:
/// only to tell whether there are any.
fn saved_secrets(ed: &Editor) -> Secrets {
    let Some(p) = &ed.project else { return Secrets::default() };
    Secrets { password: secrets::get(&p.root, hosting::PASSWORD), api_key: secrets::get(&p.root, hosting::API_KEY) }
}

/// A project's server as far as its secrets go: where the password or key
/// (and the bot token) would be sent, and how. Not its name, nor what
/// happens around an upload.
fn destination(h: &Hosting) -> Hosting {
    Hosting { server_name: String::new(), auto_upload: false, restart: false, ..h.clone() }
}

fn project_key(ed: &Editor) -> Option<String> {
    ed.project.as_ref().map(|p| p.root.display().to_string())
}

/// Whether the server in the project's files is the one last saved in
/// Bot › Remote… on this computer. The settings travel with the project (a
/// plugin, an AI agent or a Git pull can change them); the password, the API
/// key and the bot token don't, and are only sent to a server the user has
/// seen and saved.
pub fn approved(ed: &Editor) -> bool {
    let h = settings_of(ed);
    // Read back into settings before comparing: a setting added in a later
    // version doesn't make every kept server another one.
    let kept = project_key(ed).and_then(|k| ed.prefs.approved_servers.get(&k)).and_then(|kept| serde_json::from_str::<Hosting>(kept).ok());
    h.kind == Kind::None || kept.is_some_and(|kept| destination(&kept) == destination(&h))
}

/// Takes the project's server as it is now as the one saved here.
fn approve(ed: &mut Editor) {
    if let Some(key) = project_key(ed) {
        let now = serde_json::to_string(&destination(&settings_of(ed))).unwrap_or_default();
        ed.prefs.approved_servers.insert(key, now);
    }
}

/// A project from before servers were kept (it has a saved password or key,
/// and no server kept): its server is taken as it is, once. False while the
/// credential store hasn't been read yet.
fn adopt_older(ed: &mut Editor) -> bool {
    let Some(p) = &ed.project else { return false };
    let known = project_key(ed).is_some_and(|k| ed.prefs.approved_servers.contains_key(&k));
    if known || settings_of(ed).kind == Kind::None || !secrets::is_known(&p.root, hosting::PASSWORD) || !secrets::is_known(&p.root, hosting::API_KEY) {
        return false;
    }
    let s = saved_secrets(ed);
    if s.password.is_none() && s.api_key.is_none() {
        return false;
    }
    approve(ed);
    true
}

/// Where SFTP servers' keys are kept, for all projects.
fn known_hosts() -> PathBuf {
    super::app_data_dir().join("known_hosts.json")
}

/// What a running upload (or test) sends back.
enum JobMsg {
    Log(String),
    Done(Result<String, String>),
}

/// The last upload's result, for the status bar.
pub struct Outcome {
    pub at: Instant,
    pub ok: bool,
    pub text: String,
}

/// The Server tab's edits, applied with Save (and before uploading).
pub struct Draft {
    h: Hosting,
    /// Typed passwords and keys; empty keeps the saved one.
    password: String,
    password_saved: bool,
    api_key: String,
    key_saved: bool,
    servers: Vec<calagopus::Server>,
    servers_problem: Option<String>,
    /// When the key or the panel's address was last typed in: the servers
    /// are read again once that's a moment ago.
    typed_at: Option<std::time::Instant>,
    /// The servers have been read (for these key and address), even if none came.
    servers_read: bool,
}

/// A connection to the remote's console that is only listened to: the bot
/// there is asked to say which blocks run, again and again while the editor
/// looks, and stops by itself when the editor doesn't ask any more.
struct Ears {
    console: calagopus::Console,
    server: String,
    values: bool,
    /// When the bot was last asked; `None` until the console is connected.
    asked: Option<Instant>,
    /// Lines from before now are the console's memory, not news.
    since: Instant,
}

pub struct HostingUi {
    pub draft: Option<Draft>,
    /// An upload or test running: what it is, and its messages.
    job: Option<(&'static str, Receiver<JobMsg>)>,
    /// What the last upload or test said, line by line.
    log: VecDeque<String>,
    pub outcome: Option<Outcome>,
    /// The number of saves seen, to upload after a new one.
    seen_saves: Option<u64>,
    /// An upload asked for while one runs: it follows.
    again: bool,
    sign_in: Option<calagopus::SignIn>,
    servers: Option<Receiver<Result<Vec<calagopus::Server>, String>>>,
    pub console: Option<ConsoleView>,
    /// Server is picked beside Run: Run, Restart and Stop act on the server.
    pub remote: bool,
    /// The server's state while `remote`.
    pub watch: Option<super::remote::Watch>,
    /// Whether the project's server can be run from here (a Calagopus server
    /// with a saved key), with what it was worked out for.
    controllable: Option<(String, bool)>,
    /// Stop on the server, under way.
    power: Option<Receiver<Result<(), String>>>,
    /// The ear at the bot on its remote, while Remote is picked: its console,
    /// read for which blocks ran.
    ears: Option<Ears>,
    /// When `Editor::server_view` was last written.
    view_at: Option<Instant>,
}

impl HostingUi {
    pub fn new() -> Self {
        HostingUi { draft: None, job: None, log: VecDeque::new(), outcome: None, seen_saves: None, again: false, sign_in: None, servers: None, console: None, remote: false, watch: None, controllable: None, power: None, ears: None, view_at: None }
    }

    /// Bot › Remote…: the tab with the project's settings.
    pub fn open(&mut self, ed: &Editor) {
        if ed.project.is_none() {
            return;
        }
        let h = settings_of(ed);
        // What is saved, also for a server not saved here yet: Save takes it.
        let s = saved_secrets(ed);
        self.draft = Some(Draft {
            h,
            password: String::new(),
            password_saved: s.password.is_some(),
            api_key: String::new(),
            key_saved: s.api_key.is_some(),
            servers: Vec::new(),
            servers_problem: None,
            typed_at: None,
            servers_read: false,
        });
        if self.draft.as_ref().is_some_and(|d| d.h.kind == Kind::Calagopus && d.key_saved) {
            self.read_servers(ed);
        }
    }

    pub fn busy(&self) -> Option<&'static str> {
        self.job.as_ref().map(|(what, _)| *what)
    }

    /// Whether the project has somewhere to upload to.
    pub fn configured(ed: &Editor) -> bool {
        settings_of(ed).kind != Kind::None
    }

    /// Every frame: messages from a running upload, sign-in and server list,
    /// and an upload after a save when that's turned on.
    pub fn poll(&mut self, ed: &mut Editor, ctx: &egui::Context) {
        if adopt_older(ed) {
            self.controllable = None;
        }
        let mut messages = Vec::new();
        let mut gone = false;
        if let Some((_, rx)) = &self.job {
            loop {
                match rx.try_recv() {
                    Ok(m) => messages.push(m),
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        gone = true;
                        break;
                    }
                }
            }
        }
        let what = self.busy().unwrap_or_default();
        let mut done = false;
        for m in messages {
            match m {
                JobMsg::Log(line) => self.push_log(line),
                JobMsg::Done(result) => {
                    done = true;
                    let ok = result.is_ok();
                    let text = result.unwrap_or_else(|e| e);
                    ed.log(if ok { Level::Success } else { Level::Error }, if ok { text.clone() } else { tf!("{what} failed: {error}", what = what, error = text) });
                    self.push_log(text.clone());
                    if !ok && let Some(w) = &mut self.watch {
                        w.forget_expect();
                    }
                    self.outcome = Some(Outcome { at: Instant::now(), ok, text });
                    self.job = None;
                    if std::mem::take(&mut self.again) {
                        self.upload(ed);
                    }
                }
            }
        }
        // Its thread ended without saying how it went.
        if gone && !done {
            self.job = None;
        }
        if self.job.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
        }
        // An upload after each save, when turned on.
        let saves = ed.saves;
        if self.seen_saves.is_some_and(|s| s != saves) && settings_of(ed).auto_upload && ed.project.is_some() {
            self.upload(ed);
        }
        self.seen_saves = Some(saves);

        if let Some(s) = &self.sign_in {
            match s.key.try_recv() {
                Ok(key) => {
                    self.sign_in = None;
                    if let Some(p) = &ed.project {
                        match secrets::set(&p.root, hosting::API_KEY, &key) {
                            Ok(()) => {
                                ed.log(Level::Success, t!("Signed in to the panel: the API key is kept in your system's credential store."));
                                self.controllable = None;
                                if let Some(d) = &mut self.draft {
                                    d.key_saved = true;
                                    d.api_key.clear();
                                }
                                self.read_servers(ed);
                            }
                            Err(e) => ed.log(Level::Error, format!("{e:#}")),
                        }
                    }
                }
                Err(TryRecvError::Empty) => ctx.request_repaint_after(std::time::Duration::from_millis(300)),
                Err(TryRecvError::Disconnected) => {
                    self.sign_in = None;
                    ed.log(Level::Warn, t!("Signing in to the panel stopped: no key came back within 15 minutes."));
                }
            }
        }
        if let Some(rx) = &self.servers {
            match rx.try_recv() {
                Ok(result) => {
                    self.servers = None;
                    if let Some(d) = &mut self.draft {
                        match result {
                            Ok(list) => {
                                // The only one, picked when none is.
                                if d.h.server.is_empty() && list.len() == 1 {
                                    d.h.server = list[0].uuid.clone();
                                    d.h.server_name = list[0].name.clone();
                                }
                                d.servers = list;
                                d.servers_problem = None;
                            }
                            Err(e) => {
                                d.servers.clear();
                                d.servers_problem = Some(e);
                            }
                        }
                    }
                }
                Err(TryRecvError::Empty) => ctx.request_repaint_after(std::time::Duration::from_millis(200)),
                Err(TryRecvError::Disconnected) => self.servers = None,
            }
        }
        // A key pasted (or the address changed): the list is read without
        // waiting for Read again, once the typing has stopped.
        if let Some(d) = &mut self.draft
            && let Some(at) = d.typed_at
        {
            let pause = std::time::Duration::from_millis(700);
            if at.elapsed() >= pause {
                d.typed_at = None;
                if d.h.kind == Kind::Calagopus && self.servers.is_none() {
                    self.read_servers(ed);
                }
            } else {
                ctx.request_repaint_after(pause - at.elapsed());
            }
        }
        // The console is the open project's server's: another project (or
        // none, or another server chosen) closes it.
        if self.console.as_ref().is_some_and(|c| {
            let h = settings_of(ed);
            ed.project.is_none() || !h.kind.is_panel() || h.server != c.server
        }) {
            self.console = None;
        }
        if let Some(c) = &mut self.console {
            c.poll(ctx);
        }
        self.poll_remote(ed, ctx);
    }

    /// Whether the project's server can be run from here: a Calagopus server
    /// chosen, with its API key saved. Worked out again when the settings change.
    pub fn can_control(&mut self, ed: &Editor) -> bool {
        let Some(p) = &ed.project else { return false };
        let h = settings_of(ed);
        let key = format!("{}|{}", p.root.display(), serde_json::to_string(&h).unwrap_or_default());
        if let Some((k, yes)) = &self.controllable
            && *k == key
        {
            return *yes;
        }
        let yes = h.kind.is_panel() && !h.server.is_empty() && !h.panel.trim().is_empty() && approved(ed) && secrets::get(&p.root, hosting::API_KEY).is_some();
        self.controllable = Some((key, yes));
        yes
    }

    /// Server picked (and possible) or not; the server's state read while it is.
    fn poll_remote(&mut self, ed: &mut Editor, ctx: &egui::Context) {
        let picked = ed.project.as_ref().is_some_and(|p| ed.prefs.remote_bots.contains(&p.root.display().to_string()));
        self.remote = picked && self.can_control(ed);
        if !self.remote {
            self.watch = None;
        } else {
            let h = settings_of(ed);
            if self.watch.as_ref().is_none_or(|w| w.server != h.server)
                && let Some(key) = secrets_of(ed).api_key
            {
                let name = if h.server_name.is_empty() { h.server.clone() } else { h.server_name.clone() };
                self.watch = Some(super::remote::Watch::new(calagopus::Panel::new(&h.panel, &key), h.server.clone(), name));
            }
        }
        if let Some(w) = &mut self.watch {
            w.poll(ctx);
        }
        // Watching the bot there: which blocks run, lit up as for a bot
        // run from here. Only while Remote is picked.
        if !self.remote {
            self.ears = None;
        } else {
            let h = settings_of(ed);
            if self.ears.as_ref().is_none_or(|e| e.server != h.server || e.values != h.watch_values)
                && let Some(key) = secrets_of(ed).api_key
            {
                let console = calagopus::Console::open(calagopus::Panel::new(&h.panel, &key), h.server.clone());
                self.ears = Some(Ears { console, server: h.server.clone(), values: h.watch_values, asked: None, since: Instant::now() });
            }
        }
        if let Some(ears) = &mut self.ears {
            while let Ok(event) = ears.console.events.try_recv() {
                match event {
                    calagopus::ConsoleEvent::Connected => {
                        ears.asked = Some(Instant::now() - std::time::Duration::from_secs(3600));
                        ears.since = Instant::now() + std::time::Duration::from_millis(1500);
                    }
                    calagopus::ConsoleEvent::Output(line) if Instant::now() >= ears.since => {
                        let text = calagopus::plain(&line);
                        if let Some(at) = text.find(circuit_runner::protocol::PREFIX) {
                            let event = circuit_runner::protocol::parse_line(&text[at..]);
                            if matches!(event, circuit_runner::protocol::BotEvent::Node { .. } | circuit_runner::protocol::BotEvent::Zone { .. }) {
                                super::handle_event(ed, event);
                                ctx.request_repaint();
                            }
                        }
                    }
                    calagopus::ConsoleEvent::Problem(_) => ears.asked = None,
                    _ => {}
                }
            }
            // Asked for a minute at a time, every half minute: when the
            // editor goes away, the bot stops saying it by itself.
            if ears.asked.is_some_and(|at| at.elapsed() > std::time::Duration::from_secs(30)) {
                ears.console.send(if ears.values { "watch 60 values" } else { "watch 60" });
                ears.asked = Some(Instant::now());
            }
        }
        if let Some(rx) = &self.power
            && let Ok(result) = rx.try_recv()
        {
            self.power = None;
            if let Err(e) = result {
                if let Some(w) = &mut self.watch {
                    w.forget_expect();
                }
                ed.log(Level::Error, tf!("The remote didn't stop: {error}", error = e));
            }
        }
        // What plugins and agents see of it (xc.server()), twice a second.
        if self.view_at.is_none_or(|t| t.elapsed() > std::time::Duration::from_millis(500)) {
            self.view_at = Some(Instant::now());
            ed.server_view = self.view(ed);
        }
    }

    /// The server as plugins and agents see it: settings without secrets,
    /// where Run acts, the server's state, the last upload, its console.
    fn view(&mut self, ed: &Editor) -> serde_json::Value {
        if ed.project.is_none() {
            return serde_json::Value::Null;
        }
        let h = settings_of(ed);
        let can = self.can_control(ed);
        let mb = |b: u64| (b as f64 / 1_048_576.0).round();
        let w = self.watch.as_ref();
        let last = w.and_then(|w| w.last.as_ref());
        serde_json::json!({
            "kind": serde_json::to_value(h.kind).unwrap_or(serde_json::Value::Null),
            "host": h.host,
            "port": h.port,
            "user": h.user,
            "panel": h.panel,
            "server": h.server,
            "serverName": h.server_name,
            "folder": h.folder,
            "uploadAfterSave": h.auto_upload,
            "sendToken": h.include_token,
            "restartAfterUpload": h.restart,
            "savedHere": approved(ed),
            "canRunThere": can,
            "runTarget": if self.remote { "server" } else { "local" },
            "state": w.map(|w| w.state().to_string()).filter(|s| !s.is_empty()),
            "cpuPercent": last.map(|r| (r.cpu * 10.0).round() / 10.0),
            "memoryMb": last.map(|r| mb(r.memory)),
            "memoryLimitMb": last.map(|r| mb(r.memory_limit)),
            "unreachable": w.and_then(|w| w.problem.clone()),
            "uploading": self.job.is_some(),
            "lastUpload": self.outcome.as_ref().map(|o| serde_json::json!({ "ok": o.ok, "text": o.text, "secondsAgo": o.at.elapsed().as_secs() })),
            "console": self.console.as_ref().map(|c| c.lines.iter().rev().take(60).rev().map(|l| calagopus::plain(l)).collect::<Vec<_>>()),
        })
    }

    /// A power action on the server, asked by a plugin or an agent: start
    /// and restart upload what changed first (as Run does), stop and kill
    /// right away.
    pub fn remote_action(&mut self, ed: &mut Editor, action: &str) {
        if self.another_server(ed) {
            return;
        }
        if !self.can_control(ed) {
            ed.log(Level::Warn, t!("The remote can't be started or stopped from here: choose a Calagopus server in Bot › Remote… and sign in."));
            return;
        }
        match action {
            "start" | "restart" => {
                if self.job.is_some() {
                    ed.log(Level::Warn, t!("An upload is under way: press Run again once it's done."));
                    return;
                }
                if let Some(w) = &mut self.watch {
                    w.expect(action);
                }
                self.start_upload(ed, Some(if action == "start" { "start" } else { "restart" }));
            }
            "stop" | "kill" => self.power_remote(ed, if action == "stop" { "stop" } else { "kill" }),
            _ => {}
        }
    }

    /// Run on the server: what changed is uploaded, then the server starts
    /// (or restarts, when it runs), so it runs this version.
    pub fn run_remote(&mut self, ed: &mut Editor) {
        if self.job.is_some() {
            ed.log(Level::Warn, t!("An upload is under way: press Run again once it's done."));
            return;
        }
        let action = match self.watch.as_ref().map(|w| w.state()) {
            Some("running" | "starting") => "restart",
            _ => "start",
        };
        if let Some(w) = &mut self.watch {
            w.expect(action);
        }
        self.start_upload(ed, Some(action));
    }

    /// Stop on the server.
    pub fn stop_remote(&mut self, ed: &mut Editor) {
        self.power_remote(ed, "stop");
    }

    /// Stop or kill on the server.
    fn power_remote(&mut self, ed: &mut Editor, action: &'static str) {
        let h = settings_of(ed);
        let Some(key) = secrets_of(ed).api_key else { return };
        if let Some(w) = &mut self.watch {
            w.expect(action);
        }
        ed.log(Level::Info, tf!("Stopping the bot on {server}.", server = place_name(&h)));
        let panel = calagopus::Panel::new(&h.panel, &key);
        let (tx, rx) = channel();
        self.power = Some(rx);
        thread::spawn(move || {
            let _ = tx.send(panel.power(&h.server, action).map_err(|e| format!("{e:#}")));
        });
    }

    fn push_log(&mut self, line: String) {
        self.log.push_back(line);
        while self.log.len() > 200 {
            self.log.pop_front();
        }
    }

    /// Reads the servers the API key reaches, for the list.
    fn read_servers(&mut self, ed: &Editor) {
        let Some(d) = &self.draft else { return };
        let key = if d.api_key.trim().is_empty() { secrets_of(ed).api_key } else { Some(d.api_key.trim().to_string()) };
        let (Some(key), false) = (key, d.h.panel.trim().is_empty()) else { return };
        let panel = calagopus::Panel::new(d.h.panel.trim(), &key);
        let (tx, rx) = channel();
        self.servers = Some(rx);
        if let Some(d) = &mut self.draft {
            d.servers_read = true;
        }
        thread::spawn(move || {
            let _ = tx.send(panel.servers().map_err(|e| format!("{e:#}")));
        });
    }

    /// Keeps the draft: settings into the project, typed secrets into the credential store.
    fn save_draft(&mut self, ed: &mut Editor) -> bool {
        self.controllable = None;
        let (Some(d), Some(p)) = (&mut self.draft, &mut ed.project) else { return false };
        let mut ok = true;
        for (name, typed, saved) in [(hosting::PASSWORD, &mut d.password, &mut d.password_saved), (hosting::API_KEY, &mut d.api_key, &mut d.key_saved)] {
            if typed.trim().is_empty() {
                continue;
            }
            match secrets::set(&p.root, name, typed.trim()) {
                Ok(()) => {
                    typed.clear();
                    *saved = true;
                }
                Err(e) => {
                    ed.log(Level::Error, format!("{e:#}"));
                    ok = false;
                    break;
                }
            }
        }
        let Some(p) = &mut ed.project else { return false };
        let value = serde_json::to_value(&d.h).unwrap_or_default();
        if p.meta.data.get(DATA_KEY) != Some(&value) {
            p.meta.data.insert(DATA_KEY.into(), value);
            ed.edited = true;
        }
        // Saved here by the user: the secrets may go to this server.
        approve(ed);
        ok
    }

    /// Whether the project's server is one that wasn't saved on this
    /// computer, and says so: nothing is sent to it until it is.
    fn another_server(&self, ed: &mut Editor) -> bool {
        if approved(ed) {
            return false;
        }
        ed.log(Level::Warn, t!("Nothing was sent: this project's remote isn't the one you saved on this computer. Look at it in Bot › Remote… and press Save to use it."));
        true
    }

    /// Exports the bot to the project's `.circuit/upload` folder and uploads
    /// what changed. A second upload asked for meanwhile follows this one.
    pub fn upload(&mut self, ed: &mut Editor) {
        self.start_upload(ed, None);
    }

    /// Uploads what changed; `then`, a power action for the Calagopus server
    /// afterwards (start, restart), in place of the restart after uploading.
    fn start_upload(&mut self, ed: &mut Editor, then: Option<&'static str>) {
        if self.job.is_some() {
            self.again = true;
            return;
        }
        if ed.project.is_none() {
            return;
        }
        let h = settings_of(ed);
        // What's missing first: without it nothing would be sent anyway.
        if let Some(what) = h.missing(&saved_secrets(ed)) {
            ed.log(Level::Warn, tf!("Not uploaded: {what} (Bot › Remote…)", what = missing_text(what)));
            return;
        }
        if self.another_server(ed) {
            return;
        }
        let s = secrets_of(ed);
        let Some(project) = &ed.project else { return };
        let bundle = match build_bundle(project, &ed.reg, ed.mode()) {
            Ok(b) => b,
            Err(e) => return ed.log(Level::Error, format!("Could not prepare the bot: {e:#}")),
        };
        let opts = super::export_options(ed, h.include_token, false);
        // Run on the server starts (or restarts) it itself, after uploading.
        let h = Hosting { restart: h.restart && then.is_none(), ..h };
        let local = project.root.join(".circuit");
        let (stage, record) = (local.join("upload"), local.join("upload.json"));
        let (tx, rx) = channel();
        self.log.clear();
        self.job = Some((t!("Upload"), rx));
        let where_to = place_name(&h);
        thread::spawn(move || {
            let mut log = |line: String| {
                let _ = tx.send(JobMsg::Log(line));
            };
            let result = (|| {
                circuit_runner::export(&stage, &bundle, &opts, &mut log)?;
                let done = hosting::upload(&stage, &h, &s, &record, &known_hosts(), &mut log);
                // The token stays only on the server, not in a copy here.
                let _ = std::fs::remove_file(stage.join(".env"));
                done
            })();
            let said = match result {
                Ok(up) if up.sent.is_empty() => Ok(tf!("Nothing new to upload to {place}: it has this version already.", place = where_to)),
                Ok(up) => Ok(tf!("Uploaded to {place}: {files}.", place = where_to, files = up.sent.join(", "))),
                Err(e) => Err(format!("{e:#}")),
            };
            // Then started (or restarted) on the server, for Run.
            let said = match (said, then) {
                (Ok(text), Some(action)) => {
                    let panel = calagopus::Panel::new(&h.panel, s.api_key.as_deref().unwrap_or_default());
                    match panel.power(&h.server, action) {
                        Ok(()) if action == "restart" => Ok(format!("{text} {}", t!("Restarting the bot on the remote."))),
                        Ok(()) => Ok(format!("{text} {}", t!("Starting the bot on the remote."))),
                        Err(e) => Err(tf!("Uploaded, but the remote didn't start: {error}", error = format!("{e:#}"))),
                    }
                }
                (said, _) => said,
            };
            let _ = tx.send(JobMsg::Done(said));
        });
    }

    /// Logs in with the draft's settings (saved first) and out again.
    fn test(&mut self, ed: &mut Editor) {
        if self.job.is_some() || !self.save_draft(ed) {
            return;
        }
        let h = settings_of(ed);
        let s = secrets_of(ed);
        let (tx, rx) = channel();
        self.log.clear();
        self.job = Some((t!("Connection test"), rx));
        thread::spawn(move || {
            let mut log = |line: String| {
                let _ = tx.send(JobMsg::Log(line));
            };
            let result = hosting::test(&h, &s, &known_hosts(), &mut log).map(|()| t!("The connection works.").to_string()).map_err(|e| format!("{e:#}"));
            let _ = tx.send(JobMsg::Done(result));
        });
    }

    /// Bot › Remote console, for a Calagopus server.
    pub fn open_console(&mut self, ed: &mut Editor) {
        if !self.restore_console(ed) {
            ed.log(Level::Warn, t!("The remote console is for a Calagopus server: choose one in Bot › Remote… first."));
            self.open(ed);
        }
    }

    /// Connects the console to the project's Calagopus server, quietly: false
    /// when the project has none (or no API key). Also how the console tab of
    /// the last run comes back when the app starts.
    pub fn restore_console(&mut self, ed: &Editor) -> bool {
        let h = settings_of(ed);
        let Some(key) = secrets_of(ed).api_key.filter(|_| h.kind.is_panel() && !h.server.is_empty()) else { return false };
        if self.console.as_ref().is_some_and(|c| c.server == h.server) {
            return true;
        }
        let panel = calagopus::Panel::new(&h.panel, &key);
        let name = if h.server_name.is_empty() { h.server.clone() } else { h.server_name.clone() };
        self.console = Some(ConsoleView::new(panel, h.server.clone(), name));
        true
    }

    /// The Server tab.
    pub fn tab(&mut self, ui: &mut egui::Ui, ed: &mut Editor) {
        if self.draft.is_none() {
            return;
        }
        let theme = ed.theme.clone();
        let icon = ed.reg.icons.get("server").cloned();
        let busy = self.busy();
        let (mut save, mut upload, mut test, mut close, mut sign_in, mut sign_out, mut reload) = (false, false, false, false, false, false, false);
        let token_env = ed.mode().map(|m| m.token_env().to_string()).unwrap_or_else(|| "DISCORD_TOKEN".into());
        let log: Vec<String> = self.log.iter().cloned().collect();
        let waiting = self.sign_in.is_some();
        let reading = self.servers.is_some();
        // A server that came with the project's files and wasn't saved here:
        // said at the top, as Save is what lets the secrets go there.
        let another = (!approved(ed)).then(|| ed.reg.icons.clone());
        // The name a bot of this project gets on a host.
        let host_bot_name = ed.project.as_ref().map(|p| circuit_runner::host_link::project_address("", &p.meta.name).trim_start_matches("/git/").trim_end_matches(".git").to_string()).unwrap_or_default();
        let d = self.draft.as_mut().unwrap();
        dialog::tab(
            ui,
            &theme,
            dialog::Spec { id: "hosting", title: t!("Remote"), subtitle: Some(t!("Upload the bot to the remote it runs on, by hand or after every save")), icon: icon.as_deref(), width: 640.0 },
            |ui, part| match part {
                Part::Body => {
                    if let Some(icons) = &another {
                        kit::note(
                            ui,
                            icons,
                            &theme,
                            "warning",
                            t!("Check this remote"),
                            t!("These settings came with the project's files: they aren't the ones you last saved on this computer. Your saved password or API key and the bot token are only sent there once you press Save."),
                        );
                        ui.add_space(10.0);
                    }
                    kit::section(ui, &theme, t!("How it gets there"));
                    ui.horizontal_wrapped(|ui| {
                        for (kind, label) in [(Kind::Host, t!("Your host")), (Kind::Calagopus, t!("Calagopus panel")), (Kind::Sftp, t!("SFTP")), (Kind::None, t!("Nowhere"))] {
                            if kit::radio(ui, &theme, d.h.kind == kind, label).clicked() && d.h.kind != kind {
                                d.h.kind = kind;
                                // Your host: where and as whom is known from
                                // the pairing; the bot goes by the project's name.
                                if kind == Kind::Host {
                                    match super::host::pairing() {
                                        Some((address, token)) => {
                                            d.h.panel = address;
                                            d.api_key = token;
                                            d.h.server = host_bot_name.clone();
                                            d.h.server_name = d.h.server.clone();
                                            d.h.folder.clear();
                                            d.h.restart = true;
                                        }
                                        None => {
                                            d.h.panel.clear();
                                            d.h.server.clear();
                                        }
                                    }
                                }
                            }
                            ui.add_space(8.0);
                        }
                    });
                    ui.add_space(8.0);
                    match d.h.kind {
                        // An older project's setting: nothing is sent this way any more.
                        Kind::Ftp => {
                            ui.label(RichText::new(missing_text(circuit_runner::hosting::FTP_GONE)).color(theme.warning));
                        }
                        Kind::Sftp => {
                            kit::field(ui, &theme, t!("Remote address"), Some(t!("Its name or IP address, such as node1.example.com.")), |ui| kit::text_input(ui, &mut d.h.host, "example.com", false));
                            kit::field(ui, &theme, t!("Port"), Some(t!("Usually 22 (game panels often use 2022).")), |ui| {
                                let mut port = if d.h.port == 0 { String::new() } else { d.h.port.to_string() };
                                if kit::text_input(ui, &mut port, "22", false).changed() {
                                    d.h.port = port.trim().parse().unwrap_or(0);
                                }
                            });
                            kit::field(ui, &theme, t!("User name"), None, |ui| kit::text_input(ui, &mut d.h.user, "", false));
                            let saved_hint = if d.password_saved { t!("Saved: type to change it") } else { "" };
                            let password_label = if !d.h.key_file.trim().is_empty() { t!("Key file password") } else { t!("Password") };
                            kit::field(ui, &theme, password_label, Some(t!("Kept in your system's credential store, never in the project.")), |ui| kit::text_input(ui, &mut d.password, saved_hint, true));
                            kit::field(ui, &theme, t!("Key file (optional)"), Some(t!("A private key to log in with instead of the password, such as id_ed25519.")), |ui| {
                                let w = ui.available_width();
                                kit::path(ui, &theme, &mut d.h.key_file, "", false, w)
                            });
                            kit::field(ui, &theme, t!("Folder on the remote"), Some(t!("Where the bot's files go, made when missing. Empty: the folder you start in after logging in.")), |ui| kit::text_input(ui, &mut d.h.folder, "bots/my-bot", false));
                        }
                        Kind::Calagopus => {
                            if kit::field(ui, &theme, t!("Panel address"), Some(t!("The address you open the panel at.")), |ui| kit::text_input(ui, &mut d.h.panel, "https://panel.example.com", false)).changed() {
                                d.typed_at = Some(std::time::Instant::now());
                            }
                            kit::field(ui, &theme, t!("Sign in"), Some(t!("Opens the panel in your browser to make an API key for XC Circuit; approve it there and it comes back here. Or paste a key you made yourself.")), |ui| {
                                ui.horizontal(|ui| {
                                    if waiting {
                                        ui.spinner();
                                        ui.label(RichText::new(t!("Waiting for you to approve in the browser…")).color(theme.text_muted));
                                        if kit::secondary_button(ui, &theme, t!("Cancel")).clicked() {
                                            sign_out = true;
                                        }
                                    } else {
                                        let label = if d.key_saved { t!("Sign in again") } else { t!("Sign in with the panel") };
                                        ui.add_enabled_ui(!d.h.panel.trim().is_empty(), |ui| {
                                            if kit::primary_button(ui, &theme, label).clicked() {
                                                sign_in = true;
                                            }
                                        });
                                        if d.key_saved {
                                            kit::done_label(ui, t!("Signed in"), theme.action);
                                        }
                                    }
                                });
                            });
                            let hint = if d.key_saved { t!("Saved: paste to change it") } else { "c7sp_…" };
                            if kit::field(ui, &theme, t!("API key"), None, |ui| kit::text_input(ui, &mut d.api_key, hint, true)).changed() {
                                d.typed_at = Some(std::time::Instant::now());
                            }
                            kit::field(ui, &theme, t!("Remote"), d.servers_problem.as_deref(), |ui| {
                                ui.horizontal(|ui| {
                                    let shown = if d.h.server_name.is_empty() { t!("Choose a server").to_string() } else { d.h.server_name.clone() };
                                    kit::dropdown(ui, egui::Id::unique("hosting-server"), 300.0, shown, |ui| {
                                        if d.servers.is_empty() {
                                            let has_key = d.key_saved || !d.api_key.trim().is_empty();
                                            let text = if reading {
                                                t!("Reading the servers…")
                                            } else if d.h.panel.trim().is_empty() {
                                                t!("Fill in the panel's address first.")
                                            } else if !has_key {
                                                t!("Sign in, or paste an API key, first.")
                                            } else if d.servers_problem.is_some() {
                                                t!("Couldn't read the servers: see under the list.")
                                            } else if d.servers_read {
                                                t!("This API key reaches no servers.")
                                            } else {
                                                t!("Press Read again to list them.")
                                            };
                                            ui.label(RichText::new(text).color(theme.text_muted));
                                        }
                                        for s in &d.servers {
                                            if kit::option(ui, d.h.server == s.uuid, format!("{}  ·  {}", s.name, s.short)).clicked() {
                                                d.h.server = s.uuid.clone();
                                                d.h.server_name = s.name.clone();
                                            }
                                        }
                                    });
                                    if reading {
                                        ui.spinner();
                                    } else if kit::secondary_button(ui, &theme, t!("Read again")).clicked() {
                                        reload = true;
                                    }
                                });
                            });
                            kit::field(ui, &theme, t!("Folder on the remote (optional)"), Some(t!("Empty: the server's own folder, where its start command looks.")), |ui| kit::text_input(ui, &mut d.h.folder, "", false));
                            kit::toggle_row(ui, &theme, &mut d.h.restart, t!("Restart the server after uploading"), t!("So the new version of the bot runs right away."));
                        }
                        Kind::Host => {
                            if d.h.panel.trim().is_empty() {
                                ui.label(RichText::new(t!("Pair this computer with your host first: File › Host…")).color(theme.warning));
                            } else {
                                kit::done_label(ui, tf!("The bot runs on your host at {address}", address = d.h.panel), theme.action);
                                if kit::field(ui, &theme, t!("The bot's name there"), Some(t!("Lowercase letters, digits and dashes. Each bot on the host has its own.")), |ui| kit::text_input(ui, &mut d.h.server, "my-bot", false)).changed() {
                                    d.h.server = d.h.server.to_lowercase().chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').collect();
                                    d.h.server_name = d.h.server.clone();
                                }
                                kit::caption(ui, &theme, t!("The host installs the bot's packages, starts it, and starts it again after its own restart. Run, Restart and Stop work from here once Remote is picked beside Run, and Bot › Remote console shows what the bot prints."));
                            }
                        }
                        Kind::None => {
                            ui.label(RichText::new(t!("Choose how the bot gets to the remote it runs on. Export (Bot › Export…) makes a folder to copy there by hand.")).color(theme.text_muted));
                        }
                    }
                    if d.h.kind != Kind::None {
                        kit::section(ui, &theme, t!("When"));
                        kit::toggle_row(ui, &theme, &mut d.h.auto_upload, t!("Upload after every save"), t!("Ctrl+S also puts the new version on the remote. Only files that changed are sent."));
                        kit::toggle_row(
                            ui,
                            &theme,
                            &mut d.h.include_token,
                            t!("Send the bot token"),
                            &tf!("Uploads .env with {variable} and the other secrets, so the bot can log in there. Turn it off to fill in .env on the remote yourself.", variable = token_env),
                        );
                        if d.h.kind.is_panel() {
                            kit::toggle_row(
                                ui,
                                &theme,
                                &mut d.h.watch_values,
                                t!("See the values on the wires too"),
                                t!("With Remote picked beside Run, the canvas shows which blocks run there, how long they take and what fails. With this on, also what goes through the wires: real members' names and messages, sent to this computer while you watch."),
                            );
                        }
                        kit::hint(ui, &theme, t!("The remote needs Node.js 22 or newer and runs npm install itself (game panels' Node.js servers do that when they start). Its data/ folder is never touched."));
                    }
                    if busy.is_some() || !log.is_empty() {
                        kit::section(ui, &theme, t!("What happened"));
                        egui::Frame::new().fill(theme.field).corner_radius(8).inner_margin(egui::Margin::symmetric(12, 8)).show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            for line in log.iter().rev().take(12).rev() {
                                ui.label(RichText::new(line).size(12.0).color(theme.text));
                            }
                            if let Some(what) = busy {
                                ui.horizontal(|ui| {
                                    ui.spinner();
                                    ui.label(RichText::new(tf!("{what} …", what = what)).size(12.0).color(theme.text_muted));
                                });
                            }
                        });
                    }
                }
                Part::Footer => {
                    ui.add_enabled_ui(busy.is_none() && d.h.kind != Kind::None, |ui| {
                        upload = kit::primary_button(ui, &theme, t!("Save and upload")).clicked();
                        test = kit::secondary_button(ui, &theme, t!("Test connection")).clicked();
                    });
                    save = kit::secondary_button(ui, &theme, t!("Save")).clicked();
                    close = kit::secondary_button(ui, &theme, t!("Close")).clicked();
                }
            },
        );
        if sign_in {
            let panel = self.draft.as_ref().map(|d| d.h.panel.clone()).unwrap_or_default();
            let bot = ed.project.as_ref().map(|p| p.meta.name.clone()).unwrap_or_default();
            match calagopus::sign_in(&panel, &calagopus::key_name(&bot)) {
                Ok(s) => {
                    if let Err(e) = open::that(&s.url) {
                        ed.log(Level::Warn, tf!("Couldn't open the browser ({error}). Open this address: {url}", error = e, url = s.url));
                    }
                    self.sign_in = Some(s);
                }
                Err(e) => ed.log(Level::Error, format!("{e:#}")),
            }
        }
        if sign_out {
            self.sign_in = None;
        }
        if reload {
            self.save_draft(ed);
            self.read_servers(ed);
        }
        if save {
            self.save_draft(ed);
        }
        if test {
            self.test(ed);
        }
        if upload && self.save_draft(ed) {
            self.upload(ed);
        }
        if close {
            self.draft = None;
            self.sign_in = None;
        }
    }
}

/// What [`Hosting::missing`] says, in the interface's language.
fn missing_text(english: &str) -> &'static str {
    match english {
        "Choose how the bot gets to its remote." => t!("Choose how the bot gets to its remote."),
        "Fill in the remote's address." => t!("Fill in the remote's address."),
        "Fill in the user name." => t!("Fill in the user name."),
        "Fill in the password, or choose a key file." => t!("Fill in the password, or choose a key file."),
        circuit_runner::hosting::FTP_GONE => t!("FTP is no longer offered: it sends the password and the bot token unprotected. Choose SFTP or a Calagopus panel."),
        "Fill in the panel's address." => t!("Fill in the panel's address."),
        "Sign in to the panel, or paste an API key." => t!("Sign in to the panel, or paste an API key."),
        "Choose the server." => t!("Choose the server."),
        "Pair this computer with your host first (File › Host…)." => t!("Pair this computer with your host first (File › Host…)."),
        "Give the bot a name on the host." => t!("Give the bot a name on the host."),
        _ => t!("Something is missing in the remote settings."),
    }
}

/// Where uploads go, in a few words.
fn place_name(h: &Hosting) -> String {
    match h.kind {
        Kind::Calagopus | Kind::Host => if h.server_name.is_empty() { h.server.clone() } else { h.server_name.clone() },
        _ => h.host.trim().to_string(),
    }
}

/// A console line as text to lay out: in its colours, wrapped at `width`.
fn line_job(line: &[calagopus::Piece], font: &egui::FontId, text: egui::Color32, dark: bool, width: f32) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    for piece in line {
        job.append(&piece.text, 0.0, egui::TextFormat { font_id: font.clone(), color: piece_color(piece.style, text, dark), ..Default::default() });
    }
    if line.is_empty() {
        job.append(" ", 0.0, egui::TextFormat { font_id: font.clone(), ..Default::default() });
    }
    job.wrap.max_width = width;
    job
}

/// The colour of a console piece: the terminal colours like a modern
/// terminal shows them, darker on a light theme; bold makes the first eight
/// bright (as terminals do), dim fades it; `text` without a colour.
fn piece_color(style: calagopus::Style, text: egui::Color32, dark: bool) -> egui::Color32 {
    use calagopus::Color;
    // black, red, green, yellow, blue, magenta, cyan, white, then bright.
    const DARK: [[u8; 3]; 16] = [
        [0x6b, 0x72, 0x80],
        [0xf0, 0x71, 0x78],
        [0x9c, 0xd0, 0x7a],
        [0xe8, 0xc4, 0x6b],
        [0x6c, 0xb2, 0xf5],
        [0xc9, 0x8b, 0xe6],
        [0x56, 0xc8, 0xd2],
        [0xd6, 0xdb, 0xe2],
        [0x8b, 0x92, 0x9e],
        [0xff, 0x8a, 0x8f],
        [0xb6, 0xe8, 0x91],
        [0xff, 0xd8, 0x80],
        [0x8f, 0xc6, 0xff],
        [0xe0, 0xa6, 0xff],
        [0x7d, 0xdf, 0xe8],
        [0xff, 0xff, 0xff],
    ];
    const LIGHT: [[u8; 3]; 16] = [
        [0x55, 0x5b, 0x66],
        [0xc0, 0x26, 0x2d],
        [0x2f, 0x7d, 0x32],
        [0x9a, 0x6a, 0x00],
        [0x1f, 0x5f, 0xc4],
        [0x8e, 0x3a, 0xb5],
        [0x0b, 0x7f, 0x8c],
        [0x4a, 0x4f, 0x57],
        [0x7a, 0x80, 0x8a],
        [0xd6, 0x3c, 0x43],
        [0x3e, 0x96, 0x42],
        [0xb0, 0x7d, 0x00],
        [0x2d, 0x74, 0xe0],
        [0xa5, 0x4e, 0xcc],
        [0x10, 0x96, 0xa4],
        [0x22, 0x25, 0x2b],
    ];
    let table = if dark { &DARK } else { &LIGHT };
    let color = match style.color {
        None => text,
        Some(Color::Ansi(n)) => {
            let n = if style.bold && n < 8 { n + 8 } else { n } as usize;
            let [r, g, b] = table[n.min(15)];
            egui::Color32::from_rgb(r, g, b)
        }
        Some(Color::Rgb(r, g, b)) => egui::Color32::from_rgb(r, g, b),
    };
    if style.dim { color.gamma_multiply(0.6) } else { color }
}

/// A Calagopus server's console tab: state, power buttons, live output.
pub struct ConsoleView {
    console: calagopus::Console,
    panel: calagopus::Panel,
    pub server: String,
    pub name: String,
    lines: VecDeque<Vec<calagopus::Piece>>,
    /// Each line's height, wrapped at `wrap_width` (NaN: not worked out yet).
    heights: VecDeque<f32>,
    wrap_width: f32,
    state: String,
    stats: Option<(u64, u64, f64)>,
    connected: bool,
    problem: Option<String>,
    /// A power action under way, and its answer.
    action: Option<(String, Receiver<Result<(), String>>)>,
}

impl ConsoleView {
    pub(crate) fn new(panel: calagopus::Panel, server: String, name: String) -> ConsoleView {
        let console = calagopus::Console::open(panel.clone(), server.clone());
        ConsoleView { console, panel, server, name, lines: VecDeque::new(), heights: VecDeque::new(), wrap_width: 0.0, state: String::new(), stats: None, connected: false, problem: None, action: None }
    }

    pub(crate) fn poll(&mut self, ctx: &egui::Context) {
        let mut any = false;
        while let Ok(e) = self.console.events.try_recv() {
            any = true;
            match e {
                calagopus::ConsoleEvent::Connected => {
                    self.connected = true;
                    self.problem = None;
                    // The panel sends the recent lines again.
                    self.lines.clear();
                    self.heights.clear();
                }
                calagopus::ConsoleEvent::Output(line) => {
                    // What the bot says for a watching editor isn't for reading.
                    if !calagopus::plain(&line).contains(circuit_runner::protocol::PREFIX) {
                        self.push(line);
                    }
                }
                calagopus::ConsoleEvent::Status(s) => self.state = s,
                calagopus::ConsoleEvent::Stats { memory, memory_limit, cpu } => self.stats = Some((memory, memory_limit, cpu)),
                calagopus::ConsoleEvent::Problem(p) => {
                    self.connected = false;
                    self.problem = Some(p);
                }
            }
        }
        if let Some((_, rx)) = &self.action
            && let Ok(result) = rx.try_recv()
        {
            if let Err(e) = result {
                self.problem = Some(e);
            }
            self.action = None;
        }
        if any {
            ctx.request_repaint();
        }
        // The thread sends as things happen; look again soon.
        ctx.request_repaint_after(std::time::Duration::from_millis(250));
    }

    /// A line for the console (the oldest goes past 3000).
    fn push(&mut self, line: Vec<calagopus::Piece>) {
        self.lines.push_back(line);
        self.heights.push_back(f32::NAN);
        while self.lines.len() > 3000 {
            self.lines.pop_front();
            self.heights.pop_front();
        }
    }

    fn power(&mut self, action: &str) {
        let (panel, server, a) = (self.panel.clone(), self.server.clone(), action.to_string());
        let (tx, rx) = channel();
        self.action = Some((action.to_string(), rx));
        thread::spawn(move || {
            let _ = tx.send(panel.power(&server, &a).map_err(|e| format!("{e:#}")));
        });
    }

    /// The console tab (in the main area or the side bar). It takes the room
    /// it's given and no more: in a small area the lines get fewer, and in a
    /// narrow one the buttons go under the name.
    pub fn tab(&mut self, ui: &mut egui::Ui, ed: &Editor) {
        let theme = ed.theme.clone();
        let narrow = ui.available_width() < 520.0;
        egui::Frame::new().fill(theme.panel).inner_margin(egui::Margin::symmetric(12, 10)).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(&self.name).size(14.0).strong().color(theme.text));
                let (label, color) = match self.state.as_str() {
                    "running" => (t!("running"), theme.action),
                    "starting" => (t!("starting"), theme.warning),
                    "stopping" => (t!("stopping"), theme.warning),
                    "offline" => (t!("offline"), theme.text_muted),
                    _ => (t!("…"), theme.text_muted),
                };
                kit::pill(ui, label, color);
                if !narrow {
                    self.stats_label(ui, &theme);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| self.power_buttons(ui, &theme, true));
                }
            });
            if narrow {
                if self.stats.is_some() {
                    self.stats_label(ui, &theme);
                }
                ui.horizontal_wrapped(|ui| self.power_buttons(ui, &theme, false));
            }
            if let Some(p) = &self.problem {
                ui.label(RichText::new(p).size(12.0).color(theme.warning));
            } else if !self.connected {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(RichText::new(t!("Connecting to the console…")).size(12.0).color(theme.text_muted));
                });
            }
            ui.add_space(6.0);
            let font = crate::ui::fonts::console(ui.ctx(), 12.5);
            // What's left, less the box's own margins (8 above and below)
            // and the tab's margin under it (10): never more, so the tab
            // never asks for more room than it has.
            let h = (ui.available_height() - 26.0).max(0.0);
            egui::Frame::new().fill(theme.field).corner_radius(6).inner_margin(egui::Margin::same(8)).show(ui, |ui| {
                let dark = ui.visuals().dark_mode;
                ui.spacing_mut().item_spacing.y = 0.0;
                let (lines, heights, wrap_width) = (&self.lines, &mut self.heights, &mut self.wrap_width);
                egui::ScrollArea::vertical().id_salt("server-console").min_scrolled_height(0.0).max_height(h).auto_shrink([false, false]).stick_to_bottom(true).show_viewport(ui, |ui, view| {
                    // Long lines wrap at the console's width. How tall each
                    // one is gets worked out once (again only when the width
                    // changes); only the lines in view are drawn, so a long
                    // console stays quick.
                    let width = ui.available_width().max(40.0);
                    if (width - *wrap_width).abs() > 0.5 {
                        *wrap_width = width;
                        heights.iter_mut().for_each(|h| *h = f32::NAN);
                    }
                    for (line, height) in lines.iter().zip(heights.iter_mut()) {
                        if height.is_nan() {
                            *height = ui.fonts_mut(|f| f.layout_job(line_job(line, &font, theme.text, dark, width))).size().y;
                        }
                    }
                    let total: f32 = heights.iter().sum();
                    ui.set_height(total);
                    let origin = ui.max_rect().min;
                    let mut y = 0.0;
                    for (line, height) in lines.iter().zip(heights.iter()) {
                        if y + height >= view.min.y {
                            let galley = ui.fonts_mut(|f| f.layout_job(line_job(line, &font, theme.text, dark, width)));
                            // As wide as its text, so it starts at the left edge.
                            let rect = egui::Rect::from_min_size(origin + egui::vec2(0.0, y), egui::vec2(galley.size().x.min(width), *height));
                            ui.put(rect, egui::Label::new(galley));
                        }
                        y += height;
                        if y > view.max.y {
                            break;
                        }
                    }
                });
            });
        });
    }

    /// CPU and memory, as the panel last said.
    fn stats_label(&self, ui: &mut egui::Ui, theme: &crate::ui::theme::Theme) {
        if let Some((memory, limit, cpu)) = self.stats {
            let mb = |b: u64| b as f64 / 1_048_576.0;
            let memory = if limit > 0 { format!("{:.0} / {:.0} MB", mb(memory), mb(limit)) } else { format!("{:.0} MB", mb(memory)) };
            ui.label(RichText::new(tf!("CPU {cpu}% · RAM {memory}", cpu = format!("{cpu:.0}"), memory = memory)).size(12.0).color(theme.text_muted));
        }
    }

    /// Start, Restart, Stop and Kill; `reversed` for a right-to-left row.
    fn power_buttons(&mut self, ui: &mut egui::Ui, theme: &crate::ui::theme::Theme, reversed: bool) {
        let busy = self.action.is_some();
        let offline = matches!(self.state.as_str(), "offline" | "");
        let mut order = ["start", "restart", "stop", "kill"];
        if reversed {
            order.reverse();
        }
        ui.add_enabled_ui(!busy, |ui| {
            for action in order {
                let clicked = match action {
                    "start" => ui.add_enabled_ui(offline, |ui| kit::primary_button(ui, theme, t!("Start"))).inner.clicked(),
                    "restart" => kit::secondary_button(ui, theme, t!("Restart")).clicked(),
                    "stop" => kit::secondary_button(ui, theme, t!("Stop")).clicked(),
                    _ => kit::danger_button(ui, theme, t!("Kill")).on_hover_text(t!("Ends it at once, without letting it save")).clicked(),
                };
                if clicked {
                    self.power(action);
                }
            }
        });
        if busy {
            ui.spinner();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// With "Upload after every save" on, a save starts an upload; here it
    /// stops at once, as no password is saved, and says so.
    #[test]
    fn a_save_uploads_when_that_is_turned_on() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let dir = std::env::temp_dir().join(format!("xc-hosting-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        copy_dir(&root.join("examples/DemoBot"), &dir);
        let mut ed = Editor::new(vec![root.join("plugins")], None).unwrap();
        ed.open_project(&dir);
        let ctx = egui::Context::default();
        let mut ui = HostingUi::new();
        ui.poll(&mut ed, &ctx);
        let h = Hosting { kind: Kind::Sftp, host: "sftp.example.com".into(), user: "bot".into(), auto_upload: false, ..Default::default() };
        ed.project.as_mut().unwrap().meta.data.insert(DATA_KEY.into(), serde_json::to_value(&h).unwrap());
        assert_eq!(settings_of(&ed), h, "kept in the project");
        let said = |ed: &Editor| ed.log.iter().any(|l| l.text.starts_with("Not uploaded: Fill in the password, or choose a key file."));
        ed.save();
        ed.finish_saving();
        ui.poll(&mut ed, &ctx);
        assert!(!said(&ed), "not without Upload after every save");
        let h = Hosting { auto_upload: true, ..h };
        ed.project.as_mut().unwrap().meta.data.insert(DATA_KEY.into(), serde_json::to_value(&h).unwrap());
        ed.save();
        ed.finish_saving();
        ui.poll(&mut ed, &ctx);
        assert!(said(&ed), "{:?}", ed.log.iter().map(|l| &l.text).collect::<Vec<_>>());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A project's server settings travel with its files; its saved password
    /// or key and the bot token only go to the server saved on this computer
    /// (in Bot › Remote…).
    #[test]
    fn secrets_only_go_to_the_server_saved_here() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let dir = std::env::temp_dir().join(format!("xc-hosting-saved-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        copy_dir(&root.join("examples/DemoBot"), &dir);
        let mut ed = Editor::new(vec![root.join("plugins")], None).unwrap();
        ed.open_project(&dir);
        let mut ui = HostingUi::new();
        assert!(approved(&ed), "no server: nowhere to send anything");

        // A server that came with the project's files. With a key file it
        // needs no password, so nothing is missing: it would upload.
        let h = Hosting { kind: Kind::Sftp, host: "elsewhere.example.com".into(), user: "bot".into(), key_file: "C:/keys/id_ed25519".into(), auto_upload: true, ..Default::default() };
        let put = |ed: &mut Editor, h: &Hosting| ed.project.as_mut().unwrap().meta.data.insert(DATA_KEY.into(), serde_json::to_value(h).unwrap());
        put(&mut ed, &h);
        assert!(!approved(&ed));
        ui.upload(&mut ed);
        assert!(ui.busy().is_none(), "no upload started");
        assert!(ed.log.last().unwrap().text.starts_with("Nothing was sent"), "{:?}", ed.log.last().map(|l| &l.text));
        assert_eq!(ui.view(&ed)["savedHere"], false);
        assert!(secrets_of(&ed).password.is_none() && secrets_of(&ed).api_key.is_none());

        // Saved in Bot › Remote…: now it's the one.
        ui.open(&ed);
        assert!(ui.save_draft(&mut ed));
        assert!(approved(&ed));
        assert_eq!(ui.view(&ed)["savedHere"], true);
        // What happens around an upload isn't part of where it goes …
        put(&mut ed, &Hosting { auto_upload: false, restart: false, server_name: "My server".into(), ..h.clone() });
        assert!(approved(&ed));
        // … but who gets it, how, where and with or without the token is.
        for other in [
            Hosting { host: "other.example.com".into(), ..h.clone() },
            Hosting { port: 2022, ..h.clone() },
            Hosting { user: "root".into(), ..h.clone() },
            Hosting { folder: "/etc".into(), ..h.clone() },
            Hosting { include_token: !h.include_token, ..h.clone() },
            Hosting { kind: Kind::Calagopus, panel: "https://panel.example.com".into(), server: "abc".into(), ..h.clone() },
        ] {
            put(&mut ed, &other);
            assert!(!approved(&ed), "{other:?}");
        }
        // Back as saved: fine again, without saving again.
        put(&mut ed, &h);
        assert!(approved(&ed));
        // Another project (folder) with the same settings has to save its own.
        let here = ed.prefs.approved_servers.keys().next().cloned().unwrap();
        let kept = ed.prefs.approved_servers.remove(&here).unwrap();
        ed.prefs.approved_servers.insert(format!("{here}-copy"), kept);
        assert!(!approved(&ed));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The console tab takes the height it's given, however little, and no
    /// more: a small area shows fewer lines instead of running past its edge.
    /// Long lines wrap at its width.
    #[test]
    fn the_console_fits_a_small_area_and_wraps() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let ed = Editor::new(vec![root.join("plugins")], None).unwrap();
        let mut view = ConsoleView::new(calagopus::Panel::new("http://127.0.0.1:9", "c7sp_x"), "u-1".into(), "Bot".into());
        for i in 0..200 {
            view.push(vec![calagopus::Piece { text: format!("line {i}"), style: Default::default() }]);
        }
        let ctx = egui::Context::default();
        for (width, height) in [(900.0, 140.0), (900.0, 400.0), (300.0, 220.0)] {
            let mut used = egui::Rect::NOTHING;
            let raw = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height))), ..Default::default() };
            let mut out = ctx.run_ui(raw, |ui| {
                view.tab(ui, &ed);
                used = ui.min_rect();
            });
            out.textures_delta.clear();
            assert!(used.height() <= height + 0.5, "{width}×{height}: took {}", used.height());
        }
        // A long line wraps at the console's width: in a narrow one it takes
        // more rows than in a wide one.
        view.push(vec![calagopus::Piece { text: "Logged in and ready to answer slash commands in every server. ".repeat(4), style: Default::default() }]);
        let mut tall = |width: f32| {
            let raw = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 300.0))), ..Default::default() };
            let mut out = ctx.run_ui(raw, |ui| view.tab(ui, &ed));
            out.textures_delta.clear();
            *view.heights.back().unwrap()
        };
        let (wide, narrow) = (tall(2400.0), tall(300.0));
        assert!(narrow > wide * 3.0, "wide {wide}, narrow {narrow}");
    }

    /// A pasted API key lists the servers by itself once the typing stops,
    /// without Read again.
    #[test]
    fn a_pasted_key_reads_the_servers() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut ed = Editor::new(vec![root.join("plugins")], None).unwrap();
        ed.open_project(&root.join("examples/DemoBot"));
        let ctx = egui::Context::default();
        let mut ui = HostingUi::new();
        ui.open(&ed);
        let d = ui.draft.as_mut().unwrap();
        d.h = Hosting { kind: Kind::Calagopus, panel: "http://127.0.0.1:9".into(), ..Default::default() };
        d.api_key = "c7sp_pasted".into();
        d.typed_at = Some(std::time::Instant::now());
        ui.poll(&mut ed, &ctx);
        assert!(ui.servers.is_none(), "not while still typing");
        ui.draft.as_mut().unwrap().typed_at = Some(std::time::Instant::now() - std::time::Duration::from_secs(1));
        ui.poll(&mut ed, &ctx);
        assert!(ui.servers.is_some() && ui.draft.as_ref().unwrap().servers_read, "read once the typing stopped");
    }

    /// Server picked beside Run counts only for a server that can be run
    /// from here (Calagopus, with a server and a saved key): otherwise Run
    /// stays local and nothing is read from a panel.
    #[test]
    fn server_is_only_used_when_it_can_be_run_from_here() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut ed = Editor::new(vec![root.join("plugins")], None).unwrap();
        ed.open_project(&root.join("examples/DemoBot"));
        let ctx = egui::Context::default();
        let mut ui = HostingUi::new();
        let here = ed.project.as_ref().unwrap().root.display().to_string();
        ed.prefs.remote_bots.push(here);
        // An SFTP server can't be started or stopped from here.
        let h = Hosting { kind: Kind::Sftp, host: "sftp.example.com".into(), user: "bot".into(), ..Default::default() };
        ed.project.as_mut().unwrap().meta.data.insert(DATA_KEY.into(), serde_json::to_value(&h).unwrap());
        ui.poll(&mut ed, &ctx);
        assert!(!ui.remote && ui.watch.is_none() && !ui.can_control(&ed));
        // A Calagopus server, but no key saved for DemoBot: still local.
        let h = Hosting { kind: Kind::Calagopus, panel: "http://127.0.0.1:9".into(), server: "u-1".into(), ..Default::default() };
        ed.project.as_mut().unwrap().meta.data.insert(DATA_KEY.into(), serde_json::to_value(&h).unwrap());
        ui.poll(&mut ed, &ctx);
        assert!(!ui.remote && ui.watch.is_none(), "no key: Run stays local");
    }

    /// The console belongs to the project's server: it stays while that's
    /// the server, and closes when another is chosen.
    #[test]
    fn the_console_follows_the_projects_server() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut ed = Editor::new(vec![root.join("plugins")], None).unwrap();
        ed.open_project(&root.join("examples/DemoBot"));
        let ctx = egui::Context::default();
        let mut ui = HostingUi::new();
        let set = |ed: &mut Editor, server: &str| {
            let h = Hosting { kind: Kind::Calagopus, panel: "http://127.0.0.1:9".into(), server: server.into(), ..Default::default() };
            ed.project.as_mut().unwrap().meta.data.insert(DATA_KEY.into(), serde_json::to_value(&h).unwrap());
        };
        set(&mut ed, "u-1");
        // Nobody signed in to DemoBot's panel: nothing to reconnect at start.
        assert!(!ui.restore_console(&ed));
        // A console for its server (here at an address nobody answers on).
        ui.console = Some(ConsoleView::new(calagopus::Panel::new("http://127.0.0.1:9", "c7sp_x"), "u-1".into(), "Bot".into()));
        ui.poll(&mut ed, &ctx);
        assert!(ui.console.is_some(), "the project's server: it stays");
        set(&mut ed, "u-2");
        ui.poll(&mut ed, &ctx);
        assert!(ui.console.is_none(), "another server: closed");
    }

    fn copy_dir(from: &std::path::Path, to: &std::path::Path) {
        std::fs::create_dir_all(to).unwrap();
        for e in std::fs::read_dir(from).unwrap().flatten() {
            let target = to.join(e.file_name());
            if e.path().is_dir() {
                copy_dir(&e.path(), &target);
            } else {
                std::fs::copy(e.path(), target).unwrap();
            }
        }
    }
}
