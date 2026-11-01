//! Everything loaded from plugin folders, in dependency order.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use circuit_core::{BlockDef, PinType, TypeInfo};
use indexmap::IndexMap;

use crate::engine::BlockEngine;
use crate::manifest::{PluginManifest, engine_supported};
use crate::mode::BuildMode;
use crate::theme::ThemeDef;
use crate::types_def::{PinTypeDef, PinTypeFile};

#[derive(Clone, Debug)]
pub struct LoadedPlugin {
    pub manifest: PluginManifest,
    pub dir: PathBuf,
}

/// Something that went wrong while loading, shown in the Problems panel.
#[derive(Clone, Debug)]
pub struct Problem {
    pub source: String,
    pub message: String,
}

#[derive(Default)]
pub struct Registry {
    pub plugins: Vec<LoadedPlugin>,
    pub blocks: IndexMap<String, BlockDef>,
    pub types: IndexMap<String, PinTypeDef>,
    pub themes: Vec<ThemeDef>,
    pub modes: Vec<BuildMode>,
    /// Named icons: file stem → file path.
    pub icons: HashMap<String, PathBuf>,
    pub problems: Vec<Problem>,
    /// Engine key for each block id, used for `define()` calls.
    pub engine_keys: HashMap<String, String>,
}

impl Registry {
    /// Loads every plugin found directly inside the given folders.
    pub fn load(dirs: &[PathBuf], engine: &BlockEngine) -> Registry {
        let mut reg = Registry::default();
        let mut found = Vec::new();
        for dir in dirs {
            let Ok(entries) = fs::read_dir(dir) else { continue };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.join("plugin.toml").is_file() {
                    match read_manifest(&path) {
                        Ok(manifest) => found.push(LoadedPlugin { manifest, dir: path }),
                        Err(e) => reg.problem(path.display().to_string(), format!("{e:#}")),
                    }
                }
            }
        }

