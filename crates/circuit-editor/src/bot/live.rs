//! File › Live session…: working in one project together, live.
//!
//! The host's project is the one true one; guests get a copy to work in
//! (in the app's own folder) that follows it. The exchange is
//! `circuit_sync::live`'s, the sealed wire `circuit_runner::live_link`'s;
//! this is what joins them to the editor: turning the project into the
//! session's documents and back, and the window.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use circuit_core::Workspace;
use circuit_runner::live_link::{Event, PUBLIC_RELAY, Wire};
use circuit_sync::live::{Blocks, Docs, GuestSide, HostSide, META, Msg, Perms, Power, Presence};
use circuit_sync::{Side, json};
use eframe::egui::{self, RichText};
use serde_json::Value;

use crate::editor::{Editor, Level};
use crate::ui::dialog::{self, Part};
use crate::ui::kit;

/// The app's version: host and guests need the same.
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The project as the session's documents: each workspace, and the slash
/// commands. Blocks get their fixed ids first: the session tells them apart
/// by those.
pub fn docs_of(ed: &mut Editor) -> Docs {
    let Some(project) = ed.project.as_mut() else { return Docs::new() };
    let mut docs = Docs::new();
    for (key, ws) in project.workspaces.iter_mut() {
        ws.give_new_uids();
        docs.insert(key.clone(), serde_json::to_value(&*ws).unwrap_or_default());
    }
    docs.insert(META.to_string(), serde_json::json!({ "slashCommands": project.meta.data.get("slashCommands").cloned().unwrap_or_else(|| serde_json::json!([])) }));
    docs
}

/// One of the session's documents, put into the open project. `rebase`
/// gives what an earlier state of a workspace becomes with the change in
/// it, so Undo only takes back one's own work.
fn put(ed: &mut Editor, key: &str, state: Option<&Value>, rebase: impl Fn(&Value) -> Value) {
    let Some(project) = ed.project.as_mut() else { return };
    if key == META {
        match state.map(|s| s["slashCommands"].clone()).filter(|c| c.as_array().is_some_and(|a| !a.is_empty())) {
            Some(commands) => drop(project.meta.data.insert("slashCommands".into(), commands)),
            None => drop(project.meta.data.remove("slashCommands")),
        }
        ed.edited = true;
        return;
    }
    match state.and_then(|s| serde_json::from_value::<Workspace>(s.clone()).ok()) {
        Some(mut ws) => {
            ws.pin_wired_fields();
            // How this person looks at a block stays theirs.
            if let Some(old) = project.workspaces.get(key) {
                for (id, node) in ws.nodes.iter_mut() {
                    node.more = old.nodes.get(id).is_some_and(|n| n.more && n.uid == node.uid);
                }
            }
            let as_ws = |v: &Value| serde_json::from_value::<Workspace>(v.clone()).ok();
            ed.history.take_in(key, &ws, |earlier| serde_json::to_value(earlier).ok().and_then(|v| as_ws(&rebase(&v))).unwrap_or_else(|| earlier.clone()));
            project.workspaces.insert(key.to_string(), ws);
        }
        None => {
            if state.is_none() {
                project.remove_workspace(key);
            }
        }
    }
    ed.shapes.clear();
    ed.edited = true;
}

/// The project's files that aren't workspaces, as they are stored: for a
/// guest's copy.
fn other_files(root: &std::path::Path) -> BTreeMap<String, String> {
    let mut files = BTreeMap::new();
    if let Ok(text) = std::fs::read_to_string(root.join(circuit_core::project::PROJECT_FILE)) {
        files.insert(circuit_core::project::PROJECT_FILE.to_string(), text);
    }
    files
}

/// Where a guest's copy of a session's project lives.
fn guest_folder(invite: &str) -> PathBuf {
    let room: String = invite.rsplit('/').next().unwrap_or_default().split('#').next().unwrap_or_default().chars().filter(|c| c.is_ascii_alphanumeric()).take(24).collect();
    super::app_data_dir().join("sessions").join(if room.is_empty() { "session".into() } else { room })
}

enum Role {
    Host { side: HostSide, files: BTreeMap<String, String>, invite: Option<String> },
    Guest { side: GuestSide, name: String, folder: PathBuf, welcomed: bool },
}

struct Session {
    wire: Wire,
    role: Role,
    /// The project's revision when it was last looked at.
    seen: Option<u64>,
    looked: Instant,
}

#[derive(Default)]
pub struct LiveUi {
    pub open: bool,
    session: Option<Session>,
    name: String,
    relay: String,
    invite: String,
    said: Option<(bool, String)>,
    /// A guest's copy of the session's project, for the app to open.
    pub arrive: Option<PathBuf>,
    /// Where the others are, by their id.
    others: BTreeMap<String, Presence>,
    /// Where this app last said it is, and when.
    here: Option<(Presence, Instant)>,
    /// A guest asked for the bot to be started or stopped: for the app.
    pub power: Option<Power>,
    /// How much of the console the guests have (the host), and the bot's
    /// state they were last told.
    told: (usize, String),
    /// The bot at the host's, as a guest was last told.
    bot: Option<String>,
}

