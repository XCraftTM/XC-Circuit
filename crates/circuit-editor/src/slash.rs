//! The Slash Command Builder: edits `circuit.json` › `data.slashCommands`.
//! Each command can also be edited as the JSON Discord stores it.

use std::collections::BTreeMap;

use eframe::egui::{self, Color32, Frame, Margin, RichText, ScrollArea, Stroke, Ui, vec2};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::dialog::{self, Part};
use crate::editor::{Editor, Level};
use crate::i18n::n;
use crate::json_view::{self, JsonText};
use crate::locales::{self, Localized};
use crate::theme::Theme;

pub const DATA_KEY: &str = "slashCommands";

type Locales = BTreeMap<String, Localized>;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct SlashCommand {
    /// One word, or two or three for a subcommand: "ticket open". A name that
    /// others continue ("ticket", "ticket admin") holds that command's or
    /// group's description and translations.
    pub name: String,
    pub description: String,
    /// The name and description in other languages (a subcommand's name is its last word).
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub locales: Locales,
    pub options: Vec<SlashOption>,
    /// Permissions people need to see and use it (discord.js flag names); none: everyone.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub permissions: Vec<String>,
    /// Where it can be used: "guild", "botDm", "privateChannel".
    pub contexts: Vec<String>,
    /// How the bot must be added for it: "guild" (to the server), "user" (to an
    /// account, so it works everywhere). None: Discord's default, servers.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub installs: Vec<String>,
    /// Only usable in age-restricted channels.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub nsfw: bool,
    /// Older projects had one permission, and DMs on or off.
    #[serde(skip_serializing)]
    permission: String,
    #[serde(skip_serializing)]
    dm: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct SlashOption {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub description: String,
    pub required: bool,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub locales: Locales,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub choices: Vec<Choice>,
    /// Numbers: the smallest and largest value allowed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    /// Text: the shortest and longest length allowed.
    #[serde(rename = "minLength", skip_serializing_if = "Option::is_none")]
    pub min_length: Option<u32>,
    #[serde(rename = "maxLength", skip_serializing_if = "Option::is_none")]
    pub max_length: Option<u32>,
    /// Channels: the kinds that can be picked; none: all.
    #[serde(rename = "channelTypes", skip_serializing_if = "Vec::is_empty")]
    pub channel_types: Vec<String>,
    /// Suggestions come from an Autocomplete block while people type.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub autocomplete: bool,
}

impl Default for SlashOption {
    fn default() -> Self {
        SlashOption {
            name: String::new(),
            kind: "string".into(),
            description: String::new(),
            required: false,
            locales: Locales::new(),
            choices: Vec::new(),
            min: None,
            max: None,
            min_length: None,
            max_length: None,
            channel_types: Vec::new(),
            autocomplete: false,
        }
    }
}

/// A fixed answer to an option: the name people see and the value the bot gets.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Choice {
    pub name: String,
    pub value: Value,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub locales: Locales,
}

/// Discord's permissions: the discord.js flag name, its name in Discord and its bit.
const PERMISSIONS: [(&str, &str, u32); 50] = [
    ("Administrator", n("Administrator"), 3),
    ("ManageGuild", n("Manage Server"), 5),
    ("ManageRoles", n("Manage Roles"), 28),
    ("ManageChannels", n("Manage Channels"), 4),
    ("KickMembers", n("Kick Members"), 1),
    ("BanMembers", n("Ban Members"), 2),
    ("ModerateMembers", n("Time Out Members"), 40),
    ("ManageMessages", n("Manage Messages"), 13),
    ("ManageNicknames", n("Manage Nicknames"), 27),
    ("ManageWebhooks", n("Manage Webhooks"), 29),
    ("ManageThreads", n("Manage Threads"), 34),
    ("ManageEvents", n("Manage Events"), 33),
    ("ManageGuildExpressions", n("Manage Expressions"), 30),
    ("ViewAuditLog", n("View Audit Log"), 7),
    ("ViewGuildInsights", n("View Server Insights"), 19),
    ("ViewCreatorMonetizationAnalytics", n("View Creator Monetization Analytics"), 41),
    ("MentionEveryone", n("Mention @everyone, @here and All Roles"), 17),
    ("CreateInstantInvite", n("Create Invite"), 0),
    ("ChangeNickname", n("Change Nickname"), 26),
    ("ViewChannel", n("View Channels"), 10),
    ("SendMessages", n("Send Messages"), 11),
    ("SendMessagesInThreads", n("Send Messages in Threads"), 38),
    ("CreatePublicThreads", n("Create Public Threads"), 35),
    ("CreatePrivateThreads", n("Create Private Threads"), 36),
    ("EmbedLinks", n("Embed Links"), 14),
    ("AttachFiles", n("Attach Files"), 15),
    ("AddReactions", n("Add Reactions"), 6),
    ("UseExternalEmojis", n("Use External Emoji"), 18),
    ("UseExternalStickers", n("Use External Stickers"), 37),
    ("ReadMessageHistory", n("Read Message History"), 16),
    ("SendTTSMessages", n("Send Text-to-Speech Messages"), 12),
    ("SendVoiceMessages", n("Send Voice Messages"), 46),
    ("SendPolls", n("Create Polls"), 49),
    ("PinMessages", n("Pin Messages"), 51),
    ("UseApplicationCommands", n("Use Application Commands"), 31),
    ("UseExternalApps", n("Use External Apps"), 50),
    ("UseEmbeddedActivities", n("Use Activities"), 39),
    ("CreateGuildExpressions", n("Create Expressions"), 43),
    ("CreateEvents", n("Create Events"), 44),
    ("Connect", n("Connect"), 20),
    ("Speak", n("Speak"), 21),
    ("Stream", n("Video"), 9),
    ("UseVAD", n("Use Voice Activity"), 25),
    ("PrioritySpeaker", n("Priority Speaker"), 8),
    ("MuteMembers", n("Mute Members"), 22),
    ("DeafenMembers", n("Deafen Members"), 23),
    ("MoveMembers", n("Move Members"), 24),
    ("RequestToSpeak", n("Request to Speak"), 32),
    ("UseSoundboard", n("Use Soundboard"), 42),
    ("UseExternalSounds", n("Use External Sounds"), 45),
];

/// Where a command can be used, with Discord's number for it.
const CONTEXTS: [(&str, u64, &str); 3] = [("guild", 0, n("Servers")), ("botDm", 1, n("DMs with the bot")), ("privateChannel", 2, n("Group DMs and other DMs"))];

/// How the bot must be added for a command, with Discord's number.
const INSTALLS: [(&str, u64, &str); 2] = [("guild", 0, n("Added to a server")), ("user", 1, n("Added to an account"))];

/// The channel kinds an option can offer, with Discord's numbers.
const CHANNEL_TYPES: [(&str, u64, &str); 10] = [
    ("text", 0, n("Text")),
    ("voice", 2, n("Voice")),
    ("category", 4, n("Category")),
    ("announcement", 5, n("Announcement")),
    ("announcementThread", 10, n("Announcement thread")),
    ("publicThread", 11, n("Public thread")),
    ("privateThread", 12, n("Private thread")),
    ("stage", 13, n("Stage")),
    ("forum", 15, n("Forum")),
    ("media", 16, n("Media")),
];

