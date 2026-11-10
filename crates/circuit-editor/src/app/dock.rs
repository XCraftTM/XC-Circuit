//! The tabs and panels: what each tab shows, and where new ones open.

use super::*;

pub(super) struct Viewer<'a> {
    pub(super) ed: &'a mut Editor,
    pub(super) bot: &'a mut BotUi,
    pub(super) running: bool,
    pub(super) problem_count: usize,
    pub(super) problems: Vec<panels::ProblemItem>,
    pub(super) plugin_host: &'a mut crate::plugins::host::PluginHost,
    pub(super) settings: &'a mut crate::pages::settings::SettingsView,
    pub(super) restore: &'a mut crate::pages::restore::RestoreView,
    pub(super) asks: Vec<crate::pages::settings::Ask>,
    /// The Stored Data tab's state.
    pub(super) stored: &'a mut crate::pages::stored::StoredView,
    /// What the Plugins tab's buttons asked for (handled by the app).
    pub(super) side_ask: Option<crate::plugins::manager::SideAsk>,
    /// A tab dropped on the activity bar or the side bar (and where), or
    /// moved there by its menu: it becomes an icon there.
    pub(super) to_side: Option<(Tab, Option<egui::Pos2>)>,
    /// Workspaces, Stored Data or Plugins whose tab was closed: back to the activity bar.
    pub(super) closed_side: Vec<SideView>,
    /// How many workspace tabs are open: closing the last leaves a placeholder.
    pub(super) canvases: usize,
    /// What the placeholder for a workspace asked for.
    pub(super) no_workspace: Option<NoWorkspace>,
}

/// The placeholder's buttons, in the canvas area while no workspace is open.
pub(super) enum NoWorkspace {
    Open(String),
    New,
}

impl Tab {
    /// The icon shown before the tab's title.
    pub(super) fn icon(&self) -> &'static str {
        match self {
            Tab::Canvas(_) => "workspace",
            Tab::Inspector => "pointer",
            Tab::Console => "terminal",
            Tab::Problems => "alert",
            Tab::Settings => "settings",
            Tab::Backups => "clock",
            Tab::Plugin(..) | Tab::Plugins => "plug",
            Tab::EmbedBuilder => "embed",
            Tab::LayoutBuilder => "layers",
            Tab::ModalBuilder => "text",
            Tab::CanvasBuilder => "palette",
            Tab::SlashCommands => "slash",
            Tab::BotSettings => "bolt",
            Tab::Export | Tab::WorkspaceExport => "package",
            Tab::Hosting => "server",
            Tab::Git => "git",
            Tab::Host => "server",
            Tab::Live => "user-plus",
            Tab::ServerConsole => "terminal",
            Tab::WorkspaceImport => "folder-plus",
            Tab::Start => "block",
            Tab::Side(view) => view.icon(),
        }
    }
}

