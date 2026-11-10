//! A live session: several people in one project at once.
//!
//! One of them is the **host**: the project is theirs, they hold the one
//! true state of it, and they alone save it and run the bot. The others are
//! **guests**. A guest's app sends what its workspace looks like now and
//! which of the host's versions that started from; the host puts it
//! together with what it has (by what the files hold, as when two stored
//! versions meet: [`crate::json`]), and tells everyone the result. Where
//! both changed the same thing, the host's stays, so nothing needs asking.
//!
//! This module is that exchange alone, as two state machines that take and
//! give messages: no network, no window. A project is a set of documents by
//! key: each workspace under its key, and [`META`] for what belongs to the
//! project as a whole.

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{Side, json};

/// The document that holds what isn't a workspace: the slash commands.
pub const META: &str = "@meta";
/// How many of a document's earlier versions the host keeps, to put a
/// guest's change together with what happened since the guest's version.
const PAST: usize = 64;

/// A project as the session sees it: its documents by key.
pub type Docs = BTreeMap<String, Value>;

/// Which workspaces a guest may change the blocks of.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Blocks {
    #[default]
    None,
    All,
    /// These workspaces only.
    Only(BTreeSet<String>),
}

/// What a guest may do. A new guest may only watch.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Perms {
    /// Start, stop and restart the bot (at the host's).
    #[serde(default)]
    pub power: bool,
    /// Make, change and delete slash commands.
    #[serde(default)]
    pub commands: bool,
    /// Make, rename, move, turn on and off, and delete workspaces.
    #[serde(default)]
    pub workspaces: bool,
    /// Change blocks, wires and notes.
    #[serde(default)]
    pub blocks: Blocks,
}

impl Perms {
    fn blocks_of(&self, key: &str) -> bool {
        match &self.blocks {
            Blocks::All => true,
            Blocks::None => false,
            Blocks::Only(keys) => keys.contains(key),
        }
    }
}

/// What the two sides tell each other.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "m", rename_all = "snake_case")]
pub enum Msg {
    /// A guest, on arriving.
    Hello { name: String, version: String },
    /// The host's answer: the project as it is, and what the guest may do.
    /// `files` are the project's other files (the project file, the
    /// commands), as they would be stored.
    Welcome { host: String, files: BTreeMap<String, String>, docs: Docs, rev: u64, perms: Perms },
    /// The host doesn't let the guest in.
    Refused { why: String },
    /// A guest's document as it is now, made from the host's version `rev`.
    /// `state` is `None` for a deleted one.
    Propose { key: String, rev: u64, state: Option<Value> },
    /// The host's document as it is now.
    Update { key: String, rev: u64, state: Option<Value> },
    /// The host didn't take a guest's change: the guest's copy goes back to
    /// this.
    Reset { key: String, rev: u64, state: Option<Value>, why: String },
    /// What the guest may do from now on.
    Perms { perms: Perms },
}

fn merge(key: &str, base: Option<&Value>, mine: &Value, theirs: &Value, keep: Side) -> Value {
    // Whose version stays also says whose blocks keep their numbers: the
    // host's, on both sides, so everyone ends up with the same.
    let side = keep;
    let keep = move |_: &str| Some(side);
    if key == META { json::merge_with(base, mine, theirs, &keep).value } else { json::workspace_numbered(base, mine, theirs, &keep, side).value }
}

/// What a change to a document needs leave for.
#[derive(Debug, Default, PartialEq, Eq)]
struct Needs {
    commands: bool,
    workspaces: bool,
    blocks: bool,
}

fn needs(key: &str, base: Option<&Value>, proposed: Option<&Value>) -> Needs {
    if key == META {
        return Needs { commands: base != proposed, ..Default::default() };
    }
    let (Some(base), Some(proposed)) = (base, proposed) else { return Needs { workspaces: true, ..Default::default() } };
    let mut out = Needs::default();
    let keys: BTreeSet<&String> = base.as_object().into_iter().flatten().map(|(k, _)| k).chain(proposed.as_object().into_iter().flatten().map(|(k, _)| k)).collect();
    for k in keys {
        if base.get(k) != proposed.get(k) {
            match k.as_str() {
                "nodes" | "links" | "notes" | "zones" => out.blocks = true,
                _ => out.workspaces = true,
            }
        }
    }
    out
}

