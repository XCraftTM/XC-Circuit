//! File › Git…: keeping the project in a Git repository (GitHub, GitLab,
//! Gitea …), to have it on several computers and to work on it with others.
//! Store records what changed and sends it; Fetch gets what is stored there
//! and puts the two together. The work itself is `circuit_sync`'s; the
//! sign-in is kept in the system's credential store, by the place's name.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, channel};
use std::thread;
use std::time::{Duration, Instant};

use circuit_runner::secrets;
use circuit_sync::github::{self, Answer};
use circuit_sync::{Login, Merged, Side, State, Sync, Version, Who};
use eframe::egui::{self, RichText};

use crate::editor::{Editor, Level};
use crate::ui::dialog::{self, Part};
use crate::ui::kit;

/// What a job on its own thread came back with.
enum Done {
    /// How the project stands, and who is signed in at its place.
    Read(Result<Option<State>, String>, Option<String>, Vec<Version>),
    /// Looked whether newer versions are stored there.
    Checked,
    /// Went back to an earlier version: how many files changed.
    WentBack(Result<usize, String>),
    Connected(Result<(), String>),
    Stored(Result<&'static str, String>),
    Fetched(Result<Merged, String>),
    Code(Result<github::Code, String>),
    Asked(Result<Answer, String>),
    SignedIn(Result<String, String>),
    /// A project fetched into a new folder.
    Got(Result<PathBuf, String>),
}

/// What was pressed in the Sign in part.
#[derive(Default)]
struct Asked {
    github_in: bool,
    token_in: bool,
    out: bool,
    cancel: bool,
}

/// Getting a stored project onto this computer: where to.
pub struct GetDraft {
    location: String,
    name: String,
    /// In a folder the user chooses, not the app's own.
    own_folder: bool,
}

/// Copies a folder with everything in it.
fn copy_dir(from: &std::path::Path, to: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

/// What in a text looks like a secret that shouldn't be stored where others
/// can read it.
fn looks_secret(text: &str) -> Option<&'static str> {
    use std::sync::OnceLock;
    static PATTERNS: OnceLock<Vec<(regex::Regex, u8)>> = OnceLock::new();
    let patterns = PATTERNS.get_or_init(|| {
        [
            (r"https://(?:\w+\.)?discord(?:app)?\.com/api/webhooks/\d+/[\w-]+", 0),
            (r"\b[\w-]{24,28}\.[\w-]{6}\.[\w-]{27,}", 1),
            (r"\b(?:gh[pousr]_[A-Za-z0-9]{30,}|github_pat_\w{30,}|sk-[A-Za-z0-9_-]{20,}|xox[abprs]-[A-Za-z0-9-]{10,})", 2),
            (r"-----BEGIN [A-Z ]*PRIVATE KEY-----", 3),
        ]
        .into_iter()
        .map(|(p, kind)| (regex::Regex::new(p).unwrap(), kind))
        .collect()
    });
    let kind = patterns.iter().find(|(re, _)| re.is_match(text))?.1;
    Some(match kind {
        0 => t!("a webhook address"),
        1 => t!("a bot token"),
        2 => t!("an access key"),
        _ => t!("a private key"),
    })
}

/// Where the project's blocks hold something that looks like a secret.
fn secrets_in(project: &circuit_core::Project) -> Vec<String> {
    fn walk(v: &serde_json::Value, found: &mut Option<&'static str>) {
        match v {
            serde_json::Value::String(s) if found.is_none() => *found = looks_secret(s),
            serde_json::Value::Array(a) => a.iter().for_each(|v| walk(v, found)),
            serde_json::Value::Object(o) => o.values().for_each(|v| walk(v, found)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for ws in project.workspaces.values() {
        for (id, node) in &ws.nodes {
            for (field, value) in &node.values {
                let mut found = None;
                walk(value, &mut found);
                if let Some(what) = found {
                    out.push(format!("{} › #{id} › {field}: {what}", ws.name));
                }
            }
        }
    }
    out
}

/// The folder name an address suggests: `my-bot` for `…/you/my-bot.git`.
fn name_of(address: &str) -> String {
    let last = address.trim().trim_end_matches('/').rsplit('/').next().unwrap_or_default();
    if host_of(address).is_none() { String::new() } else { last.trim_end_matches(".git").to_string() }
}

/// A sign-in on GitHub under way.
struct Waiting {
    code: github::Code,
    next: Instant,
    until: Instant,
    asking: bool,
}

#[derive(Default)]
pub struct GitUi {
    pub open: bool,
    root: Option<PathBuf>,
    /// How the project stands: `None` while it has no repository.
    state: Option<State>,
    /// The account signed in at the address's place.
    account: Option<String>,
    address: String,
    message: String,
    user: String,
    token: String,
    waiting: Option<Waiting>,
    /// What is running, and the answers still to come.
    busy: Option<&'static str>,
    jobs: Vec<Receiver<Done>>,
    /// The files both sides changed, and whose to keep.
    conflicts: Vec<String>,
    keep: HashMap<String, Side>,
    /// What the last action said, and whether it went well.
    said: Option<(bool, String)>,
    /// Get from Git: open while set.
    pub get: Option<GetDraft>,
    /// The project that Get from Git brought, for the app to open.
    got: Option<PathBuf>,
    /// Disconnect… was pressed: asked about first.
    disconnecting: bool,
    /// A project moved out of the app's own folder: where to, and from
    /// where, for the app to open it there.
    pub moved: Option<(PathBuf, PathBuf)>,
    /// The newest stored versions, and whether the list shows.
    history: Vec<Version>,
    show_history: bool,
    /// Whether newer versions were looked for since the project opened.
    checked: bool,
    /// What looks like a secret in the project's blocks, found when Store
    /// was pressed; and that the user said to store it anyway.
    secrets: Vec<String>,
    secrets_ok: bool,
    /// The project and its number of saves when its repository was last
    /// read, to read it again after the next save.
    seen: Option<(PathBuf, u64)>,
}

/// The place an address is at: `github.com` for
/// `https://github.com/you/bot.git`. `None` for an address that isn't HTTPS
/// (or HTTP): the SSH form (`git@github.com:you/bot.git`) isn't spoken.
pub fn host_of(address: &str) -> Option<String> {
    let rest = address.trim().strip_prefix("https://").or_else(|| address.trim().strip_prefix("http://"))?;
    let host = rest.split('/').next()?.rsplit('@').next()?.to_ascii_lowercase();
    (!host.is_empty() && rest.contains('/')).then_some(host)
}

fn login_of(host: &str) -> Login {
    if host.is_empty() {
        return Login::default();
    }
    secrets::sync_login(host).map(|(user, token)| Login { user, token }).unwrap_or_default()
}

fn who_of(host: &str, login: &Login) -> Who {
    let name = if login.user.is_empty() { "XC Circuit".to_string() } else { login.user.clone() };
    Who { email: format!("{}@users.noreply.{host}", name.replace(' ', "-")), name }
}

fn said(e: anyhow::Error) -> String {
    format!("{e:#}")
}

impl GitUi {
    pub fn new() -> Self {
        Self::default()
    }

    /// File › Git…
    pub fn open(&mut self, ed: &Editor) {
        let Some(project) = &ed.project else { return };
        if self.root.as_deref() != Some(project.root.as_path()) {
            *self = GitUi::default();
            self.root = Some(project.root.clone());
        }
        self.open = true;
        self.read();
    }

    fn run(&mut self, what: Option<&'static str>, job: impl FnOnce() -> Done + Send + 'static) {
        let (tx, rx) = channel();
        if what.is_some() {
            self.busy = what;
        }
        self.jobs.push(rx);
        thread::spawn(move || {
            let _ = tx.send(job());
        });
    }

    /// Reads how the project stands (on its own thread: the credential store
    /// can take a moment).
    fn read(&mut self) {
        let root = self.root.clone();
        let typed = host_of(&self.address);
        self.run(None, move || {
            // Without a project (Get from Git) only who is signed in.
            let (state, history) = match root.as_deref().map(Sync::open) {
                Some(Ok(Some(sync))) => (sync.state().map(Some).map_err(said), sync.history(30).unwrap_or_default()),
                Some(Err(e)) => (Err(said(e)), Vec::new()),
                _ => (Ok(None), Vec::new()),
            };
            let host = typed.or_else(|| state.as_ref().ok().and_then(|s| s.as_ref()).and_then(|s| s.remote.as_deref()).and_then(host_of));
            let account = host.and_then(|h| secrets::sync_login(&h)).map(|(user, _)| if user.is_empty() { "…".to_string() } else { user });
            Done::Read(state, account, history)
        });
    }

    fn connect(&mut self) {
        let (Some(root), Some(host)) = (self.root.clone(), host_of(&self.address)) else { return };
        let address = self.address.trim().to_string();
        self.run(Some("Connecting"), move || {
            Done::Connected((|| {
                let sync = Sync::start(&root)?;
                sync.set_remote(&address)?;
                // Also the test that the address and the sign-in are right.
                sync.fetch(&login_of(&host))
            })()
            .map_err(said))
        });
    }

    /// What the status bar says: whether something waits, and the words.
    /// `None` for a project that isn't stored anywhere.
    pub fn status(&self) -> Option<(bool, String)> {
        let state = self.state.as_ref().filter(|s| s.remote.is_some())?;
        Some(if let Some(what) = self.busy {
            (true, tf!("{what} …", what = busy_text(what)))
        } else if !self.conflicts.is_empty() {
            (true, t!("Git: choose whose files stay").to_string())
        } else if state.behind > 0 {
            (true, tf!("Git: {count} to fetch", count = state.behind))
        } else if !state.changed.is_empty() || state.ahead > 0 {
            (true, tf!("Git: {count} to store", count = state.changed.len().max(state.ahead)))
        } else {
            (false, t!("Git: stored").to_string())
        })
    }

    /// Store by its shortcut: as the button, when the project is connected
    /// and nothing else runs. Otherwise the dialog opens.
    pub fn store_now(&mut self, ed: &mut Editor) {
        if self.ready() { self.store(ed) } else { self.open(ed) }
    }

    /// Fetch by its shortcut.
    pub fn fetch_now(&mut self, ed: &mut Editor) {
        if self.ready() && self.conflicts.is_empty() { self.fetch(ed) } else { self.open(ed) }
    }

    fn ready(&self) -> bool {
        self.busy.is_none() && self.state.as_ref().is_some_and(|s| s.remote.is_some())
    }

    /// Store: saves the project, records what changed and sends it.
    fn store(&mut self, ed: &mut Editor) {
        let (Some(root), Some(remote)) = (self.root.clone(), self.state.as_ref().and_then(|s| s.remote.clone())) else { return };
        // A repository set up by hand at an address with no place to sign in
        // at (a folder, say) is used as it is.
        let host = host_of(&remote).unwrap_or_default();
        // Something in a block that looks like a secret: asked about first,
        // as whoever can read the repository can read it.
        if !self.secrets_ok {
            let found = ed.project.as_ref().map(secrets_in).unwrap_or_default();
            if !found.is_empty() {
                self.secrets = found;
                self.open = true;
                return;
            }
        }
        self.secrets.clear();
        ed.save();
        ed.finish_saving();
        let message = if self.message.trim().is_empty() { "Changes".to_string() } else { self.message.trim().to_string() };
        self.run(Some("Storing"), move || {
            Done::Stored((|| {
                let login = login_of(&host);
                let Some(sync) = Sync::open(&root)? else { anyhow::bail!("The project has no repository yet.") };
                let recorded = sync.store(&message, &who_of(&host, &login))?;
                if !recorded && sync.state()?.ahead == 0 {
                    return Ok("nothing");
                }
                sync.send(&login)?;
                Ok("stored")
            })()
            .map_err(said))
        });
    }

    /// Goes back to an earlier version: what changed here is recorded
    /// first, and going back is a new version on top, so nothing is lost.
    fn go_back(&mut self, ed: &mut Editor, id: String) {
        let Some(root) = self.root.clone() else { return };
        let remote = self.state.as_ref().and_then(|s| s.remote.clone());
        let host = remote.as_deref().and_then(host_of).unwrap_or_default();
        ed.save();
        ed.finish_saving();
        self.run(Some("Going back"), move || {
            Done::WentBack((|| {
                let login = login_of(&host);
                let who = who_of(&host, &login);
                let Some(sync) = Sync::open(&root)? else { anyhow::bail!("The project has no repository yet.") };
                sync.store("Changes on this computer", &who)?;
                let files = sync.go_back(&id, &who)?;
                // Sent too, where that works now; otherwise with the next Store.
                if remote.is_some() {
                    let _ = sync.send(&login);
                }
                Ok(files.len())
            })()
            .map_err(said))
        });
    }

    /// Forgets where the project is stored. With `move_out`, a project kept
    /// in the app's own folder also moves into a folder the user picks.
    fn disconnect(&mut self, ed: &mut Editor, move_out: bool) {
        let Some(root) = self.root.clone() else { return };
        let to = if move_out {
            let Some(place) = rfd::FileDialog::new().set_title(t!("Where the project goes")).pick_folder() else { return };
            let to = place.join(root.file_name().unwrap_or_default());
            if to.exists() {
                self.said = Some((false, tf!("There is already a folder called {name} there.", name = to.file_name().unwrap_or_default().to_string_lossy())));
                return;
            }
            Some(to)
        } else {
            None
        };
        ed.save();
        ed.finish_saving();
        self.disconnecting = false;
        match Sync::open(&root).and_then(|s| s.map(|s| s.forget_remote()).transpose()) {
            Ok(_) => {}
            Err(e) => {
                self.said = Some((false, said(e)));
                return;
            }
        }
        self.address.clear();
        self.said = Some((true, t!("Disconnected. The project stays on this computer.").into()));
        if let Some(to) = to {
            match copy_dir(&root, &to) {
                Ok(()) => self.moved = Some((to, root)),
                Err(e) => {
                    let _ = std::fs::remove_dir_all(&to);
                    self.said = Some((false, tf!("Disconnected, but the project couldn't be moved: {error}", error = e)));
                }
            }
        }
        self.read();
    }

    /// Looks whether newer versions are stored there, without changing
    /// anything: the status bar then says so.
    fn check(&mut self) {
        let (Some(root), Some(host)) = (self.root.clone(), self.state.as_ref().and_then(|s| s.remote.as_deref()).and_then(host_of)) else { return };
        self.run(None, move || {
            if let Ok(Some(sync)) = Sync::open(&root) {
                let _ = sync.fetch(&login_of(&host));
            }
            Done::Checked
        });
    }

    /// Fetch: saves and records what changed here, gets what is stored there
    /// and puts the two together.
    fn fetch(&mut self, ed: &mut Editor) {
        let (Some(root), Some(remote)) = (self.root.clone(), self.state.as_ref().and_then(|s| s.remote.clone())) else { return };
        // A repository set up by hand at an address with no place to sign in
        // at (a folder, say) is used as it is.
        let host = host_of(&remote).unwrap_or_default();
        ed.save();
        ed.finish_saving();
        let keep = self.keep.clone();
        self.run(Some("Fetching"), move || {
            Done::Fetched((|| {
                let login = login_of(&host);
                let who = who_of(&host, &login);
                let Some(sync) = Sync::open(&root)? else { anyhow::bail!("The project has no repository yet.") };
                sync.store("Changes on this computer", &who)?;
                sync.fetch(&login)?;
                let merged = sync.merge(&keep, &who)?;
                // Put together here: what is stored there gets it too.
                if matches!(merged, Merged::Updated(_)) && sync.state()?.ahead > 0 {
                    sync.send(&login)?;
                }
                Ok(merged)
            })()
            .map_err(said))
        });
    }

    fn sign_in_with_github(&mut self) {
        self.run(Some("Signing in"), || Done::Code(github::start(github::GITHUB, github::CLIENT_ID).map_err(said)));
    }

    fn save_token(&mut self) {
        let Some(host) = host_of(&self.address) else { return };
        let (user, token) = (self.user.trim().to_string(), self.token.trim().to_string());
        self.token.clear();
        self.run(Some("Signing in"), move || Done::SignedIn(secrets::set_sync_login(&host, &user, &token).map(|_| user).map_err(said)));
    }

    fn sign_out(&mut self) {
        let Some(host) = host_of(&self.address) else { return };
        self.account = None;
        self.run(None, move || {
            let _ = secrets::set_sync_login(&host, "", "");
            Done::SignedIn(Err(String::new()))
        });
    }

    /// Every frame: what the jobs came back with, and asking GitHub whether
    /// the code was typed in.
    pub fn poll(&mut self, ed: &mut Editor, ctx: &egui::Context) {
        // A project with a repository: how it stands shows in the status
        // bar, read again after every save.
        if let Some(root) = ed.project.as_ref().map(|p| p.root.clone()) {
            let key = (root.clone(), ed.saves);
            if self.seen.as_ref() != Some(&key) && self.get.is_none() {
                self.seen = Some(key);
                if self.root.as_deref() != Some(root.as_path()) {
                    let waiting = self.waiting.take();
                    *self = GitUi { waiting, seen: self.seen.take(), ..GitUi::default() };
                    self.root = Some(root.clone());
                }
                if root.join(".git").exists() {
                    self.read();
                }
            }
        } else if self.root.is_some() && self.get.is_none() {
            self.root = None;
            self.state = None;
            self.seen = None;
            self.open = false;
        }
        let mut done = Vec::new();
        self.jobs.retain(|rx| match rx.try_recv() {
            Ok(d) => {
                done.push(d);
                false
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => true,
            Err(_) => false,
        });
        for d in done {
            self.take(d, ed);
        }
        if self.jobs.is_empty() {
            self.busy = None;
        }
        if let Some(w) = &mut self.waiting {
            let now = Instant::now();
            if now >= w.until {
                self.waiting = None;
                self.said = Some((false, t!("The code ran out before it was typed in. Sign in again.").into()));
            } else if !w.asking && now >= w.next {
                w.asking = true;
                let code = w.code.clone();
                self.run(None, move || Done::Asked(github::ask(github::GITHUB, github::CLIENT_ID, &code).map_err(said)));
            }
        }
        if !self.jobs.is_empty() || self.waiting.is_some() {
            ctx.request_repaint_after(Duration::from_millis(300));
        }
    }

    fn take(&mut self, done: Done, ed: &mut Editor) {
        match done {
            Done::Read(state, account, history) => {
                match state {
                    Ok(state) => {
                        if self.address.trim().is_empty() {
                            self.address = state.as_ref().and_then(|s| s.remote.clone()).unwrap_or_default();
                        }
                        self.state = state;
                    }
                    Err(e) => self.said = Some((false, e)),
                }
                self.account = account;
                self.history = history;
                // Once per project: is something newer stored there?
                if !self.checked && self.account.is_some() && self.state.as_ref().is_some_and(|s| s.remote.is_some()) {
                    self.checked = true;
                    self.check();
                }
            }
            Done::Checked => self.read(),
            Done::WentBack(result) => {
                match result {
                    Ok(count) => {
                        if let Some(root) = self.root.clone() {
                            ed.open_project(&root);
                        }
                        let text = tf!("Went back. Files changed: {count}", count = count);
                        ed.log(Level::Success, text.clone());
                        self.said = Some((true, text));
                    }
                    Err(e) => self.said = Some((false, e)),
                }
                self.read();
            }
            Done::Connected(result) => {
                self.said = Some(match result {
                    Ok(()) => (true, t!("Connected. Store sends the project there; Fetch gets what is stored there.").into()),
                    Err(e) => (false, e),
                });
                self.read();
            }
            Done::Stored(result) => {
                self.said = Some(match result {
                    Ok("nothing") => (true, t!("Nothing changed since the last Store.").into()),
                    Ok(_) => {
                        self.message.clear();
                        ed.log(Level::Success, t!("Stored the project and sent it."));
                        (true, t!("Stored the project and sent it.").into())
                    }
                    Err(e) if e == circuit_sync::NEWER_THERE => (false, t!("Newer changes are stored there. Press Fetch first, then Store again.").into()),
                    Err(e) => (false, e),
                });
                self.read();
            }
            Done::Fetched(result) => {
                match result {
                    Ok(Merged::UpToDate) => {
                        self.conflicts.clear();
                        self.said = Some((true, t!("Nothing new is stored there.").into()));
                    }
                    Ok(Merged::Updated(files)) => {
                        self.conflicts.clear();
                        self.keep.clear();
                        // The files changed under the editor: read them again.
                        if let Some(root) = self.root.clone() {
                            ed.open_project(&root);
                        }
                        let text = tf!("Fetched. Files changed: {count}", count = files.len());
                        ed.log(Level::Success, text.clone());
                        self.said = Some((true, text));
                    }
                    Ok(Merged::Conflicts(files)) => {
                        for file in &files {
                            self.keep.entry(file.clone()).or_insert(Side::Mine);
                        }
                        self.conflicts = files;
                        self.said = None;
                    }
                    Err(e) => self.said = Some((false, e)),
                }
                self.read();
            }
            Done::Code(result) => match result {
                Ok(code) => {
                    if let Err(e) = open::that(&code.page) {
                        ed.log(Level::Warn, tf!("Couldn't open the browser ({error}). Open this address: {url}", error = e, url = code.page));
                    }
                    let now = Instant::now();
                    self.waiting = Some(Waiting { next: now + code.wait, until: now + code.good_for, code, asking: false });
                }
                Err(e) => self.said = Some((false, e)),
            },
            Done::Asked(result) => {
                let Some(w) = &mut self.waiting else { return };
                w.asking = false;
                match result {
                    Ok(Answer::NotYet) => w.next = Instant::now() + w.code.wait,
                    Ok(Answer::Slower(wait)) => {
                        w.code.wait = wait;
                        w.next = Instant::now() + wait;
                    }
                    Ok(Answer::Token(token)) => {
                        self.waiting = None;
                        self.run(Some("Signing in"), move || {
                            Done::SignedIn((|| {
                                let name = github::account("https://api.github.com", &token)?;
                                secrets::set_sync_login("github.com", &name, &token)?;
                                Ok(name)
                            })()
                            .map_err(said))
                        });
                    }
                    Ok(Answer::Refused) => {
                        self.waiting = None;
                        self.said = Some((false, t!("The sign-in was refused on GitHub.").into()));
                    }
                    Ok(Answer::TooLate) => {
                        self.waiting = None;
                        self.said = Some((false, t!("The code ran out before it was typed in. Sign in again.").into()));
                    }
                    Err(e) => {
                        self.waiting = None;
                        self.said = Some((false, e));
                    }
                }
            }
            Done::Got(result) => match result {
                Ok(root) => {
                    self.said = None;
                    self.got = Some(root);
                }
                Err(e) => self.said = Some((false, e)),
            },
            Done::SignedIn(result) => {
                match result {
                    Ok(_) => self.said = Some((true, t!("Signed in.").into())),
                    Err(e) if e.is_empty() => {}
                    Err(e) => self.said = Some((false, e)),
                }
                self.read();
            }
        }
    }

    /// The Sign in part: who is signed in at `host`, or the ways to sign in.
    fn sign_in_block(&mut self, ui: &mut egui::Ui, theme: &crate::ui::theme::Theme, host: &str, busy: Option<&'static str>) -> Asked {
        let mut asked = Asked::default();
        ui.add_space(8.0);
        kit::section(ui, theme, t!("Sign in"));
        if let Some(name) = &self.account {
            ui.horizontal(|ui| {
                kit::done_label(ui, tf!("Signed in at {place} as {name}", place = host, name = name), theme.action);
                asked.out = kit::secondary_button(ui, theme, t!("Sign out")).clicked();
            });
        } else {
            if host == "github.com" {
                if let Some(w) = &self.waiting {
                    ui.label(t!("Type this code in on the GitHub page that opened:"));
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&w.code.user_code).monospace().size(24.0).strong());
                        if kit::secondary_button(ui, theme, t!("Copy")).clicked() {
                            ui.ctx().copy_text(w.code.user_code.clone());
                        }
                        if kit::secondary_button(ui, theme, t!("Open the page again")).clicked() {
                            let _ = open::that(&w.code.page);
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(RichText::new(t!("Waiting for you to approve in the browser…")).color(theme.text_muted));
                        asked.cancel = kit::secondary_button(ui, theme, t!("Cancel")).clicked();
                    });
                } else {
                    ui.add_enabled_ui(busy.is_none(), |ui| {
                        asked.github_in = kit::primary_button(ui, theme, t!("Sign in with GitHub")).clicked();
                    });
                }
                ui.add_space(6.0);
                kit::caption(ui, theme, t!("Or paste an access token:"));
            }
            kit::field(ui, theme, t!("User name"), Some(t!("Your account's name there. It stands on what you store.")), |ui| kit::text_input(ui, &mut self.user, "", false));
            kit::field(
                ui,
                theme,
                t!("Access token"),
                Some(t!("Make one in your account's settings there, allowed to read and write repositories (GitHub: Developer settings › Personal access tokens; GitLab: Access tokens, write_repository; Gitea: Applications). Kept in your system's credential store, never in the project.")),
                |ui| kit::text_input(ui, &mut self.token, "", true),
            );
            ui.add_enabled_ui(busy.is_none() && !self.token.trim().is_empty(), |ui| {
                asked.token_in = kit::secondary_button(ui, theme, t!("Save the sign-in")).clicked();
            });
        }
        asked
    }

    /// Start page and File › Get from Git…: asks for a repository and where
    /// to put the project.
    pub fn open_get(&mut self, location: PathBuf) {
        let waiting = self.waiting.take();
        *self = GitUi { waiting, ..GitUi::default() };
        self.get = Some(GetDraft { location: location.to_string_lossy().to_string(), name: String::new(), own_folder: false });
    }

    /// The Get from Git dialog. Gives the project's folder once it is there.
    pub fn get_dialog(&mut self, ctx: &egui::Context, ed: &mut Editor) -> Option<PathBuf> {
        if let Some(root) = self.got.take() {
            self.get = None;
            return Some(root);
        }
        self.get.as_ref()?;
        let theme = ed.theme.clone();
        let icon = ed.reg.icons.get("git").cloned();
        let busy = self.busy;
        let host = host_of(&self.address);
        let signed_in = self.account.is_some();
        let (mut get, mut close, mut retyped) = (false, false, false);
        let mut asked = Asked::default();
        let closed = dialog::show(
            ctx,
            &theme,
            dialog::Spec { id: "git-get", title: t!("Get a project from Git"), subtitle: Some(t!("Brings a project stored in a Git repository onto this computer")), icon: icon.as_deref(), width: 640.0 },
            |ui, part| match part {
                Part::Body => {
                    if kit::field(ui, &theme, t!("Repository address"), Some(t!("The HTTPS address of the repository, as its page shows it under Code or Clone.")), |ui| kit::text_input(ui, &mut self.address, "https://github.com/you/my-bot.git", false)).changed() {
                        retyped = true;
                    }
                    if host.is_none() && !self.address.trim().is_empty() {
                        ui.label(RichText::new(t!("Use the address that starts with https://, not the SSH one (git@…).")).color(theme.warning));
                    }
                    let from_address = name_of(&self.address);
                    let draft = self.get.as_mut().unwrap();
                    kit::field(ui, &theme, t!("Folder name"), Some(t!("The project's folder on this computer. Empty: the repository's name.")), |ui| kit::text_input(ui, &mut draft.name, &from_address, false));
                    ui.horizontal_wrapped(|ui| {
                        for (own, label, tip) in [
                            (false, t!("Kept by XC Circuit"), t!("No folder to look after: the project is in the app's own folder, and stays in your recent projects.")),
                            (true, t!("In a folder I choose"), t!("For projects with files you reach by their path, such as pictures or plugins of their own.")),
                        ] {
                            if kit::radio(ui, &theme, draft.own_folder == own, label).on_hover_text(tip).clicked() {
                                draft.own_folder = own;
                            }
                            ui.add_space(8.0);
                        }
                    });
                    if draft.own_folder {
                        kit::field(ui, &theme, t!("Where to put it"), None, |ui| {
                            let w = ui.available_width();
                            kit::path(ui, &theme, &mut draft.location, "", true, w)
                        });
                    }
                    if let Some(host) = &host {
                        ui.add_space(8.0);
                        kit::section(ui, &theme, t!("Sign in"));
                        asked = self.sign_in_block(ui, &theme, host, busy);
                        if !signed_in {
                            kit::caption(ui, &theme, t!("A repository anyone may read needs no sign-in to get it."));
                        }
                    }
                    ui.add_space(8.0);
                    if let Some(what) = busy {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(RichText::new(tf!("{what} …", what = busy_text(what))).size(12.0).color(theme.text_muted));
                        });
                    } else if let Some((ok, text)) = &self.said {
                        ui.label(RichText::new(text).color(if *ok { theme.action } else { theme.warning }));
                    }
                }
                Part::Footer => {
                    ui.add_enabled_ui(busy.is_none() && host.is_some(), |ui| {
                        get = kit::primary_button(ui, &theme, t!("Get the project")).clicked();
                    });
                    close = kit::secondary_button(ui, &theme, t!("Cancel")).clicked();
                }
            },
        );
        if retyped {
            self.account = None;
            self.read();
        }
        if asked.github_in {
            self.sign_in_with_github();
        }
        if asked.token_in {
            self.save_token();
        }
        if asked.out {
            self.sign_out();
        }
        if asked.cancel {
            self.waiting = None;
        }
        if get {
            self.get_project();
        }
        if (close || closed) && busy.is_none() {
            self.get = None;
            self.waiting = None;
        }
        None
    }

