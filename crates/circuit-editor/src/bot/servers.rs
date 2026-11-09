//! The bot's servers with their channels and roles, read from Discord with
//! the project's bot token (in the background, when a project opens or its
//! token is saved), so id fields can offer them in a list instead of asking
//! for ids copied from Discord.

use std::path::Path;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};

use anyhow::{Context, Result, anyhow};
use serde_json::Value;

/// Servers read at most: each costs two requests.
const MAX_SERVERS: usize = 25;

/// A server the bot is in.
#[derive(Clone, Debug, PartialEq)]
pub struct Server {
    pub id: String,
    pub name: String,
    /// In Discord's order: categories, and the channels under each.
    pub channels: Vec<Channel>,
    /// Highest first, without @everyone.
    pub roles: Vec<Role>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Channel {
    pub id: String,
    pub name: String,
    /// Discord's channel type: 0 text, 2 voice, 4 category, 5 announcement, 13 stage, 15 forum, 16 media.
    pub kind: u64,
    pub parent: Option<String>,
    pub position: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Role {
    pub id: String,
    pub name: String,
    /// Made by an app or a boost (can't be given to members).
    pub managed: bool,
}

/// What a field lets you pick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pick {
    /// Channels you write in: text, announcement, forum, media.
    Channel,
    Voice,
    Category,
    Role,
}

impl Pick {
    /// From a field's `pick`, or the pin type of a channel id field.
    pub fn of(pick: Option<&str>, type_name: &str) -> Option<Pick> {
        match pick {
            Some("role") => return Some(Pick::Role),
            Some("category") => return Some(Pick::Category),
            Some("voice") => return Some(Pick::Voice),
            Some("channel") => return Some(Pick::Channel),
            _ => {}
        }
        match type_name.trim_end_matches('?') {
            "discord.voice_channel" => Some(Pick::Voice),
            "discord.channel" => Some(Pick::Channel),
            _ => None,
        }
    }

    pub fn takes(self, c: &Channel) -> bool {
        match self {
            Pick::Channel => matches!(c.kind, 0 | 5 | 15 | 16),
            Pick::Voice => matches!(c.kind, 2 | 13),
            Pick::Category => c.kind == 4,
            Pick::Role => false,
        }
    }
}

/// The servers shown, and a read under way.
#[derive(Default)]
pub struct Servers {
    pub shown: Option<Arc<Vec<Server>>>,
    /// Why the last read failed.
    pub error: Option<String>,
    /// The project has no saved bot token (or isn't a Discord bot).
    pub no_token: bool,
    /// `Ok(None)`: no token is saved.
    pending: Option<Receiver<Result<Option<Vec<Server>>, String>>>,
}

impl Servers {
    /// Reads the servers of the bot whose token the project at `root` has saved.
    pub fn fetch(&mut self, root: &Path, mode: &str) {
        self.pending = None;
        self.error = None;
        if !mode.contains("discord") {
            self.shown = None;
            self.no_token = true;
            return;
        }
        self.no_token = false;
        let (tx, rx) = channel();
        self.pending = Some(rx);
        let root = root.to_path_buf();
        // The token too is read there: the credential store can be slow.
        std::thread::spawn(move || {
            let servers = match circuit_runner::secrets::get(&root, "token") {
                Some(token) => load(&token).map(Some).map_err(|e| format!("{e:#}")),
                None => Ok(None),
            };
            let _ = tx.send(servers);
        });
    }

    /// Takes a finished read; true when what's shown changed.
    pub fn poll(&mut self) -> bool {
        let Some(result) = self.pending.as_ref().and_then(|p| p.try_recv().ok()) else { return false };
        self.pending = None;
        match result {
            Ok(Some(list)) => {
                self.shown = Some(Arc::new(list));
                self.error = None;
            }
            Ok(None) => {
                self.shown = None;
                self.no_token = true;
            }
            Err(e) => self.error = Some(e),
        }
        true
    }

