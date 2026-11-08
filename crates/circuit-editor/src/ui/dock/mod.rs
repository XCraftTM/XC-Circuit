//! The dock: the main area's tabs, in areas split side by side or one above
//! the other, with separators to drag, and windows floating over it. Tabs
//! are dragged between areas, into a new area split off one side of
//! another, into a window of their own, or (through the viewer) out of the
//! dock altogether.
//!
//! The layout is a tree: an area ([`Leaf`]) holds tabs, a [`Split`] holds
//! two parts and how much of its room the first one gets. The tabs are the
//! app's own type; each is in the dock at most once.

mod show;

use eframe::egui::{self, Pos2, Rect};
use serde::{Deserialize, Serialize};

pub use show::{DockViewer, paint_drop, paint_ghost};

/// Which way a split's two parts lie.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Dir {
    /// Side by side: the first part on the left.
    Horizontal,
    /// One above the other: the first part on top.
    Vertical,
}

/// A side of an area.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    Left,
    Right,
    Top,
    Bottom,
}

/// An area of tabs. Its places on screen are those of the frame drawn last.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Leaf<T> {
    pub tabs: Vec<T>,
    /// The tab in front.
    pub active: usize,
    /// The whole area, tab bar and all.
    #[serde(skip, default = "nothing")]
    pub rect: Rect,
    /// Below the tab bar: where the tab in front shows.
    #[serde(skip, default = "nothing")]
    pub body: Rect,
    /// How far the tab bar is scrolled, when its tabs don't fit.
    #[serde(skip)]
    scroll: f32,
    /// Scroll the tab in front into view, next time it's drawn.
    #[serde(skip)]
    reveal: bool,
}

/// Two parts of the layout, and how they share the room.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Split<T> {
    pub dir: Dir,
    /// The share of the first part (the left or top one).
    pub fraction: f32,
    pub first: Box<Node<T>>,
    pub second: Box<Node<T>>,
    /// Where it was drawn last.
    #[serde(skip, default = "nothing")]
    pub rect: Rect,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Node<T> {
    Leaf(Leaf<T>),
    Split(Split<T>),
}

fn nothing() -> Rect {
    Rect::NOTHING
}

/// A window floating over the dock, with its own layout.
#[derive(Clone, Debug)]
pub struct Window<T> {
    pub id: u64,
    pub node: Node<T>,
    /// Where it is (its top left corner) and how big, as drawn last.
    pub pos: Pos2,
    pub size: egui::Vec2,
    /// Where to move it next frame (it was dragged by its tab bar).
    move_to: Option<Pos2>,
    /// Where it goes and how big it gets next frame (its top or left edge
    /// is being dragged; egui resizes windows it moves itself only).
    resize_to: Option<Rect>,
    /// While its top or left edge is dragged: where it was, and where the drag started.
    edge_drag: Option<(Rect, Pos2)>,
    /// Its layer, to tell which window is on top.
    layer: Option<egui::LayerId>,
}

/// Where something dropped on the dock goes.
#[derive(Clone, Debug, PartialEq)]
pub enum Target<T> {
    /// Into the tab bar of the area with `anchor`, at `index`.
    Tabs { anchor: T, index: usize },
    /// Next to the tabs of the area with this tab.
    Join(T),
    /// Into a new area split off this side of the area with this tab.
    Split(T, Edge),
}

/// A place to drop, and what to show while hovering it.
#[derive(Clone, Debug, PartialEq)]
pub struct Drop<T> {
    pub target: Target<T>,
    /// What it would take up, or (`line`) a thin mark where it would go.
    pub preview: Rect,
    pub line: bool,
}

/// Where an area was drawn, for finding what's under the pointer.
#[derive(Clone, Debug)]
pub struct Place<T> {
    pub tabs: Vec<T>,
    pub rect: Rect,
    pub bar: Rect,
    pub body: Rect,
    /// Each tab's button.
    pub tab_rects: Vec<Rect>,
    pub layer: egui::LayerId,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(bound(serialize = "T: Serialize", deserialize = "T: Deserialize<'de>"))]
pub struct Dock<T> {
    /// The main area; `None` when it has no tabs.
    pub main: Option<Node<T>>,
    /// Windows aren't kept: they hold editors whose drafts aren't either.
    #[serde(skip)]
    pub windows: Vec<Window<T>>,
    /// The tab the user last clicked into.
    #[serde(skip)]
    focused: Option<T>,
    /// A tab being dragged.
    #[serde(skip)]
    drag: Option<T>,
    /// Where each area was drawn last.
    #[serde(skip)]
    places: Vec<Place<T>>,
    #[serde(skip)]
    next_window: u64,
}

