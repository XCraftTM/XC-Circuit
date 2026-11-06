//! The tabs and panels: what each tab shows, where new ones open, and keeping the layout whole.

use super::*;

pub(super) struct Viewer<'a> {
    pub(super) ed: &'a mut Editor,
    /// The tab the user last clicked into, marked with an accent line.
    pub(super) focused_tab: Option<Tab>,
    pub(super) bot: &'a mut BotUi,
    pub(super) bundled_plugins: Option<&'a Path>,
    pub(super) plugins: &'a mut Option<crate::plugins::manager::Draft>,
    pub(super) running: bool,
    pub(super) problem_count: usize,
    pub(super) problems: Vec<panels::ProblemItem>,
    pub(super) native: &'a mut crate::plugins::native::NativeHost,
    pub(super) settings: &'a mut crate::pages::settings::SettingsView,
    pub(super) restore: &'a mut crate::pages::restore::RestoreView,
    pub(super) asks: Vec<crate::pages::settings::Ask>,
    pub(super) start: &'a mut crate::pages::start_page::StartPage,
    pub(super) has_example: bool,
    pub(super) start_pick: Option<crate::pages::start_page::Pick>,
    /// The tab titles' font, for the room left for their icons.
    pub(super) tab_font: egui::FontId,
    /// The Stored Data tab's state.
    pub(super) stored: &'a mut crate::pages::stored::StoredView,
    /// What the Plugins tab's buttons asked for (handled by the app).
    pub(super) side_ask: Option<crate::plugins::manager::SideAsk>,
    /// A tab dropped on the activity bar or the side bar (and where): it
    /// becomes an icon there.
    pub(super) to_side: Option<(Tab, Option<egui::Pos2>)>,
    /// A tab that can go there, being dragged (kept in egui's memory after
    /// the dock is drawn: egui_dock calls on_tab_button with that memory locked).
    pub(super) dragging: Option<Tab>,
}

/// Where a tab being dragged is kept for the activity bar (with the frame it
/// was seen in), so it can show that it takes it.
pub(super) const DRAGGING_TAB: &str = "xc-dragging-tab";

/// The tab being dragged that can go into the activity bar, if one was this
/// frame or the one before.
pub(super) fn dragging_tab(ctx: &egui::Context) -> Option<Tab> {
    let now = ctx.cumulative_frame_nr();
    ctx.data(|d| d.get_temp::<(Tab, u64)>(egui::Id::new(DRAGGING_TAB))).filter(|(_, at)| now.saturating_sub(*at) <= 1).map(|(t, _)| t)
}

/// Room before a tab's title for its icon.
const TAB_ICON_ROOM: f32 = 20.0;

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
            Tab::CanvasBuilder => "palette",
            Tab::SlashCommands => "slash",
            Tab::BotSettings => "bolt",
            Tab::Export | Tab::WorkspaceExport => "package",
            Tab::WorkspaceImport => "folder-plus",
            Tab::Start => "block",
            Tab::Side(view) => view.icon(),
        }
    }
}

/// A tab's title with room before it for the icon. The text keeps the
/// dock's colour (a placeholder it fills in), active or not.
fn with_icon_room(text: String, font: egui::FontId) -> WidgetText {
    let mut job = egui::text::LayoutJob::default();
    job.append(&text, TAB_ICON_ROOM, egui::TextFormat { font_id: font, color: egui::Color32::PLACEHOLDER, ..Default::default() });
    WidgetText::LayoutJob(job.into())
}

/// Paints an icon file (once its texture is loaded) into `rect`.
pub(super) fn paint_icon(ctx: &egui::Context, painter: &egui::Painter, path: &Path, rect: egui::Rect, tint: egui::Color32) {
    let size = egui::load::SizeHint::Size { width: (rect.width() * 2.0) as u32, height: (rect.height() * 2.0) as u32, maintain_aspect_ratio: true };
    if let Ok(egui::load::TexturePoll::Ready { texture }) = ctx.try_load_texture(&crate::editor::file_uri(path), egui::TextureOptions::LINEAR, size) {
        painter.image(texture.id, rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), tint);
    }
}

