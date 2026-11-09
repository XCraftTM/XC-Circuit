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
    /// Whether `${…}` in the text is filled in. Unset: yes for plain text
    /// fields (see `templated`).
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
    /// A select whose list ends in Custom…, which shows a text box for any
    /// other value. The value stays one text, so it can be connected too.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub custom: bool,
    /// A custom id field of an event (Button Pressed): offers the ids of this
    /// kind ("button", "select", "form") the project's layouts and blocks use.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggest: Option<String>,
    /// A field that gives a button, menu or form its custom id ("button",
    /// "select", "form"), for those suggestions and the checks in Problems.
    #[serde(rename = "customIdOf", skip_serializing_if = "Option::is_none")]
    pub custom_id_of: Option<String>,
    /// A text field for an id or name that can be picked from the bot's
    /// servers: "channel", "voice", "category" or "role". Channel id fields
    /// get it from their pin type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pick: Option<String>,
    #[serde(skip)]
    pub ty: PinType,
    /// Its pin type can be looked up from an id (`fromId`): instead of
    /// being wired, it can be switched to a box for typing the id. Set by
    /// the registry.
    #[serde(skip)]
    pub by_id: bool,
}

impl FieldDef {
    pub fn finish(&mut self) {
        self.ty = PinType::parse(&self.type_name);
    }

    pub fn label(&self, key: &str) -> String {
        self.label.clone().unwrap_or_else(|| title_case(key))
    }

    /// Whether `${…}` in this field's text is filled in, each name it uses
    /// with an input of its own: plain text fields and fields for any value
    /// (typed in as text), and colours (`${…}` for one from the bot), unless
    /// the block sets `template: false`.
    pub fn templated(&self) -> bool {
        self.template.unwrap_or_else(|| matches!(self.type_name.trim_end_matches('?'), "text" | "textarea" | "string" | "any" | "color") && self.choices.is_none())
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
        self.type_name == "select" || (self.choices.is_some() && !self.is_multiselect())
    }

