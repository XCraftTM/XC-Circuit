//! The bot's own name and picture, for the message previews: read from
//! Discord with the project's bot token (in the background, when a project
//! opens or its token is saved). The picture is kept in the app's cache.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};

use anyhow::{Context, Result, anyhow};
use eframe::egui::{self, Color32, Rect, Ui};
use serde_json::Value;

/// How the bot looks in Discord.
#[derive(Clone, Debug, PartialEq)]
pub struct BotProfile {
    /// Its display name, or its username.
    pub name: String,
    /// Its picture, downloaded to the cache.
    pub avatar: Option<PathBuf>,
}

/// The profile shown in previews, and one being read.
#[derive(Default)]
pub struct Profile {
    pub shown: Option<BotProfile>,
    /// `Ok(None)`: no token is saved.
    pending: Option<Receiver<Result<Option<BotProfile>, String>>>,
}

impl Profile {
    /// Reads the profile of the bot whose token the project at `root` has
    /// saved, when its build mode is a Discord one. Without a token the
    /// previews show "Your Bot".
    pub fn fetch(&mut self, root: &Path, mode: &str) {
        self.pending = None;
        if !mode.contains("discord") {
            self.shown = None;
            return;
        }
        let (tx, rx) = channel();
        self.pending = Some(rx);
        let root = root.to_path_buf();
        // The token too is read there: the credential store can be slow.
        std::thread::spawn(move || {
            let profile = match circuit_runner::secrets::get(&root, "token") {
                Some(token) => load(&token).map(Some).map_err(|e| format!("{e:#}")),
                None => Ok(None),
            };
            let _ = tx.send(profile);
        });
    }

    /// Takes a finished read: `Some(Err)` once when it failed.
    pub fn poll(&mut self) -> Option<Result<(), String>> {
        let result = self.pending.as_ref()?.try_recv().ok()?;
        self.pending = None;
        Some(result.map(|p| self.shown = p))
    }

    pub fn busy(&self) -> bool {
        self.pending.is_some()
    }
}

/// Asks Discord who the bot is, and downloads its picture unless it is cached.
fn load(token: &str) -> Result<BotProfile> {
    let me: Value = get("https://discord.com/api/v10/users/@me", Some(token))
        .and_then(|bytes| serde_json::from_slice(&bytes).context("Discord sent something unexpected"))?;
    let text = |k: &str| me.get(k).and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_string);
    let id = text("id").ok_or_else(|| anyhow!("Discord sent no bot id"))?;
    let name = text("global_name").or_else(|| text("username")).unwrap_or_else(|| "Bot".into());
    // Bots without a picture get one of Discord's default ones.
    let (url, file) = match text("avatar") {
        Some(hash) => (format!("https://cdn.discordapp.com/avatars/{id}/{hash}.png?size=128"), format!("{id}-{hash}.png")),
        None => {
            let n = id.parse::<u64>().map(|i| (i >> 22) % 6).unwrap_or(0);
            (format!("https://cdn.discordapp.com/embed/avatars/{n}.png"), format!("default-{n}.png"))
        }
    };
    let path = crate::bot::app_data_dir().join("cache").join("avatars").join(file);
    let avatar = if path.is_file() {
        Some(path)
    } else {
        get(&url, None).ok().and_then(|bytes| {
            std::fs::create_dir_all(path.parent()?).ok()?;
            std::fs::write(&path, bytes).ok()?;
            Some(path)
        })
    };
    Ok(BotProfile { name, avatar })
}

fn get(url: &str, token: Option<&str>) -> Result<Vec<u8>> {
    let mut request = ureq::get(url).header("User-Agent", "DiscordBot (https://github.com/xc-circuit, 1.0)");
    if let Some(token) = token {
        request = request.header("Authorization", &format!("Bot {token}"));
    }
    let mut response = request.call().map_err(|e| match e {
        ureq::Error::StatusCode(401) => anyhow!("Discord didn't accept the bot token"),
        e => anyhow!("couldn't reach Discord: {e}"),
    })?;
    Ok(response.body_mut().read_to_vec()?)
}

/// The bot's name in previews.
pub fn name(profile: Option<&BotProfile>) -> String {
    profile.map(|p| p.name.clone()).unwrap_or_else(|| t!("Your Bot").to_string())
}

/// The bot's picture in previews: a circle in `rect`, the accent color when
/// there is no picture.
pub fn paint_avatar(ui: &Ui, rect: Rect, profile: Option<&BotProfile>, fallback: Color32) {
    match profile.and_then(|p| p.avatar.as_ref()) {
        Some(path) => egui::Image::from_uri(crate::editor::file_uri(path)).corner_radius(rect.width() / 2.0).paint_at(ui, rect),
        None => {
            ui.painter().circle_filled(rect.center(), rect.width() / 2.0, fallback);
        }
    }
}