impl TabViewer for Viewer<'_> {
    /// The start page goes when a project opens, not by its ×.
    fn is_closeable(&self, tab: &Tab) -> bool {
        !matches!(tab, Tab::Start)
    }

    type Tab = Tab;

    fn id(&mut self, tab: &mut Tab) -> Id {
        Id::new(("tab", &*tab))
    }

    fn title(&mut self, tab: &mut Tab) -> WidgetText {
        let text: String = match tab {
            Tab::Inspector => t!("Inspector").into(),
            Tab::Console => t!("Console").into(),
            Tab::Problems => tf!("Problems  {count}", count = self.problem_count),
            Tab::Settings => t!("Settings").into(),
            Tab::Backups => t!("Backups").into(),
            Tab::Plugin(plugin, panel) => self.native.panel_title(plugin, panel).unwrap_or_else(|| panel.clone()),
            Tab::Canvas(key) => self.ed.workspace(key).map(|w| w.name.clone()).unwrap_or_else(|| key.clone()),
            Tab::EmbedBuilder => t!("Embed Builder").into(),
            Tab::LayoutBuilder => t!("Layout Builder").into(),
            Tab::CanvasBuilder => t!("Canvas Builder").into(),
            Tab::SlashCommands => t!("Slash Commands").into(),
            Tab::BotSettings => t!("Bot settings").into(),
            Tab::Export => t!("Export bot").into(),
            Tab::Plugins => t!("Plugins").into(),
            Tab::Start => t!("Start").into(),
            Tab::WorkspaceExport => t!("Export workspaces").into(),
            Tab::WorkspaceImport => t!("Import workspaces").into(),
            Tab::Side(view) => view.name().into(),
        };
        with_icon_room(text, self.tab_font.clone())
    }

    fn on_tab_button(&mut self, tab: &mut Tab, response: &egui::Response) {
        // A tab that can be an icon, dragged onto the activity bar or the side bar, goes there.
        if tab.side_key().is_some() {
            let ctx = &response.ctx;
            // egui_dock draws the dragged tab as another button (its id with
            // "dragged"); the drag itself is the original button's.
            let dragging = response.dragged() || ctx.dragged_id().is_some_and(|d| d == response.id || d.with("dragged") == response.id);
            if dragging {
                self.dragging = Some(tab.clone());
            }
            let pos = ctx.pointer_interact_pos().or_else(|| ctx.pointer_hover_pos());
            let over_side = pos.is_some_and(|p| ["activity-bar", "side-bar"].iter().any(|k| self.ed.ui_rects.get(*k).is_some_and(|r| r.contains(p))));
            if response.drag_stopped() && over_side {
                self.to_side = Some((tab.clone(), pos));
            }
        }
        let r = response.rect;
        let painter = response.ctx.layer_painter(response.layer_id);
        let focused = self.focused_tab.as_ref() == Some(tab);
        if focused {
            let line = egui::Rect::from_min_max(r.left_top(), egui::pos2(r.right(), r.top() + 2.0));
            painter.rect_filled(line, 0.0, self.ed.theme.accent);
        }
        // The icon, in the room the title leaves before its text.
        if let Some(path) = self.ed.reg.icons.get(tab.icon()) {
            let tint = if focused { self.ed.theme.accent } else if response.hovered() { self.ed.theme.text } else { self.ed.theme.text_muted };
            let at = egui::Rect::from_center_size(egui::pos2(r.left() + 8.0 + TAB_ICON_ROOM / 2.0 - 1.0, r.center().y), egui::vec2(14.0, 14.0));
            paint_icon(&response.ctx, &painter, path, at, tint);
        }
    }

    fn on_close(&mut self, tab: &mut Tab) -> OnCloseResponse {
        // Closing an editor's tab cancels it.
        match tab {
            Tab::EmbedBuilder => self.ed.embed_edit = None,
            Tab::LayoutBuilder => self.ed.layout_edit = None,
            Tab::CanvasBuilder => self.ed.canvas_edit = None,
            Tab::SlashCommands => self.ed.slash_edit = None,
            Tab::BotSettings => self.bot.settings = None,
            Tab::Export => self.bot.export = None,
            Tab::Plugins => *self.plugins = None,
            Tab::WorkspaceExport => self.ed.ws_export = None,
            Tab::WorkspaceImport => self.ed.ws_import = None,
            _ => {}
        }
        OnCloseResponse::Close
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Tab) {
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
                if let Some(ask) = side_view_ui(ui, self.ed, self.stored, self.native, self.running, &self.problems, *view) {
                    self.side_ask = Some(ask);
                }
            }
            Tab::Inspector => panels::inspector(ui, self.ed),
            Tab::Console => panels::console(ui, self.ed),
            Tab::Problems => panels::problems(ui, self.ed, self.problems.clone()),
            Tab::Settings => crate::pages::settings::ui(ui, self.ed, self.native, self.settings, &mut self.asks),
            Tab::Backups => crate::pages::restore::ui(ui, self.ed, self.restore, self.running),
            Tab::Plugin(plugin, panel) => {
                let (plugin, panel) = (plugin.clone(), panel.clone());
                self.native.panel_ui(ui, self.ed, &plugin, &panel, self.running);
            }
            Tab::Canvas(key) => {
                let key = key.clone();
                canvas::show(ui, self.ed, &key);
            }
            Tab::Start => {
                let recent = self.ed.prefs.recent_projects.clone();
                if let Some(pick) = crate::pages::start_page::show(ui, &self.ed.theme, self.start, &recent, &self.ed.reg, self.has_example) {
                    self.start_pick = Some(pick);
                }
            }
            Tab::EmbedBuilder => crate::builders::embed::tab(ui, self.ed),
            Tab::LayoutBuilder => crate::builders::layout::tab(ui, self.ed),
            Tab::CanvasBuilder => crate::builders::canvas::tab(ui, self.ed),
            Tab::SlashCommands => crate::builders::slash::tab(ui, self.ed),
            Tab::BotSettings => self.bot.settings_tab(ui, self.ed),
            Tab::Export => self.bot.export_tab(ui, self.ed),
            Tab::WorkspaceExport => crate::editor::exchange::export_tab(ui, self.ed),
            Tab::WorkspaceImport => crate::editor::exchange::import_tab(ui, self.ed),
            Tab::Plugins => {
                use crate::plugins::manager::Ask;
                let theme = self.ed.theme.clone();
                let Some(draft) = self.plugins.as_mut() else { return };
                match crate::plugins::manager::tab(ui, &theme, self.ed, self.native, draft) {
                    Some(Ask::Save(close)) => {
                        let draft = self.plugins.take().unwrap();
                        self.native.allow = draft.allow;
                        self.native.sync(self.ed, self.bundled_plugins);
                        for id in &draft.trust {
                            self.native.trust(id, self.ed);
                        }
                        if !close {
                            *self.plugins = Some(crate::plugins::manager::Draft::new(self.native));
                        }
                    }
                    Some(Ask::Cancel) => *self.plugins = None,
                    None => {}
                }
            }
        }
    }

    fn scroll_bars(&self, _tab: &Tab) -> [bool; 2] {
        [false, false]
    }
}