const KINDS: [(&str, &str, u64); 9] = [
    ("string", n("Text"), 3),
    ("integer", n("Whole number"), 4),
    ("number", n("Number"), 10),
    ("boolean", n("Yes / no"), 5),
    ("user", n("User"), 6),
    ("channel", n("Channel"), 7),
    ("role", n("Role"), 8),
    ("mentionable", n("User or role"), 9),
    ("attachment", n("File"), 11),
];

pub struct SlashEdit {
    commands: Vec<SlashCommand>,
    selected: usize,
    /// While switched to JSON: the command (first word) shown, and its text.
    json: Option<(String, JsonText)>,
}

/// Brings commands saved by older versions up to date.
fn upgrade(c: &mut SlashCommand) {
    if c.permissions.is_empty() && !c.permission.is_empty() {
        c.permissions.push(std::mem::take(&mut c.permission));
    }
    if c.contexts.is_empty() {
        c.contexts = if c.dm { vec!["guild".into(), "botDm".into()] } else { vec!["guild".into()] };
    }
    for o in &mut c.options {
        // Text options used Max for their length.
        if o.kind == "string" {
            if o.max_length.is_none() {
                o.max_length = o.max.map(|m| m.max(1.0) as u32);
            }
            (o.min, o.max) = (None, None);
        }
    }
}

pub fn load(ed: &Editor) -> Vec<SlashCommand> {
    let mut list: Vec<SlashCommand> = ed.project.as_ref().and_then(|p| p.meta.data.get(DATA_KEY)).and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default();
    list.iter_mut().for_each(upgrade);
    list
}

pub fn open(ed: &mut Editor) {
    ed.slash_edit = Some(SlashEdit { commands: load(ed), selected: 0, json: None });
}

fn root_of(name: &str) -> &str {
    name.split_whitespace().next().unwrap_or("")
}

/// Whether other commands continue this name, so it holds a command's or group's settings.
fn is_header(cmds: &[SlashCommand], name: &str) -> bool {
    !name.is_empty() && cmds.iter().any(|o| o.name.starts_with(&format!("{name} ")))
}

/// The command whose permissions and places count for a whole command: the
/// command itself, or its first subcommand.
fn settings_holder(cmds: &[SlashCommand], root: &str) -> Option<usize> {
    cmds.iter().position(|c| c.name == root).or_else(|| cmds.iter().position(|c| root_of(&c.name) == root))
}

// ---- Discord's JSON ---------------------------------------------------------

fn describe(text: &str) -> String {
    let t = text.trim();
    if t.is_empty() { "A command made with XC Circuit".into() } else { t.chars().take(100).collect() }
}

/// A number as JSON: whole numbers without ".0", as JavaScript writes them.
fn num(n: f64) -> Value {
    if n.fract() == 0.0 && n.abs() < 9e15 { json!(n as i64) } else { json!(n) }
}

fn put_locales(out: &mut Map<String, Value>, l: &Locales, description: bool) {
    if let Some(v) = locales::to_discord(l, false) {
        out.insert("name_localizations".into(), v);
    }
    if description && let Some(v) = locales::to_discord(l, true) {
        out.insert("description_localizations".into(), v);
    }
}

fn option_json(o: &SlashOption) -> Value {
    let numeric = matches!(o.kind.as_str(), "integer" | "number");
    let text = o.kind == "string";
    let mut out = Map::new();
    out.insert("type".into(), json!(KINDS.iter().find(|k| k.0 == o.kind).map_or(3, |k| k.2)));
    out.insert("name".into(), json!(o.name));
    out.insert("description".into(), json!(describe(if o.description.trim().is_empty() { &o.name } else { &o.description })));
    put_locales(&mut out, &o.locales, true);
    out.insert("required".into(), json!(o.required));
    if (text || numeric) && !o.choices.is_empty() {
        let choices: Vec<Value> = o
            .choices
            .iter()
            .take(25)
            .map(|ch| {
                let mut c = Map::new();
                c.insert("name".into(), json!(ch.name));
                put_locales(&mut c, &ch.locales, false);
                let value = if numeric { num(ch.value.as_f64().or_else(|| ch.value.as_str().and_then(|s| s.trim().parse().ok())).unwrap_or(0.0)) } else { json!(crate::widgets::as_text(&ch.value)) };
                c.insert("value".into(), value);
                Value::Object(c)
            })
            .collect();
        out.insert("choices".into(), Value::Array(choices));
    }
    if numeric {
        if let Some(v) = o.min {
            out.insert("min_value".into(), num(v));
        }
        if let Some(v) = o.max {
            out.insert("max_value".into(), num(v));
        }
    }
    if text {
        if let Some(v) = o.min_length {
            out.insert("min_length".into(), json!(v));
        }
        if let Some(v) = o.max_length {
            out.insert("max_length".into(), json!(v));
        }
    }
    if o.kind == "channel" && !o.channel_types.is_empty() {
        let types: Vec<u64> = o.channel_types.iter().filter_map(|t| CHANNEL_TYPES.iter().find(|c| c.0 == t)).map(|c| c.1).collect();
        out.insert("channel_types".into(), json!(types));
    }
    if o.autocomplete && o.choices.is_empty() && (text || numeric) {
        out.insert("autocomplete".into(), json!(true));
    }
    Value::Object(out)
}

/// Options in Discord's order: required ones first.
fn options_json(c: &SlashCommand) -> Vec<Value> {
    let mut list: Vec<&SlashOption> = c.options.iter().collect();
    list.sort_by_key(|o| !o.required);
    list.into_iter().map(option_json).collect()
}

fn sub_json(c: &SlashCommand, kind: u64, name: &str, options: Vec<Value>) -> Value {
    let mut out = Map::new();
    out.insert("type".into(), json!(kind));
    out.insert("name".into(), json!(name));
    out.insert("description".into(), json!(describe(&c.description)));
    put_locales(&mut out, &c.locales, true);
    out.insert("options".into(), Value::Array(options));
    Value::Object(out)
}