impl<T> Leaf<T> {
    pub fn new(tabs: Vec<T>) -> Self {
        Leaf { tabs, active: 0, rect: Rect::NOTHING, body: Rect::NOTHING, scroll: 0.0, reveal: false }
    }

    pub fn active_tab(&self) -> Option<&T> {
        self.tabs.get(self.active)
    }
}

impl<T: Clone + PartialEq> Leaf<T> {
    /// Adds a tab at the end, in front.
    pub fn push(&mut self, tab: T) {
        self.tabs.push(tab);
        self.active = self.tabs.len() - 1;
        self.reveal = true;
    }

    fn remove(&mut self, tab: &T) -> bool {
        let Some(i) = self.tabs.iter().position(|t| t == tab) else { return false };
        self.tabs.remove(i);
        if i < self.active || self.active >= self.tabs.len() {
            self.active = self.active.saturating_sub(1);
        }
        true
    }
}

impl<T: Clone + PartialEq> Node<T> {
    pub fn leaf(tabs: Vec<T>) -> Self {
        Node::Leaf(Leaf::new(tabs))
    }

    /// Splits this part: `tabs` get a new area on the `edge` side, and
    /// `fraction` is the share of the first part (the left or top one).
    pub fn split(&mut self, edge: Edge, fraction: f32, tabs: Vec<T>) {
        let old = std::mem::replace(self, Node::leaf(Vec::new()));
        let new = Node::leaf(tabs);
        let (dir, first, second) = match edge {
            Edge::Left => (Dir::Horizontal, new, old),
            Edge::Right => (Dir::Horizontal, old, new),
            Edge::Top => (Dir::Vertical, new, old),
            Edge::Bottom => (Dir::Vertical, old, new),
        };
        *self = Node::Split(Split { dir, fraction: fraction.clamp(0.05, 0.95), first: Box::new(first), second: Box::new(second), rect: Rect::NOTHING });
    }

    /// The part at `path` (false: first, true: second; empty: this one).
    #[cfg(test)]
    pub fn at(&self, path: &[bool]) -> Option<&Node<T>> {
        match (path.split_first(), self) {
            (None, _) => Some(self),
            (Some((second, rest)), Node::Split(s)) => if *second { &s.second } else { &s.first }.at(rest),
            _ => None,
        }
    }

    pub fn at_mut(&mut self, path: &[bool]) -> Option<&mut Node<T>> {
        match (path.split_first(), self) {
            (None, node) => Some(node),
            (Some((second, rest)), Node::Split(s)) => if *second { &mut s.second } else { &mut s.first }.at_mut(rest),
            _ => None,
        }
    }

    /// The areas, first to last.
    pub fn leaves(&self) -> Vec<&Leaf<T>> {
        match self {
            Node::Leaf(l) => vec![l],
            Node::Split(s) => {
                let mut all = s.first.leaves();
                all.extend(s.second.leaves());
                all
            }
        }
    }

    pub fn leaves_mut(&mut self) -> Vec<&mut Leaf<T>> {
        match self {
            Node::Leaf(l) => vec![l],
            Node::Split(s) => {
                let mut all = s.first.leaves_mut();
                all.extend(s.second.leaves_mut());
                all
            }
        }
    }

    /// The area (as a part, so it can be split) that has `tab`.
    pub fn part_with(&mut self, tab: &T) -> Option<&mut Node<T>> {
        if matches!(self, Node::Leaf(l) if l.tabs.contains(tab)) {
            return Some(self);
        }
        match self {
            Node::Split(s) => {
                if let Some(n) = s.first.part_with(tab) {
                    return Some(n);
                }
                s.second.part_with(tab)
            }
            Node::Leaf(_) => None,
        }
    }

    pub fn contains(&self, tab: &T) -> bool {
        self.leaves().iter().any(|l| l.tabs.contains(tab))
    }

    /// Whether every area has a tab.
    pub fn whole(&self) -> bool {
        self.leaves().iter().all(|l| !l.tabs.is_empty())
    }
}

/// Takes out the areas without tabs: the other part of their split gets
/// all its room. `None` when nothing's left.
pub fn prune<T>(node: Node<T>) -> Option<Node<T>> {
    match node {
        Node::Leaf(l) => (!l.tabs.is_empty()).then_some(Node::Leaf(l)),
        Node::Split(Split { dir, fraction, first, second, rect }) => match (prune(*first), prune(*second)) {
            (Some(first), Some(second)) => Some(Node::Split(Split { dir, fraction, first: Box::new(first), second: Box::new(second), rect })),
            (Some(one), None) | (None, Some(one)) => Some(one),
            (None, None) => None,
        },
    }
}

