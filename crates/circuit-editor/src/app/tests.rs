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
    assert_eq!(prefs.activity, ["workspaces", "search", "git", "stored-data", "live", "plugins"]);
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
        Tab::Builder(crate::builders::Kind::Embed, 1),
        Tab::Builder(crate::builders::Kind::Layout, 2),
        Tab::Builder(crate::builders::Kind::Modal, 3),
        Tab::Builder(crate::builders::Kind::Canvas, 4),
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

/// A builder opens as a tab beside the workspaces, the remote's console
/// with the panels at the bottom; neither floats over the canvas.
#[test]
fn builders_open_in_the_middle_and_the_remote_console_at_the_bottom() {
    let mut dock = default_layout();
    open_canvas(&mut dock, "main".into());
    let embed = Tab::Builder(crate::builders::Kind::Embed, 1);
    place_panel(&mut dock, embed.clone());
    place_panel(&mut dock, Tab::BotSettings);
    place_panel(&mut dock, Tab::ServerConsole);
    assert!(dock.windows.is_empty(), "nothing floats");
    let all = leaves(&dock);
    let middle = all.iter().find(|l| l.contains(&Tab::Canvas("main".into()))).unwrap();
    assert!(middle.contains(&embed) && middle.contains(&Tab::BotSettings), "{middle:?}");
    let bottom = all.iter().find(|l| l.contains(&Tab::Console)).unwrap();
    assert!(bottom.contains(&Tab::ServerConsole), "{bottom:?}");
    assert_eq!(super::chrome::window_key(&Tab::Export), "Export");
}

/// Ctrl+W closes a tab of the middle, never a panel: the one the user is
/// in, or the one in front there while a panel has the focus.
#[test]
fn close_tab_picks_a_tab_of_the_middle() {
    let mut dock = default_layout();
    open_canvas(&mut dock, "main".into());
    open_canvas(&mut dock, "other".into());
    assert_eq!(tab_to_close(&dock), Some(Tab::Canvas("other".into())));
    dock.focus(&Tab::Canvas("main".into()));
    assert_eq!(tab_to_close(&dock), Some(Tab::Canvas("main".into())));
    // In the Console: still the workspace in front, not the Console.
    dock.focus(&Tab::Console);
    assert_eq!(tab_to_close(&dock), Some(Tab::Canvas("main".into())));
    // Nothing in the middle: nothing to close.
    close_tabs(&mut dock, |t| matches!(t, Tab::Canvas(_)));
    assert_eq!(tab_to_close(&dock), None);
    use crate::editor::keymap::{Action, Shortcut};
    assert_eq!(Action::CloseTab.defaults(), [Shortcut::parse("Ctrl+W").unwrap()]);
}

/// Ctrl+1 to Ctrl+9 go to that tab of the area the user is in; 9 is its last.
#[test]
fn ctrl_and_a_digit_goes_to_that_tab_of_the_area_in_use() {
    let mut dock = default_layout();
    for key in ["a", "b", "c"] {
        open_canvas(&mut dock, key.into());
    }
    assert_eq!(nth_tab(&mut dock, 1), Some(Tab::Canvas("a".into())));
    assert_eq!(nth_tab(&mut dock, 3), Some(Tab::Canvas("c".into())));
    assert_eq!(nth_tab(&mut dock, 9), Some(Tab::Canvas("c".into())), "9 is the last one");
    assert_eq!(nth_tab(&mut dock, 4), None, "there is no fourth");
    // In the bottom panels: their tabs.
    dock.focus(&Tab::Console);
    assert_eq!(nth_tab(&mut dock, 2), Some(Tab::Problems));
    assert_eq!(nth_tab(&mut dock, 9), Some(Tab::Problems));
}

/// A closed tab is remembered with its place among its area's tabs, and
/// goes back there; builders only with what they were editing.
#[test]
fn a_closed_tab_goes_back_where_it_was() {
    use super::closed::{places, remembered};
    let mut dock = default_layout();
    for key in ["a", "b", "c"] {
        open_canvas(&mut dock, key.into());
    }
    let b = Tab::Canvas("b".into());
    let (anchor, index) = places(&dock)[&b].clone();
    assert_eq!((anchor.clone(), index), (Some(Tab::Canvas("a".into())), 1));
    dock.remove(&b);
    dock.put(b.clone(), &Target::Tabs { anchor: anchor.unwrap(), index });
    let middle = leaves(&dock).into_iter().find(|l| l.contains(&b)).unwrap();
    assert_eq!(middle, [Tab::Canvas("a".into()), b, Tab::Canvas("c".into())], "between the two again");
    // Alone in its area, it has no neighbour to go back to.
    assert_eq!(places(&dock)[&Tab::Inspector], (None, 0));
    assert!(remembered(&Tab::Canvas("a".into()), false) && remembered(&Tab::Settings, false) && remembered(&Tab::Console, false));
    let builder = Tab::Builder(crate::builders::Kind::Layout, 7);
    assert!(remembered(&builder, true) && !remembered(&builder, false), "a builder only with its draft");
    assert!(!remembered(&Tab::LayoutBuilder, true), "an earlier version's builder tab: not at all");
    assert!(!remembered(&Tab::Export, true) && !remembered(&Tab::Start, false) && !remembered(&Tab::ServerConsole, false));
    use crate::editor::keymap::{Action, Shortcut};
    assert_eq!(Action::ReopenTab.defaults(), [Shortcut::parse("Ctrl+Shift+T").unwrap()]);
}

/// The search box keeps out of the way of the menus and of what is beside
/// Run: it gets narrower, moves off the middle, or is left out.
#[test]
fn the_search_box_fits_between_the_menus_and_the_buttons() {
    use super::menus::search_place;
    let bar = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1440.0, 34.0));
    // Room enough: in the middle, as wide as it likes.
    let r = search_place(bar, 360.0, 1120.0).unwrap();
    assert_eq!((r.center().x, r.width()), (720.0, crate::app::titlebar::SEARCH_W));
    // Local | Remote beside Run reaches into the middle: it moves left.
    let r = search_place(bar, 360.0, 860.0).unwrap();
    assert!(r.right() <= 848.0 && r.left() >= 372.0, "{r:?}");
    assert_eq!(r.width(), crate::app::titlebar::SEARCH_W);
    // A narrow window: narrower, between the two.
    let narrow = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 34.0));
    let r = search_place(narrow, 360.0, 620.0).unwrap();
    assert_eq!((r.left(), r.right()), (372.0, 608.0));
    // No room: none.
    assert!(search_place(narrow, 360.0, 500.0).is_none());
}
