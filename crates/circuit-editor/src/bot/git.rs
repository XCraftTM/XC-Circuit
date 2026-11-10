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
use circuit_sync::{Login, Merged, Side, State, Sync, Who};
use eframe::egui::{self, RichText};

use crate::editor::{Editor, Level};
use crate::ui::dialog::{self, Part};
use crate::ui::kit;

/// What a job on its own thread came back with.
enum Done {
    /// How the project stands, and who is signed in at its place.
    Read(Result<Option<State>, String>, Option<String>),
    Connected(Result<(), String>),
    Stored(Result<&'static str, String>),
    Fetched(Result<Merged, String>),
    Code(Result<github::Code, String>),
    Asked(Result<Answer, String>),
    SignedIn(Result<String, String>),
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
        let Some(root) = self.root.clone() else { return };
        let typed = host_of(&self.address);
        self.run(None, move || {
            let state = Sync::open(&root).and_then(|s| s.map(|s| s.state()).transpose()).map_err(said);
            let host = typed.or_else(|| state.as_ref().ok().and_then(|s| s.as_ref()).and_then(|s| s.remote.as_deref()).and_then(host_of));
            let account = host.and_then(|h| secrets::sync_login(&h)).map(|(user, _)| if user.is_empty() { "…".to_string() } else { user });
            Done::Read(state, account)
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

    /// Store: saves the project, records what changed and sends it.
    fn store(&mut self, ed: &mut Editor) {
        let (Some(root), Some(remote)) = (self.root.clone(), self.state.as_ref().and_then(|s| s.remote.clone())) else { return };
        // A repository set up by hand at an address with no place to sign in
        // at (a folder, say) is used as it is.
        let host = host_of(&remote).unwrap_or_default();
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
        if !self.open && self.jobs.is_empty() && self.waiting.is_none() {
            return;
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
            Done::Read(state, account) => {
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
                        let text = tf!("Fetched: {count} files changed.", count = files.len());
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

    pub fn tab(&mut self, ui: &mut egui::Ui, ed: &mut Editor) {
        if !self.open {
            return;
        }
        let theme = ed.theme.clone();
        let icons = ed.reg.icons.clone();
        let icon = icons.get("branch").cloned();
        let busy = self.busy;
        let host = host_of(&self.address);
        let remote = self.state.as_ref().and_then(|s| s.remote.clone());
        let connected = remote.as_deref().is_some_and(|r| r == self.address.trim());
        let signed_in = self.account.is_some();
        let (mut connect, mut store, mut fetch, mut close, mut github_in, mut token_in, mut out, mut cancel, mut retyped) = (false, false, false, false, false, false, false, false, false);
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
                        ui.add_space(8.0);
                        kit::section(ui, &theme, t!("Sign in"));
                        if let Some(name) = &self.account {
                            ui.horizontal(|ui| {
                                kit::done_label(ui, tf!("Signed in at {place} as {name}", place = host, name = name), theme.action);
                                out = kit::secondary_button(ui, &theme, t!("Sign out")).clicked();
                            });
                        } else {
                            if host == "github.com" {
                                if let Some(w) = &self.waiting {
                                    ui.label(t!("Type this code in on the GitHub page that opened:"));
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new(&w.code.user_code).monospace().size(24.0).strong());
                                        if kit::secondary_button(ui, &theme, t!("Copy")).clicked() {
                                            ui.ctx().copy_text(w.code.user_code.clone());
                                        }
                                        if kit::secondary_button(ui, &theme, t!("Open the page again")).clicked() {
                                            let _ = open::that(&w.code.page);
                                        }
                                    });
                                    ui.horizontal(|ui| {
                                        ui.spinner();
                                        ui.label(RichText::new(t!("Waiting for you to approve in the browser…")).color(theme.text_muted));
                                        cancel = kit::secondary_button(ui, &theme, t!("Cancel")).clicked();
                                    });
                                } else {
                                    ui.add_enabled_ui(busy.is_none(), |ui| {
                                        github_in = kit::primary_button(ui, &theme, t!("Sign in with GitHub")).clicked();
                                    });
                                }
                                ui.add_space(6.0);
                                kit::caption(ui, &theme, t!("Or paste an access token:"));
                            }
                            kit::field(ui, &theme, t!("User name"), Some(t!("Your account's name there. It stands on what you store.")), |ui| kit::text_input(ui, &mut self.user, "", false));
                            kit::field(
                                ui,
                                &theme,
                                t!("Access token"),
                                Some(t!("Make one in your account's settings there, allowed to read and write repositories (GitHub: Developer settings › Personal access tokens; GitLab: Access tokens, write_repository; Gitea: Applications). Kept in your system's credential store, never in the project.")),
                                |ui| kit::text_input(ui, &mut self.token, "", true),
                            );
                            ui.add_enabled_ui(busy.is_none() && !self.token.trim().is_empty(), |ui| {
                                token_in = kit::secondary_button(ui, &theme, t!("Save the sign-in")).clicked();
                            });
                        }
                    }
                    if let (true, Some(state)) = (connected, &self.state) {
                        ui.add_space(8.0);
                        kit::section(ui, &theme, t!("What changed"));
                        if state.changed.is_empty() && state.ahead == 0 && state.behind == 0 {
                            ui.label(RichText::new(t!("Nothing changed here since the last Store.")).color(theme.text_muted));
                        }
                        if !state.changed.is_empty() {
                            ui.label(tf!("{count} files changed here:", count = state.changed.len()));
                            for file in state.changed.iter().take(8) {
                                ui.label(RichText::new(format!("  {file}")).monospace().size(11.5).color(theme.text_muted));
                            }
                            if state.changed.len() > 8 {
                                ui.label(RichText::new(tf!("  and {count} more", count = state.changed.len() - 8)).size(11.5).color(theme.text_muted));
                            }
                        }
                        if state.ahead > 0 {
                            ui.label(tf!("{count} versions stored here are not sent yet.", count = state.ahead));
                        }
                        if state.behind > 0 {
                            ui.label(tf!("{count} newer versions are stored there: press Fetch.", count = state.behind));
                        }
                        ui.add_space(6.0);
                        kit::field(ui, &theme, t!("What did you change?"), Some(t!("A few words that go with this version, such as: a welcome message.")), |ui| kit::text_input(ui, &mut self.message, "", false));
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
        let _ = std::fs::remove_dir_all(&dir);
    }
}
