//! Putting two versions of a JSON file together by what they hold: each
//! key of an object on its own, a list of texts (a workspace's wires) as a
//! set. Where a block sits and how big it is never stands in the way: this
//! computer's is kept.
//!
//! A workspace's blocks are matched by their fixed id (`uid`), not by their
//! number: two people who both added a "block 7" made two blocks, and the
//! one not stored yet gets another number, with its wires and the `${#7.…}`
//! that read it.

use std::collections::{HashMap, HashSet};

use serde_json::{Map, Value};

use crate::Side;

/// Keys that only say how something is laid out. Both changed: mine stays.
const LAYOUT: &[&str] = &["at", "rect", "width", "heights", "size"];
/// A workspace's parts that are asked about one by one, by their id.
const BY_ID: &[&str] = &["nodes", "notes", "zones"];

/// Two versions put together, and what is still to be decided.
#[derive(Debug, PartialEq)]
pub struct Outcome {
    pub value: Value,
    /// The parts both changed in different ways, with no side chosen yet:
    /// `nodes/7` for block 7, `name` for another key, `` for the whole file.
    /// For these, `value` holds mine.
    pub open: Vec<String>,
}

struct Ctx<'a> {
    keep: &'a dyn Fn(&str) -> Option<Side>,
    open: Vec<String>,
}

impl Ctx<'_> {
    /// Whose version of the part at `path` stays; mine while nobody said.
    fn side(&mut self, path: &[&str]) -> Side {
        let unit = unit_of(path);
        match (self.keep)(&unit) {
            Some(side) => side,
            None => {
                if !self.open.contains(&unit) {
                    self.open.push(unit);
                }
                Side::Mine
            }
        }
    }
}

/// The part of the file a path lies in, as it is asked about.
fn unit_of(path: &[&str]) -> String {
    match path {
        [first, id, ..] if BY_ID.contains(first) => format!("{first}/{id}"),
        [first, ..] => first.to_string(),
        [] => String::new(),
    }
}

/// `mine` and `theirs` put together, both made from `base` (`None`: the
/// file is new on both sides). `None` when both changed the same thing in
/// different ways.
pub fn merge(base: Option<&Value>, mine: &Value, theirs: &Value) -> Option<Value> {
    let out = merge_with(base, mine, theirs, &|_| None);
    out.open.is_empty().then_some(out.value)
}

/// As [`merge`], with `keep` saying whose version of a part stays where
/// both changed it.
pub fn merge_with(base: Option<&Value>, mine: &Value, theirs: &Value, keep: &dyn Fn(&str) -> Option<Side>) -> Outcome {
    let mut ctx = Ctx { keep, open: Vec::new() };
    let value = merge_at(&mut ctx, &mut Vec::new(), base, mine, theirs);
    Outcome { value, open: ctx.open }
}

fn merge_at<'a>(ctx: &mut Ctx, path: &mut Vec<&'a str>, base: Option<&Value>, mine: &'a Value, theirs: &'a Value) -> Value {
    if mine == theirs || base == Some(theirs) {
        return mine.clone();
    }
    if base == Some(mine) {
        return theirs.clone();
    }
    match (mine, theirs) {
        (Value::Object(a), Value::Object(b)) => {
            let empty = Map::new();
            let base = base.and_then(Value::as_object).unwrap_or(&empty);
            let mut out = Map::new();
            // In my order, then what only they have.
            for k in a.keys().chain(b.keys().filter(|k| !a.contains_key(*k))) {
                path.push(k);
                match (a.get(k), b.get(k), base.get(k)) {
                    (Some(x), Some(y), was) => drop(out.insert(k.clone(), merge_at(ctx, path, was, x, y))),
                    // New on one side.
                    (Some(x), None, None) | (None, Some(x), None) => drop(out.insert(k.clone(), x.clone())),
                    // Deleted on one side: gone, unless the other changed it.
                    (mine_has, theirs_has, Some(was)) => {
                        let x = mine_has.or(theirs_has).expect("one of them has it");
                        if x != was {
                            let stays = match ctx.side(path) {
                                Side::Mine => mine_has.is_some(),
                                Side::Theirs => theirs_has.is_some(),
                                Side::Both => true,
                            };
                            if stays {
                                out.insert(k.clone(), x.clone());
                            }
                        }
                    }
                    (None, None, None) => {}
                }
                path.pop();
            }
            Value::Object(out)
        }
        (Value::Array(a), Value::Array(b)) if texts(a) && texts(b) && base.is_none_or(|v| v.as_array().is_some_and(|l| texts(l))) => {
            let empty = Vec::new();
            let was = base.and_then(Value::as_array).unwrap_or(&empty);
            // Mine without what they took out, then what they added.
            let mut out: Vec<Value> = a.iter().filter(|x| b.contains(x) || !was.contains(x)).cloned().collect();
            out.extend(b.iter().filter(|x| !was.contains(x) && !a.contains(x)).cloned());
            Value::Array(out)
        }
        _ if path.last().is_some_and(|k| LAYOUT.contains(k)) => mine.clone(),
        _ => match ctx.side(path) {
            Side::Theirs => theirs.clone(),
            Side::Mine | Side::Both => mine.clone(),
        },
    }
}

