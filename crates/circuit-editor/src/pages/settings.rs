//! The Settings page, opened as a tab: appearance, editor, bot, plugins,
//! shortcuts and about. Changes apply right away and are kept between sessions.

use eframe::egui::{self, Align, Align2, Color32, CursorIcon, FontId, Layout, Rect, RichText, ScrollArea, Sense, Stroke, StrokeKind, Ui, pos2, vec2};

use crate::ui::kit;
use crate::editor::{Editor, Level};
use crate::i18n::n;
use crate::editor::prefs::{ACCENTS, UI_SCALES};
use crate::ui::theme::{Theme, mix, with_alpha};

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
    pub(crate) const ALL: [(Section, &'static str, &'static str); 7] = [
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
    ResetLayout,
    NodeSetup,
    OpenBackups,
    /// The Console's font changed: load it.
    ConsoleFont,
}

#[derive(Default)]
pub struct SettingsView {
    pub section: Section,
    /// "Create a translation": language code, name in that language, English name.
    new_translation: Option<[String; 3]>,
    /// The shortcut being recorded: its action and which of its shortcuts (or a new one).
    recording: Option<(crate::editor::keymap::Action, usize)>,
    /// What the last change did, shown under that action: "Taken from Save".
    shortcut_note: Option<(crate::editor::keymap::Action, String)>,
    /// The Shortcuts page's search.
    shortcut_filter: String,
    /// The installed fonts (name, monospaced), once read, and while they're read.
    fonts: Option<Vec<(String, bool)>>,
    font_list: Option<std::sync::mpsc::Receiver<Vec<(String, bool)>>>,
    font_filter: String,
}

pub const NAV_W: f32 = 190.0;
/// Narrower than this (the page beside the list too cramped), the list of
/// pages becomes a dropdown above the page: in a narrow dock column, say.
pub const NARROW_W: f32 = NAV_W + 360.0;

/// The strip at the top of a narrow settings tab, holding its pages' dropdown.
/// Returns the strip's inside, for the dropdown, and where the page goes.
pub fn narrow_bar(ui: &Ui, theme: &Theme, full: Rect) -> (Rect, Rect) {
    let bar = Rect::from_min_size(full.min, vec2(full.width(), 54.0));
    ui.painter().rect_filled(bar, 0.0, theme.frame);
    ui.painter().hline(bar.x_range(), bar.bottom(), Stroke::new(1.0, theme.line));
    (bar.shrink2(vec2(12.0, 10.0)), Rect::from_min_max(pos2(full.left(), bar.bottom() + 1.0), full.max))
}
const CONTENT_MAX: f32 = 680.0;

pub fn ui(ui: &mut Ui, ed: &mut Editor, view: &mut SettingsView, asks: &mut Vec<Ask>) {
    let theme = ed.theme.clone();
    let full = ui.max_rect();
    ui.painter().rect_filled(full, 0.0, theme.panel);

    // Section list: on the left, or (narrow) a dropdown above the page.
    let narrow = full.width() < NARROW_W;
    let content = if narrow {
        let (inside, content) = narrow_bar(ui, &theme, full);
        let mut bar_ui = ui.new_child(egui::UiBuilder::new().max_rect(inside).layout(Layout::left_to_right(Align::Center)));
        let shown = Section::ALL.iter().find(|(s, ..)| *s == view.section).map(|(_, n, _)| crate::i18n::t(n)).unwrap_or("");
        let mut picked = None;
        kit::dropdown(&mut bar_ui, egui::Id::unique("settings-pages"), inside.width().min(320.0), shown, |ui| {
            for (section, name, _) in Section::ALL {
                if kit::option(ui, view.section == section, crate::i18n::t(name)).clicked() {
                    picked = Some(section);
                }
            }
        });
        if let Some(section) = picked {
            view.section = section;
            view.recording = None;
        }
        content
    } else {
        let nav = Rect::from_min_size(full.min, vec2(NAV_W, full.height()));
        ui.painter().rect_filled(nav, 0.0, theme.frame);
        ui.painter().vline(nav.right(), nav.y_range(), Stroke::new(1.0, theme.line));
        let mut nav_ui = ui.new_child(egui::UiBuilder::new().max_rect(nav.shrink2(vec2(8.0, 14.0))).layout(Layout::top_down(Align::Min)));
        nav_ui.label(RichText::new(t!("SETTINGS")).size(11.0).strong().color(theme.text_muted));
        nav_ui.add_space(8.0);
        for (section, name, icon) in Section::ALL {
            if kit::nav_item(&mut nav_ui, &theme, ed.reg.icons.get(icon).map(|p| p.as_path()), crate::i18n::t(name), view.section == section, None).clicked() {
                view.section = section;
                view.recording = None;
            }
        }
        Rect::from_min_max(pos2(nav.right() + 1.0, full.top()), full.max)
    };

    // The section's settings.
    let side = if narrow { 16 } else { 32 };
    let mut content_ui = ui.new_child(egui::UiBuilder::new().max_rect(content).layout(Layout::top_down(Align::Min)));
    ScrollArea::vertical().id_salt(("settings", view.section as u8)).auto_shrink([false, false]).show(&mut content_ui, |ui| {
        egui::Frame::new().inner_margin(egui::Margin { left: side, right: side, top: 24, bottom: 32 }).show(ui, |ui| {
            // At most this wide, and never wider than the page (set_max_width sets the width).
            ui.set_max_width(ui.available_width().min(CONTENT_MAX));
            let title = Section::ALL.iter().find(|(s, ..)| *s == view.section).map(|(_, n, _)| crate::i18n::t(n)).unwrap_or("");
            ui.label(RichText::new(title).size(22.0).strong().color(theme.text));
            ui.add_space(14.0);
            match view.section {
                Section::Appearance => appearance(ui, ed, &theme, view),
                Section::Editor => editor(ui, ed, &theme, view, asks),
                Section::Bot => bot(ui, ed, &theme, asks),
                Section::Plugins => plugins(ui, ed, &theme, asks),
                Section::Backups => backups(ui, ed, &theme, asks),
                Section::Shortcuts => shortcuts(ui, ed, &theme, view),
                Section::About => about(ui, ed, &theme),
            }
        });
    });
}

