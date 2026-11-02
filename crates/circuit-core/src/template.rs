//! Texts with `${…}`: each is a JavaScript expression the runtime evaluates
//! when the block runs (runtime/circuit-runtime.js, `fillTemplate`). Inside,
//! `#4` is block #4's outputs; other names are the event's outputs, run
//! variables, values wired into the text's inputs and the block's fields.
//!
//! This reads such texts the same way the runtime does: where an expression
//! ends (strings and nested template strings included), which names it uses,
//! and the older `{name}` form to upgrade.

/// Names that are JavaScript's own, never values of the bot.
const JS_NAMES: &[&str] = &[
    "true", "false", "null", "undefined", "new", "typeof", "instanceof", "in", "of", "void", "delete", "this", "function", "return", "if", "else", "let", "const",
    "var", "await", "async", "yield", "class", "super", "NaN", "Infinity", "Math", "Date", "JSON", "String", "Number", "Boolean", "Array", "Object", "RegExp",
    "BigInt", "Symbol", "Promise", "Intl", "Map", "Set", "Error", "console", "globalThis", "parseInt", "parseFloat", "isNaN", "isFinite", "encodeURIComponent",
    "decodeURIComponent", "encodeURI", "decodeURI",
];

fn is_name_start(c: char) -> bool {
    c.is_alphabetic() || c == '_' || c == '$'
}

fn is_name_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '$'
}

/// Where the `}` closing an expression that starts at `from` is.
pub fn expression_end(chars: &[char], from: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut i = from;
    while i < chars.len() {
        match chars[i] {
            '"' | '\'' => i = string_end(chars, i),
            '`' => i = template_end(chars, i),
            '{' => depth += 1,
            '}' if depth == 0 => return Some(i),
            '}' => depth -= 1,
            _ => {}
        }
        i += 1;
    }
    None
}

fn string_end(chars: &[char], start: usize) -> usize {
    let quote = chars[start];
    let mut j = start + 1;
    while j < chars.len() {
        if chars[j] == '\\' {
            j += 1;
        } else if chars[j] == quote {
            return j;
        }
        j += 1;
    }
    chars.len()
}

fn template_end(chars: &[char], start: usize) -> usize {
    let mut j = start + 1;
    while j < chars.len() {
        match chars[j] {
            '\\' => j += 1,
            '`' => return j,
            '$' if chars.get(j + 1) == Some(&'{') => match expression_end(chars, j + 2) {
                Some(end) => j = end,
                None => return chars.len(),
            },
            _ => {}
        }
        j += 1;
    }
    chars.len()
}

/// The `${…}` expressions in a text: (where the expression starts, where its `}` is).
pub fn expressions(text: &str) -> Vec<(usize, usize)> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i + 1 < chars.len() {
        if chars[i] == '$' && chars[i + 1] == '{' {
            match expression_end(&chars, i + 2) {
                Some(end) => {
                    out.push((i + 2, end));
                    i = end;
                }
                None => break,
            }
        }
        i += 1;
    }
    out
}

/// The start of the `${…}` expression the cursor (a character index) is in,
/// one that isn't closed before it.
pub fn expression_at(chars: &[char], cursor: usize) -> Option<usize> {
    let mut i = 0;
    while i + 1 < chars.len().min(cursor) {
        if chars[i] == '$' && chars[i + 1] == '{' {
            match expression_end(chars, i + 2) {
                Some(end) if end < cursor => i = end,
                _ => return Some(i + 2),
            }
        }
        i += 1;
    }
    None
}

/// The names the `${…}` expressions in a text use, in order and once each:
/// not JavaScript's own, not members after a dot, not `#4` block references,
/// not object keys, and nothing inside strings.
pub fn names(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut out: Vec<String> = Vec::new();
    for (start, end) in expressions(text) {
        names_in(&chars[start..end], &mut out);
    }
    out
}

