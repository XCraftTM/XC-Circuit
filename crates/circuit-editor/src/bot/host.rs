//! File › Host…: pairing this computer with an XC Circuit Host (the
//! container people run themselves), keeping a project on it, and carrying
//! a project's secrets between the paired devices through its vault. The
//! host only ever gets them sealed (`circuit_runner::host_link`).

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;
use std::time::{Duration, Instant};

use circuit_runner::host_link::{self, Half, Link, VaultKey};
use circuit_runner::secrets;
use eframe::egui::{self, RichText};

use crate::editor::{Editor, Level};
use crate::ui::dialog::{self, Part};
use crate::ui::kit;

/// The host this computer is paired with, as far as the window needs it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Paired {
    /// `https://circuit.example.com`
    pub address: String,
    pub device: String,
    pub name: String,
    /// Whether this device has the vault key (without it, no secrets).
    pub has_key: bool,
}

enum Done {
    Loaded(Option<Paired>),
    /// Paired, and the recovery code when this device made the vault key.
    Paired(Result<(Paired, Option<String>), String>),
    /// The code to type into the other device.
    Code(String),
    /// The other device was let in and has the key: its name.
    Added(Result<String, String>),
    Sent(Result<usize, String>),
    /// The project's secrets from the vault, by name.
    Got(Result<BTreeMap<String, String>, String>),
    Unpaired,
}

#[derive(Default)]
pub struct HostUi {
    pub open: bool,
    loaded: bool,
    paired: Option<Paired>,
    address: String,
    code: String,
    name: String,
    recovery_typed: String,
    has_recovery: bool,
    /// Shown once, right after this device made the vault key.
    recovery: Option<String>,
    /// The code another device is to type, while waiting for it.
    adding: Option<String>,
    stop_adding: Option<Arc<AtomicBool>>,
    unpairing: bool,
    busy: Option<&'static str>,
    jobs: Vec<Receiver<Done>>,
    said: Option<(bool, String)>,
    /// A project address to connect File › Git… to, for the app.
    pub keep: Option<String>,
    /// The projects the host said were changed by another device: for the
    /// app to fetch the open one.
    pub changed: Vec<String>,
    /// The ear at the host: what it heard, and the switch that stops it.
    heard: Option<(Receiver<String>, Arc<AtomicBool>)>,
    /// About the open project, from File › Git…, for this dialog to show:
    /// where it is stored, how it stands, and what the last action said.
    pub project: (Option<String>, Option<String>, Option<(bool, String)>),
}

/// The place in the credential store and in addresses: `circuit.example.com`.
fn host_of(address: &str) -> String {
    address.trim().split_once("://").map(|(_, r)| r).unwrap_or(address.trim()).split('/').next().unwrap_or_default().to_ascii_lowercase()
}

/// Where the address of the paired host is noted (not a secret).
fn note_file() -> PathBuf {
    super::app_data_dir().join("host.json")
}

fn said(e: anyhow::Error) -> String {
    format!("{e:#}")
}

/// What is kept of a pairing: in the credential store, by the host's name.
fn save_pairing(address: &str, device: &str, name: &str, token: &str, key: Option<&VaultKey>) -> anyhow::Result<()> {
    let host = host_of(address);
    let entry = serde_json::json!({ "address": address, "device": device, "name": name, "token": token, "key": key.map(VaultKey::to_text) });
    super::set_host_login(&host, &entry.to_string())?;
    // File › Git… signs in at this host with the same token.
    super::set_sync_login(&host, name, token)?;
    let _ = std::fs::create_dir_all(super::app_data_dir());
    std::fs::write(note_file(), serde_json::json!({ "address": address }).to_string())?;
    Ok(())
}

/// The pairing kept for the noted host: what the window shows, the token
/// and the vault key.
fn load_pairing() -> Option<(Paired, String, Option<VaultKey>)> {
    let note: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(note_file()).ok()?).ok()?;
    let address = note["address"].as_str()?.to_string();
    let entry: serde_json::Value = serde_json::from_str(&super::host_login(&host_of(&address))?).ok()?;
    let key = entry["key"].as_str().and_then(|k| VaultKey::from_text(k).ok());
    let paired = Paired { address, device: entry["device"].as_str()?.into(), name: entry["name"].as_str().unwrap_or_default().into(), has_key: key.is_some() };
    Some((paired, entry["token"].as_str()?.to_string(), key))
}