fn toggle(ui: &mut Ui, theme: &Theme, on: &mut bool) -> bool {
    crate::ui::kit::toggle(ui, on, theme).changed()
}

/// One button per language; a click switches the whole interface right away.
/// Shown in Settings and on the first start page.
/// The interface language, as a dropdown (there can be many, from plugins).
pub fn language_select(ui: &mut Ui, ed: &mut Editor) {
    let languages = crate::i18n::languages(&ed.reg);
    let current = languages.iter().find(|(code, _)| *code == ed.prefs.language).map_or_else(|| ed.prefs.language.clone(), |(_, name)| name.clone());
    let mut picked = None;
    crate::ui::kit::dropdown(ui, egui::Id::unique("settings-language"), 180.0, current, |ui| {
        for (code, name) in &languages {
            if kit::option(ui, ed.prefs.language == *code, name).clicked() {
                picked = Some(code.clone());
            }
        }
    });
    if let Some(code) = picked.filter(|c| *c != ed.prefs.language) {
        ed.prefs.language = code;
        ed.apply_language();
    }
}

fn appearance(ui: &mut Ui, ed: &mut Editor, theme: &Theme, view: &mut SettingsView) {
    let ctx = ui.ctx().clone();
    kit::group(ui, theme, t!("Language"));
    kit::setting_row(ui, theme, t!("Interface language"), t!("Menus and settings; blocks stay in English. More languages come with plugins."), |ui| {
        language_select(ui, ed);
    });
    kit::setting_row(ui, theme, t!("Translate XC Circuit"), t!("Makes a file with every text to translate, saved as a plugin you can share. Changes show while you edit it."), |ui| {
        if crate::ui::kit::button(ui, theme, t!("Create a translation…")).clicked() {
            view.new_translation = Some(Default::default());
        }
    });
    if view.new_translation.is_some() {
        new_translation(ui.ctx(), ed, theme, view);
    }
    ui.add_space(14.0);
    kit::group(ui, theme, t!("Theme"));
    theme_cards(ui, ed, theme);
    let current = ed.theme.id.clone();
    ui.add_space(10.0);

    kit::setting_row(ui, theme, t!("Accent color"), t!("Used for selections, highlights and the open tab. Changes show right away."), |ui| {
        if ed.prefs.accent.is_some() && crate::ui::kit::button(ui, theme, t!("Reset")).on_hover_text(t!("Use the theme's own accent")).clicked() {
            ed.prefs.accent = None;
            ed.restyle();
            ed.theme.apply(&ctx);
        }
        // Every change while dragging in the picker restyles the whole app.
        let mut c = ed.theme.accent;
        if crate::builders::color::color_button(ui, egui::Id::unique("settings-accent"), &mut c, theme, Some(116.0)) {
            ed.prefs.accent = Some(crate::builders::color::to_hex(c));
            ed.restyle();
            ed.theme.apply(&ctx);
        }
    });
    accent_swatches(ui, ed, theme, 16.0);
    ui.add_space(14.0);

    kit::group(ui, theme, t!("Interface"));
    kit::setting_row(ui, theme, t!("Touch mode"), t!("Bigger buttons and more room between them, for touch screens and pens."), |ui| {
        if toggle(ui, theme, &mut ed.prefs.touch) {
            ed.restyle();
            ed.theme.apply(&ctx);
        }
    });
    kit::setting_row(ui, theme, t!("Interface size"), t!("Makes all text and controls bigger or smaller."), |ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        for s in UI_SCALES.iter().rev() {
            let selected = (ed.prefs.ui_scale - s).abs() < 0.01;
            let text = RichText::new(format!("{:.0}%", s * 100.0)).size(12.0).color(if selected { Color32::from_rgb(0, 24, 28) } else { theme.text });
            let b = egui::Button::new(text).min_size(vec2(52.0, crate::ui::kit::input_height(ui)));
            if crate::ui::kit::styled_button(ui, theme, b, if selected { theme.accent } else { theme.field }, Stroke::NONE).clicked() && !selected {
                ed.prefs.ui_scale = *s;
                ctx.set_zoom_factor(*s);
            }
        }
    });

    kit::group(ui, theme, t!("Canvas"));
    kit::setting_row(ui, theme, t!("Grid dots"), t!("The dotted background on canvases."), |ui| {
        toggle(ui, theme, &mut ed.prefs.show_grid);
    });
    kit::setting_row(ui, theme, t!("Snap to grid"), t!("Blocks and notes you drop line up with the grid. The magnet in the canvas toolbar switches it too."), |ui| {
        toggle(ui, theme, &mut ed.prefs.snap_to_grid);
    });
    let theme_wire = ed.reg.theme(&current).map(|d| d.shape.wire_width).unwrap_or(2.0);
    kit::setting_row(ui, theme, t!("Wire thickness"), t!("How thick the wires between blocks are drawn."), |ui| {
        if ed.prefs.wire_width.is_some() && crate::ui::kit::button(ui, theme, t!("Reset")).clicked() {
            ed.prefs.wire_width = None;
            ed.restyle();
        }
        let mut w = ed.prefs.wire_width.unwrap_or(theme_wire);
        if kit::Slider::new(&mut w, 1.0..=5.0).step(0.5).suffix(" px").show(ui, theme) {
            ed.prefs.wire_width = Some(w);
            ed.restyle();
        }
    });
}