impl LiveUi {
    pub fn new() -> Self {
        Self::default()
    }

    /// File › Live session…
    pub fn open(&mut self) {
        self.open = true;
        if self.name.is_empty() {
            self.name = std::env::var("USERNAME").or_else(|_| std::env::var("USER")).unwrap_or_default();
        }
        if self.relay.is_empty() {
            self.relay = PUBLIC_RELAY.into();
        }
    }

    /// Whether this app is a guest in a session: the bot runs at the host's.
    pub fn is_guest(&self) -> bool {
        matches!(self.session.as_ref().map(|s| &s.role), Some(Role::Guest { .. }))
    }

    /// What the status bar says, in a session.
    pub fn status(&self) -> Option<String> {
        Some(match &self.session.as_ref()?.role {
            Role::Host { side, .. } => tf!("Live: {count} guests", count = side.guests.len()),
            Role::Guest { side, .. } => tf!("Live: in {name}'s session", name = side.host),
        })
    }

    /// Starts a session for the open project, at `relay`.
    pub fn start(&mut self, ed: &mut Editor) {
        let Some(root) = ed.project.as_ref().map(|p| p.root.clone()) else { return };
        // Saved first: the guests' copies start from the stored files.
        if ed.unsaved_now() {
            ed.save();
        }
        ed.finish_saving();
        let docs = docs_of(ed);
        match Wire::host(&self.relay) {
            Ok(wire) => {
                let side = HostSide::new(self.name.trim(), VERSION, docs);
                self.session = Some(Session { wire, role: Role::Host { side, files: other_files(&root), invite: None }, seen: Some(ed.history.revision()), looked: Instant::now() });
                self.said = None;
            }
            Err(e) => self.said = Some((false, format!("{e:#}"))),
        }
    }

    /// Joins the session an invitation is for.
    pub fn join(&mut self) {
        match Wire::join(&self.invite) {
            Ok(wire) => {
                let folder = guest_folder(&self.invite);
                self.session = Some(Session { wire, role: Role::Guest { side: GuestSide::new(), name: self.name.trim().to_string(), folder, welcomed: false }, seen: None, looked: Instant::now() });
                self.said = None;
            }
            Err(e) => self.said = Some((false, format!("{e:#}"))),
        }
    }

    /// Ends the session (the host), or leaves it (a guest).
    pub fn end(&mut self) {
        self.others.clear();
        self.bot = None;
        if self.session.take().is_some() {
            self.said = Some((true, t!("The session is over.").into()));
        }
    }

    /// Every frame: what came over the wire, and what changed here.
    /// A guest's Run or Stop: asked of the host, when the guest may.
    pub fn ask_power(&mut self, ed: &mut Editor, action: Power) {
        let Some(Session { wire, role: Role::Guest { side, .. }, .. }) = self.session.as_ref() else { return };
        if side.perms.power {
            wire.send(None, serde_json::to_value(Msg::Power { action }).unwrap_or_default());
        } else {
            ed.log(Level::Warn, t!("The bot runs at the session's host: only they can start it."));
        }
    }

