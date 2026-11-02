//! Block definitions as loaded from block files, and the effective shape of
//! one placed block after `show` conditions and `define()` are applied.

use std::collections::HashSet;
use std::path::PathBuf;

use indexmap::IndexMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};

use crate::types::PinType;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BlockKind {
    Event,
    #[default]
    Action,
    Value,
}

/// Whether a field is typed inline, connected, or either.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PinMode {
    /// Inline, with a toggle to turn it into a pin.
    #[default]
    Auto,
    /// Starts as a pin.
    Pin,
    /// Inline only.
    Inline,
    /// Must be connected.
    Only,
}

impl<'de> Deserialize<'de> for PinMode {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Bool(bool),
            Str(String),
        }
        Ok(match Raw::deserialize(d)? {
            Raw::Bool(true) => PinMode::Pin,
            Raw::Bool(false) => PinMode::Inline,
            Raw::Str(s) if s == "only" => PinMode::Only,
            Raw::Str(_) => PinMode::Auto,
        })
    }
}

impl Serialize for PinMode {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            PinMode::Auto => s.serialize_str("auto"),
            PinMode::Pin => s.serialize_bool(true),
            PinMode::Inline => s.serialize_bool(false),
            PinMode::Only => s.serialize_str("only"),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct FieldDef {
    #[serde(rename = "type")]
    pub type_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<Value>,
    pub required: bool,
    pub pin: PinMode,
    pub multi: bool,
    pub advanced: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub choices: Option<Value>,
    /// Whether `{name}` placeholders in the text are filled in. Unset: yes for
    /// plain text fields (see `templated`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    #[serde(skip)]
    pub ty: PinType,
}

impl FieldDef {
    pub fn finish(&mut self) {
        self.ty = PinType::parse(&self.type_name);
    }

    pub fn label(&self, key: &str) -> String {
        self.label.clone().unwrap_or_else(|| title_case(key))
    }

    /// Whether `{name}` placeholders in this field's text are filled in, each
    /// with an input of its own: plain text fields, unless the block sets
    /// `template: false`.
    pub fn templated(&self) -> bool {
        self.template.unwrap_or_else(|| matches!(self.type_name.trim_end_matches('?'), "text" | "textarea" | "string") && self.choices.is_none())
    }

    /// Choices of a `select` field as `(group, id, label)`.
    pub fn choice_list(&self) -> Vec<(Option<String>, String, String)> {
        let mut out = Vec::new();
        let Some(Value::Object(map)) = &self.choices else {
            return out;
        };
        for (k, v) in map {
            match v {
                Value::Object(group) => {
                    for (id, label) in group {
                        out.push((Some(k.clone()), id.clone(), value_label(label)));
                    }
                }
                other => out.push((None, k.clone(), value_label(other))),
            }
        }
        out
    }

    pub fn is_select(&self) -> bool {
        self.type_name == "select" || self.choices.is_some()
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct OutputDef {
    #[serde(rename = "type")]
    pub type_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip)]
    pub ty: PinType,
}

impl OutputDef {
    pub fn finish(&mut self) {
        self.ty = PinType::parse(&self.type_name);
    }

    pub fn label(&self, key: &str) -> String {
        self.label.clone().unwrap_or_else(|| title_case(key))
    }
}

/// A block as described by its block file. Function keys arrive as `has_*` flags.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct BlockDef {
    pub id: String,
    pub name: String,
    pub description: String,
    pub author: String,
    pub version: String,
    pub category: Option<String>,
    pub icon: Option<String>,
    pub kind: BlockKind,
    pub tags: Vec<String>,
    pub docs: Option<String>,
    pub fields: IndexMap<String, FieldDef>,
    pub outputs: IndexMap<String, OutputDef>,
    pub flows: Option<Vec<String>>,
    pub intents: Vec<String>,
    pub dependencies: IndexMap<String, String>,
    pub width: Option<f32>,
    pub preview: Option<String>,
    pub deprecated: Option<Value>,
    /// Value blocks are cached per run unless they set `pure: false`.
    pub pure: Option<bool>,

