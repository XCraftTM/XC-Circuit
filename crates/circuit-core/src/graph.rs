//! A workspace graph: placed blocks, the links between their pins, Try Zones
//! and comment boxes.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;
use std::str::FromStr;

use indexmap::IndexMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};

/// The input pin every action block has.
pub const ACTION_IN: &str = "in";

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Workspace {
    pub name: String,
    /// The sidebar folder it is listed in, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
    pub nodes: IndexMap<String, Node>,
    pub links: Vec<Link>,
    #[serde(skip_serializing_if = "IndexMap::is_empty")]
    pub zones: IndexMap<String, Zone>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub comments: Vec<Comment>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Node {
    /// `id@major`, for example `discord.send_message@3`.
    pub block: String,
    #[serde(serialize_with = "round_point")]
    pub at: [f32; 2],
    #[serde(skip_serializing_if = "Map::is_empty")]
    pub values: Map<String, Value>,
    /// Fields switched to pins.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub pins: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zone: Option<String>,
    /// Heights of the text boxes on the block that were resized, by field.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub heights: BTreeMap<String, f32>,
    /// The block's width, when a text box's grip made it wider or narrower.
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "round_width")]
    pub width: Option<f32>,
}

impl Node {
    pub fn block_id(&self) -> &str {
        self.block.split('@').next().unwrap_or(&self.block)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PinRef {
    pub node: String,
    pub pin: String,
}

impl PinRef {
    pub fn new(node: impl Into<String>, pin: impl Into<String>) -> Self {
        Self { node: node.into(), pin: pin.into() }
    }
}

/// A wire from an output (or Action output) to an input. Saved as `a1.next > b2.in`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Link {
    pub from: PinRef,
    pub to: PinRef,
}

impl fmt::Display for Link {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{} > {}.{}", self.from.node, self.from.pin, self.to.node, self.to.pin)
    }
}

impl FromStr for Link {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (from, to) = s.split_once('>').ok_or_else(|| format!("link `{s}` has no `>`"))?;
        let parse = |p: &str| {
            let (node, pin) = p.trim().split_once('.').ok_or_else(|| format!("`{p}` is not `node.pin`"))?;
            Ok::<_, String>(PinRef::new(node, pin))
        };
        Ok(Link { from: parse(from)?, to: parse(to)? })
    }
}

impl Serialize for Link {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Link {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        String::deserialize(d)?.parse().map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Zone {
    pub title: String,
    /// x, y, width, height
    #[serde(serialize_with = "round_rect")]
    pub rect: [f32; 4],
    pub retry: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
}

/// The rectangle is recomputed from the zone's blocks, so it does not count as a change.
impl PartialEq for Zone {
    fn eq(&self, other: &Self) -> bool {
        self.title == other.title && self.retry == other.retry && self.timeout_ms == other.timeout_ms
    }
}

/// Copied blocks with the wires between them and the zones they use.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Clip {
    pub format: String,
    pub nodes: IndexMap<String, Node>,
    pub links: Vec<Link>,
    pub zones: IndexMap<String, Zone>,
}

pub const CLIP_FORMAT: &str = "xc-circuit-clip/1";

impl Workspace {
    /// Copies the given blocks, the wires between them and their zones.
    pub fn extract(&self, ids: &[String]) -> Clip {
        let nodes: IndexMap<String, Node> = self.nodes.iter().filter(|(k, _)| ids.contains(k)).map(|(k, n)| (k.clone(), n.clone())).collect();
        let zones = self
            .zones
            .iter()
            .filter(|(z, _)| nodes.values().any(|n| n.zone.as_deref() == Some(z.as_str())))
            .map(|(k, z)| (k.clone(), z.clone()))
            .collect::<IndexMap<_, _>>();
        let inside = |id: &str| nodes.contains_key(id) || zones.contains_key(id);
        let links = self.links.iter().filter(|l| inside(&l.from.node) && inside(&l.to.node)).cloned().collect();
        Clip { format: CLIP_FORMAT.into(), nodes, links, zones }
    }

