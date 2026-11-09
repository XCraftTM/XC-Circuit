//! The Slash Command Builder: edits `circuit.json` › `data.slashCommands`.
//! Each command can also be edited as the JSON Discord stores it.

use std::collections::BTreeMap;

use eframe::egui::{self, Color32, Frame, Margin, RichText, ScrollArea, Stroke, Ui, vec2};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::ui::kit;
use crate::ui::dialog::{self, Part};
use crate::editor::{Editor, Level};
use crate::i18n::n;
use crate::builders::json_view::{self, JsonText};
use crate::builders::locales::{self, Localized};
use crate::ui::theme::Theme;

pub const DATA_KEY: &str = "slashCommands";
/// The side bar's folders for slash commands, in their order (kept while empty).
pub const FOLDERS_KEY: &str = "slashFolders";

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
    /// The side bar folder it is listed in (on every entry of a command).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
    /// Turned off: not registered with Discord, so it can't be used (on
    /// every entry of a command).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub disabled: bool,
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
/// A permission by Discord's flag name: what to call it, and its bit.
pub(crate) fn permission(name: &str) -> Option<(&'static str, u32)> {
    PERMISSIONS.iter().find(|p| p.0 == name).map(|p| (crate::i18n::t(p.1), p.2))
}

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

#[derive(Default)]
pub struct SlashEdit {
    pub(crate) commands: Vec<SlashCommand>,
    /// The project's commands when the draft was read, last saved or last
    /// brought up to date (see `follow_project`), and as they are kept there.
    loaded: Vec<SlashCommand>,
    seen: Option<Value>,
    selected: usize,
    /// A command was picked: in a narrow tab the list then folds into a bar.
    picked: bool,
    /// The folded list is open over the form.
    list_open: bool,
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

/// Opens Slash Commands. Open already, it stays as it is: its draft may
/// hold changes that aren't saved yet.
pub fn open(ed: &mut Editor) {
    if ed.slash_edit.is_some() {
        return;
    }
    let commands = load(ed);
    let seen = ed.project.as_ref().and_then(|p| p.meta.data.get(DATA_KEY)).cloned();
    ed.slash_edit = Some(SlashEdit { loaded: commands.clone(), commands, seen, ..Default::default() });
}

/// Brings the open builder's draft up to date when the project's commands
/// changed outside it: in the side bar (a command turned off, moved to a
/// folder or to another place), by Undo, or by an AI agent. A draft without
/// changes of its own becomes the project's commands; one with changes keeps
/// them and takes what the side bar decides: each command's folder, whether
/// it's on, and the order. Without this, Save wrote the older draft over
/// what the side bar had done.
pub fn follow_project(ed: &mut Editor) {
    let Some(edit) = &ed.slash_edit else { return };
    let now = ed.project.as_ref().and_then(|p| p.meta.data.get(DATA_KEY));
    if now == edit.seen.as_ref() {
        return;
    }
    let seen = now.cloned();
    let project = load(ed);
    let Some(edit) = &mut ed.slash_edit else { return };
    let shown = edit.commands.get(edit.selected).map(|c| c.name.clone());
    if edit.commands == edit.loaded {
        edit.commands = project.clone();
    } else {
        for c in &mut edit.commands {
            if let Some(p) = project.iter().find(|p| root_of(&p.name) == root_of(&c.name)) {
                (c.folder, c.disabled) = (p.folder.clone(), p.disabled);
            }
        }
        // The project's order; commands only the draft has stay after them, in theirs.
        let order = roots(&project);
        edit.commands.sort_by_key(|c| order.iter().position(|r| r == root_of(&c.name)).unwrap_or(usize::MAX));
    }
    (edit.loaded, edit.seen) = (project, seen);
    // The command that was shown stays the one shown.
    edit.selected = shown.and_then(|n| edit.commands.iter().position(|c| c.name == n)).unwrap_or(edit.selected).min(edit.commands.len().saturating_sub(1));
    // Its JSON is written again (unless one with a mistake is being fixed).
    if let Some((root, jt)) = &mut edit.json
        && jt.error.is_none()
    {
        root.clear();
    }
}

/// Opens Slash Commands on a command (its first word).
pub fn open_at(ed: &mut Editor, root: &str) {
    open(ed);
    if let Some(edit) = &mut ed.slash_edit
        && let Some(i) = edit.commands.iter().position(|c| root_of(&c.name) == root)
    {
        edit.selected = i;
        edit.picked = true;
    }
}

/// A project's commands, brought up to date.
pub fn commands_of(project: &circuit_core::Project) -> Vec<SlashCommand> {
    let mut list: Vec<SlashCommand> = project.meta.data.get(DATA_KEY).and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default();
    list.iter_mut().for_each(upgrade);
    list
}

/// The side bar's folders (paths, for folders in folders) in the tree's
/// order: the saved ones, and any a command names that the list lacks.
pub fn folders_of(project: &circuit_core::Project, commands: &[SlashCommand]) -> Vec<String> {
    let saved: Vec<String> = project.meta.data.get(FOLDERS_KEY).and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default();
    crate::editor::folders::ordered(&saved, commands.iter().filter_map(|c| c.folder.as_deref()))
}

/// Saves commands and folders into the project. Returns whether anything changed.
pub fn save_to(project: &mut circuit_core::Project, commands: &[SlashCommand], folders: &[String]) -> bool {
    let (c, f) = (serde_json::to_value(commands).unwrap_or(Value::Null), json!(folders));
    let changed = project.meta.data.get(DATA_KEY) != Some(&c) || project.meta.data.get(FOLDERS_KEY).unwrap_or(&json!([])) != &f;
    if changed {
        project.meta.data.insert(DATA_KEY.into(), c);
        if folders.is_empty() {
            project.meta.data.remove(FOLDERS_KEY);
        } else {
            project.meta.data.insert(FOLDERS_KEY.into(), f);
        }
    }
    changed
}

/// The commands' first words, in their order.
pub fn roots(commands: &[SlashCommand]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for c in commands {
        let r = root_of(&c.name);
        if !r.is_empty() && !out.iter().any(|o| o == r) {
            out.push(r.to_string());
        }
    }
    out
}

/// Moves a command (all its entries) into `folder`, before or after another
/// command (`next_to`), or where it was.
pub fn place(commands: &mut Vec<SlashCommand>, root: &str, folder: Option<String>, next_to: Option<(&str, bool)>) {
    let Some(was) = commands.iter().position(|c| root_of(&c.name) == root) else { return };
    let mut moving: Vec<SlashCommand> = Vec::new();
    let mut kept: Vec<SlashCommand> = Vec::new();
    for c in commands.drain(..) {
        if root_of(&c.name) == root { moving.push(c) } else { kept.push(c) }
    }
    for c in &mut moving {
        c.folder = folder.clone();
    }
    let at = match next_to.filter(|(other, _)| *other != root) {
        Some((other, true)) => kept.iter().position(|c| root_of(&c.name) == other),
        Some((other, false)) => kept.iter().rposition(|c| root_of(&c.name) == other).map(|i| i + 1),
        None => None,
    }
    .unwrap_or(was.min(kept.len()));
    kept.splice(at..at, moving);
    *commands = kept;
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
                let value = if numeric { num(ch.value.as_f64().or_else(|| ch.value.as_str().and_then(|s| s.trim().parse().ok())).unwrap_or(0.0)) } else { json!(crate::ui::widgets::as_text(&ch.value)) };
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
    // Turned-off commands aren't registered.
    let kept: Vec<SlashCommand> = cmds.iter().filter(|c| !c.disabled).cloned().collect();
    let cmds = &kept[..];
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
        // "0" is Discord's "nobody but administrators", not "everyone" (which
        // is the key left out): kept as Administrator, which says the same.
        let admins_only = match item.get("default_member_permissions") {
            Some(Value::String(s)) => s.trim() == "0",
            Some(Value::Number(n)) => n.as_u64() == Some(0),
            _ => false,
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
            permissions: if admins_only { vec!["Administrator".to_string()] } else { PERMISSIONS.iter().filter(|p| bits & (1u64 << p.2) != 0).map(|p| p.0.to_string()).collect() },
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

/// Discord counts at most this many characters per command: names,
/// descriptions and choice values of the command, its subcommands and
/// options (for each, its longest translation).
const MAX_COMMAND_CHARS: usize = 8000;
/// Commands (first words) a bot can register.
const MAX_COMMANDS: usize = 100;

/// The characters Discord counts toward [`MAX_COMMAND_CHARS`] for one entry.
fn counted_chars(c: &SlashCommand) -> usize {
    let longest = |text: &str, locales: &Locales, name: bool| {
        let own = text.chars().count();
        locales.values().map(|l| if name { l.name.chars().count() } else { l.description.chars().count() }).fold(own, usize::max)
    };
    // A subcommand's name is its last word.
    let own_name = c.name.rsplit(' ').next().unwrap_or("");
    let mut n = longest(own_name, &c.locales, true) + longest(&c.description, &c.locales, false);
    for o in &c.options {
        n += longest(&o.name, &o.locales, true) + longest(&o.description, &o.locales, false);
        for ch in &o.choices {
            n += longest(&ch.name, &ch.locales, true) + crate::ui::widgets::as_text(&ch.value).chars().count();
        }
    }
    n
}

fn problems(cmds: &[SlashCommand]) -> Vec<String> {
    let mut out = Vec::new();
    // Per command (first word): its size, how many subcommands it has, and
    // how many commands there are.
    let mut roots: Vec<&str> = Vec::new();
    for c in cmds.iter().filter(|c| !c.disabled) {
        let r = root_of(&c.name);
        if !r.is_empty() && !roots.contains(&r) {
            roots.push(r);
        }
    }
    if roots.len() > MAX_COMMANDS {
        out.push(tf!("{count} commands: Discord takes {max} per bot.", count = roots.len(), max = MAX_COMMANDS));
    }
    for root in &roots {
        let entries: Vec<&SlashCommand> = cmds.iter().filter(|c| root_of(&c.name) == *root).collect();
        let size: usize = entries.iter().map(|c| counted_chars(c)).sum();
        if size > MAX_COMMAND_CHARS {
            out.push(tf!("/{command}: {count} characters in names, descriptions and choices; Discord takes {max} per command.", command = root, count = size, max = MAX_COMMAND_CHARS));
        }
        // Subcommands and groups right under it, and subcommands in each group.
        let depth = |c: &&&SlashCommand| c.name.split_whitespace().count();
        let under = entries.iter().filter(|c| depth(c) == 2).count();
        if under > 25 {
            out.push(tf!("/{command}: {count} subcommands and groups; Discord takes 25.", command = root, count = under));
        }
        for group in entries.iter().filter(|c| depth(c) == 2 && is_header(cmds, &c.name)) {
            let inside = entries.iter().filter(|c| depth(c) == 3 && c.name.starts_with(&format!("{} ", group.name))).count();
            if inside > 25 {
                out.push(tf!("/{group}: {count} subcommands; Discord takes 25.", group = group.name, count = inside));
            }
        }
    }
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
        // Discord wants the options people must fill in first.
        if c.options.iter().skip_while(|o| o.required).any(|o| o.required) {
            out.push(tf!("{command}: required options go before the optional ones", command = label));
        }
        let mut seen: Vec<&str> = Vec::new();
        for o in &c.options {
            if !o.name.is_empty() && seen.contains(&o.name.as_str()) {
                out.push(tf!("{command}: two options are called “{name}”", command = label, name = o.name));
            }
            seen.push(&o.name);
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
                let value = crate::ui::widgets::as_text(&ch.value);
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
    follow_project(ed);
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
                    if kit::primary_button(ui, &theme, t!("Save and close")).clicked() {
                        (save, close) = (true, true);
                    }
                    if kit::secondary_button(ui, &theme, t!("Save")).clicked() {
                        save = true;
                    }
                });
                cancel = kit::secondary_button(ui, &theme, t!("Cancel")).clicked();
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
            let value = serde_json::to_value(&commands).unwrap_or(Value::Null);
            if let Some(edit) = &mut ed.slash_edit {
                (edit.loaded, edit.seen) = (commands.clone(), Some(value.clone()));
            }
            p.meta.data.insert(DATA_KEY.into(), value);
            ed.edited = true;
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
    let mut new = from_discord(v)?;
    // Its side bar folder and whether it's turned off aren't in Discord's
    // JSON: the command keeps them.
    if let Some(was) = edit.commands.iter().find(|c| root_of(&c.name) == root) {
        for c in &mut new {
            (c.folder, c.disabled) = (was.folder.clone(), was.disabled);
        }
    }
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

/// Below this the preview goes on top of the form, and the list of commands
/// gets out of the way once one is picked.
const SIDE_BY_SIDE: f32 = 230.0 + 18.0 + 380.0 + 20.0 + PREVIEW_W;

fn body(ui: &mut Ui, theme: &Theme, edit: &mut SlashEdit, issues: &[String], trash: Option<&std::path::Path>, eye: Option<&std::path::Path>) {
    let h = dialog::remaining_height(ui, 420.0);
    let top_down = egui::Layout::top_down(egui::Align::Min);
    if ui.available_width() >= SIDE_BY_SIDE {
        // The list, the selected command, and on the right how it looks in Discord.
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(vec2(230.0, h), top_down, |ui| {
                if command_list(ui, theme, edit, issues, trash) {
                    edit.picked = true;
                }
            });
            let r = ui.min_rect();
            ui.painter().vline(r.right() + 8.0, r.y_range(), Stroke::new(1.0, theme.line));
            ui.add_space(18.0);
            let form_w = (ui.available_width() - PREVIEW_W - 40.0).max(380.0);
            ui.allocate_ui_with_layout(vec2(form_w, h), top_down, |ui| form_column(ui, theme, edit, None));
            if let Some(cmd) = edit.commands.get(edit.selected) {
                ui.add_space(20.0);
                discord_preview(ui, theme, cmd, eye);
            }
        });
        return;
    }
    // Narrow: the list on its own until a command is picked; then that
    // command, with its preview on top, and the list folded into a bar on the
    // left that opens it over the form.
    if !edit.picked || edit.commands.get(edit.selected).is_none() {
        edit.list_open = false;
        ui.allocate_ui_with_layout(vec2(ui.available_width().min(420.0), h), top_down, |ui| {
            if command_list(ui, theme, edit, issues, trash) {
                edit.picked = true;
            }
            if edit.commands.is_empty() {
                ui.add_space(8.0);
                ui.label(RichText::new(t!("No slash commands yet. Click “+ New command”.")).color(theme.text_muted));
            }
        });
        return;
    }
    ui.horizontal_top(|ui| {
        let bar = list_bar(ui, theme, edit, issues, h);
        ui.add_space(12.0);
        ui.allocate_ui_with_layout(vec2(ui.available_width(), h), top_down, |ui| form_column(ui, theme, edit, Some(eye)));
        if edit.list_open {
            list_over(ui, theme, edit, issues, trash, bar);
        }
    });
}

/// The commands, "+ New command" and what's wrong with them. Returns true
/// when a command was picked (or made).
fn command_list(ui: &mut Ui, theme: &Theme, edit: &mut SlashEdit, issues: &[String], trash: Option<&std::path::Path>) -> bool {
    let mut picked = false;
    kit::section(ui, theme, t!("Commands"));
    let mut remove = None;
    let width = (ui.available_width() - 46.0).max(150.0);
    for (i, c) in edit.commands.iter().enumerate() {
        let selected = i == edit.selected;
        let label = if c.name.is_empty() { "/(no name)".to_string() } else { format!("/{}", c.name) };
        let text = RichText::new(label).monospace().color(if selected { theme.text } else { theme.text_muted });
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            if ui.add_sized([width, 34.0], egui::Button::selectable(selected, text)).clicked() {
                edit.selected = i;
                picked = true;
            }
            if kit::icon_button(ui, theme, trash, "×", t!("Delete this command")).clicked() {
                remove = Some(i);
            }
        });
    }
    if let Some(i) = remove {
        edit.commands.remove(i);
        edit.selected = edit.selected.min(edit.commands.len().saturating_sub(1));
    }
    ui.add_space(8.0);
    if kit::secondary_button(ui, theme, t!("+ New command")).clicked() {
        edit.commands.push(SlashCommand {
            name: format!("command{}", edit.commands.len() + 1),
            description: t!("Describe what it does").into(),
            contexts: vec!["guild".into()],
            ..Default::default()
        });
        edit.selected = edit.commands.len() - 1;
        picked = true;
    }
    if !issues.is_empty() {
        ui.add_space(14.0);
        for p in issues.iter().take(6) {
            ui.label(RichText::new(format!("• {p}")).size(11.5).color(theme.warning));
        }
    }
    picked
}