    fn get_project(&mut self) {
        let (Some(draft), Some(host)) = (&self.get, host_of(&self.address)) else { return };
        let address = self.address.trim().to_string();
        let name = if draft.name.trim().is_empty() { name_of(&address) } else { draft.name.trim().to_string() };
        let place = if draft.own_folder { PathBuf::from(draft.location.trim()) } else { crate::bot::remotes_dir() };
        let to = place.join(&name);
        self.run(Some("Fetching"), move || {
            Done::Got((|| {
                std::fs::create_dir_all(&place)?;
                if name.is_empty() || name.contains(['/', '\\', ':']) {
                    anyhow::bail!("Give the folder a name.");
                }
                if to.exists() {
                    anyhow::bail!("There is already a folder called {name} there: choose another name.");
                }
                Sync::get(&address, &to, &login_of(&host))?;
                if !circuit_core::project::is_project(&to) {
                    // Nothing of the user's: the folder was made just now.
                    let _ = std::fs::remove_dir_all(&to);
                    anyhow::bail!("That repository holds no XC Circuit project (no circuit.project.json).");
                }
                Ok(to)
            })()
            .map_err(said))
        });
    }

    pub fn tab(&mut self, ui: &mut egui::Ui, ed: &mut Editor) {
        if !self.open {
            return;
        }
        let theme = ed.theme.clone();
        let icons = ed.reg.icons.clone();
        let icon = icons.get("git").cloned();
        let busy = self.busy;
        let host = host_of(&self.address);
        let remote = self.state.as_ref().and_then(|s| s.remote.clone());
        let connected = remote.as_deref().is_some_and(|r| r == self.address.trim());
        let signed_in = self.account.is_some();
        let (mut connect, mut store, mut fetch, mut close, mut github_in, mut token_in, mut out, mut cancel, mut retyped) = (false, false, false, false, false, false, false, false, false);
        let (mut anyway, mut not_now, mut back) = (false, false, None::<String>);
        let (mut disconnect, mut move_out) = (false, false);
        dialog::tab(
            ui,
            &theme,
            dialog::Spec { id: "git", title: "Git", subtitle: Some(t!("Keep the project in a Git repository: on several computers, or together with others")), icon: icon.as_deref(), width: 640.0 },
            |ui, part| match part {
                Part::Body => {
                    if remote.is_none() {
                        kit::note(
                            ui,
                            &icons,
                            &theme,
                            "info",
                            t!("How to set it up"),
                            t!("1. Make a new, empty repository on GitHub, GitLab or Gitea (private, if the bot is only yours).\n2. Copy its HTTPS address and paste it below.\n3. Sign in, then press Connect.\nYour bot token and passwords are never stored there: they stay in this computer's credential store."),
                        );
                        ui.add_space(10.0);
                    }
                    kit::section(ui, &theme, t!("Where it is stored"));
                    if kit::field(ui, &theme, t!("Repository address"), Some(t!("The HTTPS address of the repository, as its page shows it under Code or Clone.")), |ui| kit::text_input(ui, &mut self.address, "https://github.com/you/my-bot.git", false)).changed() {
                        retyped = true;
                    }
                    if host.is_none() && !self.address.trim().is_empty() {
                        ui.label(RichText::new(t!("Use the address that starts with https://, not the SSH one (git@…).")).color(theme.warning));
                    }
                    if let Some(host) = &host {
                        let asked = self.sign_in_block(ui, &theme, host, busy);
                        (out, cancel, github_in, token_in) = (asked.out, asked.cancel, asked.github_in, asked.token_in);
                    }
                    if let (true, Some(state)) = (connected, &self.state) {
                        ui.add_space(8.0);
                        kit::section(ui, &theme, t!("What changed"));
                        if state.changed.is_empty() && state.ahead == 0 && state.behind == 0 {
                            ui.label(RichText::new(t!("Nothing changed here since the last Store.")).color(theme.text_muted));
                        }
                        if !state.changed.is_empty() {
                            ui.label(tf!("Files changed here: {count}", count = state.changed.len()));
                            for file in state.changed.iter().take(8) {
                                ui.label(RichText::new(format!("  {file}")).monospace().size(11.5).color(theme.text_muted));
                            }
                            if state.changed.len() > 8 {
                                ui.label(RichText::new(tf!("  and {count} more", count = state.changed.len() - 8)).size(11.5).color(theme.text_muted));
                            }
                        }
                        if state.ahead > 0 {
                            ui.label(tf!("Stored here, not sent yet: {count}", count = state.ahead));
                        }
                        if state.behind > 0 {
                            ui.label(tf!("Newer versions stored there: {count}. Press Fetch.", count = state.behind));
                        }
                        ui.add_space(6.0);
                        kit::field(ui, &theme, t!("What did you change?"), Some(t!("A few words that go with this version, such as: a welcome message.")), |ui| kit::text_input(ui, &mut self.message, "", false));
                    }
                    if !self.secrets.is_empty() {
                        ui.add_space(8.0);
                        kit::note(
                            ui,
                            &icons,
                            &theme,
                            "warning",
                            t!("This looks like a secret"),
                            t!("Nothing was stored yet. Whoever runs the place the project is stored at can read it, and so can everyone the repository is shared with. Better: keep it in Bot › Settings and use it from there."),
                        );
                        for line in &self.secrets {
                            ui.label(RichText::new(format!("  {line}")).size(11.5).color(theme.text_muted));
                        }
                        ui.horizontal(|ui| {
                            anyway = kit::secondary_button(ui, &theme, t!("Store anyway")).clicked();
                            not_now = kit::secondary_button(ui, &theme, t!("Don't store")).clicked();
                        });
                    }
                    if connected && !self.history.is_empty() {
                        ui.add_space(8.0);
                        let label = if self.show_history { t!("Hide earlier versions") } else { t!("Earlier versions") };
                        if kit::link(ui, &theme, label, egui::FontId::proportional(13.0)).clicked() {
                            self.show_history = !self.show_history;
                        }
                        if self.show_history {
                            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or_default();
                            for (i, version) in self.history.iter().enumerate() {
                                ui.horizontal(|ui| {
                                    let words: String = version.message.chars().take(60).collect();
                                    ui.label(RichText::new(words).size(12.5));
                                    ui.label(RichText::new(format!("{} · {}", version.who, crate::bot::ago(Duration::from_secs((now - version.when).max(0) as u64)))).size(11.5).color(theme.text_muted));
                                    if i > 0 && busy.is_none() && kit::link(ui, &theme, t!("Go back to this"), egui::FontId::proportional(12.0)).on_hover_text(t!("Makes the project what it was then, as a new version: nothing in between is lost")).clicked() {
                                        back = Some(version.id.clone());
                                    }
                                });
                            }
                        }
                    }
                    if !self.conflicts.is_empty() {
                        ui.add_space(8.0);
                        kit::note(ui, &icons, &theme, "warning", t!("Both sides changed the same files"), t!("Nothing was changed yet. Choose for each file whose version stays, then press Put together. The other version stays in the repository's history."));
                        ui.add_space(6.0);
                        for file in &self.conflicts {
                            ui.horizontal(|ui| {
                                let side = self.keep.entry(file.clone()).or_insert(Side::Mine);
                                if kit::radio(ui, &theme, *side == Side::Mine, t!("Mine")).clicked() {
                                    *side = Side::Mine;
                                }
                                if kit::radio(ui, &theme, *side == Side::Theirs, t!("The stored one")).clicked() {
                                    *side = Side::Theirs;
                                }
                                ui.label(RichText::new(file).monospace().size(11.5));
                            });
                        }
                    }
                    if let (true, Some(root)) = (connected, &self.root) {
                        ui.add_space(8.0);
                        let kept = root.starts_with(crate::bot::remotes_dir());
                        ui.horizontal(|ui| {
                            if kit::link(ui, &theme, t!("Show the folder"), egui::FontId::proportional(12.5)).clicked() {
                                crate::editor::show_in_folder(root);
                            }
                            ui.add_space(10.0);
                            if !self.disconnecting && kit::link(ui, &theme, t!("Disconnect…"), egui::FontId::proportional(12.5)).clicked() {
                                self.disconnecting = true;
                            }
                        });
                        if self.disconnecting {
                            let waiting = self.state.as_ref().is_some_and(|s| !s.changed.is_empty() || s.ahead > 0);
                            let mut text = t!("The project stays on this computer with its history, and what is stored in the repository stays there. Store and Fetch stop until you connect again.").to_string();
                            if waiting {
                                text.push(' ');
                                text.push_str(t!("Some changes here are not stored yet: they stay here, but the repository won't have them."));
                            }
                            kit::note(ui, &icons, &theme, "warning", t!("Disconnect from the repository?"), &text);
                            ui.horizontal(|ui| {
                                disconnect = kit::secondary_button(ui, &theme, t!("Disconnect")).clicked();
                                if kept {
                                    move_out = kit::secondary_button(ui, &theme, t!("Disconnect and keep as a folder…")).on_hover_text(t!("Moves the project out of XC Circuit's own folder into one you choose")).clicked();
                                }
                                if kit::secondary_button(ui, &theme, t!("Cancel")).clicked() {
                                    self.disconnecting = false;
                                }
                            });
                        }
                    }
                    ui.add_space(8.0);
                    if let Some(what) = busy {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(RichText::new(tf!("{what} …", what = busy_text(what))).size(12.0).color(theme.text_muted));
                        });
                    } else if let Some((ok, text)) = &self.said {
                        ui.label(RichText::new(text).color(if *ok { theme.action } else { theme.warning }));
                    }
                }
                Part::Footer => {
                    if connected {
                        ui.add_enabled_ui(busy.is_none() && signed_in, |ui| {
                            if self.conflicts.is_empty() {
                                store = kit::primary_button(ui, &theme, t!("Store")).on_hover_text(t!("Saves the project, records what changed and sends it to the repository")).clicked();
                                fetch = kit::secondary_button(ui, &theme, t!("Fetch")).on_hover_text(t!("Gets what is stored in the repository and puts it together with what you have here")).clicked();
                            } else {
                                fetch = kit::primary_button(ui, &theme, t!("Put together")).clicked();
                            }
                        });
                    } else {
                        ui.add_enabled_ui(busy.is_none() && host.is_some() && signed_in, |ui| {
                            connect = kit::primary_button(ui, &theme, t!("Connect")).clicked();
                        });
                    }
                    close = kit::secondary_button(ui, &theme, t!("Close")).clicked();
                }
            },
        );
        if retyped {
            // Another place: who is signed in there?
            self.account = None;
            self.read();
        }
        if github_in {
            self.sign_in_with_github();
        }
        if token_in {
            self.save_token();
        }
        if out {
            self.sign_out();
        }
        if cancel {
            self.waiting = None;
        }
        if connect {
            self.connect();
        }
        if store {
            self.store(ed);
        }
        if disconnect || move_out {
            self.disconnect(ed, move_out);
        }
        if anyway {
            self.secrets_ok = true;
            self.store(ed);
        }
        if not_now {
            self.secrets.clear();
        }
        if let Some(id) = back {
            self.go_back(ed, id);
        }
        if fetch {
            self.fetch(ed);
        }
        if close {
            self.open = false;
        }
    }
}

