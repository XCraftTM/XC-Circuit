//! Pin types contributed by plugins (`types/*.toml`).

use std::collections::HashMap;

use serde::Deserialize;

#[derive(Clone, Debug, Default, Deserialize)]
pub struct PinTypeFile {
    #[serde(default, rename = "type")]
    pub types: Vec<PinTypeDef>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct PinTypeDef {
    pub id: String,
    pub name: String,
    /// The type this one fits into, such as `object`.
    pub extends: Option<String>,
    pub color: Option<String>,
    pub icon: Option<String>,
    /// JavaScript predicate the runtime uses to check values.
    pub check: Option<String>,
    pub description: Option<String>,
    pub converts: Vec<Conversion>,
    /// JavaScript `(id, ctx) => value` the runtime uses to find a value by
    /// its id (a channel by its id), for inputs given an id instead.
    #[serde(rename = "fromId")]
    pub from_id: Option<String>,
}

/// A class catalog (`contributes.classes`): what the values behind pin types
/// have, made from a library's type definitions (see the Discord plugin's
/// `tools/discordjs-catalog.js`).
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct ClassFile {
    /// What it was made from: `discord.js 14.28.0`.
    pub source: String,
    /// The class behind each pin type: `discord.user` → `User`.
    pub roots: HashMap<String, String>,
    pub classes: HashMap<String, ClassDef>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct ClassDef {
    pub members: Vec<Member>,
}

/// A property, getter or read-only method.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Member {
    pub name: String,
    /// `property`, `getter` or `method`.
    pub kind: String,
    /// How its type is shown: Text, Number, Yes/no, Date or a class name.
    #[serde(rename = "type")]
    pub ty: String,
    /// It can be empty (null).
    pub optional: bool,
    /// The class its value is, when it has members of its own.
    pub class: Option<String>,
    pub doc: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Conversion {
    pub to: String,
    pub via: String,
}
