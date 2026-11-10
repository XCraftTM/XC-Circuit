//! Bot › Settings: its pages and their settings, from the build mode
//! (`[[pages]]` and `[settings]` in its TOML) and from plugins
//! (`contributes.botSettings`, files with the same two tables).
//!
//! A setting goes on the page its `page` names. Without one, its `group` is
//! the page (how build modes used to list settings); with both, `group` is a
//! heading on the page.

use indexmap::IndexMap;
use serde::Deserialize;
use serde_json::Value;

use crate::mode::BuildMode;
use crate::registry::Registry;

/// A page of Bot › Settings.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct SettingsPage {
    pub id: String,
    pub name: String,
    /// An icon's name, such as `database`.
    pub icon: Option<String>,
    /// Shown under the page's title.
    pub description: Option<String>,
    /// Lower comes first; pages without one count as 50 and keep the order
    /// they're listed in (the build mode's first).
    pub order: Option<i32>,
}

/// A plugin's part of Bot › Settings (a file listed in `contributes.botSettings`).
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct SettingsFile {
    /// The build modes it is for (`discord`); empty means every one.
    pub modes: Vec<String>,
    pub pages: Vec<SettingsPage>,
    pub settings: IndexMap<String, Value>,
    #[serde(skip)]
    pub plugin: String,
}

/// One setting, placed on its page.
#[derive(Clone, Debug, PartialEq)]
pub struct Setting {
    pub key: String,
    /// As written in the TOML: `type`, `label`, `description` …
    pub spec: Value,
    /// The id of its page.
    pub page: String,
    /// A heading on the page, when the setting has both `page` and `group`.
    pub group: Option<String>,
    /// The plugin it comes from.
    pub plugin: String,
}

impl Setting {
    pub fn text(&self, key: &str) -> Option<&str> {
        self.spec.get(key).and_then(Value::as_str)
    }

    /// `text` (the default), `secret`, `select`, `boolean`, `number`,
    /// `textarea`, `color`, `slider`, `list`, `path`; `note` and `link` show
    /// something and have no value.
    pub fn kind(&self) -> &str {
        self.text("type").unwrap_or("text")
    }

    pub fn is_secret(&self) -> bool {
        self.kind() == "secret"
    }

    /// `private = true`: the value belongs to whoever set it, not to the bot
    /// (the address of a server of theirs, an id of an account of theirs, a
    /// safety switch). It is kept on their computer, not in the shared
    /// project, so a project that is shared or synced neither gives it away
    /// nor brings someone else's along.
    pub fn is_private(&self) -> bool {
        self.spec.get("private").and_then(Value::as_bool).unwrap_or(false)
    }
}

/// Everything Bot › Settings shows for a build mode.
#[derive(Clone, Debug, Default)]
pub struct BotSettings {
    /// In the order they're shown.
    pub pages: Vec<SettingsPage>,
    /// In the order they're shown on their pages.
    pub settings: Vec<Setting>,
    /// Settings that couldn't be shown, such as a key used twice.
    pub problems: Vec<String>,
}

impl BotSettings {
    /// The keys of the private settings (see [`Setting::is_private`]).
    pub fn private_keys(&self) -> Vec<String> {
        self.settings.iter().filter(|s| s.is_private() && !s.is_secret()).map(|s| s.key.clone()).collect()
    }

    pub fn on_page<'a>(&'a self, page: &'a str) -> impl Iterator<Item = &'a Setting> {
        self.settings.iter().filter(move |s| s.page == page)
    }

    /// The values a bot gets for settings that weren't changed: their `default`.
    pub fn defaults(&self) -> IndexMap<String, Value> {
        // Secrets come from the keyring; notes and links only show something.
        self.settings.iter().filter(|s| !s.is_secret() && !matches!(s.kind(), "note" | "link")).filter_map(|s| Some((s.key.clone(), s.spec.get("default")?.clone()))).collect()
    }
}

/// A page id made from a group's name: `Slash commands` → `slash-commands`.
fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    out.trim_end_matches('-').to_string()
}

