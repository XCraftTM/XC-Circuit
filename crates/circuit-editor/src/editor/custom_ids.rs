//! Custom ids: the ids a project gives its buttons, menus and forms (in
//! layouts, forms and blocks such as Button), and the events that handle them
//! (Button Pressed, Select Menu Used, Modal Submitted). The events' Custom id
//! fields offer the ids there are, and Problems says when one side has no
//! partner: an event waiting for an id nothing uses, or a button nothing handles.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use circuit_core::{Project, Workspace};
use circuit_plugins::Registry;
use serde_json::Value;

/// A kind of thing with a custom id.
pub const BUTTON: &str = "button";
pub const SELECT: &str = "select";
pub const FORM: &str = "form";

/// One custom id the project gives something.
#[derive(Clone, Debug, PartialEq)]
pub struct Source {
    pub kind: &'static str,
    pub id: String,
    pub ws: String,
    pub node: String,
}

/// An event waiting for a custom id.
#[derive(Clone, Debug, PartialEq)]
pub struct Handler {
    pub kind: String,
    pub id: String,
    /// "equals", "starts" or "pattern".
    pub matching: String,
    pub ws: String,
    pub node: String,
}

/// What the project uses: the ids given (and whether some kind's ids are made
/// while the bot runs, so they can't be known) and the events waiting for them.
#[derive(Default, Debug)]
pub struct Ids {
    pub sources: Vec<Source>,
    pub handlers: Vec<Handler>,
    /// Kinds with ids that are connected or hold ${…}: not known before the bot runs.
    pub unknown: BTreeSet<&'static str>,
}

fn kind_of(name: &str) -> Option<&'static str> {
    [BUTTON, SELECT, FORM].into_iter().find(|k| *k == name)
}

/// Collects them from the open project's workspaces that are on, leaving
/// out fields a block hides (as it shows itself): Show Modal's older Custom id
/// is hidden while its form comes from the Modal Builder, and isn't the form's id.
pub fn collect_for(ed: &mut crate::editor::Editor) -> Ids {
    let crate::editor::Editor { reg, engine, project, shapes, define_ctx, .. } = ed;
    let Some(project) = project.as_ref() else { return Ids::default() };
    shapes.begin();
    collect_with(project, reg, &mut |ws_key, ws, node_id| crate::editor::node_shape(reg, engine, shapes, ws_key, ws, node_id, define_ctx).map(|s| s.hidden.clone()).unwrap_or_default())
}

/// Collects them from the workspaces that are on, every field counted.
#[cfg(test)]
pub fn collect(project: &Project, reg: &Registry) -> Ids {
    collect_with(project, reg, &mut |_, _, _| HashSet::new())
}

/// Collects them, without the fields `hidden` names for a block.
fn collect_with(project: &Project, reg: &Registry, hidden: &mut dyn FnMut(&str, &Workspace, &str) -> HashSet<String>) -> Ids {
    let mut ids = Ids::default();
    for (ws_key, ws) in project.workspaces.iter().filter(|(_, w)| !w.disabled) {
        for (node_id, node) in &ws.nodes {
            let Some(def) = reg.block(&node.block) else { continue };
            // Only blocks with ids to give or wait for are looked at closer.
            let matters = def.fields.values().any(|f| f.custom_id_of.is_some() || f.suggest.is_some() || matches!(f.type_name.as_str(), "layout" | "modal"));
            if !matters {
                continue;
            }
            let hidden = hidden(ws_key, ws, node_id);
            for (key, f) in def.fields.iter().filter(|(k, _)| !hidden.contains(*k)) {
                let value = node.values.get(key).or(f.default.as_ref());
                let wired = ws.links_into(node_id, key).next().is_some();
                let mut give = |kind: &'static str, id: &str| {
                    let id = id.trim();
                    if id.is_empty() {
                        return;
                    }
                    if id.contains("${") {
                        ids.unknown.insert(kind);
                    } else {
                        ids.sources.push(Source { kind, id: id.to_string(), ws: ws_key.clone(), node: node_id.clone() });
                    }
                };
                match f.type_name.as_str() {
                    "layout" => {
                        if let Some(v) = value {
                            walk_layout(v.get("components").unwrap_or(&Value::Null), &mut give);
                        }
                    }
                    "modal" => {
                        if let Some(id) = value.and_then(|v| v.get("custom_id")).and_then(Value::as_str) {
                            give(FORM, id);
                        }
                    }
                    _ => {}
                }
                if let Some(kind) = f.custom_id_of.as_deref().and_then(kind_of) {
                    if wired {
                        ids.unknown.insert(kind);
                    } else if let Some(id) = value.and_then(Value::as_str) {
                        give(kind, id);
                    }
                }
                if let Some(kind) = &f.suggest {
                    let id = value.and_then(Value::as_str).unwrap_or_default().trim().to_string();
                    let matching = node.values.get("match").and_then(Value::as_str).unwrap_or("equals").to_string();
                    if !id.is_empty() {
                        ids.handlers.push(Handler { kind: kind.clone(), id, matching, ws: ws_key.clone(), node: node_id.clone() });
                    }
                }
            }
        }
    }
    ids
}

