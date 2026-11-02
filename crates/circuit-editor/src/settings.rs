//! The Settings page, opened as a tab: appearance, editor, bot, plugins,
//! shortcuts and about. Changes apply right away and are kept between sessions.

use eframe::egui::{self, Align, Align2, Color32, CursorIcon, FontId, Layout, Rect, RichText, ScrollArea, Sense, Stroke, StrokeKind, Ui, pos2, vec2};

use crate::editor::{Editor, file_uri};
use crate::native::NativeHost;
use crate::prefs::{ACCENTS, UI_SCALES};
use crate::theme::{Theme, mix, with_alpha};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Section {
    #[default]
    Appearance,
    Editor,
    Bot,
    Plugins,
    Shortcuts,
    About,
}

impl Section {
    const ALL: [(Section, &'static str, &'static str); 6] = [
        (Section::Appearance, "Appearance", "palette"),
        (Section::Editor, "Editor", "workspace"),
        (Section::Bot, "Bot", "bolt"),
        (Section::Plugins, "Plugins", "plug"),
        (Section::Shortcuts, "Shortcuts", "keyboard"),
        (Section::About, "About", "info"),
    ];
}

/// What the page asks the app to do.
pub enum Ask {
    BotSettings,
    SlashCommands,
    Export,
    ManagePlugins,
    ReloadPlugins,
    AllowNative(bool),
    ResetLayout,
}

#[derive(Default)]
pub struct SettingsView {
    pub section: Section,
    /// The Node.js version check, once run.
    node: Option<Result<String, String>>,
}

const NAV_W: f32 = 190.0;
const CONTENT_MAX: f32 = 680.0;

pub fn ui(ui: &mut Ui, ed: &mut Editor, native: &NativeHost, view: &mut SettingsView, asks: &mut Vec<Ask>) {
    let theme = ed.theme.clone();
    let full = ui.max_rect();
    ui.painter().rect_filled(full, 0.0, theme.panel);

    // Section list.
    let nav = Rect::from_min_size(full.min, vec2(NAV_W, full.height()));
    ui.painter().rect_filled(nav, 0.0, theme.frame);
    ui.painter().vline(nav.right(), nav.y_range(), Stroke::new(1.0, theme.line));
    let mut nav_ui = ui.new_child(egui::UiBuilder::new().max_rect(nav.shrink2(vec2(8.0, 14.0))).layout(Layout::top_down(Align::Min)));
    nav_ui.label(RichText::new("SETTINGS").size(11.0).strong().color(theme.text_muted));
    nav_ui.add_space(8.0);
    for (section, name, icon) in Section::ALL {
        let (rect, resp) = nav_ui.allocate_exact_size(vec2(NAV_W - 16.0, 32.0), Sense::click());
        let selected = view.section == section;
        if selected {
            nav_ui.painter().rect_filled(rect, 6.0, with_alpha(theme.accent, 38));
        } else if resp.hovered() {
            nav_ui.painter().rect_filled(rect, 6.0, theme.panel_header);
        }
        let tint = if selected { theme.accent } else { theme.text_muted };
        if let Some(path) = ed.reg.icons.get(icon) {
            egui::Image::from_uri(file_uri(path)).tint(tint).paint_at(&nav_ui, Rect::from_center_size(rect.left_center() + vec2(18.0, 0.0), vec2(16.0, 16.0)));
        }
        nav_ui.painter().text(rect.left_center() + vec2(36.0, 0.0), Align2::LEFT_CENTER, name, FontId::proportional(13.5), if selected { theme.text } else { theme.text_muted });
        if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
            view.section = section;
        }
    }

    // The section's settings.
    let content = Rect::from_min_max(pos2(nav.right() + 1.0, full.top()), full.max);
    let mut content_ui = ui.new_child(egui::UiBuilder::new().max_rect(content).layout(Layout::top_down(Align::Min)));
    ScrollArea::vertical().id_salt(("settings", view.section as u8)).auto_shrink([false, false]).show(&mut content_ui, |ui| {
        egui::Frame::new().inner_margin(egui::Margin { left: 32, right: 32, top: 24, bottom: 32 }).show(ui, |ui| {
            ui.set_max_width(CONTENT_MAX);
            let title = Section::ALL.iter().find(|(s, ..)| *s == view.section).map(|(_, n, _)| *n).unwrap_or("");
            ui.label(RichText::new(title).size(22.0).strong().color(theme.text));
            ui.add_space(14.0);
            match view.section {
                Section::Appearance => appearance(ui, ed, &theme),
                Section::Editor => editor(ui, ed, &theme, asks),
                Section::Bot => bot(ui, ed, &theme, view, asks),
                Section::Plugins => plugins(ui, ed, native, &theme, asks),
                Section::Shortcuts => shortcuts(ui, &theme),
                Section::About => about(ui, ed, &theme),
            }
        });
    });
}