/// A guest, as the host knows them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Guest {
    pub name: String,
    pub perms: Perms,
}

/// What the host's side asks for after taking something in.
#[derive(Debug, Default, PartialEq)]
pub struct HostTurn {
    /// Messages to send: to one guest, or to all (`None`).
    pub send: Vec<(Option<String>, Msg)>,
    /// Documents a guest changed: the key, what it was and what it is now
    /// (`None`: deleted). For the project in the window to follow.
    pub changed: Vec<(String, Option<Value>, Option<Value>)>,
}

pub struct HostSide {
    name: String,
    version: String,
    docs: Docs,
    rev: u64,
    /// Earlier versions by document: the version number, and the state.
    past: HashMap<String, VecDeque<(u64, Option<Value>)>>,
    pub guests: BTreeMap<String, Guest>,
}

impl HostSide {
    /// `version` is the app's: a guest with another isn't let in.
    pub fn new(name: &str, version: &str, docs: Docs) -> HostSide {
        let past = docs.iter().map(|(k, v)| (k.clone(), VecDeque::from([(0, Some(v.clone()))]))).collect();
        HostSide { name: name.into(), version: version.into(), docs, rev: 0, past, guests: BTreeMap::new() }
    }

    pub fn docs(&self) -> &Docs {
        &self.docs
    }

    fn record(&mut self, key: &str, state: Option<Value>) -> u64 {
        self.rev += 1;
        match &state {
            Some(v) => drop(self.docs.insert(key.to_string(), v.clone())),
            None => drop(self.docs.remove(key)),
        }
        let past = self.past.entry(key.to_string()).or_default();
        past.push_back((self.rev, state));
        while past.len() > PAST {
            past.pop_front();
        }
        self.rev
    }

    /// The version a document has now (0: as when the session started).
    fn rev_of(&self, key: &str) -> u64 {
        self.past.get(key).and_then(|p| p.back()).map_or(0, |(rev, _)| *rev)
    }

    /// The host changed the project themself: `now` is all of it. Gives the
    /// news for the guests.
    pub fn local(&mut self, now: &Docs) -> Vec<(Option<String>, Msg)> {
        let keys: BTreeSet<String> = self.docs.keys().chain(now.keys()).cloned().collect();
        let mut out = Vec::new();
        for key in keys {
            if self.docs.get(&key) != now.get(&key) {
                let state = now.get(&key).cloned();
                let rev = self.record(&key, state.clone());
                out.push((None, Msg::Update { key, rev, state }));
            }
        }
        out
    }

    /// A guest left (or was sent away).
    pub fn left(&mut self, id: &str) {
        self.guests.remove(id);
    }

    /// Changes what a guest may do, and tells them.
    pub fn allow(&mut self, id: &str, perms: Perms) -> Vec<(Option<String>, Msg)> {
        match self.guests.get_mut(id) {
            Some(guest) => {
                guest.perms = perms.clone();
                vec![(Some(id.to_string()), Msg::Perms { perms })]
            }
            None => Vec::new(),
        }
    }

