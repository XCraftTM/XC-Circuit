//! The side panels: Workspaces, Inspector, Console and Problems.

use circuit_core::graph::ACTION_IN;
use circuit_core::PinMode;
use eframe::egui::{self, Color32, Frame, Margin, RichText, ScrollArea, Ui};
use serde_json::Value;

use crate::editor::{Editor, Level, node_shape};
use crate::theme::with_alpha;
use crate::widgets;

fn padded(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    Frame::new().inner_margin(Margin::symmetric(12, 10)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        add(ui)
    });
}

/// Something the Inspector asks for that needs the whole editor.
enum InspectorAsk {
    EmbedBuilder(String, String, String),
    LayoutBuilder(String, String, String),
    SlashCommands,
}

pub fn inspector(ui: &mut Ui, ed: &mut Editor) {
    let mut ask = None;
    // Text boxes offer the values of earlier blocks after `{`.
    let values = ed.selected.clone().map(|(ws, node)| std::sync::Arc::new(ed.values_for(&ws, &node)));
    crate::values::set_scope(ui.ctx(), values);
    inspector_body(ui, ed, &mut ask);
    crate::values::set_scope(ui.ctx(), None);
    match ask {
        Some(InspectorAsk::EmbedBuilder(ws, node, key)) => crate::embed::open(ed, &ws, &node, &key),
        Some(InspectorAsk::LayoutBuilder(ws, node, key)) => crate::layout::open(ed, &ws, &node, &key),
        Some(InspectorAsk::SlashCommands) => crate::slash::open(ed),
        None => {}
    }
}