    /// Pastes a clip with fresh ids, moved by `offset`. Returns the new block ids.
    pub fn paste(&mut self, clip: &Clip, offset: [f32; 2]) -> Vec<String> {
        let mut map: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        for (old, zone) in &clip.zones {
            let id = (1..).map(|i| format!("z{i}")).find(|z| !self.zones.contains_key(z) && !self.nodes.contains_key(z)).unwrap();
            self.zones.insert(id.clone(), zone.clone());
            map.insert(old.clone(), id);
        }
        let mut new_ids = Vec::new();
        for (old, node) in &clip.nodes {
            let id = self.next_node_id();
            let mut n = node.clone();
            n.at = [n.at[0] + offset[0], n.at[1] + offset[1]];
            n.zone = n.zone.as_ref().and_then(|z| map.get(z).cloned());
            self.nodes.insert(id.clone(), n);
            map.insert(old.clone(), id.clone());
            new_ids.push(id);
        }
        for l in &clip.links {
            if let (Some(from), Some(to)) = (map.get(&l.from.node), map.get(&l.to.node)) {
                self.links.push(Link { from: PinRef::new(from, &l.from.pin), to: PinRef::new(to, &l.to.pin) });
            }
        }
        new_ids
    }
}

/// The wires a workspace's paths follow: Action wires, and wires between
/// value blocks (which read each other when asked for a value).
pub struct Flow {
    value: HashSet<String>,
    next: HashMap<String, Vec<String>>,
}

impl Flow {
    fn follows(value: &HashSet<String>, l: &Link) -> bool {
        l.to.pin == ACTION_IN || (value.contains(&l.from.node) && value.contains(&l.to.node))
    }

    /// Whether `link` closes a loop: an Action wire back to a block earlier
    /// on the same path (it would run forever), or value blocks that read
    /// each other (they would wait for each other forever).
    pub fn would_loop(&self, link: &Link) -> bool {
        Flow::follows(&self.value, link) && self.reaches(&link.to.node, &link.from.node)
    }

    /// Whether the wires it follows go from `from` to `to`.
    pub fn reaches(&self, from: &str, to: &str) -> bool {
        let mut seen = HashSet::new();
        let mut todo = vec![from];
        while let Some(at) = todo.pop() {
            if at == to {
                return true;
            }
            if seen.insert(at) {
                todo.extend(self.next.get(at).into_iter().flatten().map(String::as_str));
            }
        }
        false
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Comment {
    pub text: String,
    #[serde(serialize_with = "round_rect")]
    pub rect: [f32; 4],
}

impl Workspace {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), ..Default::default() }
    }

    /// The next block number in this workspace: one more than the highest so
    /// far, so ids count up `1`, `2`, `3` and a deleted block's id isn't reused.
    pub fn next_node_id(&self) -> String {
        let max = self.nodes.keys().filter_map(|id| id.parse::<u64>().ok()).max().unwrap_or(0);
        (max + 1..).map(|i| i.to_string()).find(|id| !self.zones.contains_key(id)).expect("unbounded range")
    }