    /// Takes a guest's message. `files` are the project's other files, for
    /// a guest that just arrived.
    pub fn receive(&mut self, from: &str, msg: Msg, files: &BTreeMap<String, String>) -> HostTurn {
        let mut turn = HostTurn::default();
        let to = Some(from.to_string());
        match msg {
            Msg::Hello { name, version } => {
                if version != self.version {
                    let who = if newer(&version, &self.version) { "the host" } else { "you" };
                    turn.send.push((to, Msg::Refused { why: format!("The session's host has XC Circuit {} and you have {version}: both need the same version, so {who} must update.", self.version) }));
                    return turn;
                }
                let name: String = name.trim().chars().take(40).collect();
                let guest = Guest { name: if name.is_empty() { "Guest".into() } else { name }, perms: Perms::default() };
                turn.send.push((to, Msg::Welcome { host: self.name.clone(), files: files.clone(), docs: self.docs.clone(), rev: self.rev, perms: guest.perms.clone() }));
                self.guests.insert(from.to_string(), guest);
            }
            Msg::Propose { key, rev, state } => {
                let Some(guest) = self.guests.get(from) else { return turn };
                let now = self.docs.get(&key).cloned();
                let now_rev = self.rev_of(&key);
                // What the guest started from: that version, if it is still
                // known. A document the guest made has none.
                let base = match self.past.get(&key) {
                    Some(past) => match past.iter().rev().find(|(r, _)| *r <= rev) {
                        Some((_, state)) => state.clone(),
                        // Too long ago: the guest gets the newest and tries again.
                        None => {
                            turn.send.push((to, Msg::Update { key, rev: now_rev, state: now }));
                            return turn;
                        }
                    },
                    None => None,
                };
                let wants = needs(&key, base.as_ref(), state.as_ref());
                let why = if wants.commands && !guest.perms.commands {
                    Some("You may not change the slash commands in this session.")
                } else if wants.workspaces && !guest.perms.workspaces {
                    Some("You may not make, rename or delete workspaces in this session.")
                } else if wants.blocks && !guest.perms.blocks_of(&key) {
                    Some("You may not change the blocks of this workspace in this session.")
                } else {
                    None
                };
                if let Some(why) = why {
                    turn.send.push((to, Msg::Reset { key, rev: now_rev, state: now, why: why.into() }));
                    return turn;
                }
                let merged = match (&now, &state) {
                    // Deleted by the guest: gone, unless the host changed it since.
                    (Some(mine), None) => (base.as_ref() != Some(mine)).then(|| mine.clone()),
                    // Deleted here meanwhile: it stays deleted.
                    (None, Some(theirs)) => base.is_none().then(|| theirs.clone()),
                    (Some(mine), Some(theirs)) => Some(merge(&key, base.as_ref(), mine, theirs, Side::Mine)),
                    (None, None) => None,
                };
                if merged != now {
                    let rev = self.record(&key, merged.clone());
                    turn.send.push((None, Msg::Update { key: key.clone(), rev, state: merged.clone() }));
                    turn.changed.push((key, now, merged));
                } else if merged != state {
                    // Nothing of it was new, or the host's side won: the
                    // guest's copy follows.
                    turn.send.push((to, Msg::Update { key, rev: now_rev, state: now }));
                }
            }
            _ => {}
        }
        turn
    }
}

/// Whether version `a` is newer than `b` (`0.3.1` against `0.3.0`).
fn newer(a: &str, b: &str) -> bool {
    let parts = |v: &str| -> Vec<u64> { v.split(['.', '-']).map(|p| p.parse().unwrap_or(0)).collect() };
    parts(a) > parts(b)
}

/// What the guest's side asks for after taking something in.
#[derive(Debug, Default, PartialEq)]
pub struct GuestTurn {
    /// Documents to change in the guest's project: the key, the host's
    /// state before, the host's state now, and what the guest's copy
    /// becomes (`None`: deleted).
    pub apply: Vec<Applied>,
    /// Something to tell the guest.
    pub notice: Option<String>,
    /// The session's project, on arriving.
    pub welcome: Option<(String, BTreeMap<String, String>, Docs)>,
    /// Not let in: why.
    pub refused: Option<String>,
}

/// One document changed by the host, for the guest's project to follow.
#[derive(Debug, PartialEq)]
pub struct Applied {
    pub key: String,
    pub before: Option<Value>,
    pub after: Option<Value>,
    pub local: Option<Value>,
}

#[derive(Default)]
pub struct GuestSide {
    /// The host's documents as last heard, and their versions.
    synced: Docs,
    revs: HashMap<String, u64>,
    /// What was sent and not answered yet, so it isn't sent again.
    proposed: HashMap<String, Option<Value>>,
    pub perms: Perms,
    pub host: String,
}

impl GuestSide {
    pub fn new() -> GuestSide {
        GuestSide::default()
    }