impl<T: Clone + PartialEq> Default for Dock<T> {
    fn default() -> Self {
        Dock { main: None, windows: Vec::new(), focused: None, drag: None, places: Vec::new(), next_window: 1 }
    }
}

impl<T: Clone + PartialEq> Dock<T> {
    /// A dock with one area of these tabs.
    pub fn new(tabs: Vec<T>) -> Self {
        Dock { main: (!tabs.is_empty()).then(|| Node::leaf(tabs)), ..Default::default() }
    }

    /// Whether the main area has no tabs.
    pub fn is_empty(&self) -> bool {
        self.main.is_none()
    }

    /// The main area and every window's layout.
    fn nodes(&self) -> impl Iterator<Item = &Node<T>> {
        self.main.iter().chain(self.windows.iter().map(|w| &w.node))
    }

    fn nodes_mut(&mut self) -> impl Iterator<Item = &mut Node<T>> {
        self.main.iter_mut().chain(self.windows.iter_mut().map(|w| &mut w.node))
    }

    /// Every tab, the main area's first.
    pub fn tabs(&self) -> impl Iterator<Item = &T> {
        self.nodes().flat_map(|n| n.leaves().into_iter().flat_map(|l| l.tabs.iter()))
    }

    pub fn contains(&self, tab: &T) -> bool {
        self.nodes().any(|n| n.contains(tab))
    }

    /// The first tab that matches.
    pub fn find(&self, matches: impl Fn(&T) -> bool) -> Option<&T> {
        self.tabs().find(|t| matches(t))
    }

    /// The area that has `tab`.
    pub fn leaf_with(&mut self, tab: &T) -> Option<&mut Leaf<T>> {
        self.nodes_mut().find_map(|n| match n.part_with(tab)? {
            Node::Leaf(l) => Some(l),
            Node::Split(_) => None,
        })
    }

    /// The area (as a part, so it can be split) that has `tab`.
    pub fn part_with(&mut self, tab: &T) -> Option<&mut Node<T>> {
        self.nodes_mut().find_map(|n| n.part_with(tab))
    }

    /// Whether `tab` is in front in its area.
    pub fn is_active(&self, tab: &T) -> bool {
        self.nodes().any(|n| n.leaves().iter().any(|l| l.active_tab() == Some(tab)))
    }

    /// Brings a tab to the front of its area. In the area the user is in,
    /// it becomes the tab they're in.
    pub fn activate(&mut self, tab: &T) {
        let focused = self.focused.clone();
        if let Some(leaf) = self.leaf_with(tab)
            && let Some(i) = leaf.tabs.iter().position(|t| t == tab)
        {
            leaf.active = i;
            leaf.reveal = true;
            if focused.is_some_and(|f| leaf.tabs.contains(&f)) {
                self.focused = Some(tab.clone());
            }
        }
    }

    /// Brings a tab to the front and makes it the one the user is in.
    pub fn focus(&mut self, tab: &T) {
        if self.contains(tab) {
            self.activate(tab);
            self.focused = Some(tab.clone());
        }
    }

    /// The tab the user is in: the one last clicked into, while it's there
    /// and in front (not behind another tab of its area).
    pub fn focused(&self) -> Option<&T> {
        self.focused.as_ref().filter(|t| self.is_active(t))
    }

    /// The tab being dragged.
    pub fn dragged(&self) -> Option<&T> {
        self.drag.as_ref()
    }

    /// Takes a tab out. An area left without tabs goes, and so does a window.
    pub fn remove(&mut self, tab: &T) -> bool {
        let mut found = false;
        if let Some(main) = &mut self.main
            && main.leaves_mut().into_iter().any(|l| l.remove(tab))
        {
            found = true;
            self.main = self.main.take().and_then(prune);
        }
        for w in &mut self.windows {
            if !found && w.node.leaves_mut().into_iter().any(|l| l.remove(tab)) {
                found = true;
            }
        }
        if found {
            self.windows.retain_mut(|w| match prune(w.node.clone()) {
                Some(node) => {
                    w.node = node;
                    true
                }
                None => false,
            });
        }
        found
    }

    /// Takes out every tab that matches.
    pub fn remove_where(&mut self, matches: impl Fn(&T) -> bool) {
        while let Some(tab) = self.find(&matches).cloned() {
            self.remove(&tab);
        }
    }

