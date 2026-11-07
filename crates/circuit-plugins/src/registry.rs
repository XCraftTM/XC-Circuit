//! Everything loaded from plugin folders, in dependency order.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use circuit_core::{BlockDef, PinType, TypeInfo};
use indexmap::IndexMap;

use crate::engine::BlockEngine;
use crate::manifest::{PluginManifest, app_supported, compare_versions, engine_supported};
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
    /// Plugins found but turned off: not loaded, listed so they can be turned on again.
    pub disabled: Vec<LoadedPlugin>,
    pub blocks: IndexMap<String, BlockDef>,
    pub types: IndexMap<String, PinTypeDef>,
    /// Pin types that can be found by an id (their own `fromId`, or one of
    /// a type they extend).
    pub id_types: std::collections::HashSet<String>,
    /// Class catalogs: class name → members, shared with text boxes.
    pub classes: std::sync::Arc<HashMap<String, crate::ClassDef>>,
    /// The class behind each pin type, from the catalogs' `roots`.
    pub type_classes: HashMap<String, String>,
    pub themes: Vec<ThemeDef>,
    pub modes: Vec<BuildMode>,
    /// Plugins' parts of Bot › Settings.
    pub bot_settings: Vec<crate::bot_settings::SettingsFile>,
    /// Translations, one per language code (files for the same language are merged).
    pub locales: Vec<crate::LocaleDef>,
    /// Named icons: file stem → file path.
    pub icons: HashMap<String, PathBuf>,
    /// Blocks whose icon is a file next to them: block id → that file, when
    /// it exists (checked once, when loading).
    block_icons: HashMap<String, PathBuf>,
    pub problems: Vec<Problem>,
    /// Engine key for each block id, used for `define()` calls.
    pub engine_keys: HashMap<String, String>,
}

impl Registry {
    /// Loads every plugin found directly inside the given folders.
    pub fn load(dirs: &[PathBuf], engine: &BlockEngine) -> Registry {
        Self::load_except(dirs, engine, &[])
    }

    /// [`Self::load`], leaving out the plugins with these ids (turned off).
    pub fn load_except(dirs: &[PathBuf], engine: &BlockEngine, disabled: &[String]) -> Registry {
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

        // The same plugin in several folders (the bundled one and a
        // downloaded update, say): the highest version loads.
        let mut best: Vec<LoadedPlugin> = Vec::new();
        for p in found {
            match best.iter().position(|b| b.manifest.id == p.manifest.id) {
                Some(i) if compare_versions(&p.manifest.version, &best[i].manifest.version) == std::cmp::Ordering::Greater => best[i] = p,
                Some(_) => {}
                None => best.push(p),
            }
        }

        let (off, best): (Vec<_>, Vec<_>) = best.into_iter().partition(|p| disabled.contains(&p.manifest.id));
        reg.disabled = off;
        for plugin in order_by_dependencies(best, &mut reg) {
            reg.load_plugin(&plugin, engine);
            reg.plugins.push(plugin);
        }
        reg.mark_id_fields();
        reg
    }

    /// Fields whose type (or one it extends) can be found by an id (see
    /// [`Registry::mark_id_field`]).
    fn mark_id_fields(&mut self) {
        self.id_types = self
            .types
            .keys()
            .filter(|id| {
                let mut at = Some(id.as_str());
                for _ in 0..16 {
                    let Some(t) = at.and_then(|a| self.types.get(a)) else { break };
                    if t.from_id.is_some() {
                        return true;
                    }
                    at = t.extends.as_deref();
                }
                false
            })
            .cloned()
            .collect();
        let types = std::mem::take(&mut self.id_types);
        for def in self.blocks.values_mut() {
            for f in def.fields.values_mut() {
                Self::mark(&types, f);
            }
        }
        self.id_types = types;
    }

    /// When a field's type can be found by an id, the field can be switched
    /// from its pin to a box for the id: `pin: "only"` becomes a pin that can
    /// be turned off. For the fields a block adds in `define()` too.
    pub fn mark_id_field(&self, f: &mut circuit_core::FieldDef) {
        Self::mark(&self.id_types, f);
    }