impl DockViewer<Tab> for Viewer<'_> {
    /// The start page goes when a project opens, not by its ×.
    fn closeable(&self, tab: &Tab) -> bool {
        !matches!(tab, Tab::Start)
    }

    /// A builder's window stays wide enough for it (and tall enough for its
    /// buttons and some of what's above them).
    fn min_size(&self, tab: &Tab) -> egui::Vec2 {
        match tab.min_width() {
            0.0 => egui::Vec2::ZERO,
            w => egui::vec2(w, 360.0),
        }
    }

    fn system_titlebar(&self) -> bool {
        self.ed.prefs.system_titlebar
    }

    fn icon(&self, tab: &Tab) -> Option<PathBuf> {
        self.ed.reg.icons.get(tab.icon()).cloned()
    }

    fn title(&mut self, tab: &Tab) -> String {
        match tab {
            Tab::Inspector => t!("Inspector").into(),
            Tab::Console => t!("Console").into(),
            Tab::Problems => tf!("Problems  {count}", count = self.problem_count),
            Tab::Settings => t!("Settings").into(),
            Tab::Backups => t!("Backups").into(),
            Tab::Plugin(plugin, panel) => self.plugin_host.panel_title(plugin, panel).unwrap_or_else(|| panel.clone()),
            Tab::Canvas(key) => self.ed.workspace(key).map(|w| w.name.clone()).unwrap_or_else(|| key.clone()),
            Tab::EmbedBuilder => t!("Embed Builder").into(),
            Tab::LayoutBuilder => t!("Layout Builder").into(),
            Tab::ModalBuilder => t!("Modal Builder").into(),
            Tab::CanvasBuilder => t!("Canvas Builder").into(),
            Tab::SlashCommands => t!("Slash Commands").into(),
            Tab::BotSettings => t!("Bot settings").into(),
            Tab::Export => t!("Export bot").into(),
            Tab::Hosting => t!("Remote").into(),
            Tab::Git => "Git".into(),
            Tab::Host => "Host".into(),
            Tab::Live => t!("Live session").into(),
            Tab::ServerConsole => match &self.bot.hosting.console {
                Some(c) => tf!("Console of {server}", server = c.name),
                None => t!("Remote console").into(),
            },
            Tab::Plugins => t!("Plugins").into(),
            // With a project open it holds the place of its workspaces.
            Tab::Start if self.ed.project.is_some() => t!("No workspace").into(),
            Tab::Start => t!("Start").into(),
            Tab::WorkspaceExport => t!("Export workspaces").into(),
            Tab::WorkspaceImport => t!("Import workspaces").into(),
            Tab::Side(view) => view.name().into(),
        }
    }

    /// One that can be an icon in the activity bar: put it there.
    fn tab_menu(&mut self, ui: &mut egui::Ui, tab: &Tab) {
        if tab.side_key().is_some() && crate::ui::kit::menu_item(ui, t!("Move to the activity bar"), None, true).clicked() {
            self.to_side = Some((tab.clone(), None));
            ui.close();
        }
    }

    /// Let go of over the activity bar or the side bar: it becomes an icon there.
    fn take_drop(&mut self, tab: &Tab, pos: egui::Pos2) -> bool {
        let over = ["activity-bar", "side-bar"].iter().any(|k| self.ed.ui_rects.get(*k).is_some_and(|r| r.contains(pos)));
        if over && tab.side_key().is_some() {
            self.to_side = Some((tab.clone(), Some(pos)));
        }
        over && tab.side_key().is_some()
    }

    fn on_close(&mut self, tab: &Tab) -> Option<Tab> {
        // The last workspace's tab leaves a placeholder in its place, so the
        // area stays (and the Inspector doesn't take the whole window).
        if matches!(tab, Tab::Canvas(_)) && self.canvases <= 1 && self.ed.project.is_some() {
            return Some(Tab::Start);
        }
        // Closing an editor's tab cancels it.
        match tab {
            Tab::EmbedBuilder => self.ed.embed_edit = None,
            Tab::LayoutBuilder => self.ed.layout_edit = None,
            Tab::ModalBuilder => self.ed.modal_edit = None,
            Tab::CanvasBuilder => self.ed.canvas_edit = None,
            Tab::SlashCommands => self.ed.slash_edit = None,
            Tab::BotSettings => self.bot.settings = None,
            Tab::Export => self.bot.export = None,
            Tab::Hosting => self.bot.hosting.draft = None,
            Tab::Git => self.bot.git.open = false,
            Tab::Host => self.bot.host.open = false,
            Tab::Live => self.bot.live.open = false,
            Tab::ServerConsole => self.bot.hosting.console = None,
            Tab::WorkspaceExport => self.ed.ws_export = None,
            Tab::WorkspaceImport => self.ed.ws_import = None,
            Tab::Side(view) => self.closed_side.push(*view),
            _ => {}
        }
        None
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &Tab) {
        crate::app::watchdog::mark(format!("in the {} tab", self.title(tab)));
        let name = match tab {
            Tab::Canvas(_) => Some("canvas"),
            Tab::Inspector => Some("inspector"),
            Tab::Console => Some("console"),
            _ => None,
        };
        if let Some(name) = name {
            self.ed.ui_rects.insert(name.into(), ui.max_rect());
        }
        match tab {
            Tab::Side(view) => {
                if let Some(ask) = side_view_ui(ui, self.ed, self.stored, self.plugin_host, self.running, &self.problems, *view) {
                    self.side_ask = Some(ask);
                }
            }
            Tab::Inspector => panels::inspector(ui, self.ed),
            Tab::Console => panels::console(ui, self.ed),
            Tab::Problems => panels::problems(ui, self.ed, self.problems.clone()),
            Tab::Settings => crate::pages::settings::ui(ui, self.ed, self.settings, &mut self.asks),
            Tab::Backups => crate::pages::restore::ui(ui, self.ed, self.restore, self.running),
            Tab::Plugin(plugin, panel) => {
                let (plugin, panel) = (plugin.clone(), panel.clone());
                self.plugin_host.panel_ui(ui, self.ed, &plugin, &panel, self.running);
            }
            Tab::Canvas(key) => {
                let key = key.clone();
                canvas::show(ui, self.ed, &key);
            }
            // Without a project the start window shows, not the tabs.
            Tab::Start => self.no_workspace = no_workspace(ui, self.ed),
            // The builders keep a size they work at; a smaller tab or window scrolls.
            Tab::EmbedBuilder => at_least(ui, tab.min_width(), |ui| crate::builders::embed::tab(ui, self.ed)),
            Tab::LayoutBuilder => at_least(ui, tab.min_width(), |ui| crate::builders::layout::tab(ui, self.ed)),
            Tab::ModalBuilder => at_least(ui, tab.min_width(), |ui| crate::builders::modal::tab(ui, self.ed)),
            Tab::CanvasBuilder => at_least(ui, tab.min_width(), |ui| crate::builders::canvas::tab(ui, self.ed)),
            Tab::SlashCommands => at_least(ui, tab.min_width(), |ui| crate::builders::slash::tab(ui, self.ed)),
            Tab::BotSettings => self.bot.settings_tab(ui, self.ed),
            Tab::Export => self.bot.export_tab(ui, self.ed),
            Tab::Hosting => self.bot.hosting.tab(ui, self.ed),
            Tab::Git => self.bot.git.tab(ui, self.ed),
            Tab::Host => self.bot.host.tab(ui, self.ed),
            Tab::Live => self.bot.live.tab(ui, self.ed),
            Tab::ServerConsole => {
                if let Some(c) = &mut self.bot.hosting.console {
                    c.tab(ui, self.ed);
                }
            }
            Tab::WorkspaceExport => crate::editor::exchange::export_tab(ui, self.ed),
            Tab::WorkspaceImport => crate::editor::exchange::import_tab(ui, self.ed),
            // Only in layouts saved when plugins had a tab of their own: closed on load.
            Tab::Plugins => {}
        }
    }
}