/// Workspaces, Stored Data or Plugins, as a tab or in the side bar. Returns
/// what the Plugins view's buttons asked for.
pub(super) fn side_view_ui(
    ui: &mut egui::Ui,
    ed: &mut Editor,
    stored: &mut crate::pages::stored::StoredView,
    native: &mut crate::plugins::native::NativeHost,
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
            crate::plugins::manager::side(ui, &theme, ed, native)
        }
    }
}

/// Where the main area is: the whole layout, or (with a column of Workspaces
/// and the like on the left) its right part. New areas for panels split
/// this, not the column.
fn main_part(tree: &Tree<Tab>) -> NodeIndex {
    let root = NodeIndex::root();
    if tree.len() > 2
        && matches!(tree[root], Node::Horizontal(_))
        && matches!(&tree[root.left()], Node::Leaf(leaf) if leaf.tabs().iter().all(|t| t.side() == Side::Left))
    {
        root.right()
    } else {
        root
    }
}


/// Gives a change in the dock's size (`grow`) to the canvases, so the panels
/// beside and below them (Inspector, Console …) keep their size. Splits are
/// fractions, so on their own every part would grow and shrink with the dock.
pub(super) fn keep_panel_sizes(tree: &mut Tree<Tab>, index: NodeIndex, grow: egui::Vec2) {
    if index.0 >= tree.len() {
        return;
    }
    let (horizontal, rect, fraction) = match &tree[index] {
        Node::Horizontal(s) => (true, s.rect, s.fraction),
        Node::Vertical(s) => (false, s.rect, s.fraction),
        _ => return,
    };
    let (size, along) = if horizontal { (rect.width(), grow.x) } else { (rect.height(), grow.y) };
    let (left, right) = (index.left(), index.right());
    // The side with the canvases takes it all; otherwise both share it.
    let to_left = match (has_center(tree, left), has_center(tree, right)) {
        (true, false) => along,
        (false, true) => 0.0,
        _ => along * fraction,
    };
    if along != 0.0 && size.is_finite() && size > 1.0 {
        let f = ((fraction * size + to_left) / (size + along)).clamp(0.05, 0.95);
        if let Node::Horizontal(s) | Node::Vertical(s) = &mut tree[index] {
            s.fraction = f;
        }
    }
    let (grow_left, grow_right) = if horizontal {
        (egui::vec2(to_left, grow.y), egui::vec2(along - to_left, grow.y))
    } else {
        (egui::vec2(grow.x, to_left), egui::vec2(grow.x, along - to_left))
    };
    keep_panel_sizes(tree, left, grow_left);
    keep_panel_sizes(tree, right, grow_right);
}