/// One command as Discord stores it, with its subcommands and groups.
/// Subcommands come first, then groups, as the bot registers them.
fn root_json(cmds: &[SlashCommand], root: &str) -> Value {
    let words = |c: &SlashCommand| c.name.split_whitespace().map(str::to_string).collect::<Vec<_>>();
    let entries: Vec<&SlashCommand> = cmds.iter().filter(|c| root_of(&c.name) == root).collect();
    let me = entries.iter().find(|c| words(c).len() == 1).copied();
    let mut nested = Vec::new();
    for c in entries.iter().filter(|c| words(c).len() == 2 && !is_header(cmds, &c.name)) {
        nested.push(sub_json(c, 1, &words(c)[1], options_json(c)));
    }
    let mut groups: Vec<String> = Vec::new();
    for c in entries.iter().filter(|c| words(c).len() >= 3) {
        let g = words(c)[1].clone();
        if !groups.contains(&g) {
            groups.push(g);
        }
    }
    for g in groups {
        let subs: Vec<Value> = entries.iter().filter(|c| words(c).len() >= 3 && words(c)[1] == g).map(|c| sub_json(c, 1, &words(c)[2], options_json(c))).collect();
        let header = entries.iter().find(|c| c.name == format!("{root} {g}")).copied().cloned().unwrap_or(SlashCommand { description: g.clone(), ..Default::default() });
        nested.push(sub_json(&header, 2, &g, subs));
    }
    let settings = settings_holder(cmds, root).map(|i| &cmds[i]).cloned().unwrap_or_default();
    let mut out = Map::new();
    out.insert("name".into(), json!(root));
    let description = match me {
        Some(c) if !c.description.trim().is_empty() => describe(&c.description),
        _ => describe(nested.first().and_then(|n| n["description"].as_str()).unwrap_or_default()),
    };
    out.insert("description".into(), json!(description));
    if let Some(c) = me {
        put_locales(&mut out, &c.locales, true);
    }
    let options = if nested.is_empty() { me.map(options_json).unwrap_or_default() } else { nested };
    out.insert("options".into(), Value::Array(options));
    let bits: u64 = settings.permissions.iter().filter_map(|p| PERMISSIONS.iter().find(|q| q.0 == p)).fold(0, |b, q| b | 1u64 << q.2);
    if bits != 0 {
        out.insert("default_member_permissions".into(), json!(bits.to_string()));
    }
    let contexts: Vec<u64> = settings.contexts.iter().filter_map(|c| CONTEXTS.iter().find(|x| x.0 == c)).map(|x| x.1).collect();
    out.insert("contexts".into(), json!(if contexts.is_empty() { vec![0] } else { contexts }));
    let installs: Vec<u64> = settings.installs.iter().filter_map(|c| INSTALLS.iter().find(|x| x.0 == c)).map(|x| x.1).collect();
    if !installs.is_empty() {
        out.insert("integration_types".into(), json!(installs));
    }
    if settings.nsfw {
        out.insert("nsfw".into(), json!(true));
    }
    Value::Object(out)
}

/// The builder's commands as Discord stores them, one per first word.
#[cfg(test)]
pub fn to_discord(cmds: &[SlashCommand]) -> Vec<Value> {
    let mut roots: Vec<&str> = Vec::new();
    for c in cmds {
        let r = root_of(&c.name);
        if !r.is_empty() && !roots.contains(&r) {
            roots.push(r);
        }
    }
    roots.into_iter().map(|r| root_json(cmds, r)).collect()
}

fn text(v: &Value, key: &str) -> String {
    v.get(key).and_then(Value::as_str).unwrap_or_default().to_string()
}

fn parse_option(v: &Value) -> Result<SlashOption, String> {
    let ty = v.get("type").and_then(Value::as_u64).unwrap_or(3);
    let kind = KINDS.iter().find(|k| k.2 == ty).map(|k| k.0).ok_or_else(|| tf!("option type {number} isn't one Discord has", number = ty))?;
    let names = |key: &str| v.get(key).and_then(Value::as_array).cloned().unwrap_or_default();
    Ok(SlashOption {
        name: text(v, "name"),
        kind: kind.into(),
        description: text(v, "description"),
        required: v.get("required").and_then(Value::as_bool).unwrap_or(false),
        locales: locales::from_discord(v.get("name_localizations"), v.get("description_localizations")),
        choices: names("choices")
            .iter()
            .map(|c| Choice { name: text(c, "name"), value: c.get("value").cloned().unwrap_or(Value::Null), locales: locales::from_discord(c.get("name_localizations"), None) })
            .collect(),
        min: v.get("min_value").and_then(Value::as_f64),
        max: v.get("max_value").and_then(Value::as_f64),
        min_length: v.get("min_length").and_then(Value::as_u64).map(|n| n as u32),
        max_length: v.get("max_length").and_then(Value::as_u64).map(|n| n as u32),
        channel_types: names("channel_types").iter().filter_map(Value::as_u64).filter_map(|n| CHANNEL_TYPES.iter().find(|c| c.1 == n)).map(|c| c.0.to_string()).collect(),
        autocomplete: v.get("autocomplete").and_then(Value::as_bool).unwrap_or(false),
    })
}

/// Commands in Discord's JSON (one, or a list) as the builder's commands.
pub fn from_discord(v: &Value) -> Result<Vec<SlashCommand>, String> {
    let list = match v {
        Value::Array(items) => items.clone(),
        Value::Object(_) => vec![v.clone()],
        _ => return Err(t!("paste a command, or a list of commands").into()),
    };
    let mut out = Vec::new();
    for item in &list {
        if !item.is_object() {
            return Err(t!("each command must be an object { … }").into());
        }
        if item.get("type").and_then(Value::as_u64).is_some_and(|t| t != 1) {
            return Err(t!("right-click (context menu) commands come from Context Menu Used blocks").into());
        }
        let root = text(item, "name");
        if root.is_empty() {
            return Err(t!("a command needs a \"name\"").into());
        }
        let ints = |key: &str| item.get(key).and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_u64).collect::<Vec<_>>());
        let bits: u64 = match item.get("default_member_permissions") {
            Some(Value::String(s)) => s.parse().unwrap_or(0),
            Some(Value::Number(n)) => n.as_u64().unwrap_or(0),
            _ => 0,
        };
        let contexts = match ints("contexts") {
            Some(c) => CONTEXTS.iter().filter(|x| c.contains(&x.1)).map(|x| x.0.to_string()).collect(),
            None if item.get("dm_permission").and_then(Value::as_bool) == Some(false) => vec!["guild".to_string()],
            None => vec!["guild".to_string(), "botDm".to_string()],
        };
        let base = SlashCommand {
            name: root.clone(),
            description: text(item, "description"),
            locales: locales::from_discord(item.get("name_localizations"), item.get("description_localizations")),
            permissions: PERMISSIONS.iter().filter(|p| bits & (1u64 << p.2) != 0).map(|p| p.0.to_string()).collect(),
            contexts,
            installs: ints("integration_types").map(|c| INSTALLS.iter().filter(|x| c.contains(&x.1)).map(|x| x.0.to_string()).collect()).unwrap_or_default(),
            nsfw: item.get("nsfw").and_then(Value::as_bool).unwrap_or(false),
            ..Default::default()
        };
        let options = item.get("options").and_then(Value::as_array).cloned().unwrap_or_default();
        let nested = |o: &Value| o.get("type").and_then(Value::as_u64).is_some_and(|t| t == 1 || t == 2);
        if !options.iter().any(nested) {
            out.push(SlashCommand { options: options.iter().map(parse_option).collect::<Result<_, _>>()?, ..base });
            continue;
        }
        out.push(base);
        let entry = |name: String, o: &Value| -> Result<SlashCommand, String> {
            let opts = o.get("options").and_then(Value::as_array).cloned().unwrap_or_default();
            Ok(SlashCommand {
                name,
                description: text(o, "description"),
                locales: locales::from_discord(o.get("name_localizations"), o.get("description_localizations")),
                options: opts.iter().filter(|x| !nested(x)).map(parse_option).collect::<Result<_, _>>()?,
                ..Default::default()
            })
        };
        for o in &options {
            match o.get("type").and_then(Value::as_u64) {
                Some(1) => out.push(entry(format!("{root} {}", text(o, "name")), o)?),
                Some(2) => {
                    let group = format!("{root} {}", text(o, "name"));
                    out.push(SlashCommand { options: Vec::new(), ..entry(group.clone(), o)? });
                    for s in o.get("options").and_then(Value::as_array).into_iter().flatten() {
                        out.push(entry(format!("{group} {}", text(s, "name")), s)?);
                    }
                }
                _ => return Err(t!("a command with subcommands can't have options of its own").into()),
            }
        }
    }
    for c in &mut out {
        if c.contexts.is_empty() && root_of(&c.name) == c.name {
            c.contexts.push("guild".into());
        }
    }
    Ok(out)
}

