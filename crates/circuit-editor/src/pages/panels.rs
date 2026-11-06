//! The side panels: Workspaces, Inspector, Console and Problems.

use circuit_core::graph::ACTION_IN;
use circuit_core::PinMode;
use eframe::egui::{self, Color32, Frame, Margin, RichText, ScrollArea, Ui};
use serde_json::Value;

use crate::ui::kit;
use crate::editor::{Editor, Level, node_shape};
use crate::ui::theme::with_alpha;
use crate::ui::widgets;

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
    CanvasBuilder(String, String, String),
    SlashCommands,
}

pub fn inspector(ui: &mut Ui, ed: &mut Editor) {
    let mut ask = None;
    // Text boxes offer the values of earlier blocks after `{`.
    let values = ed.selected.clone().map(|(ws, node)| std::sync::Arc::new(ed.values_for(&ws, &node)));
    crate::editor::values::set_scope(ui.ctx(), values);
    inspector_body(ui, ed, &mut ask);
    crate::editor::values::set_scope(ui.ctx(), None);
    match ask {
        Some(InspectorAsk::EmbedBuilder(ws, node, key)) => crate::builders::embed::open(ed, &ws, &node, &key),
        Some(InspectorAsk::LayoutBuilder(ws, node, key)) => crate::builders::layout::open(ed, &ws, &node, &key),
        Some(InspectorAsk::CanvasBuilder(ws, node, key)) => crate::builders::canvas::open(ed, &ws, &node, &key),
        Some(InspectorAsk::SlashCommands) => crate::builders::slash::open(ed),
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
    let Editor { reg, engine, project, shapes, edited, define_ctx, ui_rects, tour_parts, .. } = ed;
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
                let top = ui.cursor().top();
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
                                // The chip shows whether it's on; the fonts have no filled circle.
                                let text = t!("pin");
                                let color = if pinned { theme.pin_color(&f.ty, reg) } else { theme.text_muted };
                                if kit::filter_chip(ui, &theme, text, color, pinned).on_hover_text(t!("Switch between typing a value and connecting a pin")).clicked() {
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
                        kit::hint(ui, &theme, t!("Optional: connect what goes there. Unconnected, the name is looked up in the event and earlier blocks."));
                    } else if pinned {
                        ui.label(RichText::new(tf!("Waiting for a {type} wire", type = f.ty.display(reg))).italics().color(theme.text_muted));
                    } else if f.type_name == "embed" {
                        if crate::ui::kit::button(ui, &theme, t!("Open Embed Builder")).clicked() {
                            *ask = Some(InspectorAsk::EmbedBuilder(ws_key.clone(), node_id.clone(), key.clone()));
                        }
                    } else if f.type_name == "canvas" {
                        if crate::ui::kit::button(ui, &theme, t!("Open Canvas Builder")).clicked() {
                            *ask = Some(InspectorAsk::CanvasBuilder(ws_key.clone(), node_id.clone(), key.clone()));
                        }
                    } else if f.type_name == "layout" {
                        if crate::ui::kit::button(ui, &theme, t!("Open Layout Builder")).clicked() {
                            *ask = Some(InspectorAsk::LayoutBuilder(ws_key.clone(), node_id.clone(), key.clone()));
                        }
                    } else {
                        let mut value = ws.nodes[&node_id].values.get(key).cloned().or_else(|| f.default.clone()).unwrap_or(Value::Null);
                        let id = ui.scope_id().with(("inspector", &node_id, key));
                        if kit::called(ui, &f.label(key), |ui| widgets::value_editor(ui, id, f, &mut value, ui.available_width(), true, &theme)) {
                            ws.nodes.get_mut(&node_id).unwrap().values.insert(key.clone(), value);
                            *edited = true;
                        }
                        if f.templated() {
                            ui.label(RichText::new(t!("${…} holds JavaScript: type ${ for values from earlier blocks, or ${xp * 2}")).size(11.0).color(theme.text_muted));
                        }
                        if f.type_name == "slash-command-ref" && crate::ui::kit::button(ui, &theme, t!("Edit slash commands…")).clicked() {
                            *ask = Some(InspectorAsk::SlashCommands);
                        }
                    }
                });
                if *tour_parts {
                    let x = ui.max_rect().x_range();
                    ui_rects.insert(format!("field:{key}"), egui::Rect::from_x_y_ranges(x.min + 6.0..=x.max - 8.0, top + 2.0..=ui.cursor().top() - 2.0));
                }
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
                        let (dot, _) = ui.allocate_exact_size(egui::vec2(10.0, 14.0), egui::Sense::hover());
                        ui.painter().circle_filled(dot.center(), 4.0, color);
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
                if crate::ui::kit::button(ui, &theme, t!("Block source")).clicked() {
                    let _ = open::that(&def.source);
                }
                if let Some(docs) = &def.docs
                    && crate::ui::kit::button(ui, &theme, t!("Docs")).clicked()
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
fn level_look(level: Level, theme: &crate::ui::theme::Theme) -> (&'static str, Color32) {
    match level {
        Level::Info => ("INFO ", Color32::from_rgb(0x6F, 0xA8, 0xFF)),
        Level::Success => ("DONE ", Color32::from_rgb(0x3D, 0xDC, 0x97)),
        Level::Warn => ("WARN ", theme.warning),
        Level::Error => ("ERROR", theme.error),
    }
}