/// The paired host's address and this device's token, for a bot that runs
/// there (Bot › Remote…).
pub fn pairing() -> Option<(String, String)> {
    load_pairing().map(|(paired, token, _)| (paired.address, token))
}

/// A connection to the paired host, said hello to.
fn linked() -> anyhow::Result<(Link, Paired, Option<VaultKey>)> {
    let (paired, token, key) = load_pairing().ok_or_else(|| anyhow::anyhow!("This computer isn't paired with a host."))?;
    let mut link = Link::open(&paired.address)?;
    link.hello(&token)?;
    Ok((link, paired, key))
}

/// This computer's name, to tell the devices apart.
fn computer_name() -> String {
    std::env::var("COMPUTERNAME").or_else(|_| std::env::var("HOSTNAME")).ok().filter(|n| !n.trim().is_empty()).unwrap_or_else(|| "My computer".into())
}

/// The secrets of the open project that travel between devices: the bot's
/// and its plugins' and databases', not the ones for uploading from this
/// computer.
fn travelling(ed: &Editor) -> Vec<String> {
    super::secret_names(ed).into_iter().filter(|n| n != circuit_runner::hosting::PASSWORD && n != circuit_runner::hosting::API_KEY).collect()
}

impl HostUi {
    pub fn new() -> Self {
        Self::default()
    }

    /// File › Host…
    pub fn open(&mut self) {
        self.open = true;
        if self.name.is_empty() {
            self.name = computer_name();
        }
        self.run(None, || Done::Loaded(load_pairing().map(|(p, _, _)| p)));
    }

    fn run(&mut self, what: Option<&'static str>, job: impl FnOnce() -> Done + Send + 'static) {
        self.run_many(what, move |tx| {
            let _ = tx.send(job());
        });
    }

    /// A job that may say more than one thing.
    fn run_many(&mut self, what: Option<&'static str>, job: impl FnOnce(Sender<Done>) + Send + 'static) {
        let (tx, rx) = channel();
        if what.is_some() {
            self.busy = what;
        }
        self.jobs.push(rx);
        thread::spawn(move || job(tx));
    }

    fn pair(&mut self) {
        let (address, code, name) = (self.address.trim().trim_end_matches('/').to_string(), self.code.clone(), self.name.trim().to_string());
        let recovery = self.has_recovery.then(|| self.recovery_typed.clone()).filter(|r| !r.trim().is_empty());
        self.run(Some("Pairing"), move || {
            Done::Paired((|| {
                let (for_host, secret) = host_link::code_parts(&code);
                if for_host.len() != 9 {
                    anyhow::bail!("A pairing code has 8 letters and digits (the first device's, from the host's log) or 16 (from another device).");
                }
                let mut link = Link::open(&address)?;
                if !link.keeps() {
                    anyhow::bail!("That host only passes live sessions on: it keeps nothing, so there is nothing to pair with.");
                }
                let recovered = recovery.as_deref().map(VaultKey::from_recovery_code).transpose()?;
                let half = Half::new()?;
                let (device, token) = link.pair(&for_host, &name, &half.public)?;
                // The vault key: from the device that showed the code, from
                // the recovery code, or new (the very first device).
                let (key, made) = if !secret.is_empty() {
                    let keys = link.wait_keys(Duration::from_secs(60))?;
                    match keys.first() {
                        Some((_, wrapped)) => (Some(half.unwrap(wrapped, &secret)?), false),
                        None => (None, false),
                    }
                } else if let Some(key) = recovered {
                    (Some(key), false)
                } else {
                    (Some(VaultKey::new()?), true)
                };
                save_pairing(&address, &device, &name, &token, key.as_ref())?;
                let code = key.as_ref().filter(|_| made).map(VaultKey::recovery_code);
                Ok((Paired { address, device, name, has_key: key.is_some() }, code))
            })()
            .map_err(said))
        });
    }

