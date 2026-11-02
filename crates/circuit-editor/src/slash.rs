//! The Slash Command Builder: edits `circuit.json` › `data.slashCommands`.

use eframe::egui::{self, Color32, ComboBox, Frame, Margin, RichText, ScrollArea, Stroke, Ui, vec2};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::dialog::{self, Part};
use crate::editor::{Editor, Level};
use crate::theme::{Theme, with_alpha};

pub const DATA_KEY: &str = "slashCommands";

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct SlashCommand {
    pub name: String,
    pub description: String,
    pub options: Vec<SlashOption>,
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
}

impl Default for SlashOption {
    fn default() -> Self {
        SlashOption { name: String::new(), kind: "string".into(), description: String::new(), required: false, choices: Vec::new(), min: None, max: None }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Choice {
    pub name: String,
    pub value: Value,
}

const KINDS: [(&str, &str); 9] = [
    ("string", "Text"),
    ("integer", "Whole number"),
    ("number", "Number"),
    ("boolean", "Yes / no"),
    ("user", "User"),
    ("channel", "Channel"),
    ("role", "Role"),
    ("mentionable", "User or role"),
    ("attachment", "File"),
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

/// Discord's rules: 1–32 lowercase letters, digits, `-` or `_`.
fn name_problem(name: &str) -> Option<&'static str> {
    if name.is_empty() {
        Some("needs a name")
    } else if name.len() > 32 {
        Some("name is longer than 32 characters")
    } else if !name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_') {
        Some("use lowercase letters, numbers, - and _ only")
    } else {
        None
    }
}

fn problems(cmds: &[SlashCommand]) -> Vec<String> {
    let mut out = Vec::new();
    for (i, c) in cmds.iter().enumerate() {
        let label = if c.name.is_empty() { format!("Command {}", i + 1) } else { format!("/{}", c.name) };
        if let Some(p) = name_problem(&c.name) {
            out.push(format!("{label}: {p}"));
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

pub fn window(ctx: &egui::Context, ed: &mut Editor) {
    let Some(edit) = &mut ed.slash_edit else { return };
    let theme = ed.theme.clone();
    let icon = ed.reg.icons.get("slash").cloned();
    let issues = problems(&edit.commands);
    let (mut save, mut cancel) = (false, false);
    let close = dialog::window(
        ctx,
        &theme,
        dialog::Spec {
            id: "slash-builder",
            title: "Slash Commands",
            subtitle: Some("Registered automatically when the bot starts"),
            icon: icon.as_deref(),
            width: 980.0,
        },
        |ui, part| match part {
            Part::Body => body(ui, &theme, edit, &issues),
            Part::Footer => {
                save = ui.add_enabled_ui(issues.is_empty(), |ui| dialog::primary_button(ui, &theme, "Save commands")).inner.clicked();
                cancel = dialog::secondary_button(ui, &theme, "Cancel").clicked();
                if !issues.is_empty() {
                    ui.label(RichText::new(format!("{} to fix", issues.len())).size(12.0).color(theme.warning));
                }
            }
        },
    );
    if save {
        let edit = ed.slash_edit.take().unwrap();
        let mut commands = edit.commands;
        // Turn the typed choice lists into choices.
        for (c, texts) in commands.iter_mut().zip(edit.choice_text) {
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
        ed.log(Level::Success, format!("Saved {} slash commands", commands.len()));
    } else if cancel || close {
        ed.slash_edit = None;
    }
}

fn body(ui: &mut Ui, theme: &Theme, edit: &mut SlashEdit, issues: &[String]) {
    ui.horizontal_top(|ui| {
        // Command list.
        ui.allocate_ui_with_layout(vec2(230.0, 520.0), egui::Layout::top_down(egui::Align::Min), |ui| {
            dialog::section(ui, theme, "Commands");
            let mut remove = None;
            for (i, c) in edit.commands.iter().enumerate() {
                let selected = i == edit.selected;
                let label = if c.name.is_empty() { "/(no name)".to_string() } else { format!("/{}", c.name) };
                let text = RichText::new(label).monospace().color(if selected { theme.text } else { theme.text_muted });
                let resp = ui.add_sized([220.0, 30.0], egui::Button::selectable(selected, text));
                if resp.clicked() {
                    edit.selected = i;
                }
                resp.context_menu(|ui| {
                    if ui.button("Delete command").clicked() {
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
            if dialog::secondary_button(ui, theme, "+ New command").clicked() {
                edit.commands.push(SlashCommand { name: format!("command{}", edit.commands.len() + 1), description: "Describe what it does".into(), options: Vec::new() });
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

        // The selected command.
        ui.vertical(|ui| {
            let Some(cmd) = edit.commands.get_mut(edit.selected) else {
                ui.add_space(40.0);
                ui.label(RichText::new("No slash commands yet. Click “+ New command”.").color(theme.text_muted));
                return;
            };
            let texts = &mut edit.choice_text[edit.selected];
            ScrollArea::vertical().id_salt("slash-form").max_height(520.0).auto_shrink([false, true]).show(ui, |ui| {
                ui.set_width(ui.available_width());
                dialog::section(ui, theme, "Command");
                ui.columns(2, |cols| {
                    dialog::field(&mut cols[0], theme, "Name", Some("What people type after /"), |ui| dialog::text_input(ui, &mut cmd.name, "rank", false));
                    dialog::field(&mut cols[1], theme, "Description", Some("Shown in Discord's command list"), |ui| {
                        dialog::text_input(ui, &mut cmd.description, "Shows your rank", false)
                    });
                });
                dialog::section(ui, theme, &format!("Options · {} of 25", cmd.options.len()));
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
                if cmd.options.len() < 25 && dialog::secondary_button(ui, theme, "+ Add option").clicked() {
                    cmd.options.push(SlashOption { name: format!("option{}", cmd.options.len() + 1), description: "Describe this option".into(), ..Default::default() });
                    texts.push(String::new());
                }
                ui.add_space(12.0);
                dialog::section(ui, theme, "How it looks in Discord");
                preview(ui, theme, cmd);
            });
        });
    });
}

fn option_card(ui: &mut Ui, theme: &Theme, o: &mut SlashOption, choices: &mut String, mut on_remove: impl FnMut()) {
    Frame::new().fill(theme.panel_header).stroke(Stroke::new(1.0, theme.line)).corner_radius(9).inner_margin(Margin::same(12)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.allocate_ui(vec2(170.0, 32.0), |ui| dialog::text_input(ui, &mut o.name, "name", false));
            let current = KINDS.iter().find(|(k, _)| *k == o.kind).map(|(_, l)| *l).unwrap_or("Text");
            ComboBox::from_id_salt(ui.id().with("kind")).width(140.0).selected_text(current).show_ui(ui, |ui| {
                for (k, l) in KINDS {
                    ui.selectable_value(&mut o.kind, k.to_string(), l);
                }
            });
            ui.label(RichText::new("Required").size(12.0).color(theme.text_muted));
            crate::widgets::toggle(ui, &mut o.required, theme);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::Button::new(RichText::new("Remove").color(theme.error)).small()).clicked() {
                    on_remove();
                }
            });
        });
        ui.add_space(6.0);
        dialog::text_input(ui, &mut o.description, "What this option is for", false);
        if matches!(o.kind.as_str(), "string" | "integer" | "number") {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.allocate_ui(vec2(ui.available_width() - 210.0, 32.0), |ui| dialog::text_input(ui, choices, "Choices (optional): Low, Medium, High", false));
                let (lo, hi) = if o.kind == "string" { ("", "Max length") } else { ("Min", "Max") };
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

fn preview(ui: &mut Ui, theme: &Theme, cmd: &SlashCommand) {
    Frame::new().fill(Color32::from_rgb(0x38, 0x3A, 0x40)).corner_radius(8).inner_margin(Margin::symmetric(12, 10)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(format!("/{}", cmd.name)).monospace().color(Color32::WHITE));
            for o in &cmd.options {
                let chip = Frame::new().fill(if o.required { Color32::from_rgb(0x4E, 0x50, 0x58) } else { Color32::from_rgb(0x2B, 0x2D, 0x31) }).corner_radius(4).inner_margin(Margin::symmetric(6, 2));
                chip.show(ui, |ui| {
                    ui.label(RichText::new(format!("{}{}", o.name, if o.required { "" } else { "?" })).size(12.5).color(Color32::from_rgb(0xDB, 0xDE, 0xE1)));
                });
            }
        });
        ui.label(RichText::new(&cmd.description).size(12.0).color(with_alpha(Color32::WHITE, 150)));
    });
    let _ = theme;
}
