use super::*;

/// Undo in the Slash Commands tab takes back a saved change to the
/// commands, and the open builder shows them as they were.
#[test]
fn undo_takes_back_project_data() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = std::env::temp_dir().join(format!("xc-undo-data-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    Project::create(&dir, "U", "discord-bot").unwrap();
    let mut ed = Editor::new(vec![root.join("plugins")], None).unwrap();
    ed.open_project(&dir);
    let commands = serde_json::json!([{ "name": "ping", "description": "Pong", "options": [] }]);
    ed.project.as_mut().unwrap().meta.data.insert("slashCommands".into(), commands);
    assert!(ed.unsaved_now());
    crate::builders::slash::open(&mut ed);
    assert_eq!(ed.slash_edit.as_ref().unwrap().commands.len(), 1);

    ed.undo_project = true;
    assert_eq!(ed.undo_key(), crate::editor::history::PROJECT);
    assert!(ed.undo());
    assert!(ed.project.as_ref().unwrap().meta.data.is_empty());
    assert!(ed.slash_edit.as_ref().unwrap().commands.is_empty(), "the builder shows the commands as they are now");
    assert!(!ed.unsaved_now(), "back as saved");
    assert!(ed.redo());
    assert_eq!(ed.slash_edit.as_ref().unwrap().commands.len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Every bundled block's define() runs in the sandbox with its defaults
/// and with each choice of its select fields, and gives a usable shape.
#[test]
fn older_texts_upgrade_to_javascript() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut ed = Editor::new(vec![root.join("plugins")], None).unwrap();
    ed.open_project(&root.join("examples/DemoBot"));
    assert!(!ed.unsaved_now(), "the example is up to date");
    let nodes = &mut ed.project.as_mut().unwrap().workspaces.get_mut("rank-command").unwrap().nodes;
    nodes.get_mut("4").unwrap().values.insert("text".into(), "Hi {user}, {#3.embed.title} x{3}".into());
    nodes.get_mut("3").unwrap().values["embed"]["title"] = "Rank of {user}".into();
    ed.upgrade_texts();
    let nodes = &ed.project.as_ref().unwrap().workspaces["rank-command"].nodes;
    assert_eq!(nodes["4"].values["text"], "Hi ${user}, ${#3.embed.title} x{3}");
    assert_eq!(nodes["3"].values["embed"]["title"], "Rank of ${user}", "texts inside designs too");
    assert!(ed.unsaved_now());
}

#[test]
fn every_block_defines_cleanly() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let ed = Editor::new(vec![root.join("plugins")], None).unwrap();
    assert!(ed.reg.problems.is_empty(), "{:?}", ed.reg.problems);
    assert!(ed.reg.blocks.len() > 60 && ed.reg.blocks.values().filter(|b| b.has_define).count() > 15, "the bundled blocks loaded");
    let project = serde_json::json!({ "name": "Test", "mode": "discord-bot", "settings": {}, "data": { "slashCommands": [{ "name": "ping", "description": "Pong", "options": [] }] } });
    let mut failures = Vec::new();
    for def in ed.reg.blocks.values() {
        let defaults: Map<String, Value> = def.fields.iter().filter_map(|(k, f)| f.default.clone().map(|d| (k.clone(), d))).collect();
        let mut cases = vec![defaults.clone()];
        for (k, f) in &def.fields {
            for (_, choice, _) in f.choice_list() {
                let mut v = defaults.clone();
                v.insert(k.clone(), Value::String(choice));
                cases.push(v);
            }
        }
        for values in cases {
            if def.has_define {
                let key = &ed.reg.engine_keys[&def.id];
                match ed.engine.define(key, &values, &Map::new(), &project) {
                    Ok(d) => {
                        let shape = circuit_core::BlockShape::build(def, &values, Some(&d));
                        assert!(!shape.fields.is_empty() || !shape.outputs.is_empty() || def.kind != circuit_core::BlockKind::Value, "{} has no shape", def.id);
                    }
                    Err(e) => failures.push(format!("{} with {:?}: {e:#}", def.id, values)),
                }
            } else {
                circuit_core::BlockShape::build(def, &values, None);
            }
        }
    }
    assert!(failures.is_empty(), "define() failed:\n{}", failures.join("\n"));

    // Send Message's Defer, then delete: no text of its own, and how long it
    // thinks (Delete after) on the block instead of under Advanced.
    let send = ed.reg.block("discord.send_message").unwrap();
    let mut values: Map<String, Value> = send.fields.iter().filter_map(|(k, f)| f.default.clone().map(|d| (k.clone(), d))).collect();
    values.insert("mode".into(), "deferDelete".into());
    let d = ed.engine.define(&ed.reg.engine_keys[&send.id], &values, &Map::new(), &project).unwrap();
    let shape = circuit_core::BlockShape::build(send, &values, Some(&d));
    let shown: Vec<&str> = shape.visible_fields().map(|(k, _)| k.as_str()).collect();
    assert!(shown.contains(&"deleteAfter") && shown.contains(&"interaction") && !shown.contains(&"text"), "{shown:?}");
    assert!(!shape.field("deleteAfter").unwrap().advanced);
}

/// Upgrades the bundled example, starters and plugin starters to the
/// blocks' current versions (as opening them in the editor does) and
/// saves them: `cargo test -p circuit-editor upgrade_bundled_projects -- --ignored`
#[test]
#[ignore]
fn upgrade_bundled_projects() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(root.join("examples/templates")).unwrap().flatten().map(|e| e.path()).collect();
    dirs.push(root.join("examples/DemoBot"));
    for plugin in std::fs::read_dir(root.join("plugins")).unwrap().flatten() {
        dirs.extend(std::fs::read_dir(plugin.path().join("templates")).into_iter().flatten().flatten().map(|e| e.path()));
    }
    for dir in dirs {
        let mut ed = Editor::new(vec![root.join("plugins"), dir.join("plugins")], None).unwrap();
        ed.open_project(&dir);
        if ed.edited {
            ed.project.as_mut().unwrap().save().unwrap();
            println!("upgraded {}", dir.display());
        }
    }
}