// ---- Checks -----------------------------------------------------------------

/// A command name: one to three words (the later ones are subcommands), each
/// following Discord's rules.
fn command_name_problem(name: &str) -> Option<&'static str> {
    let words: Vec<&str> = name.split(' ').collect();
    if words.len() > 3 {
        return Some(t!("at most three words: command, group and subcommand"));
    }
    words.iter().find_map(|w| name_problem(w))
}

/// Discord's rules: 1–32 lowercase letters, digits, `-` or `_`.
fn name_problem(name: &str) -> Option<&'static str> {
    if name.is_empty() {
        Some(t!("needs a name"))
    } else if name.len() > 32 {
        Some(t!("name is longer than 32 characters"))
    } else if !name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_') {
        Some(t!("use lowercase letters, numbers, - and _ only"))
    } else {
        None
    }
}

fn problems(cmds: &[SlashCommand]) -> Vec<String> {
    let mut out = Vec::new();
    for (i, c) in cmds.iter().enumerate() {
        let label = if c.name.is_empty() { format!("Command {}", i + 1) } else { format!("/{}", c.name) };
        if let Some(p) = command_name_problem(&c.name) {
            out.push(format!("{label}: {p}"));
        }
        let header = is_header(cmds, &c.name);
        // A command with subcommands can't have options of its own.
        if !c.options.is_empty() && header {
            out.push(tf!("{command}: has subcommands, so its options belong to them", command = label));
        }
        if cmds.iter().filter(|o| o.name == c.name).count() > 1 && !c.name.is_empty() {
            out.push(format!("{label}: the name is used twice"));
        }
        if c.description.trim().is_empty() || c.description.chars().count() > 100 {
            out.push(format!("{label}: description must be 1–100 characters"));
        }
        if settings_holder(cmds, root_of(&c.name)) == Some(i) && c.contexts.is_empty() {
            out.push(tf!("{command}: pick at least one place where it works", command = label));
        }
        out.extend(locales::problems(&c.locales, true, 32, true).into_iter().map(|p| format!("{label}: {p}")));
        if c.options.len() > 25 {
            out.push(format!("{label}: at most 25 options"));
        }
        for o in &c.options {
            let at = format!("{label} › {}", o.name);
            if let Some(p) = name_problem(&o.name) {
                out.push(format!("{label} › option: {p}"));
            }
            if o.description.trim().is_empty() || o.description.chars().count() > 100 {
                out.push(tf!("{option}: needs a description of up to 100 characters", option = at));
            }
            out.extend(locales::problems(&o.locales, true, 32, true).into_iter().map(|p| format!("{at}: {p}")));
            if let (Some(a), Some(b)) = (o.min, o.max)
                && a > b
            {
                out.push(tf!("{option}: the smallest value is above the largest", option = at));
            }
            if let (Some(a), Some(b)) = (o.min_length, o.max_length)
                && a > b
            {
                out.push(tf!("{option}: the shortest length is above the longest", option = at));
            }
            if o.min_length.is_some_and(|n| n > 6000) || o.max_length.is_some_and(|n| n == 0 || n > 6000) {
                out.push(tf!("{option}: lengths go from 1 to 6000", option = at));
            }
            if o.choices.len() > 25 {
                out.push(tf!("{option}: at most 25 choices", option = at));
            }
            let numeric = matches!(o.kind.as_str(), "integer" | "number");
            for ch in &o.choices {
                if ch.name.trim().is_empty() || ch.name.chars().count() > 100 {
                    out.push(tf!("{option}: choice names must be 1–100 characters", option = at));
                }
                let value = crate::widgets::as_text(&ch.value);
                if numeric && value.trim().parse::<f64>().is_err() {
                    out.push(tf!("{option}: the value of “{choice}” must be a number", option = at, choice = ch.name));
                } else if value.trim().is_empty() || value.chars().count() > 100 {
                    out.push(tf!("{option}: choice values must be 1–100 characters", option = at));
                }
                out.extend(locales::problems(&ch.locales, false, 100, false).into_iter().map(|p| format!("{at} › {}: {p}", ch.name)));
            }
        }
    }
    out.dedup();
    out
}

// ---- The tab ----------------------------------------------------------------

/// The Slash Commands tab.
pub fn tab(ui: &mut Ui, ed: &mut Editor) {
    let Some(edit) = &mut ed.slash_edit else { return };
    let theme = ed.theme.clone();
    let icon = ed.reg.icons.get("slash").cloned();
    let trash = ed.reg.icons.get("trash").cloned();
    let eye = ed.reg.icons.get("eye").cloned();
    let issues = problems(&edit.commands);
    let (mut save, mut close, mut cancel) = (false, false, false);
    let json_broken = edit.json.as_ref().is_some_and(|(_, j)| j.error.is_some());
    dialog::tab(
        ui,
        &theme,
        dialog::Spec {
            id: "slash-builder",
            title: t!("Slash Commands"),
            subtitle: Some(t!("Registered automatically when the bot starts")),
            icon: icon.as_deref(),
            width: 1300.0,
        },
        |ui, part| match part {
            Part::Body => body(ui, &theme, edit, &issues, trash.as_deref(), eye.as_deref()),
            Part::Footer => {
                ui.add_enabled_ui(issues.is_empty() && !json_broken, |ui| {
                    if dialog::primary_button(ui, &theme, t!("Save and close")).clicked() {
                        (save, close) = (true, true);
                    }
                    if dialog::secondary_button(ui, &theme, t!("Save")).clicked() {
                        save = true;
                    }
                });
                cancel = dialog::secondary_button(ui, &theme, t!("Cancel")).clicked();
                if json_broken {
                    ui.label(RichText::new(t!("Fix the JSON first")).size(12.0).color(theme.warning));
                } else if !issues.is_empty() {
                    ui.label(RichText::new(tf!("{count} to fix", count = issues.len())).size(12.0).color(theme.warning));
                }
            }
        },
    );
    if save {
        // Save keeps the editor open with the same draft; Save and close ends it.
        let commands = ed.slash_edit.as_ref().unwrap().commands.clone();
        if close {
            ed.slash_edit = None;
        }
        if let Some(p) = ed.project.as_mut() {
            p.meta.data.insert(DATA_KEY.into(), serde_json::to_value(&commands).unwrap_or(Value::Null));
            ed.dirty = true;
        }
        ed.refresh_define_ctx();
        ed.log(Level::Success, tf!("Saved {count} slash commands", count = commands.len()));
    } else if cancel {
        ed.slash_edit = None;
    }
}