/// Whether a part of the layout holds canvases (or other middle tabs).
pub(super) fn has_center(tree: &Tree<Tab>, index: NodeIndex) -> bool {
    if index.0 >= tree.len() {
        return false;
    }
    match &tree[index] {
        Node::Leaf(leaf) => leaf.tabs().iter().any(|t| t.side() == Side::Center),
        Node::Horizontal(_) | Node::Vertical(_) => has_center(tree, index.left()) || has_center(tree, index.right()),
        Node::Empty => false,
    }
}

/// The tour's reduced window: canvas, Inspector and Console only.
pub(super) fn tour_layout() -> DockState<Tab> {
    let mut dock = DockState::new(vec![Tab::Console]);
    let surface = dock.main_surface_mut();
    let [_bottom, top] = surface.split_above(NodeIndex::root(), 0.74, vec![Tab::Canvas(crate::pages::tour::WORKSPACE.into())]);
    surface.split_right(top, 0.72, vec![Tab::Inspector]);
    dock
}

pub(super) fn default_layout() -> DockState<Tab> {
    let mut dock = DockState::new(vec![Tab::Console, Tab::Problems]);
    let surface = dock.main_surface_mut();
    // Root starts as the bottom panel; the top area holds (canvases) | Inspector.
    // The side bar, outside the dock, has Workspaces. A split's fraction is
    // the share of its left/top child.
    surface.split_above(NodeIndex::root(), 0.76, vec![Tab::Inspector]);
    dock
}

/// Opens a tab in a window of its own over the middle of the app. It can be
/// docked from there like any other tab.
pub(super) fn open_floating(dock: &mut DockState<Tab>, ctx: &egui::Context, tab: Tab, size: egui::Vec2) {
    let area = ctx.content_rect();
    let size = size.min(area.size() - egui::vec2(48.0, 48.0)).max(egui::vec2(320.0, 240.0));
    let surface = dock.add_window(vec![tab]);
    if let Some(w) = dock.get_window_state_mut(surface) {
        w.set_position(area.center() - size / 2.0).set_size(size);
    }
}

/// Opens a side panel next to the open tabs of its kind (the bottom panels
/// together, Inspector with plugin panels, …). Only when none is open does it
/// get a new area on its side.
pub(super) fn place_panel(dock: &mut DockState<Tab>, tab: Tab) {
    let side = tab.side();
    if side == Side::Center {
        return open_center(dock, tab);
    }
    let group = dock.find_tab_from(|t| t.side() == side).map(|p| p.node_path());
    match group {
        Some(np) if dock.leaf_mut(np).is_ok() => {
            if let Ok(leaf) = dock.leaf_mut(np) {
                leaf.append_tab(tab.clone());
            }
        }
        _ if dock.main_surface().is_empty() => *dock.main_surface_mut() = Tree::new(vec![tab.clone()]),
        _ => {
            let surface = dock.main_surface_mut();
            let main = main_part(surface);
            match side {
                Side::Left => {
                    surface.split_left(NodeIndex::root(), 0.22, vec![tab.clone()]);
                }
                Side::Right => {
                    surface.split_right(main, 0.78, vec![tab.clone()]);
                }
                _ => {
                    surface.split_below(main, 0.75, vec![tab.clone()]);
                }
            }
        }
    }
    if let Some(path) = dock.find_tab(&tab) {
        let _ = dock.set_active_tab(path);
        dock.set_focused_node_and_surface(path.node_path());
    }
}