/// The folded list: a slim bar with an arrow, how many commands there are,
/// and a dot when something needs fixing. A click opens the list over the form.
fn list_bar(ui: &mut Ui, theme: &Theme, edit: &mut SlashEdit, issues: &[String], h: f32) -> egui::Rect {
    let (rect, resp) = ui.allocate_exact_size(vec2(30.0, h), egui::Sense::click());
    kit::describe(&resp, egui::Role::Button, t!("Commands"));
    let lit = resp.hovered() || edit.list_open;
    let fill = if lit { crate::ui::theme::mix(theme.panel_header, theme.text, 0.08) } else { theme.panel_header };
    let p = ui.painter();
    p.rect(rect, 7.0, fill, Stroke::new(1.0, theme.line), egui::StrokeKind::Inside);
    // An arrow pointing the way the list opens (or closes).
    let c = egui::pos2(rect.center().x, rect.top() + 20.0);
    let d = if edit.list_open { -1.0 } else { 1.0 };
    let s = Stroke::new(1.8, if lit { theme.text } else { theme.text_muted });
    p.line_segment([c + vec2(-3.0 * d, -5.0), c + vec2(3.0 * d, 0.0)], s);
    p.line_segment([c + vec2(3.0 * d, 0.0), c + vec2(-3.0 * d, 5.0)], s);
    p.text(egui::pos2(rect.center().x, rect.top() + 44.0), egui::Align2::CENTER_CENTER, edit.commands.len().to_string(), egui::FontId::proportional(12.0), theme.text_muted);
    if !issues.is_empty() {
        p.circle_filled(egui::pos2(rect.center().x, rect.top() + 62.0), 4.0, theme.warning);
    }
    let tip = if issues.is_empty() { t!("Show the commands").to_string() } else { tf!("Show the commands · {count} to fix", count = issues.len()) };
    if resp.on_hover_text(tip).clicked() {
        edit.list_open = !edit.list_open;
    }
    rect
}