    /// Puts `new` where `old` is.
    pub fn replace(&mut self, old: &T, new: T) {
        if let Some(leaf) = self.leaf_with(old)
            && let Some(i) = leaf.tabs.iter().position(|t| t == old)
        {
            leaf.tabs[i] = new;
        }
    }

    /// Adds a tab to the main area's first area (the main area, when empty).
    pub fn push_to_first(&mut self, tab: T) {
        match &mut self.main {
            Some(main) => main.leaves_mut()[0].push(tab),
            None => self.main = Some(Node::leaf(vec![tab])),
        }
    }

    /// Opens tabs in a window of their own.
    pub fn add_window(&mut self, tabs: Vec<T>, rect: Rect) {
        let id = self.next_window;
        self.next_window += 1;
        self.windows.push(Window { id, node: Node::leaf(tabs), pos: rect.min, size: rect.size(), move_to: None, resize_to: None, edge_drag: None, layer: None });
    }

    /// Where something let go of at `pos` would go (`dragged`: a tab of the
    /// dock being moved). `top` is the layer under the pointer: a window
    /// over the main area takes it.
    pub fn target_at(&self, pos: Pos2, dragged: Option<&T>, top: Option<egui::LayerId>) -> Option<Drop<T>> {
        let under: Vec<&Place<T>> = self.places.iter().filter(|p| p.rect.contains(pos)).collect();
        let place = under.iter().rev().find(|p| Some(p.layer) == top).or(under.last())?;
        // The area is known by a tab of its own that isn't being dragged.
        let anchor = place.tabs.iter().find(|t| Some(*t) != dragged)?.clone();
        let from = dragged.and_then(|d| place.tabs.iter().position(|t| t == d));
        if place.bar.contains(pos) {
            let index = place.tab_rects.iter().filter(|r| r.center().x < pos.x).count();
            if from.is_some_and(|f| index == f || index == f + 1) {
                return None;
            }
            let x = match place.tab_rects.get(index) {
                Some(r) => r.left(),
                None => place.tab_rects.last().map_or(place.bar.left() + 2.0, |r| r.right()),
            };
            let x = x.clamp(place.bar.left() + 1.0, place.bar.right() - 1.0);
            let preview = Rect::from_min_max(egui::pos2(x - 1.0, place.bar.top() + 4.0), egui::pos2(x + 1.0, place.bar.bottom() - 4.0));
            return Some(Drop { target: Target::Tabs { anchor, index }, preview, line: true });
        }
        let body = place.body;
        if !body.contains(pos) {
            return None;
        }
        let join = (from.is_none()).then(|| Drop { target: Target::Join(anchor.clone()), preview: body, line: false });
        if body.width() < 2.0 || body.height() < 2.0 {
            return join;
        }
        let x = (pos.x - body.left()) / body.width();
        let y = (pos.y - body.top()) / body.height();
        // The edge it's nearest to, in parts of the area; the middle half joins.
        let (near, edge) = [(x, Edge::Left), (1.0 - x, Edge::Right), (y, Edge::Top), (1.0 - y, Edge::Bottom)].into_iter().min_by(|a, b| a.0.total_cmp(&b.0))?;
        if near >= 0.25 {
            return join;
        }
        let c = body.center();
        let preview = match edge {
            Edge::Left => Rect::from_min_max(body.min, egui::pos2(c.x, body.max.y)),
            Edge::Right => Rect::from_min_max(egui::pos2(c.x, body.min.y), body.max),
            Edge::Top => Rect::from_min_max(body.min, egui::pos2(body.max.x, c.y)),
            Edge::Bottom => Rect::from_min_max(egui::pos2(body.min.x, c.y), body.max),
        };
        Some(Drop { target: Target::Split(anchor, edge), preview, line: false })
    }

    /// Whether `pos` is over an area of the dock (or one of its windows).
    pub fn over(&self, pos: Pos2) -> bool {
        self.places.iter().any(|p| p.rect.contains(pos))
    }

    /// Puts a tab that isn't in the dock where a drop says, in front.
    pub fn put(&mut self, tab: T, target: &Target<T>) {
        match target {
            Target::Tabs { anchor, index } => {
                if let Some(leaf) = self.leaf_with(anchor) {
                    let at = (*index).min(leaf.tabs.len());
                    leaf.tabs.insert(at, tab.clone());
                }
            }
            Target::Join(anchor) => {
                if let Some(leaf) = self.leaf_with(anchor) {
                    leaf.push(tab.clone());
                }
            }
            Target::Split(anchor, edge) => {
                if let Some(part) = self.part_with(anchor) {
                    part.split(*edge, 0.5, vec![tab.clone()]);
                }
            }
        }
        if !self.contains(&tab) {
            self.push_to_first(tab.clone());
        }
        self.focus(&tab);
    }

