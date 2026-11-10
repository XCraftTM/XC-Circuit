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
    /// A guest's session ended: the copy is to be closed.
    ended: bool,
    /// The start page's Join a live session… is open.
    joining: bool,
    /// Where each of the others is drawn: their pointer glides there
    /// instead of jumping from one place said to the next.
    drawn: BTreeMap<String, [f32; 2]>,
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
    /// The host: the profile the guests have, the server list each guest
    /// has (which list, by where it lives; 0: none), and up to when blocks
    /// that ran were passed on.
    told_profile: Option<crate::bot::profile::BotProfile>,
    told_servers: BTreeMap<String, usize>,
    told_ran: Option<Instant>,
    /// A guest: the invitation they came with, and after a dropped
    /// connection, when to try again and the how-manieth time that is.
    came_with: String,
    retry: Option<(Instant, u32)>,
    /// The host's plugins that don't come with the app: id, version, folder.
    plugin_dirs: Vec<(String, String, PathBuf)>,
    /// A guest: the parts of the host's plugins that have arrived, by id.
    plugin_parts: BTreeMap<String, (String, Vec<Option<String>>)>,
    /// A guest: the server list and the profile the host gave, shown again
    /// when the guest's own (empty) read comes in over them.
    given_servers: Option<std::sync::Arc<Vec<crate::bot::servers::Server>>>,
    given_profile: Option<crate::bot::profile::BotProfile>,
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

    /// The start page's Join a live session…
    pub fn ask_to_join(&mut self) {
        self.open();
        self.open = false;
        self.joining = true;
        self.said = None;
    }

    /// The window that asks for a name and an invitation, where no project
    /// is open. Joined, the session's project opens; when the session is
    /// over, it closes again.
    pub fn join_dialog(&mut self, ctx: &egui::Context, ed: &Editor) {
        if !self.joining {
            return;
        }
        // In: the project takes over from here.
        if matches!(self.session.as_ref().map(|s| &s.role), Some(Role::Guest { welcomed: true, .. })) {
            self.joining = false;
            return;
        }
        let theme = ed.theme.clone();
        let icon = ed.reg.icons.get("user-plus").cloned();
        let waiting = self.session.is_some();
        let (mut join, mut cancel) = (false, false);
        let closed = dialog::show(
            ctx,
            &theme,
            dialog::Spec { id: "live-join", title: t!("Join a live session"), subtitle: Some(t!("Work in someone's project with them, while they have it open")), icon: icon.as_deref(), width: 560.0 },
            |ui, part| match part {
                Part::Body => {
                    kit::field(ui, &theme, t!("Your name"), Some(t!("How the others see you.")), |ui| kit::text_input(ui, &mut self.name, "", false));
                    kit::field(ui, &theme, t!("Invitation"), Some(t!("The text the host sent you. It starts with xc-live:")), |ui| kit::text_input(ui, &mut self.invite, "xc-live:…", false));
                    kit::caption(ui, &theme, t!("The host's project opens here for as long as the session lasts. It is saved at the host's, and the bot runs there."));
                    ui.add_space(6.0);
                    if waiting {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(RichText::new(t!("Joining…")).color(theme.text_muted));
                        });
                    } else if let Some((ok, text)) = &self.said {
                        ui.label(RichText::new(text).color(if *ok { theme.action } else { theme.warning }));
                    }
                }
                Part::Footer => {
                    ui.add_enabled_ui(!waiting && !self.invite.trim().is_empty() && !self.name.trim().is_empty(), |ui| {
                        join = kit::primary_button(ui, &theme, t!("Join")).clicked();
                    });
                    cancel = kit::secondary_button(ui, &theme, t!("Cancel")).clicked();
                }
            },
        );
        if join {
            self.join();
        }
        if cancel || closed {
            self.joining = false;
            if waiting {
                self.end();
                self.said = None;
            }
        }
    }

    /// Whether this app is a guest in a session: the bot runs at the host's.
    pub fn is_guest(&self) -> bool {
        matches!(self.session.as_ref().map(|s| &s.role), Some(Role::Guest { .. }))
    }

    /// For a guest: how the bot at the host's stands, as last told.
    pub fn guest_bot(&self) -> Option<super::BotState> {
        if !self.is_guest() {
            return None;
        }
        Some(match self.bot.as_deref() {
            Some("running") => super::BotState::Running,
            Some("starting") => super::BotState::Starting,
            _ => super::BotState::Stopped,
        })
    }

    /// Whether a guest's session just ended: once, for the app to close
    /// the copy and go back to the start page.
    pub fn take_ended(&mut self) -> bool {
        std::mem::take(&mut self.ended)
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
                let mut side = HostSide::new(self.name.trim(), VERSION, docs);
                // The plugins a guest may lack: all but the app's own.
                self.plugin_dirs = ed.reg.plugins.iter().filter(|p| !ed.bundled_plugins.contains(&p.manifest.id) && !ed.optional_plugins.iter().any(|(id, _)| *id == p.manifest.id)).map(|p| (p.manifest.id.clone(), p.manifest.version.clone(), p.dir.clone())).collect();
                side.plugins = self.plugin_dirs.iter().map(|(id, version, _)| (id.clone(), version.clone())).collect();
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
                self.came_with = self.invite.trim().to_string();
                self.retry = None;
                self.plugin_parts.clear();
                self.session = Some(Session { wire, role: Role::Guest { side: GuestSide::new(), name: self.name.trim().to_string(), folder, welcomed: false }, seen: None, looked: Instant::now() });
                self.said = None;
            }
            Err(e) => self.said = Some((false, format!("{e:#}"))),
        }
    }

    /// Ends the session (the host), or leaves it (a guest).
    pub fn end(&mut self) {
        self.retry = None;
        self.ended = self.is_guest();
        self.drawn.clear();
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
        if self.session.is_none() && !ed.live_others.is_empty() {
            // No session, nobody to show.
            ed.live_others.clear();
        }
        let Some(session) = self.session.as_mut() else { return };
        // After a dropped connection: back in with the same invitation.
        if let Some((when, tries)) = self.retry
            && Instant::now() >= when
        {
            if let Ok(wire) = Wire::join(&self.came_with) {
                session.wire = wire;
            }
            // Under way: the next word from the wire decides what follows.
            self.retry = Some((Instant::now() + Duration::from_secs(3600), tries));
        }
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
                        // The plugins the guest may lack follow, in parts.
                        for (plugin, version, dir) in &self.plugin_dirs {
                            match pack_plugin(dir) {
                                Some(text) => {
                                    let parts: Vec<String> = text.as_bytes().chunks(120_000).map(|c| String::from_utf8_lossy(c).into_owned()).collect();
                                    for (part, text) in parts.iter().enumerate() {
                                        session.wire.send(Some(&id), serde_json::to_value(Msg::Plugin { id: plugin.clone(), version: version.clone(), part, parts: parts.len(), text: text.clone() }).unwrap_or_default());
                                    }
                                }
                                None => ed.log(Level::Warn, tf!("The plugin {plugin} is too big to pass on to guests: they see its blocks as unknown.", plugin = plugin)),
                            }
                        }
                        // The newcomer learns where everyone is, and how the bot stands.
                        self.here = None;
                        self.told.1.clear();
                        self.told_profile = None;
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
                (Role::Guest { name, side, welcomed, .. }, Event::Joined) => {
                    let hello = if *welcomed { side.hello_again(name, VERSION) } else { GuestSide::hello(name, VERSION) };
                    session.wire.send(None, serde_json::to_value(hello).unwrap_or_default());
                }
                (Role::Guest { side, folder, welcomed, .. }, Event::From { data, .. }) => {
                    let Ok(msg) = serde_json::from_value::<Msg>(data) else { continue };
                    let now = if *welcomed { docs_of(ed) } else { Docs::new() };
                    let turn = side.receive(msg, &now);
                    if let Some(why) = turn.refused {
                        over = Some(why);
                        continue;
                    }
                    if turn.back {
                        self.retry = None;
                        // What changed here while away is looked at again, and sent.
                        session.seen = None;
                        self.said = Some((true, t!("Back in the session.").into()));
                        ed.log(Level::Success, t!("Back in the session."));
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
                    for msg in turn.show {
                        match msg {
                            Msg::Profile { name, picture } => {
                                // The picture goes beside the copy, as a file to show.
                                let file = picture.and_then(|p| base64::Engine::decode(&base64::engine::general_purpose::STANDARD, p).ok()).filter(|b| b.len() <= 2_000_000).and_then(|bytes| {
                                    let path = folder.join(".circuit").join("bot-picture.png");
                                    std::fs::create_dir_all(path.parent()?).ok()?;
                                    std::fs::write(&path, bytes).ok()?;
                                    Some(path)
                                });
                                self.given_profile = Some(crate::bot::profile::BotProfile { name, avatar: file });
                            }
                            Msg::Servers { servers } => {
                                self.given_servers = serde_json::from_value::<Vec<crate::bot::servers::Server>>(servers).ok().filter(|l| !l.is_empty()).map(std::sync::Arc::new);
                                if self.given_servers.is_none() {
                                    ed.servers.shown = None;
                                }
                            }
                            Msg::Ran { blocks } => {
                                for (ws, node, error, ms) in blocks {
                                    let now = Instant::now();
                                    ed.activity.insert((ws, node), crate::bot::Activity { at: now, error, ms: ms.map(|ms| (ms, now)) });
                                }
                            }
                            Msg::Values { values } => {
                                for (ws, node, pin, value) in values {
                                    ed.wire_values.insert((ws, node, pin), (value, Instant::now()));
                                }
                            }
                            Msg::Plugin { id, version, part, parts, text } => {
                                if parts == 0 || parts > 200 || part >= parts {
                                    continue;
                                }
                                let entry = self.plugin_parts.entry(id.clone()).or_insert_with(|| (version.clone(), vec![None; parts]));
                                if entry.1.len() != parts {
                                    *entry = (version.clone(), vec![None; parts]);
                                }
                                entry.1[part] = Some(text);
                                if entry.1.iter().all(Option::is_some) {
                                    let (version, pieces) = self.plugin_parts.remove(&id).unwrap_or_default();
                                    let text: String = pieces.into_iter().flatten().collect();
                                    // One this app has the same version of is used as it is.
                                    let have = ed.reg.plugins.iter().any(|p| p.manifest.id == id && p.manifest.version == version && !p.dir.starts_with(&*folder));
                                    if !have && unpack_plugin(folder, &id, &text).is_ok() {
                                        // Someone else's code: the app asks before it loads.
                                        ed.reload_plugins();
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
                // A guest whose connection dropped tries to get back in,
                // for a while; a session that is over is over.
                (Role::Guest { welcomed, .. }, Event::Closed { why }) if *welcomed && !why.is_empty() && !why.contains("isn't open") && self.retry.is_none_or(|(_, tries)| tries < 40) => {
                    let tries = self.retry.map_or(0, |(_, tries)| tries);
                    self.retry = Some((Instant::now() + Duration::from_secs(3), tries + 1));
                    self.said = Some((false, t!("The connection dropped. Trying to get back in…").into()));
                }
                (_, Event::Closed { why }) => over = Some(why),
                _ => {}
            }
        }
        if let Some(why) = over {
            self.ended = self.is_guest();
            self.session = None;
            self.retry = None;
            self.drawn.clear();
            self.others.clear();
            ed.live_others.clear();
            self.bot = None;
            self.given_servers = None;
            self.given_profile = None;
            self.told_servers.clear();
            self.told_profile = None;
            let text = if why.is_empty() { t!("The session is over.").to_string() } else { tf!("The session is over: {why}", why = why) };
            ed.log(Level::Info, text.clone());
            self.said = Some((why.is_empty(), text));
            return;
        }
        // What changed here goes out: looked at when the project's revision
        // moved, a few times a second at most.
        let revision = ed.history.revision();
        let ready = match &session.role {
            // Not while the connection is away: nothing would arrive.
            Role::Guest { welcomed, folder, .. } => *welcomed && self.retry.is_none() && ed.project.as_ref().is_some_and(|p| p.root == *folder),
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
            if self.here.as_ref().is_none_or(|(last, when)| *last != here && when.elapsed() > Duration::from_millis(50)) {
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
        // The host passes on what it read with the bot's token, and what
        // the bot just did; a guest shows what it was given.
        match &session.role {
            Role::Host { side, .. } if !side.guests.is_empty() => {
                if self.told_profile != ed.bot_profile.shown
                    && let Some(profile) = ed.bot_profile.shown.clone()
                {
                    let picture = profile.avatar.as_ref().and_then(|p| std::fs::read(p).ok()).filter(|b| b.len() <= 150_000).map(|b| base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b));
                    session.wire.send(None, serde_json::to_value(Msg::Profile { name: profile.name.clone(), picture }).unwrap_or_default());
                    self.told_profile = Some(profile);
                }
                self.told_servers.retain(|id, _| side.guests.contains_key(id));
                for (id, guest) in &side.guests {
                    let list = ed.servers.shown.as_ref().filter(|_| guest.perms.servers);
                    let which = list.map_or(0, |l| std::sync::Arc::as_ptr(l) as usize);
                    if self.told_servers.get(id).copied().unwrap_or(0) != which {
                        self.told_servers.insert(id.clone(), which);
                        let servers = list.map(|l| serde_json::to_value(&**l).unwrap_or_default()).unwrap_or_else(|| serde_json::json!([]));
                        session.wire.send(Some(id), serde_json::to_value(Msg::Servers { servers }).unwrap_or_default());
                    }
                }
                // Blocks that ran since the last look, and the values on
                // their wires for those who may see them.
                let since = self.told_ran.unwrap_or_else(Instant::now);
                let now = Instant::now();
                let blocks: Vec<(String, String, Option<String>, Option<f64>)> = ed.activity.iter().filter(|(_, a)| a.at > since && a.at <= now).map(|((ws, node), a)| (ws.clone(), node.clone(), a.error.clone(), a.ms.map(|(ms, _)| ms))).collect();
                if !blocks.is_empty() {
                    session.wire.send(None, serde_json::to_value(Msg::Ran { blocks }).unwrap_or_default());
                }
                let values: Vec<(String, String, String, String)> = ed.wire_values.iter().filter(|(_, (_, at))| *at > since && *at <= now).map(|((ws, node, pin), (value, _))| (ws.clone(), node.clone(), pin.clone(), value.chars().take(2000).collect())).take(500).collect();
                if !values.is_empty() {
                    for (id, _) in side.guests.iter().filter(|(_, g)| g.perms.values) {
                        session.wire.send(Some(id), serde_json::to_value(Msg::Values { values: values.clone() }).unwrap_or_default());
                    }
                }
                self.told_ran = Some(now);
            }
            Role::Host { .. } => self.told_ran = Some(Instant::now()),
            Role::Guest { .. } => {
                if let Some(list) = &self.given_servers
                    && !ed.servers.busy()
                    && !ed.servers.shown.as_ref().is_some_and(|s| std::sync::Arc::ptr_eq(s, list))
                {
                    ed.servers.show(list.clone());
                }
                if let Some(profile) = &self.given_profile
                    && !ed.bot_profile.busy()
                    && ed.bot_profile.shown.as_ref() != Some(profile)
                {
                    ed.bot_profile.show(profile.clone());
                }
            }
        }
        // Each pointer glides towards where its owner last said it is: a
        // share of the way every frame, so it moves evenly between the few
        // places a second that come over the wire.
        let step = (ctx.input(|i| i.stable_dt).min(0.1) * 14.0).clamp(0.05, 1.0);
        let mut moving = false;
        self.drawn.retain(|id, _| self.others.get(id).is_some_and(|o| o.at.is_some()));
        for (id, other) in &self.others {
            let Some(to) = other.at else { continue };
            let at = self.drawn.entry(id.clone()).or_insert(to);
            let (dx, dy) = (to[0] - at[0], to[1] - at[1]);
            // Far away (another part of the canvas): there at once.
            if dx.abs() + dy.abs() > 1500.0 || dx.abs() + dy.abs() < 0.5 {
                *at = to;
            } else {
                *at = [at[0] + dx * step, at[1] + dy * step];
                moving = true;
            }
        }
        ed.live_others = self.others.iter().filter_map(|(id, o)| Some(crate::editor::LiveOther { name: o.name.clone(), color: colour_of(&o.id), ws: o.ws.clone()?, at: self.drawn.get(id).copied(), selected: o.selected.clone() })).collect();
        if moving {
            ctx.request_repaint();
        } else {
            ctx.request_repaint_after(Duration::from_millis(if self.others.is_empty() { 250 } else { 60 }));
        }
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
                                    kit::checkbox(ui, &theme, &mut perms.servers, t!("Servers, channels, roles")).on_hover_text(t!("Lets the guest pick from the bot's servers, channels and roles, which you read with the bot's token. Without it a field only shows the id in it."));
                                    kit::checkbox(ui, &theme, &mut perms.values, t!("Values on wires")).on_hover_text(t!("Lets the guest see what went through the wires when the bot ran: real members' names and messages. Which blocks ran and what failed, every guest sees."));
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
                for (to, msg) in side.sent_away(&id) {
                    wire.send(to.as_deref(), serde_json::to_value(&msg).unwrap_or_default());
                }
                self.others.remove(&id);
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

/// A plugin's files as one text, to pass on: each file by its path, as
/// base64. `None` when it is more than a session should carry.
fn pack_plugin(dir: &std::path::Path) -> Option<String> {
    fn walk(dir: &std::path::Path, base: &std::path::Path, files: &mut BTreeMap<String, String>, total: &mut usize) -> Option<()> {
        for entry in std::fs::read_dir(dir).ok()?.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') || name == "node_modules" {
                continue;
            }
            let path = entry.path();
            if entry.file_type().ok()?.is_dir() {
                walk(&path, base, files, total)?;
            } else {
                let bytes = std::fs::read(&path).ok()?;
                *total += bytes.len();
                if *total > 4_000_000 {
                    return None;
                }
                let rel = path.strip_prefix(base).ok()?.to_string_lossy().replace('\\', "/");
                files.insert(rel, base64::Engine::encode(&base64::engine::general_purpose::STANDARD, bytes));
            }
        }
        Some(())
    }
    let mut files = BTreeMap::new();
    walk(dir, dir, &mut files, &mut 0)?;
    serde_json::to_string(&files).ok()
}

/// Writes a plugin the host passed on into the guest's copy, as one of the
/// project's own plugins: those only load once the user said yes.
fn unpack_plugin(folder: &std::path::Path, id: &str, text: &str) -> anyhow::Result<()> {
    let plain = |part: &str| !part.is_empty() && part != "." && part != ".." && !part.contains([':', '\\']);
    if !plain(id) || id.contains('/') {
        anyhow::bail!("not a plugin's id");
    }
    let files: BTreeMap<String, String> = serde_json::from_str(text)?;
    let to = folder.join("plugins").join(id);
    let _ = std::fs::remove_dir_all(&to);
    for (path, data) in files {
        // Only plain paths: nothing that could land outside the folder.
        if !path.split('/').all(plain) {
            continue;
        }
        let file = to.join(&path);
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(file, base64::Engine::decode(&base64::engine::general_purpose::STANDARD, data)?)?;
    }
    Ok(())
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

        // A plugin goes over as text and lands among the copy's own
        // plugins, which only load after a yes; nothing lands elsewhere.
        let plugin = dir.join("my.plugin");
        std::fs::create_dir_all(plugin.join("blocks").join("fun")).unwrap();
        std::fs::create_dir_all(plugin.join("node_modules").join("x")).unwrap();
        std::fs::write(plugin.join("plugin.toml"), "id = \"my.plugin\"\nname = \"Mine\"\nversion = \"1.0.0\"\n").unwrap();
        std::fs::write(plugin.join("blocks").join("fun").join("hi.js"), [0u8, 159, 146, 150]).unwrap();
        std::fs::write(plugin.join("node_modules").join("x").join("big.js"), "not passed on").unwrap();
        let text = pack_plugin(&plugin).unwrap();
        assert!(!text.contains("node_modules"));
        unpack_plugin(&copy, "my.plugin", &text).unwrap();
        assert_eq!(std::fs::read(copy.join("plugins/my.plugin/blocks/fun/hi.js")).unwrap(), [0u8, 159, 146, 150]);
        assert_eq!(crate::editor::trust::own_plugins(&copy).iter().map(|o| o.id.as_str()).collect::<Vec<_>>(), ["my.plugin"]);
        let evil = serde_json::json!({ "../../outside.js": "eA==", "ok.js": "eA==", "C:/x.js": "eA==" }).to_string();
        unpack_plugin(&copy, "other", &evil).unwrap();
        assert!(copy.join("plugins/other/ok.js").is_file() && !dir.join("outside.js").exists() && !copy.join("outside.js").exists());
        assert!(unpack_plugin(&copy, "../x", &text).is_err() && unpack_plugin(&copy, "a/b", &text).is_err());
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

        // What the host read with the bot's token: the profile reaches
        // every guest, the servers only one who may see them.
        host_ed.bot_profile.show(crate::bot::profile::BotProfile { name: "Circuit Bot".into(), avatar: None });
        let servers = std::sync::Arc::new(vec![crate::bot::servers::Server { id: "1".into(), name: "My server".into(), channels: vec![crate::bot::servers::Channel { id: "2".into(), name: "general".into(), kind: 0, parent: None, position: 0 }], roles: Vec::new() }]);
        host_ed.servers.show(servers.clone());
        turn(&mut host, &mut host_ed, &mut guest, &mut guest_ed, &mut |_, _, _, ged| ged.bot_profile.shown.as_ref().is_some_and(|p| p.name == "Circuit Bot"));
        assert!(guest_ed.servers.shown.is_none(), "not without the leave for it");
        if let Some(Session { wire, role: Role::Host { side, .. }, .. }) = host.session.as_mut() {
            let id = side.guests.keys().next().unwrap().clone();
            for (to, msg) in side.allow(&id, Perms { blocks: Blocks::All, power: true, servers: true, ..Default::default() }) {
                wire.send(to.as_deref(), serde_json::to_value(&msg).unwrap());
            }
        }
        turn(&mut host, &mut host_ed, &mut guest, &mut guest_ed, &mut |_, _, _, ged| ged.servers.shown.as_deref() == Some(&*servers));

        // A block that ran at the host's lights up at the guest's; the
        // value on its wire stays the host's without the leave for it.
        let now = Instant::now() + Duration::from_millis(5);
        std::thread::sleep(Duration::from_millis(30));
        host_ed.activity.insert(("main".into(), "1".into()), crate::bot::Activity { at: now, error: None, ms: Some((12.0, now)) });
        host_ed.wire_values.insert(("main".into(), "1".into(), "result".into()), ("a member's message".into(), now));
        turn(&mut host, &mut host_ed, &mut guest, &mut guest_ed, &mut |_, _, _, ged| ged.activity.get(&("main".to_string(), "1".to_string())).is_some_and(|a| a.ms.map(|m| m.0) == Some(12.0)));
        assert!(guest_ed.wire_values.is_empty());

        // The guest's connection drops, with a block of theirs not sent
        // yet: they get back in as who they were, and the block arrives.
        guest_ed.project.as_mut().unwrap().workspaces["main"].nodes.insert("7".into(), log("made while away"));
        guest.retry = Some((Instant::now(), 1));
        turn(&mut host, &mut host_ed, &mut guest, &mut guest_ed, &mut |_, _, g, _| g.retry.is_none() && g.said.as_ref().is_some_and(|(ok, text)| *ok && text.contains("Back in")));
        turn(&mut host, &mut host_ed, &mut guest, &mut guest_ed, &mut |h, hed, _, _| {
            let Some(Role::Host { side, .. }) = h.session.as_ref().map(|s| &s.role) else { return false };
            side.guests.len() == 1 && side.guests.values().all(|g| g.name == "Grace" && g.perms.power && g.perms.servers) && hed.project.as_ref().unwrap().workspaces["main"].nodes.values().any(|n| n.values["message"] == "made while away")
        });

        // The host ends it: over for the guest, whose copy stays open.
        host.end();
        turn(&mut host, &mut host_ed, &mut guest, &mut guest_ed, &mut |_, _, g, _| !g.is_guest());
        assert!(guest.said.as_ref().unwrap().1.contains("over") && guest_ed.live_others.is_empty(), "and the pointers are gone");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