impl Tab {
    /// The narrowest a builder works at (with its preview on top of the
    /// form): a narrower area scrolls sideways instead of squeezing it, and
    /// a window can't get narrower.
    pub(super) fn min_width(&self) -> f32 {
        match self {
            Tab::EmbedBuilder => 540.0,
            Tab::LayoutBuilder => 640.0,
            Tab::ModalBuilder => 620.0,
            Tab::CanvasBuilder => 620.0,
            Tab::SlashCommands => 600.0,
            _ => 0.0,
        }
    }
}

/// Draws `add` at least `min_width` wide: a narrower tab scrolls sideways
/// instead of squeezing it. The height is the tab's, so what's in it keeps
/// its own scrolling (up and down, with the wheel) and its buttons at the
/// bottom in view.
fn at_least(ui: &mut egui::Ui, min_width: f32, add: impl FnOnce(&mut egui::Ui)) {
    // A few points short (a window's frame, at its smallest) doesn't scroll.
    if ui.available_width() + 16.0 >= min_width {
        return add(ui);
    }
    // A solid bar, so it's plain that there's more (floating ones only show near the pointer).
    let scroll = ui.spacing().scroll;
    ui.spacing_mut().scroll = egui::style::ScrollStyle::solid();
    egui::ScrollArea::horizontal().id_salt("at-least").auto_shrink([false, false]).show(ui, |ui| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(min_width, ui.available_height()), egui::Sense::hover());
        let mut inner = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(egui::Layout::top_down(egui::Align::Min)));
        inner.set_clip_rect(rect.intersect(ui.clip_rect()));
        inner.spacing_mut().scroll = scroll;
        add(&mut inner);
    });
}

/// Workspaces, Stored Data or Plugins, as a tab or in the side bar. Returns
/// what the Plugins view's buttons asked for.
pub(super) fn side_view_ui(
    ui: &mut egui::Ui,
    ed: &mut Editor,
    stored: &mut crate::pages::stored::StoredView,
    plugin_host: &mut crate::plugins::host::PluginHost,
    running: bool,
    problems: &[panels::ProblemItem],
    view: SideView,
) -> Option<crate::plugins::manager::SideAsk> {
    match view {
        SideView::Workspaces => {
            // The Problems panel's entries, counted per workspace.
            let mut counts = std::collections::HashMap::new();
            for (_, _, at) in problems {
                if let Some((ws, _)) = at {
                    *counts.entry(ws.clone()).or_insert(0) += 1;
                }
            }
            crate::pages::sidebar::panel(ui, ed, &counts);
            None
        }
        SideView::StoredData => {
            crate::pages::stored::panel(ui, ed, stored, running);
            None
        }
        SideView::Plugins => {
            let theme = ed.theme.clone();
            crate::plugins::manager::side(ui, &theme, ed, plugin_host)
        }
    }
}