/// A small picture of a theme: title bar, sidebar, canvas with a block and a wire.
pub fn theme_card(ui: &mut Ui, t: &Theme, page: &Theme, selected: bool, accent_override: Option<Color32>) -> bool {
    let (rect, resp) = ui.allocate_exact_size(vec2(170.0, 132.0), Sense::click());
    kit::describe_selected(&resp, egui::Role::RadioButton, selected, &t.name);
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

fn editor(ui: &mut Ui, ed: &mut Editor, theme: &Theme, view: &mut SettingsView, asks: &mut Vec<Ask>) {
    let ctx = ui.ctx().clone();
    kit::group(ui, theme, t!("Window"));
    kit::setting_row(ui, theme, t!("Activity bar"), t!("The icons on the far left for Workspaces, Stored Data, Plugins and Settings."), |ui| {
        toggle(ui, theme, &mut ed.prefs.activity_bar);
    });
    kit::setting_row(ui, theme, t!("System title bar"), t!("Use the operating system's title bar instead of XC Circuit's own (with the search box)."), |ui| {
        if toggle(ui, theme, &mut ed.prefs.system_titlebar) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Decorations(ed.prefs.system_titlebar));
        }
    });
    kit::setting_row(ui, theme, t!("Panel layout"), t!("Put every panel back where it started."), |ui| {
        if crate::ui::kit::button(ui, theme, t!("Reset layout")).clicked() {
            asks.push(Ask::ResetLayout);
        }
    });
    kit::setting_row(ui, theme, t!("Save layout and settings after"), t!("XC Circuit saves the panel layout, the open project's place and these settings this long after the last change (and always when it closes). Each change in the meantime starts the wait again."), |ui| {
        let range = crate::editor::prefs::SAVE_AFTER_RANGE;
        kit::Slider::new(&mut ed.prefs.settings_save_after_secs, range).step(1.0).suffix(" s").width(140.0).show(ui, theme);
    });
    kit::setting_row(ui, theme, t!("Accessibility"), t!("Screen readers and the touch keyboard can see what's in the window. If XC Circuit stops answering while a program that reads windows runs (some password managers and writing helpers), try turning this off."), |ui| {
        toggle(ui, theme, &mut ed.prefs.accessibility);
    });
    // For older computers: lighter drawing, and another way to draw.
    let software = crate::editor::prefs::software_drawing();
    // A card, not a row: the list goes under the text.
    crate::ui::kit::setting_card(ui, theme, |ui| {
        use crate::ui::theme::Lighter;
        crate::ui::kit::setting_title(ui, theme, t!("Lighter drawing"), t!("For older computers: tick what to leave out, so there's less to draw."));
        ui.add_space(6.0);
        // Automatic, while drawing in software, has all of them.
        let forced = software && ed.prefs.lighter_auto;
        let mut parts = ed.prefs.lighter(software);
        let mut auto = ed.prefs.lighter_auto;
        let mut changed = false;
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 4.0;
            ui.add_enabled_ui(!forced, |ui| {
                let mut all = parts.all();
                if crate::ui::kit::checkbox_mixed(ui, theme, &mut all, parts.any(), t!("All")).changed() {
                    parts = if all { Lighter::ALL } else { Lighter::NONE };
                    changed = true;
                }
                ui.indent("lighter-parts", |ui| {
                    let rows: [(&mut bool, &str, &str); 4] = [
                        (&mut parts.shadows, t!("No shadows"), t!("Windows, menus and blocks without the shadow under them.")),
                        (&mut parts.animations, t!("No fading"), t!("Menus, tooltips and panels appear and go at once instead of fading.")),
                        (&mut parts.glows, t!("Still glows"), t!("Empty required fields and blocks that just ran glow without pulsing, so the app doesn't draw itself again 30 times a second for them.")),
                        (&mut parts.smoothing, t!("Simpler edges"), t!("Edges without their soft fringe: about half as many triangles to draw, a little more jagged.")),
                    ];
                    for (on, name, tip) in rows {
                        changed |= crate::ui::kit::checkbox(ui, theme, on, name).on_hover_text(tip).changed();
                    }
                });
            });
            ui.add_space(4.0);
            changed |= crate::ui::kit::checkbox(ui, theme, &mut auto, t!("All without a graphics card"))
                .on_hover_text(t!("When the app has to draw in software (no graphics card driver it could use), all of them are left out, whatever is ticked."))
                .changed();
            if forced {
                ui.label(RichText::new(t!("All on now: the app draws without a graphics card.")).size(11.5).color(theme.text_muted));
            }
        });
        if changed {
            if !forced {
                ed.prefs.lighter_parts = parts;
            }
            ed.prefs.lighter_auto = auto;
            ed.restyle();
            ed.theme.apply(&ctx);
        }
    });
    let now = crate::app::graphics::in_use().map(|g| tf!("Now: {graphics}.", graphics = g)).unwrap_or_default();
    let about = format!("{} {now}", t!("How the app draws, from its next start. Automatic tries DirectX 12 or Vulkan first; on an older computer, OpenGL or Older OpenGL can draw with the graphics card where those can't."));
    kit::setting_row(ui, theme, t!("Graphics"), &about, |ui| {
        let chosen = crate::app::graphics::chosen();
        let options = [("auto", t!("Automatic")), ("opengl", t!("OpenGL")), ("glow", t!("Older OpenGL"))];
        let shown = options.iter().find(|(s, _)| *s == chosen).map_or(t!("Automatic"), |(_, n)| *n);
        crate::ui::kit::dropdown(ui, egui::Id::unique("settings-graphics"), 200.0, shown, |ui| {
            for (stage, name) in options {
                if crate::ui::kit::option(ui, chosen == stage, name).clicked() && chosen != stage {
                    match crate::app::graphics::choose(stage) {
                        Ok(()) => ed.log(Level::Info, tf!("The app draws with {graphics} when it starts again.", graphics = name)),
                        Err(e) => ed.log(Level::Warn, tf!("Couldn't keep the graphics choice: {error}", error = e)),
                    }
                }
            }
        });
    });
    console_font(ui, ed, theme, view, asks);
    kit::group(ui, theme, t!("Undo"));
    kit::setting_row(ui, theme, t!("Undo steps"), t!("How many changes each workspace can undo. Every step keeps a copy of the workspace, so fewer steps use less memory on big projects."), |ui| {
        let range = crate::editor::prefs::UNDO_STEPS_RANGE;
        kit::Slider::new(&mut ed.prefs.undo_steps, range).step(10.0).width(140.0).show(ui, theme);
    });
    kit::group(ui, theme, t!("Startup"));
    kit::setting_row(ui, theme, t!("Reopen last project"), t!("Open the project you had open last time. When off, XC Circuit starts with the example."), |ui| {
        toggle(ui, theme, &mut ed.prefs.reopen_last_project);
    });
    kit::setting_row(ui, theme, t!("Look for updates"), t!("At start, ask GitHub whether there is a newer XC Circuit, and say so in the status bar. Help › Check for Updates asks any time."), |ui| {
        toggle(ui, theme, &mut ed.prefs.check_updates);
    });
}

