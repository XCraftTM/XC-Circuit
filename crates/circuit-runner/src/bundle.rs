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

/// Collects the project's workspaces, the source of every block it uses and the
/// mode's runtime libraries.
pub fn build_bundle(project: &Project, reg: &Registry, mode: Option<&BuildMode>) -> Result<Bundle> {
    let used: BTreeSet<String> = project
        .workspaces
        .values()
        .flat_map(|ws| ws.nodes.values().map(|n| n.block_id().to_string()))
        .collect();

    let mut dependencies: IndexMap<String, String> = mode.map(|m| m.dependencies.clone()).unwrap_or_default();
    let mut blocks = Map::new();
    for id in &used {
        let Some(def) = reg.block(id) else { continue };
        let source = fs::read_to_string(&def.source).with_context(|| format!("reading {}", def.source.display()))?;
        blocks.insert(id.clone(), json!({ "source": source, "file": def.source.display().to_string(), "plugin": def.plugin }));
        for (name, version) in &def.dependencies {
            dependencies.entry(name.clone()).or_insert_with(|| version.clone());
        }
    }

    let mut libs = Vec::new();
    for spec in mode.map(|m| m.runtime_libs.as_slice()).unwrap_or_default() {
        let (plugin_id, rel) = spec.split_once('/').ok_or_else(|| anyhow!("runtime library `{spec}` should look like plugin.id/path.js"))?;
        let plugin = reg.plugins.iter().find(|p| p.manifest.id == plugin_id).ok_or_else(|| anyhow!("runtime library `{spec}`: plugin `{plugin_id}` is not loaded"))?;
        let path = plugin.dir.join(rel);
        let source = fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        libs.push(json!({ "source": source, "file": path.display().to_string() }));
    }

    let workspaces: Map<String, Value> = project
        .workspaces
        .iter()
        .map(|(k, ws)| Ok((k.clone(), serde_json::to_value(ws)?)))
        .collect::<Result<_>>()?;

    let value = json!({
        "project": { "name": project.meta.name, "mode": project.meta.mode, "settings": project.meta.settings, "data": project.meta.data },
        "blocks": blocks,
        "libs": libs,
        "workspaces": workspaces,
    });
    Ok(Bundle { value, dependencies })
}