/// A group heading.
fn group(ui: &mut Ui, theme: &Theme, title: &str) {
    ui.add_space(10.0);
    ui.label(RichText::new(title.to_uppercase()).size(11.0).strong().color(theme.text_muted));
    ui.add_space(6.0);
}

/// One setting: its name and description on the left, the control on the right.
fn row(ui: &mut Ui, theme: &Theme, title: &str, description: &str, control: impl FnOnce(&mut Ui)) {
    egui::Frame::new().fill(theme.panel_header).stroke(Stroke::new(1.0, theme.line)).corner_radius(8).inner_margin(egui::Margin::symmetric(16, 12)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.set_width((ui.available_width() - 250.0).max(200.0));
                ui.spacing_mut().item_spacing.y = 3.0;
                ui.label(RichText::new(title).size(13.5).color(theme.text));
                if !description.is_empty() {
                    ui.label(RichText::new(description).size(11.5).color(theme.text_muted));
                }
            });
            ui.with_layout(Layout::right_to_left(Align::Center), control);
        });
    });
    ui.add_space(8.0);
}

fn toggle(ui: &mut Ui, theme: &Theme, on: &mut bool) -> bool {
    crate::widgets::toggle(ui, on, theme).changed()
}

fn appearance(ui: &mut Ui, ed: &mut Editor, theme: &Theme) {
    let ctx = ui.ctx().clone();
    group(ui, theme, "Theme");
    let themes: Vec<Theme> = ed.reg.themes.iter().map(Theme::from_def).collect();
    let current = ed.theme.id.clone();
    let mut pick: Option<String> = None;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(12.0, 12.0);
        for t in &themes {
            if theme_card(ui, t, theme, t.id == current, ed.prefs.accent.is_some().then_some(ed.theme.accent)) {
                pick = Some(t.id.clone());
            }
        }
    });
    if let Some(id) = pick {
        ed.set_theme(&id);
        ed.theme.apply(&ctx);
    }
    ui.add_space(10.0);

    row(ui, theme, "Accent color", "Used for selections, highlights and the open tab. Changes show right away.", |ui| {
        if ed.prefs.accent.is_some() && ui.button("Reset").on_hover_text("Use the theme's own accent").clicked() {
            ed.prefs.accent = None;
            ed.restyle();
            ed.theme.apply(&ctx);
        }
        // Every change while dragging in the picker restyles the whole app.
        let mut c = ed.theme.accent;
        if crate::color::color_button(ui, egui::Id::new("settings-accent"), &mut c, theme, Some(116.0)) {
            ed.prefs.accent = Some(crate::color::to_hex(c));
            ed.restyle();
            ed.theme.apply(&ctx);
        }
    });
    // Quick picks.
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        ui.add_space(16.0);
        for (name, hex) in ACCENTS {
            let Some(c) = crate::color::parse_hex(hex) else { continue };
            let (r, resp) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::click());
            let selected = crate::color::to_hex(ed.theme.accent) == hex;
            ui.painter().circle_filled(r.center(), 11.0, c);
            if selected {
                ui.painter().circle_stroke(r.center(), 13.0, Stroke::new(2.0, theme.text));
            } else if resp.hovered() {
                ui.painter().circle_stroke(r.center(), 13.0, Stroke::new(1.5, theme.text_muted));
            }
            if resp.on_hover_text(name).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                ed.prefs.accent = Some(hex.to_string());
                ed.restyle();
                ed.theme.apply(&ctx);
            }
        }
    });
    ui.add_space(14.0);

    group(ui, theme, "Interface");
    row(ui, theme, "Interface size", "Makes all text and controls bigger or smaller.", |ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        for s in UI_SCALES.iter().rev() {
            let selected = (ed.prefs.ui_scale - s).abs() < 0.01;
            let text = RichText::new(format!("{:.0}%", s * 100.0)).size(12.0).color(if selected { Color32::from_rgb(0, 24, 28) } else { theme.text });
            let b = egui::Button::new(text).fill(if selected { theme.accent } else { theme.field }).min_size(vec2(46.0, 26.0));
            if ui.add(b).clicked() && !selected {
                ed.prefs.ui_scale = *s;
                ctx.set_zoom_factor(*s);
            }
        }
    });

    group(ui, theme, "Canvas");
    row(ui, theme, "Grid dots", "The dotted background on canvases.", |ui| {
        toggle(ui, theme, &mut ed.prefs.show_grid);
    });
    let theme_wire = ed.reg.theme(&current).map(|d| d.shape.wire_width).unwrap_or(2.0);
    row(ui, theme, "Wire thickness", "How thick the wires between blocks are drawn.", |ui| {
        if ed.prefs.wire_width.is_some() && ui.button("Reset").clicked() {
            ed.prefs.wire_width = None;
            ed.restyle();
        }
        let mut w = ed.prefs.wire_width.unwrap_or(theme_wire);
        if ui.add(egui::Slider::new(&mut w, 1.0..=5.0).step_by(0.5).suffix(" px")).changed() {
            ed.prefs.wire_width = Some(w);
            ed.restyle();
        }
    });
}