/// The forms the project designs in the Modal Builder (Show Modal's form), in
/// the workspaces that are on, for blocks' `define()` (as `project.forms`):
/// `[{ customId, inputs: [{ id, kind, label }] }]`. An input's kind is
/// text, options, users, roles, mentionables, channels or files. Forms
/// with a worked-out id (`${…}`) are left out.
pub fn forms(project: &Project, reg: &Registry) -> Value {
    let mut out = Vec::new();
    for ws in project.workspaces.values().filter(|w| !w.disabled) {
        for node in ws.nodes.values() {
            let Some(def) = reg.block(&node.block) else { continue };
            for (key, f) in def.fields.iter().filter(|(_, f)| f.type_name == "modal") {
                let Some(form) = node.values.get(key).or(f.default.as_ref()) else { continue };
                let Some(id) = form.get("custom_id").and_then(Value::as_str).map(str::trim).filter(|id| !id.is_empty() && !id.contains("${")) else { continue };
                let mut inputs = Vec::new();
                form_inputs(form.get("components").unwrap_or(&Value::Null), None, &mut inputs);
                out.push(serde_json::json!({ "customId": id, "inputs": inputs }));
            }
        }
    }
    Value::Array(out)
}

/// A form's inputs: inside labels (their question), and older rows.
fn form_inputs(list: &Value, label: Option<&str>, out: &mut Vec<Value>) {
    for c in list.as_array().into_iter().flatten() {
        let kind = match c.get("type").and_then(Value::as_u64).unwrap_or(0) {
            18 => {
                let question = c.get("label").and_then(Value::as_str);
                if let Some(inner) = c.get("component") {
                    form_inputs(&Value::Array(vec![inner.clone()]), question, out);
                }
                continue;
            }
            1 => {
                form_inputs(c.get("components").unwrap_or(&Value::Null), label, out);
                continue;
            }
            4 => "text",
            3 => "options",
            5 => "users",
            6 => "roles",
            7 => "mentionables",
            8 => "channels",
            19 => "files",
            _ => continue,
        };
        let Some(id) = c.get("custom_id").and_then(Value::as_str).map(str::trim).filter(|id| !id.is_empty()) else { continue };
        let label = label.or_else(|| c.get("label").and_then(Value::as_str)).unwrap_or(id);
        out.push(serde_json::json!({ "id": id, "kind": kind, "label": label }));
    }
}