    fn mark(types: &std::collections::HashSet<String>, f: &mut circuit_core::FieldDef) {
        if !f.by_id && types.contains(f.type_name.trim_end_matches('?')) {
            f.by_id = true;
            if f.pin == circuit_core::block::PinMode::Only {
                f.pin = circuit_core::block::PinMode::Pin;
            }
        }
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

        for path in expand(&plugin.dir, &c.classes) {
            match std::fs::read_to_string(&path).map_err(anyhow::Error::from).and_then(|t| serde_json::from_str::<crate::types_def::ClassFile>(&t).map_err(anyhow::Error::from)) {
                Ok(file) => {
                    self.type_classes.extend(file.roots);
                    std::sync::Arc::make_mut(&mut self.classes).extend(file.classes);
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

        for path in expand(&plugin.dir, &c.locales) {
            match read_toml::<crate::LocaleDef>(&path) {
                Ok(l) if l.meta.code.trim().is_empty() => self.problem(display(&path), "a translation needs [meta] code, such as \"de\""),
                Ok(l) => match self.locales.iter_mut().find(|old| old.meta.code == l.meta.code) {
                    Some(old) => old.merge(l),
                    None => {
                        let mut first = crate::LocaleDef { meta: l.meta.clone(), ..Default::default() };
                        first.merge(l);
                        self.locales.push(first);
                    }
                },
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

        self.bot_settings.retain(|f| f.plugin != m.id);
        for path in expand(&plugin.dir, &c.bot_settings) {
            match read_toml::<crate::bot_settings::SettingsFile>(&path) {
                Ok(mut file) => {
                    file.plugin = m.id.clone();
                    self.bot_settings.push(file);
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
        let dir = path.parent().unwrap_or(Path::new("."));
        for asset in &def.assets {
            if !dir.join(asset).is_file() {
                self.problem(display(path), format!("`requires.assets` lists {asset}, but there's no such file next to the block"));
            }
        }
        self.engine_keys.insert(def.id.clone(), key);
        self.block_icons.remove(&def.id);
        if let Some(icon) = def.icon.as_deref().filter(|i| i.ends_with(".svg") || i.ends_with(".png")) {
            let file = path.parent().unwrap_or(Path::new(".")).join(icon);
            if file.is_file() {
                self.block_icons.insert(def.id.clone(), file);
            }
        }
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
                if let Some(path) = self.block_icons.get(&def.id) {
                    return Some(path.clone());
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
    anyhow::ensure!(
        app_supported(m.engines.app.as_deref()),
        "needs XC Circuit {} or newer (this is {})",
        m.engines.app.as_deref().unwrap_or("?"),
        crate::manifest::APP_VERSION
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

    /// Of the same plugin in two folders, the higher version loads; one that
    /// needs a newer app doesn't load at all.
    #[test]
    fn the_highest_version_of_a_plugin_loads() {
        let base = std::env::temp_dir().join(format!("xc-plugin-versions-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let put = |folder: &str, version: &str, app: &str| {
            let dir = base.join(folder).join("acme.tools");
            fs::create_dir_all(&dir).unwrap();
            let engines = if app.is_empty() { String::new() } else { format!("
[engines]
app = \"{app}\"
") };
            fs::write(dir.join("plugin.toml"), format!("id = \"acme.tools\"
name = \"Tools\"
version = \"{version}\"
{engines}")).unwrap();
            base.join(folder)
        };
        let bundled = put("bundled", "1.2.0", "");
        let update = put("updates", "1.10.0", "");
        let engine = BlockEngine::new().unwrap();
        let reg = Registry::load(&[bundled.clone(), update.clone()], &engine);
        assert_eq!(reg.plugins.len(), 1);
        assert_eq!(reg.plugins[0].manifest.version, "1.10.0", "1.10 is newer than 1.2");
        let reg = Registry::load(&[update.clone(), bundled.clone()], &engine);
        assert_eq!(reg.plugins[0].manifest.version, "1.10.0", "whatever the order");

        let future = put("future", "9.0.0", "99.0.0");
        let reg = Registry::load(&[bundled, future], &engine);
        assert_eq!(reg.plugins[0].manifest.version, "1.2.0", "the one for a newer app is left out");
        assert!(reg.problems.iter().any(|p| p.message.contains("needs XC Circuit 99.0.0 or newer")), "{:?}", reg.problems.iter().map(|p| &p.message).collect::<Vec<_>>());
        let _ = fs::remove_dir_all(&base);

        assert_eq!(compare_versions("0.1.0", "0.1"), std::cmp::Ordering::Equal);
        assert_eq!(compare_versions("1.2.0-beta", "1.1.9"), std::cmp::Ordering::Greater);
    }

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

    /// A plugin turned off isn't loaded (its blocks aren't there) but is still listed.
    /// Channel inputs (categories, threads and voice channels too) can be
    /// switched from their pin to a box for the channel's id.
    #[test]
    fn channel_fields_can_take_an_id() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins");
        let engine = BlockEngine::new().unwrap();
        let reg = Registry::load(&[root], &engine);
        for t in ["discord.channel", "discord.thread", "discord.voice_channel", "fluxer.channel"] {
            assert!(reg.id_types.contains(t), "{t}");
        }
        assert!(!reg.id_types.contains("discord.user"));
        for (block, key) in [("discord.delete_channel", "channel"), ("discord.create_channel", "category"), ("fluxer.create_invite", "channel")] {
            let f = &reg.block(block).unwrap_or_else(|| panic!("{block}")).fields[key];
            assert!(f.by_id, "{block}.{key}");
            assert_eq!(f.pin, circuit_core::block::PinMode::Pin, "{block}.{key}: starts as a pin that can be turned off");
        }
        // Fields a block adds itself are marked when its shape is built.
        let mut added = circuit_core::FieldDef { type_name: "discord.channel".into(), pin: circuit_core::block::PinMode::Only, ..Default::default() };
        reg.mark_id_field(&mut added);
        assert!(added.by_id && added.pin == circuit_core::block::PinMode::Pin);
        let mut user = circuit_core::FieldDef { type_name: "discord.user".into(), pin: circuit_core::block::PinMode::Only, ..Default::default() };
        reg.mark_id_field(&mut user);
        assert!(!user.by_id && user.pin == circuit_core::block::PinMode::Only);
    }

    #[test]
    fn plugins_turned_off_are_listed_not_loaded() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins");
        let engine = BlockEngine::new().unwrap();
        let reg = Registry::load_except(&[root], &engine, &["xc.server".to_string()]);
        assert!(reg.plugins.iter().all(|p| p.manifest.id != "xc.server"));
        assert_eq!(reg.disabled.iter().map(|p| p.manifest.id.as_str()).collect::<Vec<_>>(), ["xc.server"]);
        assert!(!reg.blocks.keys().any(|k| k.starts_with("server.")), "its blocks aren't loaded");
        assert!(reg.block("discord.send_message@3").is_some(), "the others are");
    }

    /// Every bundled block sits in one of the Discord mode's categories (not
    /// the folder it happens to be in) and has other names to be found by.
    #[test]
    fn bundled_blocks_have_a_known_category_and_aliases() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins");
        let engine = BlockEngine::new().unwrap();
        let reg = Registry::load(&[root], &engine);
        let mode = reg.mode("discord-bot").unwrap();
        let mut wrong = Vec::new();
        for def in reg.blocks.values().filter(|d| d.plugin == "xc.core" || d.plugin == "xc.discord") {
            let cat = def.category.as_deref().unwrap_or("");
            if mode.categories.iter().all(|c| c.id != cat) {
                wrong.push(format!("{}: unknown category `{cat}`", def.id));
            }
            if def.aliases.is_empty() {
                wrong.push(format!("{}: no aliases", def.id));
            }
        }
        assert!(wrong.is_empty(), "{wrong:#?}");
        // No category is left empty.
        let empty: Vec<&str> = mode.categories.iter().filter(|c| !reg.blocks.values().any(|d| d.category.as_deref() == Some(c.id.as_str()))).map(|c| c.id.as_str()).collect();
        assert!(empty.is_empty(), "categories without blocks: {empty:?}");
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

        // Names used in an embed's ${…} become input pins, except names earlier blocks give.
        let values = json!({ "embed": { "title": "Rank of ${user}", "description": "${xp * 2} XP", "fields": [{ "name": "L", "value": "${level}" }] } });
        let known = ["user".to_string()].into_iter().collect();
        let shape = BlockShape::build_with(reg.block("discord.build_embed").unwrap(), values.as_object().unwrap(), None, &known);
        assert!(shape.field("xp").is_some() && shape.field("level").is_some() && shape.field("user").is_none());

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