    pub has_define: bool,
    pub has_run: bool,
    pub has_listen: bool,
    pub has_setup: bool,
    pub has_summary: bool,
    pub has_migrate: bool,

    /// Id of the plugin that contributed this block.
    #[serde(skip)]
    pub plugin: String,
    /// The block file on disk.
    #[serde(skip)]
    pub source: PathBuf,
    /// JavaScript made from a TypeScript file (or one with `import` lines); runs instead of `source`.
    #[serde(skip)]
    pub compiled: Option<String>,
}

impl BlockDef {
    /// Parses type strings and returns problems with required keys.
    pub fn finish(&mut self) -> Vec<String> {
        for f in self.fields.values_mut() {
            f.finish();
        }
        for o in self.outputs.values_mut() {
            o.finish();
        }
        let mut problems = Vec::new();
        for (key, value) in [
            ("id", &self.id),
            ("name", &self.name),
            ("description", &self.description),
            ("author", &self.author),
            ("version", &self.version),
        ] {
            if value.trim().is_empty() {
                problems.push(format!("missing `{key}`"));
            }
        }
        match self.kind {
            BlockKind::Event if !self.has_listen => problems.push("event block without `listen()`".into()),
            BlockKind::Action | BlockKind::Value if !self.has_run => problems.push("block without `run()`".into()),
            _ => {}
        }
        problems
    }

    pub fn major_version(&self) -> u64 {
        self.version.split('.').next().and_then(|m| m.parse().ok()).unwrap_or(0)
    }

    /// The reference stored in projects: `id@major`.
    pub fn reference(&self) -> String {
        format!("{}@{}", self.id, self.major_version())
    }

    /// Default inline values for a newly placed block.
    pub fn default_values(&self) -> Map<String, Value> {
        self.fields
            .iter()
            .filter_map(|(k, f)| f.default.clone().map(|v| (k.clone(), v)))
            .collect()
    }

    /// Keys of fields that start out as pins.
    pub fn default_pins(&self) -> Vec<String> {
        self.fields
            .iter()
            .filter(|(_, f)| matches!(f.pin, PinMode::Pin | PinMode::Only))
            .map(|(k, _)| k.clone())
            .collect()
    }

    /// Action outputs before `define()`.
    pub fn base_flows(&self) -> Vec<String> {
        match self.kind {
            BlockKind::Value => Vec::new(),
            _ => self.flows.clone().unwrap_or_else(|| vec!["next".into()]),
        }
    }
}

/// What a block's `define()` returned.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct DefineResult {
    pub add: IndexMap<String, FieldDef>,
    pub hide: Vec<String>,
    pub set: IndexMap<String, Value>,
    pub outputs: Option<IndexMap<String, OutputDef>>,
    pub flows: Option<Vec<String>>,
}

/// The effective look of one placed block.
#[derive(Clone, Debug, Default)]
pub struct BlockShape {
    pub fields: Vec<(String, FieldDef)>,
    pub hidden: HashSet<String>,
    pub outputs: Vec<(String, OutputDef)>,
    pub flows: Vec<String>,
    pub action_in: bool,
}

