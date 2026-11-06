use super::*;

fn leaves(dock: &DockState<Tab>) -> Vec<Vec<Tab>> {
    dock.main_surface().iter().filter_map(|n| n.tabs().map(|t| t.to_vec())).collect()
}

#[test]
fn panels_open_next_to_their_kind() {
    let mut dock = default_layout();
    close_tabs(&mut dock, |t| *t == Tab::Problems);
    place_panel(&mut dock, Tab::Problems);
    let bottom = leaves(&dock).into_iter().find(|l| l.contains(&Tab::Console)).unwrap();
    assert!(bottom.contains(&Tab::Problems), "it joins Console: {bottom:?}");
    assert_eq!(leaves(&dock).len(), 2, "no new area");

    // With the whole bottom group closed, it gets a new area below.
    close_tabs(&mut dock, |t| t.side() == Side::Bottom);
    assert_eq!(leaves(&dock).len(), 1);
    place_panel(&mut dock, Tab::Problems);
    assert_eq!(leaves(&dock).len(), 2);
    assert!(tree_ok(dock.main_surface()));

    // Workspaces dragged out of the activity bar gets a column on the left,
    // and panels opened after that stay out of it.
    place_panel(&mut dock, Tab::Side(crate::editor::prefs::SideView::Workspaces));
    close_tabs(&mut dock, |t| t.side() == Side::Bottom);
    place_panel(&mut dock, Tab::Console);
    assert!(tree_ok(dock.main_surface()));
    let root = NodeIndex::root();
    assert!(matches!(&dock.main_surface()[root.left()], Node::Leaf(l) if l.tabs() == [Tab::Side(crate::editor::prefs::SideView::Workspaces)]), "the column is still the left half");
}

/// Workspaces, Stored Data and Plugins start in the activity bar, not the dock.
#[test]
fn the_default_layout_leaves_the_side_bar_s_views_to_the_activity_bar() {
    let dock = default_layout();
    assert!(dock.find_tab_from(|t| matches!(t, Tab::Side(_))).is_none());
    let prefs = crate::editor::prefs::Prefs::default();
    assert_eq!(prefs.activity, ["workspaces", "stored-data", "plugins"]);
    assert_eq!(prefs.side_shown.as_deref(), Some("workspaces"));
}

/// What can be an icon in the activity bar has a key that comes back as the
/// same tab; workspaces, builders and pages can't.
#[test]
fn activity_bar_keys_come_back_as_the_same_tab() {
    use crate::editor::prefs::SideView;
    let side = SideView::ALL.map(Tab::Side);
    for tab in side.into_iter().chain([Tab::Inspector, Tab::Console, Tab::Problems, Tab::Plugin("xc.stats".into(), "stats".into())]) {
        let key = tab.side_key().expect("it can");
        assert_eq!(Tab::from_side_key(&key), Some(tab));
    }
    for tab in [Tab::Canvas("main".into()), Tab::Settings, Tab::BotSettings, Tab::Start, Tab::EmbedBuilder] {
        assert_eq!(tab.side_key(), None, "{tab:?}");
    }
    assert_eq!(Tab::from_side_key("nonsense"), None);
}

/// Icons dragged in the activity bar, or dropped on it, go where they're let go.
#[test]
fn activity_bar_icons_go_where_they_are_dropped() {
    use super::activity::{icon_after, put_in_activity};
    let mut list: Vec<String> = ["a", "b", "c"].map(String::from).to_vec();
    put_in_activity(&mut list, "c", Some("a"));
    assert_eq!(list, ["c", "a", "b"]);
    put_in_activity(&mut list, "c", None);
    assert_eq!(list, ["a", "b", "c"], "to the end");
    put_in_activity(&mut list, "b", Some("b"));
    assert_eq!(list, ["a", "b", "c"], "onto itself: stays");
    put_in_activity(&mut list, "new", Some("b"));
    assert_eq!(list, ["a", "new", "b", "c"], "a tab from the dock");

    let slot = |k: &str, top: f32| (k.to_string(), egui::Rect::from_min_size(egui::pos2(0.0, top), egui::vec2(44.0, 40.0)));
    let slots = [slot("a", 0.0), slot("b", 42.0), slot("c", 84.0)];
    assert_eq!(icon_after(&slots, 5.0), Some("a"), "over the top half of the first: before it");
    assert_eq!(icon_after(&slots, 30.0), Some("b"));
    assert_eq!(icon_after(&slots, 120.0), None, "below the middle of the last: at the end");
}