    pub fn links_into<'a>(&'a self, node: &'a str, pin: &'a str) -> impl Iterator<Item = &'a Link> + 'a {
        self.links.iter().filter(move |l| l.to.node == node && l.to.pin == pin)
    }

    pub fn is_connected(&self, node: &str, pin: &str) -> bool {
        self.links.iter().any(|l| (l.to.node == node && l.to.pin == pin) || (l.from.node == node && l.from.pin == pin))
    }

    pub fn remove_node(&mut self, id: &str) -> Option<Node> {
        self.links.retain(|l| l.from.node != id && l.to.node != id);
        self.nodes.shift_remove(id)
    }

    /// Adds a link. Data inputs take one wire unless `multi`, so an older wire into
    /// the same input is replaced. Action inputs accept any number of wires.
    ///
    /// Action paths never split: an output leads to one block, so a new Action
    /// wire from it replaces the one it had. Branches come from blocks with
    /// several outputs (If, Switch, For Each).
    pub fn connect(&mut self, link: Link, replace_existing_input: bool) {
        if self.links.contains(&link) {
            return;
        }
        if replace_existing_input {
            self.links.retain(|l| l.to != link.to);
        }
        if link.to.pin == ACTION_IN {
            self.links.retain(|l| !(l.from == link.from && l.to.pin == ACTION_IN));
        }
        self.links.push(link);
    }

    /// What runs or is read after what, to find loops. `value_block` tells
    /// which blocks work out their outputs when read.
    pub fn flow(&self, value_block: impl Fn(&str) -> bool) -> Flow {
        let value: HashSet<String> = self.nodes.keys().filter(|id| value_block(id)).cloned().collect();
        let mut next: HashMap<String, Vec<String>> = HashMap::new();
        for l in &self.links {
            if Flow::follows(&value, l) {
                next.entry(l.from.node.clone()).or_default().push(l.to.node.clone());
            }
        }
        // A block in a Try Zone goes on through the zone's Done and Error outputs.
        for (id, n) in &self.nodes {
            if let Some(zone) = &n.zone {
                next.entry(id.clone()).or_default().push(zone.clone());
            }
        }
        Flow { value, next }
    }

    /// Whether `link` would close a loop (see `Flow::would_loop`).
    pub fn would_loop(&self, link: &Link, value_block: impl Fn(&str) -> bool) -> bool {
        self.flow(value_block).would_loop(link)
    }

    /// The wires that are part of a loop.
    pub fn loops(&self, value_block: impl Fn(&str) -> bool) -> Vec<&Link> {
        let flow = self.flow(value_block);
        self.links.iter().filter(|l| flow.would_loop(l)).collect()
    }

    /// Puts block `id` into the action paths that use its outputs: right
    /// before the first block of each path that reads them (also through value
    /// blocks such as Compare in between). For blocks that were value blocks
    /// and run as action blocks now. A second path gets a copy of the block.
    /// Returns how many paths it went into; 0 when it is in one already or
    /// nothing in a path reads it.
    pub fn put_in_path(&mut self, id: &str, value_block: impl Fn(&str) -> bool) -> usize {
        if !self.nodes.contains_key(id) || self.links_into(id, ACTION_IN).next().is_some() {
            return 0;
        }
        // The action blocks a wire from `id` ends up in, through value blocks.
        let readers_of = |start: &str| -> Vec<String> {
            let mut out = Vec::new();
            let mut seen = HashSet::new();
            // In wiring order, so the first path stays the original's.
            let mut todo = std::collections::VecDeque::from([start.to_string()]);
            while let Some(at) = todo.pop_front() {
                if !seen.insert(at.clone()) {
                    continue;
                }
                if at != id && !value_block(&at) {
                    out.push(at);
                    continue;
                }
                todo.extend(self.links.iter().filter(|l| l.from.node == at && l.to.pin != ACTION_IN).map(|l| l.to.node.clone()));
            }
            out
        };
        // Only action wires (and Try Zones) count for what runs after what.
        let flow = self.flow(|_| false);
        let mut readers: Vec<String> = Vec::new();
        for r in readers_of(id) {
            if !readers.contains(&r) && self.links_into(&r, ACTION_IN).next().is_some() {
                readers.push(r);
            }
        }
        // The first reader of each path: one no other reader runs before.
        let firsts: Vec<String> = readers.iter().filter(|c| !readers.iter().any(|o| o != *c && flow.reaches(o, c))).cloned().collect();
        if firsts.is_empty() {
            return 0;
        }
        // Which path each of its wires serves, for the copies.
        let owner_of = |link: &Link| -> usize {
            let rs = if value_block(&link.to.node) { readers_of(&link.to.node) } else { vec![link.to.node.clone()] };
            firsts.iter().position(|f| rs.iter().any(|r| r == f || flow.reaches(f, r))).unwrap_or(0)
        };
        let owners: Vec<(usize, usize)> = self.links.iter().enumerate().filter(|(_, l)| l.from.node == id && l.to.pin != ACTION_IN).map(|(i, l)| (i, owner_of(l))).collect();
        for (n, first) in firsts.iter().enumerate() {
            let at = if n == 0 {
                id.to_string()
            } else {
                let copy_id = self.next_node_id();
                let mut copy = self.nodes[id].clone();
                copy.at = [copy.at[0] + 40.0 * n as f32, copy.at[1] + 40.0 * n as f32];
                self.nodes.insert(copy_id.clone(), copy);
                for &(i, _) in owners.iter().filter(|(_, o)| *o == n) {
                    self.links[i].from.node = copy_id.clone();
                }
                copy_id
            };
            for l in self.links.iter_mut().filter(|l| l.to.node == *first && l.to.pin == ACTION_IN) {
                l.to.node = at.clone();
            }
            self.links.push(Link { from: PinRef::new(&at, "next"), to: PinRef::new(first, ACTION_IN) });
            let zone = self.nodes.get(first).and_then(|f| f.zone.clone());
            if let Some(node) = self.nodes.get_mut(&at) {
                node.zone = zone;
            }
        }
        firsts.len()
    }

    /// Action outputs that lead to more than one block (made before paths
    /// couldn't split), with how many.
    pub fn split_paths(&self) -> Vec<(PinRef, usize)> {
        let mut out: Vec<(PinRef, usize)> = Vec::new();
        for l in self.links.iter().filter(|l| l.to.pin == ACTION_IN) {
            match out.iter_mut().find(|(p, _)| *p == l.from) {
                Some((_, n)) => *n += 1,
                None => out.push((l.from.clone(), 1)),
            }
        }
        out.retain(|(_, n)| *n > 1);
        out
    }
}