/// Closes every tab that matches, one at a time. (egui_dock's `retain_tabs`
/// can break the layout: when an area empties and its neighbour is split,
/// it moves only the neighbour's top node and leaves its parts behind.)
pub(super) fn close_tabs(dock: &mut DockState<Tab>, matches: impl Fn(&Tab) -> bool) {
    while let Some(path) = dock.find_tab_from(&matches) {
        dock.remove_tab(path);
    }
}

/// Whether a layout tree is whole: every split has two halves, every part
/// hangs off a split, and every area has a tab.
pub(super) fn tree_ok(tree: &Tree<Tab>) -> bool {
    let nodes: Vec<_> = tree.iter().collect();
    let at = |i: NodeIndex| nodes.get(i.0).copied();
    nodes.iter().enumerate().all(|(i, node)| {
        let index = NodeIndex(i);
        node.is_empty()
            || (index.parent().is_none_or(|p| at(p).is_some_and(|p| p.is_parent()))
                && (!node.is_parent() || (at(index.left()).is_some_and(|n| !n.is_empty()) && at(index.right()).is_some_and(|n| !n.is_empty())))
                && (!node.is_leaf() || node.tabs_count() > 0))
    })
}

/// Rebuilds a broken layout from the default one, with the same tabs open.
/// Returns whether it had to.
pub(super) fn repair_layout(dock: &mut DockState<Tab>) -> bool {
    if dock.iter_surfaces().all(|s| s.node_tree().is_none_or(tree_ok)) {
        return false;
    }
    let tabs: Vec<Tab> = dock.iter_all_tabs().map(|(_, t)| t.clone()).filter(|t| !t.floats()).collect();
    let mut fresh = default_layout();
    close_tabs(&mut fresh, |t| !tabs.contains(t));
    for tab in tabs {
        if fresh.find_tab(&tab).is_none() {
            place_panel(&mut fresh, tab);
        }
    }
    *dock = fresh;
    true
}

/// Focuses a workspace's canvas tab, or opens it next to the other canvases.
pub(super) fn open_canvas(dock: &mut DockState<Tab>, key: String) {
    open_center(dock, Tab::Canvas(key));
}

/// Takes the canvases out of a layout but keeps their area: in each area that
/// has some, the first becomes a Start tab, which the next canvas to open
/// joins (and which then goes). Closing them all would close the area too,
/// and the next canvas would squeeze in next to the Inspector.
pub(super) fn clear_canvases(dock: &mut DockState<Tab>) {
    while let Some(path) = dock.find_tab_from(|t| matches!(t, Tab::Canvas(_))) {
        let has_start = dock.leaf(path.node_path()).is_ok_and(|l| l.tabs().contains(&Tab::Start));
        if has_start {
            dock.remove_tab(path);
        } else if let Ok(leaf) = dock.leaf_mut(path.node_path()) {
            leaf.tabs_mut()[path.tab.0] = Tab::Start;
        }
    }
}

/// Focuses a tab of the main area (canvases, Settings), or opens it there.
pub(super) fn open_center(dock: &mut DockState<Tab>, tab: Tab) {
    if let Some(path) = dock.find_tab(&tab) {
        let _ = dock.set_active_tab(path);
        dock.set_focused_node_and_surface(path.node_path());
        return;
    }
    let middle = dock.find_tab_from(|t| matches!(t, Tab::Canvas(_) | Tab::Start));
    let target = middle.map(|p| p.node_path()).or_else(|| dock.find_tab(&Tab::Inspector).map(|p| p.node_path()));
    match target {
        Some(np) if middle.is_some() => {
            if let Ok(leaf) = dock.leaf_mut(np) {
                leaf.append_tab(tab.clone());
            }
        }
        Some(np) => {
            // No canvas yet: split the area left of the Inspector.
            let surface = dock.main_surface_mut();
            surface.split_left(np.node, 0.78, vec![tab.clone()]);
        }
        None => dock.push_to_first_leaf(tab.clone()),
    }
    if let Some(path) = dock.find_tab(&tab) {
        let _ = dock.set_active_tab(path);
    }
}