    /// Lets one more device in: shows a code, waits for the device that
    /// types it, and gives it the vault key.
    fn add_device(&mut self) {
        let stop = Arc::new(AtomicBool::new(false));
        self.stop_adding = Some(stop.clone());
        self.run_many(Some("Waiting for the other device"), move |tx| {
            let added = (|| {
                let (mut link, _, key) = linked()?;
                let key = key.ok_or_else(|| anyhow::anyhow!("This device has no vault key to pass on."))?;
                let known: Vec<String> = link.devices()?.into_iter().map(|d| d.id).collect();
                let (for_host, secret) = (link.invite()?, host_link::code_secret()?);
                let _ = tx.send(Done::Code(format!("{for_host}-{secret}")));
                let until = Instant::now() + Duration::from_secs(600);
                loop {
                    if stop.load(Ordering::Relaxed) {
                        anyhow::bail!("");
                    }
                    if Instant::now() > until {
                        anyhow::bail!("No device typed the code in time. Add a device again for a new code.");
                    }
                    if let Some(new) = link.devices()?.into_iter().find(|d| !known.contains(&d.id)) {
                        link.key_put(&new.id, &host_link::wrap(&key, &new.key, &secret)?)?;
                        return Ok(new.name);
                    }
                    thread::sleep(Duration::from_millis(1500));
                }
            })();
            let _ = tx.send(Done::Added(added.map_err(said)));
        });
    }

    /// Sends the open project's secrets to the vault, sealed. What other
    /// devices put there and this one doesn't have stays.
    fn send_secrets(&mut self, ed: &Editor) {
        let Some(project) = &ed.project else { return };
        let id = project.meta.id.clone();
        let mine: BTreeMap<String, String> = travelling(ed).into_iter().filter_map(|n| secrets::get(&project.root, &n).filter(|v| !v.is_empty()).map(|v| (n, v))).collect();
        self.run(Some("Sending the secrets"), move || {
            Done::Sent((|| {
                let (mut link, _, key) = linked()?;
                let key = key.ok_or_else(|| anyhow::anyhow!("This device has no vault key: pair it again with a code from a device that has one."))?;
                let (sealed, rev) = link.vault_get(&id)?;
                let mut all: BTreeMap<String, String> = match sealed {
                    Some(sealed) => serde_json::from_slice(&key.open(&sealed)?)?,
                    None => BTreeMap::new(),
                };
                let count = mine.len();
                all.extend(mine);
                link.vault_put(&id, &key.seal(serde_json::to_string(&all)?.as_bytes())?, rev)?;
                Ok(count)
            })()
            .map_err(said))
        });
    }

    fn get_secrets(&mut self, ed: &Editor) {
        let Some(project) = &ed.project else { return };
        let id = project.meta.id.clone();
        self.run(Some("Getting the secrets"), move || {
            Done::Got((|| {
                let (mut link, _, key) = linked()?;
                let key = key.ok_or_else(|| anyhow::anyhow!("This device has no vault key: pair it again with a code from a device that has one."))?;
                match link.vault_get(&id)?.0 {
                    Some(sealed) => Ok(serde_json::from_slice(&key.open(&sealed)?)?),
                    None => Ok(BTreeMap::new()),
                }
            })()
            .map_err(said))
        });
    }

    fn unpair(&mut self) {
        self.unpairing = false;
        self.run(Some("Unpairing"), || {
            if let Ok((mut link, paired, _)) = linked() {
                // Off the host's list too, when it can be reached.
                let _ = link.remove_device(&paired.device);
            }
            if let Some((paired, _, _)) = load_pairing() {
                let host = host_of(&paired.address);
                let _ = super::set_host_login(&host, "");
                let _ = super::set_sync_login(&host, "", "");
            }
            let _ = std::fs::remove_file(note_file());
            Done::Unpaired
        });
    }

