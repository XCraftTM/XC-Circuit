use super::*;
use crate::editor::prefs::SideView;
use crate::ui::dock::Target;

fn leaves(dock: &Dock<Tab>) -> Vec<Vec<Tab>> {
    dock.main.iter().flat_map(|m| m.leaves()).map(|l| l.tabs.clone()).collect()
}

fn whole(dock: &Dock<Tab>) -> bool {
    dock.main.as_ref().is_some_and(|m| m.whole())
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
    assert!(whole(&dock));

    // Workspaces dragged out of the activity bar gets a column on the left,
    // and panels opened after that stay out of it.
    place_panel(&mut dock, Tab::Side(SideView::Workspaces));
    close_tabs(&mut dock, |t| t.side() == Side::Bottom);
    place_panel(&mut dock, Tab::Console);
    assert!(whole(&dock));
    let Some(Node::Split(s)) = &dock.main else { panic!("a split") };
    assert!(matches!(&*s.first, Node::Leaf(l) if l.tabs == [Tab::Side(SideView::Workspaces)]), "the column is still the left half");
    assert_eq!(dock.focused(), Some(&Tab::Console), "what opens is the tab the user is in");
}

/// Workspaces, Stored Data and Plugins start in the activity bar, not the dock.
#[test]
fn the_default_layout_leaves_the_side_bar_s_views_to_the_activity_bar() {
    let dock = default_layout();
    assert!(dock.find(|t| matches!(t, Tab::Side(_))).is_none());
    assert_eq!(leaves(&dock), [vec![Tab::Inspector], vec![Tab::Console, Tab::Problems]]);
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
    for tab in side.into_iter().chain([Tab::Inspector, Tab::Console, Tab::Problems, Tab::ServerConsole, Tab::Git, Tab::Host, Tab::Live, Tab::Plugin("my.plugin".into(), "stats".into())]) {
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

/// An icon dropped on the main area becomes a tab where it was let go.
#[test]
fn an_icon_dropped_on_the_main_area_becomes_a_tab_there() {
    let mut dock = Dock::new(vec![Tab::Canvas("main".into())]);
    dock.main.as_mut().unwrap().split(Edge::Right, 0.75, vec![Tab::Inspector]);
    dock.put(Tab::Side(SideView::Workspaces), &Target::Join(Tab::Inspector));
    assert_eq!(leaves(&dock)[1], [Tab::Inspector, Tab::Side(SideView::Workspaces)], "next to the Inspector");
    dock.put(Tab::Console, &Target::Split(Tab::Canvas("main".into()), Edge::Bottom));
    assert_eq!(leaves(&dock), [vec![Tab::Canvas("main".into())], vec![Tab::Console], vec![Tab::Inspector, Tab::Side(SideView::Workspaces)]]);
    // The area it was dropped on is gone meanwhile: it still gets a place.
    dock.put(Tab::Problems, &Target::Join(Tab::Backups));
    assert!(dock.contains(&Tab::Problems));
    assert!(whole(&dock));
}

#[test]
fn a_saved_layout_without_tabs_in_an_area_is_tidied() {
    let mut dock = default_layout();
    assert!(!repair_layout(&mut dock), "a whole layout is left alone");
    for leaf in dock.main.as_mut().unwrap().leaves_mut() {
        if leaf.tabs.contains(&Tab::Console) {
            leaf.tabs.clear();
        }
    }
    assert!(repair_layout(&mut dock), "an empty area goes");
    assert_eq!(leaves(&dock), [vec![Tab::Inspector]]);
}

#[test]
fn a_new_project_s_canvas_opens_where_the_old_ones_were() {
    // Inspector on the right, the full height; the Console only under the canvas.
    let mut dock = Dock::new(vec![Tab::Canvas("old".into()), Tab::Canvas("other".into())]);
    let main = dock.main.as_mut().unwrap();
    main.split(Edge::Right, 0.8, vec![Tab::Inspector]);
    main.at_mut(&[false]).unwrap().split(Edge::Bottom, 0.7, vec![Tab::Console]);
    let canvas_area = |dock: &Dock<Tab>| leaves(dock).iter().position(|l| l.iter().any(|t| matches!(t, Tab::Canvas(_))));

    let before = canvas_area(&dock);
    clear_canvases(&mut dock);
    assert!(dock.find(|t| matches!(t, Tab::Canvas(_))).is_none());
    assert_eq!(leaves(&dock).len(), 3, "the canvas area stays, with a Start tab");
    open_canvas(&mut dock, "new".into());
    assert_eq!(canvas_area(&dock), before, "in the old canvas area");
    close_tabs(&mut dock, |t| matches!(t, Tab::Start));
    assert_eq!(canvas_area(&dock), before);
    assert_eq!(dock.tabs().count(), 3, "new canvas, Inspector, Console");
}

/// The panels next to the canvases keep their size when the window grows.
#[test]
fn panels_keep_their_size_when_the_window_grows() {
    let mut dock = Dock::new(vec![Tab::Canvas("main".into())]);
    let main = dock.main.as_mut().unwrap();
    main.split(Edge::Right, 0.8, vec![Tab::Inspector]);
    let Node::Split(s) = main else { panic!() };
    s.rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1000.0, 600.0));
    keep_panel_sizes(main, egui::vec2(500.0, 0.0));
    let Node::Split(s) = main else { panic!() };
    assert!((s.fraction * 1500.0 - 1300.0).abs() < 0.5, "the Inspector stays 200 wide: {}", s.fraction);
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
        Tab::ModalBuilder,
        Tab::CanvasBuilder,
        Tab::SlashCommands,
        Tab::BotSettings,
        Tab::Export,
        Tab::Hosting,
        Tab::Git,
        Tab::Host,
        Tab::Live,
        Tab::ServerConsole,
        Tab::Plugins,
        Tab::WorkspaceExport,
        Tab::WorkspaceImport,
        Tab::Start,
    ];
    for tab in tabs {
        assert!(ed.reg.icons.contains_key(tab.icon()), "{tab:?}: no icon {}", tab.icon());
    }
}

