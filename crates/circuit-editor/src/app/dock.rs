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
        match tab {
            Tab::Inspector => t!("Inspector").into(),
            Tab::Console => t!("Console").into(),
            Tab::Problems => tf!("Problems  {count}", count = self.problem_count).into(),
            Tab::Settings => t!("Settings").into(),
            Tab::Backups => t!("Backups").into(),
            Tab::Plugin(plugin, panel) => self.native.panel_title(plugin, panel).unwrap_or_else(|| panel.clone()).into(),
            Tab::Canvas(key) => self.ed.workspace(key).map(|w| w.name.clone()).unwrap_or_else(|| key.clone()).into(),
            Tab::EmbedBuilder => t!("Embed Builder").into(),
            Tab::LayoutBuilder => t!("Layout Builder").into(),
            Tab::SlashCommands => t!("Slash Commands").into(),
            Tab::BotSettings => t!("Bot settings").into(),
            Tab::Export => t!("Export bot").into(),
            Tab::Plugins => t!("Plugins").into(),
            Tab::Start => t!("Start").into(),
            Tab::WorkspaceExport => t!("Export workspaces").into(),
            Tab::WorkspaceImport => t!("Import workspaces").into(),
        }
    }

    fn on_tab_button(&mut self, tab: &mut Tab, response: &egui::Response) {
        if self.focused_tab.as_ref() == Some(tab) {
            let r = response.rect;
            let line = egui::Rect::from_min_max(r.left_top(), egui::pos2(r.right(), r.top() + 2.0));
            response.ctx.layer_painter(response.layer_id).rect_filled(line, 0.0, self.ed.theme.accent);
        }
    }

    fn on_close(&mut self, tab: &mut Tab) -> OnCloseResponse {
        // Closing an editor's tab cancels it.
        match tab {
            Tab::EmbedBuilder => self.ed.embed_edit = None,
            Tab::LayoutBuilder => self.ed.layout_edit = None,
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
            Tab::Inspector => panels::inspector(ui, self.ed),
            Tab::Console => panels::console(ui, self.ed),
            Tab::Problems => panels::problems(ui, self.ed, std::mem::take(&mut self.problems)),
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
            match side {
                Side::Right => {
                    surface.split_right(NodeIndex::root(), 0.78, vec![tab.clone()]);
                }
                _ => {
                    surface.split_below(NodeIndex::root(), 0.75, vec![tab.clone()]);
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