fn names_in(code: &[char], out: &mut Vec<String>) {
    let prev_significant = |i: usize| code[..i].iter().rev().find(|c| !c.is_whitespace()).copied();
    let mut i = 0;
    while i < code.len() {
        let c = code[i];
        if c == '"' || c == '\'' {
            i = string_end(code, i);
        } else if c == '`' {
            // A template string: only its own ${…} parts are code.
            let end = template_end(code, i).min(code.len());
            let inner: String = code[i..end.min(code.len())].iter().collect();
            for (s, e) in expressions(&inner) {
                let part: Vec<char> = inner.chars().collect();
                names_in(&part[s..e], out);
            }
            i = end;
        } else if c == '#' {
            // #4: a block's outputs.
            i += 1;
            while i < code.len() && is_name_char(code[i]) {
                i += 1;
            }
            continue;
        } else if c.is_ascii_digit() {
            while i < code.len() && (is_name_char(code[i]) || code[i] == '.') {
                i += 1;
            }
            continue;
        } else if is_name_start(c) {
            let start = i;
            while i < code.len() && is_name_char(code[i]) {
                i += 1;
            }
            let name: String = code[start..i].iter().collect();
            let after = code[i..].iter().find(|c| !c.is_whitespace()).copied();
            let before = prev_significant(start);
            let member = before == Some('.');
            let key = after == Some(':') && matches!(before, Some('{') | Some(','));
            if !member && !key && !JS_NAMES.contains(&name.as_str()) && !out.contains(&name) {
                out.push(name);
            }
            continue;
        }
        i += 1;
    }
}

/// A text in the older `{name}` form (`{user}`, `{user.name}`, `{#4.message}`)
/// in the `${…}` form, or `None` when it has none. Other braces stay.
pub fn upgrade(text: &str) -> Option<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len() + 8);
    let mut changed = false;
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '{' && (i == 0 || chars[i - 1] != '$') {
            let mut j = i + 1;
            if chars.get(j) == Some(&'#') {
                j += 1;
            }
            let name_start = j;
            let mut ok = chars.get(j).is_some_and(|c| is_name_start(*c) || (name_start > i + 1 && c.is_alphanumeric()));
            while ok && j < chars.len() && (is_name_char(chars[j]) || chars[j] == '.') {
                j += 1;
            }
            ok = ok && chars.get(j) == Some(&'}') && chars[j - 1] != '.';
            if ok {
                out.push('$');
                out.extend(&chars[i..=j]);
                changed = true;
                i = j + 1;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    changed.then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_in_expressions() {
        assert_eq!(names("Hi ${user}, you have ${xp * 2} XP (${user.name})"), vec!["user", "xp"]);
        assert_eq!(names("${member?.nickname ?? user.username}"), vec!["member", "user"]);
        assert_eq!(names("${#4.message.content} ${Math.max(a, 1)} ${'b + c'} ${ {key: d}.key }"), vec!["a", "d"]);
        assert_eq!(names("${`${e} and f`} ${g ? h : 'x'}"), vec!["e", "g", "h"]);
        assert!(names("{old} \\d{3} $5 ${").is_empty(), "only ${{…}} is code");
    }

    #[test]
    fn expressions_end_after_strings_and_objects() {
        assert_eq!(expressions("a ${'}'} b ${ {x: 1}.x } c"), vec![(4, 7), (13, 23)]);
        let open: Vec<char> = "Hi ${user.na".chars().collect();
        assert_eq!(expression_at(&open, 12), Some(5), "in an expression that isn't closed");
        let closed: Vec<char> = "Hi ${a} ok ${b}".chars().collect();
        assert_eq!(expression_at(&closed, 9), None, "after a closed one");
        assert_eq!(expression_at(&closed, 14), Some(13), "in a closed one");
    }

    #[test]
    fn the_older_form_upgrades() {
        assert_eq!(upgrade("Hi {user}, {#4.message.url} and {user.name}!").unwrap(), "Hi ${user}, ${#4.message.url} and ${user.name}!");
        assert_eq!(upgrade("${user} \\d{3} {a,b} {} { x } {x.}"), None, "other braces and the new form stay");
    }
}