/// An icon dropped on the main area joins the tabs of the area under it, or
/// (near an edge) gets a new area split off that side.
#[test]
fn an_icon_dropped_on_the_main_area_becomes_a_tab_there() {
    use super::activity::{dock_drop_at, dock_tab_at};
    use crate::editor::prefs::SideView;
    use egui_dock::Split;
    let mut dock = DockState::new(vec![Tab::Canvas("main".into())]);
    dock.main_surface_mut().split_right(NodeIndex::root(), 0.75, vec![Tab::Inspector]);
    // Where the areas are once drawn: the canvas 0..300, the Inspector 300..400,
    // each with a 24 high tab bar.
    for (_, leaf) in dock.iter_leaves_mut() {
        let x = if leaf.tabs.contains(&Tab::Inspector) { 300.0 } else { 0.0 };
        let w = if x == 0.0 { 300.0 } else { 100.0 };
        leaf.rect = egui::Rect::from_min_size(egui::pos2(x, 0.0), egui::vec2(w, 200.0));
        leaf.viewport = egui::Rect::from_min_max(egui::pos2(x, 24.0), egui::pos2(x + w, 200.0));
    }
    let at = |x: f32, y: f32| dock_drop_at(&dock, egui::pos2(x, y));
    assert_eq!(at(150.0, 10.0).and_then(|d| d.split), None, "on the tab bar: joins");
    assert_eq!(at(150.0, 110.0).and_then(|d| d.split), None, "the middle: joins");
    assert_eq!(at(10.0, 110.0).and_then(|d| d.split), Some(Split::Left));
    assert_eq!(at(150.0, 195.0).and_then(|d| d.split), Some(Split::Below));
    assert!(at(500.0, 100.0).is_none(), "outside the dock");
    let join = at(350.0, 110.0);
    let below = at(150.0, 195.0);

    dock_tab_at(&mut dock, Tab::Side(SideView::Workspaces), join);
    let leaf = leaves(&dock).into_iter().find(|l| l.contains(&Tab::Inspector)).unwrap();
    assert!(leaf.contains(&Tab::Side(SideView::Workspaces)), "next to the Inspector: {leaf:?}");

    dock_tab_at(&mut dock, Tab::Console, below);
    assert!(tree_ok(dock.main_surface()));
    assert_eq!(leaves(&dock).len(), 3, "a new area");
    assert!(leaves(&dock).contains(&vec![Tab::Console]));

    // The area it was dropped on isn't one any more (it's a split now): where its kind opens.
    let gone = below.map(|d| super::activity::DockDrop { path: egui_dock::NodePath { surface: d.path.surface, node: NodeIndex::root() }, ..d });
    dock_tab_at(&mut dock, Tab::Problems, gone);
    assert!(dock.find_tab(&Tab::Problems).is_some());
    assert!(tree_ok(dock.main_surface()));
}

#[test]
fn closing_every_bottom_tab_keeps_the_layout_whole() {
    // The bottom area's neighbour is a split (Workspaces | Inspector), the
    // case where retain_tabs breaks the tree.
    let mut dock = default_layout();
    close_tabs(&mut dock, |t| t.side() == Side::Bottom);
    assert!(tree_ok(dock.main_surface()));
    let mut broken = default_layout();
    broken.retain_tabs(|t| t.side() != Side::Bottom);
    if !tree_ok(broken.main_surface()) {
        assert!(repair_layout(&mut broken), "a broken layout is rebuilt");
        assert!(tree_ok(broken.main_surface()));
        let open: Vec<Tab> = broken.iter_all_tabs().map(|(_, t)| t.clone()).collect();
        assert!(open.contains(&Tab::Inspector));
        assert!(!open.contains(&Tab::Console), "with the same tabs open");
    }
    assert!(!repair_layout(&mut dock), "a whole layout is left alone");
}

#[test]
fn a_new_project_s_canvas_opens_where_the_old_ones_were() {
    // Inspector on the right, the full height; the Console only under the canvas.
    let mut dock = DockState::new(vec![Tab::Canvas("old".into()), Tab::Canvas("other".into())]);
    let surface = dock.main_surface_mut();
    let [canvas, _] = surface.split_right(NodeIndex::root(), 0.8, vec![Tab::Inspector]);
    surface.split_below(canvas, 0.7, vec![Tab::Console]);
    let canvas_rect_index = |dock: &DockState<Tab>| dock.find_tab_from(|t| matches!(t, Tab::Canvas(_))).map(|p| p.node_path().node);

    let before = canvas_rect_index(&dock);
    clear_canvases(&mut dock);
    assert!(dock.find_tab_from(|t| matches!(t, Tab::Canvas(_))).is_none());
    assert!(tree_ok(dock.main_surface()), "the canvas area stays, with a Start tab");
    open_canvas(&mut dock, "new".into());
    assert_eq!(canvas_rect_index(&dock), before, "in the old canvas area");
    close_tabs(&mut dock, |t| matches!(t, Tab::Start));
    assert_eq!(canvas_rect_index(&dock), before);
    assert_eq!(dock.iter_all_tabs().count(), 3, "new canvas, Inspector, Console");
}

#[test]
fn projects_start_from_templates() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let base = std::env::temp_dir().join(format!("xc-templates-{}", std::process::id()));
    for entry in std::fs::read_dir(root.join("examples/templates")).unwrap().flatten() {
        let dir = base.join(entry.file_name());
        std::fs::create_dir_all(&dir).unwrap();
        create_from_template(&entry.path(), &dir, "My Bot").unwrap();
        let made = Project::load(&dir).unwrap();
        let template = Project::load(entry.path()).unwrap();
        assert_eq!(made.meta.name, "My Bot");
        assert_eq!(made.workspaces.len(), template.workspaces.len());
        assert_eq!(made.meta.data, template.meta.data, "slash commands come along");
        assert!(dir.join(".gitignore").is_file());
        assert!(create_from_template(&entry.path(), &dir, "Again").is_err(), "an existing project isn't overwritten");
    }
    let _ = std::fs::remove_dir_all(base);
}

/// Every tab has an icon, and it's one the bundled plugins have.
#[test]
fn every_tab_has_an_icon() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let ed = Editor::new(vec![root.join("plugins")], None).unwrap();
    let tabs = [
        Tab::Canvas("x".into()),
        Tab::Inspector,
        Tab::Console,
        Tab::Problems,
        Tab::Settings,
        Tab::Backups,
        Tab::Plugin("p".into(), "q".into()),
        Tab::EmbedBuilder,
        Tab::LayoutBuilder,
        Tab::CanvasBuilder,
        Tab::SlashCommands,
        Tab::BotSettings,
        Tab::Export,
        Tab::Plugins,
        Tab::WorkspaceExport,
        Tab::WorkspaceImport,
        Tab::Start,
    ];
    for tab in tabs {
        assert!(ed.reg.icons.contains_key(tab.icon()), "{tab:?}: no icon {}", tab.icon());
    }
}