    /// `bot` is how the bot stands here: `stopped`, `starting` or `running`.
    pub fn poll(&mut self, ed: &mut Editor, ctx: &egui::Context, bot: &str) {
        let Some(session) = self.session.as_mut() else { return };
        let mut over: Option<String> = None;
        for event in session.wire.events() {
            match (&mut session.role, event) {
                (Role::Host { invite, .. }, Event::Opened { invite: text }) => *invite = Some(text),
                (Role::Host { side, .. }, Event::Peer { id, on: false }) => {
                    if let Some(guest) = side.guests.get(&id) {
                        ed.log(Level::Info, tf!("{name} left the session", name = guest.name));
                    }
                    for (to, msg) in side.left(&id) {
                        session.wire.send(to.as_deref(), serde_json::to_value(&msg).unwrap_or_default());
                    }
                    self.others.remove(&id);
                }
                (Role::Host { side, files, .. }, Event::From { id, data }) => {
                    let Ok(msg) = serde_json::from_value::<Msg>(data) else { continue };
                    let arriving = matches!(msg, Msg::Hello { .. });
                    let turn = side.receive(&id, msg, files);
                    for (to, msg) in turn.send {
                        session.wire.send(to.as_deref(), serde_json::to_value(&msg).unwrap_or_default());
                    }
                    for (key, before, after) in turn.changed {
                        // An earlier state gets the guest's change too.
                        let (before, after2) = (before.clone(), after.clone());
                        put(ed, &key, after.as_ref(), move |earlier| match (&before, &after2) {
                            (Some(b), Some(a)) => json::workspace_numbered(Some(b), earlier, a, &|_| Some(Side::Theirs), Side::Theirs).value,
                            _ => earlier.clone(),
                        });
                    }
                    if arriving && let Some(guest) = side.guests.get(&id) {
                        ed.log(Level::Info, tf!("{name} joined the session", name = guest.name));
                        // The newcomer learns where everyone is, and how the bot stands.
                        self.here = None;
                        self.told.1.clear();
                    }
                    if let Some(here) = turn.presence {
                        self.others.insert(here.id.clone(), here);
                    }
                    if let Some(action) = turn.power {
                        self.power = Some(action);
                    }
                    if let Some(notice) = turn.notice {
                        ed.log(Level::Info, notice);
                    }
                }
                (Role::Guest { name, .. }, Event::Joined) => session.wire.send(None, serde_json::to_value(GuestSide::hello(name, VERSION)).unwrap_or_default()),
                (Role::Guest { side, folder, welcomed, .. }, Event::From { data, .. }) => {
                    let Ok(msg) = serde_json::from_value::<Msg>(data) else { continue };
                    let now = if *welcomed { docs_of(ed) } else { Docs::new() };
                    let turn = side.receive(msg, &now);
                    if let Some(why) = turn.refused {
                        over = Some(why);
                        continue;
                    }
                    if let Some((_, files, docs)) = turn.welcome {
                        // The copy to work in: written as a project, then opened.
                        match write_copy(folder, &files, &docs) {
                            Ok(()) => {
                                self.arrive = Some(folder.clone());
                                *welcomed = true;
                                session.seen = None;
                            }
                            Err(e) => over = Some(format!("{e:#}")),
                        }
                        continue;
                    }
                    for a in turn.apply {
                        let (before, after) = (a.before.clone(), a.after.clone());
                        put(ed, &a.key, a.local.as_ref(), move |earlier| match (&before, &after) {
                            (Some(b), Some(n)) => json::workspace_numbered(Some(b), earlier, n, &|_| Some(Side::Theirs), Side::Theirs).value,
                            _ => earlier.clone(),
                        });
                    }
                    if let Some(notice) = turn.notice {
                        ed.log(Level::Warn, notice.clone());
                        self.said = Some((false, notice));
                    }
                    if let Some(here) = turn.presence {
                        self.others.insert(here.id.clone(), here);
                    }
                    if let Some(id) = turn.gone {
                        self.others.remove(&id);
                    }
                    for (level, text) in turn.console {
                        ed.log(level_of(&level), text);
                    }
                    if let Some(state) = turn.bot {
                        self.bot = Some(state);
                    }
                }
                (_, Event::Closed { why }) => over = Some(why),
                _ => {}
            }
        }
        if let Some(why) = over {
            self.session = None;
            self.others.clear();
            ed.live_others.clear();
            self.bot = None;
            let text = if why.is_empty() { t!("The session is over.").to_string() } else { tf!("The session is over: {why}", why = why) };
            ed.log(Level::Info, text.clone());
            self.said = Some((why.is_empty(), text));
            return;
        }
        // What changed here goes out: looked at when the project's revision
        // moved, a few times a second at most.
        let revision = ed.history.revision();
        let ready = match &session.role {
            Role::Guest { welcomed, folder, .. } => *welcomed && ed.project.as_ref().is_some_and(|p| p.root == *folder),
            Role::Host { .. } => true,
        };
        if ready && session.seen != Some(revision) && session.looked.elapsed() > Duration::from_millis(200) {
            session.seen = Some(revision);
            session.looked = Instant::now();
            let docs = docs_of(ed);
            match &mut session.role {
                Role::Host { side, .. } => {
                    for (to, msg) in side.local(&docs) {
                        session.wire.send(to.as_deref(), serde_json::to_value(&msg).unwrap_or_default());
                    }
                }
                Role::Guest { side, .. } => {
                    for msg in side.local(&docs) {
                        session.wire.send(None, serde_json::to_value(&msg).unwrap_or_default());
                    }
                }
            }
        }
        // Where this one is, for the others: said when it changed, ten times
        // a second at most.
        if ready {
            let ws = ed.active_workspace.clone().filter(|k| ed.project.as_ref().is_some_and(|p| p.workspaces.contains_key(k)));
            let at = ws.as_ref().and_then(|k| ed.views.get(k)).and_then(|v| v.pointer).map(|p| [p.x.round(), p.y.round()]);
            let mut selected: Vec<String> = match (&ed.selected, &ws) {
                (Some((k, id)), Some(ws)) if k == ws => ed.multi.iter().cloned().chain([id.clone()]).collect::<BTreeSet<String>>().into_iter().collect(),
                _ => Vec::new(),
            };
            selected.truncate(200);
            let here = Presence { ws, at, selected, ..Default::default() };
            if self.here.as_ref().is_none_or(|(last, when)| *last != here && when.elapsed() > Duration::from_millis(100)) {
                match &session.role {
                    Role::Host { side, .. } => {
                        for (to, msg) in side.presence(here.clone()) {
                            session.wire.send(to.as_deref(), serde_json::to_value(&msg).unwrap_or_default());
                        }
                    }
                    Role::Guest { .. } => session.wire.send(None, serde_json::to_value(Msg::Presence(here.clone())).unwrap_or_default()),
                }
                self.here = Some((here, Instant::now()));
            }
        }
        // The host passes on what the bot wrote, and whether it runs.
        if let Role::Host { side, .. } = &session.role
            && !side.guests.is_empty()
        {
            if ed.log.len() < self.told.0 {
                self.told.0 = 0;
            }
            let lines: Vec<(String, String)> = ed.log[self.told.0..].iter().take(100).map(|l| (level_name(l.level).to_string(), l.text.clone())).collect();
            if !lines.is_empty() {
                self.told.0 += lines.len();
                session.wire.send(None, serde_json::to_value(Msg::Console { lines }).unwrap_or_default());
            }
            if self.told.1 != bot {
                self.told.1 = bot.to_string();
                session.wire.send(None, serde_json::to_value(Msg::Bot { state: bot.to_string() }).unwrap_or_default());
            }
        } else if matches!(session.role, Role::Host { .. }) {
            // Nobody to tell: what was written before a guest came stays here.
            self.told = (ed.log.len(), String::new());
        }
        ed.live_others = self.others.values().filter_map(|o| Some(crate::editor::LiveOther { name: o.name.clone(), color: colour_of(&o.id), ws: o.ws.clone()?, at: o.at, selected: o.selected.clone() })).collect();
        ctx.request_repaint_after(Duration::from_millis(if self.others.is_empty() { 250 } else { 60 }));
    }