impl BlockShape {
    pub fn build(def: &BlockDef, values: &Map<String, Value>, define: Option<&DefineResult>) -> BlockShape {
        let mut fields: IndexMap<String, FieldDef> = def.fields.clone();
        let mut outputs = def.outputs.clone();
        let mut flows = def.base_flows();
        let mut hidden = HashSet::new();

        if let Some(d) = define {
            for (k, f) in &d.add {
                let mut f = f.clone();
                f.finish();
                fields.insert(k.clone(), f);
            }
            for (k, patch) in &d.set {
                if let Some(field) = fields.get_mut(k) {
                    *field = merge_field(field, patch);
                }
            }
            if let Some(outs) = &d.outputs {
                for (k, o) in outs {
                    let mut o = o.clone();
                    o.finish();
                    outputs.insert(k.clone(), o);
                }
            }
            if let Some(f) = &d.flows {
                flows = f.clone();
            }
            hidden.extend(d.hide.iter().cloned());
        }

        for (k, f) in &fields {
            if let Some(cond) = &f.show
                && !show_matches(cond, values, &fields)
            {
                hidden.insert(k.clone());
            }
        }

        // Each {name} typed into a text field gets an input right below it, to
        // connect the value that goes there. Unconnected, the name is looked up
        // in the event, variables and earlier outputs as before.
        let mut extra: Vec<(usize, String, FieldDef)> = Vec::new();
        for (i, (k, f)) in fields.iter().enumerate() {
            if hidden.contains(k) || !f.templated() || f.pin == PinMode::Only {
                continue;
            }
            let text = values.get(k).or(f.default.as_ref()).and_then(Value::as_str).unwrap_or_default();
            for name in placeholders(text) {
                if fields.contains_key(&name) || extra.iter().any(|(_, n, _)| *n == name) {
                    continue;
                }
                let mut input = FieldDef {
                    type_name: "any".into(),
                    label: Some(format!("{{{name}}}")),
                    description: Some(format!("Goes where {{{name}}} is in {}.", f.label(k))),
                    pin: PinMode::Only,
                    ..Default::default()
                };
                input.finish();
                extra.push((i, name, input));
            }
        }
        if !extra.is_empty() {
            let mut placed = IndexMap::with_capacity(fields.len() + extra.len());
            for (i, (k, f)) in fields.into_iter().enumerate() {
                placed.insert(k, f);
                for (_, name, input) in extra.iter().filter(|(at, ..)| *at == i) {
                    placed.insert(name.clone(), input.clone());
                }
            }
            fields = placed;
        }

        BlockShape {
            fields: fields.into_iter().collect(),
            hidden,
            outputs: outputs.into_iter().collect(),
            flows,
            action_in: def.kind == BlockKind::Action,
        }
    }

    pub fn field(&self, key: &str) -> Option<&FieldDef> {
        self.fields.iter().find(|(k, _)| k == key).map(|(_, f)| f)
    }

    pub fn output(&self, key: &str) -> Option<&OutputDef> {
        self.outputs.iter().find(|(k, _)| k == key).map(|(_, o)| o)
    }

    pub fn visible_fields(&self) -> impl Iterator<Item = (&String, &FieldDef)> {
        self.fields.iter().filter(|(k, _)| !self.hidden.contains(k)).map(|(k, f)| (k, f))
    }
}

/// Applies a `define()` `set` patch to a field.
fn merge_field(field: &FieldDef, patch: &Value) -> FieldDef {
    let mut base = serde_json::to_value(field).unwrap_or(Value::Null);
    if let (Value::Object(base_map), Value::Object(patch_map)) = (&mut base, patch) {
        for (k, v) in patch_map {
            base_map.insert(k.clone(), v.clone());
        }
    }
    let mut merged: FieldDef = serde_json::from_value(base).unwrap_or_else(|_| field.clone());
    merged.finish();
    merged
}

/// Evaluates `show: { mode: ["edit", "reply"], private: true }`.
/// The names of the `{name}` placeholders in a text, in order and once each.
/// `{user.name}` gives `user`; `{3}` and other non-names are left alone.
pub fn placeholders(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find('{') {
        rest = &rest[open + 1..];
        let Some(close) = rest.find('}') else { break };
        let inner = &rest[..close];
        let head = inner.split('.').next().unwrap_or_default();
        let is_name = head.chars().next().is_some_and(|c| c.is_alphabetic() || c == '_') && inner.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '.' || c == '$');
        if is_name && !out.iter().any(|n| n == head) {
            out.push(head.to_string());
        }
        rest = &rest[close + 1..];
    }
    out
}