/// The list opened from the bar, over the form. Picking a command, a click
/// outside or Escape closes it.
fn list_over(ui: &mut Ui, theme: &Theme, edit: &mut SlashEdit, issues: &[String], trash: Option<&std::path::Path>, bar: egui::Rect) {
    let card = Frame::new()
        .fill(theme.panel)
        .stroke(Stroke::new(1.0, theme.node_border))
        .corner_radius(10)
        .inner_margin(Margin::same(12))
        .shadow(egui::Shadow { offset: [0, 10], blur: 28, spread: 0, color: Color32::from_black_alpha(130) });
    let area = egui::Area::new(ui.scope_id().with("slash-list-over")).order(egui::Order::Foreground).fixed_pos(bar.right_top() + vec2(6.0, 0.0)).show(ui.ctx(), |ui| {
        card.show(ui, |ui| {
            ui.set_width(260.0);
            ScrollArea::vertical().id_salt("slash-list-over-scroll").max_height(bar.height() - 24.0).show(ui, |ui| {
                if command_list(ui, theme, edit, issues, trash) {
                    edit.list_open = false;
                }
            });
        });
    });
    let outside = ui.input(|i| i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|p| !area.response.rect.contains(p) && !bar.contains(p)));
    if outside || ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
        edit.list_open = false;
    }
}

