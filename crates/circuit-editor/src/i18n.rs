//! Translations. Interface texts are written in English in the code, wrapped
//! in `t!("…")` (or `tf!` when they have `{placeholders}`, or `n("…")` where
//! a macro can't run, such as in constants, then `t()` where shown), and
//! looked up in the chosen language. Languages come from plugin locale files
//! (`contributes.locales`); a missing entry shows the English text.

use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt::Display;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use circuit_plugins::Registry;

/// `t!("English text")`: the text in the current language.
macro_rules! t {
    ($s:literal) => {
        $crate::i18n::t($s)
    };
}

/// `tf!("{count} to fix", count = n)`: the text in the current language,
/// with its placeholders filled in.
macro_rules! tf {
    ($s:literal $(, $k:ident = $v:expr)* $(,)?) => {
        $crate::i18n::tf($s, &[$((stringify!($k), &$v as &dyn std::fmt::Display)),*])
    };
}

/// The texts the code wraps in `t!`, `tf!` or `n`; kept current by a test.
pub use crate::ui_strings::UI_STRINGS;

/// The language everything is written in.
pub const ENGLISH: &str = "en";

struct Catalog {
    /// Values are leaked: `t` hands out `&'static str`. Only a language change
    /// loads new ones, so this stays small.
    strings: HashMap<String, &'static str>,
    labels: HashMap<String, String>,
}

static CURRENT: RwLock<Option<Catalog>> = RwLock::new(None);

/// The text in the current language.
pub fn t(english: &'static str) -> &'static str {
    match CURRENT.read() {
        Ok(guard) => guard.as_ref().and_then(|c| c.strings.get(english).copied()).unwrap_or(english),
        Err(_) => english,
    }
}

/// Marks a text for translation where `t` can't be called (constants); pass
/// it through `t` where it is shown.
pub const fn n(english: &'static str) -> &'static str {
    english
}

/// `t` with `{name}` placeholders filled in.
pub fn tf(english: &'static str, args: &[(&str, &dyn Display)]) -> String {
    let mut s = t(english).to_string();
    for (name, value) in args {
        s = s.replace(&format!("{{{name}}}"), &value.to_string());
    }
    s
}

/// A text that isn't a literal in the code (a mode's category name, say).
pub fn tr(english: &str) -> Cow<'_, str> {
    match CURRENT.read() {
        Ok(guard) => match guard.as_ref().and_then(|c| c.strings.get(english)) {
            Some(s) => Cow::Owned(s.to_string()),
            None => Cow::Borrowed(english),
        },
        Err(_) => Cow::Borrowed(english),
    }
}

/// A block's field, input or output label.
pub fn label(english: &str) -> Cow<'_, str> {
    match CURRENT.read() {
        Ok(guard) => match guard.as_ref().and_then(|c| c.labels.get(english)) {
            Some(s) => Cow::Owned(s.clone()),
            None => Cow::Borrowed(english),
        },
        Err(_) => Cow::Borrowed(english),
    }
}

/// The languages to choose from: English, then every language a plugin
/// translates to, as (code, name in that language).
pub fn languages(reg: &Registry) -> Vec<(String, String)> {
    let mut list = vec![(ENGLISH.to_string(), "English".to_string())];
    for l in &reg.locales {
        if l.meta.code != ENGLISH && !list.iter().any(|(c, _)| *c == l.meta.code) {
            let name = if l.meta.name.is_empty() { l.meta.code.clone() } else { l.meta.name.clone() };
            list.push((l.meta.code.clone(), name));
        }
    }
    list
}

/// Block names and descriptions as the plugins wrote them, to switch back to.
pub type BlockEnglish = HashMap<String, (String, String)>;

pub fn block_english(reg: &Registry) -> BlockEnglish {
    reg.blocks.iter().map(|(id, b)| (id.clone(), (b.name.clone(), b.description.clone()))).collect()
}