fn show_matches(cond: &Value, values: &Map<String, Value>, fields: &IndexMap<String, FieldDef>) -> bool {
    let Value::Object(cond) = cond else {
        return true;
    };
    cond.iter().all(|(key, expected)| {
        let actual = values
            .get(key)
            .cloned()
            .or_else(|| fields.get(key).and_then(|f| f.default.clone()))
            .unwrap_or(Value::Null);
        match expected {
            Value::Array(options) => options.contains(&actual),
            other => *other == actual,
        }
    })
}

fn value_label(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Object(o) => o.get("name").or_else(|| o.get("label")).map(value_label).unwrap_or_default(),
        other => other.to_string(),
    }
}

pub fn title_case(key: &str) -> String {
    key.split(['_', '-'])
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(first) => first.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {

    #[test]
    fn finds_placeholders() {
        assert_eq!(placeholders("Hi {user}, you have {xp} XP ({user.name})"), vec!["user", "xp"]);
        assert!(placeholders("\\d{3} {a,b} {} { x }").is_empty(), "regex counts and other braces aren't names");
        assert_eq!(placeholders("{_x}{y2}"), vec!["_x", "y2"]);
    }

    #[test]
    fn text_fields_get_an_input_per_placeholder() {
        let def: BlockDef = serde_json::from_value(serde_json::json!({
            "id": "t.send", "name": "Send", "kind": "action",
            "fields": {
                "text": { "type": "textarea" },
                "code": { "type": "text", "template": false },
                "mode": { "type": "select", "choices": { "a": "A" } },
                "after": { "type": "number" }
            }
        }))
        .unwrap();
        let mut def = def;
        def.finish();
        let values = serde_json::json!({ "text": "Hi {name}, {name} has {count} {after}", "code": "{skip}" });
        let shape = BlockShape::build(&def, values.as_object().unwrap(), None);
        let keys: Vec<&str> = shape.fields.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(keys, vec!["text", "name", "count", "code", "mode", "after"], "inputs right after their text, once each; existing fields aren't doubled");
        let name = shape.field("name").unwrap();
        assert_eq!(name.pin, PinMode::Only);
        assert_eq!(name.label("name"), "{name}");
        assert!(!shape.field("code").unwrap().templated() && !shape.field("mode").unwrap().templated());
    }

    use super::*;
    use serde_json::json;

    fn def() -> BlockDef {
        let mut d: BlockDef = serde_json::from_value(json!({
            "id": "t.send", "name": "Send", "description": "d", "author": "a", "version": "3.1.0",
            "kind": "action", "has_run": true,
            "fields": {
                "mode": { "type": "select", "default": "reply", "choices": { "Interaction": { "reply": "Reply" }, "send": "Send" } },
                "private": { "type": "boolean", "show": { "mode": ["reply"] } },
                "target": { "type": "discord.channel", "pin": "only" }
            }
        }))
        .unwrap();
        assert!(d.finish().is_empty());
        d
    }

    #[test]
    fn show_conditions_and_define() {
        let d = def();
        assert_eq!(d.reference(), "t.send@3");
        assert_eq!(d.field_choice_count(), 2);
        let mut values = d.default_values();
        let shape = BlockShape::build(&d, &values, None);
        assert!(!shape.hidden.contains("private"));
        values.insert("mode".into(), json!("send"));
        let shape = BlockShape::build(&d, &values, None);
        assert!(shape.hidden.contains("private"));

        let define: DefineResult = serde_json::from_value(json!({
            "add": { "extra": { "type": "number" } },
            "set": { "target": { "required": true } },
            "flows": ["each", "done"]
        }))
        .unwrap();
        let shape = BlockShape::build(&d, &values, Some(&define));
        assert_eq!(shape.field("extra").unwrap().ty, PinType::Number);
        assert!(shape.field("target").unwrap().required);
        assert_eq!(shape.field("target").unwrap().pin, PinMode::Only);
        assert_eq!(shape.flows, vec!["each", "done"]);
    }

    impl BlockDef {
        fn field_choice_count(&self) -> usize {
            self.fields["mode"].choice_list().len()
        }
    }
}