/// Puts the commands from pasted or edited JSON in place of the command it
/// showed (`root`), and of any command with the same name. Returns the first
/// word of the first new command.
fn apply_json(edit: &mut SlashEdit, root: &str, v: &Value) -> Result<String, String> {
    let new = from_discord(v)?;
    let new_roots: Vec<String> = new.iter().map(|c| root_of(&c.name).to_string()).collect();
    let goes = |c: &SlashCommand| root_of(&c.name) == root || new_roots.iter().any(|r| r == root_of(&c.name));
    let at = edit.commands.iter().take_while(|c| root_of(&c.name) != root).filter(|c| !goes(c)).count();
    edit.commands.retain(|c| !goes(c));
    let first = new_roots.first().cloned().unwrap_or_default();
    let at = at.min(edit.commands.len());
    edit.commands.splice(at..at, new);
    edit.selected = at;
    Ok(first)
}

fn body(ui: &mut Ui, theme: &Theme, edit: &mut SlashEdit, issues: &[String], trash: Option<&std::path::Path>, eye: Option<&std::path::Path>) {
    ui.horizontal_top(|ui| {
        // Command list.
        let h = dialog::remaining_height(ui, 420.0);
        ui.allocate_ui_with_layout(vec2(230.0, h), egui::Layout::top_down(egui::Align::Min), |ui| {
            dialog::section(ui, theme, t!("Commands"));
            let mut remove = None;
            for (i, c) in edit.commands.iter().enumerate() {
                let selected = i == edit.selected;
                let label = if c.name.is_empty() { "/(no name)".to_string() } else { format!("/{}", c.name) };
                let text = RichText::new(label).monospace().color(if selected { theme.text } else { theme.text_muted });
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    if ui.add_sized([180.0, 34.0], egui::Button::selectable(selected, text)).clicked() {
                        edit.selected = i;
                    }
                    if dialog::icon_button(ui, theme, trash, "×", t!("Delete this command")).clicked() {
                        remove = Some(i);
                    }
                });
            }
            if let Some(i) = remove {
                edit.commands.remove(i);
                edit.selected = edit.selected.min(edit.commands.len().saturating_sub(1));
            }
            ui.add_space(8.0);
            if dialog::secondary_button(ui, theme, t!("+ New command")).clicked() {
                edit.commands.push(SlashCommand {
                    name: format!("command{}", edit.commands.len() + 1),
                    description: t!("Describe what it does").into(),
                    contexts: vec!["guild".into()],
                    ..Default::default()
                });
                edit.selected = edit.commands.len() - 1;
            }
            if !issues.is_empty() {
                ui.add_space(14.0);
                for p in issues.iter().take(6) {
                    ui.label(RichText::new(format!("• {p}")).size(11.5).color(theme.warning));
                }
            }
        });
        let r = ui.min_rect();
        ui.painter().vline(r.right() + 8.0, r.y_range(), Stroke::new(1.0, theme.line));
        ui.add_space(18.0);

        // The selected command, and on the right how it looks in Discord.
        let form_w = (ui.available_width() - PREVIEW_W - 40.0).max(380.0);
        ui.allocate_ui_with_layout(vec2(form_w, h), egui::Layout::top_down(egui::Align::Min), |ui| {
            if edit.commands.get(edit.selected).is_none() {
                ui.add_space(40.0);
                ui.label(RichText::new(t!("No slash commands yet. Click “+ New command”.")).color(theme.text_muted));
                return;
            }
            let root = root_of(&edit.commands[edit.selected].name).to_string();
            // Builder | JSON.
            if let Some(json) = json_view::switch(ui, theme, edit.json.is_some()) {
                if json {
                    edit.json = Some((root.clone(), JsonText::new(&root_json(&edit.commands, &root))));
                } else if edit.json.as_ref().is_none_or(|(_, j)| j.error.is_none()) {
                    edit.json = None;
                }
            }
            ui.add_space(8.0);
            if let Some((shown, jt)) = &mut edit.json {
                // Another command was picked: show its JSON (unless this one has an error to fix).
                if *shown != root && jt.error.is_none() {
                    *shown = root.clone();
                    *jt = JsonText::new(&root_json(&edit.commands, &root));
                }
                let help = tf!(
                    "/{command} as Discord stores it, with its subcommands. Edit it, or paste a command (from the Discord docs or discord.js toJSON()); it applies as soon as it is valid.",
                    command = shown
                );
                let shown_now = shown.clone();
                if let Some(v) = json_view::editor(ui, theme, ui.id().with("slash-json"), jt, &help) {
                    let mut jt_after = jt.clone();
                    match apply_json(edit, &shown_now, &v) {
                        Ok(first) => edit.json = Some((first, jt_after)),
                        Err(e) => {
                            jt_after.error = Some(e);
                            edit.json = Some((shown_now, jt_after));
                        }
                    }
                }
                return;
            }
            ScrollArea::vertical().id_salt("slash-form").max_height(dialog::remaining_height(ui, 300.0)).auto_shrink([false, true]).show(ui, |ui| {
                ui.set_width(ui.available_width());
                command_form(ui, theme, edit, &root);
            });
        });
        if let Some(cmd) = edit.commands.get(edit.selected) {
            ui.add_space(20.0);
            discord_preview(ui, theme, cmd, eye);
        }
    });
}

