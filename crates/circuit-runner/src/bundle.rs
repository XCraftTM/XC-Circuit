//! The bundle: everything the runtime needs, as one JSON value.

use std::collections::BTreeSet;
use std::fs;

use anyhow::{Context, Result, anyhow};
use circuit_core::Project;
use circuit_plugins::{BuildMode, Registry};
use indexmap::IndexMap;
use serde_json::{Map, Value, json};

pub struct Bundle {
    pub value: Value,
    /// npm packages: the mode's plus every used block's `dependencies`.
    pub dependencies: IndexMap<String, String>,
}

/// npm packages for the kinds of database in Bot › Settings › Databases
/// (SQLite is built into Node.js).
pub const DATABASE_DRIVERS: [(&str, &str, &str); 3] = [("mariadb", "mysql2", "^3.24.6"), ("postgres", "pg", "^8.23.1"), ("mongodb", "mongodb", "^7.8.0")];

/// The npm packages the project's databases need.
pub fn database_dependencies(settings: &Map<String, Value>) -> IndexMap<String, String> {
    let kinds: Vec<&str> = settings.get("databases").and_then(Value::as_array).into_iter().flatten().filter_map(|d| d.get("kind").and_then(Value::as_str)).collect();
    DATABASE_DRIVERS.iter().filter(|(kind, ..)| kinds.contains(kind)).map(|(_, package, version)| (package.to_string(), version.to_string())).collect()
}

/// Collects the project's workspaces, the source of every block it uses and the
/// mode's runtime libraries.
pub fn build_bundle(project: &Project, reg: &Registry, mode: Option<&BuildMode>) -> Result<Bundle> {
    // Workspaces that are turned off aren't part of the bot.
    let enabled = || project.workspaces.iter().filter(|(_, ws)| !ws.disabled);
    let used: BTreeSet<String> = enabled().flat_map(|(_, ws)| ws.nodes.values().map(|n| n.block_id().to_string())).collect();

    let mut dependencies: IndexMap<String, String> = mode.map(|m| m.dependencies.clone()).unwrap_or_default();
    let mut blocks = Map::new();
    for id in &used {
        let Some(def) = reg.block(id) else { continue };
        let source = match &def.compiled {
            Some(js) => js.clone(),
            None => fs::read_to_string(&def.source).with_context(|| format!("reading {}", def.source.display()))?,
        };
        blocks.insert(id.clone(), json!({ "source": source, "file": def.source.display().to_string(), "plugin": def.plugin }));
        for (name, version) in &def.dependencies {
            dependencies.entry(name.clone()).or_insert_with(|| version.clone());
        }
    }

    for (name, version) in database_dependencies(&project.meta.settings) {
        dependencies.entry(name).or_insert(version);
    }

    let mut libs = Vec::new();
    for spec in mode.map(|m| m.runtime_libs.as_slice()).unwrap_or_default() {
        let (plugin_id, rel) = spec.split_once('/').ok_or_else(|| anyhow!("runtime library `{spec}` should look like plugin.id/path.js"))?;
        let plugin = reg.plugins.iter().find(|p| p.manifest.id == plugin_id).ok_or_else(|| anyhow!("runtime library `{spec}`: plugin `{plugin_id}` is not loaded"))?;
        let path = plugin.dir.join(rel);
        let source = fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        libs.push(json!({ "source": source, "file": path.display().to_string() }));
    }

    // Pin types that convert into others, for the runtime to apply on wires.
    let types: Map<String, Value> = reg
        .types
        .iter()
        .filter(|(_, t)| t.check.is_some() && !t.converts.is_empty())
        .map(|(id, t)| (id.clone(), json!({ "check": t.check, "converts": t.converts.iter().map(|c| json!({ "to": c.to, "via": c.via })).collect::<Vec<_>>() })))
        .collect();

    let workspaces: Map<String, Value> = enabled()
        .map(|(k, ws)| Ok((k.clone(), serde_json::to_value(ws)?)))
        .collect::<Result<_>>()?;

    // Bot › Settings: what was saved, and the defaults of the rest.
    let mut settings: Map<String, Value> = mode.map(|m| reg.bot_settings(m).defaults().into_iter().collect()).unwrap_or_default();
    for (key, value) in &project.meta.settings {
        settings.insert(key.clone(), value.clone());
    }

    let value = json!({
        "project": {
            "name": project.meta.name,
            "mode": project.meta.mode,
            "settings": settings,
            "data": project.meta.data,
            "tokenEnv": mode.map(|m| m.token_env()).unwrap_or("DISCORD_TOKEN"),
        },
        "blocks": blocks,
        "libs": libs,
        "types": types,
        "workspaces": workspaces,
    });
    Ok(Bundle { value, dependencies })
}

