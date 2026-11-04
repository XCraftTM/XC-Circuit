//! The runtime's debug protocol: stdout lines starting with `@@xc ` followed by JSON.

use serde::Deserialize;

pub const PREFIX: &str = "@@xc ";

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "t", rename_all = "lowercase")]
pub enum BotEvent {
    Log {
        level: String,
        text: String,
        ws: Option<String>,
        node: Option<String>,
    },
    Node {
        ws: String,
        node: String,
        phase: String,
        ms: Option<f64>,
        error: Option<String>,
        /// On exit: a short text for each output, for wire tooltips.
        #[serde(default)]
        outputs: Option<std::collections::BTreeMap<String, String>>,
    },
    Zone {
        ws: String,
        zone: String,
        phase: String,
        error: Option<String>,
        #[serde(default)]
        outputs: Option<std::collections::BTreeMap<String, String>>,
    },
    Ready {},
    /// The bot needs a decision (such as how to start when its saved data is
    /// empty). Answer with `Session::answer`.
    Ask {
        id: String,
        text: String,
        choices: Vec<Choice>,
    },
    /// Anything the bot printed itself (`console.log`), or a line we can't parse.
    #[serde(skip)]
    Plain(String),
}

#[derive(Clone, Debug, Deserialize)]
pub struct Choice {
    pub value: String,
    pub label: String,
}

pub fn parse_line(line: &str) -> BotEvent {
    match line.strip_prefix(PREFIX) {
        Some(json) => serde_json::from_str(json).unwrap_or_else(|_| BotEvent::Plain(line.to_string())),
        None => BotEvent::Plain(line.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_protocol_lines() {
        match parse_line(r#"@@xc {"t":"node","ws":"main","node":"a1","phase":"exit","ms":1.5}"#) {
            BotEvent::Node { node, phase, ms, .. } => assert_eq!((node.as_str(), phase.as_str(), ms), ("a1", "exit", Some(1.5))),
            other => panic!("{other:?}"),
        }
        assert!(matches!(parse_line("hello"), BotEvent::Plain(s) if s == "hello"));
        assert!(matches!(parse_line(r#"@@xc {"t":"ready"}"#), BotEvent::Ready {}));
    }
}