impl Registry {
    /// Bot › Settings for `mode`: its own pages and settings, then the plugins'.
    pub fn bot_settings(&self, mode: &BuildMode) -> BotSettings {
        let mut out = BotSettings::default();
        let add = |out: &mut BotSettings, plugin: &str, pages: &[SettingsPage], settings: &IndexMap<String, Value>| {
            for p in pages {
                if p.id.trim().is_empty() {
                    out.problems.push(format!("{plugin}: a settings page needs an id"));
                } else if !out.pages.iter().any(|old| old.id == p.id) {
                    out.pages.push(p.clone());
                }
            }
            for (key, spec) in settings {
                if out.settings.iter().any(|s| s.key == *key) {
                    out.problems.push(format!("{plugin}: the setting `{key}` exists already, so this one isn't shown"));
                    continue;
                }
                let text = |k: &str| spec.get(k).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
                let (page, group) = match (text("page"), text("group")) {
                    (Some(page), group) => (page, group),
                    (None, Some(group)) => {
                        let id = slug(&group);
                        if !out.pages.iter().any(|p| p.id == id) {
                            out.pages.push(SettingsPage { id: id.clone(), name: group, ..Default::default() });
                        }
                        (id, None)
                    }
                    (None, None) => {
                        if !out.pages.iter().any(|p| p.id == "general") {
                            out.pages.push(SettingsPage { id: "general".into(), name: "General".into(), ..Default::default() });
                        }
                        ("general".to_string(), None)
                    }
                };
                if !out.pages.iter().any(|p| p.id == page) {
                    out.problems.push(format!("{plugin}: the setting `{key}` is on page `{page}`, which isn't listed in [[pages]]"));
                    out.pages.push(SettingsPage { id: page.clone(), name: page.clone(), ..Default::default() });
                }
                out.settings.push(Setting { key: key.clone(), spec: spec.clone(), page, group, plugin: plugin.to_string() });
            }
        };
        add(&mut out, &mode.plugin, &mode.pages, &mode.settings);
        for file in self.bot_settings.iter().filter(|f| f.modes.is_empty() || f.modes.contains(&mode.id)) {
            add(&mut out, &file.plugin, &file.pages, &file.settings);
        }
        // Pages without settings aren't shown; the rest by `order`, keeping the listed order.
        out.pages.retain(|p| out.settings.iter().any(|s| s.page == p.id));
        out.pages.sort_by_key(|p| p.order.unwrap_or(50));
        // Settings in page order, each page's in the order they were listed.
        let rank = |s: &Setting| out.pages.iter().position(|p| p.id == s.page).unwrap_or(usize::MAX);
        let mut settings = std::mem::take(&mut out.settings);
        settings.sort_by_key(|s| rank(s));
        out.settings = settings;
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mode(toml_text: &str) -> BuildMode {
        let mut m: BuildMode = toml::from_str(toml_text).unwrap();
        m.plugin = "test.mode".into();
        m
    }

    #[test]
    fn groups_become_pages() {
        let m = mode(
            r#"
            id = "discord"
            [settings]
            token = { type = "secret", group = "Connection" }
            devServer = { group = "Slash commands" }
            shards = { group = "Connection" }
            prefix = {}
            "#,
        );
        let s = Registry::default().bot_settings(&m);
        let ids: Vec<&str> = s.pages.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["connection", "slash-commands", "general"]);
        assert_eq!(s.pages[1].name, "Slash commands");
        let keys: Vec<&str> = s.settings.iter().map(|x| x.key.as_str()).collect();
        assert_eq!(keys, ["token", "shards", "devServer", "prefix"]);
        assert!(s.settings[0].is_secret() && s.settings[0].group.is_none());
        assert!(s.problems.is_empty());
    }

    #[test]
    fn plugins_add_pages() {
        let m = mode(
            r#"
            id = "discord"
            [[pages]]
            id = "connection"
            name = "Connection"
            icon = "plug"
            order = 10
            [settings]
            token = { type = "secret", page = "connection" }
            prefix = { group = "Messages", default = "!" }
            "#,
        );
        let mut reg = Registry::default();
        let file = |text: &str, plugin: &str| {
            let mut f: SettingsFile = toml::from_str(text).unwrap();
            f.plugin = plugin.into();
            f
        };
        reg.bot_settings.push(file(
            r#"
            [[pages]]
            id = "databases"
            name = "Databases"
            icon = "database"
            order = 20
            [[pages]]
            id = "empty"
            name = "Nothing here"
            [settings]
            dbDefault = { type = "select", page = "databases", group = "Default", choices = { json = "JSON file" }, default = "json" }
            dbPassword = { type = "secret", page = "databases" }
            prefix = { label = "Taken already" }
            "#,
            "xc.database",
        ));
        reg.bot_settings.push(file("modes = [\"fluxer\"]\n[settings]\nfluxerOnly = {}\n", "xc.other"));
        let s = reg.bot_settings(&m);
        let ids: Vec<&str> = s.pages.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["connection", "databases", "messages"], "by order; empty pages and other modes' left out");
        let keys: Vec<&str> = s.settings.iter().map(|x| x.key.as_str()).collect();
        assert_eq!(keys, ["token", "dbDefault", "dbPassword", "prefix"]);
        assert_eq!(s.settings[1].group.as_deref(), Some("Default"));
        assert_eq!(s.settings[1].plugin, "xc.database");
        assert_eq!(s.problems.len(), 1, "{:?}", s.problems);
        assert!(s.problems[0].contains("`prefix`"));
        let defaults = s.defaults();
        assert_eq!(defaults.get("prefix"), Some(&Value::from("!")));
        assert_eq!(defaults.get("dbDefault"), Some(&Value::from("json")));
        assert!(!defaults.contains_key("token"));
    }

    #[test]
    fn unknown_pages_still_show() {
        let m = mode("id = \"x\"\n[settings]\na = { page = \"nowhere\" }\n");
        let s = Registry::default().bot_settings(&m);
        assert_eq!(s.pages[0].id, "nowhere");
        assert_eq!(s.problems.len(), 1);
    }

    #[test]
    fn slugs() {
        assert_eq!(slug("Slash commands"), "slash-commands");
        assert_eq!(slug("Voice & Music"), "voice-music");
        assert_eq!(slug("  Connection "), "connection");
    }
}