    /// The first message, on arriving.
    pub fn hello(name: &str, version: &str) -> Msg {
        Msg::Hello { name: name.into(), version: version.into() }
    }

    /// The guest changed the project: `now` is all of it. Gives what to
    /// send to the host.
    pub fn local(&mut self, now: &Docs) -> Vec<Msg> {
        let keys: BTreeSet<String> = self.synced.keys().chain(now.keys()).cloned().collect();
        let mut out = Vec::new();
        for key in keys {
            let (mine, theirs) = (now.get(&key), self.synced.get(&key));
            if mine != theirs && self.proposed.get(&key).map(Option::as_ref) != Some(mine) {
                self.proposed.insert(key.clone(), mine.cloned());
                out.push(Msg::Propose { rev: self.revs.get(&key).copied().unwrap_or(0), state: mine.cloned(), key });
            }
        }
        out
    }

    /// Takes the host's message; `now` is the guest's project as it is.
    pub fn receive(&mut self, msg: Msg, now: &Docs) -> GuestTurn {
        let mut turn = GuestTurn::default();
        match msg {
            Msg::Welcome { host, files, docs, rev, perms } => {
                self.revs = docs.keys().map(|k| (k.clone(), rev)).collect();
                self.synced = docs.clone();
                self.proposed.clear();
                self.perms = perms;
                self.host = host.clone();
                turn.welcome = Some((host, files, docs));
            }
            Msg::Refused { why } => turn.refused = Some(why),
            Msg::Perms { perms } => {
                self.perms = perms;
                // What was refused before may be welcome now.
                self.proposed.clear();
            }
            Msg::Update { key, rev, state } | Msg::Reset { key, rev, state, .. } if rev < self.revs.get(&key).copied().unwrap_or(0) => {
                // Older than what is here already: overtaken on the way.
                let _ = (key, state);
            }
            Msg::Update { key, rev, state } => {
                let (mine, before) = (now.get(&key).cloned(), self.synced.get(&key).cloned());
                let local = if mine == before {
                    state.clone()
                } else {
                    match (&mine, &state) {
                        // Deleted by the host: gone here too.
                        (_, None) => None,
                        // Deleted here, not answered yet: it stays so.
                        (None, Some(_)) => None,
                        (Some(mine), Some(theirs)) => Some(merge(&key, before.as_ref(), mine, theirs, Side::Theirs)),
                    }
                };
                self.take(&key, rev, state.clone());
                if local != mine {
                    turn.apply.push(Applied { key, before, after: state, local });
                }
            }
            Msg::Reset { key, rev, state, why } => {
                let (mine, before) = (now.get(&key).cloned(), self.synced.get(&key).cloned());
                self.take(&key, rev, state.clone());
                // Not asked again as it is: the copy is the host's now.
                if state != mine {
                    turn.apply.push(Applied { key, before, after: state.clone(), local: state });
                }
                turn.notice = Some(why);
            }
            Msg::Hello { .. } | Msg::Propose { .. } => {}
        }
        turn
    }

