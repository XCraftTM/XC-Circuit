//! The Slash Command Builder: edits `circuit.json` › `data.slashCommands`.

use eframe::egui::{self, Color32, ComboBox, Frame, Margin, RichText, ScrollArea, Stroke, Ui, vec2};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::dialog::{self, Part};
use crate::i18n::n;
use crate::editor::{Editor, Level};
use crate::theme::Theme;

pub const DATA_KEY: &str = "slashCommands";

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct SlashCommand {
    /// One word, or two or three for a subcommand: "ticket open".
    pub name: String,
    pub description: String,
    pub options: Vec<SlashOption>,
    /// The permission needed to see and use it (a discord.js flag name); empty = everyone.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub permission: String,
    /// Also usable in direct messages with the bot.
    pub dm: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct SlashOption {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub description: String,
    pub required: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub choices: Vec<Choice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    /// Suggestions come from an Autocomplete block while people type.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub autocomplete: bool,
}

impl Default for SlashOption {
    fn default() -> Self {
        SlashOption { name: String::new(), kind: "string".into(), description: String::new(), required: false, choices: Vec::new(), min: None, max: None, autocomplete: false }
    }
}

/// Who may use a command: the permission it needs.
const PERMISSIONS: [(&str, &str); 9] = [
    ("", n("Everyone")),
    ("ManageMessages", n("Manage Messages")),
    ("ModerateMembers", n("Time Out Members")),
    ("KickMembers", n("Kick Members")),
    ("BanMembers", n("Ban Members")),
    ("ManageRoles", n("Manage Roles")),
    ("ManageChannels", n("Manage Channels")),
    ("ManageGuild", n("Manage Server")),
    ("Administrator", n("Administrators")),
];

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Choice {
    pub name: String,
    pub value: Value,
}

const KINDS: [(&str, &str); 9] = [
    ("string", n("Text")),
    ("integer", n("Whole number")),
    ("number", n("Number")),
    ("boolean", n("Yes / no")),
    ("user", n("User")),
    ("channel", n("Channel")),
    ("role", n("Role")),
    ("mentionable", n("User or role")),
    ("attachment", n("File")),
];

pub struct SlashEdit {
    commands: Vec<SlashCommand>,
    selected: usize,
    /// Choices typed as "Low, Medium, High", per (command, option).
    choice_text: Vec<Vec<String>>,
}

pub fn load(ed: &Editor) -> Vec<SlashCommand> {
    ed.project
        .as_ref()
        .and_then(|p| p.meta.data.get(DATA_KEY))
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default()
}

pub fn open(ed: &mut Editor) {
    let commands = load(ed);
    let choice_text = commands
        .iter()
        .map(|c| c.options.iter().map(|o| o.choices.iter().map(|ch| ch.name.clone()).collect::<Vec<_>>().join(", ")).collect())
        .collect();
    ed.slash_edit = Some(SlashEdit { commands, selected: 0, choice_text });
}

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
        // A command with subcommands can't have options of its own.
        if !c.options.is_empty() && cmds.iter().any(|o| o.name.starts_with(&format!("{} ", c.name))) {
            out.push(tf!("{command}: has subcommands, so its options belong to them", command = label));
        }
        if cmds.iter().filter(|o| o.name == c.name).count() > 1 && !c.name.is_empty() {
            out.push(format!("{label}: the name is used twice"));
        }
        if c.description.trim().is_empty() || c.description.len() > 100 {
            out.push(format!("{label}: description must be 1–100 characters"));
        }
        for o in &c.options {
            if let Some(p) = name_problem(&o.name) {
                out.push(format!("{label} › option: {p}"));
            }
            if o.description.trim().is_empty() {
                out.push(format!("{label} › {}: needs a description", o.name));
            }
        }
    }
    out.dedup();
    out
}

