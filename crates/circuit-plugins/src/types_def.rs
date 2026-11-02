//! Pin types contributed by plugins (`types/*.toml`).

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
    /// Properties texts can use after a dot, such as `{user.username}`.
    pub properties: Vec<Property>,
}

/// A property of a value of this type: its path (`roles.highest.name`), its
/// name and its type.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Property {
    pub path: String,
    pub name: String,
    #[serde(rename = "type")]
    pub ty: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Conversion {
    pub to: String,
    pub via: String,
}