fn inspector_body(ui: &mut Ui, ed: &mut Editor, ask: &mut Option<InspectorAsk>) {
    let theme = ed.theme.clone();
    let Some((ws_key, node_id)) = ed.selected.clone() else {
        padded(ui, |ui| {
            ui.label(RichText::new(t!("Select a block to see its settings.")).color(theme.text_muted));
        });
        return;
    };
    let Editor { reg, engine, project, shapes, edited, define_ctx, .. } = ed;
    let mode = project.as_ref().and_then(|p| reg.mode(&p.meta.mode));
    let Some(ws) = project.as_mut().and_then(|p| p.workspaces.get_mut(&ws_key)) else { return };
    shapes.begin();
    let Some(shape) = node_shape(reg, engine, shapes, &ws_key, ws, &node_id, define_ctx) else {
        padded(ui, |ui| {
            ui.label(RichText::new(t!("This block's plugin is not loaded.")).color(theme.error));
        });
        return;
    };
    let Some(def) = ws.nodes.get(&node_id).and_then(|n| reg.block(&n.block)).cloned() else { return };

    ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        padded(ui, |ui| {
            ui.horizontal(|ui| {
                if let Some(path) = reg.block_icon(&def, mode) {
                    ui.add(egui::Image::from_uri(crate::editor::file_uri(&path)).tint(theme.text).fit_to_exact_size(egui::vec2(20.0, 20.0)));
                }
                ui.vertical(|ui| {
                    ui.label(RichText::new(&def.name).strong().size(15.0));
                    ui.label(RichText::new(tf!("by {author} · v{version} · #{id}", author = def.author, version = def.version, id = node_id)).size(11.0).color(theme.text_muted));
                });
            });
            ui.add_space(4.0);
            ui.label(RichText::new(&def.description).color(theme.text_muted));
        });
        ui.separator();

        let mut toggle: Option<String> = None;
        let mut fields = |ui: &mut Ui, advanced: bool| {
            for (key, f) in shape.visible_fields().filter(|(_, f)| f.advanced == advanced) {
                let node = &ws.nodes[&node_id];
                let pinned = f.pin == PinMode::Only || node.pins.contains(key);
                let source = ws.links_into(&node_id, key).next().map(|l| l.from.clone());
                padded(ui, |ui| {
                    ui.horizontal(|ui| {
                        let label = ui.label(RichText::new(f.label(key)).color(theme.text_muted));
                        if let Some(d) = &f.description {
                            label.on_hover_text(d);
                        }
                        if f.required {
                            ui.label(RichText::new(t!("required")).size(10.0).color(theme.warning));
                        }
                        if matches!(f.pin, PinMode::Auto | PinMode::Pin) && source.is_none() {
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let text = if pinned { t!("● pin") } else { t!("○ pin") };
                                let color = if pinned { theme.pin_color(&f.ty, reg) } else { theme.text_muted };
                                if ui.add(egui::Button::new(RichText::new(text).size(11.0).color(color)).selected(pinned)).on_hover_text(t!("Switch between typing a value and connecting a pin")).clicked() {
                                    toggle = Some(key.clone());
                                }
                            });
                        }
                    });
                    if let Some(src) = &source {
                        let from = ws.nodes.get(&src.node).and_then(|n| reg.block(&n.block)).map(|d| d.name.clone()).unwrap_or_default();
                        Frame::new().fill(theme.field).corner_radius(6).inner_margin(Margin::symmetric(8, 5)).show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.label(tf!("Connected · {block} ({pin})", block = from, pin = src.pin));
                        });
                    } else if pinned && f.label.as_deref().is_some_and(|l| l.starts_with("${")) {
                        // A text's {name} input: connecting it is up to you.
                        ui.label(RichText::new(t!("Optional: connect what goes there. Unconnected, the name is looked up in the event and earlier blocks.")).size(11.5).color(theme.text_muted));
                    } else if pinned {
                        ui.label(RichText::new(tf!("Waiting for a {type} wire", type = f.ty.display(reg))).italics().color(theme.text_muted));
                    } else if f.type_name == "embed" {
                        if crate::dialog::button(ui, &theme, t!("Open Embed Builder")).clicked() {
                            *ask = Some(InspectorAsk::EmbedBuilder(ws_key.clone(), node_id.clone(), key.clone()));
                        }
                    } else if f.type_name == "layout" {
                        if crate::dialog::button(ui, &theme, t!("Open Layout Builder")).clicked() {
                            *ask = Some(InspectorAsk::LayoutBuilder(ws_key.clone(), node_id.clone(), key.clone()));
                        }
                    } else {
                        let mut value = ws.nodes[&node_id].values.get(key).cloned().or_else(|| f.default.clone()).unwrap_or(Value::Null);
                        let id = ui.id().with(("inspector", &node_id, key));
                        if widgets::value_editor(ui, id, f, &mut value, ui.available_width(), true, &theme) {
                            ws.nodes.get_mut(&node_id).unwrap().values.insert(key.clone(), value);
                            *edited = true;
                        }
                        if f.templated() {
                            ui.label(RichText::new(t!("${…} holds JavaScript: type ${ for values from earlier blocks, or ${xp * 2}")).size(11.0).color(theme.text_muted));
                        }
                        if f.type_name == "slash-command-ref" && crate::dialog::button(ui, &theme, t!("Edit slash commands…")).clicked() {
                            *ask = Some(InspectorAsk::SlashCommands);
                        }
                    }
                });
            }
        };
        fields(ui, false);
        if shape.visible_fields().any(|(_, f)| f.advanced) {
            egui::CollapsingHeader::new(t!("Advanced")).default_open(false).show(ui, |ui| fields(ui, true));
        }
        if let Some(key) = toggle {
            let n = ws.nodes.get_mut(&node_id).unwrap();
            if let Some(i) = n.pins.iter().position(|p| *p == key) {
                n.pins.remove(i);
            } else {
                n.pins.push(key);
            }
            *edited = true;
        }

        if !shape.outputs.is_empty() {
            ui.separator();
            padded(ui, |ui| {
                ui.label(RichText::new(t!("OUTPUTS")).size(11.0).strong().color(theme.text_muted));
                for (k, o) in &shape.outputs {
                    ui.horizontal(|ui| {
                        let color = theme.pin_color(&o.ty, reg);
                        ui.label(RichText::new("●").color(color));
                        let l = ui.label(o.label(k));
                        if let Some(d) = &o.description {
                            l.on_hover_text(d);
                        }
                        ui.label(RichText::new(o.ty.display(reg)).size(11.0).color(theme.text_muted));
                    });
                }
            });
        }

        ui.separator();
        padded(ui, |ui| {
            ui.horizontal(|ui| {
                if crate::dialog::button(ui, &theme, t!("Block source")).clicked() {
                    let _ = open::that(&def.source);
                }
                if let Some(docs) = &def.docs
                    && crate::dialog::button(ui, &theme, t!("Docs")).clicked()
                {
                    let _ = open::that(docs);
                }
            });
        });
    });
}