/// The starter templates and the example load, use blocks that exist, and
/// every wire connects an output (or flow) to an input that the block has.
#[test]
fn recent_projects_list() {
    let mut prefs = crate::editor::prefs::Prefs::default();
    for p in ["C:/a", "C:/b", "C:\\A\\"] {
        prefs.remember_project(Path::new(p));
    }
    assert_eq!(prefs.recent_projects, ["C:\\A\\", "C:/b"], "the same folder once, the latest first");
    for i in 0..20 {
        prefs.remember_project(Path::new(&format!("C:/p{i}")));
    }
    assert_eq!(prefs.recent_projects.len(), crate::editor::prefs::RECENT_KEPT);
    prefs.forget_project("c:/p19");
    assert_eq!(prefs.recent_projects[0], "C:/p18");
}

#[test]
fn templates_are_wired_correctly() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let ed = Editor::new(vec![root.join("plugins")], None).unwrap();
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(root.join("examples/templates")).unwrap().flatten().map(|e| e.path()).collect();
    dirs.push(root.join("examples/DemoBot"));
    // Plugins' own starters (Fluxer's).
    for plugin in std::fs::read_dir(root.join("plugins")).unwrap().flatten() {
        dirs.extend(std::fs::read_dir(plugin.path().join("templates")).into_iter().flatten().flatten().map(|e| e.path()));
    }
    assert!(dirs.len() >= 8, "the templates, Fluxer's and the example");
    let mut wrong = Vec::new();
    for dir in dirs {
        let project = Project::load(&dir).unwrap();
        assert!(project.unreadable.is_empty(), "{}: {:?}", dir.display(), project.unreadable);
        let meta = serde_json::to_value(&project.meta).unwrap();
        let mut cache = ShapeCache::default();
        for (key, ws) in &project.workspaces {
            let place = format!("{} › {key}", dir.file_name().unwrap().to_string_lossy());
            let mut shapes = HashMap::new();
            for id in ws.nodes.keys() {
                match node_shape(&ed.reg, &ed.engine, &mut cache, key, ws, id, &meta) {
                    Some(s) => {
                        shapes.insert(id.clone(), s);
                    }
                    None => wrong.push(format!("{place}: #{id} uses a missing block {}", ws.nodes[id].block)),
                }
                // Saved with the block's current version, and in a path when it is an action block.
                if let Some(def) = ed.reg.block(&ws.nodes[id].block) {
                    if ws.nodes[id].block != def.reference() {
                        wrong.push(format!("{place}: #{id} is {} (now {})", ws.nodes[id].block, def.reference()));
                    }
                    if def.kind == circuit_core::BlockKind::Action && ws.links_into(id, circuit_core::graph::ACTION_IN).next().is_none() {
                        wrong.push(format!("{place}: nothing runs #{id} {}", def.name));
                    }
                }
            }
            for link in &ws.links {
                let from_ok = if ws.zones.contains_key(&link.from.node) {
                    ["done", "error", "message", "details"].contains(&link.from.pin.as_str())
                } else {
                    shapes.get(&link.from.node).is_some_and(|s| s.output(&link.from.pin).is_some() || s.flows.contains(&link.from.pin))
                };
                let to_ok = shapes.get(&link.to.node).is_some_and(|s| link.to.pin == circuit_core::graph::ACTION_IN || s.field(&link.to.pin).is_some());
                if !from_ok || !to_ok {
                    wrong.push(format!("{place}: {}.{} > {}.{}", link.from.node, link.from.pin, link.to.node, link.to.pin));
                }
                // A wire for a ${name} in the block's text must reach that
                // name's input, not an option of the block with the same
                // name (which the text would then read instead).
                let used_in_text = ws.nodes.get(&link.to.node).is_some_and(|n| {
                    n.values.values().filter_map(Value::as_str).any(|t| circuit_core::block::placeholders(t).contains(&link.to.pin))
                });
                let placeholder = format!("${{{}}}", link.to.pin);
                if used_in_text && shapes.get(&link.to.node).and_then(|s| s.field(&link.to.pin)).is_some_and(|f| f.label.as_deref() != Some(placeholder.as_str())) {
                    wrong.push(format!("{place}: {}.{} > {}.{} reaches the block's own {} option, not {placeholder}", link.from.node, link.from.pin, link.to.node, link.to.pin, link.to.pin));
                }
            }
        }
    }
    assert!(wrong.is_empty(), "wrong wires:\n{}", wrong.join("\n"));
}