fn busy_text(what: &str) -> &'static str {
    match what {
        "Connecting" => t!("Connecting"),
        "Storing" => t!("Storing"),
        "Fetching" => t!("Fetching"),
        "Going back" => t!("Going back"),
        _ => t!("Signing in"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_place_of_an_address() {
        assert_eq!(host_of("https://github.com/you/bot.git").as_deref(), Some("github.com"));
        assert_eq!(host_of(" https://Git.Example.com:3000/team/bot ").as_deref(), Some("git.example.com:3000"));
        assert_eq!(host_of("https://someone@gitlab.com/you/bot.git").as_deref(), Some("gitlab.com"));
        assert_eq!(host_of("git@github.com:you/bot.git"), None, "SSH isn't spoken");
        assert_eq!(host_of("https://github.com"), None, "no repository in it");
        assert_eq!(host_of(""), None);
        assert_eq!(name_of("https://github.com/you/my-bot.git"), "my-bot");
        assert_eq!(name_of("https://git.example.com/team/bot/"), "bot");
        assert_eq!(name_of("git@github.com:you/bot.git"), "");
    }

    /// Connect, Store and Fetch through the dialog's own steps, against a
    /// repository in a folder.
    #[test]
    fn stores_and_fetches_a_project() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let dir = std::env::temp_dir().join(format!("xc-git-ui-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let project = dir.join("bot");
        circuit_core::Project::create(&project, "Bot", "none").unwrap();
        let mut ed = Editor::new(vec![root.join("plugins")], None).unwrap();
        ed.open_project(&project);
        let mut ui = GitUi::new();
        ui.open(&ed);
        let ctx = egui::Context::default();
        let wait = |ui: &mut GitUi, ed: &mut Editor| {
            for _ in 0..600 {
                ui.poll(ed, &ctx);
                if ui.jobs.is_empty() {
                    return;
                }
                thread::sleep(Duration::from_millis(10));
            }
            panic!("still busy");
        };
        wait(&mut ui, &mut ed);
        assert!(ui.state.is_none() && !project.join(".git").exists(), "no repository until Connect");

        // A repository in a folder stands in for GitHub.
        let stored = dir.join("stored.git").to_string_lossy().replace('\\', "/");
        Sync::start_bare(&dir.join("stored.git")).unwrap();
        Sync::start(&project).unwrap().set_remote(&stored).unwrap();
        ui.open(&ed);
        wait(&mut ui, &mut ed);
        let state = ui.state.clone().unwrap();
        assert!(state.remote.as_deref() == Some(stored.as_str()) && state.changed.iter().any(|f| f == "workspaces/main.json"), "{state:?}");
        assert_eq!(ui.address, stored, "the address it is stored at shows");

        // A webhook address in a block: asked about before anything is stored.
        let node = circuit_core::graph::Node { block: "core.log@1".into(), values: serde_json::json!({ "message": "to https://discord.com/api/webhooks/123456789012345678/abcDEF_ghi-jkl" }).as_object().cloned().unwrap(), ..Default::default() };
        ed.project.as_mut().unwrap().workspaces["main"].nodes.insert("1".into(), node);
        ui.store(&mut ed);
        assert!(ui.jobs.is_empty() && ui.secrets.len() == 1 && ui.secrets[0].contains("#1 › message"), "{:?}", ui.secrets);
        ed.project.as_mut().unwrap().workspaces["main"].nodes.clear();
        assert!(looks_secret(&["MTIzNDU2Nzg5MDEyMzQ1Njc4OTAx", "GaBcDe", "abcdefghijklmnopqrstuvwxyz0123456789ab"].join(".")).is_some() && looks_secret("ghp_abcdefghijklmnopqrstuvwxyz0123456789").is_some());
        assert!(looks_secret("Hello ${user}, see https://discord.com/channels/1/2").is_none() && looks_secret("1.2.3").is_none());

        ui.message = "The first version".into();
        ui.store(&mut ed);
        wait(&mut ui, &mut ed);
        assert_eq!(ui.said.clone().map(|(ok, _)| ok), Some(true), "{:?}", ui.said);
        assert!(ui.state.as_ref().unwrap().changed.is_empty() && ui.message.is_empty());
        ui.store(&mut ed);
        wait(&mut ui, &mut ed);
        assert_eq!(ui.said.clone().unwrap().1, t!("Nothing changed since the last Store."));

        // Someone else renames the workspace and stores that.
        let other_dir = dir.join("other");
        let nobody = Login::default();
        let them = Who { name: "them".into(), email: "them@example.com".into() };
        let other = Sync::get(&stored, &other_dir, &nobody).unwrap();
        let file = other_dir.join("workspaces/main.json");
        let rename = |to: &str| {
            let text = std::fs::read_to_string(&file).unwrap();
            let v: serde_json::Value = serde_json::from_str(&text).unwrap();
            std::fs::write(&file, text.replace(&format!("\"name\": \"{}\"", v["name"].as_str().unwrap()), &format!("\"name\": \"{to}\""))).unwrap();
        };
        rename("Theirs");
        other.store("Renamed", &them).unwrap();
        other.send(&nobody).unwrap();
        ui.fetch(&mut ed);
        wait(&mut ui, &mut ed);
        assert_eq!(ed.project.as_ref().unwrap().workspaces["main"].name, "Theirs", "the project was read again: {:?}", ui.said);

        // Both rename it: nothing changes until one is picked.
        rename("Theirs again");
        other.store("Renamed again", &them).unwrap();
        other.send(&nobody).unwrap();
        ed.project.as_mut().unwrap().workspaces["main"].name = "Mine".into();
        ui.fetch(&mut ed);
        wait(&mut ui, &mut ed);
        assert_eq!(ui.conflicts, ["workspaces/main.json"], "{:?}", ui.said);
        assert_eq!(ed.project.as_ref().unwrap().workspaces["main"].name, "Mine");
        ui.keep.insert("workspaces/main.json".into(), Side::Theirs);
        ui.fetch(&mut ed);
        wait(&mut ui, &mut ed);
        assert!(ui.conflicts.is_empty(), "{:?}", ui.said);
        assert_eq!(ed.project.as_ref().unwrap().workspaces["main"].name, "Theirs again");
        assert_eq!(ui.state.as_ref().map(|s| (s.ahead, s.behind, s.changed.len())), Some((0, 0, 0)), "put together and sent");

        // The history, and going back to the first version.
        assert!(ui.history.len() >= 4 && ui.history.last().unwrap().message == "The first version", "{:?}", ui.history);
        let first = ui.history.last().unwrap().id.clone();
        ui.go_back(&mut ed, first);
        wait(&mut ui, &mut ed);
        assert_eq!(ed.project.as_ref().unwrap().workspaces["main"].name, "Main", "{:?}", ui.said);
        assert_eq!(ui.history[0].message, "Back to: The first version");
        assert_eq!(ui.state.as_ref().map(|s| s.ahead), Some(0), "and sent");

        // Disconnecting: the address is forgotten, the project and its history stay.
        ui.disconnect(&mut ed, false);
        wait(&mut ui, &mut ed);
        assert!(ui.state.as_ref().is_some_and(|s| s.remote.is_none()) && ui.address.is_empty() && !ui.history.is_empty(), "{:?}", ui.said);
        assert!(ui.status().is_none(), "nothing in the status bar for a project stored nowhere");

        // Getting it onto another computer: into a new folder, by its name.
        let mut get = GitUi::new();
        get.open_get(dir.clone());
        get.address = stored.clone();
        assert!(host_of(&stored).is_none(), "a folder is no place to sign in at: the dialog asks for an HTTPS address");
        let to = dir.join("copy");
        let login = Login::default();
        Sync::get(&stored, &to, &login).unwrap();
        assert!(circuit_core::project::is_project(&to) && !to.join(".circuit").join("local.json").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