/// The Slash Commands tab.
pub fn tab(ui: &mut Ui, ed: &mut Editor) {
    let Some(edit) = &mut ed.slash_edit else { return };
    let theme = ed.theme.clone();
    let icon = ed.reg.icons.get("slash").cloned();
    let trash = ed.reg.icons.get("trash").cloned();
    let eye = ed.reg.icons.get("eye").cloned();
    let issues = problems(&edit.commands);
    let (mut save, mut close, mut cancel) = (false, false, false);
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
                ui.add_enabled_ui(issues.is_empty(), |ui| {
                    if dialog::primary_button(ui, &theme, t!("Save and close")).clicked() {
                        (save, close) = (true, true);
                    }
                    if dialog::secondary_button(ui, &theme, t!("Save")).clicked() {
                        save = true;
                    }
                });
                cancel = dialog::secondary_button(ui, &theme, t!("Cancel")).clicked();
                if !issues.is_empty() {
                    ui.label(RichText::new(tf!("{count} to fix", count = issues.len())).size(12.0).color(theme.warning));
                }
            }
        },
    );
    if save {
        // Save keeps the editor open with the same draft; Save and close ends it.
        let edit = ed.slash_edit.as_ref().unwrap();
        let (mut commands, choice_text) = (edit.commands.clone(), edit.choice_text.clone());
        if close {
            ed.slash_edit = None;
        }
        // Turn the typed choice lists into choices.
        for (c, texts) in commands.iter_mut().zip(choice_text) {
            for (o, text) in c.options.iter_mut().zip(texts) {
                let numeric = o.kind == "integer" || o.kind == "number";
                o.choices = text
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(|s| Choice {
                        name: s.to_string(),
                        value: if numeric { s.parse::<f64>().map(Value::from).unwrap_or(Value::from(s)) } else { Value::from(s.to_lowercase()) },
                    })
                    .collect();
                if !matches!(o.kind.as_str(), "string" | "integer" | "number") {
                    o.choices.clear();
                    o.min = None;
                    o.max = None;
                }
            }
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

fn body(ui: &mut Ui, theme: &Theme, edit: &mut SlashEdit, issues: &[String], trash: Option<&std::path::Path>, eye: Option<&std::path::Path>) {
    ui.horizontal_top(|ui| {
        // Command list.
        ui.allocate_ui_with_layout(vec2(230.0, 520.0), egui::Layout::top_down(egui::Align::Min), |ui| {
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
                edit.choice_text.remove(i);
                edit.selected = edit.selected.min(edit.commands.len().saturating_sub(1));
            }
            ui.add_space(8.0);
            if dialog::secondary_button(ui, theme, t!("+ New command")).clicked() {
                edit.commands.push(SlashCommand { name: format!("command{}", edit.commands.len() + 1), description: t!("Describe what it does").into(), ..Default::default() });
                edit.choice_text.push(Vec::new());
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
        ui.allocate_ui_with_layout(vec2(form_w, 560.0), egui::Layout::top_down(egui::Align::Min), |ui| {
            let Some(cmd) = edit.commands.get_mut(edit.selected) else {
                ui.add_space(40.0);
                ui.label(RichText::new(t!("No slash commands yet. Click “+ New command”.")).color(theme.text_muted));
                return;
            };
            let texts = &mut edit.choice_text[edit.selected];
            ScrollArea::vertical().id_salt("slash-form").max_height(520.0).auto_shrink([false, true]).show(ui, |ui| {
                ui.set_width(ui.available_width());
                dialog::section(ui, theme, t!("Command"));
                ui.columns(2, |cols| {
                    dialog::field(&mut cols[0], theme, t!("Name"), Some(t!("What people type after /. Two words make a subcommand: ticket open")), |ui| dialog::text_input(ui, &mut cmd.name, "rank", false));
                    dialog::field(&mut cols[1], theme, t!("Description"), Some(t!("Shown in Discord's command list")), |ui| {
                        dialog::text_input(ui, &mut cmd.description, t!("Shows your rank"), false)
                    });
                });
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(t!("Who can use it")).color(theme.text));
                    let current = PERMISSIONS.iter().find(|(k, _)| *k == cmd.permission).map(|(_, l)| crate::i18n::t(l)).unwrap_or(t!("Everyone"));
                    ComboBox::from_id_salt(ui.id().with("permission")).width(180.0).selected_text(current).show_ui(ui, |ui| {
                        for (k, l) in PERMISSIONS {
                            ui.selectable_value(&mut cmd.permission, k.to_string(), crate::i18n::t(l));
                        }
                    })
                    .response
                    .on_hover_text(t!("People without this permission don't see the command. Server admins can change it in Server Settings › Integrations."));
                    ui.add_space(16.0);
                    ui.label(RichText::new(t!("Works in DMs")).color(theme.text));
                    crate::widgets::toggle(ui, &mut cmd.dm, theme).on_hover_text(t!("Also usable in direct messages with the bot."));
                });
                ui.label(RichText::new(t!("For subcommands, the first one's settings count for the whole command.")).size(11.5).color(theme.text_muted));
                dialog::section(ui, theme, &tf!("Options · {count} of 25", count = cmd.options.len()));
                let mut remove = None;
                for (i, o) in cmd.options.iter_mut().enumerate() {
                    if texts.len() <= i {
                        texts.push(String::new());
                    }
                    option_card(ui, theme, o, &mut texts[i], || remove = Some(i));
                }
                if let Some(i) = remove {
                    cmd.options.remove(i);
                    texts.remove(i);
                }
                if cmd.options.len() < 25 && dialog::secondary_button(ui, theme, t!("+ Add option")).clicked() {
                    cmd.options.push(SlashOption { name: format!("option{}", cmd.options.len() + 1), description: t!("Describe this option").into(), ..Default::default() });
                    texts.push(String::new());
                }
            });
        });
        if let Some(cmd) = edit.commands.get(edit.selected) {
            ui.add_space(20.0);
            discord_preview(ui, theme, cmd, &edit.choice_text[edit.selected], eye);
        }
    });
}

fn option_card(ui: &mut Ui, theme: &Theme, o: &mut SlashOption, choices: &mut String, mut on_remove: impl FnMut()) {
    Frame::new().fill(theme.panel_header).stroke(Stroke::new(1.0, theme.line)).corner_radius(9).inner_margin(Margin::same(12)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.allocate_ui(vec2(170.0, 32.0), |ui| dialog::text_input(ui, &mut o.name, "name", false));
            let current = KINDS.iter().find(|(k, _)| *k == o.kind).map(|(_, l)| crate::i18n::t(l)).unwrap_or(t!("Text"));
            ComboBox::from_id_salt(ui.id().with("kind")).width(140.0).selected_text(current).show_ui(ui, |ui| {
                for (k, l) in KINDS {
                    ui.selectable_value(&mut o.kind, k.to_string(), crate::i18n::t(l));
                }
            });
            ui.label(RichText::new(t!("Required")).size(12.0).color(theme.text_muted));
            crate::widgets::toggle(ui, &mut o.required, theme);
            if matches!(o.kind.as_str(), "string" | "integer" | "number") && choices.trim().is_empty() {
                ui.label(RichText::new(t!("Autocomplete")).size(12.0).color(theme.text_muted));
                crate::widgets::toggle(ui, &mut o.autocomplete, theme).on_hover_text(t!("Suggest values while people type, from an Autocomplete block."));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::Button::new(RichText::new(t!("Remove")).color(theme.error)).small()).clicked() {
                    on_remove();
                }
            });
        });
        ui.add_space(6.0);
        dialog::text_input(ui, &mut o.description, t!("What this option is for"), false);
        if matches!(o.kind.as_str(), "string" | "integer" | "number") {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.allocate_ui(vec2(ui.available_width() - 210.0, 32.0), |ui| dialog::text_input(ui, choices, t!("Choices (optional): Low, Medium, High"), false));
                let (lo, hi) = if o.kind == "string" { ("", t!("Max length")) } else { (t!("Min"), t!("Max")) };
                if !lo.is_empty() {
                    number_box(ui, lo, &mut o.min);
                }
                number_box(ui, hi, &mut o.max);
            });
        }
    });
    ui.add_space(8.0);
}

fn number_box(ui: &mut Ui, label: &str, v: &mut Option<f64>) {
    let mut text = v.map(|n| n.to_string()).unwrap_or_default();
    if ui.add(crate::inputs::TextLine::new(&mut text).hint(label).width(82.0)).changed() {
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
fn discord_preview(ui: &mut Ui, theme: &Theme, cmd: &SlashCommand, choice_text: &[String], eye: Option<&std::path::Path>) {
    let choices_of = |i: usize| -> Vec<String> { choice_text.get(i).map(|t| t.split(',').map(str::trim).filter(|s| !s.is_empty()).map(str::to_string).collect()).unwrap_or_default() };
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
                        let kind = KINDS.iter().find(|(k, _)| *k == o.kind).map(|(_, l)| crate::i18n::t(l)).unwrap_or(t!("Text"));
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