fn texts(list: &[Value]) -> bool {
    list.iter().all(Value::is_string)
}

/// What a block is told apart by: its fixed id, or its number in a file
/// from before blocks had one.
fn ident(key: &str, node: &Value) -> String {
    match node["uid"].as_str().filter(|u| !u.is_empty()) {
        Some(uid) => format!("u:{uid}"),
        None => format!("k:{key}"),
    }
}

fn nodes_of(doc: &Value) -> impl Iterator<Item = (&String, &Value)> {
    doc["nodes"].as_object().into_iter().flatten()
}

/// The workspace with its blocks numbered as `to` says (old number → new):
/// the blocks, the wires' ends, and the `${#4.…}` in every block's values.
fn renumbered(doc: &Value, to: &HashMap<String, String>) -> Value {
    if to.iter().all(|(old, new)| old == new) {
        return doc.clone();
    }
    fn texts_in(v: &mut Value, to: &HashMap<String, String>) {
        match v {
            Value::String(s) => {
                if let Some(new) = circuit_core::template::rename_blocks(s, |id| to.get(id).cloned()) {
                    *s = new;
                }
            }
            Value::Array(a) => a.iter_mut().for_each(|v| texts_in(v, to)),
            Value::Object(o) => o.values_mut().for_each(|v| texts_in(v, to)),
            _ => {}
        }
    }
    let mut out = doc.clone();
    if let Some(nodes) = doc["nodes"].as_object() {
        let mut renamed = Map::new();
        for (key, node) in nodes {
            let mut node = node.clone();
            if let Some(values) = node.get_mut("values") {
                texts_in(values, to);
            }
            renamed.insert(to.get(key).cloned().unwrap_or_else(|| key.clone()), node);
        }
        out["nodes"] = Value::Object(renamed);
    }
    if let Some(links) = out.get_mut("links").and_then(Value::as_array_mut) {
        for link in links {
            let Some(text) = link.as_str() else { continue };
            let end = |end: &str| match end.split_once('.') {
                Some((id, pin)) => format!("{}.{pin}", to.get(id).map(String::as_str).unwrap_or(id)),
                None => end.to_string(),
            };
            if let Some((from, into)) = text.split_once(" > ") {
                *link = Value::String(format!("{} > {}", end(from), end(into)));
            }
        }
    }
    out
}