    /// Moves a tab of the dock where a drop says.
    pub fn move_tab(&mut self, tab: &T, target: &Target<T>) {
        let mut target = target.clone();
        // In its own tab bar, the places after it move up one when it goes.
        if let Target::Tabs { anchor, index } = &mut target
            && let Some(leaf) = self.leaf_with(anchor)
            && let Some(from) = leaf.tabs.iter().position(|t| t == tab)
            && from < *index
        {
            *index -= 1;
        }
        if self.remove(tab) {
            self.put(tab.clone(), &target);
        }
    }

    /// Moves a tab into a window of its own at `rect` (a window that has
    /// only it just moves there). Not the main area's last tab.
    pub fn float(&mut self, tab: &T, rect: Rect) {
        if let Some(w) = self.windows.iter_mut().find(|w| w.node.leaves().iter().map(|l| l.tabs.len()).sum::<usize>() == 1 && w.node.contains(tab)) {
            w.move_to = Some(rect.min);
            return;
        }
        let last = self.main.as_ref().is_some_and(|m| m.leaves().iter().map(|l| l.tabs.len()).sum::<usize>() == 1 && m.contains(tab));
        if last || !self.remove(tab) {
            return;
        }
        self.add_window(vec![tab.clone()], rect);
        self.focus(tab);
    }

    /// The size of the area that has `tab`, as drawn last.
    pub fn size_of(&self, tab: &T) -> Option<egui::Vec2> {
        self.places.iter().find(|p| p.tabs.contains(tab)).map(|p| p.rect.size())
    }
}

/// Layouts saved by egui_dock (before the dock was the app's own), read
/// once and turned into a [`Dock`]: the main area's tree, without windows.
pub mod legacy {
    use super::*;

    #[derive(Deserialize)]
    pub struct DockState<T> {
        surfaces: Vec<Surface<T>>,
    }

    #[derive(Deserialize)]
    enum Surface<T> {
        Empty,
        Main(Tree<T>),
        Window(serde::de::IgnoredAny, serde::de::IgnoredAny),
    }

    #[derive(Deserialize)]
    struct Tree<T> {
        nodes: Vec<Node<T>>,
    }

    /// A binary tree in a list: a node's parts are at 2i+1 and 2i+2.
    #[derive(Deserialize)]
    enum Node<T> {
        Empty,
        Leaf(LeafNode<T>),
        Vertical(SplitNode),
        Horizontal(SplitNode),
    }

    #[derive(Deserialize)]
    struct LeafNode<T> {
        tabs: Vec<T>,
        active: Index,
    }

    #[derive(Deserialize)]
    struct Index(usize);

    #[derive(Deserialize)]
    struct SplitNode {
        fraction: f32,
    }

    impl<T: Clone + PartialEq> DockState<T> {
        pub fn convert(self) -> super::Dock<T> {
            let main = self.surfaces.into_iter().find_map(|s| match s {
                Surface::Main(tree) => Some(tree),
                Surface::Empty | Surface::Window(..) => None,
            });
            let main = main.and_then(|mut tree| node(&mut tree.nodes, 0));
            super::Dock { main, ..Default::default() }
        }
    }

