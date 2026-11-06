//! Discord's languages, and the Translations list builders use for names and
//! descriptions that Discord shows in each person's language.

use std::collections::BTreeMap;

use eframe::egui::{self, RichText, Ui, vec2};
use serde::{Deserialize, Serialize};

use crate::ui::kit;
use crate::i18n::n;
use crate::ui::theme::Theme;

/// The languages Discord can show, by its locale code.
pub const DISCORD_LOCALES: [(&str, &str); 32] = [
    ("bg", n("Bulgarian")),
    ("cs", n("Czech")),
    ("da", n("Danish")),
    ("de", n("German")),
    ("el", n("Greek")),
    ("en-GB", n("English (UK)")),
    ("en-US", n("English (US)")),
    ("es-ES", n("Spanish")),
    ("es-419", n("Spanish (Latin America)")),
    ("fi", n("Finnish")),
    ("fr", n("French")),
    ("hi", n("Hindi")),
    ("hr", n("Croatian")),
    ("hu", n("Hungarian")),
    ("id", n("Indonesian")),
    ("it", n("Italian")),
    ("ja", n("Japanese")),
    ("ko", n("Korean")),
    ("lt", n("Lithuanian")),
    ("nl", n("Dutch")),
    ("no", n("Norwegian")),
    ("pl", n("Polish")),
    ("pt-BR", n("Portuguese (Brazil)")),
    ("ro", n("Romanian")),
    ("ru", n("Russian")),
    ("sv-SE", n("Swedish")),
    ("th", n("Thai")),
    ("tr", n("Turkish")),
    ("uk", n("Ukrainian")),
    ("vi", n("Vietnamese")),
    ("zh-CN", n("Chinese (China)")),
    ("zh-TW", n("Chinese (Taiwan)")),
];

/// A language's name in the editor's language, with its code: "German · de".
pub fn locale_label(code: &str) -> String {
    match DISCORD_LOCALES.iter().find(|(c, _)| *c == code) {
        Some((c, name)) => format!("{} · {c}", crate::i18n::t(name)),
        None => code.to_string(),
    }
}

/// A name (and description) in one language.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Localized {
    pub name: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub description: String,
}

/// The Translations list: one row per language, with a name and (when
/// `description` is set) a description. Returns whether anything changed.
pub fn editor(ui: &mut Ui, theme: &Theme, id: egui::Id, map: &mut BTreeMap<String, Localized>, description: bool) -> bool {
    let mut rows: Vec<(String, Localized)> = map.clone().into_iter().collect();
    let mut changed = false;
    let mut remove = None;
    let h = crate::ui::kit::input_height(ui);
    let width = ui.available_width();
    for (i, (code, text)) in rows.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            ui.set_min_height(h);
            let taken: Vec<String> = map.keys().filter(|k| *k != code).cloned().collect();
            crate::ui::kit::dropdown(ui, id.with(("locale", i)), 180.0, locale_label(code), |ui| {
                egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                    for (c, _) in DISCORD_LOCALES.iter().filter(|(c, _)| !taken.iter().any(|t| t == c)) {
                        if kit::option(ui, code == c, locale_label(c)).clicked() && code != c {
                            *code = c.to_string();
                            changed = true;
                        }
                    }
                });
            })
            .accessible_name(t!("Language"));
            let rest = (width - 180.0 - 110.0).max(160.0);
            let name_w = if description { (rest * 0.4).floor() } else { rest };
            changed |= ui.add(crate::ui::kit::TextLine::new(&mut text.name).hint(t!("Name")).width(name_w)).changed();
            if description {
                changed |= ui.add(crate::ui::kit::TextLine::new(&mut text.description).hint(t!("Description")).width(rest - name_w - 8.0)).changed();
            }
            if kit::danger_button(ui, theme, t!("Remove")).clicked() {
                remove = Some(i);
            }
        });
    }
    if let Some(i) = remove {
        rows.remove(i);
        changed = true;
    }
    let free = DISCORD_LOCALES.iter().map(|(c, _)| *c).find(|c| !rows.iter().any(|(r, _)| r == c));
    ui.horizontal(|ui| {
        if let Some(code) = free
            && kit::button(ui, theme, t!("+ Add language")).clicked()
        {
            rows.push((code.to_string(), Localized::default()));
            changed = true;
        }
        if rows.is_empty() {
            kit::hint(ui, theme, t!("People whose Discord is set to another language see these instead."));
        }
    });
    ui.add_space(4.0);
    if changed {
        *map = rows.into_iter().collect();
    }
    changed
}

/// A collapsible Translations section around [`editor`], with the count in its title.
pub fn section(ui: &mut Ui, theme: &Theme, id: egui::Id, map: &mut BTreeMap<String, Localized>, description: bool) {
    let title = if map.is_empty() { t!("Translations").to_string() } else { tf!("Translations · {count}", count = map.len()) };
    egui::CollapsingHeader::new(RichText::new(title).size(12.5).color(theme.text)).id_salt(id).show(ui, |ui| {
        ui.spacing_mut().item_spacing = vec2(8.0, 6.0);
        editor(ui, theme, id.with("rows"), map, description);
    });
}

/// Problems with a translated name or description, by Discord's rules: names
/// are 1–32 characters without spaces or capitals, descriptions 1–100.
pub fn problems(map: &BTreeMap<String, Localized>, description: bool, max_name: usize, command_name: bool) -> Vec<String> {
    let mut out = Vec::new();
    for (code, l) in map {
        let n = l.name.chars().count();
        if n == 0 || n > max_name {
            out.push(tf!("{language}: the name must be 1–{max} characters", language = code, max = max_name));
        } else if command_name && l.name.chars().any(|c| c.is_whitespace() || c.is_uppercase()) {
            out.push(tf!("{language}: the name can't have spaces or capitals", language = code));
        }
        if description && (l.description.trim().is_empty() || l.description.chars().count() > 100) {
            out.push(tf!("{language}: the description must be 1–100 characters", language = code));
        }
    }
    out
}

/// The JSON form Discord uses: `{ "de": "…" }` from each translation's name or description.
pub fn to_discord(map: &BTreeMap<String, Localized>, description: bool) -> Option<serde_json::Value> {
    let out: serde_json::Map<String, serde_json::Value> = map
        .iter()
        .map(|(c, l)| (c.clone(), if description { &l.description } else { &l.name }))
        .filter(|(_, s)| !s.is_empty())
        .map(|(c, s)| (c, serde_json::Value::from(s.clone())))
        .collect();
    (!out.is_empty()).then_some(serde_json::Value::Object(out))
}

/// Reads Discord's `name_localizations` and `description_localizations` back.
pub fn from_discord(names: Option<&serde_json::Value>, descriptions: Option<&serde_json::Value>) -> BTreeMap<String, Localized> {
    let mut out: BTreeMap<String, Localized> = BTreeMap::new();
    for (v, is_desc) in [(names, false), (descriptions, true)] {
        for (code, text) in v.and_then(|v| v.as_object()).into_iter().flatten() {
            let Some(text) = text.as_str() else { continue };
            let entry = out.entry(code.clone()).or_default();
            if is_desc {
                entry.description = text.into();
            } else {
                entry.name = text.into();
            }
        }
    }
    out
}
