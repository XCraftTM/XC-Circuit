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

/// Panels (and only panels) can move into the side bar, under keys that come back as the same tab.
#[test]
fn panels_have_side_bar_keys() {
    for tab in [Tab::Inspector, Tab::Console, Tab::Problems, Tab::Plugin("xc.stats".into(), "stats".into())] {
        let key = tab.side_key().expect("a panel");
        assert_eq!(Tab::from_side_key(&key), Some(tab));
    }
    for tab in [Tab::Canvas("main".into()), Tab::Settings, Tab::BotSettings, Tab::Start] {
        assert!(tab.side_key().is_none(), "{tab:?} stays in the main area");
    }
    assert_eq!(Tab::from_side_key("nothing"), None);
}