fn command_form(ui: &mut Ui, theme: &Theme, edit: &mut SlashEdit, root: &str) {
    let i = edit.selected;
    let header = is_header(&edit.commands, &edit.commands[i].name);
    let holds_settings = settings_holder(&edit.commands, root) == Some(i);
    let cmd = &mut edit.commands[i];
    dialog::section(ui, theme, t!("Command"));
    ui.columns(2, |cols| {
        dialog::field(&mut cols[0], theme, t!("Name"), Some(t!("What people type after /. Two words make a subcommand: ticket open")), |ui| dialog::text_input(ui, &mut cmd.name, "rank", false));
        dialog::field(&mut cols[1], theme, t!("Description"), Some(t!("Shown in Discord's command list (up to 100 characters)")), |ui| {
            dialog::text_input(ui, &mut cmd.description, t!("Shows your rank"), false)
        });
    });
    locales::section(ui, theme, ui.id().with(("cmd-locales", i)), &mut cmd.locales, true);
    ui.add_space(6.0);

    if holds_settings {
        dialog::section(ui, theme, t!("Who and where"));
        ui.horizontal(|ui| {
            ui.set_min_height(crate::inputs::input_height(ui));
            ui.label(RichText::new(t!("Who can use it")).color(theme.text));
            permissions_dropdown(ui, &mut cmd.permissions);
            ui.add_space(16.0);
            ui.label(RichText::new(t!("Age-restricted")).color(theme.text));
            crate::widgets::toggle(ui, &mut cmd.nsfw, theme).on_hover_text(t!("Only usable in age-restricted (NSFW) channels."));
        });
        ui.label(RichText::new(t!("People without all of these permissions don't see the command. Server admins can change it in Server Settings › Integrations.")).size(11.5).color(theme.text_muted));
        ui.add_space(6.0);
        pick_row(ui, theme, t!("Works in"), &CONTEXTS, &mut cmd.contexts, t!("Nowhere"));
        pick_row(ui, theme, t!("When the bot is"), &INSTALLS, &mut cmd.installs, t!("Added to a server"));
        ui.label(RichText::new(t!("Added to an account, the bot's commands work in any server or DM that person is in. Turn it on in the Discord developer portal › Installation too.")).size(11.5).color(theme.text_muted));
    } else {
        ui.label(RichText::new(tf!("Who can use it and where it works are set on /{command}.", command = root)).size(12.0).color(theme.text_muted));
    }

    if header {
        ui.add_space(8.0);
        ui.label(RichText::new(tf!("/{command} only groups its subcommands, so the options are on them.", command = cmd.name)).size(12.0).color(theme.text_muted));
        return;
    }
    dialog::section(ui, theme, &tf!("Options · {count} of 25", count = cmd.options.len()));
    let (mut remove, mut swap) = (None, None);
    let count = cmd.options.len();
    for (n, o) in cmd.options.iter_mut().enumerate() {
        ui.push_id(("option", i, n), |ui| {
            option_card(ui, theme, o, n, count, &mut remove, &mut swap);
        });
    }
    if let Some((a, b)) = swap {
        cmd.options.swap(a, b);
    }
    if let Some(n) = remove {
        cmd.options.remove(n);
    }
    if cmd.options.len() < 25 && dialog::secondary_button(ui, theme, t!("+ Add option")).clicked() {
        cmd.options.push(SlashOption { name: format!("option{}", cmd.options.len() + 1), description: t!("Describe this option").into(), ..Default::default() });
    }
    ui.label(RichText::new(t!("Discord lists required options first.")).size(11.5).color(theme.text_muted));
}

/// A row with a label and a dropdown picking any number of `options` (keys
/// with their English labels), centered on one line.
fn pick_row(ui: &mut Ui, theme: &Theme, label: &str, options: &[(&str, u64, &'static str)], list: &mut Vec<String>, none: &str) {
    ui.horizontal(|ui| {
        ui.set_min_height(crate::inputs::input_height(ui));
        ui.label(RichText::new(label).color(theme.text));
        let options: Vec<(String, &'static str)> = options.iter().map(|(k, _, l)| (k.to_string(), *l)).collect();
        crate::widgets::multi_select(ui, ui.id().with(label), 260.0, &options, list, none);
    });
}

/// Picks any number of permissions; the summary says how many.
fn permissions_dropdown(ui: &mut Ui, list: &mut Vec<String>) {
    let name = |k: &str| PERMISSIONS.iter().find(|p| p.0 == k).map(|p| crate::i18n::t(p.1)).unwrap_or(k).to_string();
    let summary = match list.len() {
        0 => t!("Everyone").to_string(),
        1 => name(&list[0]),
        n => tf!("{first} + {more} more", first = name(&list[0]), more = n - 1),
    };
    ui.scope(|ui| {
        ui.spacing_mut().interact_size.y = crate::inputs::input_height(ui);
        egui::ComboBox::from_id_salt(ui.id().with("permissions")).width(240.0).selected_text(summary).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show_ui(ui, |ui| {
            if ui.selectable_label(list.is_empty(), t!("Everyone")).clicked() {
                list.clear();
            }
            ui.separator();
            ScrollArea::vertical().max_height(340.0).show(ui, |ui| {
                for (key, label, _) in PERMISSIONS {
                    let mut on = list.iter().any(|k| k == key);
                    if ui.checkbox(&mut on, crate::i18n::t(label)).changed() {
                        if on {
                            list.push(key.into());
                        } else {
                            list.retain(|k| k != key);
                        }
                    }
                }
            });
        })
    })
    .inner
    .response
    .on_hover_text(t!("People need all the permissions you tick."));
}

fn option_card(ui: &mut Ui, theme: &Theme, o: &mut SlashOption, i: usize, count: usize, remove: &mut Option<usize>, swap: &mut Option<(usize, usize)>) {
    Frame::new().fill(theme.panel_header).stroke(Stroke::new(1.0, theme.line)).corner_radius(9).inner_margin(Margin::same(12)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        let numeric = matches!(o.kind.as_str(), "integer" | "number");
        let texty = o.kind == "string";
        ui.horizontal(|ui| {
            let h = crate::inputs::input_height(ui);
            ui.set_min_height(h);
            ui.allocate_ui(vec2(160.0, h), |ui| dialog::text_input(ui, &mut o.name, "name", false));
            let current = KINDS.iter().find(|k| k.0 == o.kind).map(|k| crate::i18n::t(k.1)).unwrap_or(t!("Text"));
            crate::widgets::dropdown(ui, ui.id().with("kind"), 130.0, current, |ui| {
                for (k, l, _) in KINDS {
                    ui.selectable_value(&mut o.kind, k.to_string(), crate::i18n::t(l));
                }
            });
            ui.label(RichText::new(t!("Required")).size(12.0).color(theme.text_muted));
            crate::widgets::toggle(ui, &mut o.required, theme);
            if (texty || numeric) && o.choices.is_empty() {
                ui.label(RichText::new(t!("Autocomplete")).size(12.0).color(theme.text_muted));
                crate::widgets::toggle(ui, &mut o.autocomplete, theme).on_hover_text(t!("Suggest values while people type, from an Autocomplete block."));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if dialog::danger_button(ui, theme, t!("Remove")).clicked() {
                    *remove = Some(i);
                }
                if ui.add_enabled_ui(i + 1 < count, |ui| dialog::button(ui, theme, t!("Down"))).inner.clicked() {
                    *swap = Some((i, i + 1));
                }
                if ui.add_enabled_ui(i > 0, |ui| dialog::button(ui, theme, t!("Up"))).inner.clicked() {
                    *swap = Some((i, i - 1));
                }
            });
        });
        ui.add_space(6.0);
        dialog::text_input(ui, &mut o.description, t!("What this option is for"), false);
        ui.add_space(6.0);
        if texty {
            ui.horizontal(|ui| {
                ui.label(RichText::new(t!("Length")).size(12.0).color(theme.text_muted));
                length_box(ui, t!("Shortest"), &mut o.min_length);
                length_box(ui, t!("Longest"), &mut o.max_length);
            });
        } else if numeric {
            ui.horizontal(|ui| {
                ui.label(RichText::new(t!("Value")).size(12.0).color(theme.text_muted));
                number_box(ui, t!("Smallest"), &mut o.min);
                number_box(ui, t!("Largest"), &mut o.max);
            });
        } else if o.kind == "channel" {
            pick_row(ui, theme, t!("Channel kinds"), &CHANNEL_TYPES, &mut o.channel_types, t!("Every kind"));
        }
        if (texty || numeric) && !o.autocomplete {
            choices(ui, theme, o, numeric);
        }
        locales::section(ui, theme, ui.id().with("locales"), &mut o.locales, true);
    });
    ui.add_space(8.0);
}

/// The fixed answers of an option, each with a name, a value and translations.
fn choices(ui: &mut Ui, theme: &Theme, o: &mut SlashOption, numeric: bool) {
    ui.add_space(4.0);
    ui.label(RichText::new(tf!("Choices · {count} of 25 (optional: people pick one instead of typing)", count = o.choices.len())).size(12.0).color(theme.text_muted));
    let mut remove = None;
    let w = ui.available_width();
    for (i, ch) in o.choices.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            ui.set_min_height(crate::inputs::input_height(ui));
            let field_w = ((w - 230.0) / 2.0).max(120.0);
            ui.add(crate::inputs::TextLine::new(&mut ch.name).hint(t!("Name people see")).width(field_w));
            let mut value = crate::widgets::as_text(&ch.value);
            if ui.add(crate::inputs::TextLine::new(&mut value).hint(t!("Value the bot gets")).width(field_w)).changed() {
                ch.value = if numeric { value.trim().parse::<f64>().map(Value::from).unwrap_or(Value::from(value)) } else { Value::from(value) };
            }
            let label = if ch.locales.is_empty() { t!("Translations").to_string() } else { tf!("Translations · {count}", count = ch.locales.len()) };
            let resp = dialog::button(ui, theme, label);
            egui::Popup::menu(&resp).id(ui.id().with(("choice-locales", i))).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
                ui.set_width(460.0);
                locales::editor(ui, theme, ui.id().with(("choice-rows", i)), &mut ch.locales, false);
            });
            if dialog::danger_button(ui, theme, "×").on_hover_text(t!("Remove this choice")).clicked() {
                remove = Some(i);
            }
        });
    }
    if let Some(i) = remove {
        o.choices.remove(i);
    }
    if o.choices.len() < 25 && dialog::button(ui, theme, t!("+ Add choice")).clicked() {
        let n = o.choices.len() + 1;
        let value = if numeric { Value::from(n) } else { Value::from(format!("choice{n}")) };
        o.choices.push(Choice { name: tf!("Choice {number}", number = n), value, locales: Locales::new() });
    }
}