/// A small picture of a theme: title bar, sidebar, canvas with a block and a wire.
fn theme_card(ui: &mut Ui, t: &Theme, page: &Theme, selected: bool, accent_override: Option<Color32>) -> bool {
    let (rect, resp) = ui.allocate_exact_size(vec2(170.0, 132.0), Sense::click());
    let accent = accent_override.unwrap_or(t.accent);
    let preview = Rect::from_min_size(rect.min, vec2(rect.width(), 100.0));
    let p = ui.painter();
    p.rect_filled(preview, 8.0, t.canvas);
    let top = Rect::from_min_size(preview.min, vec2(preview.width(), 14.0));
    p.rect_filled(top, egui::CornerRadius { nw: 8, ne: 8, sw: 0, se: 0 }, t.frame);
    let side = Rect::from_min_max(pos2(preview.left(), top.bottom()), pos2(preview.left() + 38.0, preview.bottom()));
    p.rect_filled(side, egui::CornerRadius { nw: 0, ne: 0, sw: 8, se: 0 }, t.panel);
    p.rect_filled(Rect::from_min_size(side.min + vec2(6.0, 8.0), vec2(26.0, 5.0)), 2.0, with_alpha(accent, 140));
    for i in 0..3 {
        p.rect_filled(Rect::from_min_size(side.min + vec2(6.0, 18.0 + i as f32 * 9.0), vec2(22.0, 4.0)), 2.0, with_alpha(t.text_muted, 90));
    }
    let node = Rect::from_min_size(pos2(side.right() + 16.0, top.bottom() + 16.0), vec2(56.0, 44.0));
    p.rect(node, 5.0, t.node, Stroke::new(1.0, t.node_border), StrokeKind::Inside);
    p.rect_filled(Rect::from_min_size(node.min, vec2(node.width(), 11.0)), egui::CornerRadius { nw: 5, ne: 5, sw: 0, se: 0 }, t.node_header);
    let node2 = Rect::from_min_size(pos2(node.right() + 18.0, node.top() + 18.0), vec2(40.0, 30.0));
    p.rect(node2, 5.0, t.node, Stroke::new(1.0, accent), StrokeKind::Inside);
    let (a, b) = (pos2(node.right(), node.top() + 20.0), pos2(node2.left(), node2.top() + 8.0));
    p.add(egui::epaint::CubicBezierShape::from_points_stroke([a, a + vec2(10.0, 0.0), b - vec2(10.0, 0.0), b], false, Color32::TRANSPARENT, Stroke::new(2.0, t.action)));
    let border = if selected { Stroke::new(2.0, page.accent) } else if resp.hovered() { Stroke::new(1.0, page.text_muted) } else { Stroke::new(1.0, page.line) };
    p.rect_stroke(preview, 8.0, border, StrokeKind::Outside);
    p.text(pos2(rect.left() + 2.0, preview.bottom() + 12.0), Align2::LEFT_CENTER, &t.name, FontId::proportional(13.0), page.text);
    p.text(pos2(rect.right() - 2.0, preview.bottom() + 12.0), Align2::RIGHT_CENTER, if t.dark { "Dark" } else { "Light" }, FontId::proportional(11.0), page.text_muted);
    resp.on_hover_cursor(CursorIcon::PointingHand).clicked()
}

