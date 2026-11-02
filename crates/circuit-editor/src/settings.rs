//! The Settings page, opened as a tab: appearance, editor, bot, plugins,
//! shortcuts and about. Changes apply right away and are kept between sessions.

use eframe::egui::{self, Align, Align2, Color32, CursorIcon, FontId, Layout, Rect, RichText, ScrollArea, Sense, Stroke, StrokeKind, Ui, pos2, vec2};

use crate::editor::{Editor, file_uri};
use crate::i18n::n;
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
    Backups,
    Shortcuts,
    About,
}

impl Section {
    const ALL: [(Section, &'static str, &'static str); 7] = [
        (Section::Appearance, n("Appearance"), "palette"),
        (Section::Editor, n("Editor"), "workspace"),
        (Section::Bot, n("Bot"), "bolt"),
        (Section::Plugins, n("Plugins"), "plug"),
        (Section::Backups, n("Backups"), "clock"),
        (Section::Shortcuts, n("Shortcuts"), "keyboard"),
        (Section::About, n("About"), "info"),
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
    NodeSetup,
    OpenBackups,
}

#[derive(Default)]
pub struct SettingsView {
    pub section: Section,
    /// The Node.js version check, once run.
    node: Option<Result<String, String>>,
    /// "Create a translation": language code, name in that language, English name.
    new_translation: Option<[String; 3]>,
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
    nav_ui.label(RichText::new(t!("SETTINGS")).size(11.0).strong().color(theme.text_muted));
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
        nav_ui.painter().text(rect.left_center() + vec2(36.0, 0.0), Align2::LEFT_CENTER, crate::i18n::t(name), FontId::proportional(13.5), if selected { theme.text } else { theme.text_muted });
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
            let title = Section::ALL.iter().find(|(s, ..)| *s == view.section).map(|(_, n, _)| crate::i18n::t(n)).unwrap_or("");
            ui.label(RichText::new(title).size(22.0).strong().color(theme.text));
            ui.add_space(14.0);
            match view.section {
                Section::Appearance => appearance(ui, ed, &theme, view),
                Section::Editor => editor(ui, ed, &theme, asks),
                Section::Bot => bot(ui, ed, &theme, view, asks),
                Section::Plugins => plugins(ui, ed, native, &theme, asks),
                Section::Backups => backups(ui, ed, &theme, asks),
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

/// One button per language; a click switches the whole interface right away.
/// Shown in Settings and on the first start page.
pub fn language_buttons(ui: &mut Ui, ed: &mut Editor, theme: &Theme) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        for (code, name) in crate::i18n::languages(&ed.reg) {
            let selected = ed.prefs.language == code;
            let ink = if selected { Color32::from_rgb(0, 24, 28) } else { theme.text };
            let b = egui::Button::new(RichText::new(&name).size(13.0).color(ink)).min_size(vec2(96.0, crate::inputs::input_height(ui).max(ui.spacing().interact_size.y)));
            if crate::dialog::styled_button(ui, theme, b, if selected { theme.accent } else { theme.field }, Stroke::NONE).clicked() && !selected {
                ed.prefs.language = code;
                ed.apply_language();
            }
        }
    });
}