/// The Console's font, picked from the fonts installed on the computer, and its size.
fn console_font(ui: &mut Ui, ed: &mut Editor, theme: &Theme, view: &mut SettingsView, asks: &mut Vec<Ask>) {
    let ctx = ui.ctx().clone();
    // The list is read once, in the background: reading every font takes a moment.
    if view.fonts.is_none() && view.font_list.is_none() {
        let (tx, rx) = std::sync::mpsc::channel();
        let repaint = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(crate::ui::fonts::installed_families());
            repaint.request_repaint();
        });
        view.font_list = Some(rx);
    }
    if let Some(list) = view.font_list.as_ref().and_then(|rx| rx.try_recv().ok()) {
        view.fonts = Some(list);
        view.font_list = None;
    }
    kit::group(ui, theme, t!("Console"));
    let builtin = t!("Built in (monospaced)");
    kit::setting_row(ui, theme, t!("Font"), t!("The Console's text. In a monospaced font its columns line up."), |ui| {
        let shown = ed.prefs.console_font.clone().unwrap_or_else(|| builtin.to_string());
        let mut pick: Option<Option<String>> = None;
        kit::search_dropdown(ui, egui::Id::unique("console-font"), 260.0, shown, |ui| {
            let search = ui.add(crate::ui::inputs::TextLine::new(&mut view.font_filter).hint(t!("Search fonts")));
            // Typing searches right away.
            if ui.memory(|m| m.focused().is_none()) {
                search.request_focus();
            }
            ui.add_space(4.0);
            if kit::option(ui, ed.prefs.console_font.is_none(), builtin).clicked() {
                pick = Some(None);
                ui.close();
            }
            let Some(list) = &view.fonts else {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(RichText::new(t!("Looking for fonts…")).color(theme.text_muted));
                });
                return;
            };
            let filter = view.font_filter.to_lowercase();
            ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                // Monospaced fonts first: the Console's columns line up in them.
                for (mono, title) in [(true, t!("Monospaced")), (false, t!("Other fonts"))] {
                    let names: Vec<&String> = list.iter().filter(|(n, m)| *m == mono && (filter.is_empty() || n.to_lowercase().contains(&filter))).map(|(n, _)| n).collect();
                    if names.is_empty() {
                        continue;
                    }
                    ui.label(RichText::new(title).size(11.0).strong().color(theme.text_muted));
                    for name in names {
                        if kit::option(ui, ed.prefs.console_font.as_deref() == Some(name.as_str()), name).clicked() {
                            pick = Some(Some(name.clone()));
                            ui.close();
                        }
                    }
                }
            });
        });
        if let Some(p) = pick
            && p != ed.prefs.console_font
        {
            ed.prefs.console_font = p;
            asks.push(Ask::ConsoleFont);
        }
    });
    kit::setting_row(ui, theme, t!("Font size"), t!("The Console's text size."), |ui| {
        kit::Number::new(&mut ed.prefs.console_font_size).range(8.0..=32.0).decimals(0).suffix(" pt").show(ui, theme);
        if crate::ui::kit::button(ui, theme, t!("Reset")).clicked() {
            ed.prefs.console_font_size = crate::editor::prefs::CONSOLE_FONT_SIZE;
        }
    });
    kit::setting_row(ui, theme, t!("Line spacing"), t!("Room between the Console's lines."), |ui| {
        kit::Number::new(&mut ed.prefs.console_line_spacing).range(0.0..=16.0).decimals(0).suffix(" pt").show(ui, theme);
        if crate::ui::kit::button(ui, theme, t!("Reset")).clicked() {
            ed.prefs.console_line_spacing = crate::editor::prefs::CONSOLE_LINE_SPACING;
        }
    });
    // What it looks like.
    let font = crate::ui::fonts::console(&ctx, ed.prefs.console_font_size);
    // Two lines, as the Console spaces them.
    let row_h = ui.fonts_mut(|f| f.row_height(&font)) + ed.prefs.console_line_spacing.max(0.0);
    egui::Frame::new().fill(theme.panel_header).corner_radius(6).inner_margin(egui::Margin::symmetric(12, 8)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = 0.0;
        for (level, color) in [("INFO ", theme.accent), ("DONE ", theme.action)] {
            ui.horizontal(|ui| {
                ui.set_height(row_h);
                ui.label(RichText::new("12:00:00").font(font.clone()).color(theme.text_muted));
                ui.label(RichText::new(level).font(font.clone()).color(color));
                ui.add(egui::Label::new(RichText::new(t!("Loaded 6 plugins, 268 blocks, 25 pin types, 3 themes")).font(font.clone()).color(theme.text)).truncate());
            });
        }
    });
}