    pub fn busy(&self) -> bool {
        self.pending.is_some()
    }
}

/// Asks Discord for the bot's servers, then each one's channels and roles.
fn load(token: &str) -> Result<Vec<Server>> {
    let guilds = get("https://discord.com/api/v10/users/@me/guilds?limit=200", token)?;
    let mut out = Vec::new();
    for g in guilds.as_array().into_iter().flatten().take(MAX_SERVERS) {
        let text = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
        let id = text(g, "id");
        if id.is_empty() {
            continue;
        }
        let channels = get(&format!("https://discord.com/api/v10/guilds/{id}/channels"), token).unwrap_or(Value::Null);
        let roles = get(&format!("https://discord.com/api/v10/guilds/{id}/roles"), token).unwrap_or(Value::Null);
        out.push(server(id.clone(), text(g, "name"), &channels, &roles));
    }
    Ok(out)
}

/// A server from Discord's answers: channels in the order Discord shows them
/// (each category, then the channels in it), roles highest first.
pub fn server(id: String, name: String, channels: &Value, roles: &Value) -> Server {
    let text = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
    let mut list: Vec<Channel> = channels
        .as_array()
        .into_iter()
        .flatten()
        .map(|c| Channel {
            id: text(c, "id"),
            name: text(c, "name"),
            kind: c.get("type").and_then(Value::as_u64).unwrap_or(0),
            parent: c.get("parent_id").and_then(Value::as_str).map(str::to_string),
            position: c.get("position").and_then(Value::as_i64).unwrap_or(0),
        })
        .collect();
    // Sorted by where they show: the category's place (or the channel's own, without one), then the channel's.
    let place = |c: &Channel, all: &[Channel]| -> (i64, u8, i64) {
        match c.parent.as_ref().and_then(|p| all.iter().find(|x| &x.id == p)) {
            Some(cat) => (cat.position, 1, c.position),
            None if c.kind == 4 => (c.position, 0, 0),
            None => (-1, 0, c.position),
        }
    };
    let keys: Vec<(i64, u8, i64)> = list.iter().map(|c| place(c, &list)).collect();
    let mut order: Vec<usize> = (0..list.len()).collect();
    order.sort_by_key(|&i| keys[i]);
    list = order.into_iter().map(|i| list[i].clone()).collect();
    let mut roles: Vec<(i64, Role)> = roles
        .as_array()
        .into_iter()
        .flatten()
        .filter(|r| text(r, "id") != id)
        .map(|r| (r.get("position").and_then(Value::as_i64).unwrap_or(0), Role { id: text(r, "id"), name: text(r, "name"), managed: r.get("managed").and_then(Value::as_bool).unwrap_or(false) }))
        .collect();
    roles.sort_by_key(|(p, _)| -p);
    Server { id, name, channels: list, roles: roles.into_iter().map(|(_, r)| r).collect() }
}

fn get(url: &str, token: &str) -> Result<Value> {
    let mut response = ureq::get(url)
        .header("User-Agent", "DiscordBot (https://github.com/xc-circuit, 1.0)")
        .header("Authorization", &format!("Bot {token}"))
        .call()
        .map_err(|e| match e {
            ureq::Error::StatusCode(401) => anyhow!("Discord didn't accept the bot token"),
            e => anyhow!("couldn't reach Discord: {e}"),
        })?;
    serde_json::from_slice(&response.body_mut().read_to_vec()?).context("Discord sent something unexpected")
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn channels_in_discords_order_and_roles_highest_first() {
        let channels = json!([
            { "id": "t2", "name": "rules", "type": 0, "parent_id": "cat1", "position": 1 },
            { "id": "cat1", "name": "INFO", "type": 4, "position": 0 },
            { "id": "v1", "name": "Lounge", "type": 2, "parent_id": "cat2", "position": 0 },
            { "id": "cat2", "name": "VOICE", "type": 4, "position": 1 },
            { "id": "t1", "name": "welcome", "type": 0, "parent_id": "cat1", "position": 0 },
            { "id": "top", "name": "announcements", "type": 5, "position": 0 }
        ]);
        let roles = json!([
            { "id": "g1", "name": "@everyone", "position": 0 },
            { "id": "r1", "name": "Member", "position": 1 },
            { "id": "r2", "name": "Mod", "position": 5 },
            { "id": "r3", "name": "Server Booster", "position": 2, "managed": true }
        ]);
        let s = server("g1".into(), "Home".into(), &channels, &roles);
        let names: Vec<&str> = s.channels.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["announcements", "INFO", "welcome", "rules", "VOICE", "Lounge"]);
        assert_eq!(s.roles.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(), ["Mod", "Server Booster", "Member"], "no @everyone");
        assert!(s.roles[1].managed);
        let text: Vec<&str> = s.channels.iter().filter(|c| Pick::Channel.takes(c)).map(|c| c.name.as_str()).collect();
        assert_eq!(text, ["announcements", "welcome", "rules"]);
        assert_eq!(s.channels.iter().filter(|c| Pick::Voice.takes(c)).count(), 1);
        assert_eq!(s.channels.iter().filter(|c| Pick::Category.takes(c)).count(), 2);
        assert_eq!(Pick::of(None, "discord.voice_channel?"), Some(Pick::Voice));
        assert_eq!(Pick::of(Some("role"), "text"), Some(Pick::Role));
        assert_eq!(Pick::of(None, "discord.thread"), None, "threads aren't listed by Discord here");
    }
}