    /// Listens at the paired host for projects that other devices changed,
    /// for as long as the app runs (started once; it finds its way back
    /// after a lost connection, and gives up when the pairing is gone).
    fn listen(&mut self) {
        if self.heard.is_some() {
            return;
        }
        let (tx, rx) = channel();
        let stop = Arc::new(AtomicBool::new(false));
        self.heard = Some((rx, stop.clone()));
        thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                if let Ok((mut link, paired, _)) = linked() {
                    while !stop.load(Ordering::Relaxed) {
                        match link.next_change(Duration::from_secs(25)) {
                            // Not what this device stored itself.
                            Ok(Some((project, by))) if by != paired.device => {
                                if tx.send(project).is_err() {
                                    return;
                                }
                            }
                            Ok(_) => {}
                            Err(_) => break,
                        }
                    }
                }
                // Not paired, or the connection is gone: looked at again later.
                for _ in 0..20 {
                    if stop.load(Ordering::Relaxed) {
                        return;
                    }
                    thread::sleep(Duration::from_millis(500));
                }
            }
        });
    }

    pub fn poll(&mut self, ed: &mut Editor, ctx: &egui::Context) {
        self.listen();
        if let Some((heard, _)) = &self.heard {
            self.changed.extend(heard.try_iter());
        }
        if self.jobs.is_empty() {
            return;
        }
        let mut done = Vec::new();
        self.jobs.retain(|rx| loop {
            match rx.try_recv() {
                Ok(d) => done.push(d),
                Err(std::sync::mpsc::TryRecvError::Empty) => return true,
                Err(_) => return false,
            }
        });
        for d in done {
            self.take(d, ed);
        }
        if self.jobs.is_empty() {
            self.busy = None;
        }
        ctx.request_repaint_after(Duration::from_millis(300));
    }

    fn take(&mut self, done: Done, ed: &mut Editor) {
        match done {
            Done::Loaded(paired) => {
                if let Some(p) = &paired
                    && self.address.is_empty()
                {
                    self.address = p.address.clone();
                }
                self.paired = paired;
                self.loaded = true;
            }
            Done::Paired(Ok((paired, recovery))) => {
                self.said = Some(if paired.has_key { (true, t!("Paired.").into()) } else { (false, t!("Paired, but the key for the secrets didn't arrive. Unpair, then pair again with a new code, and leave the other device's window open until this one says Paired.").into()) });
                ed.log(Level::Success, tf!("Paired with the host at {address}", address = paired.address));
                self.paired = Some(paired);
                self.recovery = recovery;
                self.code.clear();
                self.recovery_typed.clear();
            }
            Done::Paired(Err(e)) => self.said = Some((false, e)),
            Done::Code(code) => self.adding = Some(code),
            Done::Added(result) => {
                self.adding = None;
                self.stop_adding = None;
                match result {
                    Ok(name) => self.said = Some((true, tf!("{name} is paired and has the key for the secrets.", name = name))),
                    Err(e) if e.is_empty() => {}
                    Err(e) => self.said = Some((false, e)),
                }
            }
            Done::Sent(result) => {
                self.said = Some(match result {
                    Ok(count) => (true, tf!("Sent, sealed. Secrets of this project in the vault from here: {count}", count = count)),
                    Err(e) => (false, e),
                })
            }
            Done::Got(Ok(all)) => {
                let Some(root) = ed.project.as_ref().map(|p| p.root.clone()) else { return };
                let wanted = travelling(ed);
                let mut count = 0;
                for (name, value) in all.iter().filter(|(n, v)| wanted.contains(n) && !v.is_empty()) {
                    if secrets::set(&root, name, value).is_ok() {
                        count += 1;
                    }
                }
                self.said = Some((true, if count == 0 { t!("The vault has no secrets for this project yet.").to_string() } else { tf!("Got the secrets and saved them on this computer: {count}", count = count) }));
            }
            Done::Got(Err(e)) => self.said = Some((false, e)),
            Done::Unpaired => {
                self.paired = None;
                self.recovery = None;
                self.said = Some((true, t!("Unpaired. Nothing of the host is kept on this computer any more.").into()));
            }
        }
    }

    pub fn tab(&mut self, ui: &mut egui::Ui, ed: &mut Editor) {
        if !self.open {
            return;
        }
        let theme = ed.theme.clone();
        let icons = ed.reg.icons.clone();
        let icon = icons.get("server").cloned();
        let busy = self.busy;
        let has_project = ed.project.is_some();
        let (mut pair, mut add, mut cancel_add, mut keep, mut send, mut get, mut unpair, mut close) = (false, false, false, false, false, false, false, false);
        dialog::tab(
            ui,
            &theme,
            dialog::Spec { id: "host", title: "Host", subtitle: Some(t!("A small server of your own: for your projects, your secrets on all your devices, and live sessions")), icon: icon.as_deref(), width: 640.0 },
            |ui, part| match part {
                Part::Body => {
                    if !self.loaded {
                        ui.spinner();
                        return;
                    }
                    match self.paired.clone() {
                        None => {
                            kit::note(
                                ui,
                                &icons,
                                &theme,
                                "info",
                                t!("Pair this computer with your host"),
                                t!("The XC Circuit Host is a container you run on a server of your own; the docs say how. The first device pairs with the code in the host's log. Every other one pairs with a code that a paired device shows under Add a device."),
                            );
                            ui.add_space(10.0);
                            kit::field(ui, &theme, t!("Host address"), Some(t!("Where your host is reached, such as https://circuit.example.com")), |ui| kit::text_input(ui, &mut self.address, "https://circuit.example.com", false));
                            kit::field(ui, &theme, t!("Pairing code"), Some(t!("8 letters and digits from the host's log, or 16 from another device.")), |ui| kit::text_input(ui, &mut self.code, "ABCD-EFGH", false));
                            kit::field(ui, &theme, t!("This device's name"), Some(t!("To tell your devices apart.")), |ui| kit::text_input(ui, &mut self.name, "", false));
                            kit::checkbox(ui, &theme, &mut self.has_recovery, t!("I have a recovery code (all my other devices are gone)"));
                            if self.has_recovery {
                                kit::field(ui, &theme, t!("Recovery code"), Some(t!("The 52 letters and digits you wrote down when the first device was paired.")), |ui| kit::text_input(ui, &mut self.recovery_typed, "", false));
                            }
                        }
                        Some(paired) => {
                            ui.horizontal(|ui| {
                                kit::done_label(ui, tf!("Paired with {host} as {name}", host = host_of(&paired.address), name = paired.name), theme.action);
                            });
                            if !paired.has_key {
                                ui.label(RichText::new(t!("This device has no key for the secrets yet.")).color(theme.warning));
                            }
                            if let Some(code) = self.recovery.clone() {
                                ui.add_space(8.0);
                                kit::note(
                                    ui,
                                    &icons,
                                    &theme,
                                    "warning",
                                    t!("Write this recovery code down"),
                                    t!("It is the key your secrets are sealed with. Your devices pass it on to each other; if all of them are ever gone, this code is the only way to read the secrets on the host again. It is shown this once."),
                                );
                                ui.label(RichText::new(&code).monospace().size(13.0));
                                ui.horizontal(|ui| {
                                    if kit::secondary_button(ui, &theme, t!("Copy")).clicked() {
                                        ui.ctx().copy_text(code.clone());
                                    }
                                    if kit::secondary_button(ui, &theme, t!("I wrote it down")).clicked() {
                                        self.recovery = None;
                                    }
                                });
                            }
                            ui.add_space(8.0);
                            kit::section(ui, &theme, t!("This project"));
                            // Kept here already: how it stands. Otherwise the
                            // one button that does it all.
                            let kept = self.project.0.as_deref().is_some_and(|at| at.starts_with(&format!("{}/git/", paired.address.trim_end_matches('/'))));
                            if kept {
                                kit::done_label(ui, t!("This project is kept on the host, and stays in step by itself."), theme.action);
                                if let Some(status) = &self.project.1 {
                                    ui.label(RichText::new(status).size(12.0).color(theme.text_muted));
                                }
                            } else if let Some((false, why)) = &self.project.2 {
                                ui.label(RichText::new(why).color(theme.warning));
                            }
                            ui.add_enabled_ui(has_project && busy.is_none(), |ui| {
                                ui.horizontal_wrapped(|ui| {
                                    if !kept {
                                        keep = kit::primary_button(ui, &theme, t!("Keep this project on the host")).on_hover_text(t!("Stores the project on the host. From then on every save goes there, and what your other devices store comes here, by itself.")).clicked();
                                    }
                                    ui.add_enabled_ui(paired.has_key, |ui| {
                                        send = kit::secondary_button(ui, &theme, t!("Send its secrets")).on_hover_text(t!("Seals the bot token and the project's other secrets on this computer and puts them in the host's vault, for your other devices")).clicked();
                                        get = kit::secondary_button(ui, &theme, t!("Get its secrets")).on_hover_text(t!("Gets the project's secrets from the vault and saves them in this computer's credential store")).clicked();
                                    });
                                });
                            });
                            kit::caption(ui, &theme, t!("The host can't read the secrets: they are sealed on your devices."));
                            ui.add_space(8.0);
                            kit::section(ui, &theme, t!("Your devices"));
                            match &self.adding {
                                Some(code) => {
                                    ui.label(t!("On the other device, open File › Host… and type in this code:"));
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new(code).monospace().size(22.0).strong());
                                        if kit::secondary_button(ui, &theme, t!("Copy")).clicked() {
                                            ui.ctx().copy_text(code.clone());
                                        }
                                    });
                                    ui.horizontal(|ui| {
                                        ui.spinner();
                                        ui.label(RichText::new(t!("Leave this open until the other device says Paired.")).color(theme.text_muted));
                                        cancel_add = kit::secondary_button(ui, &theme, t!("Cancel")).clicked();
                                    });
                                }
                                None => {
                                    ui.add_enabled_ui(busy.is_none() && paired.has_key, |ui| {
                                        add = kit::secondary_button(ui, &theme, t!("Add a device")).clicked();
                                    });
                                }
                            }
                            ui.add_space(8.0);
                            if self.unpairing {
                                kit::note(ui, &icons, &theme, "warning", t!("Unpair this computer?"), t!("It is taken off the host's list, and its token and its key for the secrets are deleted here. Your projects and the secrets saved on this computer stay."));
                                ui.horizontal(|ui| {
                                    unpair = kit::secondary_button(ui, &theme, t!("Unpair")).clicked();
                                    if kit::secondary_button(ui, &theme, t!("Cancel")).clicked() {
                                        self.unpairing = false;
                                    }
                                });
                            } else if kit::link(ui, &theme, t!("Unpair this computer…"), egui::FontId::proportional(12.5)).clicked() {
                                self.unpairing = true;
                            }
                        }
                    }
                    ui.add_space(8.0);
                    if let Some(what) = busy.filter(|_| self.adding.is_none()) {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(RichText::new(tf!("{what} …", what = busy_text(what))).size(12.0).color(theme.text_muted));
                        });
                    } else if let Some((ok, text)) = &self.said {
                        ui.label(RichText::new(text).color(if *ok { theme.action } else { theme.warning }));
                    }
                }
                Part::Footer => {
                    if self.loaded && self.paired.is_none() {
                        ui.add_enabled_ui(busy.is_none() && !self.address.trim().is_empty() && !self.code.trim().is_empty() && !self.name.trim().is_empty(), |ui| {
                            pair = kit::primary_button(ui, &theme, t!("Pair")).clicked();
                        });
                    }
                    close = kit::secondary_button(ui, &theme, t!("Close")).clicked();
                }
            },
        );
        if pair {
            self.pair();
        }
        if add {
            self.add_device();
        }
        if cancel_add {
            if let Some(stop) = &self.stop_adding {
                stop.store(true, Ordering::Relaxed);
            }
            self.adding = None;
        }
        if keep && let (Some(paired), Some(project)) = (&self.paired, &ed.project) {
            self.keep = Some(host_link::project_address(&paired.address, &project.meta.name));
        }
        if send {
            self.send_secrets(ed);
        }
        if get {
            self.get_secrets(ed);
        }
        if unpair {
            self.unpair();
        }
        if close {
            self.open = false;
        }
    }
}