    pub fn tab(&mut self, ui: &mut egui::Ui, ed: &mut Editor) {
        if !self.open {
            return;
        }
        let theme = ed.theme.clone();
        let icons = ed.reg.icons.clone();
        let icon = icons.get("user-plus").cloned();
        let has_project = ed.project.is_some();
        let workspaces: Vec<(String, String)> = ed.project.as_ref().map(|p| p.workspaces.iter().map(|(k, w)| (k.clone(), w.name.clone())).collect()).unwrap_or_default();
        let (mut start, mut join, mut end, mut close) = (false, false, false, false);
        let mut allow: Vec<(String, Perms)> = Vec::new();
        let mut kick: Option<String> = None;
        dialog::tab(
            ui,
            &theme,
            dialog::Spec { id: "live", title: t!("Live session"), subtitle: Some(t!("Work in one project together, at the same time")), icon: icon.as_deref(), width: 640.0 },
            |ui, part| match part {
                Part::Body => {
                    match self.session.as_ref().map(|s| &s.role) {
                        None => {
                            kit::note(
                                ui,
                                &icons,
                                &theme,
                                "info",
                                t!("How it works"),
                                t!("One person hosts: the project is theirs, it is saved on their computer, and the bot runs there. Guests see every change as it happens and may change what the host lets them. Everything between you is sealed; the relay in the middle only passes it on."),
                            );
                            ui.add_space(8.0);
                            kit::field(ui, &theme, t!("Your name"), Some(t!("How the others see you.")), |ui| kit::text_input(ui, &mut self.name, "", false));
                            ui.add_space(6.0);
                            kit::section(ui, &theme, t!("Host a session"));
                            kit::field(ui, &theme, t!("Relay"), Some(t!("Where the session meets: the public relay, or your own host's address.")), |ui| kit::text_input(ui, &mut self.relay, PUBLIC_RELAY, false));
                            ui.add_enabled_ui(has_project && !self.name.trim().is_empty(), |ui| {
                                start = kit::primary_button(ui, &theme, t!("Start a session for this project")).clicked();
                            });
                            ui.add_space(10.0);
                            kit::section(ui, &theme, t!("Join a session"));
                            kit::field(ui, &theme, t!("Invitation"), Some(t!("The text the host sent you. It starts with xc-live:")), |ui| kit::text_input(ui, &mut self.invite, "xc-live:…", false));
                            ui.add_enabled_ui(!self.invite.trim().is_empty() && !self.name.trim().is_empty(), |ui| {
                                join = kit::secondary_button(ui, &theme, t!("Join")).clicked();
                            });
                        }
                        Some(Role::Host { side, invite, .. }) => {
                            kit::section(ui, &theme, t!("Invitation"));
                            match invite {
                                Some(invite) => {
                                    ui.label(t!("Send this to the people you want in. Whoever has it can join, so only give it to them."));
                                    ui.horizontal(|ui| {
                                        let short: String = invite.chars().take(54).collect();
                                        ui.label(RichText::new(format!("{short}…")).monospace().size(11.5).color(theme.text_muted));
                                        if kit::secondary_button(ui, &theme, t!("Copy")).clicked() {
                                            ui.ctx().copy_text(invite.clone());
                                        }
                                    });
                                }
                                None => {
                                    ui.horizontal(|ui| {
                                        ui.spinner();
                                        ui.label(RichText::new(t!("Opening the session…")).color(theme.text_muted));
                                    });
                                }
                            }
                            ui.add_space(8.0);
                            kit::section(ui, &theme, t!("Guests"));
                            if side.guests.is_empty() {
                                ui.label(RichText::new(t!("Nobody is here yet.")).color(theme.text_muted));
                            }
                            for (id, guest) in &side.guests {
                                ui.label(RichText::new(&guest.name).strong());
                                let mut perms = guest.perms.clone();
                                ui.horizontal_wrapped(|ui| {
                                    kit::checkbox(ui, &theme, &mut perms.power, t!("The bot (start, stop)"));
                                    kit::checkbox(ui, &theme, &mut perms.commands, t!("Slash commands"));
                                    kit::checkbox(ui, &theme, &mut perms.workspaces, t!("Workspaces"));
                                    let mut all = perms.blocks == Blocks::All;
                                    if kit::checkbox(ui, &theme, &mut all, t!("Blocks, everywhere")).changed() {
                                        perms.blocks = if all { Blocks::All } else { Blocks::None };
                                    }
                                    if kit::link(ui, &theme, t!("Send away"), egui::FontId::proportional(12.0)).clicked() {
                                        kick = Some(id.clone());
                                    }
                                });
                                if perms.blocks != Blocks::All {
                                    // Blocks in some workspaces only.
                                    let mut only: BTreeSet<String> = match &perms.blocks {
                                        Blocks::Only(keys) => keys.clone(),
                                        _ => BTreeSet::new(),
                                    };
                                    ui.horizontal_wrapped(|ui| {
                                        ui.label(RichText::new(t!("Blocks in:")).size(12.0).color(theme.text_muted));
                                        for (key, name) in &workspaces {
                                            let mut on = only.contains(key);
                                            if kit::checkbox(ui, &theme, &mut on, name.as_str()).changed() {
                                                if on { only.insert(key.clone()) } else { only.remove(key) };
                                            }
                                        }
                                    });
                                    perms.blocks = if only.is_empty() { Blocks::None } else { Blocks::Only(only) };
                                }
                                if perms != guest.perms {
                                    allow.push((id.clone(), perms));
                                }
                                ui.add_space(4.0);
                            }
                            kit::caption(ui, &theme, t!("A new guest only watches. Tick what each may change; you can change it at any time."));
                        }
                        Some(Role::Guest { side, welcomed, .. }) => {
                            if *welcomed {
                                kit::done_label(ui, tf!("You are in {name}'s session", name = side.host), theme.action);
                                let p = &side.perms;
                                let mut may: Vec<&str> = Vec::new();
                                if p.power {
                                    may.push(t!("the bot"));
                                }
                                if p.commands {
                                    may.push(t!("slash commands"));
                                }
                                if p.workspaces {
                                    may.push(t!("workspaces"));
                                }
                                match &p.blocks {
                                    Blocks::All => may.push(t!("blocks, everywhere")),
                                    Blocks::Only(_) => may.push(t!("blocks, in some workspaces")),
                                    Blocks::None => {}
                                }
                                ui.label(if may.is_empty() { t!("You are watching: the host hasn't let you change anything yet.").to_string() } else { tf!("You may change: {list}", list = may.join(", ")) });
                                if let Some(state) = &self.bot {
                                    ui.label(match state.as_str() {
                                        "running" => t!("The bot is running at the host's."),
                                        "starting" => t!("The bot is starting at the host's."),
                                        _ => t!("The bot is stopped."),
                                    });
                                }
                                kit::caption(ui, &theme, t!("The project is saved at the host's, and the bot runs there. Your copy is only for this session."));
                            } else {
                                ui.horizontal(|ui| {
                                    ui.spinner();
                                    ui.label(RichText::new(t!("Joining…")).color(theme.text_muted));
                                });
                            }
                        }
                    }
                    ui.add_space(8.0);
                    if let Some((ok, text)) = &self.said {
                        ui.label(RichText::new(text).color(if *ok { theme.action } else { theme.warning }));
                    }
                }
                Part::Footer => {
                    match self.session.as_ref().map(|s| &s.role) {
                        Some(Role::Host { .. }) => end = kit::secondary_button(ui, &theme, t!("End the session")).clicked(),
                        Some(Role::Guest { .. }) => end = kit::secondary_button(ui, &theme, t!("Leave the session")).clicked(),
                        None => {}
                    }
                    close = kit::secondary_button(ui, &theme, t!("Close")).clicked();
                }
            },
        );
        if let Some(Session { wire, role: Role::Host { side, .. }, .. }) = self.session.as_mut() {
            for (id, perms) in allow {
                for (to, msg) in side.allow(&id, perms) {
                    wire.send(to.as_deref(), serde_json::to_value(&msg).unwrap_or_default());
                }
            }
            if let Some(id) = kick {
                wire.kick(&id);
                side.left(&id);
            }
        }
        if start {
            self.start(ed);
        }
        if join {
            self.join();
        }
        if end {
            self.end();
        }
        if close {
            self.open = false;
        }
    }
}