/// Where the main area is: the whole layout, or (with a column of Workspaces
/// and the like on the left) its right part. New areas for panels split
/// this, not the column.
fn main_part(main: &Node<Tab>) -> Vec<bool> {
    match main {
        Node::Split(s) if s.dir == Dir::Horizontal && matches!(&*s.first, Node::Leaf(l) if l.tabs.iter().all(|t| t.side() == Side::Left)) => vec![true],
        _ => Vec::new(),
    }
}

/// Gives a change in the dock's size (`grow`) to the canvases, so the panels
/// beside and below them (Inspector, Console …) keep their size. Splits are
/// fractions, so on their own every part would grow and shrink with the dock.
pub(super) fn keep_panel_sizes(node: &mut Node<Tab>, grow: egui::Vec2) {
    let Node::Split(s) = node else { return };
    let horizontal = s.dir == Dir::Horizontal;
    let (size, along) = if horizontal { (s.rect.width(), grow.x) } else { (s.rect.height(), grow.y) };
    // The side with the canvases takes it all; otherwise both share it.
    let to_first = match (has_center(&s.first), has_center(&s.second)) {
        (true, false) => along,
        (false, true) => 0.0,
        _ => along * s.fraction,
    };
    if along != 0.0 && size.is_finite() && size > 1.0 {
        s.fraction = ((s.fraction * size + to_first) / (size + along)).clamp(0.05, 0.95);
    }
    let (grow_first, grow_second) = if horizontal {
        (egui::vec2(to_first, grow.y), egui::vec2(along - to_first, grow.y))
    } else {
        (egui::vec2(grow.x, to_first), egui::vec2(grow.x, along - to_first))
    };
    keep_panel_sizes(&mut s.first, grow_first);
    keep_panel_sizes(&mut s.second, grow_second);
}

/// Whether a part of the layout holds canvases (or other middle tabs).
pub(super) fn has_center(node: &Node<Tab>) -> bool {
    node.leaves().iter().any(|l| l.tabs.iter().any(|t| t.side() == Side::Center))
}

/// The tour's reduced window: canvas, Inspector and Console only.
pub(super) fn tour_layout() -> Dock<Tab> {
    let mut dock = Dock::new(vec![Tab::Console]);
    if let Some(main) = &mut dock.main {
        main.split(Edge::Top, 0.74, vec![Tab::Canvas(crate::pages::tour::WORKSPACE.into())]);
        if let Some(top) = main.at_mut(&[false]) {
            top.split(Edge::Right, 0.72, vec![Tab::Inspector]);
        }
    }
    dock
}

pub(super) fn default_layout() -> Dock<Tab> {
    let mut dock = Dock::new(vec![Tab::Console, Tab::Problems]);
    // The bottom panels, and above them (canvases) | Inspector. A split's
    // fraction is the share of its first (left or top) part.
    if let Some(main) = &mut dock.main {
        main.split(Edge::Top, 0.76, vec![Tab::Inspector]);
    }
    dock
}

/// Opens a side panel next to the open tabs of its kind (the bottom panels
/// together, Inspector with plugin panels, …). Only when none is open does it
/// get a new area on its side.
pub(super) fn place_panel(dock: &mut Dock<Tab>, tab: Tab) {
    let side = tab.side();
    if side == Side::Center {
        return open_center(dock, tab);
    }
    let group = dock.main.as_ref().and_then(|m| m.leaves().into_iter().find_map(|l| l.tabs.iter().find(|t| t.side() == side).cloned()));
    match (group, &mut dock.main) {
        (Some(group), _) => {
            if let Some(leaf) = dock.leaf_with(&group) {
                leaf.push(tab.clone());
            }
        }
        (None, None) => dock.main = Some(Node::leaf(vec![tab.clone()])),
        (None, Some(main)) => {
            let path = main_part(main);
            match side {
                Side::Left => main.split(Edge::Left, 0.22, vec![tab.clone()]),
                Side::Right => {
                    if let Some(part) = main.at_mut(&path) {
                        part.split(Edge::Right, 0.78, vec![tab.clone()]);
                    }
                }
                _ => {
                    if let Some(part) = main.at_mut(&path) {
                        part.split(Edge::Bottom, 0.75, vec![tab.clone()]);
                    }
                }
            }
        }
    }
    dock.focus(&tab);
}

/// Closes every tab that matches.
pub(super) fn close_tabs(dock: &mut Dock<Tab>, matches: impl Fn(&Tab) -> bool) {
    dock.remove_where(matches);
}

