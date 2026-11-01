//! Pin types: JavaScript values underneath, friendly names on top.

use std::fmt;

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum PinType {
    Action,
    Text,
    Number,
    Boolean,
    Date,
    #[default]
    Any,
    Object,
    List(Box<PinType>),
    Optional(Box<PinType>),
    Union(Vec<PinType>),
    /// A plugin type such as `discord.member`, or a field type such as `embed`.
    Named(String),
}

/// How well a value of one type fits a pin of another.
/// Ordered from worst to best so `max`/`min` pick the best or worst match.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Compat {
    /// Refused.
    No,
    /// Connects, but the runtime checks the value (`any` into a typed pin).
    Warn,
    /// Connects through an automatic conversion (number → text).
    Convert,
    /// Same type or a subtype.
    Yes,
}

/// What the type checker needs to know about plugin types.
pub trait TypeInfo {
    /// The type a named type extends, such as `object` for `discord.member`.
    fn parent(&self, id: &str) -> Option<PinType>;
    /// Whether a plugin declared an automatic conversion between two named types.
    fn converts(&self, from: &str, to: &str) -> bool;
    /// Display name of a named type.
    fn display_name(&self, id: &str) -> Option<String>;
}

/// Type information with no plugin types at all.
pub struct NoTypes;

impl TypeInfo for NoTypes {
    fn parent(&self, _: &str) -> Option<PinType> {
        None
    }
    fn converts(&self, _: &str, _: &str) -> bool {
        false
    }
    fn display_name(&self, _: &str) -> Option<String> {
        None
    }
}

impl PinType {
    /// Parses a type string such as `list<discord.role>`, `text | number` or `discord.guild?`.
    pub fn parse(s: &str) -> PinType {
        let s = s.trim();
        if s.is_empty() {
            return PinType::Any;
        }
        let parts = split_top_level(s, '|');
        if parts.len() > 1 {
            return PinType::Union(parts.into_iter().map(PinType::parse).collect());
        }
        if let Some(inner) = s.strip_suffix('?') {
            return PinType::Optional(Box::new(PinType::parse(inner)));
        }
        if let Some(inner) = s.strip_prefix("list<").and_then(|r| r.strip_suffix('>')) {
            return PinType::List(Box::new(PinType::parse(inner)));
        }
        if s == "object" || s.starts_with("object<") {
            return PinType::Object;
        }
        match s {
            "action" => PinType::Action,
            "text" | "string" | "textarea" | "select" | "color" | "code" | "file" => PinType::Text,
            "number" => PinType::Number,
            "boolean" | "bool" => PinType::Boolean,
            "date" => PinType::Date,
            "any" | "unknown" => PinType::Any,
            other => PinType::Named(other.to_string()),
        }
    }

    pub fn is_action(&self) -> bool {
        matches!(self, PinType::Action)
    }

    /// The type without an `Optional` wrapper.
    pub fn required(&self) -> &PinType {
        match self {
            PinType::Optional(t) => t.required(),
            other => other,
        }
    }

    pub fn is_optional(&self) -> bool {
        matches!(self, PinType::Optional(_))
    }

    /// Friendly name, such as "List of Role".
    pub fn display(&self, info: &dyn TypeInfo) -> String {
        match self {
            PinType::Action => "Action".into(),
            PinType::Text => "Text".into(),
            PinType::Number => "Number".into(),
            PinType::Boolean => "Boolean".into(),
            PinType::Date => "Date".into(),
            PinType::Any => "Any".into(),
            PinType::Object => "Object".into(),
            PinType::List(t) => format!("List of {}", t.display(info)),
            PinType::Optional(t) => format!("{} (optional)", t.display(info)),
            PinType::Union(ts) => ts.iter().map(|t| t.display(info)).collect::<Vec<_>>().join(" or "),
            PinType::Named(id) => info.display_name(id).unwrap_or_else(|| id.clone()),
        }
    }

    /// Checks whether a value of type `self` may connect to a pin of type `to`.
    pub fn compat(&self, to: &PinType, info: &dyn TypeInfo) -> Compat {
        use PinType::*;
        if self.is_action() || to.is_action() {
            return if self.is_action() && to.is_action() { Compat::Yes } else { Compat::No };
        }
        if self == to || matches!(to, Any) {
            return Compat::Yes;
        }
        if matches!(self, Any) {
            return Compat::Warn;
        }
        match (self, to) {
            (_, Optional(t)) => self.required().compat(t, info),
            (Optional(f), _) => f.compat(to, info).min(Compat::Warn),
            (_, Union(ts)) => ts.iter().map(|t| self.compat(t, info)).max().unwrap_or(Compat::No),
            (Union(fs), _) => fs.iter().map(|f| f.compat(to, info)).min().unwrap_or(Compat::No),
            (List(a), List(b)) => a.compat(b, info),
            (Named(a), Named(b)) => {
                if info.converts(a, b) {
                    return Compat::Convert;
                }
                match info.parent(a) {
                    Some(parent) => parent.compat(to, info).min(Compat::Yes),
                    None => Compat::No,
                }
            }
            // Unknown named types are objects; known ones walk their `extends` chain.
            (Named(a), Object) => match info.parent(a) {
                Some(parent) => parent.compat(to, info),
                None => Compat::Yes,
            },
            (Named(_), Text) => Compat::Convert,
            (Number | Boolean | Date | Object, Text) => Compat::Convert,
            (Date, Number) => Compat::Convert,
            _ => Compat::No,
        }
    }
}

impl fmt::Display for PinType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.display(&NoTypes))
    }
}

/// Splits on `sep` outside of `<…>` and `{…}`.
fn split_top_level(s: &str, sep: char) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut start = 0;
    for (i, c) in s.char_indices() {
        match c {
            '<' | '{' => depth += 1,
            '>' | '}' => depth -= 1,
            c if c == sep && depth == 0 => {
                parts.push(&s[start..i]);
                start = i + c.len_utf8();
            }
            _ => {}
        }
    }
    parts.push(&s[start..]);
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nested_types() {
        assert_eq!(
            PinType::parse("list<discord.role>"),
            PinType::List(Box::new(PinType::Named("discord.role".into())))
        );
        assert_eq!(
            PinType::parse("text | number"),
            PinType::Union(vec![PinType::Text, PinType::Number])
        );
        assert_eq!(
            PinType::parse("discord.guild?"),
            PinType::Optional(Box::new(PinType::Named("discord.guild".into())))
        );
        assert_eq!(PinType::parse("object<{ a: text | number }>"), PinType::Object);
    }

    #[test]
    fn connection_rules() {
        let i = NoTypes;
        assert_eq!(PinType::Number.compat(&PinType::Text, &i), Compat::Convert);
        assert_eq!(PinType::Any.compat(&PinType::Number, &i), Compat::Warn);
        assert_eq!(PinType::Text.compat(&PinType::Number, &i), Compat::No);
        assert_eq!(PinType::Action.compat(&PinType::Action, &i), Compat::Yes);
        assert_eq!(PinType::Action.compat(&PinType::Any, &i), Compat::No);
        assert_eq!(PinType::Named("x.y".into()).compat(&PinType::Object, &i), Compat::Yes);
    }
}