/// The database blocks list the project's databases, and add a column,
/// a comparison and a value for each condition.
#[test]
fn database_blocks_list_the_databases() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let ed = Editor::new(vec![root.join("plugins")], None).unwrap();
    let project = serde_json::json!({ "settings": { "databases": [{ "id": "db1", "name": "Main", "kind": "sqlite" }, { "id": "db2", "name": "Levels", "kind": "mariadb" }] }, "data": {} });
    let shape = |block: &str, values: Value| {
        let def = ed.reg.block(block).unwrap();
        let values = values.as_object().unwrap().clone();
        let d = ed.engine.define(&ed.reg.engine_keys[&def.id], &values, &Map::new(), &project).unwrap();
        circuit_core::BlockShape::build(def, &values, Some(&d))
    };
    let get = shape("core.get_rows", serde_json::json!({ "conditions": 2 }));
    let field = |s: &circuit_core::BlockShape, k: &str| s.fields.iter().find(|(key, _)| key == k).map(|(_, f)| f.clone());
    let choices: Vec<String> = field(&get, "database").unwrap().choice_list().into_iter().map(|(_, v, label)| format!("{v}={label}")).collect();
    assert_eq!(choices, ["default=Default database", "db1=Main", "db2=Levels", "given=Given here"]);
    // The connection's fields only show for Given here, and its secrets can only be connected.
    let shown = |s: &circuit_core::BlockShape, k: &str| s.visible_fields().any(|(key, _)| key == k);
    assert!(!shown(&get, "dbHost") && !shown(&get, "dbPassword"));
    let given = shape("core.get_rows", serde_json::json!({ "database": "given" }));
    for k in ["dbKind", "dbHost", "dbPort", "dbUser", "dbPassword", "dbName", "dbSsl", "dbUrl"] {
        assert!(shown(&given, k), "{k}");
    }
    for k in ["dbPassword", "dbUrl"] {
        assert_eq!(field(&given, k).unwrap().pin, circuit_core::PinMode::Only, "{k}");
    }
    for k in ["where1", "where1Op", "where1Value", "where2", "where2Value"] {
        assert!(field(&get, k).is_some(), "{k}");
    }
    assert!(field(&get, "where3").is_none());
    let update = shape("core.update_rows", serde_json::json!({ "columns": 2, "conditions": 1 }));
    for k in ["column1", "column2Value", "where1"] {
        assert!(field(&update, k).is_some(), "{k}");
    }
    let insert = shape("core.insert_row", serde_json::json!({}));
    assert!(field(&insert, "column2").is_some() && field(&insert, "column3").is_none(), "two columns to start with");
}