fn bot(ui: &mut Ui, ed: &mut Editor, theme: &Theme, asks: &mut Vec<Ask>) {
    kit::group(ui, theme, t!("Running"));
    kit::setting_row(ui, theme, t!("Restart on save"), t!("Saving restarts a running bot so it uses your changes."), |ui| {
        toggle(ui, theme, &mut ed.prefs.auto_restart);
    });
    // Asked on a thread of its own: starting node can take long.
    // Once a minute is plenty here (Check again asks at once).
    let node = circuit_runner::node::check_node_every(std::time::Duration::from_secs(60));
    if node.is_none() {
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
    }
    let (status, color) = match &node {
        Some(Ok(v)) => (format!("Node.js {v}"), theme.action),
        Some(Err(e)) => (e.clone(), theme.warning),
        None => (String::new(), theme.text_muted),
    };
    let which = ed.prefs.node_path.clone().unwrap_or_else(|| t!("node from the PATH").into());
    kit::setting_row(ui, theme, "Node.js", &format!("Bots run with Node.js 22 or newer. Using: {which}"), |ui| {
        if crate::ui::kit::button(ui, theme, t!("Set up…")).on_hover_text(t!("Find Node.js on this computer or install it")).clicked() {
            asks.push(Ask::NodeSetup);
            circuit_runner::node::forget_node_check();
        }
        if crate::ui::kit::button(ui, theme, t!("Check again")).clicked() {
            circuit_runner::node::forget_node_check();
        }
        ui.label(RichText::new(status).size(12.0).color(color));
    });
    let packages = crate::bot::app_data_dir().join("packages");
    kit::setting_row(ui, theme, t!("npm packages"), &tf!("Installed once for all bots: {folder}", folder = packages.display()), |ui| {
        if crate::ui::kit::button(ui, theme, t!("Open folder")).clicked() {
            let _ = std::fs::create_dir_all(&packages);
            let _ = open::that(&packages);
        }
    });
    if let Some(p) = &ed.project {
        kit::group(ui, theme, &format!("This project · {}", p.meta.name));
        kit::setting_row(ui, theme, t!("Bot settings"), t!("Token and the other settings of this project's bot."), |ui| {
            if crate::ui::kit::button(ui, theme, t!("Open…")).clicked() {
                asks.push(Ask::BotSettings);
            }
        });
        kit::setting_row(ui, theme, t!("Slash commands"), t!("The commands this bot registers with Discord."), |ui| {
            if crate::ui::kit::button(ui, theme, t!("Edit…")).clicked() {
                asks.push(Ask::SlashCommands);
            }
        });
        kit::setting_row(ui, theme, t!("Export"), t!("Write a folder that runs with node index.js."), |ui| {
            if crate::ui::kit::button(ui, theme, t!("Export…")).clicked() {
                asks.push(Ask::Export);
            }
        });
    }
}

fn backups(ui: &mut Ui, ed: &mut Editor, theme: &Theme, asks: &mut Vec<Ask>) {
    kit::group(ui, theme, t!("Project backups"));
    kit::setting_row(ui, theme, t!("Back up on every save"), t!("Keeps a copy of the project each time you save, outside the project folder."), |ui| {
        toggle(ui, theme, &mut ed.prefs.backups);
    });
    kit::setting_row(ui, theme, t!("Keep the newest"), t!("How many of the most recent backups are always kept."), |ui| {
        kit::Slider::new(&mut ed.prefs.backup_keep, 5..=100).suffix(" backups").width(140.0).show(ui, theme);
    });
    kit::setting_row(ui, theme, t!("And one a day for"), t!("Older backups thin out to the last one of each day."), |ui| {
        kit::Slider::new(&mut ed.prefs.backup_days, 0..=60).suffix(" days").width(140.0).show(ui, theme);
    });
    kit::setting_row(ui, theme, t!("Restore"), t!("See what changed and bring back the project, a workspace or the bot's saved data."), |ui| {
        if crate::ui::kit::button(ui, theme, t!("Restore from backup…")).clicked() {
            asks.push(Ask::OpenBackups);
        }
    });
    let folder = crate::bot::app_data_dir().join("backups");
    kit::setting_row(ui, theme, t!("Backups folder"), &folder.display().to_string(), |ui| {
        if crate::ui::kit::button(ui, theme, t!("Open")).clicked() {
            let _ = std::fs::create_dir_all(&folder);
            let _ = open::that(&folder);
        }
    });
    kit::group(ui, theme, t!("Unsaved changes"));
    kit::setting_row(ui, theme, t!("Crash recovery"), t!("While you have unsaved changes, a copy of them is kept this often. If XC Circuit or your computer stops, you're offered your changes back the next time you open the project."), |ui| {
        let range = crate::editor::prefs::SAVE_SECS_RANGE;
        kit::Slider::new(&mut ed.prefs.recovery_secs, range).step(5.0).suffix(" s").width(140.0).show(ui, theme);
    });
    kit::group(ui, theme, t!("The bot's saved data"));
    kit::setting_row(ui, theme, t!("Backups by the bot"), t!("Every bot backs up its saved data each time it starts and every 6 hours, keeping the newest 10. If the data is ever empty, it asks whether to restore a backup."), |_| {});
}