fn appearance(ui: &mut Ui, ed: &mut Editor, theme: &Theme, view: &mut SettingsView) {
    let ctx = ui.ctx().clone();
    group(ui, theme, t!("Language"));
    row(ui, theme, t!("Interface language"), t!("Menus, settings and block names. More languages come with plugins."), |ui| {
        ui.with_layout(Layout::left_to_right(Align::Center), |ui| language_buttons(ui, ed, theme));
    });
    row(ui, theme, t!("Translate XC Circuit"), t!("Makes a file with every text to translate, saved as a plugin you can share. Changes show while you edit it."), |ui| {
        if crate::dialog::button(ui, theme, t!("Create a translation…")).clicked() {
            view.new_translation = Some(Default::default());
        }
    });
    if view.new_translation.is_some() {
        new_translation(ui.ctx(), ed, theme, view);
    }
    ui.add_space(14.0);
    group(ui, theme, t!("Theme"));
    theme_cards(ui, ed, theme);
    let current = ed.theme.id.clone();
    ui.add_space(10.0);

    row(ui, theme, t!("Accent color"), t!("Used for selections, highlights and the open tab. Changes show right away."), |ui| {
        if ed.prefs.accent.is_some() && crate::dialog::button(ui, theme, t!("Reset")).on_hover_text(t!("Use the theme's own accent")).clicked() {
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
    accent_swatches(ui, ed, theme, 16.0);
    ui.add_space(14.0);

    group(ui, theme, t!("Interface"));
    row(ui, theme, t!("Touch mode"), t!("Bigger buttons and more room between them, for touch screens and pens."), |ui| {
        if toggle(ui, theme, &mut ed.prefs.touch) {
            ed.restyle();
            ed.theme.apply(&ctx);
        }
    });
    row(ui, theme, t!("Interface size"), t!("Makes all text and controls bigger or smaller."), |ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        for s in UI_SCALES.iter().rev() {
            let selected = (ed.prefs.ui_scale - s).abs() < 0.01;
            let text = RichText::new(format!("{:.0}%", s * 100.0)).size(12.0).color(if selected { Color32::from_rgb(0, 24, 28) } else { theme.text });
            let b = egui::Button::new(text).min_size(vec2(52.0, crate::inputs::input_height(ui)));
            if crate::dialog::styled_button(ui, theme, b, if selected { theme.accent } else { theme.field }, Stroke::NONE).clicked() && !selected {
                ed.prefs.ui_scale = *s;
                ctx.set_zoom_factor(*s);
            }
        }
    });

    group(ui, theme, t!("Canvas"));
    row(ui, theme, t!("Grid dots"), t!("The dotted background on canvases."), |ui| {
        toggle(ui, theme, &mut ed.prefs.show_grid);
    });
    let theme_wire = ed.reg.theme(&current).map(|d| d.shape.wire_width).unwrap_or(2.0);
    row(ui, theme, t!("Wire thickness"), t!("How thick the wires between blocks are drawn."), |ui| {
        if ed.prefs.wire_width.is_some() && crate::dialog::button(ui, theme, t!("Reset")).clicked() {
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
pub fn theme_card(ui: &mut Ui, t: &Theme, page: &Theme, selected: bool, accent_override: Option<Color32>) -> bool {
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
    p.text(pos2(rect.right() - 2.0, preview.bottom() + 12.0), Align2::RIGHT_CENTER, if t.dark { t!("Dark") } else { t!("Light") }, FontId::proportional(11.0), page.text_muted);
    resp.on_hover_cursor(CursorIcon::PointingHand).clicked()
}

fn editor(ui: &mut Ui, ed: &mut Editor, theme: &Theme, asks: &mut Vec<Ask>) {
    let ctx = ui.ctx().clone();
    group(ui, theme, t!("Window"));
    row(ui, theme, t!("Activity bar"), t!("The icons on the far left for Workspaces, Stored Data, Plugins and Settings."), |ui| {
        toggle(ui, theme, &mut ed.prefs.activity_bar);
    });
    row(ui, theme, t!("System title bar"), t!("Use the operating system's title bar instead of XC Circuit's own (with the search box)."), |ui| {
        if toggle(ui, theme, &mut ed.prefs.system_titlebar) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Decorations(ed.prefs.system_titlebar));
        }
    });
    row(ui, theme, t!("Panel layout"), t!("Put every panel back where it started."), |ui| {
        if crate::dialog::button(ui, theme, t!("Reset layout")).clicked() {
            asks.push(Ask::ResetLayout);
        }
    });
    group(ui, theme, t!("Startup"));
    row(ui, theme, t!("Reopen last project"), t!("Open the project you had open last time. When off, XC Circuit starts with the example."), |ui| {
        toggle(ui, theme, &mut ed.prefs.reopen_last_project);
    });
}

fn bot(ui: &mut Ui, ed: &mut Editor, theme: &Theme, view: &mut SettingsView, asks: &mut Vec<Ask>) {
    group(ui, theme, t!("Running"));
    row(ui, theme, t!("Restart on save"), t!("Saving restarts a running bot so it uses your changes."), |ui| {
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
    let which = ed.prefs.node_path.clone().unwrap_or_else(|| t!("node from the PATH").into());
    row(ui, theme, "Node.js", &format!("Bots run with Node.js 22 or newer. Using: {which}"), |ui| {
        if crate::dialog::button(ui, theme, t!("Set up…")).on_hover_text(t!("Find Node.js on this computer or install it")).clicked() {
            asks.push(Ask::NodeSetup);
            view.node = None;
        }
        if crate::dialog::button(ui, theme, t!("Check again")).clicked() {
            view.node = None;
        }
        ui.label(RichText::new(status).size(12.0).color(color));
    });
    let packages = crate::bot::app_data_dir().join("packages");
    row(ui, theme, t!("npm packages"), &tf!("Installed once for all bots: {folder}", folder = packages.display()), |ui| {
        if crate::dialog::button(ui, theme, t!("Open folder")).clicked() {
            let _ = std::fs::create_dir_all(&packages);
            let _ = open::that(&packages);
        }
    });
    if let Some(p) = &ed.project {
        group(ui, theme, &format!("This project · {}", p.meta.name));
        row(ui, theme, t!("Bot settings"), t!("Token and the other settings of this project's bot."), |ui| {
            if crate::dialog::button(ui, theme, t!("Open…")).clicked() {
                asks.push(Ask::BotSettings);
            }
        });
        row(ui, theme, t!("Slash commands"), t!("The commands this bot registers with Discord."), |ui| {
            if crate::dialog::button(ui, theme, t!("Edit…")).clicked() {
                asks.push(Ask::SlashCommands);
            }
        });
        row(ui, theme, t!("Export"), t!("Write a folder that runs with node index.js."), |ui| {
            if crate::dialog::button(ui, theme, t!("Export…")).clicked() {
                asks.push(Ask::Export);
            }
        });
    }
}

fn backups(ui: &mut Ui, ed: &mut Editor, theme: &Theme, asks: &mut Vec<Ask>) {
    group(ui, theme, t!("Project backups"));
    row(ui, theme, t!("Back up on every save"), t!("Keeps a copy of the project each time you save, outside the project folder."), |ui| {
        toggle(ui, theme, &mut ed.prefs.backups);
    });
    row(ui, theme, t!("Keep the newest"), t!("How many of the most recent backups are always kept."), |ui| {
        ui.add(egui::Slider::new(&mut ed.prefs.backup_keep, 5..=100).suffix(" backups"));
    });
    row(ui, theme, t!("And one a day for"), t!("Older backups thin out to the last one of each day."), |ui| {
        ui.add(egui::Slider::new(&mut ed.prefs.backup_days, 0..=60).suffix(" days"));
    });
    row(ui, theme, t!("Restore"), t!("See what changed and bring back the project, a workspace or the bot's saved data."), |ui| {
        if crate::dialog::button(ui, theme, t!("Restore from backup…")).clicked() {
            asks.push(Ask::OpenBackups);
        }
    });
    let folder = crate::bot::app_data_dir().join("backups");
    row(ui, theme, t!("Backups folder"), &folder.display().to_string(), |ui| {
        if crate::dialog::button(ui, theme, t!("Open")).clicked() {
            let _ = std::fs::create_dir_all(&folder);
            let _ = open::that(&folder);
        }
    });
    group(ui, theme, t!("Unsaved changes"));
    row(ui, theme, t!("Crash recovery"), t!("While you have unsaved changes, a copy is kept every 30 seconds. If XC Circuit or your computer stops, you're offered your changes back the next time you open the project."), |_| {});
    group(ui, theme, t!("The bot's saved data"));
    row(ui, theme, t!("Backups by the bot"), t!("Every bot backs up its saved data each time it starts and every 6 hours, keeping the newest 10. If the data is ever empty, it asks whether to restore a backup."), |_| {});
}

fn plugins(ui: &mut Ui, ed: &mut Editor, native: &NativeHost, theme: &Theme, asks: &mut Vec<Ask>) {
    group(ui, theme, t!("Plugins"));
    row(ui, theme, t!("Installed plugins"), &tf!("{plugins} plugins with {blocks} blocks and {themes} themes are loaded.", plugins = ed.reg.plugins.len(), blocks = ed.reg.blocks.len(), themes = ed.reg.themes.len()), |ui| {
        if crate::dialog::button(ui, theme, t!("Manage…")).clicked() {
            asks.push(Ask::ManagePlugins);
        }
        if crate::dialog::button(ui, theme, t!("Reload")).on_hover_text(t!("Load plugin files again")).clicked() {
            asks.push(Ask::ReloadPlugins);
        }
    });
    let mut allow = native.allow;
    row(ui, theme, t!("Allow native plugins"), t!("Native plugins are compiled Rust code with full access to your computer. Only turn this on for plugins you trust."), |ui| {
        if toggle(ui, theme, &mut allow) {
            asks.push(Ask::AllowNative(allow));
        }
    });
    group(ui, theme, t!("Plugin folders"));
    for dir in ed.plugin_dirs.clone() {
        row(ui, theme, &dir.display().to_string(), if dir.is_dir() { "" } else { t!("Doesn't exist yet") }, |ui| {
            if crate::dialog::button(ui, theme, t!("Open")).clicked() {
                let _ = std::fs::create_dir_all(&dir);
                let _ = open::that(&dir);
            }
        });
    }
}

const SHORTCUTS: [(&str, &str); 18] = [
    (n("Ctrl+P"), n("Command palette: commands, workspaces and blocks")),
    (n("Ctrl+,"), n("Settings")),
    (n("Ctrl+S"), n("Save (restarts a running bot)")),
    (n("F5"), n("Run or restart the bot")),
    (n("Shift+F5"), n("Stop the bot")),
    (n("Ctrl+Z"), n("Undo")),
    (n("Ctrl+Y"), n("Redo")),
    (n("Ctrl+C / Ctrl+X / Ctrl+V"), n("Copy, cut and paste blocks")),
    (n("Ctrl+D"), n("Duplicate the selected blocks")),
    (n("Ctrl+A"), n("Select all blocks")),
    (n("Delete"), n("Delete the selected blocks or wire")),
    (n("Space"), n("Open the block picker at the mouse")),
    (n("Right-click"), n("Block picker on empty canvas; selects a block or wire (its actions show as buttons)")),
    (n("Shift-drag"), n("Select blocks with a box")),
    (n("Mouse wheel"), n("Zoom the canvas")),
    (n("Drag empty canvas"), n("Pan")),
    (n("Double-click"), n("Rename a workspace or folder in the sidebar")),
    (n("Esc"), n("Close a picker, menu or editor")),
];

fn shortcuts(ui: &mut Ui, theme: &Theme) {
    egui::Frame::new().fill(theme.panel_header).stroke(Stroke::new(1.0, theme.line)).corner_radius(8).inner_margin(egui::Margin::symmetric(16, 8)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        egui::Grid::new("shortcuts").num_columns(2).spacing(vec2(24.0, 10.0)).show(ui, |ui| {
            for (keys, what) in SHORTCUTS {
                egui::Frame::new().fill(theme.field).corner_radius(5).inner_margin(egui::Margin::symmetric(8, 3)).show(ui, |ui| {
                    ui.label(RichText::new(crate::i18n::t(keys)).monospace().size(12.0).color(theme.text));
                });
                ui.label(RichText::new(crate::i18n::t(what)).size(13.0).color(theme.text));
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
            ui.label(RichText::new(t!("XC™ Circuit")).size(20.0).strong().color(theme.text));
            ui.label(RichText::new(format!("Version {}", env!("CARGO_PKG_VERSION"))).color(theme.text_muted));
            ui.label(RichText::new(t!("Build bots by connecting blocks.")).color(theme.text));
        });
    });
    ui.add_space(14.0);
    group(ui, theme, t!("Where things are kept"));
    let data = crate::bot::app_data_dir();
    for (title, path) in [(t!("App data"), data.clone()), (t!("npm packages"), data.join("packages")), (t!("Plugins you install"), data.join("plugins"))] {
        row(ui, theme, title, &path.display().to_string(), |ui| {
            if crate::dialog::button(ui, theme, t!("Open")).clicked() {
                let _ = std::fs::create_dir_all(&path);
                let _ = open::that(&path);
            }
        });
    }
    if let Some(p) = &ed.project {
        let root = p.root.clone();
        row(ui, theme, t!("This project"), &root.display().to_string(), |ui| {
            if crate::dialog::button(ui, theme, t!("Open")).clicked() {
                let _ = open::that(&root);
            }
        });
    }
    ui.add_space(6.0);
    ui.label(RichText::new(t!("Bot tokens are kept in Windows Credential Manager or the Linux Secret Service, never in project files.")).size(11.5).color(mix(theme.text_muted, theme.panel, 0.1)));
}


/// The quick accent colors, applied to the whole app when clicked.
pub fn accent_swatches(ui: &mut Ui, ed: &mut Editor, theme: &Theme, indent: f32) {
    let ctx = ui.ctx().clone();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        ui.add_space(indent);
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
}

/// Applies a theme picked from its card.
pub fn theme_cards(ui: &mut Ui, ed: &mut Editor, theme: &Theme) {
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
        ed.theme.apply(ui.ctx());
    }
}

/// The "Create a translation" question: which language.
fn new_translation(ctx: &egui::Context, ed: &mut Editor, theme: &Theme, view: &mut SettingsView) {
    let Some([code, name, english]) = view.new_translation.as_mut() else { return };
    let mut create = false;
    let mut cancel = false;
    let close = crate::dialog::show(
        ctx,
        theme,
        crate::dialog::Spec { id: "new-translation", title: t!("Create a translation"), subtitle: Some(t!("Every text is listed in English, ready to fill in")), icon: None, width: 460.0 },
        |ui, part| match part {
            crate::dialog::Part::Body => {
                crate::dialog::field(ui, theme, t!("Language code"), Some(t!("Such as fr, es or pt-BR. An existing one (de) starts from its translation.")), |ui| {
                    crate::dialog::text_input(ui, code, "fr", false);
                });
                crate::dialog::field(ui, theme, t!("Name in that language"), None, |ui| {
                    crate::dialog::text_input(ui, name, "Français", false);
                });
                crate::dialog::field(ui, theme, t!("Name in English"), None, |ui| {
                    crate::dialog::text_input(ui, english, "French", false);
                });
            }
            crate::dialog::Part::Footer => {
                create = ui.add_enabled_ui(!code.trim().is_empty(), |ui| crate::dialog::primary_button(ui, theme, t!("Create"))).inner.clicked();
                cancel = crate::dialog::secondary_button(ui, theme, t!("Cancel")).clicked();
            }
        },
    );
    if create {
        let dir = crate::bot::app_data_dir().join("plugins");
        let fallback = code.trim().to_string();
        let name = if name.trim().is_empty() { fallback.clone() } else { name.trim().to_string() };
        match crate::i18n::create_translation(&dir, &ed.reg, &ed.block_english, code.trim(), &name, english.trim()) {
            Ok(file) => {
                ed.log(crate::editor::Level::Success, tf!("Translation file ready: {file}", file = file.display()));
                let _ = open::that(&file);
                ed.reload_plugins();
            }
            Err(e) => ed.log(crate::editor::Level::Error, tf!("Couldn't create the translation: {error}", error = format!("{e:#}"))),
        }
        view.new_translation = None;
    } else if cancel || close {
        view.new_translation = None;
    }
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