/// The selected command: Builder | JSON, and its form or its JSON. With
/// `preview_on_top` (narrow tabs), how it looks in Discord goes above it.
fn form_column(ui: &mut Ui, theme: &Theme, edit: &mut SlashEdit, preview_on_top: Option<Option<&std::path::Path>>) {
    let Some(cmd) = edit.commands.get(edit.selected) else {
        ui.add_space(40.0);
        ui.label(RichText::new(t!("No slash commands yet. Click “+ New command”.")).color(theme.text_muted));
        return;
    };
    if let Some(eye) = preview_on_top {
        dialog::preview_on_top(ui, "slash-preview", 0.4, |ui| discord_preview(ui, theme, cmd, eye));
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
            if let Some(v) = json_view::editor(ui, theme, ui.scope_id().with("slash-json"), jt, &help) {
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
}

fn command_form(ui: &mut Ui, theme: &Theme, edit: &mut SlashEdit, root: &str) {
    let i = edit.selected;
    let header = is_header(&edit.commands, &edit.commands[i].name);
    let holds_settings = settings_holder(&edit.commands, root) == Some(i);
    let cmd = &mut edit.commands[i];
    kit::section(ui, theme, t!("Command"));
    ui.columns(2, |cols| {
        kit::field(&mut cols[0], theme, t!("Name"), Some(t!("What people type after /. Two words make a subcommand: ticket open")), |ui| kit::text_input(ui, &mut cmd.name, "rank", false));
        kit::field(&mut cols[1], theme, t!("Description"), Some(t!("Shown in Discord's command list (up to 100 characters)")), |ui| {
            kit::text_input(ui, &mut cmd.description, t!("Shows your rank"), false)
        });
    });
    locales::section(ui, theme, ui.scope_id().with(("cmd-locales", i)), &mut cmd.locales, true);
    ui.add_space(6.0);

    if holds_settings {
        kit::section(ui, theme, t!("Who and where"));
        ui.horizontal(|ui| {
            ui.set_min_height(crate::ui::kit::input_height(ui));
            let name = ui.label(RichText::new(t!("Who can use it")).color(theme.text)).id;
            kit::labelled(ui, name, |ui| permissions_dropdown(ui, theme, &mut cmd.permissions));
            ui.add_space(16.0);
            let name = ui.label(RichText::new(t!("Age-restricted")).color(theme.text)).id;
            crate::ui::kit::toggle(ui, &mut cmd.nsfw, theme).labelled_by(name).on_hover_text(t!("Only usable in age-restricted (NSFW) channels."));
        });
        kit::hint(ui, theme, t!("People without all of these permissions don't see the command. Server admins can change it in Server Settings › Integrations."));
        ui.add_space(6.0);
        pick_row(ui, theme, t!("Works in"), &CONTEXTS, &mut cmd.contexts, t!("Nowhere"));
        pick_row(ui, theme, t!("When the bot is"), &INSTALLS, &mut cmd.installs, t!("Added to a server"));
        kit::hint(ui, theme, t!("Added to an account, the bot's commands work in any server or DM that person is in. Turn it on in the Discord developer portal › Installation too."));
    } else {
        ui.label(RichText::new(tf!("Who can use it and where it works are set on /{command}.", command = root)).size(12.0).color(theme.text_muted));
    }

    if header {
        ui.add_space(8.0);
        ui.label(RichText::new(tf!("/{command} only groups its subcommands, so the options are on them.", command = cmd.name)).size(12.0).color(theme.text_muted));
        return;
    }
    kit::section(ui, theme, &tf!("Options · {count} of 25", count = cmd.options.len()));
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
    if cmd.options.len() < 25 && kit::secondary_button(ui, theme, t!("+ Add option")).clicked() {
        cmd.options.push(SlashOption { name: format!("option{}", cmd.options.len() + 1), description: t!("Describe this option").into(), ..Default::default() });
    }
    kit::hint(ui, theme, t!("Discord lists required options first."));
}

/// A row with a label and a dropdown picking any number of `options` (keys
/// with their English labels), centered on one line.
fn pick_row(ui: &mut Ui, theme: &Theme, label: &str, options: &[(&str, u64, &'static str)], list: &mut Vec<String>, none: &str) {
    ui.horizontal(|ui| {
        ui.set_min_height(crate::ui::kit::input_height(ui));
        let name = ui.label(RichText::new(label).color(theme.text)).id;
        let options: Vec<(String, &'static str)> = options.iter().map(|(k, _, l)| (k.to_string(), *l)).collect();
        kit::labelled(ui, name, |ui| crate::ui::kit::multi_select(ui, theme, ui.scope_id().with(label), 260.0, &options, list, none));
    });
}

/// Picks any number of permissions; the summary says how many.
fn permissions_dropdown(ui: &mut Ui, theme: &Theme, list: &mut Vec<String>) {
    let name = |k: &str| PERMISSIONS.iter().find(|p| p.0 == k).map(|p| crate::i18n::t(p.1)).unwrap_or(k).to_string();
    let summary = match list.len() {
        0 => t!("Everyone").to_string(),
        1 => name(&list[0]),
        n => tf!("{first} + {more} more", first = name(&list[0]), more = n - 1),
    };
    let resp = ui.scope(|ui| {
        ui.spacing_mut().interact_size.y = crate::ui::kit::input_height(ui);
        egui::ComboBox::from_id_salt(ui.scope_id().with("permissions")).width(240.0).selected_text(summary).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show_ui(ui, |ui| {
            if kit::option(ui, list.is_empty(), t!("Everyone")).clicked() {
                list.clear();
            }
            ui.separator();
            ScrollArea::vertical().max_height(340.0).show(ui, |ui| {
                for (key, label, _) in PERMISSIONS {
                    let mut on = list.iter().any(|k| k == key);
                    if kit::checkbox(ui, theme, &mut on, crate::i18n::t(label)).changed() {
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
    .response;
    kit::named(ui, resp).on_hover_text(t!("People need all the permissions you tick."));
}

fn option_card(ui: &mut Ui, theme: &Theme, o: &mut SlashOption, i: usize, count: usize, remove: &mut Option<usize>, swap: &mut Option<(usize, usize)>) {
    Frame::new().fill(theme.panel_header).stroke(Stroke::new(1.0, theme.line)).corner_radius(9).inner_margin(Margin::same(12)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        let numeric = matches!(o.kind.as_str(), "integer" | "number");
        let texty = o.kind == "string";
        ui.horizontal(|ui| {
            let h = crate::ui::kit::input_height(ui);
            ui.set_min_height(h);
            ui.allocate_ui(vec2(160.0, h), |ui| kit::text_input(ui, &mut o.name, "name", false));
            let current = KINDS.iter().find(|k| k.0 == o.kind).map(|k| crate::i18n::t(k.1)).unwrap_or(t!("Text"));
            crate::ui::kit::dropdown(ui, ui.scope_id().with("kind"), 130.0, current, |ui| {
                for (k, l, _) in KINDS {
                    ui.selectable_value(&mut o.kind, k.to_string(), crate::i18n::t(l));
                }
            })
            .accessible_name(t!("Type"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if kit::danger_button(ui, theme, t!("Remove")).clicked() {
                    *remove = Some(i);
                }
                if ui.add_enabled_ui(i + 1 < count, |ui| kit::button(ui, theme, t!("Down"))).inner.clicked() {
                    *swap = Some((i, i + 1));
                }
                if ui.add_enabled_ui(i > 0, |ui| kit::button(ui, theme, t!("Up"))).inner.clicked() {
                    *swap = Some((i, i - 1));
                }
            });
        });
        ui.add_space(6.0);
        kit::text_input(ui, &mut o.description, t!("What this option is for"), false);
        ui.add_space(6.0);
        // The switches on a row of their own, so the row above never runs out of room.
        ui.horizontal(|ui| {
            ui.set_min_height(crate::ui::kit::input_height(ui));
            let name = ui.label(RichText::new(t!("Required")).size(12.0).color(theme.text_muted)).id;
            crate::ui::kit::toggle(ui, &mut o.required, theme).labelled_by(name);
            if (texty || numeric) && o.choices.is_empty() {
                ui.add_space(16.0);
                let name = ui.label(RichText::new(t!("Autocomplete")).size(12.0).color(theme.text_muted)).id;
                crate::ui::kit::toggle(ui, &mut o.autocomplete, theme).labelled_by(name).on_hover_text(t!("Suggest values while people type, from an Autocomplete block."));
            }
        });
        if texty {
            ui.horizontal(|ui| {
                ui.set_min_height(crate::ui::kit::input_height(ui));
                ui.label(RichText::new(t!("Length")).size(12.0).color(theme.text_muted));
                length_box(ui, t!("Shortest"), &mut o.min_length);
                length_box(ui, t!("Longest"), &mut o.max_length);
            });
        } else if numeric {
            ui.horizontal(|ui| {
                ui.set_min_height(crate::ui::kit::input_height(ui));
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
        locales::section(ui, theme, ui.scope_id().with("locales"), &mut o.locales, true);
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
            ui.set_min_height(crate::ui::kit::input_height(ui));
            let field_w = ((w - 230.0) / 2.0).max(120.0);
            ui.add(crate::ui::kit::TextLine::new(&mut ch.name).hint(t!("Name people see")).width(field_w));
            // A number as it is typed (1, not 1.0).
            let shown = match ch.value.as_f64() {
                Some(n) if numeric => n.to_string(),
                _ => crate::ui::widgets::as_text(&ch.value),
            };
            if let Some(value) = typed_box(ui, shown, t!("Value the bot gets"), field_w) {
                ch.value = if numeric { value.trim().parse::<f64>().map(Value::from).unwrap_or(Value::from(value)) } else { Value::from(value) };
            }
            let label = if ch.locales.is_empty() { t!("Translations").to_string() } else { tf!("Translations · {count}", count = ch.locales.len()) };
            let resp = kit::button(ui, theme, label);
            egui::Popup::menu(&resp).id(ui.scope_id().with(("choice-locales", i))).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
                ui.set_width(460.0);
                locales::editor(ui, theme, ui.scope_id().with(("choice-rows", i)), &mut ch.locales, false);
            });
            if kit::danger_button(ui, theme, "×").on_hover_text(t!("Remove this choice")).clicked() {
                remove = Some(i);
            }
        });
    }
    if let Some(i) = remove {
        o.choices.remove(i);
    }
    if o.choices.len() < 25 && kit::button(ui, theme, t!("+ Add choice")).clicked() {
        let n = o.choices.len() + 1;
        let value = if numeric { Value::from(n) } else { Value::from(format!("choice{n}")) };
        o.choices.push(Choice { name: tf!("Choice {number}", number = n), value, locales: Locales::new() });
    }
}

/// A text box for a value kept as something else (a number): while it has
/// the text cursor it shows what was typed, not the value written out again
/// after each key, which would take "-" and the point of "1." away before
/// the number is whole. Returns the text when it changed.
fn typed_box(ui: &mut Ui, shown: String, hint: &str, width: f32) -> Option<String> {
    // The id the box gets (see `TextLine`).
    let id = ui.next_auto_id();
    let key = id.with("typed");
    let mut text = ui.data(|d| d.get_temp::<String>(key)).filter(|_| ui.memory(|m| m.has_focus(id))).unwrap_or(shown);
    let resp = ui.add(crate::ui::kit::TextLine::new(&mut text).hint(hint).width(width));
    let changed = resp.changed();
    let typing = resp.has_focus();
    ui.data_mut(|d| {
        if typing {
            d.insert_temp(key, text.clone());
        } else {
            d.remove::<String>(key);
        }
    });
    changed.then_some(text)
}

fn number_box(ui: &mut Ui, label: &str, v: &mut Option<f64>) {
    let shown = v.map(|n| n.to_string()).unwrap_or_default();
    if let Some(text) = typed_box(ui, shown, label, 96.0) {
        *v = text.trim().parse().ok();
    }
}

fn length_box(ui: &mut Ui, label: &str, v: &mut Option<u32>) {
    let mut text = v.map(|n| n.to_string()).unwrap_or_default();
    if ui.add(crate::ui::kit::TextLine::new(&mut text).hint(label).width(96.0)).changed() {
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

    /// The builder, open with changes that aren't saved: a click on a command
    /// in the side bar shows that command and keeps the changes, and what the
    /// side bar does meanwhile (a command turned off, moved) isn't undone by Save.
    #[test]
    fn the_open_builder_keeps_its_draft_and_follows_the_side_bar() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let dir = std::env::temp_dir().join(format!("xc-slash-follow-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        circuit_core::Project::create(&dir, "S", "discord-bot").unwrap();
        let mut ed = Editor::new(vec![root.join("plugins")], None).unwrap();
        ed.open_project(&dir);
        let saved = json!([{ "name": "ping", "description": "Pong", "options": [] }, { "name": "help", "description": "Help", "options": [] }]);
        ed.project.as_mut().unwrap().meta.data.insert(DATA_KEY.into(), saved);
        open(&mut ed);
        ed.slash_edit.as_mut().unwrap().commands[0].description = "Changed".into();
        open_at(&mut ed, "help");
        let edit = ed.slash_edit.as_ref().unwrap();
        assert_eq!((edit.commands[0].description.as_str(), edit.selected), ("Changed", 1), "the draft stays, and help is shown");

        // The side bar: ping turned off, help into a folder and above ping.
        let project = ed.project.as_mut().unwrap();
        let mut commands = commands_of(project);
        commands[0].disabled = true;
        place(&mut commands, "help", Some("Info".into()), Some(("ping", true)));
        assert!(save_to(project, &commands, &["Info".to_string()]));
        follow_project(&mut ed);
        let edit = ed.slash_edit.as_ref().unwrap();
        let names: Vec<&str> = edit.commands.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["help", "ping"]);
        assert_eq!(edit.selected, 0, "help is still the one shown");
        assert_eq!(edit.commands[0].folder.as_deref(), Some("Info"));
        assert!(edit.commands[1].disabled && edit.commands[1].description == "Changed", "turned off, with the draft's change");

        // A draft without changes of its own takes the project's commands whole.
        ed.slash_edit = None;
        open(&mut ed);
        ed.project.as_mut().unwrap().meta.data.insert(DATA_KEY.into(), json!([{ "name": "only", "description": "One", "options": [] }]));
        follow_project(&mut ed);
        assert_eq!(ed.slash_edit.as_ref().unwrap().commands.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["only"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Smallest and Largest take a number typed key by key, with its minus
    /// and its point (-5.2 used to end up as 52).
    #[test]
    fn a_number_is_typed_with_its_minus_and_point() {
        let ctx = egui::Context::default();
        crate::ui::fonts::install(&ctx);
        let mut v: Option<f64> = None;
        let run = |events: Vec<egui::Event>, v: &mut Option<f64>| {
            let raw = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, vec2(400.0, 200.0))), events, ..Default::default() };
            let mut out = ctx.run_ui(raw, |ui| number_box(ui, "Smallest", v));
            out.textures_delta.clear();
        };
        let p = egui::pos2(30.0, 14.0);
        let press = |pressed: bool| egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        run(vec![], &mut v);
        run(vec![egui::Event::PointerMoved(p), press(true), press(false)], &mut v);
        run(vec![], &mut v);
        let mut seen = Vec::new();
        for key in ["-", "5", ".", "2"] {
            run(vec![egui::Event::Text(key.into())], &mut v);
            run(vec![], &mut v);
            seen.push(v);
        }
        assert_eq!(seen, [None, Some(-5.0), Some(-5.0), Some(-5.2)]);
    }

    /// A command edited as JSON stays in its side bar folder, and turned off
    /// when it was: Discord's JSON has neither.
    #[test]
    fn json_edits_keep_the_folder_and_the_switch() {
        let cmd = |name: &str| SlashCommand { name: name.into(), description: "d".into(), folder: Some("Fun".into()), disabled: true, contexts: vec!["guild".into()], ..Default::default() };
        let other = SlashCommand { name: "help".into(), description: "d".into(), contexts: vec!["guild".into()], ..Default::default() };
        let mut edit = SlashEdit { commands: vec![other, cmd("game start"), cmd("game stop")], selected: 1, picked: true, ..Default::default() };
        let mut v = root_json(&edit.commands, "game");
        v["name"] = json!("play");
        assert_eq!(apply_json(&mut edit, "game", &v).unwrap(), "play");
        let names: Vec<&str> = edit.commands.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["help", "play", "play start", "play stop"]);
        assert!(edit.commands[1..].iter().all(|c| c.folder.as_deref() == Some("Fun") && c.disabled), "{:?}", edit.commands);
        assert!(edit.commands[0].folder.is_none() && !edit.commands[0].disabled);
    }

    /// `"default_member_permissions": "0"` means nobody but administrators:
    /// a command pasted with it doesn't become one for everyone.
    #[test]
    fn a_command_for_admins_only_stays_one() {
        for zero in [json!("0"), json!(0)] {
            let cmds = from_discord(&json!({ "name": "secret", "description": "d", "default_member_permissions": zero })).unwrap();
            assert_eq!(cmds[0].permissions, ["Administrator"]);
            assert_eq!(root_json(&cmds, "secret")["default_member_permissions"], "8");
        }
        // Left out is everyone, as before; other bits are kept.
        let open = from_discord(&json!({ "name": "open", "description": "d" })).unwrap();
        assert!(open[0].permissions.is_empty() && root_json(&open, "open").get("default_member_permissions").is_none());
        let some = from_discord(&json!({ "name": "mod", "description": "d", "default_member_permissions": "6" })).unwrap();
        assert_eq!(some[0].permissions, ["KickMembers", "BanMembers"]);
    }

    /// Commands move in the side bar as a whole, subcommands included, and
    /// turned-off ones aren't registered.
    #[test]
    fn commands_move_and_turn_off() {
        let cmd = |name: &str| SlashCommand { name: name.into(), description: "d".into(), contexts: vec!["guild".into()], ..Default::default() };
        let mut list = vec![cmd("ping"), cmd("ticket open"), cmd("ticket close"), cmd("help")];
        assert_eq!(roots(&list), ["ping", "ticket", "help"]);
        place(&mut list, "ticket", Some("Support".into()), Some(("ping", true)));
        let names: Vec<&str> = list.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["ticket open", "ticket close", "ping", "help"]);
        assert!(list[..2].iter().all(|c| c.folder.as_deref() == Some("Support")), "every entry goes into the folder");
        place(&mut list, "ticket", None, Some(("help", false)));
        assert_eq!(roots(&list), ["ping", "help", "ticket"]);
        place(&mut list, "help", Some("Info".into()), None);
        assert_eq!(roots(&list), ["ping", "help", "ticket"], "into a folder without a place: it stays where it was");
        list.iter_mut().filter(|c| c.name.starts_with("ticket")).for_each(|c| c.disabled = true);
        let sent: Vec<String> = to_discord(&list).iter().map(|c| c["name"].as_str().unwrap().to_string()).collect();
        assert_eq!(sent, ["ping", "help"], "turned-off commands aren't registered");
    }

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
        let mut edit = SlashEdit { commands: input, selected: 2, picked: true, ..Default::default() };
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

    /// Discord's limits (from its docs) on a whole command: required options
    /// first, names once, 25 subcommands and 8000 characters per command.
    #[test]
    fn problems_follow_discords_command_limits() {
        let opt = |name: &str, required: bool| SlashOption { name: name.into(), kind: "string".into(), description: "d".into(), required, ..Default::default() };
        let cmd = |name: &str, options: Vec<SlashOption>| SlashCommand { name: name.into(), description: "d".into(), contexts: vec!["guild".into()], options, ..Default::default() };
        let order = vec![cmd("ask", vec![opt("topic", false), opt("who", true), opt("topic", false)])];
        let p = problems(&order).join("\n");
        assert!(p.contains("required options go before the optional ones") && p.contains("two options are called “topic”"), "{p}");
        assert!(problems(&[cmd("ask", vec![opt("who", true), opt("topic", false)])]).is_empty());

        let mut many = vec![cmd("admin", vec![])];
        many.extend((0..26).map(|i| cmd(&format!("admin s{i}"), vec![])));
        assert!(problems(&many).iter().any(|p| p.contains("26 subcommands and groups")), "{:?}", problems(&many));

        // 26 subcommands of 25 options with long descriptions: over 8000 characters.
        let long = |n: &str| SlashOption { description: "x".repeat(100), ..opt(n, false) };
        let mut big = vec![cmd("big", vec![])];
        big.extend((0..4).map(|i| SlashCommand { description: "y".repeat(100), ..cmd(&format!("big s{i}"), (0..25).map(|j| long(&format!("o{j}"))).collect()) }));
        assert!(problems(&big).iter().any(|p| p.contains("Discord takes 8000 per command")), "{:?}", problems(&big));
    }
}