fn busy_text(what: &str) -> &'static str {
    match what {
        "Pairing" => t!("Pairing"),
        "Waiting for the other device" => t!("Waiting for the other device"),
        "Sending the secrets" => t!("Sending the secrets"),
        "Getting the secrets" => t!("Getting the secrets"),
        _ => t!("Unpairing"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_place_of_a_host() {
        assert_eq!(host_of("https://Circuit.Example.com/"), "circuit.example.com");
        assert_eq!(host_of(" http://127.0.0.1:7421 "), "127.0.0.1:7421");
        assert!(!computer_name().is_empty());
    }

    /// The dialog's own steps against a running host (`XC_HOST_TEST` names
    /// a file with its Git address and a paired device's token): this
    /// computer comes in as a second device, gets the key, and a project's
    /// secret goes to the vault and comes back. Skipped without one. It
    /// writes to the system's credential store, and tidies up after itself.
    #[test]
    fn pairs_and_carries_a_secret() {
        let Some(file) = std::env::var_os("XC_HOST_TEST") else { return };
        let text = std::fs::read_to_string(file).unwrap();
        let mut lines = text.lines();
        let (git, token) = (lines.next().unwrap(), lines.next().unwrap());
        let address = git.split("/git/").next().unwrap().to_string();
        let dir = std::env::temp_dir().join(format!("xc-host-ui-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        // SAFETY: the only test that sets it, and it only runs by hand.
        unsafe { std::env::set_var("XC_CIRCUIT_DATA", dir.join("data")) };
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        circuit_core::Project::create(dir.join("bot"), "My Bot", "none").unwrap();
        let mut ed = Editor::new(vec![root.join("plugins")], None).unwrap();
        ed.open_project(&dir.join("bot"));
        let ctx = egui::Context::default();
        let wait = |ui: &mut HostUi, ed: &mut Editor| {
            for _ in 0..3000 {
                ui.poll(ed, &ctx);
                if ui.jobs.is_empty() {
                    return;
                }
                thread::sleep(Duration::from_millis(10));
            }
            panic!("still busy");
        };

        // The device that is paired already shows a code and waits.
        let mut first = Link::open(&address).unwrap();
        first.hello(token).unwrap();
        let key = VaultKey::new().unwrap();
        let known: Vec<String> = first.devices().unwrap().into_iter().map(|d| d.id).collect();
        let (for_host, secret) = (first.invite().unwrap(), host_link::code_secret().unwrap());
        let other = {
            let (key, secret) = (key.clone(), secret.clone());
            thread::spawn(move || {
                for _ in 0..100 {
                    if let Some(new) = first.devices().unwrap().into_iter().find(|d| !known.contains(&d.id)) {
                        first.key_put(&new.id, &host_link::wrap(&key, &new.key, &secret).unwrap()).unwrap();
                        return first;
                    }
                    thread::sleep(Duration::from_millis(100));
                }
                panic!("nobody came");
            })
        };

        let mut ui = HostUi::new();
        ui.open();
        wait(&mut ui, &mut ed);
        assert!(ui.loaded && ui.paired.is_none());
        ui.address = address.clone();
        ui.name = "Test computer".into();
        ui.code = "ABCD".into();
        ui.pair();
        wait(&mut ui, &mut ed);
        assert!(ui.said.as_ref().unwrap().1.contains("8 letters and digits"), "{:?}", ui.said);
        ui.code = format!("{for_host}-{secret}").to_lowercase();
        ui.pair();
        wait(&mut ui, &mut ed);
        let paired = ui.paired.clone().unwrap_or_else(|| panic!("{:?}", ui.said));
        assert!(paired.has_key && ui.recovery.is_none(), "the key came from the other device: {:?}", ui.said);
        let mut first = other.join().unwrap();
        assert_eq!(load_pairing().map(|(p, _, k)| (p, k == Some(key.clone()))), Some((paired.clone(), true)));
        assert!(crate::bot::sync_login(&host_of(&address)).is_some(), "File › Git… is signed in there too");

        // A secret of the project: sealed, sent, gone here, and back.
        let project = ed.project.as_ref().unwrap();
        let (root, id) = (project.root.clone(), project.meta.id.clone());
        secrets::set(&root, "token", "bot-token-for-the-test").unwrap();
        ui.send_secrets(&ed);
        wait(&mut ui, &mut ed);
        assert_eq!(ui.said.clone().map(|(ok, _)| ok), Some(true), "{:?}", ui.said);
        let sealed = first.vault_get(&id).unwrap().0.unwrap();
        assert!(!sealed.contains("bot-token") && String::from_utf8(key.open(&sealed).unwrap()).unwrap().contains("bot-token-for-the-test"));
        secrets::set(&root, "token", "").unwrap();
        assert!(secrets::get(&root, "token").is_none());
        ui.get_secrets(&ed);
        wait(&mut ui, &mut ed);
        assert_eq!(secrets::get(&root, "token").as_deref(), Some("bot-token-for-the-test"), "{:?}", ui.said);

        // Keeping the project there: the address File › Git… connects to.
        assert_eq!(host_link::project_address(&paired.address, "My Bot"), format!("{address}/git/my-bot.git"));

        // Unpairing: off the host's list, nothing kept here.
        secrets::set(&root, "token", "").unwrap();
        ui.unpair();
        wait(&mut ui, &mut ed);
        assert!(ui.paired.is_none() && load_pairing().is_none() && crate::bot::sync_login(&host_of(&address)).is_none());
        assert!(!first.devices().unwrap().iter().any(|d| d.id == paired.device));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
