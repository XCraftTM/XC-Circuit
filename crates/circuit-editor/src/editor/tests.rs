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
        for (text, _, at) in crate::pages::panels::collect_problems(&ed) {
            found.push(format!("{} {at:?}: {text}", dir.file_name().unwrap().to_string_lossy()));
        }
    }
    if std::env::var_os("XC_LIST_TEMPLATE_PROBLEMS").is_some() {
        println!("{}", found.join("\n"));
    }
    // Their descriptions say to fill these in.
    let left_to_you = ["› Add Role", "› Find Channel"];
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