/// A colour for someone in the session: the same for the same person, one
/// of a few that stand out on the canvas.
fn colour_of(id: &str) -> egui::Color32 {
    const COLOURS: [(u8, u8, u8); 6] = [(0xE9, 0x5A, 0x8C), (0xF0, 0x8A, 0x24), (0x8E, 0x6C, 0xEF), (0x2B, 0xA8, 0x7E), (0x3D, 0x8B, 0xFD), (0xD9, 0x48, 0x41)];
    let n = id.bytes().fold(0usize, |n, b| n.wrapping_mul(31).wrapping_add(b as usize));
    let (r, g, b) = COLOURS[n % COLOURS.len()];
    egui::Color32::from_rgb(r, g, b)
}

fn level_name(level: Level) -> &'static str {
    match level {
        Level::Info => "info",
        Level::Success => "success",
        Level::Warn => "warn",
        Level::Error => "error",
        Level::Mcp => "mcp",
    }
}

fn level_of(name: &str) -> Level {
    match name {
        "success" => Level::Success,
        "warn" => Level::Warn,
        "error" => Level::Error,
        _ => Level::Info,
    }
}

/// Writes a guest's copy of the session's project: its other files, and
/// each workspace.
fn write_copy(folder: &std::path::Path, files: &BTreeMap<String, String>, docs: &Docs) -> anyhow::Result<()> {
    let _ = std::fs::remove_dir_all(folder);
    std::fs::create_dir_all(folder.join("workspaces"))?;
    for (path, text) in files {
        // Only plain names: nothing that could land outside the folder.
        if path.contains("..") || path.starts_with(['/', '\\']) || path.contains(':') {
            continue;
        }
        let to = folder.join(path);
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(to, text)?;
    }
    for (key, state) in docs.iter().filter(|(k, _)| *k != META) {
        if key.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
            std::fs::write(folder.join("workspaces").join(format!("{key}.json")), serde_json::to_string_pretty(state)? + "\n")?;
        }
    }
    if let Some(commands) = docs.get(META).map(|m| &m["slashCommands"]).and_then(Value::as_array).filter(|c| !c.is_empty()) {
        std::fs::create_dir_all(folder.join("commands"))?;
        for command in commands {
            let name: String = command["name"].as_str().unwrap_or("command").chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_').collect();
            std::fs::write(folder.join("commands").join(format!("{name}.json")), serde_json::to_string_pretty(&serde_json::json!([command]))? + "\n")?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use circuit_core::graph::Node;

    fn editor(dir: &std::path::Path, name: &str) -> Editor {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        circuit_core::Project::create(dir, name, "none").unwrap();
        let mut ed = Editor::new(vec![root.join("plugins")], None).unwrap();
        ed.open_project(dir);
        ed
    }

    /// The project goes into documents and comes back the same, and a
    /// change from someone else doesn't end up in one's own undo.
    #[test]
    fn a_project_follows_the_sessions_documents() {
        let dir = std::env::temp_dir().join(format!("xc-live-ui-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut ed = editor(&dir.join("bot"), "Bot");
        let log = |text: &str| Node { block: "core.log@1".into(), values: serde_json::json!({ "message": text }).as_object().cloned().unwrap(), ..Default::default() };
        ed.project.as_mut().unwrap().workspaces["main"].nodes.insert("1".into(), log("mine"));
        ed.history.track(ed.project.as_ref().unwrap(), true);
        let docs = docs_of(&mut ed);
        assert!(docs.contains_key(META) && docs["main"]["nodes"]["1"]["uid"].as_str().is_some_and(|u| !u.is_empty()), "blocks get their ids");
        assert_eq!(docs_of(&mut ed), docs, "the same again");

        // Someone else adds a block: it is there, and Undo leaves it there
        // while taking back my own block.
        let before = docs["main"].clone();
        let mut theirs = before.clone();
        theirs["nodes"]["2"] = serde_json::to_value(Node { uid: "theirs".into(), ..log("theirs") }).unwrap();
        let (b, a) = (before.clone(), theirs.clone());
        put(&mut ed, "main", Some(&theirs), move |earlier| json::workspace_numbered(Some(&b), earlier, &a, &|_| Some(Side::Theirs), Side::Theirs).value);
        assert_eq!(docs_of(&mut ed)["main"], theirs, "back out as it went in");
        let project = ed.project.as_mut().unwrap();
        let ws = project.workspaces.get_mut("main").unwrap();
        assert!(ed.history.undo("main", ws));
        assert!(!ws.nodes.contains_key("1") && ws.nodes.contains_key("2"), "my block is undone, theirs stays: {:?}", ws.nodes.keys().collect::<Vec<_>>());
        assert!(ed.history.redo("main", ws) && ws.nodes.len() == 2);

        // A workspace made and deleted by someone else, and the slash commands.
        put(&mut ed, "extra", Some(&serde_json::json!({ "name": "Extra" })), |v| v.clone());
        assert_eq!(ed.project.as_ref().unwrap().workspaces["extra"].name, "Extra");
        put(&mut ed, "extra", None, |v| v.clone());
        assert!(!ed.project.as_ref().unwrap().workspaces.contains_key("extra"));
        put(&mut ed, META, Some(&serde_json::json!({ "slashCommands": [{ "name": "ping" }] })), |v| v.clone());
        assert_eq!(docs_of(&mut ed)[META]["slashCommands"][0]["name"], "ping");

        // A guest's copy is a project that opens.
        let copy = dir.join("copy");
        write_copy(&copy, &other_files(&dir.join("bot")), &docs_of(&mut ed)).unwrap();
        write_copy(&copy, &BTreeMap::from([("../outside.txt".to_string(), "x".to_string())]), &Docs::new()).unwrap();
        assert!(!dir.join("outside.txt").exists(), "nothing lands outside the folder");
        write_copy(&copy, &other_files(&dir.join("bot")), &docs_of(&mut ed)).unwrap();
        let opened = circuit_core::Project::load(&copy).unwrap();
        assert_eq!((opened.meta.name.as_str(), opened.workspaces["main"].nodes.len()), ("Bot", 2));
        assert_eq!(opened.meta.data["slashCommands"][0]["name"], "ping");
        assert!(guest_folder("xc-live:https://relay.example/0123abcd#KEY").ends_with("0123abcd"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Two editors in one session over a relay (`XC_RELAY_TEST` is its
    /// address): the guest arrives, watches, is let change blocks, and both
    /// end up the same. Skipped without one.
    #[test]
    fn two_apps_in_one_session() {
        let Ok(relay) = std::env::var("XC_RELAY_TEST") else { return };
        let dir = std::env::temp_dir().join(format!("xc-live-two-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        // SAFETY: only this test, run by hand, sets it.
        unsafe { std::env::set_var("XC_CIRCUIT_DATA", dir.join("data")) };
        let ctx = egui::Context::default();
        let mut host_ed = editor(&dir.join("bot"), "Bot");
        let mut host = LiveUi::new();
        host.open();
        host.name = "Ada".into();
        host.relay = relay;
        host.start(&mut host_ed);
        let mut guest_ed = editor(&dir.join("other"), "Other");
        let mut guest = LiveUi::new();
        guest.name = "Grace".into();
        let turn = |host: &mut LiveUi, host_ed: &mut Editor, guest: &mut LiveUi, guest_ed: &mut Editor, until: &mut dyn FnMut(&LiveUi, &Editor, &LiveUi, &Editor) -> bool| {
            for _ in 0..400 {
                host_ed.history.track(host_ed.project.as_ref().unwrap(), true);
                host.poll(host_ed, &ctx, "stopped");
                if let Some(project) = guest_ed.project.as_ref() {
                    guest_ed.history.track(project, true);
                }
                guest.poll(guest_ed, &ctx, "stopped");
                if let Some(folder) = guest.arrive.take() {
                    guest_ed.open_project(&folder);
                }
                if until(host, host_ed, guest, guest_ed) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            panic!("it didn't happen: host says {:?}, guest says {:?}", host.said, guest.said);
        };
        let invite_of = |ui: &LiveUi| match ui.session.as_ref().map(|s| &s.role) {
            Some(Role::Host { invite, .. }) => invite.clone(),
            _ => None,
        };
        turn(&mut host, &mut host_ed, &mut guest, &mut guest_ed, &mut |h, _, _, _| invite_of(h).is_some());
        guest.invite = invite_of(&host).unwrap();
        guest.join();
        turn(&mut host, &mut host_ed, &mut guest, &mut guest_ed, &mut |_, _, g, ged| g.is_guest() && ged.project.as_ref().is_some_and(|p| p.meta.name == "Bot"));
        assert_eq!(host.status().as_deref(), Some("Live: 1 guests"));

        // The host adds a block: the guest sees it.
        let log = |text: &str| Node { block: "core.log@1".into(), values: serde_json::json!({ "message": text }).as_object().cloned().unwrap(), ..Default::default() };
        host_ed.project.as_mut().unwrap().workspaces["main"].nodes.insert("1".into(), log("from the host"));
        turn(&mut host, &mut host_ed, &mut guest, &mut guest_ed, &mut |_, _, _, ged| ged.project.as_ref().unwrap().workspaces["main"].nodes.contains_key("1"));
        // The guest only watches: their block is taken back.
        guest_ed.project.as_mut().unwrap().workspaces["main"].nodes.insert("2".into(), log("from the guest"));
        turn(&mut host, &mut host_ed, &mut guest, &mut guest_ed, &mut |_, _, g, ged| !ged.project.as_ref().unwrap().workspaces["main"].nodes.contains_key("2") && g.said.as_ref().is_some_and(|(_, t)| t.contains("may not change the blocks")));
        assert_eq!(host_ed.project.as_ref().unwrap().workspaces["main"].nodes.len(), 1);
        // Let in: now it arrives at the host's.
        if let Some(Session { wire, role: Role::Host { side, .. }, .. }) = host.session.as_mut() {
            let id = side.guests.keys().next().unwrap().clone();
            for (to, msg) in side.allow(&id, Perms { blocks: Blocks::All, ..Default::default() }) {
                wire.send(to.as_deref(), serde_json::to_value(&msg).unwrap());
            }
        }
        turn(&mut host, &mut host_ed, &mut guest, &mut guest_ed, &mut |_, _, g, _| matches!(g.session.as_ref().map(|s| &s.role), Some(Role::Guest { side, .. }) if side.perms.blocks == Blocks::All));
        guest_ed.project.as_mut().unwrap().workspaces["main"].nodes.insert("2".into(), log("from the guest"));
        turn(&mut host, &mut host_ed, &mut guest, &mut guest_ed, &mut |_, hed, _, _| hed.project.as_ref().unwrap().workspaces["main"].nodes.values().any(|n| n.values["message"] == "from the guest"));
        let said = |ed: &Editor| -> BTreeSet<String> { ed.project.as_ref().unwrap().workspaces["main"].nodes.values().map(|n| format!("{} {}", n.uid, n.values["message"])).collect() };
        turn(&mut host, &mut host_ed, &mut guest, &mut guest_ed, &mut |_, hed, _, ged| said(hed).len() == 2 && said(hed) == said(ged));

        // Where the guest is shows at the host's: the workspace, the
        // pointer, and the block they have selected.
        let block = guest_ed.project.as_ref().unwrap().workspaces["main"].nodes.keys().next().unwrap().clone();
        guest_ed.active_workspace = Some("main".into());
        guest_ed.selected = Some(("main".into(), block.clone()));
        guest_ed.views.entry("main".into()).or_default().pointer = Some(egui::pos2(120.0, 80.0));
        turn(&mut host, &mut host_ed, &mut guest, &mut guest_ed, &mut |_, hed, _, _| hed.live_others.iter().any(|o| o.name == "Grace" && o.ws == "main" && o.at == Some([120.0, 80.0]) && o.selected == [block.clone()]));
        host_ed.active_workspace = Some("main".into());
        turn(&mut host, &mut host_ed, &mut guest, &mut guest_ed, &mut |_, _, _, ged| ged.live_others.iter().any(|o| o.name == "Ada" && o.ws == "main"));

        // What the host's console says reaches the guest.
        host_ed.log(Level::Success, "Bot ready, says the host's console");
        turn(&mut host, &mut host_ed, &mut guest, &mut guest_ed, &mut |_, _, _, ged| ged.log.iter().any(|l| l.text == "Bot ready, says the host's console" && l.level == Level::Success));

        // The bot: a guest's Run is refused until the host allows it.
        guest.ask_power(&mut guest_ed, Power::Start);
        assert!(guest_ed.log.last().unwrap().text.contains("only they can start it") && host.power.is_none());
        if let Some(Session { wire, role: Role::Host { side, .. }, .. }) = host.session.as_mut() {
            let id = side.guests.keys().next().unwrap().clone();
            for (to, msg) in side.allow(&id, Perms { blocks: Blocks::All, power: true, ..Default::default() }) {
                wire.send(to.as_deref(), serde_json::to_value(&msg).unwrap());
            }
        }
        turn(&mut host, &mut host_ed, &mut guest, &mut guest_ed, &mut |_, _, g, _| matches!(g.session.as_ref().map(|s| &s.role), Some(Role::Guest { side, .. }) if side.perms.power));
        guest.ask_power(&mut guest_ed, Power::Start);
        turn(&mut host, &mut host_ed, &mut guest, &mut guest_ed, &mut |h, _, _, _| h.power == Some(Power::Start));
        assert!(host_ed.log.iter().any(|l| l.text == "Grace started the bot"));

        // The host ends it: over for the guest, whose copy stays open.
        host.end();
        turn(&mut host, &mut host_ed, &mut guest, &mut guest_ed, &mut |_, _, g, _| !g.is_guest());
        assert!(guest.said.as_ref().unwrap().1.contains("over") && guest_ed.live_others.is_empty(), "and the pointers are gone");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