/// Takes the areas without tabs out of a saved layout. Returns whether it had to.
pub(super) fn repair_layout(dock: &mut Dock<Tab>) -> bool {
    if dock.main.as_ref().is_none_or(|m| m.whole()) {
        return false;
    }
    dock.main = dock.main.take().and_then(crate::ui::dock::prune);
    true
}

/// Where a workspace's canvas goes while none is open: what to do, the
/// project's workspaces to open, and a button for a new one.
fn no_workspace(ui: &mut egui::Ui, ed: &Editor) -> Option<NoWorkspace> {
    let theme = &ed.theme;
    let project = ed.project.as_ref()?;
    let mut ask = None;
    ui.painter().rect_filled(ui.max_rect(), 0.0, theme.panel);
    egui::ScrollArea::vertical().id_salt("no-workspace").auto_shrink([false, false]).show(ui, |ui| {
        let width = ui.available_width().min(380.0);
        ui.add_space((ui.available_height() * 0.18).clamp(24.0, 140.0));
        ui.horizontal(|ui| {
            ui.add_space(((ui.available_width() - width) / 2.0).max(16.0));
            ui.vertical(|ui| {
                ui.set_width(width);
                if let Some(icon) = ed.reg.icons.get("workspace") {
                    let (rect, _) = ui.allocate_exact_size(egui::vec2(44.0, 44.0), egui::Sense::hover());
                    ui.painter().rect_filled(rect, 10.0, crate::ui::theme::with_alpha(theme.accent, 34));
                    egui::Image::from_uri(crate::editor::file_uri(icon)).tint(theme.accent).paint_at(ui, rect.shrink(11.0));
                    ui.add_space(10.0);
                }
                ui.label(RichText::new(t!("No workspace open")).size(17.0).strong().color(theme.text));
                ui.add_space(2.0);
                let hint = if project.workspaces.is_empty() { t!("Make a workspace to put blocks in.") } else { t!("Open a workspace to edit its blocks here, or make a new one.") };
                ui.label(RichText::new(hint).size(12.5).color(theme.text_muted));
                ui.add_space(14.0);
                for (key, ws) in project.workspaces.iter().take(12) {
                    let text = RichText::new(&ws.name).color(theme.text);
                    if ui.add_sized([width, 32.0], egui::Button::new(text).fill(theme.panel_header)).on_hover_text(t!("Open this workspace")).clicked() {
                        ask = Some(NoWorkspace::Open(key.clone()));
                    }
                }
                if project.workspaces.len() > 12 {
                    ui.label(RichText::new(tf!("And {count} more in Workspaces.", count = project.workspaces.len() - 12)).size(12.0).color(theme.text_muted));
                }
                ui.add_space(10.0);
                if crate::ui::kit::primary_button(ui, theme, t!("+ New workspace")).clicked() {
                    ask = Some(NoWorkspace::New);
                }
            });
        });
    });
    ask
}

/// Focuses a workspace's canvas tab, or opens it next to the other canvases.
pub(super) fn open_canvas(dock: &mut Dock<Tab>, key: String) {
    open_center(dock, Tab::Canvas(key));
}

/// Takes the canvases out of a layout but keeps their area: in each area that
/// has some, the first becomes a Start tab, which the next canvas to open
/// joins (and which then goes). Closing them all would close the area too,
/// and the next canvas would squeeze in next to the Inspector.
pub(super) fn clear_canvases(dock: &mut Dock<Tab>) {
    while let Some(canvas) = dock.find(|t| matches!(t, Tab::Canvas(_))).cloned() {
        if dock.leaf_with(&canvas).is_some_and(|l| l.tabs.contains(&Tab::Start)) {
            dock.remove(&canvas);
        } else {
            dock.replace(&canvas, Tab::Start);
        }
    }
}

/// Focuses a tab of the main area (canvases, Settings), or opens it there.
pub(super) fn open_center(dock: &mut Dock<Tab>, tab: Tab) {
    if dock.contains(&tab) {
        dock.focus(&tab);
        return;
    }
    match dock.find(|t| matches!(t, Tab::Canvas(_) | Tab::Start)).cloned() {
        Some(middle) => {
            if let Some(leaf) = dock.leaf_with(&middle) {
                leaf.push(tab.clone());
            }
        }
        // No canvas yet: split the area left of the Inspector.
        None => match dock.part_with(&Tab::Inspector) {
            Some(part) => part.split(Edge::Left, 0.78, vec![tab.clone()]),
            None => dock.push_to_first(tab.clone()),
        },
    }
    dock.focus(&tab);
}