    /// Several of its choices can be picked: its value is a list of their keys.
    pub fn is_multiselect(&self) -> bool {
        self.type_name == "multiselect"
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

/// How a block file groups what describes it. A key may also stand on its
/// own at the top, as block files did before the groups.
pub const BLOCK_GROUPS: [(&str, &[&str]); 4] = [
    ("info", &["name", "description", "author", "version", "docs", "deprecated"]),
    ("search", &["category", "aliases", "tags"]),
    ("look", &["icon", "width", "preview"]),
    ("requires", &["intents", "dependencies", "permissions", "assets"]),
];

/// Moves the keys in a block file's groups (`info`, `search`, `look`,
/// `requires`) to the top, where `BlockDef` reads them. Returns problems:
/// keys a group doesn't have, and keys given both in a group and on top.
pub fn ungroup(block: &mut Value) -> Vec<String> {
    let mut problems = Vec::new();
    let Some(obj) = block.as_object_mut() else { return problems };
    for (group, keys) in BLOCK_GROUPS {
        let Some(found) = obj.remove(group) else { continue };
        let Value::Object(entries) = found else {
            problems.push(format!("`{group}` should be an object"));
            continue;
        };
        for (key, value) in entries {
            if !keys.contains(&key.as_str()) {
                problems.push(format!("`{group}` has no `{key}` (it takes {})", keys.join(", ")));
            } else if obj.contains_key(&key) {
                problems.push(format!("`{key}` is given twice: in `{group}` and on its own"));
            } else {
                obj.insert(key, value);
            }
        }
    }
    problems
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
    /// Other names it is found by in the block picker, such as "Reply".
    pub aliases: Vec<String>,
    pub tags: Vec<String>,
    pub docs: Option<String>,
    pub fields: IndexMap<String, FieldDef>,
    pub outputs: IndexMap<String, OutputDef>,
    pub flows: Option<Vec<String>>,
    pub intents: Vec<String>,
    /// Permissions the bot needs for it, by Discord's flag names: a list, or
    /// a list per value of one field (`{ action: { kick: ["KickMembers"] } }`).
    pub permissions: Value,
    pub dependencies: IndexMap<String, String>,
    /// Files the block needs in the bot (fonts, pictures), relative to the
    /// block file. They go into the bot with it; `ctx.asset(name)` finds them.
    pub assets: Vec<String>,
    pub width: Option<f32>,
    pub preview: Option<String>,
    pub deprecated: Option<Value>,
    /// Value blocks are cached per run unless they set `pure: false`.
    pub pure: Option<bool>,

    pub has_define: bool,
    pub has_run: bool,
    pub has_listen: bool,
    pub has_migrate: bool,
    /// `run` is an `async` function: the block waits for something (Discord,
    /// the web, a timer, stored data) before the flow goes on.
    pub async_run: bool,

    /// Id of the plugin that contributed this block.
    #[serde(skip)]
    pub plugin: String,
    /// The block file on disk.
    #[serde(skip)]
    pub source: PathBuf,
    /// JavaScript made from a TypeScript file (or one with `import` lines); runs instead of `source`.
    #[serde(skip)]
    pub compiled: Option<String>,
    /// Problems found while reading the file (see `ungroup`), reported by `finish`.
    #[serde(skip)]
    pub load_problems: Vec<String>,
}

impl BlockDef {
    /// Whether the block waits while it runs (`Some(true)`, async) or is done
    /// at once (`Some(false)`, sync). Events don't run like that: `None`.
    pub fn timing(&self) -> Option<bool> {
        (self.kind != BlockKind::Event && self.has_run).then_some(self.async_run)
    }

    /// Parses type strings and returns problems with required keys.
    pub fn finish(&mut self) -> Vec<String> {
        for f in self.fields.values_mut() {
            f.finish();
        }
        for o in self.outputs.values_mut() {
            o.finish();
        }
        let mut problems = std::mem::take(&mut self.load_problems);
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
            // Advanced ones stay hidden until their pin is turned on in the Inspector.
            .filter(|(_, f)| matches!(f.pin, PinMode::Pin | PinMode::Only) && !f.advanced)
            .map(|(k, _)| k.clone())
            .collect()
    }

    /// Action outputs before `define()`. Value blocks have `next` too: put
    /// in a flow, they run there (and pass the flow on); left out of one,
    /// they're worked out when a block reads them.
    pub fn base_flows(&self) -> Vec<String> {
        self.flows.clone().unwrap_or_else(|| vec!["next".into()])
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
        BlockShape::build_with(def, values, define, &HashSet::new())
    }

    /// [`BlockShape::build`], where the names in `known` are values the block
    /// gets from earlier blocks (the event's outputs, run variables): a name
    /// typed for one of them needs no input.
    pub fn build_with(def: &BlockDef, values: &Map<String, Value>, define: Option<&DefineResult>, known: &HashSet<String>) -> BlockShape {
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

        // What a block needs comes first: required fields (not advanced ones)
        // above the rest, each keeping its place among its kind. A define()
        // that adds a required input (Send Message's Interaction) adds it at
        // the end, where it would be missed.
        let (needed, rest): (Vec<_>, Vec<_>) = fields.into_iter().partition(|(_, f)| f.required && !f.advanced);
        fields = needed.into_iter().chain(rest).collect();

        // Each name a ${…} in a text field uses (or in any text of a design such
        // as an embed) gets an input right below it, to connect its value,
        // unless the block gets that name from earlier blocks anyway.
        // Unconnected, the name is looked up in the event, variables and
        // earlier outputs when the block runs.
        let mut extra: Vec<(usize, String, FieldDef)> = Vec::new();
        for (i, (k, f)) in fields.iter().enumerate() {
            if hidden.contains(k) || !f.templated() || f.pin == PinMode::Only {
                continue;
            }
            let mut texts = Vec::new();
            strings_in(values.get(k).or(f.default.as_ref()).unwrap_or(&Value::Null), &mut texts);
            for name in texts.iter().flat_map(|t| placeholders(t)) {
                if fields.contains_key(&name) || known.contains(&name) || extra.iter().any(|(_, n, _)| *n == name) {
                    continue;
                }
                let mut input = FieldDef {
                    type_name: "any".into(),
                    label: Some(format!("${{{name}}}")),
                    description: Some(format!("The value of {name} in {}.", f.label(k))),
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
            // Value blocks can be put in a flow too, to say when they run.
            action_in: matches!(def.kind, BlockKind::Action | BlockKind::Value),
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
/// Every string in a value, nested ones included, in order.
fn strings_in<'a>(v: &'a Value, out: &mut Vec<&'a str>) {
    match v {
        Value::String(s) => out.push(s),
        Value::Array(a) => a.iter().for_each(|x| strings_in(x, out)),
        Value::Object(m) => m.values().for_each(|x| strings_in(x, out)),
        _ => {}
    }
}

/// The names a text's `${…}` expressions use, in order and once each (see
/// [`crate::template::names`]): each can get an input.
pub fn placeholders(text: &str) -> Vec<String> {
    crate::template::names(text)
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
    fn known_names_need_no_input() {
        let def: BlockDef = serde_json::from_value(json!({
            "id": "t.b", "name": "B", "kind": "action",
            "fields": {
                "text": { "type": "textarea" },
                "card": { "type": "embed", "template": true }
            }
        }))
        .unwrap();
        let values = json!({ "text": "Hi ${user}, ${xp} XP", "card": { "title": "${user}", "fields": [{ "value": "${level}" }] } });
        let known: HashSet<String> = ["user".to_string()].into();
        let shape = BlockShape::build_with(&def, values.as_object().unwrap(), None, &known);
        let keys: Vec<&str> = shape.fields.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(keys, ["text", "xp", "card", "level"], "user comes from earlier blocks; the design's texts count too");
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
        let values = serde_json::json!({ "text": "Hi ${name}, ${name} has ${count + after}", "code": "${skip}" });
        let shape = BlockShape::build(&def, values.as_object().unwrap(), None);
        let keys: Vec<&str> = shape.fields.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(keys, vec!["text", "name", "count", "code", "mode", "after"], "inputs right after their text, once each; existing fields aren't doubled");
        let name = shape.field("name").unwrap();
        assert_eq!(name.pin, PinMode::Only);
        assert_eq!(name.label("name"), "${name}");
        assert!(!shape.field("code").unwrap().templated() && !shape.field("mode").unwrap().templated());
        // A colour field takes ${…} too (a colour from the bot or an input).
        let color: FieldDef = serde_json::from_value(json!({ "type": "color" })).unwrap();
        assert!(color.templated());
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