fn editor(ui: &mut Ui, ed: &mut Editor, theme: &Theme, asks: &mut Vec<Ask>) {
    let ctx = ui.ctx().clone();
    group(ui, theme, "Window");
    row(ui, theme, "Activity bar", "The icons on the far left for Workspaces, Stored Data, Plugins and Settings.", |ui| {
        toggle(ui, theme, &mut ed.prefs.activity_bar);
    });
    row(ui, theme, "System title bar", "Use the operating system's title bar instead of XC Circuit's own (with the search box).", |ui| {
        if toggle(ui, theme, &mut ed.prefs.system_titlebar) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Decorations(ed.prefs.system_titlebar));
        }
    });
    row(ui, theme, "Panel layout", "Put every panel back where it started.", |ui| {
        if ui.button("Reset layout").clicked() {
            asks.push(Ask::ResetLayout);
        }
    });
    group(ui, theme, "Startup");
    row(ui, theme, "Reopen last project", "Open the project you had open last time. When off, XC Circuit starts with the example.", |ui| {
        toggle(ui, theme, &mut ed.prefs.reopen_last_project);
    });
}

fn bot(ui: &mut Ui, ed: &mut Editor, theme: &Theme, view: &mut SettingsView, asks: &mut Vec<Ask>) {
    group(ui, theme, "Running");
    row(ui, theme, "Restart on save", "Saving restarts a running bot so it uses your changes.", |ui| {
        toggle(ui, theme, &mut ed.prefs.auto_restart);
    });
    if view.node.is_none() {
        view.node = Some(circuit_runner::node::check_node().map_err(|e| format!("{e:#}")));
    }
    let (status, color) = match &view.node {
        Some(Ok(v)) => (format!("Node.js {v}"), theme.action),
        Some(Err(e)) => (e.clone(), theme.warning),
        None => (String::new(), theme.text_muted),
    };
    row(ui, theme, "Node.js", "Bots run with Node.js 22 or newer.", |ui| {
        if ui.button("Check again").clicked() {
            view.node = None;
        }
        ui.label(RichText::new(status).size(12.0).color(color));
    });
    let packages = crate::bot::app_data_dir().join("packages");
    row(ui, theme, "npm packages", &format!("Installed once for all bots: {}", packages.display()), |ui| {
        if ui.button("Open folder").clicked() {
            let _ = std::fs::create_dir_all(&packages);
            let _ = open::that(&packages);
        }
    });
    if let Some(p) = &ed.project {
        group(ui, theme, &format!("This project · {}", p.meta.name));
        row(ui, theme, "Bot settings", "Token and the other settings of this project's bot.", |ui| {
            if ui.button("Open…").clicked() {
                asks.push(Ask::BotSettings);
            }
        });
        row(ui, theme, "Slash commands", "The commands this bot registers with Discord.", |ui| {
            if ui.button("Edit…").clicked() {
                asks.push(Ask::SlashCommands);
            }
        });
        row(ui, theme, "Export", "Write a folder that runs with node index.js.", |ui| {
            if ui.button("Export…").clicked() {
                asks.push(Ask::Export);
            }
        });
    }
}

fn plugins(ui: &mut Ui, ed: &mut Editor, native: &NativeHost, theme: &Theme, asks: &mut Vec<Ask>) {
    group(ui, theme, "Plugins");
    row(ui, theme, "Installed plugins", &format!("{} plugins with {} blocks and {} themes are loaded.", ed.reg.plugins.len(), ed.reg.blocks.len(), ed.reg.themes.len()), |ui| {
        if ui.button("Manage…").clicked() {
            asks.push(Ask::ManagePlugins);
        }
        if ui.button("Reload").on_hover_text("Load plugin files again").clicked() {
            asks.push(Ask::ReloadPlugins);
        }
    });
    let mut allow = native.allow;
    row(ui, theme, "Allow native plugins", "Native plugins are compiled Rust code with full access to your computer. Only turn this on for plugins you trust.", |ui| {
        if toggle(ui, theme, &mut allow) {
            asks.push(Ask::AllowNative(allow));
        }
    });
    group(ui, theme, "Plugin folders");
    for dir in ed.plugin_dirs.clone() {
        row(ui, theme, &dir.display().to_string(), if dir.is_dir() { "" } else { "Doesn't exist yet" }, |ui| {
            if ui.button("Open").clicked() {
                let _ = std::fs::create_dir_all(&dir);
                let _ = open::that(&dir);
            }
        });
    }
}