pub fn console(ui: &mut Ui, ed: &mut Editor) {
    let theme = ed.theme.clone();
    let key = egui::Id::unique(CONSOLE_LEVEL_KEY);
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
        // Wrapping: in the narrow side bar the chips go on to a second line.
        ui.horizontal_wrapped(|ui| {
            if crate::ui::kit::button(ui, &theme, t!("Clear")).clicked() {
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
                if crate::ui::kit::filter_chip(ui, &theme, &label, color, l == level).clicked() {
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
    // as the Show block button, so only the rows in view are laid out. A line
    // too long for the width is broken into rows here (between words where it
    // can be), under its text: a narrow Console (in the side bar) shows it all.
    // The font and size picked in Settings › Editor › Console.
    let font = crate::ui::fonts::console(ui.ctx(), ed.prefs.console_font_size);
    let char_w = ui.fonts_mut(|f| f.glyph_width(&font, 'm')).max(1.0);
    let gap = ui.spacing().item_spacing.x;
    // Time (8) and level (5) with their gaps; the Show block link on lines that have it.
    let lead = char_w * 13.0 + gap * 3.0;
    let link_font = egui::FontId::proportional((font.size * 0.9).max(10.0));
    let button = ui.fonts_mut(|f| f.layout_no_wrap(t!("Show block").to_string(), link_font.clone(), theme.accent).size().x) + gap * 2.0;
    let width = ui.available_width() - 24.0 - ui.spacing().scroll.bar_width - 4.0;
    // Narrow (the side bar): the time and level get a row of their own, and the
    // text the whole width under it, a little indented.
    let indent = char_w * 2.0;
    let stacked = width - lead - button < char_w * 32.0;
    let columns = |has_button: bool| {
        let room = if stacked { width - indent } else { width - lead - if has_button { button } else { 0.0 } };
        ((room / char_w).floor() as usize).max(8)
    };
    let rows: Vec<(usize, usize, &str)> = ed
        .log
        .iter()
        .enumerate()
        .filter(|(_, l)| level.is_none_or(|want| l.level == want))
        .flat_map(|(i, l)| {
            let cols = columns(l.target.is_some());
            // Stacked, row 0 is the time and level alone; the text starts at row 1.
            let head = stacked.then_some("");
            head.into_iter().chain(l.text.split('\n').flat_map(move |part| wrap_columns(part.trim_end_matches('\r'), cols))).enumerate().map(move |(n, part)| (i, n, part))
        })
        .collect();
    // As tall as the text, and the room between lines set in Settings › Editor › Console.
    let row_h = ui.fonts_mut(|f| f.row_height(&font)) + ed.prefs.console_line_spacing.max(0.0);
    ui.spacing_mut().item_spacing.y = 0.0;
    ScrollArea::vertical().auto_shrink([false, false]).stick_to_bottom(true).show_rows(ui, row_h, rows.len(), |ui, range| {
        Frame::new().inner_margin(Margin::symmetric(12, 0)).show(ui, |ui| {
            for &(i, n, part) in &rows[range] {
                let line = &ed.log[i];
                let (tag, color) = level_look(line.level, &theme);
                ui.horizontal(|ui| {
                    ui.set_height(row_h);
                    if stacked && n > 0 {
                        ui.add_space(indent);
                        ui.label(RichText::new(part).font(font.clone()));
                    } else {
                        // The time and level only on a line's first row.
                        let (time, tag) = if n == 0 { (line.time.as_str(), tag) } else { ("        ", "     ") };
                        ui.label(RichText::new(time).font(font.clone()).color(theme.text_muted));
                        ui.label(RichText::new(tag).font(font.clone()).color(color));
                        if !stacked {
                            ui.label(RichText::new(part).font(font.clone()));
                        }
                    }
                    if n == 0
                        && let Some(t) = &line.target
                        && crate::ui::kit::link(ui, &theme, t!("Show block"), link_font.clone()).on_hover_text(t!("Show the block that wrote this")).clicked()
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

/// `text` in pieces of at most `cols` characters, broken after a space where
/// there is one (a long word is cut).
fn wrap_columns(text: &str, cols: usize) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = text;
    while rest.chars().count() > cols {
        let cut = rest.char_indices().nth(cols).map_or(rest.len(), |(i, _)| i);
        let at = rest[..cut].rfind(' ').filter(|&i| i > 0).map_or(cut, |i| i + 1);
        out.push(&rest[..at]);
        rest = &rest[at..];
    }
    out.push(rest);
    out
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
        // Call blocks in workspaces that are on, whose When Called block is
        // only in workspaces that are off: the call would find nothing.
        let called_in = |on: bool| -> Vec<(String, String)> {
            let mut out = Vec::new();
            for ws in project.workspaces.values().filter(|ws| ws.disabled != on) {
                for node in ws.nodes.values().filter(|n| n.block_id() == "core.on_called") {
                    if let Some(name) = node.values.get("name").and_then(Value::as_str) {
                        out.push((name.to_string(), ws.name.clone()));
                    }
                }
            }
            out
        };
        let (live, off) = (called_in(true), called_in(false));
        for (key, ws) in project.workspaces.iter().filter(|(_, ws)| !ws.disabled) {
            for (id, node) in ws.nodes.iter().filter(|(_, n)| n.block_id() == "core.call_workspace") {
                let Some(name) = node.values.get("name").and_then(Value::as_str) else { continue };
                if live.iter().any(|(n, _)| n == name) {
                    continue;
                }
                if let Some((_, there)) = off.iter().find(|(n, _)| n == name) {
                    items.push((
                        format!("{} › Call", ws.name),
                        tf!("calls {name}, but its When Called block is in {workspace}, which is turned off", name = name, workspace = there),
                        Some((key.clone(), id.clone())),
                    ));
                }
            }
        }
        // Workspaces that are off don't run, so their blocks aren't checked.
        for (key, ws) in project.workspaces.iter().filter(|(_, ws)| !ws.disabled) {
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
            for link in ws.loops(|id| crate::canvas::is_value_block(ws, &ed.reg, id)) {
                let name = ws.nodes.get(&link.from.node).and_then(|n| ed.reg.block(&n.block)).map(|d| d.name.clone()).or_else(|| ws.zones.get(&link.from.node).map(|z| format!("Try {}", z.title))).unwrap_or_default();
                let why = if link.to.pin == ACTION_IN {
                    tf!("its {pin} wire leads back to a block before it, so this path never ends. To repeat blocks, use Repeat, Loop While or For Each", pin = link.from.pin)
                } else {
                    tf!("its {pin} wire goes to a block it reads from, so they would wait for each other forever", pin = link.from.pin)
                };
                items.push((format!("{} › {name}", ws.name), why, Some((key.clone(), link.from.node.clone()))));
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
                // Wrapping: a long message (or a narrow panel) goes on to more lines.
                ui.horizontal_wrapped(|ui| {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn console_lines_wrap_between_words() {
        assert_eq!(wrap_columns("Loaded 5 plugins, 267 blocks", 12), ["Loaded 5 ", "plugins, ", "267 blocks"]);
        assert_eq!(wrap_columns("short", 12), ["short"]);
        assert_eq!(wrap_columns("C:/Users/Admin/AppData/Local", 10), ["C:/Users/A", "dmin/AppDa", "ta/Local"], "a long word is cut");
        assert_eq!(wrap_columns("äöü äöü äöü", 5), ["äöü ", "äöü ", "äöü"], "by characters, not bytes");
    }
    use circuit_core::{Node, Workspace};

    /// A Call whose When Called block is only in a workspace that is off is a
    /// problem; the blocks of a workspace that is off aren't checked.
    #[test]
    fn calls_into_workspaces_that_are_off() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut ed = Editor::new(vec![root.join("plugins")], None).unwrap();
        let dir = std::env::temp_dir().join(format!("xc-problems-off-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        circuit_core::Project::create(&dir, "Off", "none").unwrap();
        ed.open_project(&dir);
        let node = |block: &str, name: &str| Node { block: block.into(), values: serde_json::json!({ "name": name }).as_object().cloned().unwrap(), ..Default::default() };
        let project = ed.project.as_mut().unwrap();
        let mut caller = Workspace::new("Main");
        caller.nodes.insert("go".into(), node("core.on_start@1", ""));
        caller.nodes.insert("call".into(), node("core.call_workspace@1", "greet"));
        caller.links.push("go.next > call.in".parse().unwrap());
        let mut helper = Workspace::new("Helpers");
        helper.nodes.insert("w".into(), node("core.on_called@1", "greet"));
        // A block nothing runs: a problem only while the workspace is on.
        helper.nodes.insert("lonely".into(), node("core.log@1", ""));
        project.workspaces.insert("main".into(), caller);
        project.workspaces.insert("helpers".into(), helper);
        let about = |items: &[ProblemItem], what: &str| items.iter().filter(|(_, why, _)| why.contains(what)).count();
        let items = collect_problems(&ed);
        assert_eq!(about(&items, "turned off"), 0);
        assert_eq!(about(&items, "nothing runs this block"), 1);

        ed.project.as_mut().unwrap().workspaces.get_mut("helpers").unwrap().disabled = true;
        let items = collect_problems(&ed);
        assert_eq!(about(&items, "calls greet, but its When Called block is in Helpers, which is turned off"), 1, "{items:?}");
        assert_eq!(about(&items, "nothing runs this block"), 0, "blocks of a workspace that is off aren't checked");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