fn plugins(ui: &mut Ui, ed: &mut Editor, theme: &Theme, asks: &mut Vec<Ask>) {
    kit::group(ui, theme, t!("Plugins"));
    kit::setting_row(ui, theme, t!("Installed plugins"), &tf!("{plugins} plugins with {blocks} blocks and {themes} themes are loaded.", plugins = ed.reg.plugins.len(), blocks = ed.reg.blocks.len(), themes = ed.reg.themes.len()), |ui| {
        if crate::ui::kit::button(ui, theme, t!("Manage…")).clicked() {
            asks.push(Ask::ManagePlugins);
        }
        if crate::ui::kit::button(ui, theme, t!("Reload")).on_hover_text(t!("Load plugin files again")).clicked() {
            asks.push(Ask::ReloadPlugins);
        }
    });
    kit::group(ui, theme, t!("Plugin folders"));
    for dir in ed.plugin_dirs.clone() {
        kit::setting_row(ui, theme, &dir.display().to_string(), if dir.is_dir() { "" } else { t!("Doesn't exist yet") }, |ui| {
            if crate::ui::kit::button(ui, theme, t!("Open")).clicked() {
                let _ = std::fs::create_dir_all(&dir);
                let _ = open::that(&dir);
            }
        });
    }
}

const SHORTCUTS: [(&str, &str); 7] = [
    (n("Ctrl+C / Ctrl+X / Ctrl+V"), n("Copy, cut and paste blocks (your system's keys)")),
    (n("Right-click"), n("Block picker on empty canvas; selects a block or wire (its actions show as buttons)")),
    (n("Shift-drag"), n("Select blocks with a box")),
    (n("Mouse wheel"), n("Zoom the canvas")),
    (n("Drag empty canvas"), n("Pan")),
    (n("Double-click"), n("Rename a workspace or folder in the sidebar")),
    (n("Esc"), n("Close a picker, menu or editor")),
];

