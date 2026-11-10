//! Putting two versions of a JSON file together by what they hold: each
//! key of an object on its own, a list of texts (a workspace's wires) as a
//! set. Where a block sits and how big it is never stands in the way: this
//! computer's is kept.

use serde_json::{Map, Value};

/// Keys that only say how something is laid out. Both changed: mine stays.
const LAYOUT: &[&str] = &["at", "rect", "width", "heights", "size"];

/// `mine` and `theirs` put together, both made from `base` (`None`: the
/// file is new on both sides). `None` when both changed the same thing in
/// different ways.
pub fn merge(base: Option<&Value>, mine: &Value, theirs: &Value) -> Option<Value> {
    merge_at("", base, mine, theirs)
}

fn merge_at(key: &str, base: Option<&Value>, mine: &Value, theirs: &Value) -> Option<Value> {
    if mine == theirs || base == Some(theirs) {
        return Some(mine.clone());
    }
    if base == Some(mine) {
        return Some(theirs.clone());
    }
    match (mine, theirs) {
        (Value::Object(a), Value::Object(b)) => {
            let empty = Map::new();
            let base = base.and_then(Value::as_object).unwrap_or(&empty);
            let mut out = Map::new();
            // In my order, then what only they have.
            for k in a.keys().chain(b.keys().filter(|k| !a.contains_key(*k))) {
                match (a.get(k), b.get(k), base.get(k)) {
                    (Some(x), Some(y), was) => drop(out.insert(k.clone(), merge_at(k, was, x, y)?)),
                    // New on one side.
                    (Some(x), None, None) | (None, Some(x), None) => drop(out.insert(k.clone(), x.clone())),
                    // Deleted on one side: gone, unless the other changed it.
                    (Some(x), None, Some(was)) | (None, Some(x), Some(was)) => {
                        if x != was {
                            return None;
                        }
                    }
                    (None, None, _) => {}
                }
            }
            Some(Value::Object(out))
        }
        (Value::Array(a), Value::Array(b)) if texts(a) && texts(b) && base.is_none_or(|v| v.as_array().is_some_and(|l| texts(l))) => {
            let empty = Vec::new();
            let was = base.and_then(Value::as_array).unwrap_or(&empty);
            // Mine without what they took out, then what they added.
            let mut out: Vec<Value> = a.iter().filter(|x| b.contains(x) || !was.contains(x)).cloned().collect();
            out.extend(b.iter().filter(|x| !was.contains(x) && !a.contains(x)).cloned());
            Some(Value::Array(out))
        }
        _ if LAYOUT.contains(&key) => Some(mine.clone()),
        _ => None,
    }
}

fn texts(list: &[Value]) -> bool {
    list.iter().all(Value::is_string)
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
}