    fn node<T: Clone + PartialEq>(nodes: &mut [Node<T>], i: usize) -> Option<super::Node<T>> {
        let taken = std::mem::replace(nodes.get_mut(i)?, Node::Empty);
        let (dir, fraction) = match taken {
            Node::Empty => return None,
            Node::Leaf(l) => {
                let mut leaf = super::Leaf::new(l.tabs);
                leaf.active = l.active.0.min(leaf.tabs.len().saturating_sub(1));
                return prune(super::Node::Leaf(leaf));
            }
            Node::Horizontal(s) => (Dir::Horizontal, s.fraction),
            Node::Vertical(s) => (Dir::Vertical, s.fraction),
        };
        let (first, second) = (node(nodes, 2 * i + 1), node(nodes, 2 * i + 2));
        match (first, second) {
            (Some(first), Some(second)) => Some(super::Node::Split(Split { dir, fraction: fraction.clamp(0.05, 0.95), first: Box::new(first), second: Box::new(second), rect: Rect::NOTHING })),
            (Some(one), None) | (None, Some(one)) => Some(one),
            (None, None) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tabs(dock: &Dock<&'static str>) -> Vec<&'static str> {
        dock.tabs().copied().collect()
    }

    fn areas(dock: &Dock<&'static str>) -> Vec<Vec<&'static str>> {
        dock.main.iter().flat_map(|m| m.leaves()).map(|l| l.tabs.clone()).collect()
    }

    /// A window's left and top edges resize it too; its right and bottom edges stay.
    #[test]
    fn windows_resize_at_their_left_and_top_edges() {
        struct Viewer;
        impl DockViewer<&'static str> for Viewer {
            fn title(&mut self, tab: &&'static str) -> String {
                tab.to_string()
            }
            fn icon(&self, _: &&'static str) -> Option<std::path::PathBuf> {
                None
            }
            fn ui(&mut self, _: &mut egui::Ui, _: &&'static str) {}
        }
        let mut dock = Dock::new(vec!["canvas"]);
        dock.add_window(vec!["layout"], Rect::from_min_size(egui::pos2(300.0, 200.0), egui::vec2(500.0, 400.0)));
        let ctx = egui::Context::default();
        let theme = crate::ui::theme::Theme::fallback();
        let mut frame = |dock: &mut Dock<&'static str>, events: Vec<egui::Event>| {
            let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 900.0))), events, ..Default::default() };
            let mut out = ctx.run_ui(raw, |ui| dock.show(ui, &theme, &mut Viewer));
            out.textures_delta.clear();
        };
        for _ in 0..3 {
            frame(&mut dock, vec![]);
        }
        let rect = |dock: &Dock<&'static str>| Rect::from_min_size(dock.windows[0].pos, dock.windows[0].size);
        let before = rect(&dock);
        let drag = |frame: &mut dyn FnMut(&mut Dock<&'static str>, Vec<egui::Event>), dock: &mut Dock<&'static str>, from: egui::Pos2, by: egui::Vec2| {
            let press = |pos, pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
            frame(dock, vec![egui::Event::PointerMoved(from)]);
            frame(dock, vec![press(from, true)]);
            for i in 1..=6 {
                frame(dock, vec![egui::Event::PointerMoved(from + by * (i as f32 / 6.0))]);
            }
            frame(dock, vec![press(from + by, false)]);
            for _ in 0..3 {
                frame(dock, vec![]);
            }
        };
        // The left edge, 60 px further left.
        drag(&mut frame, &mut dock, egui::pos2(before.left() + 3.0, before.center().y), egui::vec2(-60.0, 0.0));
        let after = rect(&dock);
        assert!((after.left() - (before.left() - 60.0)).abs() < 3.0 && (after.right() - before.right()).abs() < 3.0, "{before:?} to {after:?}");
        // The top edge, 40 px down.
        drag(&mut frame, &mut dock, egui::pos2(after.center().x, after.top() + 3.0), egui::vec2(0.0, 40.0));
        let last = rect(&dock);
        assert!((last.top() - (after.top() + 40.0)).abs() < 3.0 && (last.bottom() - after.bottom()).abs() < 3.0, "{after:?} to {last:?}");
    }

    #[test]
    fn splits_put_the_new_area_on_its_side() {
        let mut dock = Dock::new(vec!["console"]);
        let main = dock.main.as_mut().unwrap();
        main.split(Edge::Top, 0.76, vec!["inspector"]);
        assert_eq!(areas(&dock), [vec!["inspector"], vec!["console"]], "on top, first");
        let main = dock.main.as_mut().unwrap();
        let Node::Split(s) = main else { panic!() };
        assert_eq!((s.dir, s.fraction), (Dir::Vertical, 0.76), "the first part's share");
        main.at_mut(&[false]).unwrap().split(Edge::Left, 0.8, vec!["canvas"]);
        assert_eq!(areas(&dock), [vec!["canvas"], vec!["inspector"], vec!["console"]]);
        assert!(dock.main.as_ref().unwrap().at(&[false, true]).is_some_and(|n| n.contains(&"inspector")));
    }