    fn take(&mut self, key: &str, rev: u64, state: Option<Value>) {
        match state {
            Some(v) => drop(self.synced.insert(key.to_string(), v)),
            None => drop(self.synced.remove(key)),
        }
        self.revs.insert(key.to_string(), rev);
        self.proposed.remove(key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ws(name: &str, nodes: Value, links: Value) -> Value {
        json!({ "name": name, "nodes": nodes, "links": links })
    }

    fn node(uid: &str, text: &str, at: [i32; 2]) -> Value {
        json!({ "block": "log@1", "uid": uid, "at": at, "values": { "message": text } })
    }

    /// A session with one host and its guests, their messages passed on by
    /// hand, so a test decides what arrives when.
    struct Table {
        host: HostSide,
        host_docs: Docs,
        guests: BTreeMap<String, (GuestSide, Docs, Vec<String>)>,
        files: BTreeMap<String, String>,
    }

    impl Table {
        fn new(docs: Docs) -> Table {
            Table { host: HostSide::new("Ada", "0.3.0", docs.clone()), host_docs: docs, guests: BTreeMap::new(), files: BTreeMap::from([("circuit.project.json".to_string(), "{}".to_string())]) }
        }

        fn deliver(&mut self, send: Vec<(Option<String>, Msg)>) {
            for (to, msg) in send {
                let ids: Vec<String> = match to {
                    Some(id) => vec![id],
                    None => self.guests.keys().cloned().collect(),
                };
                for id in ids {
                    let Some((guest, docs, notices)) = self.guests.get_mut(&id) else { continue };
                    let turn = guest.receive(msg.clone(), docs);
                    if let Some((_, _, all)) = turn.welcome {
                        *docs = all;
                    }
                    for a in turn.apply {
                        match a.local {
                            Some(v) => drop(docs.insert(a.key, v)),
                            None => drop(docs.remove(&a.key)),
                        }
                    }
                    notices.extend(turn.notice);
                    notices.extend(turn.refused);
                }
            }
        }

        fn to_host(&mut self, from: &str, msgs: Vec<Msg>) {
            for msg in msgs {
                let turn = self.host.receive(from, msg, &self.files);
                for (key, _, now) in turn.changed {
                    match now {
                        Some(v) => drop(self.host_docs.insert(key, v)),
                        None => drop(self.host_docs.remove(&key)),
                    }
                }
                self.deliver(turn.send);
            }
        }

        fn join(&mut self, id: &str, name: &str, version: &str) {
            self.guests.insert(id.into(), (GuestSide::new(), Docs::new(), Vec::new()));
            self.to_host(id, vec![GuestSide::hello(name, version)]);
        }

        /// The guest's edits go to the host, and the answers come back.
        fn guest_sync(&mut self, id: &str) {
            let (guest, docs, _) = self.guests.get_mut(id).unwrap();
            let msgs = guest.local(&docs.clone());
            self.to_host(id, msgs);
        }

        fn host_sync(&mut self) {
            let send = self.host.local(&self.host_docs.clone());
            self.deliver(send);
        }

        fn docs(&mut self, id: &str) -> &mut Docs {
            &mut self.guests.get_mut(id).unwrap().1
        }

        fn same_everywhere(&self) -> bool {
            self.guests.values().all(|(_, docs, _)| *docs == self.host_docs) && *self.host.docs() == self.host_docs
        }
    }

    fn start() -> Table {
        Table::new(Docs::from([("main".to_string(), ws("Main", json!({ "1": node("a", "one", [0, 0]) }), json!([]))), (META.to_string(), json!({ "slashCommands": [] }))]))
    }

    #[test]
    fn a_guest_arrives_and_watches() {
        let mut t = start();
        t.join("g1", "  Grace  ", "0.3.0");
        assert_eq!(t.host.guests["g1"], Guest { name: "Grace".into(), perms: Perms::default() });
        assert_eq!((t.guests["g1"].0.host.as_str(), t.same_everywhere()), ("Ada", true));

        // Another version of the app isn't let in, and is told who updates.
        t.join("old", "Old", "0.2.10");
        assert!(!t.host.guests.contains_key("old") && t.guests["old"].2[0].contains("so you must update"), "{:?}", t.guests["old"].2);
        t.join("new", "New", "0.4.0");
        assert!(t.guests["new"].2[0].contains("so the host must update"));
        t.guests.remove("old");
        t.guests.remove("new");

        // What the host does shows at the guest's.
        t.host_docs.get_mut("main").unwrap()["nodes"]["2"] = node("b", "two", [300, 0]);
        t.host_docs.insert("extra".into(), ws("Extra", json!({}), json!([])));
        t.host_sync();
        assert!(t.same_everywhere() && t.docs("g1").contains_key("extra"));
        t.host_docs.remove("extra");
        t.host_sync();
        assert!(t.same_everywhere() && !t.docs("g1").contains_key("extra"));

        // A guest who may only watch gets their change taken back, and is told.
        t.docs("g1").get_mut("main").unwrap()["nodes"]["1"]["values"]["message"] = "mine".into();
        t.guest_sync("g1");
        assert!(t.same_everywhere() && t.host_docs["main"]["nodes"]["1"]["values"]["message"] == "one");
        assert!(t.guests["g1"].2[0].contains("may not change the blocks"));
    }

    #[test]
    fn guests_change_what_they_may() {
        let mut t = start();
        t.join("g1", "Grace", "0.3.0");
        t.join("g2", "Linus", "0.3.0");
        let send = t.host.allow("g1", Perms { blocks: Blocks::Only(BTreeSet::from(["main".to_string()])), ..Default::default() });
        t.deliver(send);
        assert_eq!(t.guests["g1"].0.perms.blocks, Blocks::Only(BTreeSet::from(["main".to_string()])));

        // Blocks in the workspace they were given: yes. Its name: no.
        t.docs("g1").get_mut("main").unwrap()["nodes"]["2"] = node("g", "from Grace", [300, 0]);
        t.guest_sync("g1");
        assert!(t.same_everywhere() && t.host_docs["main"]["nodes"]["2"]["uid"] == "g", "and the other guest has it too");
        t.docs("g1").get_mut("main").unwrap()["name"] = "Renamed".into();
        t.guest_sync("g1");
        assert!(t.same_everywhere() && t.host_docs["main"]["name"] == "Main");
        assert!(t.guests["g1"].2.last().unwrap().contains("make, rename or delete workspaces"));
        // A new workspace, a deleted one, the slash commands: each its own leave.
        t.docs("g1").insert("new".into(), ws("New", json!({}), json!([])));
        t.guest_sync("g1");
        assert!(t.same_everywhere() && !t.host_docs.contains_key("new"));
        *t.docs("g1").get_mut(META).unwrap() = json!({ "slashCommands": [{ "name": "ping" }] });
        t.guest_sync("g1");
        assert!(t.same_everywhere() && t.guests["g1"].2.last().unwrap().contains("slash commands"));

        let send = t.host.allow("g1", Perms { commands: true, workspaces: true, blocks: Blocks::All, power: false });
        t.deliver(send);
        t.docs("g1").insert("new".into(), ws("New", json!({}), json!([])));
        *t.docs("g1").get_mut(META).unwrap() = json!({ "slashCommands": [{ "name": "ping" }] });
        t.docs("g1").get_mut("main").unwrap()["name"] = "Renamed".into();
        t.guest_sync("g1");
        assert!(t.same_everywhere() && t.host_docs.contains_key("new") && t.host_docs["main"]["name"] == "Renamed" && t.host_docs[META]["slashCommands"][0]["name"] == "ping");
        t.docs("g1").remove("new");
        t.guest_sync("g1");
        assert!(t.same_everywhere() && !t.host_docs.contains_key("new"));
        // The one who may only watch still can't.
        t.docs("g2").remove("main");
        t.guest_sync("g2");
        assert!(t.same_everywhere() && t.host_docs.contains_key("main"));
    }

    #[test]
    fn everyone_at_once() {
        let mut t = start();
        t.join("g1", "Grace", "0.3.0");
        t.join("g2", "Linus", "0.3.0");
        for id in ["g1", "g2"] {
            let send = t.host.allow(id, Perms { blocks: Blocks::All, workspaces: true, ..Default::default() });
            t.deliver(send);
        }
        // All three change the same workspace before any of it arrives:
        // each adds a block 2 (three blocks, then), the host moves block 1
        // and one guest changes its message, the other the workspace's name.
        t.host_docs.get_mut("main").unwrap()["nodes"]["2"] = node("h", "host's", [0, 300]);
        t.host_docs.get_mut("main").unwrap()["nodes"]["1"]["at"] = json!([50, 50]);
        let g1 = t.docs("g1").get_mut("main").unwrap();
        g1["nodes"]["2"] = node("x", "Grace's, after ${#1.result}", [300, 0]);
        g1["links"] = json!(["1.next > 2.in"]);
        g1["nodes"]["1"]["values"]["message"] = "one, by Grace".into();
        let g2 = t.docs("g2").get_mut("main").unwrap();
        g2["nodes"]["2"] = node("y", "Linus's", [600, 0]);
        g2["name"] = "Welcome".into();
        // The guests' messages are on their way while the host's news goes out.
        let from_g1 = { let (g, d, _) = t.guests.get_mut("g1").unwrap(); g.local(&d.clone()) };
        let from_g2 = { let (g, d, _) = t.guests.get_mut("g2").unwrap(); g.local(&d.clone()) };
        t.host_sync();
        t.to_host("g1", from_g1);
        t.to_host("g2", from_g2);
        // Until everyone has said and heard everything.
        for _ in 0..4 {
            t.guest_sync("g1");
            t.guest_sync("g2");
            t.host_sync();
        }
        assert!(t.same_everywhere(), "host {:#}\ng1 {:#}\ng2 {:#}", t.host_docs["main"], t.guests["g1"].1["main"], t.guests["g2"].1["main"]);
        let main = &t.host_docs["main"];
        let nodes = main["nodes"].as_object().unwrap();
        let by_uid = |uid: &str| nodes.iter().find(|(_, n)| n["uid"] == uid).map(|(k, n)| (k.clone(), n.clone())).unwrap_or_else(|| panic!("no block {uid}: {nodes:?}"));
        assert_eq!(nodes.len(), 4, "the first block and one from each: {nodes:?}");
        assert_eq!(by_uid("h").0, "2", "the host's kept the number");
        assert_eq!((main["name"].as_str(), by_uid("a").1["values"]["message"].as_str(), by_uid("a").1["at"].clone()), (Some("Welcome"), Some("one, by Grace"), json!([50, 50])));
        // Grace's block got another number, and its wire and what reads block 1 are right.
        let (grace, block) = by_uid("x");
        assert!(grace != "2" && block["values"]["message"] == "Grace's, after ${#1.result}");
        assert!(main["links"].as_array().unwrap().contains(&json!(format!("1.next > {grace}.in"))), "{:?}", main["links"]);

        // The same setting, two ways: the host's stays, and the guest sees it.
        t.host_docs.get_mut("main").unwrap()["nodes"]["1"]["values"]["message"] = "the host says".into();
        t.docs("g1").get_mut("main").unwrap()["nodes"]["1"]["values"]["message"] = "Grace says".into();
        let from_g1 = { let (g, d, _) = t.guests.get_mut("g1").unwrap(); g.local(&d.clone()) };
        t.host_sync();
        t.to_host("g1", from_g1);
        t.guest_sync("g1");
        assert!(t.same_everywhere() && t.host_docs["main"]["nodes"]["1"]["values"]["message"] == "the host says");

        // A guest who left is forgotten; what they did stays.
        t.host.left("g2");
        t.guests.remove("g2");
        assert!(!t.host.guests.contains_key("g2") && t.same_everywhere());
    }

    #[test]
    fn says_what_a_change_needs() {
        let base = ws("W", json!({}), json!([]));
        let mut blocks = base.clone();
        blocks["nodes"]["1"] = node("a", "x", [0, 0]);
        assert_eq!(needs("w", Some(&base), Some(&blocks)), Needs { blocks: true, ..Default::default() });
        let mut named = base.clone();
        named["folder"] = "F".into();
        assert_eq!(needs("w", Some(&base), Some(&named)), Needs { workspaces: true, ..Default::default() });
        assert_eq!(needs("w", None, Some(&base)), Needs { workspaces: true, ..Default::default() });
        assert_eq!(needs("w", Some(&base), None), Needs { workspaces: true, ..Default::default() });
        assert_eq!(needs(META, Some(&json!({})), Some(&json!({ "slashCommands": [] }))), Needs { commands: true, ..Default::default() });
        assert_eq!(needs("w", Some(&base), Some(&base)), Needs::default());
        assert!(newer("0.3.1", "0.3.0") && newer("0.10.0", "0.9.9") && !newer("0.3.0", "0.3.0"));
        // Messages go over the wire as JSON, and come back the same.
        let msg = Msg::Propose { key: "main".into(), rev: 3, state: Some(base) };
        assert_eq!(serde_json::from_str::<Msg>(&serde_json::to_string(&msg).unwrap()).unwrap(), msg);
    }
}