/// Buttons (not links or premium) and menus in a layout, nested ones too.
fn walk_layout(list: &Value, give: &mut dyn FnMut(&'static str, &str)) {
    for c in list.as_array().into_iter().flatten() {
        let ty = c.get("type").and_then(Value::as_u64).unwrap_or(0);
        let id = c.get("custom_id").and_then(Value::as_str);
        match (ty, id) {
            (2, Some(id)) if !matches!(c.get("style").and_then(Value::as_u64), Some(5 | 6)) => give(BUTTON, id),
            (3 | 5 | 6 | 7 | 8, Some(id)) => give(SELECT, id),
            _ => {}
        }
        if let Some(a) = c.get("accessory") {
            walk_layout(&Value::Array(vec![a.clone()]), give);
        }
        walk_layout(c.get("components").unwrap_or(&Value::Null), give);
    }
}

impl Handler {
    /// Whether this event runs for `id`, as the runtime matches it.
    pub fn takes(&self, id: &str) -> bool {
        match self.matching.as_str() {
            "starts" => id.starts_with(&self.id),
            // A pattern we can't read is taken as matching: no false alarm.
            "pattern" => regex::Regex::new(&self.id).map(|r| r.is_match(id)).unwrap_or(true),
            _ => id == self.id,
        }
    }
}

impl Ids {
    /// The ids of each kind, sorted, for the events' Custom id fields.
    pub fn suggestions(&self) -> BTreeMap<String, Vec<String>> {
        let mut out: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for s in &self.sources {
            out.entry(s.kind.to_string()).or_default().insert(s.id.clone());
        }
        out.into_iter().map(|(k, v)| (k, v.into_iter().collect())).collect()
    }

    /// What doesn't fit: (workspace, block, message).
    pub fn problems(&self) -> Vec<(String, String, String)> {
        let mut out = Vec::new();
        let noun = |kind: &str| match kind {
            SELECT => t!("menu"),
            FORM => t!("form"),
            _ => t!("button"),
        };
        // An event waiting for an exact id nothing gives (unless such ids are made while running).
        for h in self.handlers.iter().filter(|h| h.matching == "equals" && !h.id.contains("${")) {
            let Some(kind) = kind_of(&h.kind) else { continue };
            if self.unknown.contains(kind) || self.sources.iter().any(|s| s.kind == kind && s.id == h.id) {
                continue;
            }
            out.push((h.ws.clone(), h.node.clone(), tf!("no {thing} has the custom id “{id}”, so this never runs: check it is spelled like the {thing}'s", thing = noun(kind), id = h.id)));
        }
        // A button, menu or form no event takes.
        let mut told = BTreeSet::new();
        for s in &self.sources {
            if self.handlers.iter().any(|h| h.kind == s.kind && h.takes(&s.id)) || !told.insert((s.kind, s.id.clone())) {
                continue;
            }
            let event = match s.kind {
                SELECT => t!("Select Menu Used"),
                FORM => t!("Modal Submitted"),
                _ => t!("Button Pressed"),
            };
            out.push((s.ws.clone(), s.node.clone(), tf!("nothing handles the {thing} “{id}”: add {event} with this custom id", thing = noun(s.kind), id = s.id, event = event)));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn modal_submitted_gives_its_forms_inputs_as_what_they_are() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut ed = crate::editor::Editor::new(vec![root.join("plugins")], None).unwrap();
        let dir = std::env::temp_dir().join(format!("xc-form-outputs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        Project::create(&dir, "Forms", "discord-bot").unwrap();
        ed.open_project(&dir);
        let node = |block: &str, values: Value| circuit_core::Node { block: block.into(), values: values.as_object().cloned().unwrap_or_default(), ..Default::default() };
        let form = |extra: Value| json!({ "form": { "title": "Apply", "custom_id": "apply", "components": [
            { "type": 18, "label": "Your name", "component": { "type": 4, "custom_id": "name", "style": 1 } },
            { "type": 18, "label": "Who vouches", "component": { "type": 5, "custom_id": "who" } },
            { "type": 18, "label": "Team", "component": { "type": 6, "custom_id": "team" } },
            { "type": 18, "label": "Proof", "component": { "type": 19, "custom_id": "proof" } },
            { "type": 10, "content": "Thanks!" },
            extra
        ] } });
        let key = {
            let p = ed.project.as_mut().unwrap();
            let (key, ws) = p.workspaces.iter_mut().next().unwrap();
            ws.nodes.insert("show".into(), node("discord.show_modal@1", form(json!({ "type": 10, "content": "" }))));
            ws.nodes.insert("got".into(), node("discord.on_modal@1", json!({ "customId": "apply" })));
            key.clone()
        };
        ed.refresh_define_ctx();
        let outputs = |ed: &mut crate::editor::Editor| {
            let crate::editor::Editor { reg, engine, project, shapes, define_ctx, .. } = ed;
            shapes.begin();
            let ws = &project.as_ref().unwrap().workspaces[&key];
            let shape = crate::editor::node_shape(reg, engine, shapes, &key, ws, "got", define_ctx).unwrap();
            shape.outputs.iter().map(|(k, o)| (k.clone(), o.ty.to_string())).collect::<std::collections::BTreeMap<_, _>>()
        };
        let o = outputs(&mut ed);
        assert_eq!(o.get("name").map(String::as_str), Some("Text"), "{o:?}");
        assert_eq!(o.get("who").map(String::as_str), Some("List of discord.member"));
        assert_eq!(o.get("team").map(String::as_str), Some("List of discord.role"));
        assert_eq!(o.get("proof").map(String::as_str), Some("List of Object"));
        // A question added to the form shows up after the edit.
        ed.project.as_mut().unwrap().workspaces.get_mut(&key).unwrap().nodes.get_mut("show").unwrap().values =
            form(json!({ "type": 18, "label": "Where", "component": { "type": 8, "custom_id": "where" } })).as_object().cloned().unwrap();
        ed.refresh_forms();
        assert_eq!(outputs(&mut ed).get("where").map(String::as_str), Some("List of discord.channel"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ids_in_layouts_blocks_and_forms_meet_their_events() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let ed = crate::editor::Editor::new(vec![root.join("plugins")], None).unwrap();
        let dir = std::env::temp_dir().join(format!("xc-custom-ids-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut project = Project::create(&dir, "Ids", "discord-bot").unwrap();
        let node = |block: &str, values: Value| circuit_core::Node { block: block.into(), values: values.as_object().cloned().unwrap_or_default(), ..Default::default() };
        let ws = project.workspaces.values_mut().next().unwrap();
        ws.nodes.insert(
            "lay".into(),
            node("discord.build_layout@1", json!({ "layout": { "components": [{ "type": 17, "components": [
                { "type": 1, "components": [
                    { "type": 2, "style": 4, "label": "Close", "custom_id": "ticket-close" },
                    { "type": 2, "style": 5, "label": "Site", "url": "https://x.example" },
                    { "type": 2, "style": 1, "label": "Vote", "custom_id": "vote:${id}" }
                ] },
                { "type": 1, "components": [{ "type": 3, "custom_id": "ticket-open", "options": [] }] }
            ] }] } })),
        );
        ws.nodes.insert("btn".into(), node("discord.button@1", json!({ "customId": "help" })));
        ws.nodes.insert("form".into(), node("discord.show_modal@1", json!({ "form": { "title": "T", "custom_id": "apply", "components": [] } })));
        ws.nodes.insert("onClose".into(), node("discord.on_button@1", json!({ "customId": "ticket-close" })));
        ws.nodes.insert("onTypo".into(), node("discord.on_button@1", json!({ "customId": "tiket-close" })));
        ws.nodes.insert("onVote".into(), node("discord.on_button@1", json!({ "customId": "vote:", "match": "starts" })));
        ws.nodes.insert("onApply".into(), node("discord.on_modal@1", json!({ "customId": "apply" })));
        let ids = collect(&project, &ed.reg);
        let sugg = ids.suggestions();
        assert_eq!(sugg[BUTTON], ["help", "ticket-close"], "links have no id; ${{…}} ids aren't known");
        assert_eq!(sugg[SELECT], ["ticket-open"]);
        assert!(sugg[FORM].contains(&"apply".to_string()));
        assert!(ids.unknown.contains(BUTTON));
        let problems = ids.problems();
        let about = |node: &str| problems.iter().filter(|(_, n, _)| n == node).map(|(_, _, m)| m.clone()).collect::<Vec<_>>();
        assert!(about("onClose").is_empty() && about("onVote").is_empty() && about("onApply").is_empty(), "{problems:?}");
        assert!(about("onTypo").is_empty(), "made-while-running button ids exist, so a typo can't be told apart: {problems:?}");
        assert!(about("btn").iter().any(|m| m.contains("nothing handles the button “help”")), "{problems:?}");
        assert!(about("lay").iter().any(|m| m.contains("nothing handles the menu “ticket-open”")), "{problems:?}");
        assert!(!about("lay").iter().any(|m| m.contains("ticket-close")), "handled: {problems:?}");

        // Show Modal's older Custom id ("my-form") is hidden while the form
        // comes from the Modal Builder: as the editor shows the block, it isn't
        // a form's id, so nothing says "my-form" needs handling.
        let mut ed = ed;
        ed.open_project(&dir);
        ed.project = Some(project.clone());
        let shown = collect_for(&mut ed);
        assert!(!shown.sources.iter().any(|s| s.id == "my-form"), "{:?}", shown.sources);
        assert!(shown.sources.iter().any(|s| s.kind == FORM && s.id == "apply"));
        assert!(collect(&project, &ed.reg).sources.iter().any(|s| s.id == "my-form"), "counting every field finds it");

        // Without ids made while running, the typo shows.
        project.workspaces.values_mut().next().unwrap().nodes.shift_remove("lay");
        let ids = collect(&project, &ed.reg);
        let problems = ids.problems();
        assert!(problems.iter().any(|(_, n, m)| n == "onTypo" && m.contains("no button has the custom id “tiket-close”")), "{problems:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