        for plugin in order_by_dependencies(found, &mut reg) {
            reg.load_plugin(&plugin, engine);
            reg.plugins.push(plugin);
        }
        reg
    }

    fn problem(&mut self, source: impl Into<String>, message: impl Into<String>) {
        self.problems.push(Problem { source: source.into(), message: message.into() });
    }

    fn load_plugin(&mut self, plugin: &LoadedPlugin, engine: &BlockEngine) {
        let m = &plugin.manifest;
        let c = &m.contributes;

        for path in expand(&plugin.dir, &c.icons) {
            if let Some(stem) = path.file_stem() {
                self.icons.insert(stem.to_string_lossy().into_owned(), path);
            }
        }

        for path in expand(&plugin.dir, &c.pin_types) {
            match read_toml::<PinTypeFile>(&path) {
                Ok(file) => {
                    for t in file.types {
                        self.types.insert(t.id.clone(), t);
                    }
                }
                Err(e) => self.problem(display(&path), format!("{e:#}")),
            }
        }

        for path in expand(&plugin.dir, &c.themes) {
            match read_toml::<ThemeDef>(&path) {
                Ok(t) => {
                    self.themes.retain(|old| old.id != t.id);
                    self.themes.push(t);
                }
                Err(e) => self.problem(display(&path), format!("{e:#}")),
            }
        }

        for path in expand(&plugin.dir, &c.build_modes) {
            match read_toml::<BuildMode>(&path) {
                Ok(mut mode) => {
                    mode.plugin = m.id.clone();
                    self.modes.retain(|old| old.id != mode.id);
                    self.modes.push(mode);
                }
                Err(e) => self.problem(display(&path), format!("{e:#}")),
            }
        }

        for path in expand(&plugin.dir, &c.blocks) {
            self.load_block(plugin, &path, engine);
        }
    }

    fn load_block(&mut self, plugin: &LoadedPlugin, path: &Path, engine: &BlockEngine) {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or_default();
        let source = match fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => return self.problem(display(path), e.to_string()),
        };
        // TypeScript is converted to JavaScript; `import` lines become require().
        let compiled = if ext == "ts" {
            match crate::ts::to_javascript(path, &source) {
                Ok(js) => Some(js),
                Err(e) => return self.problem(display(path), format!("TypeScript: {e:#}")),
            }
        } else if source.lines().any(|l| l.trim_start().starts_with("import ")) {
            Some(crate::ts::imports_to_require(&source))
        } else {
            None
        };
        let source = compiled.clone().unwrap_or(source);
        let key = display(path);
        let mut def = match engine.load_block(&key, &source) {
            Ok(def) => def,
            Err(e) => return self.problem(display(path), format!("{e:#}")),
        };
        for p in def.finish() {
            self.problem(display(path), p);
        }
        if def.id.is_empty() {
            return;
        }
        if def.category.is_none() {
            def.category = path.parent().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned());
        }
        def.plugin = plugin.manifest.id.clone();
        def.source = path.to_path_buf();
        def.compiled = compiled;

        if let Some(existing) = self.blocks.get(&def.id)
            && existing.plugin != def.plugin
            && !plugin.manifest.overrides.contains(&def.id)
        {
            self.problem(
                display(path),
                format!("block id `{}` is already used by plugin `{}`; add it to `overrides` to replace it", def.id, existing.plugin),
            );
            return;
        }
        self.engine_keys.insert(def.id.clone(), key);
        self.blocks.insert(def.id.clone(), def);
    }

    pub fn block(&self, id: &str) -> Option<&BlockDef> {
        self.blocks.get(id.split('@').next().unwrap_or(id))
    }

    pub fn mode(&self, id: &str) -> Option<&BuildMode> {
        self.modes.iter().find(|m| m.id == id)
    }

    pub fn theme(&self, id: &str) -> Option<&ThemeDef> {
        self.themes.iter().find(|t| t.id == id)
    }

    /// Resolves a block's icon to a file: its own path, a named icon, the category's icon, or `block`.
    pub fn block_icon(&self, def: &BlockDef, mode: Option<&BuildMode>) -> Option<PathBuf> {
        if let Some(icon) = &def.icon {
            if icon.ends_with(".svg") || icon.ends_with(".png") {
                let path = def.source.parent().unwrap_or(Path::new(".")).join(icon);
                if path.is_file() {
                    return Some(path);
                }
            } else if let Some(p) = self.icons.get(icon) {
                return Some(p.clone());
            }
        }
        let category_icon = def
            .category
            .as_deref()
            .and_then(|c| mode.and_then(|m| m.category(c)))
            .and_then(|c| c.icon.as_deref())
            .and_then(|i| self.icons.get(i));
        category_icon.or_else(|| self.icons.get("block")).cloned()
    }
}

impl TypeInfo for Registry {
    fn parent(&self, id: &str) -> Option<PinType> {
        self.types.get(id).map(|t| PinType::parse(t.extends.as_deref().unwrap_or("object")))
    }

    fn converts(&self, from: &str, to: &str) -> bool {
        self.types.get(from).is_some_and(|t| t.converts.iter().any(|c| c.to == to))
    }

    fn display_name(&self, id: &str) -> Option<String> {
        self.types.get(id).map(|t| t.name.clone())
    }
}

fn read_manifest(dir: &Path) -> Result<PluginManifest> {
    let m: PluginManifest = read_toml(&dir.join("plugin.toml"))?;
    anyhow::ensure!(!m.id.is_empty(), "plugin.toml has no `id`");
    anyhow::ensure!(
        engine_supported(m.engines.circuit.as_deref()),
        "needs host API {}, this build has {}.{}",
        m.engines.circuit.as_deref().unwrap_or("?"),
        crate::manifest::HOST_API.0,
        crate::manifest::HOST_API.1
    );
    Ok(m)
}

fn read_toml<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let text = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}