fn round_point<S: Serializer>(p: &[f32; 2], s: S) -> Result<S::Ok, S::Error> {
    [p[0].round() as i64, p[1].round() as i64].serialize(s)
}

fn round_width<S: Serializer>(w: &Option<f32>, s: S) -> Result<S::Ok, S::Error> {
    w.map(|w| w.round() as i64).serialize(s)
}

fn round_rect<S: Serializer>(r: &[f32; 4], s: S) -> Result<S::Ok, S::Error> {
    r.map(|v| v.round() as i64).serialize(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn info_blocks_go_into_the_paths_that_read_them() {
        let link = |s: &str| s.parse::<Link>().unwrap();
        let ws_with = |nodes: &[&str], links: &[&str]| {
            let mut ws = Workspace::new("t");
            for id in nodes {
                ws.nodes.insert(id.to_string(), Node { block: format!("{id}@1"), at: [10.0, 20.0], ..Default::default() });
            }
            ws.links = links.iter().map(|l| link(l)).collect();
            ws
        };
        let has = |ws: &Workspace, l: &str| ws.links.contains(&link(l));
        let value = |id: &str| id.starts_with("cmp");

        // Before the block that reads it.
        let mut ws = ws_with(&["ev", "send", "info"], &["ev.next > send.in", "info.ping > send.ping"]);
        assert_eq!(ws.put_in_path("info", value), 1);
        assert!(has(&ws, "ev.next > info.in") && has(&ws, "info.next > send.in") && !has(&ws, "ev.next > send.in"));
        assert_eq!(ws.put_in_path("info", value), 0, "in the path already");

        // Through a value block, and only before the first reader of a path.
        let mut ws = ws_with(
            &["ev", "check", "send", "info", "cmp"],
            &["ev.next > check.in", "check.true > send.in", "info.servers > cmp.a", "cmp.result > check.a", "info.ping > send.ping"],
        );
        assert_eq!(ws.put_in_path("info", value), 1);
        assert!(has(&ws, "ev.next > info.in") && has(&ws, "info.next > check.in") && has(&ws, "check.true > send.in"));

        // Two paths: a copy for the second, reading its own wires.
        let mut ws = ws_with(&["a", "sendA", "b", "sendB", "info"], &["a.next > sendA.in", "b.next > sendB.in", "info.ping > sendA.x", "info.ping > sendB.y"]);
        assert_eq!(ws.put_in_path("info", value), 2);
        let copy = ws.nodes.keys().find(|k| k.parse::<u64>().is_ok()).cloned().unwrap();
        assert!(has(&ws, "a.next > info.in") && has(&ws, "info.next > sendA.in") && has(&ws, "info.ping > sendA.x"));
        assert!(has(&ws, &format!("b.next > {copy}.in")) && has(&ws, &format!("{copy}.next > sendB.in")) && has(&ws, &format!("{copy}.ping > sendB.y")));
        assert_eq!(ws.nodes[&copy].at, [50.0, 60.0]);

        // Into the reader's Try Zone; nothing to do without a reader in a path.
        let mut ws = ws_with(&["ev", "send", "info", "lonely"], &["ev.next > send.in", "info.ping > send.ping", "info.ping > lonely.x"]);
        ws.nodes["send"].zone = Some("z".into());
        assert_eq!(ws.put_in_path("info", value), 1);
        assert_eq!(ws.nodes["info"].zone.as_deref(), Some("z"));
        let mut ws = ws_with(&["info", "lonely"], &["info.ping > lonely.x"]);
        assert_eq!(ws.put_in_path("info", value), 0);
    }

    #[test]
    fn finds_loops() {
        let mut ws = Workspace::new("t");
        for id in ["start", "a", "b", "math1", "math2", "send"] {
            ws.nodes.insert(id.into(), Node { block: format!("{id}@1"), ..Default::default() });
        }
        ws.nodes["b"].zone = Some("z".into());
        let value = |id: &str| id.starts_with("math");
        let link = |s: &str| s.parse::<Link>().unwrap();
        for l in ["start.next > a.in", "a.next > b.in", "math1.result > math2.a", "math2.result > send.text"] {
            ws.links.push(link(l));
        }
        assert!(ws.loops(value).is_empty());
        // Back to a block earlier on the path, or to itself.
        assert!(ws.would_loop(&link("b.next > a.in"), value));
        assert!(ws.would_loop(&link("a.next > a.in"), value));
        assert!(!ws.would_loop(&link("a.next > send.in"), value));
        // Out of the Try Zone around b and back into it.
        assert!(ws.would_loop(&link("z.error > b.in"), value));
        assert!(ws.would_loop(&link("z.error > a.in"), value));
        // Value blocks reading each other wait forever; reading an action block's
        // output (worked out when it ran) is fine.
        assert!(ws.would_loop(&link("math2.result > math1.b"), value));
        assert!(!ws.would_loop(&link("send.message > math1.b"), value));

        ws.links.push(link("b.next > a.in"));
        assert_eq!(ws.loops(value).iter().map(|l| l.to_string()).collect::<Vec<_>>(), ["a.next > b.in", "b.next > a.in"]);
    }

    #[test]
    fn link_round_trip() {
        let l: Link = "a1.next > b2.in".parse().unwrap();
        assert_eq!(l.from, PinRef::new("a1", "next"));
        assert_eq!(l.to.to_string_pair(), ("b2", "in"));
        assert_eq!(serde_json::to_string(&l).unwrap(), "\"a1.next > b2.in\"");
    }

    impl PinRef {
        fn to_string_pair(&self) -> (&str, &str) {
            (&self.node, &self.pin)
        }
    }

    #[test]
    fn copy_paste_remaps_ids() {
        let mut w: Workspace = serde_json::from_value(serde_json::json!({
            "name": "t",
            "nodes": { "a": { "block": "x@1", "at": [0, 0], "zone": "z1" }, "b": { "block": "y@1", "at": [10, 0] }, "c": { "block": "y@1", "at": [20, 0] } },
            "links": ["a.next > b.in", "b.next > c.in", "z1.done > b.in"],
            "zones": { "z1": { "title": "T", "rect": [0, 0, 1, 1] } }
        }))
        .unwrap();
        let clip = w.extract(&["a".into(), "b".into()]);
        assert_eq!(clip.links.len(), 2, "wires to blocks outside the selection are left out");
        let ids = w.paste(&clip, [100.0, 0.0]);
        assert_eq!(ids, ["1", "2"]);
        assert_eq!(w.nodes.len(), 5);
        assert_eq!(w.zones.len(), 2);
        assert_eq!(w.nodes[&ids[0]].at, [100.0, 0.0]);
        assert_eq!(w.nodes[&ids[0]].zone.as_deref(), Some("z2"));
        assert!(w.links.iter().any(|l| l.from.node == ids[0] && l.to.node == ids[1]));
    }

    #[test]
    fn block_ids_count_up() {
        let mut w = Workspace::new("t");
        assert_eq!(w.next_node_id(), "1");
        for id in ["1", "2", "7", "a1"] {
            w.nodes.insert(id.into(), Node::default());
        }
        assert_eq!(w.next_node_id(), "8", "after the highest number, ignoring old letter ids");
    }

    #[test]
    fn connect_replaces_single_input() {
        let mut w = Workspace::new("t");
        w.connect("a.x > c.v".parse().unwrap(), true);
        w.connect("b.x > c.v".parse().unwrap(), true);
        assert_eq!(w.links.len(), 1);
        assert_eq!(w.links[0].from.node, "b");
    }

    #[test]
    fn action_paths_never_split() {
        let mut w = Workspace::new("t");
        w.connect("a.next > b.in".parse().unwrap(), false);
        w.connect("a.next > c.in".parse().unwrap(), false);
        assert_eq!(w.links, vec!["a.next > c.in".parse::<Link>().unwrap()], "the new wire replaces the old one");
        // Paths may still join: two blocks can run the same next block.
        w.connect("b.next > c.in".parse().unwrap(), false);
        assert_eq!(w.links.len(), 2);
        assert!(w.split_paths().is_empty());
        w.links.push("a.next > d.in".parse().unwrap());
        assert_eq!(w.split_paths(), vec![(PinRef::new("a", "next"), 2)]);
    }
}