/// Each starter opens with no problems but the values left for you to fill
/// in (a role or channel id, an address): `XC_LIST_TEMPLATE_PROBLEMS=1` prints them.
#[test]
fn templates_open_without_surprises() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(root.join("examples/templates")).unwrap().flatten().map(|e| e.path()).collect();
    dirs.extend(std::fs::read_dir(root.join("plugins/xc.fluxer/templates")).unwrap().flatten().map(|e| e.path()));
    let mut found = Vec::new();
    for dir in dirs {
        let mut ed = Editor::new(vec![root.join("plugins")], None).unwrap();
        ed.open_project(&dir);
        for (text, _, at) in crate::pages::panels::collect_problems(&mut ed) {
            found.push(format!("{} {at:?}: {text}", dir.file_name().unwrap().to_string_lossy()));
        }
    }
    if std::env::var_os("XC_LIST_TEMPLATE_PROBLEMS").is_some() {
        println!("{}", found.join("\n"));
    }
    // Their descriptions say to fill these in.
    let left_to_you = ["› Add Role", "› Find Channel", "› On New Post"];
    let surprises: Vec<&String> = found.iter().filter(|p| !left_to_you.iter().any(|l| p.ends_with(l))).collect();
    assert!(surprises.is_empty(), "templates open with problems:\n{surprises:#?}");
}

/// Get Property lists what the connected value has (a Discord user's
/// username, id …, from the class catalogs) with Custom… for any other, and
/// its Property can be connected.
#[test]
fn get_property_lists_the_connected_values_properties() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let ed = Editor::new(vec![root.join("plugins")], None).unwrap();
    let def = ed.reg.block("core.get_property").unwrap();
    let shape = |connected: Value| {
        let d = ed.engine.define(&ed.reg.engine_keys[&def.id], &Map::new(), connected.as_object().unwrap(), &serde_json::json!({ "data": {} })).unwrap();
        circuit_core::BlockShape::build(def, &Map::new(), Some(&d))
    };
    let field = |s: &circuit_core::BlockShape| s.fields.iter().find(|(k, _)| k == "path").map(|(_, f)| f.clone()).unwrap();
    let user = field(&shape(serde_json::json!({ "object": "discord.user" })));
    assert!(user.custom && user.is_select(), "{user:?}");
    let keys: Vec<String> = user.choice_list().into_iter().map(|(_, k, _)| k).collect();
    assert!(keys.contains(&"username".to_string()) && keys.contains(&"id".to_string()), "{keys:?}");
    assert!(!keys.contains(&"send".to_string()), "methods aren't properties");
    // An object of a known shape, and one of no known shape (a plain text box).
    let typed = field(&shape(serde_json::json!({ "object": "object<{ name: text, xp: number }>" })));
    assert_eq!(typed.choice_list().into_iter().map(|(_, k, _)| k).collect::<Vec<_>>(), ["name", "xp"]);
    let plain = field(&shape(serde_json::json!({ "object": "any" })));
    assert!(!plain.is_select() && !plain.custom);
    assert_ne!(plain.pin, circuit_core::PinMode::Inline, "Property can be connected");
}