/// The side bar shows one view, or two: a second one in its lower half.
#[test]
fn the_side_bar_shows_a_second_view_below() {
    use activity::{click_in_side, show_in_side, tidy_side};
    let mut p = crate::editor::prefs::Prefs::default();
    let shown = |p: &crate::editor::prefs::Prefs| (p.side_shown.clone(), p.side_lower.clone());
    let s = |k: &str| Some(k.to_string());
    assert_eq!(shown(&p), (s("workspaces"), None));
    // Stored Data dropped on the lower half: under Workspaces.
    show_in_side(&mut p, "stored-data", true);
    assert_eq!(shown(&p), (s("workspaces"), s("stored-data")));
    // Plugins on the upper half: it takes Workspaces' place.
    show_in_side(&mut p, "plugins", false);
    assert_eq!(shown(&p), (s("plugins"), s("stored-data")));
    // The upper one dropped below: the two swap.
    show_in_side(&mut p, "plugins", true);
    assert_eq!(shown(&p), (s("stored-data"), s("plugins")));
    // Clicking the upper one's icon hides it: the lower one takes the whole bar.
    click_in_side(&mut p, "stored-data");
    assert_eq!(shown(&p), (s("plugins"), None));
    // Nothing above: one dropped below is simply shown.
    click_in_side(&mut p, "plugins");
    assert_eq!(shown(&p), (None, None));
    show_in_side(&mut p, "workspaces", true);
    assert_eq!(shown(&p), (s("workspaces"), None));
    // An icon taken out of the activity bar leaves its half.
    show_in_side(&mut p, "stored-data", true);
    p.activity.retain(|k| k != "stored-data");
    tidy_side(&mut p);
    assert_eq!(shown(&p), (s("workspaces"), None));
    // The split stays in a sensible range.
    p.side_split = 2.0;
    tidy_side(&mut p);
    assert_eq!(p.side_split, 0.85);
}

/// A builder's window opens where it was last and as big, by its kind,
/// kept inside the app's window; a window it shares with other tabs, or one
/// of its own outside the app, isn't counted.
#[test]
fn builder_windows_remember_their_place() {
    use super::chrome::{remember_place, window_key};
    let mut dock: Dock<Tab> = Dock::new(vec![Tab::Inspector]);
    dock.add_window(vec![Tab::EmbedBuilder], egui::Rect::from_min_size(egui::pos2(50.0, 50.0), egui::vec2(900.0, 640.0)));
    let mut places = std::collections::BTreeMap::new();
    let key = window_key(&Tab::EmbedBuilder);
    assert_eq!(key, "EmbedBuilder");
    dock.windows[0].pos = egui::pos2(349.6, 98.2);
    dock.windows[0].size = egui::vec2(1340.4, 900.6);
    remember_place(&dock, &mut places, &Tab::EmbedBuilder, key.clone());
    assert_eq!(places[&key], [350.0, 98.0, 1340.0, 901.0]);
    // Out in a window of its own: its place there isn't the in-app one.
    dock.windows[0].os = Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(400.0, 300.0)));
    dock.windows[0].size = egui::vec2(400.0, 300.0);
    remember_place(&dock, &mut places, &Tab::EmbedBuilder, key.clone());
    assert_eq!(places[&key], [350.0, 98.0, 1340.0, 901.0]);
    // Saved with the settings, and read back.
    let prefs = crate::editor::prefs::Prefs { window_places: places, ..Default::default() };
    let back: crate::editor::prefs::Prefs = serde_json::from_str(&serde_json::to_string(&prefs).unwrap()).unwrap();
    assert_eq!(back.window_places[&key], [350.0, 98.0, 1340.0, 901.0]);
    // In a smaller window it's moved (and shrunk) to stay inside.
    use super::dock::fit_inside;
    let area = egui::Rect::from_min_size(egui::pos2(0.0, 30.0), egui::vec2(1200.0, 800.0));
    let r = fit_inside(egui::Rect::from_min_size(egui::pos2(900.0, 600.0), egui::vec2(1340.0, 900.0)), area);
    assert!(area.contains_rect(r), "{r:?}");
    let same = egui::Rect::from_min_size(egui::pos2(100.0, 100.0), egui::vec2(500.0, 400.0));
    assert_eq!(fit_inside(same, area), same, "one that fits stays where it was");
}