/// Two versions of a workspace put together. Blocks are matched by their
/// fixed id; where the same number means two blocks, the stored one keeps
/// it. `Side::Both` for a block keeps mine and adds theirs as a copy.
pub fn workspace(base: Option<&Value>, mine: &Value, theirs: &Value, keep: &dyn Fn(&str) -> Option<Side>) -> Outcome {
    // The number each block ends up with: the stored side's first, as that
    // is what everyone else already has.
    let mut number: HashMap<String, String> = HashMap::new();
    let mut taken: HashSet<String> = HashSet::new();
    let mut next = [Some(mine), Some(theirs), base].into_iter().flatten().flat_map(nodes_of).filter_map(|(k, _)| k.parse::<u64>().ok()).max().unwrap_or(0) + 1;
    for doc in [Some(theirs), Some(mine), base].into_iter().flatten() {
        for (key, node) in nodes_of(doc) {
            let who = ident(key, node);
            if number.contains_key(&who) {
                continue;
            }
            let new = if taken.contains(key) {
                next += 1;
                (next - 1).to_string()
            } else {
                key.clone()
            };
            taken.insert(new.clone());
            number.insert(who, new);
        }
    }
    let in_step = |doc: &Value| -> Value {
        let to: HashMap<String, String> = nodes_of(doc).map(|(key, node)| (key.clone(), number[&ident(key, node)].clone())).collect();
        renumbered(doc, &to)
    };
    let (a, b, was) = (in_step(mine), in_step(theirs), base.map(in_step));
    let mut out = merge_with(was.as_ref(), &a, &b, keep);
    // Both: mine stays where it is, theirs comes in beside it as a block of
    // its own (a new number, and a new fixed id when the file is next saved).
    let copies: Vec<(String, Value)> = nodes_of(&b)
        .filter(|(key, node)| keep(&format!("nodes/{key}")) == Some(Side::Both) && a["nodes"].get(key.as_str()).is_some_and(|m| m != *node))
        .map(|(key, node)| (key.clone(), node.clone()))
        .collect();
    for (_, mut node) in copies {
        if let Some(o) = node.as_object_mut() {
            o.remove("uid");
            if let Some([x, y]) = o.get("at").and_then(Value::as_array).map(Vec::as_slice) {
                let moved = serde_json::json!([x.as_f64().unwrap_or(0.0) + 40.0, y.as_f64().unwrap_or(0.0) + 40.0]);
                o.insert("at".into(), moved);
            }
        }
        if let Some(nodes) = out.value.get_mut("nodes").and_then(Value::as_object_mut) {
            nodes.insert(next.to_string(), node);
            next += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn puts_two_versions_together() {
        let base = json!({ "a": 1, "b": { "x": 1, "y": 2 }, "links": ["1 > 2", "2 > 3"], "gone": true });
        let mine = json!({ "a": 2, "b": { "x": 1, "y": 2, "z": 3 }, "links": ["1 > 2", "2 > 3", "3 > 4"], "gone": true });
        let theirs = json!({ "a": 1, "b": { "x": 9, "y": 2 }, "links": ["2 > 3", "5 > 6"], "new": [1, 2] });
        assert_eq!(merge(Some(&base), &mine, &theirs), Some(json!({ "a": 2, "b": { "x": 9, "y": 2, "z": 3 }, "links": ["2 > 3", "3 > 4", "5 > 6"], "new": [1, 2] })));
        // The same thing two ways.
        assert_eq!(merge(Some(&json!({ "a": 1 })), &json!({ "a": 2 }), &json!({ "a": 3 })), None);
        assert_eq!(merge(Some(&json!({ "a": 1 })), &json!({ "a": 2 }), &json!({ "a": 2 })), Some(json!({ "a": 2 })));
        // Deleted by one, changed by the other.
        assert_eq!(merge(Some(&json!({ "n": { "v": 1 } })), &json!({}), &json!({ "n": { "v": 2 } })), None);
        assert_eq!(merge(Some(&json!({ "n": { "v": 1 } })), &json!({}), &json!({ "n": { "v": 1 }, "m": 1 })), Some(json!({ "m": 1 })));
        // Where things sit: mine.
        assert_eq!(merge(Some(&json!({ "at": [0, 0], "rect": [0, 0, 1, 1] })), &json!({ "at": [1, 1], "rect": [0, 0, 2, 2] }), &json!({ "at": [5, 5], "rect": [0, 0, 1, 1] })), Some(json!({ "at": [1, 1], "rect": [0, 0, 2, 2] })));
        // Lists of other things are one piece.
        assert_eq!(merge(Some(&json!({ "l": [1] })), &json!({ "l": [1, 2] }), &json!({ "l": [1, 3] })), None);
        // New on both sides, without a version before.
        assert_eq!(merge(None, &json!({ "a": 1 }), &json!({ "b": 2 })), Some(json!({ "a": 1, "b": 2 })));
        assert_eq!(merge(None, &json!({ "a": 1 }), &json!({ "a": 2 })), None);
    }

    #[test]
    fn asks_part_by_part() {
        let base = json!({ "name": "W", "nodes": { "1": { "v": 1 }, "2": { "v": 1 }, "3": { "v": 1 } } });
        let mine = json!({ "name": "Mine", "nodes": { "1": { "v": 2 }, "2": { "v": 2 }, "3": { "v": 1, "w": 1 } } });
        let theirs = json!({ "name": "Theirs", "nodes": { "1": { "v": 3 }, "3": { "v": 1, "x": 1 } } });
        let asked = merge_with(Some(&base), &mine, &theirs, &|_| None);
        assert_eq!(asked.open, ["name", "nodes/1", "nodes/2"], "block 3 went together, block 2 was deleted there");
        assert_eq!(asked.value["nodes"]["1"]["v"], 2, "mine while nobody said");
        let keep = |unit: &str| match unit {
            "name" => Some(Side::Theirs),
            "nodes/1" => Some(Side::Mine),
            "nodes/2" => Some(Side::Theirs),
            _ => None,
        };
        let done = merge_with(Some(&base), &mine, &theirs, &keep);
        assert!(done.open.is_empty());
        assert_eq!(done.value, json!({ "name": "Theirs", "nodes": { "1": { "v": 2 }, "3": { "v": 1, "w": 1, "x": 1 } } }));
    }

    #[test]
    fn two_blocks_with_one_number_both_stay() {
        let node = |uid: &str, text: &str| json!({ "block": "log@1", "uid": uid, "at": [0, 0], "values": { "message": text } });
        let base = json!({ "nodes": { "1": node("a", "one") }, "links": [] });
        // Both add a block 2. Mine reads its own block 2 from block 3.
        let mine = json!({ "nodes": { "1": node("a", "one"), "2": node("m", "mine"), "3": node("n", "after ${#2.result} and ${#1.x}") }, "links": ["1.next > 2.in", "2.next > 3.in"] });
        let theirs = json!({ "nodes": { "1": node("a", "one"), "2": node("t", "theirs") }, "links": ["1.next > 2.in"] });
        let out = workspace(Some(&base), &mine, &theirs, &|_| None);
        assert!(out.open.is_empty(), "{:?}", out.open);
        let nodes = out.value["nodes"].as_object().unwrap();
        assert_eq!(nodes["2"]["uid"], "t", "the stored block keeps the number");
        assert_eq!((nodes["4"]["uid"].as_str(), nodes["3"]["uid"].as_str()), (Some("m"), Some("n")), "mine got the next free one: {nodes:?}");
        assert_eq!(nodes["3"]["values"]["message"], "after ${#4.result} and ${#1.x}", "and what read it follows");
        let links: Vec<&str> = out.value["links"].as_array().unwrap().iter().filter_map(Value::as_str).collect();
        assert_eq!(links, ["1.next > 4.in", "4.next > 3.in", "1.next > 2.in"]);

        // Later the other side has it renumbered while I changed my block:
        // one block, by its id, under the stored number.
        let stored = out.value.clone();
        let before = mine.clone();
        let mut mine = mine.clone();
        mine["nodes"]["2"]["values"]["message"] = "mine, changed".into();
        let again = workspace(Some(&before), &mine, &stored, &|_| None);
        assert!(again.open.is_empty());
        assert_eq!(again.value["nodes"]["4"]["values"]["message"], "mine, changed");
        assert_eq!(again.value["nodes"]["2"]["uid"], "t");

        // The same block changed two ways: asked about by its number, and
        // Both keeps mine and adds theirs beside it.
        let base = json!({ "nodes": { "1": node("a", "one") } });
        let (mine, theirs) = (json!({ "nodes": { "1": node("a", "mine") } }), json!({ "nodes": { "1": node("a", "theirs") } }));
        assert_eq!(workspace(Some(&base), &mine, &theirs, &|_| None).open, ["nodes/1"]);
        let both = workspace(Some(&base), &mine, &theirs, &|_| Some(Side::Both));
        let nodes = both.value["nodes"].as_object().unwrap();
        assert!(both.open.is_empty() && nodes.len() == 2);
        assert_eq!((nodes["1"]["values"]["message"].as_str(), nodes["2"]["values"]["message"].as_str()), (Some("mine"), Some("theirs")));
        assert!(nodes["2"].get("uid").is_none() && nodes["2"]["at"] == json!([40.0, 40.0]));
    }
}