/// Saving writes on the saving thread: the project counts as saved at once,
/// and a save that fails makes it unsaved again, keeping the workspace files
/// it was to delete for the next save.
#[test]
fn saving_happens_on_its_own_thread() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = std::env::temp_dir().join(format!("xc-saving-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut p = Project::create(dir.join("bot"), "S", "discord-bot").unwrap();
    p.workspaces.insert("other".into(), Workspace::new("Other"));
    p.save().unwrap();
    let mut ed = Editor::new(vec![root.join("plugins")], None).unwrap();
    ed.backups = circuit_core::backup::Backups::new(dir.join("backups"));
    ed.open_project(&dir.join("bot"));
    let other = ed.project.as_ref().unwrap().workspace_path("other");

    // Something in the way of the project file: the save fails.
    let meta = dir.join("bot").join(circuit_core::project::PROJECT_FILE);
    std::fs::remove_file(&meta).unwrap();
    std::fs::create_dir(&meta).unwrap();
    ed.project.as_mut().unwrap().remove_workspace("other");
    ed.save();
    assert!(!ed.unsaved_now(), "saved as far as the window knows, while it's written");
    ed.finish_saving();
    assert!(ed.unsaved_now(), "unsaved again once writing failed");
    assert!(ed.log.iter().any(|l| l.text.starts_with("Save failed")));
    assert_eq!(ed.project.as_ref().unwrap().removed, vec!["other".to_string()], "still to be deleted");
    assert!(other.exists() && ed.saves == 0);

    std::fs::remove_dir(&meta).unwrap();
    ed.save();
    ed.finish_saving();
    assert!(!ed.unsaved_now() && ed.saves == 1);
    assert!(meta.is_file() && !other.exists(), "written, and the removed workspace's file is gone");
    assert_eq!(ed.backups.list(&dir.join("bot")).len(), 1, "with a backup");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A block whose define() never ends is stopped a few times, then skipped
/// for its kind: many of them can't freeze the window. Reloading plugins
/// tries again.
#[test]
fn a_define_that_has_to_be_stopped_is_not_run_again() {
    let dir = std::env::temp_dir().join(format!("xc-slow-define-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let plugin = dir.join("plugins").join("t.slow");
    std::fs::create_dir_all(plugin.join("blocks")).unwrap();
    std::fs::write(plugin.join("plugin.toml"), "id = \"t.slow\"\nname = \"Slow\"\nversion = \"1.0.0\"\n\n[contributes]\nblocks = [\"blocks/*.js\"]\n").unwrap();
    std::fs::write(
        plugin.join("blocks").join("slow.js"),
        "module.exports = { id: 't.slow', kind: 'action', info: { name: 'Slow', version: '1.0.0' }, fields: { a: { type: 'text', default: '' } }, define() { while (true) {} }, run() {} };",
    )
    .unwrap();
    let mut ed = Editor::new(vec![dir.join("plugins")], None).unwrap();
    assert!(ed.reg.block("t.slow").is_some_and(|d| d.has_define), "{:?}", ed.log.iter().map(|l| &l.text).collect::<Vec<_>>());
    let mut ws = Workspace::new("W");
    for i in 0..10 {
        ws.nodes.insert(format!("{i}"), Node { block: "t.slow@1".into(), at: [0.0, i as f32 * 100.0], ..Default::default() });
    }
    let start = std::time::Instant::now();
    for id in ws.nodes.keys() {
        let shape = shapes::node_shape(&ed.reg, &ed.engine, &mut ed.shapes, "w", &ws, id, &ed.define_ctx).expect("a shape all the same");
        assert!(shape.fields.iter().any(|(k, _)| k == "a"), "with its fixed fields");
    }
    let took = start.elapsed();
    assert!(took < circuit_plugins::engine::BUDGET * (shapes::STOPPED_TIMES + 3), "stopped a few times, not ten: {took:?}");
    let reports = ed.shapes.take_reports();
    assert_eq!(reports.len(), 1, "said once: {reports:?}");
    assert!(reports[0].starts_with("Slow: its define() took longer than 50 ms 3 times in a row"));

    ed.reload_plugins();
    ed.shapes.begin();
    for _ in 0..shapes::STOPPED_TIMES {
        shapes::node_shape(&ed.reg, &ed.engine, &mut ed.shapes, "w", &ws, "0", &ed.define_ctx);
    }
    assert_eq!(ed.shapes.take_reports().len(), 1, "tried again after a reload");
    let _ = std::fs::remove_dir_all(&dir);
}

/// One define() that has to be stopped (the computer busy for a moment) only
/// counts towards stopping its kind: a call in time starts the count again,
/// and the block's shape is worked out anew, not kept without its pins.
#[test]
fn one_slow_define_doesnt_stop_its_kind() {
    let dir = std::env::temp_dir().join(format!("xc-hiccup-define-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let plugin = dir.join("plugins").join("t.hiccup");
    std::fs::create_dir_all(plugin.join("blocks")).unwrap();
    std::fs::write(plugin.join("plugin.toml"), "id = \"t.hiccup\"\nname = \"Hiccup\"\nversion = \"1.0.0\"\n\n[contributes]\nblocks = [\"blocks/*.js\"]\n").unwrap();
    // Slow only when told: its value `slow` is true.
    std::fs::write(
        plugin.join("blocks").join("hiccup.js"),
        "module.exports = { id: 't.hiccup', kind: 'action', info: { name: 'Hiccup', version: '1.0.0' }, fields: { slow: { type: 'boolean', default: false } }, define({ values }) { if (values.slow) { while (true) {} } return { add: { extra: { type: 'text', default: '' } } }; }, run() {} };",
    )
    .unwrap();
    let mut ed = Editor::new(vec![dir.join("plugins")], None).unwrap();
    let mut ws = Workspace::new("W");
    let node = |slow: bool| Node { block: "t.hiccup@1".into(), values: serde_json::json!({ "slow": slow }).as_object().cloned().unwrap(), ..Default::default() };
    ws.nodes.insert("a".into(), node(true));
    ws.nodes.insert("b".into(), node(false));
    let has_extra = |ed: &mut Editor, id: &str| {
        ed.shapes.begin();
        shapes::node_shape(&ed.reg, &ed.engine, &mut ed.shapes, "w", &ws, id, &ed.define_ctx).unwrap().fields.iter().any(|(k, _)| k == "extra")
    };
    for _ in 0..shapes::STOPPED_TIMES - 1 {
        assert!(!has_extra(&mut ed, "a"), "stopped: no pins of its own this time");
        assert!(has_extra(&mut ed, "b"), "one in time: the count starts again");
    }
    assert!(has_extra(&mut ed, "b"));
    assert!(ed.shapes.take_reports().is_empty(), "never stopped {} times in a row", shapes::STOPPED_TIMES);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Block source shows a block's file in its folder: Explorer gets the path
/// with backslashes, after its switch for selecting a file.
#[test]
fn a_file_is_shown_in_its_folder_not_opened() {
    let args = explorer_args(std::path::Path::new("C:/Apps/XC Circuit/plugins/xc.core/blocks/log.js"));
    assert_eq!(args, ["/select,", r"C:\Apps\XC Circuit\plugins\xc.core\blocks\log.js"]);
}

/// Links that plugins give open only when they are web pages: the system
/// would open a program or a file just as readily.
#[test]
fn only_web_pages_count_as_links() {
    for url in ["https://discord.com/developers/applications/123/bot", "http://localhost:3000/docs", "HTTPS://Example.com"] {
        assert!(is_web_link(url), "{url}");
    }
    for url in ["", "https://", "https:///etc/passwd", "calc.exe", "C:/Windows/System32/calc.exe", "file:///C:/Windows/System32/calc.exe", "//server/share/run.exe", "javascript:alert(1)", "ms-settings:privacy", "https://example.com/a b", " https://example.com"] {
        assert!(!is_web_link(url), "{url}");
    }
}

/// A project's own plugins load once the user said yes to them, for this
/// folder: not before, not in place of one of the app's plugins, and the
/// app's own examples need no asking.
#[test]
fn a_project_s_own_plugins_wait_for_a_yes() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = std::env::temp_dir().join(format!("xc-own-plugins-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let bot = dir.join("bot");
    Project::create(&bot, "Own", "none").unwrap();
    let plugin = |folder: &str, id: &str, name: &str, version: &str, block: &str| {
        let p = bot.join("plugins").join(folder);
        std::fs::create_dir_all(p.join("blocks")).unwrap();
        std::fs::write(p.join("plugin.toml"), format!("id = \"{id}\"\nname = \"{name}\"\nversion = \"{version}\"\n\n[contributes]\nblocks = [\"blocks/*.js\"]\n")).unwrap();
        std::fs::write(p.join("blocks").join("a.js"), format!("module.exports = {{ id: '{block}', kind: 'action', info: {{ name: 'A', version: '1.0.0' }}, run() {{}} }};")).unwrap();
    };
    plugin("t.own", "t.own", "Own Blocks", "1.0.0", "t.own_block");

    let mut ed = Editor::new(vec![root.join("plugins")], None).unwrap();
    ed.open_project(&bot);
    let waiting = |ed: &Editor| ed.plugins_waiting.iter().map(|o| o.name.clone()).collect::<Vec<_>>();
    assert!(ed.reg.block("t.own_block").is_none(), "not loaded before the yes");
    assert!(ed.reg.plugins.iter().all(|p| p.manifest.id != "t.own"), "not read at all");
    assert_eq!(waiting(&ed), ["Own Blocks"]);
    assert!(ed.plugin_ask, "the question is open");
    assert!(crate::pages::panels::collect_problems(&mut ed).iter().any(|(_, m, _)| m.contains("aren't loaded: Own Blocks")));
    // Not now: reloading plugins doesn't ask again, and still doesn't load them.
    ed.plugin_ask = false;
    ed.reload_plugins();
    assert!(!ed.plugin_ask && ed.reg.block("t.own_block").is_none());

    // Yes: they load, and stay loaded when the project opens again.
    ed.trust_own_plugins();
    ed.reload_plugins();
    assert!(ed.reg.block("t.own_block").is_some());
    assert!(waiting(&ed).is_empty() && !ed.plugin_ask);
    ed.close_project();
    ed.open_project(&bot);
    assert!(ed.reg.block("t.own_block").is_some() && !ed.plugin_ask);

    // One that arrives later (a pull) is asked about; the others stay loaded.
    plugin("t.later", "t.later", "Later Blocks", "1.0.0", "t.later_block");
    ed.reload_plugins();
    assert_eq!(waiting(&ed), ["Later Blocks"]);
    assert!(ed.plugin_ask && ed.reg.block("t.own_block").is_some() && ed.reg.block("t.later_block").is_none());

    // One with the id of a plugin that comes with the app never loads.
    ed.bundled_plugins = vec!["xc.core".into()];
    plugin("core-copy", "xc.core", "Core Copy", "99.0.0", "t.fake_core_block");
    ed.trust_own_plugins();
    ed.reload_plugins();
    assert_eq!(ed.plugins_refused.iter().map(|o| o.name.as_str()).collect::<Vec<_>>(), ["Core Copy"]);
    assert!(ed.reg.block("t.fake_core_block").is_none() && ed.reg.block("t.later_block").is_some());
    assert!(ed.reg.plugins.iter().any(|p| p.manifest.id == "xc.core" && p.manifest.version != "99.0.0"), "the app's own Core is the one loaded");
    assert!(ed.reg.problems.iter().any(|p| p.message.contains("can't take the place of one of the app's")), "{:?}", ed.reg.problems);

    // Another folder with the same plugins has to be said yes to itself.
    let copy = dir.join("copy");
    std::fs::create_dir_all(&copy).unwrap();
    for part in [circuit_core::project::PROJECT_FILE, "workspaces", "plugins"] {
        copy_all(&bot.join(part), &copy.join(part));
    }
    ed.open_project(&copy);
    assert!(ed.plugin_ask && ed.reg.block("t.own_block").is_none());

    // The app's own examples came with the app: DemoBot's plugin just loads.
    let mut ed = Editor::new(vec![root.join("plugins")], None).unwrap();
    ed.open_project(&root.join("examples/DemoBot"));
    assert!(ed.plugins_waiting.is_empty() && !ed.plugin_ask);
    assert!(ed.reg.plugins.iter().any(|p| p.manifest.id == "demo.custom"));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Copies a file, or a folder with what's in it.
fn copy_all(from: &std::path::Path, to: &std::path::Path) {
    if from.is_dir() {
        std::fs::create_dir_all(to).unwrap();
        for e in std::fs::read_dir(from).unwrap().flatten() {
            copy_all(&e.path(), &to.join(e.file_name()));
        }
    } else {
        std::fs::copy(from, to).unwrap();
    }
}