/// Switches the interface and the registry's block texts to `code`. An
/// unknown code means English.
pub fn set_language(reg: &mut Registry, english: &BlockEnglish, code: &str) {
    let locale = reg.locales.iter().find(|l| l.meta.code == code && code != ENGLISH).cloned();
    for (id, block) in reg.blocks.iter_mut() {
        let Some((name, description)) = english.get(id) else { continue };
        let text = locale.as_ref().and_then(|l| l.blocks.get(id));
        let pick = |translated: Option<&String>, original: &String| translated.filter(|s| !s.trim().is_empty()).unwrap_or(original).clone();
        block.name = pick(text.map(|t| &t.name), name);
        block.description = pick(text.map(|t| &t.description), description);
    }
    let catalog = locale.map(|l| Catalog {
        strings: l.strings.into_iter().filter(|(_, v)| !v.trim().is_empty()).map(|(k, v)| (k, &*Box::leak(v.into_boxed_str()))).collect(),
        labels: l.labels.into_iter().filter(|(_, v)| !v.trim().is_empty()).collect(),
    });
    if let Ok(mut current) = CURRENT.write() {
        *current = catalog;
    }
}

/// Writes a translation file with every text to fill in: the interface, the
/// block labels and each block's name and description. Entries of `base`
/// (a language being extended) are filled in already; the rest are empty
/// and show in English until translated.
pub fn template(reg: &Registry, english: &BlockEnglish, code: &str, name: &str, english_name: &str) -> String {
    let base = reg.locales.iter().find(|l| l.meta.code == code);
    let q = |s: &str| toml_quote(s);
    let mut out = String::new();
    out.push_str("# An XC Circuit translation. Fill in the text after each = sign; entries\n");
    out.push_str("# left empty show in English. Placeholders such as {count} stay as they are.\n");
    out.push_str("# Saved changes show right away while XC Circuit is open.\n\n");
    out.push_str(&format!("[meta]\ncode = {}\nname = {}\nenglish_name = {}\n\n", q(code), q(name), q(english_name)));

    out.push_str("[strings]\n");
    let mut strings: Vec<String> = UI_STRINGS.iter().map(|s| s.to_string()).collect();
    for mode in &reg.modes {
        strings.extend(mode.categories.iter().map(|c| c.name.clone()));
        for spec in mode.settings.values() {
            for key in ["group", "label", "description", "placeholder", "patternHint"] {
                if let Some(s) = spec.get(key).and_then(|v| v.as_str()).filter(|s| s.chars().any(char::is_alphabetic)) {
                    strings.push(s.to_string());
                }
            }
        }
    }
    let mut seen = std::collections::HashSet::new();
    for s in strings.into_iter().filter(|s| seen.insert(s.clone())) {
        let have = base.and_then(|b| b.strings.get(&s)).map(String::as_str).unwrap_or("");
        out.push_str(&format!("{} = {}\n", q(&s), q(have)));
    }

    out.push_str("\n[labels]\n");
    let mut labels: Vec<String> = Vec::new();
    for b in reg.blocks.values() {
        for (key, f) in &b.fields {
            labels.push(f.label(key));
            for (group, _, text) in f.choice_list() {
                labels.extend(group);
                labels.push(text);
            }
        }
        for (key, o) in &b.outputs {
            labels.push(o.label(key));
        }
    }
    labels.sort();
    labels.dedup();
    for l in labels {
        let have = base.and_then(|b| b.labels.get(&l)).map(String::as_str).unwrap_or("");
        out.push_str(&format!("{} = {}\n", q(&l), q(have)));
    }

    let mut ids: Vec<&String> = english.keys().collect();
    ids.sort();
    for id in ids {
        let (en_name, en_desc) = &english[id];
        let have = base.and_then(|b| b.blocks.get(id));
        out.push_str(&format!("\n# {en_name}: {}\n", en_desc.replace('\n', " ")));
        out.push_str(&format!("[blocks.{}]\n", q(id)));
        out.push_str(&format!("name = {}\n", q(have.map(|h| h.name.as_str()).unwrap_or(""))));
        out.push_str(&format!("description = {}\n", q(have.map(|h| h.description.as_str()).unwrap_or(""))));
    }
    out
}