#[cfg(test)]
mod tests {
    use super::*;
    use circuit_core::{Node, Workspace};

    /// A workspace that is turned off isn't in the bot, and neither are
    /// blocks only it uses.
    #[test]
    fn leaves_out_workspaces_that_are_off() {
        let dir = std::env::temp_dir().join(format!("xc-bundle-off-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let mut project = Project::create(&dir, "Off", "none").unwrap();
        let node = |block: &str| Node { block: block.into(), ..Default::default() };
        let mut on = Workspace::new("On");
        on.nodes.insert("a".into(), node("core.on_start@1"));
        let mut off = Workspace::new("Off");
        off.nodes.insert("b".into(), node("core.wait@1"));
        off.disabled = true;
        project.workspaces.insert("on".into(), on);
        project.workspaces.insert("off".into(), off);
        let engine = circuit_plugins::BlockEngine::new().unwrap();
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins");
        let reg = Registry::load(&[root], &engine);
        let bundle = build_bundle(&project, &reg, None).unwrap();
        let workspaces = bundle.value["workspaces"].as_object().unwrap();
        assert!(workspaces.contains_key("on") && !workspaces.contains_key("off"));
        let blocks = bundle.value["blocks"].as_object().unwrap();
        assert!(blocks.contains_key("core.on_start") && !blocks.contains_key("core.wait"));
        // Saved with the project, and only when off.
        let json = serde_json::to_value(&project.workspaces["off"]).unwrap();
        assert_eq!(json["disabled"], true);
        assert!(serde_json::to_value(&project.workspaces["on"]).unwrap().get("disabled").is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    /// The bot gets each setting's default until another value is saved,
    /// and never a secret.
    #[test]
    fn settings_have_their_defaults() {
        let dir = std::env::temp_dir().join(format!("xc-bundle-settings-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let mut project = Project::create(&dir, "Settings", "discord-bot").unwrap();
        let engine = circuit_plugins::BlockEngine::new().unwrap();
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins");
        let reg = Registry::load(&[root], &engine);
        let mode = reg.modes.iter().find(|m| m.id == "discord-bot").unwrap();
        let settings = |project: &Project| build_bundle(project, &reg, Some(mode)).unwrap().value["project"]["settings"].clone();
        assert_eq!(settings(&project)["prefix"], "!");
        assert!(settings(&project).get("token").is_none());
        project.meta.settings.insert("prefix".into(), Value::from("?"));
        assert_eq!(settings(&project)["prefix"], "?");
        let _ = fs::remove_dir_all(&dir);
    }

    /// Each kind of database brings its npm package; SQLite needs none.
    #[test]
    fn databases_bring_their_packages() {
        let settings = |kinds: &[&str]| {
            let list: Vec<Value> = kinds.iter().enumerate().map(|(i, k)| json!({ "id": format!("db{i}"), "kind": k })).collect();
            let mut m = Map::new();
            m.insert("databases".into(), Value::Array(list));
            database_dependencies(&m)
        };
        assert!(settings(&[]).is_empty());
        assert!(settings(&["sqlite"]).is_empty());
        let deps = settings(&["mariadb", "postgres", "mariadb", "mongodb"]);
        assert_eq!(deps.keys().map(String::as_str).collect::<Vec<_>>(), ["mysql2", "pg", "mongodb"]);
    }
}