/// The console's level filter, kept between sessions: None shows every line.
const CONSOLE_LEVEL_KEY: &str = "console-level";

/// A level's tag in the console and its color.
fn level_look(level: Level, theme: &crate::theme::Theme) -> (&'static str, Color32) {
    match level {
        Level::Info => ("INFO ", Color32::from_rgb(0x6F, 0xA8, 0xFF)),
        Level::Success => ("DONE ", Color32::from_rgb(0x3D, 0xDC, 0x97)),
        Level::Warn => ("WARN ", theme.warning),
        Level::Error => ("ERROR", theme.error),
    }
}

pub fn console(ui: &mut Ui, ed: &mut Editor) {
    let theme = ed.theme.clone();
    let key = egui::Id::new(CONSOLE_LEVEL_KEY);
    let level: Option<Level> = ui.ctx().data_mut(|d| d.get_persisted::<u8>(key)).and_then(|n| match n {
        1 => Some(Level::Info),
        2 => Some(Level::Success),
        3 => Some(Level::Warn),
        4 => Some(Level::Error),
        _ => None,
    });
    // A toolbar row like the Stored Data panel's: Clear, then chips that show
    // one level of lines, each with how many there are.
    Frame::new().inner_margin(Margin::symmetric(12, 8)).show(ui, |ui| {
        ui.horizontal(|ui| {
            if crate::dialog::button(ui, &theme, t!("Clear")).clicked() {
                ed.log.clear();
            }
            ui.add_space(6.0);
            ui.spacing_mut().item_spacing.x = 6.0;
            let count = |l: Level| ed.log.iter().filter(|x| x.level == l).count();
            let options = [
                (None, t!("All").to_string(), theme.text),
                (Some(Level::Info), format!("{} {}", t!("Info"), count(Level::Info)), level_look(Level::Info, &theme).1),
                (Some(Level::Success), format!("{} {}", t!("Done"), count(Level::Success)), level_look(Level::Success, &theme).1),
                (Some(Level::Warn), format!("{} {}", t!("Warnings"), count(Level::Warn)), theme.warning),
                (Some(Level::Error), format!("{} {}", t!("Errors"), count(Level::Error)), theme.error),
            ];
            for (l, label, color) in options {
                if crate::dialog::filter_chip(ui, &theme, &label, color, l == level).clicked() {
                    let n: u8 = match l {
                        None => 0,
                        Some(Level::Info) => 1,
                        Some(Level::Success) => 2,
                        Some(Level::Warn) => 3,
                        Some(Level::Error) => 4,
                    };
                    ui.ctx().data_mut(|d| d.insert_persisted(key, n));
                }
            }
        });
    });
    let mut jump: Option<(String, String)> = None;
    // One screen row per line of text (stack traces span several), all as tall
    // as the Show block button, so only the rows in view are laid out.
    let rows: Vec<(usize, usize, &str)> = ed
        .log
        .iter()
        .enumerate()
        .filter(|(_, l)| level.is_none_or(|want| l.level == want))
        .flat_map(|(i, l)| l.text.split('\n').enumerate().map(move |(n, part)| (i, n, part.trim_end_matches('\r'))))
        .collect();
    let row_h = crate::inputs::input_height(ui);
    ui.spacing_mut().item_spacing.y = 0.0;
    ScrollArea::vertical().auto_shrink([false, false]).stick_to_bottom(true).show_rows(ui, row_h, rows.len(), |ui, range| {
        Frame::new().inner_margin(Margin::symmetric(12, 0)).show(ui, |ui| {
            for &(i, n, part) in &rows[range] {
                let line = &ed.log[i];
                let (tag, color) = level_look(line.level, &theme);
                ui.horizontal(|ui| {
                    ui.set_height(row_h);
                    // The time and level only on a line's first row.
                    let (time, tag) = if n == 0 { (line.time.as_str(), tag) } else { ("        ", "     ") };
                    ui.label(RichText::new(time).monospace().color(theme.text_muted));
                    ui.label(RichText::new(tag).monospace().color(color));
                    ui.label(RichText::new(part).monospace());
                    if n == 0
                        && let Some(t) = &line.target
                        && crate::dialog::button(ui, &theme, t!("Show block")).on_hover_text(t!("Show the block that wrote this")).clicked()
                    {
                        jump = Some(t.clone());
                    }
                });
            }
        });
    });
    if let Some((ws, node)) = jump {
        ed.reveal(&ws, &node);
    }
}