fn toml_quote(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Creates a plugin that holds a new translation in the user's plugin
/// folder and returns the file to edit. An existing file is kept.
pub fn create_translation(plugins_dir: &Path, reg: &Registry, english: &BlockEnglish, code: &str, name: &str, english_name: &str) -> anyhow::Result<PathBuf> {
    let code = code.trim();
    anyhow::ensure!(!code.is_empty() && code.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'), "a language code is letters such as de, fr or pt-BR");
    let dir = plugins_dir.join(format!("translation-{}", code.to_lowercase()));
    let file = dir.join("locales").join(format!("{code}.toml"));
    if file.exists() {
        return Ok(file);
    }
    std::fs::create_dir_all(file.parent().unwrap())?;
    let title = if english_name.trim().is_empty() { name } else { english_name };
    let manifest = format!(
        "id          = {}\nname        = {}\nversion     = \"1.0.0\"\nauthor      = \"\"\ndescription = {}\n\n[engines]\ncircuit = \"^1.0\"\n\n[contributes]\nlocales = [\"locales/*.toml\"]\n",
        toml_quote(&format!("translation.{}", code.to_lowercase())),
        toml_quote(&format!("{title} translation")),
        toml_quote(&format!("XC Circuit in {title}")),
    );
    std::fs::write(dir.join("plugin.toml"), manifest)?;
    std::fs::write(&file, template(reg, english, code, name, english_name))?;
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use circuit_plugins::LocaleDef;

    /// Every text wrapped in `t!`, `tf!` or `n` in the editor's code, read from the source files.
    fn wrapped_in_source() -> Vec<String> {
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut found = Vec::new();
        let mut files: Vec<PathBuf> = std::fs::read_dir(&src).unwrap().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "rs")).collect();
        files.sort();
        for path in files {
            if path.file_name().is_some_and(|f| f == "ui_strings.rs" || f == "i18n.rs") {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            found.extend(literals(&text));
        }
        let mut seen = std::collections::HashSet::new();
        found.retain(|s| seen.insert(s.clone()));
        found
    }

    /// The string literals passed to `t!(`, `tf!(` and `n(`, as written in the source (escapes kept).
    fn literals(text: &str) -> Vec<String> {
        let bytes = text.as_bytes();
        let mut out = Vec::new();
        for marker in ["t!(\"", "tf!(\"", "n(\""] {
            let mut from = 0;
            while let Some(i) = text[from..].find(marker) {
                let start = from + i;
                from = start + marker.len();
                // A whole word: `t!(` but not `print!(`; `n(` but not `open(`.
                if start > 0 && (bytes[start - 1].is_ascii_alphanumeric() || bytes[start - 1] == b'_') {
                    continue;
                }
                let mut j = from;
                let mut escaped = false;
                while j < bytes.len() {
                    match bytes[j] {
                        b'\\' if !escaped => escaped = true,
                        b'"' if !escaped => break,
                        _ => escaped = false,
                    }
                    j += 1;
                }
                out.push(text[from..j].to_string());
                from = j;
            }
        }
        out
    }

    fn unescape(s: &str) -> String {
        s.replace("\\\"", "\"").replace("\\n", "\n").replace("\\\\", "\\")
    }

    #[test]
    fn the_string_list_is_current() {
        let found = wrapped_in_source();
        let listed: Vec<String> = UI_STRINGS.iter().map(|s| s.to_string()).collect();
        let found_plain: Vec<String> = found.iter().map(|s| unescape(s)).collect();
        if found_plain != listed {
            if std::env::var_os("XC_UPDATE_STRINGS").is_some() {
                let mut file = String::from("//! Every interface text wrapped in `t!`, `tf!` or `n`, for translation\n//! templates and checks. Generated: run the tests with XC_UPDATE_STRINGS=1.\n\npub const UI_STRINGS: &[&str] = &[\n");
                for s in &found {
                    file.push_str(&format!("    \"{s}\",\n"));
                }
                file.push_str("];\n");
                std::fs::write(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ui_strings.rs"), file).unwrap();
            } else {
                panic!("src/ui_strings.rs is out of date: run the tests with XC_UPDATE_STRINGS=1");
            }
        }
    }

    #[test]
    fn german_translates_everything() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins/xc.core/locales/de.toml");
        let de: LocaleDef = toml::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let missing: Vec<&str> = UI_STRINGS.iter().copied().filter(|s| de.strings.get(*s).is_none_or(|v| v.trim().is_empty())).collect();
        assert!(missing.is_empty(), "German is missing {} texts:\n{}", missing.len(), missing.join("\n"));
        // Placeholders survive translation.
        for (en, de_text) in &de.strings {
            for part in en.split('{').skip(1) {
                if let Some(name) = part.split('}').next().filter(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')) {
                    assert!(de_text.contains(&format!("{{{name}}}")), "{en:?} → {de_text:?} lost {{{name}}}");
                }
            }
        }
    }

    #[test]
    fn switching_language_and_back() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut ed = crate::editor::Editor::new(vec![root.join("plugins")], None).unwrap();
        let english = block_english(&ed.reg);
        assert!(languages(&ed.reg).iter().any(|(c, n)| c == "de" && n == "Deutsch"));
        set_language(&mut ed.reg, &english, "de");
        assert_eq!(t("Save and close"), "Speichern und schließen");
        assert_eq!(tf("{count} to fix", &[("count", &3)]), "3 zu beheben");
        assert_ne!(ed.reg.block("core.math").unwrap().name, "Math", "block names are translated");
        set_language(&mut ed.reg, &english, ENGLISH);
        assert_eq!(t("Save and close"), "Save and close");
        assert_eq!(ed.reg.block("core.math").unwrap().name, "Math");
    }

    /// Writes a full template for `XC_TEMPLATE_CODE` (default `de`) to `XC_TEMPLATE_OUT`:
    /// `XC_TEMPLATE_OUT=de.toml cargo test -p circuit-editor write_template -- --ignored`
    #[test]
    #[ignore]
    fn write_template() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let ed = crate::editor::Editor::new(vec![root.join("plugins")], None).unwrap();
        let code = std::env::var("XC_TEMPLATE_CODE").unwrap_or_else(|_| "de".into());
        let out = std::env::var("XC_TEMPLATE_OUT").expect("XC_TEMPLATE_OUT");
        std::fs::write(out, template(&ed.reg, &block_english(&ed.reg), &code, "", "")).unwrap();
    }

    #[test]
    fn templates_list_everything_and_keep_existing_entries() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let ed = crate::editor::Editor::new(vec![root.join("plugins")], None).unwrap();
        let english = block_english(&ed.reg);
        let fresh: LocaleDef = toml::from_str(&template(&ed.reg, &english, "fr", "Français", "French")).unwrap();
        assert_eq!(fresh.meta.code, "fr");
        assert!(fresh.strings.len() >= UI_STRINGS.len(), "every interface text is listed");
        assert!(fresh.strings.values().all(String::is_empty));
        assert!(fresh.blocks.contains_key("core.math"));
        let german: LocaleDef = toml::from_str(&template(&ed.reg, &english, "de", "Deutsch", "German")).unwrap();
        assert_eq!(german.strings["Save and close"], "Speichern und schließen", "extending a language starts from what it has");

        let dir = std::env::temp_dir().join(format!("xc-translation-{}", std::process::id()));
        let file = create_translation(&dir, &ed.reg, &english, "fr", "Français", "French").unwrap();
        assert!(file.ends_with("translation-fr/locales/fr.toml"));
        assert!(dir.join("translation-fr/plugin.toml").is_file());
        assert!(create_translation(&dir, &ed.reg, &english, "../x", "", "").is_err());
        let _ = std::fs::remove_dir_all(dir);
    }
}