    #[test]
    fn an_area_left_without_tabs_goes() {
        let mut dock = Dock::new(vec!["console", "problems"]);
        dock.main.as_mut().unwrap().split(Edge::Top, 0.7, vec!["inspector"]);
        dock.main.as_mut().unwrap().at_mut(&[false]).unwrap().split(Edge::Left, 0.8, vec!["canvas"]);
        // Closing the bottom area: the other part (itself split) takes its room, whole.
        dock.remove_where(|t| *t == "console" || *t == "problems");
        assert_eq!(areas(&dock), [vec!["canvas"], vec!["inspector"]]);
        assert!(dock.main.as_ref().unwrap().whole());
        dock.remove(&"canvas");
        dock.remove(&"inspector");
        assert!(dock.is_empty());
        dock.push_to_first("start");
        assert_eq!(tabs(&dock), ["start"]);
    }

    /// A tab put behind another one isn't the one the user is in any more:
    /// the one brought to the front in its area is.
    #[test]
    fn focus_follows_the_tab_in_front() {
        let mut dock = Dock::new(vec!["settings", "canvas"]);
        dock.main.as_mut().unwrap().split(Edge::Right, 0.8, vec!["inspector"]);
        dock.focus(&"settings");
        dock.activate(&"canvas");
        assert_eq!(dock.focused(), Some(&"canvas"), "same area: the focus moves along");
        dock.focus(&"settings");
        dock.activate(&"inspector");
        assert_eq!(dock.focused(), Some(&"settings"), "another area: it stays");
        dock.leaf_with(&"canvas").unwrap().active = 1;
        assert_eq!(dock.focused(), None, "behind another tab: not the one the user is in");
    }

    #[test]
    fn the_tab_in_front_stays_when_others_go() {
        let mut dock = Dock::new(vec!["a", "b", "c"]);
        dock.focus(&"c");
        dock.remove(&"a");
        assert!(dock.is_active(&"c"));
        dock.remove(&"c");
        assert!(dock.is_active(&"b"), "the one before it, when it goes");
        assert_eq!(dock.focused(), None, "a tab that's gone isn't the one the user is in");
    }