/// A shortcut's box, its label centred on the letters themselves (a plain
/// button centres the text's line, which sits high in the monospace font).
fn key_chip(ui: &mut Ui, text: &str, fill: Color32, stroke: Stroke, ink: Color32) -> egui::Response {
    let font = FontId::monospace(12.0);
    let galley = ui.painter().layout_no_wrap(text.to_string(), font.clone(), ink);
    let size = vec2((galley.size().x + 22.0).max(64.0), 24.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    kit::describe(&resp, egui::Role::Button, text);
    let fill = if resp.hovered() { mix(fill, theme_hover(ui), 0.12) } else { fill };
    ui.painter().rect(rect, 5.0, fill, stroke, StrokeKind::Inside);
    // Capitals' height decides the middle, so "Ctrl+," and "F5" sit alike.
    let caps = ui.painter().layout_no_wrap("H".into(), font, ink).mesh_bounds;
    let at = pos2(rect.center().x - galley.size().x / 2.0, rect.center().y - caps.center().y);
    ui.painter().galley(at, galley, ink);
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

/// What a hovered chip lightens towards.
fn theme_hover(ui: &Ui) -> Color32 {
    ui.visuals().widgets.hovered.bg_fill
}

/// Settings › Shortcuts: every action with its shortcuts, changed by clicking
/// one and pressing the new keys.
fn shortcuts(ui: &mut Ui, ed: &mut Editor, theme: &Theme, view: &mut SettingsView) {
    use crate::editor::keymap::{self, Action, PER_ACTION, Shortcut};
    // Recording: the next key (with Ctrl, Shift, Alt) or side button is it; Esc cancels.
    if let Some((action, slot)) = view.recording {
        ed.recording_frame = Some(ui.ctx().cumulative_pass_nr());
        let events = ui.ctx().input_mut(|i| std::mem::take(&mut i.events));
        let pressed = events.iter().find_map(Shortcut::from_event);
        let cancelled = events.iter().any(|e| matches!(e, egui::Event::Key { key: egui::Key::Escape, pressed: true, .. }));
        if let Some(new) = pressed {
            let mut list = keymap::shortcuts(&ed.prefs.shortcuts, action);
            list.retain(|s| *s != new);
            if slot < list.len() {
                list[slot] = new;
            } else {
                list.push(new);
            }
            list.truncate(PER_ACTION);
            let taken = keymap::set(&mut ed.prefs.shortcuts, action, list);
            view.shortcut_note = (!taken.is_empty()).then(|| {
                let names: Vec<&str> = taken.iter().map(|a| a.name()).collect();
                (action, tf!("{shortcut} was taken from: {actions}", shortcut = new.text(), actions = names.join(", ")))
            });
            view.recording = None;
        } else if cancelled {
            view.recording = None;
        }
        ui.ctx().request_repaint();
    }

    ui.label(RichText::new(t!("Click a shortcut, then press the keys you want: any key, with Ctrl, Shift and Alt as you like, or the mouse's side buttons. Esc cancels. Each action can have two.")).size(12.5).color(theme.text_muted));
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.add(crate::ui::kit::TextLine::new(&mut view.shortcut_filter).hint(t!("Search actions or keys")).width(260.0));
        if !ed.prefs.shortcuts.is_empty() && crate::ui::kit::button(ui, theme, t!("Reset all")).on_hover_text(t!("Every shortcut back to how it came")).clicked() {
            ed.prefs.shortcuts.clear();
            view.shortcut_note = None;
        }
    });
    ui.add_space(6.0);

    let filter = view.shortcut_filter.trim().to_lowercase();
    let mut last_group = "";
    for action in Action::ALL {
        let list = keymap::shortcuts(&ed.prefs.shortcuts, action);
        let texts: Vec<String> = list.iter().map(Shortcut::text).collect();
        if !filter.is_empty() && !action.name().to_lowercase().contains(&filter) && !texts.iter().any(|t| t.to_lowercase().contains(&filter)) {
            continue;
        }
        if action.group() != last_group {
            last_group = action.group();
            kit::group(ui, theme, last_group);
            if action.canvas_only() {
                kit::hint(ui, theme, t!("These work while the mouse is over a canvas."));
                ui.add_space(4.0);
            }
        }
        let changed = list != action.defaults();
        let mut start: Option<usize> = None;
        let mut remove: Option<usize> = None;
        let mut reset = false;
        egui::Frame::new().fill(theme.panel_header).stroke(Stroke::new(1.0, theme.line)).corner_radius(8).inner_margin(egui::Margin::symmetric(14, 8)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            // A row as tall as the key boxes, with the name centred on them.
            let row_w = ui.available_width();
            ui.allocate_ui_with_layout(vec2(row_w, 26.0), Layout::left_to_right(Align::Center), |ui| {
                ui.set_min_height(26.0);
                ui.label(RichText::new(action.name()).size(13.0).color(theme.text));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    let back = if action.defaults().is_empty() { t!("Back to no shortcut").to_string() } else { tf!("Back to {keys}", keys = action.defaults().iter().map(Shortcut::text).collect::<Vec<_>>().join(", ")) };
                    if changed && ui.link(RichText::new(t!("Reset")).size(12.0)).on_hover_text(back).clicked() {
                        reset = true;
                    }
                    let recording_here = view.recording.filter(|(a, _)| *a == action).map(|(_, s)| s);
                    // Shown left to right: the shortcuts, then "+".
                    let slots = (list.len() + 1).min(PER_ACTION);
                    for slot in (0..slots).rev() {
                        let text = match (recording_here == Some(slot), list.get(slot)) {
                            (true, _) => {
                                let m = ui.input(|i| i.modifiers);
                                let held: Vec<&str> = [(m.command || m.ctrl, "Ctrl"), (m.shift, "Shift"), (m.alt, "Alt")].into_iter().filter(|(on, _)| *on).map(|(_, n)| n).collect();
                                if held.is_empty() { t!("Press keys…").to_string() } else { format!("{}+…", held.join("+")) }
                            }
                            (false, Some(s)) => s.text(),
                            (false, None) => t!("+ Add").to_string(),
                        };
                        let active = recording_here == Some(slot);
                        let fill = if active { mix(theme.field, theme.accent, 0.3) } else { theme.field };
                        let stroke = if active { Stroke::new(1.5, theme.accent) } else { Stroke::new(1.0, theme.line) };
                        let ink = if list.get(slot).is_some() || active { theme.text } else { theme.text_muted };
                        let resp = key_chip(ui, &text, fill, stroke, ink);
                        let resp = if list.get(slot).is_some() { resp.on_hover_text(t!("Click to change, right-click to remove")) } else { resp.on_hover_text(t!("Add another shortcut")) };
                        if resp.clicked() {
                            start = Some(slot);
                        }
                        if resp.secondary_clicked() && slot < list.len() {
                            remove = Some(slot);
                        }
                    }
                    if list.is_empty() && recording_here.is_none() {
                        ui.label(RichText::new(t!("No shortcut")).size(12.0).color(theme.text_muted));
                    }
                });
            });
            if let Some((a, note)) = &view.shortcut_note
                && *a == action
            {
                ui.label(RichText::new(note).size(11.5).color(theme.warning));
            }
        });
        ui.add_space(6.0);
        if let Some(slot) = start {
            view.recording = Some((action, slot));
            view.shortcut_note = None;
        }
        if let Some(slot) = remove {
            let mut l = list.clone();
            l.remove(slot);
            keymap::set(&mut ed.prefs.shortcuts, action, l);
        }
        if reset {
            let taken = keymap::set(&mut ed.prefs.shortcuts, action, action.defaults());
            view.shortcut_note = (!taken.is_empty()).then(|| (action, tf!("Taken back from: {actions}", actions = taken.iter().map(|a| a.name()).collect::<Vec<_>>().join(", "))));
        }
    }

    kit::group(ui, theme, t!("Mouse and fixed keys"));
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
        ui.add(egui::Image::new(egui::include_image!("../../assets/icon.svg")).fit_to_exact_size(vec2(56.0, 56.0)));
        ui.add_space(8.0);
        ui.vertical(|ui| {
            ui.label(RichText::new(t!("XC™ Circuit")).size(20.0).strong().color(theme.text));
            ui.label(RichText::new(format!("Version {}", env!("CARGO_PKG_VERSION"))).color(theme.text_muted));
            ui.label(RichText::new(t!("Build bots by connecting blocks.")).color(theme.text));
        });
    });
    ui.add_space(14.0);
    kit::group(ui, theme, t!("Where things are kept"));
    let data = crate::bot::app_data_dir();
    for (title, path) in [(t!("App data"), data.clone()), (t!("npm packages"), data.join("packages")), (t!("Plugins you install"), data.join("plugins"))] {
        kit::setting_row(ui, theme, title, &path.display().to_string(), |ui| {
            if crate::ui::kit::button(ui, theme, t!("Open")).clicked() {
                let _ = std::fs::create_dir_all(&path);
                let _ = open::that(&path);
            }
        });
    }
    if let Some(p) = &ed.project {
        let root = p.root.clone();
        kit::setting_row(ui, theme, t!("This project"), &root.display().to_string(), |ui| {
            if crate::ui::kit::button(ui, theme, t!("Open")).clicked() {
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
            let Some(c) = crate::builders::color::parse_hex(hex) else { continue };
            let (r, resp) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::click());
            let selected = crate::builders::color::to_hex(ed.theme.accent) == hex;
            kit::describe_selected(&resp, egui::Role::RadioButton, selected, name);
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
    let close = crate::ui::dialog::show(
        ctx,
        theme,
        crate::ui::dialog::Spec { id: "new-translation", title: t!("Create a translation"), subtitle: Some(t!("Every text is listed in English, ready to fill in")), icon: None, width: 460.0 },
        |ui, part| match part {
            crate::ui::dialog::Part::Body => {
                crate::ui::kit::field(ui, theme, t!("Language code"), Some(t!("Such as fr, es or pt-BR. An existing one (de) starts from its translation.")), |ui| {
                    crate::ui::kit::text_input(ui, code, "fr", false);
                });
                crate::ui::kit::field(ui, theme, t!("Name in that language"), None, |ui| {
                    crate::ui::kit::text_input(ui, name, "Français", false);
                });
                crate::ui::kit::field(ui, theme, t!("Name in English"), None, |ui| {
                    crate::ui::kit::text_input(ui, english, "French", false);
                });
            }
            crate::ui::dialog::Part::Footer => {
                create = ui.add_enabled_ui(!code.trim().is_empty(), |ui| crate::ui::kit::primary_button(ui, theme, t!("Create"))).inner.clicked();
                cancel = crate::ui::kit::secondary_button(ui, theme, t!("Cancel")).clicked();
            }
        },
    );
    if create {
        let dir = crate::bot::app_data_dir().join("plugins");
        let fallback = code.trim().to_string();
        let name = if name.trim().is_empty() { fallback.clone() } else { name.trim().to_string() };
        match crate::i18n::create_translation(&dir, ed, code.trim(), &name, english.trim()) {
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

    /// Lighter drawing's parts tick one by one, or all at once with All,
    /// and the theme follows at once.
    #[test]
    fn lighter_drawing_parts_tick_one_by_one_or_all() {
        use crate::ui::theme::Lighter;
        use egui_kittest::{Harness, kittest::Queryable};
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut ed = Editor::new(vec![root.join("plugins")], None).unwrap();
        ed.prefs.language = crate::i18n::ENGLISH.into();
        ed.apply_language();
        ed.restyle();
        let view = SettingsView { section: Section::Editor, ..Default::default() };
        let mut h = Harness::builder().with_size(vec2(1000.0, 4000.0)).build_ui_state(
            |ui, (ed, view): &mut (Editor, SettingsView)| {
                let mut asks = Vec::new();
                super::ui(ui, ed, view, &mut asks);
            },
            (ed, view),
        );
        h.run();
        h.get_by_label("No shadows").click();
        h.run();
        assert_eq!(h.state().0.prefs.lighter_parts, Lighter { shadows: true, ..Lighter::NONE });
        assert!(h.state().0.theme.lighter.shadows && !h.state().0.theme.lighter.smoothing);
        h.get_by_label("All").click();
        h.run();
        assert_eq!((h.state().0.prefs.lighter_parts, h.state().0.theme.lighter), (Lighter::ALL, Lighter::ALL));
        h.get_by_label("All").click();
        h.run();
        assert_eq!(h.state().0.prefs.lighter_parts, Lighter::NONE);
        h.get_by_label("All without a graphics card").click();
        h.run();
        assert!(!h.state().0.prefs.lighter_auto);
    }

    #[test]
    fn accent_override_follows_theme_switches() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut ed = Editor::new(vec![root.join("plugins")], Some("graphite")).unwrap();
        let light_own = ed.reg.theme("light").map(Theme::from_def).unwrap().accent;
        ed.prefs.accent = Some("#FF5CA8".into());
        ed.restyle();
        assert_eq!(crate::builders::color::to_hex(ed.theme.accent), "#FF5CA8");
        ed.set_theme("light");
        assert_eq!(crate::builders::color::to_hex(ed.theme.accent), "#FF5CA8", "the override stays when switching themes");
        ed.prefs.accent = None;
        ed.restyle();
        assert_eq!(ed.theme.accent, light_own, "Reset brings back the theme's own accent");
    }

    /// The language setting stays put from frame to frame, at any width.
    #[test]
    fn language_row_holds_still() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut ed = Editor::new(vec![root.join("plugins")], Some("graphite")).unwrap();
        let theme = ed.theme.clone();
        for width in [420.0, 560.0, 760.0, 1100.0] {
            let ctx = egui::Context::default();
            let mut places = Vec::new();
            for _ in 0..12 {
                let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, vec2(width, 600.0))), ..Default::default() };
                let mut out = ctx.run_ui(input, |ui| {
                    kit::setting_row(ui, &theme, "Interface language", "Menus and settings; blocks stay in English. More languages come with plugins.", |ui| {
                        let before = ui.min_rect();
                        language_select(ui, &mut ed);
                        places.push(ui.min_rect().union(before));
                    });
                });
                out.textures_delta.clear();
            }
            let last = &places[places.len() - 4..];
            assert!(last.windows(2).all(|w| w[0] == w[1]), "at {width}: {last:?}");
            assert!(last[0].right() <= width, "at {width}: inside the window");
        }
    }
}