fn number_box(ui: &mut Ui, label: &str, v: &mut Option<f64>) {
    let mut text = v.map(|n| n.to_string()).unwrap_or_default();
    if ui.add(crate::inputs::TextLine::new(&mut text).hint(label).width(96.0)).changed() {
        *v = text.trim().parse().ok();
    }
}

fn length_box(ui: &mut Ui, label: &str, v: &mut Option<u32>) {
    let mut text = v.map(|n| n.to_string()).unwrap_or_default();
    if ui.add(crate::inputs::TextLine::new(&mut text).hint(label).width(96.0)).changed() {
        *v = text.trim().parse().ok();
    }
}

/// The width of the "How it looks in Discord" column.
const PREVIEW_W: f32 = 340.0;

/// Discord's own colors, so the preview looks like the real thing in any theme.
mod discord {
    use eframe::egui::Color32;
    pub const PANEL: Color32 = Color32::from_rgb(0x31, 0x33, 0x38);
    pub const POPUP: Color32 = Color32::from_rgb(0x2B, 0x2D, 0x31);
    pub const POPUP_HOVER: Color32 = Color32::from_rgb(0x35, 0x37, 0x3C);
    pub const INPUT: Color32 = Color32::from_rgb(0x38, 0x3A, 0x40);
    pub const CHIP_ACTIVE: Color32 = Color32::from_rgb(0x4E, 0x50, 0x58);
    pub const LINE: Color32 = Color32::from_rgb(0x1E, 0x1F, 0x22);
    pub const HEADER: Color32 = Color32::from_rgb(0xB5, 0xBA, 0xC1);
    pub const TEXT: Color32 = Color32::from_rgb(0xDB, 0xDE, 0xE1);
    pub const BRIGHT: Color32 = Color32::from_rgb(0xF2, 0xF3, 0xF5);
    pub const MUTED: Color32 = Color32::from_rgb(0x94, 0x9B, 0xA4);
    pub const ACTION: Color32 = Color32::from_rgb(0x3D, 0xFF, 0x6E);
}

