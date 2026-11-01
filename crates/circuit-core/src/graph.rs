//! A workspace graph: placed blocks, the links between their pins, Try Zones
//! and comment boxes.

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
            let id = (1..).map(|i| format!("n{i}")).find(|n| !self.nodes.contains_key(n) && !self.zones.contains_key(n)).unwrap();
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

    /// A short id not used in this workspace yet: `n1`, `n2`, …
    pub fn next_node_id(&self) -> String {
        (1..)
            .map(|i| format!("n{i}"))
            .find(|id| !self.nodes.contains_key(id))
            .expect("unbounded range")
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
    pub fn connect(&mut self, link: Link, replace_existing_input: bool) {
        if self.links.contains(&link) {
            return;
        }
        if replace_existing_input {
            self.links.retain(|l| l.to != link.to);
        }
        self.links.push(link);
    }
}

fn round_point<S: Serializer>(p: &[f32; 2], s: S) -> Result<S::Ok, S::Error> {
    [p[0].round() as i64, p[1].round() as i64].serialize(s)
}

fn round_rect<S: Serializer>(r: &[f32; 4], s: S) -> Result<S::Ok, S::Error> {
    r.map(|v| v.round() as i64).serialize(s)
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(ids.len(), 2);
        assert_eq!(w.nodes.len(), 5);
        assert_eq!(w.zones.len(), 2);
        assert_eq!(w.nodes[&ids[0]].at, [100.0, 0.0]);
        assert_eq!(w.nodes[&ids[0]].zone.as_deref(), Some("z2"));
        assert!(w.links.iter().any(|l| l.from.node == ids[0] && l.to.node == ids[1]));
    }

    #[test]
    fn connect_replaces_single_input() {
        let mut w = Workspace::new("t");
        w.connect("a.x > c.v".parse().unwrap(), true);
        w.connect("b.x > c.v".parse().unwrap(), true);
        assert_eq!(w.links.len(), 1);
        assert_eq!(w.links[0].from.node, "b");
    }
}