    /// Two areas drawn side by side: a 0..300 and b 300..400, each with a 24
    /// high tab bar and tabs 60 wide.
    fn drawn() -> Dock<&'static str> {
        let mut dock = Dock::new(vec!["a1", "a2"]);
        dock.main.as_mut().unwrap().split(Edge::Right, 0.75, vec!["b1"]);
        let layer = egui::LayerId::background();
        for (tabs, x, w) in [(vec!["a1", "a2"], 0.0, 300.0), (vec!["b1"], 300.0, 100.0)] {
            let rect = Rect::from_min_size(egui::pos2(x, 0.0), egui::vec2(w, 200.0));
            let bar = Rect::from_min_size(rect.min, egui::vec2(w, 24.0));
            let tab_rects = (0..tabs.len()).map(|i| Rect::from_min_size(egui::pos2(x + 60.0 * i as f32, 0.0), egui::vec2(60.0, 24.0))).collect();
            dock.places.push(Place { tabs, rect, bar, body: Rect::from_min_max(egui::pos2(x, 24.0), rect.max), tab_rects, layer });
        }
        dock
    }

    #[test]
    fn what_is_under_the_pointer_takes_the_drop() {
        let dock = drawn();
        let at = |x: f32, y: f32, dragged: Option<&'static str>| dock.target_at(egui::pos2(x, y), dragged.as_ref(), None).map(|d| d.target);
        assert_eq!(at(150.0, 110.0, None), Some(Target::Join("a1")), "the middle joins");
        assert_eq!(at(10.0, 110.0, None), Some(Target::Split("a1", Edge::Left)));
        assert_eq!(at(150.0, 195.0, None), Some(Target::Split("a1", Edge::Bottom)));
        assert_eq!(at(130.0, 10.0, None), Some(Target::Tabs { anchor: "a1", index: 2 }), "after the last tab");
        assert_eq!(at(20.0, 10.0, None), Some(Target::Tabs { anchor: "a1", index: 0 }));
        assert_eq!(at(500.0, 100.0, None), None, "outside");
        // Its own area: joining or a place next to itself does nothing; a side splits it off.
        assert_eq!(at(150.0, 110.0, Some("a1")), None);
        assert_eq!(at(70.0, 10.0, Some("a1")), None, "right after itself");
        assert_eq!(at(130.0, 10.0, Some("a1")), Some(Target::Tabs { anchor: "a2", index: 2 }));
        assert_eq!(at(10.0, 110.0, Some("a1")), Some(Target::Split("a2", Edge::Left)));
        assert_eq!(at(350.0, 110.0, Some("b1")), None, "an area of only itself");
    }

    #[test]
    fn dropped_tabs_move_there() {
        let mut dock = drawn();
        dock.move_tab(&"a1", &Target::Tabs { anchor: "a2", index: 2 });
        assert_eq!(areas(&dock), [vec!["a2", "a1"], vec!["b1"]], "to the end of its own bar");
        dock.move_tab(&"b1", &Target::Join("a2"));
        assert_eq!(areas(&dock), [vec!["a2", "a1", "b1"]], "its area went");
        assert_eq!(dock.focused(), Some(&"b1"));
        dock.move_tab(&"a1", &Target::Split("a2", Edge::Bottom));
        assert_eq!(areas(&dock), [vec!["a2", "b1"], vec!["a1"]]);
        assert!(matches!(dock.main.as_ref().unwrap(), Node::Split(s) if s.dir == Dir::Vertical));
        dock.put("new", &Target::Tabs { anchor: "b1", index: 1 });
        assert_eq!(areas(&dock), [vec!["a2", "new", "b1"], vec!["a1"]]);
    }

    #[test]
    fn tabs_float_in_windows_and_dock_back() {
        let mut dock = Dock::new(vec!["a", "b"]);
        let rect = Rect::from_min_size(egui::pos2(100.0, 100.0), egui::vec2(400.0, 300.0));
        dock.float(&"b", rect);
        assert_eq!(areas(&dock), [vec!["a"]]);
        assert_eq!(dock.windows.len(), 1);
        assert!(dock.contains(&"b"));
        dock.float(&"a", rect);
        assert_eq!(dock.windows.len(), 1, "not the main area's last tab");
        dock.move_tab(&"b", &Target::Join("a"));
        assert!(dock.windows.is_empty(), "an empty window goes");
        assert_eq!(areas(&dock), [vec!["a", "b"]]);
    }

    #[test]
    fn layouts_round_trip() {
        let mut dock = Dock::new(vec!["console"]);
        dock.main.as_mut().unwrap().split(Edge::Top, 0.7, vec!["inspector"]);
        dock.add_window(vec!["builder"], Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(400.0, 300.0)));
        let text = serde_json::to_string(&dock).unwrap();
        let back: Dock<String> = serde_json::from_str(&text).unwrap();
        assert_eq!(back.tabs().cloned().collect::<Vec<_>>(), ["inspector", "console"], "windows aren't kept");
    }

    /// A layout as egui_dock saved it (taken from the app's storage).
    #[test]
    fn egui_dock_layouts_are_read() {
        #[derive(Clone, Debug, PartialEq, Deserialize)]
        enum Tab {
            Canvas(String),
            Inspector,
            Console,
        }
        let saved = r#"(surfaces:[Main((nodes:[Horizontal((rect:(min:(x:44.0,y:34.0),max:(x:2560.0,y:1416.0)),fraction:0.80445105,fully_collapsed:false,collapsed_leaf_count:0)),Leaf((rect:(min:(x:44.0,y:34.0),max:(x:2067.0,y:1416.0)),viewport:(min:(x:44.0,y:66.0),max:(x:2067.0,y:1416.0)),tabs:[Canvas("main")],active:(0),scroll:0.0,collapsed:false,tab_bar_hidden:false)),Leaf((rect:(min:(x:2068.0,y:34.0),max:(x:2560.0,y:1416.0)),viewport:(min:(x:2068.0,y:66.0),max:(x:2560.0,y:1416.0)),tabs:[Inspector, Console],active:(1),scroll:0.0,collapsed:false,tab_bar_hidden:false))],focused_node:Some((2)),collapsed:false,collapsed_leaf_count:0)),Window((nodes:[Leaf((rect:(min:(x:0.0,y:0.0),max:(x:1.0,y:1.0)),viewport:(min:(x:0.0,y:0.0),max:(x:1.0,y:1.0)),tabs:[Console],active:(0),scroll:0.0,collapsed:false,tab_bar_hidden:false))]),(screen_rect:None,dragged:false,next_position:None,next_size:None,new:false))],focused_surface:Some((0)))"#;
        let old: legacy::DockState<Tab> = ron::from_str(saved).unwrap();
        let dock = old.convert();
        let Some(Node::Split(s)) = &dock.main else { panic!("a split") };
        assert_eq!(s.dir, Dir::Horizontal);
        assert!((s.fraction - 0.804).abs() < 0.001);
        let leaves = dock.main.as_ref().unwrap().leaves();
        assert_eq!(leaves[0].tabs, [Tab::Canvas("main".into())]);
        assert_eq!(leaves[1].tabs, [Tab::Inspector, Tab::Console]);
        assert_eq!(leaves[1].active, 1);
        assert!(dock.windows.is_empty());
    }
}