/// The right-hand column: Discord's suggestion popup and chat box for the
/// command as it is now, and the event block saving makes available.
fn discord_preview(ui: &mut Ui, theme: &Theme, cmd: &SlashCommand, eye: Option<&std::path::Path>) {
    let choices_of = |i: usize| -> Vec<String> { cmd.options.get(i).map(|o| o.choices.iter().map(|c| c.name.clone()).collect()).unwrap_or_default() };
    // The option the popup shows: the first one with choices.
    let active = cmd.options.iter().enumerate().find(|(i, _)| !choices_of(*i).is_empty()).map(|(i, _)| i);
    let name = if cmd.name.is_empty() { "command".to_string() } else { cmd.name.clone() };

    Frame::new().fill(discord::PANEL).corner_radius(10).show(ui, |ui| ui.vertical(|ui| {
        ui.set_width(PREVIEW_W);
        ui.spacing_mut().item_spacing = vec2(6.0, 0.0);
        // Header.
        Frame::new().inner_margin(Margin::symmetric(16, 12)).show(ui, |ui| {
            ui.set_width(PREVIEW_W - 32.0);
            ui.horizontal(|ui| {
                if let Some(path) = eye {
                    let (r, _) = ui.allocate_exact_size(vec2(14.0, 14.0), egui::Sense::hover());
                    egui::Image::from_uri(crate::editor::file_uri(path)).tint(discord::HEADER).paint_at(ui, r);
                }
                ui.label(RichText::new(t!("How it looks in Discord")).size(12.0).color(discord::HEADER));
            });
        });
        let r = ui.min_rect();
        ui.painter().hline(r.x_range(), r.bottom(), Stroke::new(1.0, discord::LINE));

        Frame::new().inner_margin(Margin { left: 14, right: 14, top: 18, bottom: 14 }).show(ui, |ui| {
            ui.set_width(PREVIEW_W - 28.0);
            // The suggestion popup above the chat box.
            let top = egui::CornerRadius { nw: 8, ne: 8, sw: 0, se: 0 };
            Frame::new().fill(discord::POPUP).stroke(Stroke::new(1.0, discord::LINE)).corner_radius(top).show(ui, |ui| {
                ui.set_width(ui.available_width());
                let heading = |ui: &mut Ui, text: String| {
                    Frame::new().inner_margin(Margin { left: 12, right: 12, top: 10, bottom: 6 }).show(ui, |ui| {
                        ui.label(RichText::new(text.to_uppercase()).size(11.0).strong().color(discord::HEADER));
                    });
                };
                let row = |ui: &mut Ui, first: bool, main: &str, side: Option<&str>| {
                    Frame::new().fill(if first { discord::POPUP_HOVER } else { Color32::TRANSPARENT }).inner_margin(Margin::symmetric(12, 7)).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(main).size(14.0).color(if first { discord::BRIGHT } else { discord::TEXT }));
                            if let Some(s) = side.filter(|s| !s.is_empty()) {
                                ui.label(RichText::new(s).size(12.0).color(discord::MUTED));
                            }
                        });
                    });
                };
                match active {
                    Some(i) => {
                        let o = &cmd.options[i];
                        heading(ui, format!("{} · {}", o.name, o.description));
                        for (n, c) in choices_of(i).iter().enumerate() {
                            row(ui, n == 0, c, None);
                        }
                    }
                    None if !cmd.options.is_empty() => {
                        heading(ui, t!("Options").to_string());
                        for (n, o) in cmd.options.iter().enumerate() {
                            row(ui, n == 0, &o.name, Some(&o.description));
                        }
                    }
                    None => {
                        heading(ui, t!("Commands").to_string());
                        row(ui, true, &format!("/{name}"), Some(&cmd.description));
                    }
                }
                ui.add_space(4.0);
            });
            // The chat box with the command typed in.
            let bottom = egui::CornerRadius { nw: 0, ne: 0, sw: 8, se: 8 };
            Frame::new().fill(discord::INPUT).corner_radius(bottom).inner_margin(Margin::symmetric(12, 10)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(format!("/{name}")).monospace().size(14.0).color(discord::TEXT));
                    for (i, o) in cmd.options.iter().enumerate() {
                        let is_active = Some(i) == active;
                        let fill = if is_active { discord::CHIP_ACTIVE } else { discord::POPUP };
                        Frame::new().fill(fill).corner_radius(4).inner_margin(Margin::symmetric(6, 2)).show(ui, |ui| {
                            ui.spacing_mut().item_spacing.x = 4.0;
                            let label = if o.required { o.name.clone() } else { format!("{}?", o.name) };
                            if is_active {
                                ui.label(RichText::new(label).size(13.0).color(discord::BRIGHT));
                            } else {
                                ui.label(RichText::new(label).size(13.0).color(discord::MUTED));
                                ui.label(RichText::new("…").size(13.0).color(discord::TEXT));
                            }
                        });
                    }
                });
            });

            // The event block this command gives you.
            ui.add_space(18.0);
            let card = Frame::new().fill(Color32::from_rgb(0x2B, 0x2E, 0x33)).corner_radius(9).inner_margin(Margin::same(12)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.spacing_mut().item_spacing.y = 8.0;
                ui.label(RichText::new(t!("Saving makes this event block available in the block picker:")).size(12.0).color(theme.text));
                Frame::new().fill(Color32::from_rgb(0x36, 0x3A, 0x41)).stroke(Stroke::new(1.0, Color32::from_rgb(0x45, 0x4A, 0x52))).corner_radius(7).inner_margin(Margin::symmetric(10, 8)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(tf!("Slash Command · /{name}", name = name)).strong().color(Color32::from_rgb(0xE7, 0xEA, 0xED)));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let (r, _) = ui.allocate_exact_size(vec2(11.0, 13.0), egui::Sense::hover());
                            ui.painter().add(egui::Shape::convex_polygon(vec![r.left_top(), r.right_center(), r.left_bottom()], discord::ACTION, Stroke::NONE));
                        });
                    });
                });
                let outputs: Vec<String> = cmd
                    .options
                    .iter()
                    .map(|o| {
                        let kind = KINDS.iter().find(|k| k.0 == o.kind).map(|k| crate::i18n::t(k.1)).unwrap_or(t!("Text"));
                        format!("{} ({kind})", circuit_core::block::title_case(&o.name))
                    })
                    .collect();
                let text = if outputs.is_empty() { t!("It has no option outputs yet.").to_string() } else { tf!("Its outputs are your options: {list}.", list = outputs.join(", ")) };
                ui.label(RichText::new(text).size(12.0).color(theme.text_muted));
            });
            // Dashed outline, as in "something you can take from here".
            let r = card.response.rect;
            let stroke = Stroke::new(1.0, Color32::from_rgb(0x5C, 0x63, 0x6C));
            let pts = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
            ui.painter().extend(egui::Shape::dashed_line(&pts, stroke, 4.0, 3.0));
        });
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The same commands the runtime test turns into Discord's JSON
    /// (runtime/test/discord_blocks.js), so the editor and the bot agree.
    fn fixture(name: &str) -> Value {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../runtime/test/fixtures").join(name);
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    #[test]
    fn json_matches_what_the_bot_registers() {
        let input: Vec<SlashCommand> = serde_json::from_value(fixture("slash-input.json")).unwrap();
        assert_eq!(Value::Array(to_discord(&input)), fixture("slash-discord.json"));
    }

    #[test]
    fn pasted_json_comes_back_the_same() {
        let discord = fixture("slash-discord.json");
        let parsed = from_discord(&discord).unwrap();
        assert!(problems(&parsed).is_empty(), "{:?}", problems(&parsed));
        assert_eq!(Value::Array(to_discord(&parsed)), discord);
        let names: Vec<&str> = parsed.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["ping", "ticket", "ticket open", "ticket close", "admin", "admin reload", "admin config", "admin config set"]);
        assert_eq!(parsed[1].permissions, ["KickMembers", "ManageMessages"]);
        assert_eq!(parsed[2].locales["fr"].name, "ouvrir");
    }

    #[test]
    fn pasting_one_command_replaces_it() {
        let input: Vec<SlashCommand> = serde_json::from_value(fixture("slash-input.json")).unwrap();
        let mut edit = SlashEdit { commands: input, selected: 2, json: None };
        let pasted = json!({ "name": "support", "description": "Get help", "options": [{ "type": 5, "name": "urgent", "description": "Is it urgent?" }] });
        assert_eq!(apply_json(&mut edit, "ticket", &pasted).unwrap(), "support");
        let names: Vec<&str> = edit.commands.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["ping", "support", "admin config", "admin config set", "admin reload"]);
        assert_eq!(edit.selected, 1);
        assert_eq!(edit.commands[1].options[0].kind, "boolean");
        assert!(apply_json(&mut edit, "support", &json!({ "name": "who", "type": 2 })).is_err(), "context menus are blocks");
        assert!(apply_json(&mut edit, "support", &json!("text")).is_err());
    }

    #[test]
    fn older_projects_upgrade() {
        let mut c: SlashCommand = serde_json::from_value(json!({ "name": "kick", "description": "Kick", "permission": "KickMembers", "dm": true, "options": [{ "name": "why", "type": "string", "description": "Why", "max": 50 }] })).unwrap();
        upgrade(&mut c);
        assert_eq!(c.permissions, ["KickMembers"]);
        assert_eq!(c.contexts, ["guild", "botDm"]);
        assert_eq!((c.options[0].max_length, c.options[0].max), (Some(50), None));
        let saved = serde_json::to_value(&c).unwrap();
        assert!(saved.get("permission").is_none() && saved.get("dm").is_none(), "{saved}");
    }
}
