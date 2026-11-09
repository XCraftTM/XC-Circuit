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

/// What stands where one of the bot's secrets was.
pub const HIDDEN: &str = "[hidden secret]";
/// Secrets shorter than this aren't looked for: a short value turns up in
/// other texts by chance.
pub const SECRET_MIN: usize = 8;
/// A secret is known by this much of its start: long values are often shown
/// cut short (a block's outputs on a wire).
const KNOWN_BY: usize = 12;

/// Replaces every secret in `text`, or what is shown of one from its start,
/// with [`HIDDEN`].
pub fn hide_in(text: &mut String, secrets: &[String]) {
    for secret in secrets.iter().filter(|s| s.len() >= SECRET_MIN) {
        let start_len = secret.char_indices().nth(KNOWN_BY).map_or(secret.len(), |(i, _)| i);
        let mut from = 0;
        while let Some(at) = text[from..].find(&secret[..start_len]) {
            let start = from + at;
            // As far as it goes on like the secret, to the end of a character.
            let mut end = start + text[start..].bytes().zip(secret.bytes()).take_while(|(a, b)| a == b).count();
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            text.replace_range(start..end, HIDDEN);
            from = start + HIDDEN.len();
        }
    }
}

impl BotEvent {
    /// Takes the bot's secrets (its token, logins, database passwords) out
    /// of everything the event says. A bot can be made to print them (a Log
    /// block with `${process.env…}`); the Console, plugins' scripts and AI
    /// agents then get [`HIDDEN`] in their place.
    pub fn hide(&mut self, secrets: &[String]) {
        if secrets.is_empty() {
            return;
        }
        match self {
            BotEvent::Log { text, .. } | BotEvent::Plain(text) => hide_in(text, secrets),
            BotEvent::Node { error, outputs, .. } | BotEvent::Zone { error, outputs, .. } => {
                if let Some(e) = error {
                    hide_in(e, secrets);
                }
                for value in outputs.iter_mut().flat_map(|o| o.values_mut()) {
                    hide_in(value, secrets);
                }
            }
            BotEvent::Ask { text, choices, .. } => {
                hide_in(text, secrets);
                for c in choices {
                    hide_in(&mut c.label, secrets);
                }
            }
            BotEvent::Ready {} => {}
        }
    }
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

    /// The bot's secrets don't show in what it says: whole, cut short, more
    /// than once, in a log line, a block's outputs or an error.
    #[test]
    fn secrets_are_taken_out_of_what_the_bot_says() {
        let token = "MTIzNDU2Nzg5MDEy.GhIjKl.mnopqrstuvwxyz0123456789";
        let secrets = vec![token.to_string(), "pässwörter-sind-lang".to_string(), "short".to_string()];
        let mut text = format!("token {token} and again {token}!");
        hide_in(&mut text, &secrets);
        assert_eq!(text, "token [hidden secret] and again [hidden secret]!");
        // Cut short, as a wire's tooltip shows a long value.
        let mut cut = format!("{}…", &token[..20]);
        hide_in(&mut cut, &secrets);
        assert_eq!(cut, "[hidden secret]…");
        let mut umlauts = "db: pässwörter-sind-lang, pässwörter-sin".to_string();
        hide_in(&mut umlauts, &secrets);
        assert_eq!(umlauts, "db: [hidden secret], [hidden secret]");
        // A short secret would hide ordinary words; less than a secret's start isn't one.
        let mut plain = format!("a short text, {} too", &token[..8]);
        hide_in(&mut plain, &secrets);
        assert_eq!(plain, format!("a short text, {} too", &token[..8]));

        let mut log = parse_line(&format!(r#"@@xc {{"t":"log","level":"info","text":"it is {token}"}}"#));
        log.hide(&secrets);
        assert!(matches!(&log, BotEvent::Log { text, .. } if text == "it is [hidden secret]"), "{log:?}");
        let mut node = parse_line(&format!(r#"@@xc {{"t":"node","ws":"main","node":"1","phase":"exit","error":"bad {token}","outputs":{{"value":"{token}"}}}}"#));
        node.hide(&secrets);
        assert!(matches!(&node, BotEvent::Node { error: Some(e), outputs: Some(o), .. } if e == "bad [hidden secret]" && o["value"] == HIDDEN), "{node:?}");
        let mut plain = BotEvent::Plain(format!("printed {token}"));
        plain.hide(&secrets);
        assert!(matches!(&plain, BotEvent::Plain(t) if t == "printed [hidden secret]"));
    }
}