/// One entry in the Problems panel: where, what, and the block to jump to.
pub type ProblemItem = (String, String, Option<(String, String)>);

/// Plugin problems plus checks of every workspace in the open project.
pub fn collect_problems(ed: &Editor) -> Vec<ProblemItem> {
    let mut items: Vec<ProblemItem> = ed.reg.problems.iter().map(|p| (p.source.clone(), p.message.clone(), None)).collect();
    if let Some(project) = &ed.project {
        for (stem, why) in &project.unreadable {
            items.push((format!("workspaces/{stem}.json"), format!("couldn't be read, so it isn't open (the file is kept as it is): {why}"), None));
        }
        for (key, ws) in &project.workspaces {
            for (id, node) in &ws.nodes {
                let Some(def) = ed.reg.block(&node.block) else {
                    items.push((format!("{} › #{id}", ws.name), format!("block `{}` is not installed", node.block), Some((key.clone(), id.clone()))));
                    continue;
                };
                if def.kind == circuit_core::BlockKind::Action && ws.links_into(id, ACTION_IN).next().is_none() {
                    items.push((format!("{} › {}", ws.name, def.name), t!("nothing runs this block (no Action wire into it)").into(), Some((key.clone(), id.clone()))));
                }
                for (k, f) in &def.fields {
                    let connected = ws.links_into(id, k).next().is_some();
                    let empty = node.values.get(k).or(f.default.as_ref()).is_none_or(|v| v.is_null() || *v == Value::String(String::new()));
                    if f.required && !connected && empty && f.show.is_none() {
                        items.push((format!("{} › {}", ws.name, def.name), format!("`{}` needs a value", f.label(k)), Some((key.clone(), id.clone()))));
                    }
                }
            }
            for (from, count) in ws.split_paths() {
                let name = ws.nodes.get(&from.node).and_then(|n| ed.reg.block(&n.block)).map(|d| d.name.clone()).or_else(|| ws.zones.get(&from.node).map(|z| format!("Try {}", z.title))).unwrap_or_default();
                items.push((
                    format!("{} › {name}", ws.name),
                    tf!("its {pin} output leads to {count} blocks: an action path can't split. Chain them one after another, or branch with If or Switch", pin = from.pin, count = count),
                    Some((key.clone(), from.node.clone())),
                ));
            }
            for link in &ws.links {
                let from_ok = ws.nodes.contains_key(&link.from.node) || ws.zones.contains_key(&link.from.node);
                if !from_ok || !ws.nodes.contains_key(&link.to.node) {
                    items.push((ws.name.clone(), format!("wire `{link}` points to a missing block"), None));
                    continue;
                }
                // Action wires leave a Try Zone only through its Done / Error outputs.
                if link.to.pin == ACTION_IN
                    && let Some(zone) = ws.nodes.get(&link.from.node).and_then(|n| n.zone.as_ref())
                    && ws.nodes[&link.to.node].zone.as_ref() != Some(zone)
                {
                    let name = ws.zones.get(zone).map(|z| z.title.clone()).unwrap_or_default();
                    items.push((
                        format!("{} › Try {name}", ws.name),
                        t!("an Action wire leaves the Try Zone; connect the zone's Done output instead").into(),
                        Some((key.clone(), link.from.node.clone())),
                    ));
                }
            }
        }
    }
    items
}

pub fn problems(ui: &mut Ui, ed: &mut Editor, items: Vec<ProblemItem>) {
    let theme = ed.theme.clone();
    ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        Frame::new().inner_margin(Margin::symmetric(12, 8)).show(ui, |ui| {
            if items.is_empty() {
                ui.label(RichText::new(t!("No problems")).color(theme.text_muted));
            }
            for (source, message, target) in items {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("⚠").color(theme.warning));
                    let resp = ui.add(egui::Label::new(RichText::new(&message)).sense(egui::Sense::click()));
                    ui.label(RichText::new(&source).size(11.0).color(with_alpha(theme.text_muted, 200)));
                    if let Some(t) = target
                        && resp.clicked()
                    {
                        ed.reveal(&t.0, &t.1);
                    }
                });
            }
        });
    });
}