/// Expands contribution globs such as `blocks/**/*.{js,ts}` inside a plugin folder.
fn expand(dir: &Path, patterns: &[String]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for pattern in patterns {
        for p in brace_expand(pattern) {
            let full = dir.join(&p).to_string_lossy().replace('\\', "/");
            if let Ok(paths) = glob::glob(&full) {
                out.extend(paths.flatten().filter(|p| p.is_file()));
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// `blocks/*.{js,ts}` → `blocks/*.js`, `blocks/*.ts` (the glob crate has no braces).
fn brace_expand(pattern: &str) -> Vec<String> {
    let (Some(open), Some(close)) = (pattern.find('{'), pattern.find('}')) else {
        return vec![pattern.to_string()];
    };
    pattern[open + 1..close]
        .split(',')
        .flat_map(|alt| brace_expand(&format!("{}{}{}", &pattern[..open], alt, &pattern[close + 1..])))
        .collect()
}

/// Sorts plugins so dependencies load first. Plugins in a cycle or with a
/// missing dependency are dropped and reported.
fn order_by_dependencies(plugins: Vec<LoadedPlugin>, reg: &mut Registry) -> Vec<LoadedPlugin> {
    let mut pending: Vec<LoadedPlugin> = plugins;
    let mut done: Vec<LoadedPlugin> = Vec::new();
    loop {
        let before = pending.len();
        let (ready, rest): (Vec<_>, Vec<_>) = pending.into_iter().partition(|p| {
            p.manifest.dependencies.keys().all(|d| done.iter().any(|x| &x.manifest.id == d))
        });
        done.extend(ready);
        pending = rest;
        if pending.is_empty() || pending.len() == before {
            break;
        }
    }
    for p in pending {
        reg.problem(
            p.manifest.id.clone(),
            format!(
                "not loaded: dependencies {:?} are missing or form a cycle",
                p.manifest.dependencies.keys().collect::<Vec<_>>()
            ),
        );
    }
    done
}

fn display(p: &Path) -> String {
    p.display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brace_patterns() {
        assert_eq!(brace_expand("blocks/**/*.{js,cjs}"), vec!["blocks/**/*.js", "blocks/**/*.cjs"]);
        assert_eq!(brace_expand("themes/*.toml"), vec!["themes/*.toml"]);
    }

    /// Loads the real plugins shipped in the repository.
    #[test]
    fn loads_bundled_plugins() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins");
        let engine = BlockEngine::new().unwrap();
        let reg = Registry::load(&[root], &engine);
        for p in &reg.problems {
            eprintln!("{}: {}", p.source, p.message);
        }
        assert!(reg.problems.is_empty(), "bundled plugins have problems");
        assert!(reg.block("discord.send_message@3").is_some());
        assert!(reg.mode("discord-bot").is_some());
        assert!(reg.theme("graphite").is_some());
    }

    /// The Discord blocks' define() and migrate() as the editor calls them.
    #[test]
    fn discord_blocks_define_and_migrate() {
        use circuit_core::BlockShape;
        use serde_json::{Map, json};

        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins");
        let engine = BlockEngine::new().unwrap();
        let reg = Registry::load(&[root], &engine);
        let key = |id: &str| reg.engine_keys[id].clone();

        // Placeholders in an embed become input pins; event names do not.
        let values = json!({ "embed": { "title": "Rank of {user}", "description": "{xp} XP", "fields": [{ "name": "L", "value": "{level}" }] } });
        let values = values.as_object().unwrap();
        let d = engine.define(&key("discord.build_embed"), values, &Map::new(), &json!({})).unwrap();
        assert!(d.add.contains_key("xp") && d.add.contains_key("level") && !d.add.contains_key("user"));

        // Version 1 values move into the embed object.
        let old = json!({ "title": "Hi", "description": "There", "color": "#FF0000" });
        let migrated = engine.migrate(&key("discord.build_embed"), old.as_object().unwrap(), "1").unwrap();
        assert_eq!(migrated["embed"]["title"], "Hi");
        assert_eq!(migrated["embed"]["color"], "#FF0000");

        // Slash command options become typed outputs.
        let project = json!({ "data": { "slashCommands": [
            { "name": "rank", "description": "d", "options": [{ "name": "target", "type": "user", "description": "x", "required": false }] }
        ] } });
        let values = json!({ "command": "rank" });
        let def = reg.block("discord.on_slash_command").unwrap();
        let d = engine.define(&key("discord.on_slash_command"), values.as_object().unwrap(), &Map::new(), &project).unwrap();
        let shape = BlockShape::build(def, values.as_object().unwrap(), Some(&d));
        assert_eq!(shape.output("target").unwrap().type_name, "discord.user?");
        assert_eq!(shape.field("command").unwrap().choice_list().len(), 1);
    }
}