const SHORTCUTS: [(&str, &str); 18] = [
    ("Ctrl+P", "Command palette: commands, workspaces and blocks"),
    ("Ctrl+,", "Settings"),
    ("Ctrl+S", "Save (restarts a running bot)"),
    ("F5", "Run or restart the bot"),
    ("Shift+F5", "Stop the bot"),
    ("Ctrl+Z", "Undo"),
    ("Ctrl+Y", "Redo"),
    ("Ctrl+C / Ctrl+X / Ctrl+V", "Copy, cut and paste blocks"),
    ("Ctrl+D", "Duplicate the selected blocks"),
    ("Ctrl+A", "Select all blocks"),
    ("Delete", "Delete the selected blocks or wire"),
    ("Space", "Open the block picker at the mouse"),
    ("Right-click", "Block picker on the canvas; menus on blocks, wires and fields"),
    ("Shift-drag", "Select blocks with a box"),
    ("Mouse wheel", "Zoom the canvas"),
    ("Drag empty canvas", "Pan"),
    ("Double-click", "Rename a workspace or folder in the sidebar"),
    ("Esc", "Close a picker, menu or editor"),
];

fn shortcuts(ui: &mut Ui, theme: &Theme) {
    egui::Frame::new().fill(theme.panel_header).stroke(Stroke::new(1.0, theme.line)).corner_radius(8).inner_margin(egui::Margin::symmetric(16, 8)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        egui::Grid::new("shortcuts").num_columns(2).spacing(vec2(24.0, 10.0)).show(ui, |ui| {
            for (keys, what) in SHORTCUTS {
                egui::Frame::new().fill(theme.field).corner_radius(5).inner_margin(egui::Margin::symmetric(8, 3)).show(ui, |ui| {
                    ui.label(RichText::new(keys).monospace().size(12.0).color(theme.text));
                });
                ui.label(RichText::new(what).size(13.0).color(theme.text));
                ui.end_row();
            }
        });
    });
}

fn about(ui: &mut Ui, ed: &Editor, theme: &Theme) {
    ui.horizontal(|ui| {
        ui.add(egui::Image::new(egui::include_image!("../assets/icon.svg")).fit_to_exact_size(vec2(56.0, 56.0)));
        ui.add_space(8.0);
        ui.vertical(|ui| {
            ui.label(RichText::new("XC™ Circuit").size(20.0).strong().color(theme.text));
            ui.label(RichText::new(format!("Version {}", env!("CARGO_PKG_VERSION"))).color(theme.text_muted));
            ui.label(RichText::new("Build bots by connecting blocks.").color(theme.text));
        });
    });
    ui.add_space(14.0);
    group(ui, theme, "Where things are kept");
    let data = crate::bot::app_data_dir();
    for (title, path) in [("App data", data.clone()), ("npm packages", data.join("packages")), ("Plugins you install", data.join("plugins"))] {
        row(ui, theme, title, &path.display().to_string(), |ui| {
            if ui.button("Open").clicked() {
                let _ = std::fs::create_dir_all(&path);
                let _ = open::that(&path);
            }
        });
    }
    if let Some(p) = &ed.project {
        let root = p.root.clone();
        row(ui, theme, "This project", &root.display().to_string(), |ui| {
            if ui.button("Open").clicked() {
                let _ = open::that(&root);
            }
        });
    }
    ui.add_space(6.0);
    ui.label(RichText::new("Bot tokens are kept in Windows Credential Manager or the Linux Secret Service, never in project files.").size(11.5).color(mix(theme.text_muted, theme.panel, 0.1)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accent_override_follows_theme_switches() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut ed = Editor::new(vec![root.join("plugins")], Some("graphite")).unwrap();
        let light_own = ed.reg.theme("light").map(Theme::from_def).unwrap().accent;
        ed.prefs.accent = Some("#FF5CA8".into());
        ed.restyle();
        assert_eq!(crate::color::to_hex(ed.theme.accent), "#FF5CA8");
        ed.set_theme("light");
        assert_eq!(crate::color::to_hex(ed.theme.accent), "#FF5CA8", "the override stays when switching themes");
        ed.prefs.accent = None;
        ed.restyle();
        assert_eq!(ed.theme.accent, light_own, "Reset brings back the theme's own accent");
    }
}
