//! Translation files (`contributes.locales`): the editor's texts and a
//! plugin's block names in another language, keyed by the English text.

use indexmap::IndexMap;
use serde::Deserialize;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct LocaleDef {
    pub meta: LocaleMeta,
    /// Interface texts: English → translation. Empty translations are skipped.
    pub strings: IndexMap<String, String>,
    /// Block field, input and output labels: English → translation.
    pub labels: IndexMap<String, String>,
    /// Block names and descriptions, by block id (`core.math`).
    pub blocks: IndexMap<String, BlockText>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct LocaleMeta {
    /// Language code: `de`, `fr`, `pt-BR`.
    pub code: String,
    /// The language's name in itself: `Deutsch`.
    pub name: String,
    /// The language's name in English: `German`.
    pub english_name: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct BlockText {
    pub name: String,
    pub description: String,
}

impl LocaleDef {
    /// Adds another file's entries for the same language; later files win.
    pub fn merge(&mut self, other: LocaleDef) {
        if self.meta.name.is_empty() {
            self.meta.name = other.meta.name;
        }
        if self.meta.english_name.is_empty() {
            self.meta.english_name = other.meta.english_name;
        }
        let filled = |map: IndexMap<String, String>| map.into_iter().filter(|(_, v)| !v.trim().is_empty());
        self.strings.extend(filled(other.strings));
        self.labels.extend(filled(other.labels));
        self.blocks.extend(other.blocks);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merges_files_of_one_language() {
        let mut a: LocaleDef = toml::from_str(
            r#"
            [meta]
            code = "de"
            name = "Deutsch"
            [strings]
            "Save" = "Speichern"
            "Cancel" = "Abbrechen"
            "#,
        )
        .unwrap();
        let b: LocaleDef = toml::from_str(
            r#"
            [meta]
            code = "de"
            english_name = "German"
            [strings]
            "Cancel" = "Abbruch"
            "Open" = ""
            [labels]
            "Message" = "Nachricht"
            [blocks."core.math"]
            name = "Mathe"
            "#,
        )
        .unwrap();
        a.merge(b);
        assert_eq!(a.meta.name, "Deutsch");
        assert_eq!(a.meta.english_name, "German");
        assert_eq!(a.strings["Cancel"], "Abbruch", "later files win");
        assert!(!a.strings.contains_key("Open"), "empty entries stay English");
        assert_eq!(a.labels["Message"], "Nachricht");
        assert_eq!(a.blocks["core.math"].name, "Mathe");
        assert_eq!(a.blocks["core.math"].description, "");
    }
}
