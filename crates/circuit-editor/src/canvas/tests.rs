use super::*;

/// Runs one frame of a workspace canvas headlessly.
fn frame(ctx: &egui::Context, ed: &mut Editor, ws: &str, events: Vec<egui::Event>) {
    let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1400.0, 900.0))), events, ..Default::default() };
    let mut out = ctx.run_ui(raw, |ui| show(ui, ed, ws));
    out.textures_delta.clear();
}

/// How long a frame takes with many blocks, most of them out of view:
/// cargo test --release -p circuit-editor frame_time -- --ignored --nocapture
#[test]
#[ignore]
fn frame_time_with_many_blocks() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut ed = Editor::new(vec![root.join("plugins")], None).unwrap();
    ed.open_project(&root.join("examples/DemoBot"));
    // 400 Send Message blocks in a 20×20 grid; the view shows a corner.
    let ws = ed.project.as_mut().unwrap().workspaces.get_mut("rank-command").unwrap();
    for i in 0..400 {
        let id = format!("m{i}");
        let mut values = serde_json::Map::new();
        values.insert("text".into(), Value::String(format!("Hello ${{user}} number {i}")));
        ws.nodes.insert(id, circuit_core::Node { block: "discord.send_message@3".into(), at: [(i % 20) as f32 * 420.0, 800.0 + (i / 20) as f32 * 420.0], values, ..Default::default() });
    }
    let ctx = egui::Context::default();
    for _ in 0..5 {
        frame(&ctx, &mut ed, "rank-command", vec![]);
    }
    let start = std::time::Instant::now();
    let n = 30;
    for i in 0..n {
        // The pointer moving: what a frame costs while using the canvas.
        frame(&ctx, &mut ed, "rank-command", vec![egui::Event::PointerMoved(pos2(300.0 + i as f32, 300.0))]);
    }
    println!("frame: {:.2} ms", start.elapsed().as_secs_f64() * 1000.0 / n as f64);
}

#[test]
fn typing_a_brace_offers_earlier_values() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut ed = Editor::new(vec![root.join("plugins")], None).unwrap();
    ed.open_project(&root.join("examples/DemoBot"));
    let ctx = egui::Context::default();
    let settle = |ed: &mut Editor| {
        for _ in 0..6 {
            frame(&ctx, ed, "rank-command", vec![]);
        }
    };
    settle(&mut ed);
    let field = ("4".to_string(), "text".to_string());
    let rect = ed.views["rank-command"].text_fields.iter().find(|(k, _)| *k == field).map(|(_, r)| *r).unwrap();
    let p = rect.center();
    let press = |pressed: bool| egui::Event::PointerButton { pos: p, button: PointerButton::Primary, pressed, modifiers: Default::default() };
    frame(&ctx, &mut ed, "rank-command", vec![egui::Event::PointerMoved(p), press(true), press(false)]);
    settle(&mut ed);
    let key = |key: Key| egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() };
    frame(&ctx, &mut ed, "rank-command", vec![key(Key::End), key(Key::Enter)]);
    frame(&ctx, &mut ed, "rank-command", vec![egui::Event::Text("Hi ${us".into())]);
    settle(&mut ed);
    // The list is open; Enter puts in the first match instead of a new line.
    frame(&ctx, &mut ed, "rank-command", vec![key(Key::Enter)]);
    settle(&mut ed);
    let text = ed.workspace("rank-command").unwrap().nodes["4"].values["text"].as_str().unwrap().to_string();
    assert!(text.ends_with("
Hi ${#1.user}"), "values go in with the block's number: {text:?}");
    // Typing goes on after the placeholder.
    frame(&ctx, &mut ed, "rank-command", vec![egui::Event::Text("!".into())]);
    let text = ed.workspace("rank-command").unwrap().nodes["4"].values["text"].as_str().unwrap().to_string();
    assert!(text.ends_with("${#1.user}!"), "{text:?}");

    // Clicking a value in the list picks it too (pressing takes the text
    // cursor from the box for a moment).
    frame(&ctx, &mut ed, "rank-command", vec![egui::Event::Text(" ${us".into())]);
    settle(&mut ed);
    let list = crate::editor::values::open_list_rect(&ctx).unwrap_or(Rect::NOTHING);
    assert!(list.is_positive(), "the list is open");
    let row = pos2(list.left() + 60.0, list.top() + 40.0);
    let at = |pressed: bool| egui::Event::PointerButton { pos: row, button: PointerButton::Primary, pressed, modifiers: Default::default() };
    frame(&ctx, &mut ed, "rank-command", vec![egui::Event::PointerMoved(row)]);
    frame(&ctx, &mut ed, "rank-command", vec![at(true)]);
    frame(&ctx, &mut ed, "rank-command", vec![at(false)]);
    settle(&mut ed);
    let text = ed.workspace("rank-command").unwrap().nodes["4"].values["text"].as_str().unwrap().to_string();
    assert!(text.ends_with("${#1.user}! ${#1.user}"), "{text:?}");
    frame(&ctx, &mut ed, "rank-command", vec![egui::Event::Text("?".into())]);
    let text = ed.workspace("rank-command").unwrap().nodes["4"].values["text"].as_str().unwrap().to_string();
    assert!(text.ends_with("${#1.user}?"), "the text cursor is back after it: {text:?}");
}

#[test]
fn a_brace_lists_blocks_then_values_then_members() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut ed = Editor::new(vec![root.join("plugins")], None).unwrap();
    ed.open_project(&root.join("examples/DemoBot"));
    let ctx = egui::Context::default();
    let settle = |ed: &mut Editor| {
        for _ in 0..12 {
            frame(&ctx, ed, "rank-command", vec![]);
        }
    };
    settle(&mut ed);
    let field = ("4".to_string(), "text".to_string());
    let p = ed.views["rank-command"].text_fields.iter().find(|(k, _)| *k == field).map(|(_, r)| r.center()).unwrap();
    let press = |pressed: bool| egui::Event::PointerButton { pos: p, button: PointerButton::Primary, pressed, modifiers: Default::default() };
    frame(&ctx, &mut ed, "rank-command", vec![egui::Event::PointerMoved(p), press(true), press(false)]);
    settle(&mut ed);
    let key = |key: Key| egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() };
    let text = |ed: &Editor| ed.workspace("rank-command").unwrap().nodes["4"].values["text"].as_str().unwrap().to_string();
    let step = |ed: &mut Editor, events: Vec<egui::Event>| {
        frame(&ctx, ed, "rank-command", events);
        settle(ed);
    };
    step(&mut ed, vec![key(Key::End)]);

    // { lists the blocks; Enter goes into the first (the Slash Command), Enter
    // again puts in its first value.
    step(&mut ed, vec![egui::Event::Text(" ${".into())]);
    step(&mut ed, vec![key(Key::Enter)]);
    step(&mut ed, vec![key(Key::Enter)]);
    assert!(text(&ed).ends_with(" ${#1.interaction}"), "{:?}", text(&ed));

    // → goes into a value's members, ← back out, Enter puts the value in.
    step(&mut ed, vec![egui::Event::Text(" ${".into())]);
    step(&mut ed, vec![key(Key::Enter)]);
    step(&mut ed, vec![key(Key::ArrowDown)]);
    step(&mut ed, vec![key(Key::ArrowRight)]);
    step(&mut ed, vec![key(Key::ArrowLeft)]);
    step(&mut ed, vec![key(Key::Enter)]);
    assert!(text(&ed).ends_with(" ${#1.user}"), "{:?}", text(&ed));

    // After a dot: the members of that type, from discord.js.
    step(&mut ed, vec![egui::Event::Text(" ${#1.user.glob".into())]);
    step(&mut ed, vec![key(Key::Enter)]);
    assert!(text(&ed).ends_with(" ${#1.user.globalName}"), "{:?}", text(&ed));
}

#[test]
fn example_action_paths_dont_split() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut dirs = vec![root.join("examples/DemoBot")];
    dirs.extend(std::fs::read_dir(root.join("examples/templates")).unwrap().map(|e| e.unwrap().path()));
    for dir in dirs {
        let project = circuit_core::Project::load(&dir).unwrap();
        for (key, ws) in &project.workspaces {
            assert!(ws.split_paths().is_empty(), "{} {key}: {:?}", dir.display(), ws.split_paths());
        }
    }
}

#[test]
fn blocks_in_the_examples_dont_overlap() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut dirs = vec![root.join("examples/DemoBot")];
    dirs.extend(std::fs::read_dir(root.join("examples/templates")).unwrap().map(|e| e.unwrap().path()));
    let mut overlaps = Vec::new();
    for dir in dirs {
        let mut ed = Editor::new(vec![root.join("plugins")], None).unwrap();
        ed.open_project(&dir);
        let keys: Vec<String> = ed.project.as_ref().unwrap().workspaces.keys().cloned().collect();
        for ws in keys {
            let ctx = egui::Context::default();
            ed.ui_rects.clear();
            for _ in 0..3 {
                frame(&ctx, &mut ed, &ws, vec![]);
            }
            let nodes: Vec<(&String, &Rect)> = ed.ui_rects.iter().filter(|(k, _)| k.starts_with("node:")).collect();
            for (i, (a, ra)) in nodes.iter().enumerate() {
                for (b, rb) in &nodes[i + 1..] {
                    if ra.shrink(1.0).intersects(**rb) {
                        overlaps.push(format!("{} {ws}: {a} and {b}", dir.file_name().unwrap().to_string_lossy()));
                    }
                }
            }
        }
    }
    assert!(overlaps.is_empty(), "blocks overlap:
{}", overlaps.join("
"));
}

/// Moves overlapping blocks in the examples down until they fit:
/// `cargo test -p circuit-editor -- --ignored spread_example_blocks`.
#[test]
#[ignore]
fn spread_example_blocks() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut dirs = vec![root.join("examples/DemoBot")];
    dirs.extend(std::fs::read_dir(root.join("examples/templates")).unwrap().map(|e| e.unwrap().path()));
    for dir in dirs {
        let mut ed = Editor::new(vec![root.join("plugins")], None).unwrap();
        ed.open_project(&dir);
        let keys: Vec<String> = ed.project.as_ref().unwrap().workspaces.keys().cloned().collect();
        for ws in keys {
            let ctx = egui::Context::default();
            ed.ui_rects.clear();
            for _ in 0..3 {
                frame(&ctx, &mut ed, &ws, vec![]);
            }
            let zoom = ed.views[&ws].zoom;
            let w = ed.workspace(&ws).unwrap();
            // Scene rectangles, top to bottom.
            let mut rects: Vec<(String, Rect)> = w
                .nodes
                .iter()
                .filter_map(|(id, n)| ed.ui_rects.get(&format!("node:{id}")).map(|r| (id.clone(), Rect::from_min_size(pos2(n.at[0], n.at[1]), r.size() / zoom))))
                .collect();
            rects.sort_by(|a, b| a.1.top().total_cmp(&b.1.top()));
            let mut placed: Vec<Rect> = Vec::new();
            let mut moved = Vec::new();
            for (id, mut r) in rects {
                while let Some(hit) = placed.iter().find(|p| p.expand(12.0).intersects(r)) {
                    r = r.translate(vec2(0.0, hit.bottom() + 30.0 - r.top()));
                }
                placed.push(r);
                moved.push((id, r.min));
            }
            let w = ed.project.as_mut().unwrap().workspaces.get_mut(&ws).unwrap();
            for (id, at) in moved {
                w.nodes.get_mut(&id).unwrap().at = [at.x, at.y];
            }
        }
        ed.project.as_mut().unwrap().save().unwrap();
    }
}

#[test]
fn arranging_lines_blocks_up_and_spreads_them() {
    let r = |x: f32, y: f32, w: f32, h: f32| Rect::from_min_size(pos2(x, y), vec2(w, h));
    let rects = vec![("a".to_string(), r(0.0, 0.0, 100.0, 50.0)), ("b".to_string(), r(300.0, 40.0, 200.0, 80.0)), ("c".to_string(), r(120.0, 400.0, 100.0, 60.0))];
    let at = |how| arrange(&rects, how).into_iter().collect::<HashMap<String, Pos2>>();
    let left = at(Arrange::Left);
    assert!(left.values().all(|p| p.x == 0.0) && left["b"].y == 40.0);
    let right = at(Arrange::Right);
    assert_eq!((right["a"].x, right["b"].x), (400.0, 300.0), "right edges at 500");
    let middle = at(Arrange::Middle);
    assert_eq!(middle["a"].y, 230.0 - 25.0, "centred on the middle of everything (0..460)");
    // Spread across: first stays, last ends where it ended, equal gaps.
    let across = at(Arrange::SpreadAcross);
    assert_eq!((across["a"].x, across["c"].x, across["b"].x), (0.0, 150.0, 300.0));
    // Too tight for equal gaps: they keep some room instead of overlapping.
    let tight = vec![("a".to_string(), r(0.0, 0.0, 100.0, 50.0)), ("b".to_string(), r(50.0, 0.0, 100.0, 50.0))];
    let p = arrange(&tight, Arrange::SpreadAcross);
    assert_eq!(p[1].1.x, 100.0 + GRID * 2.0);
    assert_eq!((snap(10.0), snap(12.0), snap(-30.0)), (0.0, GRID, -GRID));
}

#[test]
fn notes_move_with_their_blocks_and_resize() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut ed = Editor::new(vec![root.join("plugins")], None).unwrap();
    ed.open_project(&root.join("examples/DemoBot"));
    let mut ws = Workspace::new("Notes");
    ws.nodes.insert("1".into(), circuit_core::Node { block: "core.log@1".into(), at: [100.0, 140.0], ..Default::default() });
    ws.nodes.insert("2".into(), circuit_core::Node { block: "core.log@1".into(), at: [900.0, 140.0], ..Default::default() });
    ws.notes.insert("n1".into(), Note { title: "Hello".into(), text: "A note".into(), rect: [60.0, 40.0, 500.0, 400.0], color: None, loose: false });
    ed.project.as_mut().unwrap().workspaces.insert("notes".into(), ws);
    let ctx = egui::Context::default();
    let settle = |ed: &mut Editor| {
        for _ in 0..4 {
            frame(&ctx, ed, "notes", vec![]);
        }
    };
    settle(&mut ed);
    // Scene to screen, as the canvas draws it.
    let screen = |ed: &Editor, p: Pos2| {
        let v = &ed.views["notes"];
        v.last_origin.unwrap().0 + v.offset + p.to_vec2() * v.zoom
    };
    let drag = |ed: &mut Editor, from: Pos2, by: Vec2, alt: bool| {
        let modifiers = egui::Modifiers { alt, ..Default::default() };
        let press = |pos: Pos2, pressed: bool| egui::Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers };
        for _ in 0..3 {
            frame(&ctx, ed, "notes", vec![egui::Event::PointerMoved(from)]);
        }
        frame(&ctx, ed, "notes", vec![egui::Event::ModifiersChanged(modifiers), press(from, true)]);
        // Steps bigger than egui's drag threshold, so none is taken for a click.
        for step in 1..=10 {
            frame(&ctx, ed, "notes", vec![egui::Event::PointerMoved(from + by * (step as f32 / 10.0))]);
        }
        frame(&ctx, ed, "notes", vec![press(from + by, false), egui::Event::ModifiersChanged(Default::default())]);
        settle(ed);
    };
    let note = |ed: &Editor| ed.workspace("notes").unwrap().notes["n1"].rect;
    let at = |ed: &Editor, id: &str| ed.workspace("notes").unwrap().nodes[id].at;
    let zoom = ed.views["notes"].zoom;

    // The title band moves the note and the block inside it, not the one outside.
    let from = screen(&ed, pos2(140.0, 60.0));
    drag(&mut ed, from, vec2(100.0, 50.0), false);
    let moved = vec2(100.0, 50.0) / zoom;
    assert!((note(&ed)[0] - (60.0 + moved.x)).abs() < 1.0 && (note(&ed)[1] - (40.0 + moved.y)).abs() < 1.0, "{:?}", note(&ed));
    assert!((at(&ed, "1")[0] - (100.0 + moved.x)).abs() < 1.0, "the block inside moves along: {:?}", at(&ed, "1"));
    assert_eq!(at(&ed, "2"), [900.0, 140.0], "the block outside stays");

    // With Alt, only the note moves.
    let r = note(&ed);
    let from = screen(&ed, pos2(r[0] + 80.0, r[1] + 20.0));
    drag(&mut ed, from, vec2(-100.0, 0.0), true);
    assert!((note(&ed)[0] - (r[0] - 100.0 / zoom)).abs() < 1.0, "{r:?} -> {:?}", note(&ed));
    assert!((at(&ed, "1")[0] - (100.0 + moved.x)).abs() < 1.0, "the block stays: {:?}", at(&ed, "1"));

    // A note set not to move its blocks moves alone; with Alt, it takes them along.
    ed.project.as_mut().unwrap().workspaces.get_mut("notes").unwrap().notes.get_mut("n1").unwrap().loose = true;
    let (r, b) = (note(&ed), at(&ed, "1"));
    let from = screen(&ed, pos2(r[0] + 80.0, r[1] + 20.0));
    drag(&mut ed, from, vec2(100.0, 0.0), false);
    assert!((note(&ed)[0] - (r[0] + 100.0 / zoom)).abs() < 1.0, "{r:?} -> {:?}", note(&ed));
    assert_eq!(at(&ed, "1"), b, "the block stays");
    let r = note(&ed);
    let from = screen(&ed, pos2(r[0] + 80.0, r[1] + 20.0));
    drag(&mut ed, from, vec2(-100.0, 0.0), true);
    assert!((at(&ed, "1")[0] - (b[0] - 100.0 / zoom)).abs() < 1.0, "with Alt the block moves: {:?}", at(&ed, "1"));
    ed.project.as_mut().unwrap().workspaces.get_mut("notes").unwrap().notes.get_mut("n1").unwrap().loose = false;

    // The bottom-right corner resizes it; it doesn't get smaller than its minimum.
    let r = note(&ed);
    let from = screen(&ed, pos2(r[0] + r[2], r[1] + r[3]));
    drag(&mut ed, from, vec2(100.0, 80.0), false);
    assert!((note(&ed)[2] - (r[2] + 100.0 / zoom)).abs() < 1.5 && (note(&ed)[3] - (r[3] + 80.0 / zoom)).abs() < 1.5, "{:?}", note(&ed));
    let r = note(&ed);
    let from = screen(&ed, pos2(r[0], r[1] + r[3] / 2.0));
    drag(&mut ed, from, vec2(2000.0, 0.0), false);
    assert!((note(&ed)[2] - NOTE_MIN.x).abs() < 0.5, "{:?}", note(&ed));
    assert!((note(&ed)[0] + note(&ed)[2] - (r[0] + r[2])).abs() < 0.5, "the right edge stays put");

    // It's saved with the workspace.
    let json = serde_json::to_value(ed.workspace("notes").unwrap()).unwrap();
    assert_eq!(json["notes"]["n1"]["title"], "Hello");

    // A click on its title selects it, and lets go of selected blocks.
    ed.selected = Some(("notes".into(), "2".into()));
    ed.multi = std::iter::once("2".to_string()).collect();
    let r = note(&ed);
    let p = screen(&ed, pos2(r[0] + 20.0, r[1] + 20.0));
    let press = |pressed: bool| egui::Event::PointerButton { pos: p, button: PointerButton::Primary, pressed, modifiers: Default::default() };
    frame(&ctx, &mut ed, "notes", vec![egui::Event::PointerMoved(p)]);
    frame(&ctx, &mut ed, "notes", vec![press(true), press(false)]);
    settle(&mut ed);
    assert_eq!(ed.note_selection("notes"), vec!["n1".to_string()]);
    assert!(ed.selected.is_none() && ed.multi.is_empty());

    // Duplicate copies it; Delete removes the selected ones; Ctrl+A selects them all.
    ed.active_workspace = Some("notes".into());
    ed.duplicate();
    let ws = ed.workspace("notes").unwrap();
    assert_eq!((ws.notes.len(), ws.notes["n2"].title.as_str()), (2, "Hello"));
    assert_eq!(ed.note_selection("notes"), vec!["n2".to_string()], "the copy is selected");
    ed.delete_selection();
    assert_eq!(ed.workspace("notes").unwrap().notes.len(), 1);
    ed.select_all();
    assert_eq!(ed.note_selection("notes"), vec!["n1".to_string()]);
    assert_eq!(ed.multi.len(), 2);
}

#[test]
fn text_fields_keep_focus_and_their_size() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut ed = Editor::new(vec![root.join("plugins")], None).unwrap();
    ed.open_project(&root.join("examples/DemoBot"));
    let ctx = egui::Context::default();
    let settle = |ed: &mut Editor| {
        for _ in 0..6 {
            frame(&ctx, ed, "rank-command", vec![]);
        }
    };
    settle(&mut ed);
    let field = ("4".to_string(), "text".to_string());
    let box_of = |ed: &Editor| ed.views["rank-command"].text_fields.iter().find(|(k, _)| *k == field).map(|(_, r)| *r).unwrap();
    let text = |ed: &Editor| ed.workspace("rank-command").unwrap().nodes["4"].values["text"].as_str().unwrap().to_string();
    let press = |pos: Pos2, pressed: bool| egui::Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Default::default() };

    // The box is several lines tall from the start (this one is for long text).
    let before = box_of(&ed);
    let zoom = ed.views["rank-command"].zoom;
    assert!((before.height() - TEXT_AREA_H * zoom).abs() < 1.0, "starts {} px tall", before.height());

    // Clicking into it and typing, then typing again later, all goes in.
    let p = before.center();
    frame(&ctx, &mut ed, "rank-command", vec![egui::Event::PointerMoved(p), press(p, true), press(p, false)]);
    settle(&mut ed);
    frame(&ctx, &mut ed, "rank-command", vec![egui::Event::Text("!!".into())]);
    settle(&mut ed);
    frame(&ctx, &mut ed, "rank-command", vec![egui::Event::Text("?".into())]);
    assert!(text(&ed).ends_with("!!?"), "typing stays in the box: {}", text(&ed));

    // Dragging the corner grip down and right makes it taller and wider, and the block with it.
    let grip = before.right_bottom() - vec2(8.0, 8.0);
    for _ in 0..60 {
        frame(&ctx, &mut ed, "rank-command", vec![egui::Event::PointerMoved(grip)]);
    }
    frame(&ctx, &mut ed, "rank-command", vec![press(grip, true)]);
    for step in 1..=10 {
        frame(&ctx, &mut ed, "rank-command", vec![egui::Event::PointerMoved(grip + vec2(8.0 * step as f32, 12.0 * step as f32))]);
    }
    frame(&ctx, &mut ed, "rank-command", vec![press(grip + vec2(80.0, 120.0), false)]);
    settle(&mut ed);
    let after = box_of(&ed);
    assert!((after.height() - before.height() - 120.0).abs() < 3.0, "grew {} px", after.height() - before.height());
    assert!((after.width() - before.width() - 80.0).abs() < 3.0, "grew {} px wider", after.width() - before.width());
    let node = &ed.workspace("rank-command").unwrap().nodes["4"];
    assert!((node.heights["text"] * zoom - after.height()).abs() < 2.0, "the height is kept with the block: {}", node.heights["text"]);
    assert!(node.width.is_some_and(|w| (w - NODE_W - 80.0 / zoom).abs() < 3.0), "so is the width: {:?}", node.width);

    // Double-clicking the grip puts the size back.
    let grip = after.right_bottom() - vec2(8.0, 8.0);
    for _ in 0..60 {
        frame(&ctx, &mut ed, "rank-command", vec![egui::Event::PointerMoved(grip)]);
    }
    for pressed in [true, false, true, false] {
        frame(&ctx, &mut ed, "rank-command", vec![press(grip, pressed)]);
    }
    settle(&mut ed);
    let node = &ed.workspace("rank-command").unwrap().nodes["4"];
    assert!(!node.heights.contains_key("text") && node.width.is_none(), "the size is reset");
    assert!((box_of(&ed).size() - before.size()).length() < 1.0);
}

#[test]
fn toolbar_buttons_show_the_pointer() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut ed = Editor::new(vec![root.join("plugins")], None).unwrap();
    ed.open_project(&root.join("examples/DemoBot"));
    let ctx = egui::Context::default();
    ed.theme.apply(&ctx);
    // Add block is the first button of the toolbar, bottom left.
    let over = pos2(60.0, 900.0 - 12.0 - 17.0);
    let mut cursor = egui::CursorIcon::Default;
    for _ in 0..4 {
        let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1400.0, 900.0))), events: vec![egui::Event::PointerMoved(over)], ..Default::default() };
        let mut out = ctx.run_ui(raw, |ui| show(ui, &mut ed, "rank-command"));
        out.textures_delta.clear();
        cursor = out.platform_output.cursor_icon;
    }
    assert_eq!(cursor, egui::CursorIcon::PointingHand);
}

#[test]
fn wire_hit_test() {
    let (a, b) = (pos2(0.0, 0.0), pos2(200.0, 100.0));
    assert!(wire_distance(a, b, pos2(2.0, 1.0)) < 3.0, "near the start");
    assert!(wire_distance(a, b, pos2(100.0, 50.0)) < 3.0, "the middle of an S curve");
    assert!(wire_distance(a, b, pos2(100.0, 0.0)) > 10.0);
    assert_eq!(wire_distance(a, b, pos2(500.0, 500.0)), f32::INFINITY);
}

/// Send Message shows its Files on the block; an advanced input (Components)
/// shows there once it's a pin, so it can be connected.
#[test]
fn files_and_pinned_advanced_inputs_show_on_the_block() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let ed = Editor::new(vec![root.join("plugins")], None).unwrap();
    let def = ed.reg.block("discord.send_message").unwrap();
    let values = serde_json::json!({ "mode": "send" });
    let shape = circuit_core::BlockShape::build(def, values.as_object().unwrap(), None);
    let keys = |pins: &[String]| {
        let l = layout::layout_node("1", [0.0, 0.0], pins, None, Some(def), Some(&shape), &Default::default(), &Default::default());
        l.rows.iter().filter_map(|(r, _)| if let layout::RowKind::Field { key, .. } = r { Some(key.clone()) } else { None }).collect::<Vec<_>>()
    };
    let plain = keys(&[]);
    assert!(plain.contains(&"files".to_string()) && plain.contains(&"components".to_string()), "buttons and menus can be connected: {plain:?}");
    // An advanced input shows once its pin is turned on: Show Modal's Inputs.
    let def = ed.reg.block("discord.show_modal").unwrap();
    let shape = circuit_core::BlockShape::build(def, &Default::default(), None);
    let keys = |pins: &[String]| {
        let l = layout::layout_node("1", [0.0, 0.0], pins, None, Some(def), Some(&shape), &Default::default(), &Default::default());
        l.rows.iter().filter_map(|(r, _)| if let layout::RowKind::Field { key, .. } = r { Some(key.clone()) } else { None }).collect::<Vec<_>>()
    };
    assert!(!keys(&[]).contains(&"inputs".to_string()));
    assert!(keys(&["inputs".to_string()]).contains(&"inputs".to_string()));
}

/// The glow of a missing required input pulses between faint and full, once every 1.5 seconds.
#[test]
fn missing_inputs_pulse() {
    let at = |t: f64| super::glow_strength(t);
    assert!((0..60).map(|i| at(i as f64 * 0.05)).all(|s| (0.0..=1.0).contains(&s)));
    assert!((at(0.375) - 1.0).abs() < 1e-3 && at(1.125) < 1e-3, "full a quarter in, faint three quarters in");
    assert!((at(0.2) - at(1.7)).abs() < 1e-3, "it repeats");
}

/// Blocks with a message preview get as wide as the message, as Discord
/// sizes it: a short one keeps the usual width, a row of buttons gets room,
/// and long text stops at Discord's widest. A width set by hand wins.
#[test]
fn preview_blocks_get_as_wide_as_their_message() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut ed = Editor::new(vec![root.join("plugins")], None).unwrap();
    ed.open_project(&root.join("examples/DemoBot"));
    let button = |label: &str| serde_json::json!({ "type": 2, "style": 1, "label": label, "custom_id": label });
    let layouts = [
        ("short", serde_json::json!({ "components": [{ "type": 10, "content": "Hi" }] })),
        ("buttons", serde_json::json!({ "components": [{ "type": 1, "components": [button("Close ticket"), button("Transcript"), button("Claim ticket")] }] })),
        ("long", serde_json::json!({ "components": [{ "type": 10, "content": "A very long line of text that would not fit into any block on one line, so it wraps where Discord wraps it." }] })),
    ];
    let ws = ed.project.as_mut().unwrap().workspaces.get_mut("rank-command").unwrap();
    ws.nodes.clear();
    ws.links.clear();
    for (i, (id, value)) in layouts.iter().enumerate() {
        let mut values = serde_json::Map::new();
        values.insert("layout".into(), value.clone());
        ws.nodes.insert(id.to_string(), circuit_core::Node { block: "discord.build_layout@1".into(), at: [0.0, i as f32 * 600.0], values, ..Default::default() });
    }
    let mut values = serde_json::Map::new();
    values.insert("layout".into(), layouts[2].1.clone());
    ws.nodes.insert("given".into(), circuit_core::Node { block: "discord.build_layout@1".into(), at: [0.0, 1800.0], values, width: Some(320.0), ..Default::default() });
    let ctx = egui::Context::default();
    for _ in 0..3 {
        frame(&ctx, &mut ed, "rank-command", vec![]);
    }
    let width = |id: &str| ed.views["rank-command"].block_rects().into_iter().find(|(k, _)| k == id).unwrap().1.width();
    assert_eq!(width("short"), 300.0, "a short message keeps the usual width");
    assert!(width("buttons") > 300.0 && width("buttons") < 508.0, "room for the buttons side by side: {}", width("buttons"));
    assert_eq!(width("long"), 508.0, "long text stops at Discord's widest");
    assert_eq!(width("given"), 320.0, "a width it was given stays");
}

/// Holding a dragged wire at an edge of the canvas moves the canvas that way;
/// away from the edges, or far off over other panels, nothing moves.
#[test]
fn a_wire_held_at_an_edge_moves_the_canvas() {
    let outer = Rect::from_min_size(pos2(100.0, 50.0), vec2(800.0, 600.0));
    assert_eq!(edge_push(outer, pos2(500.0, 350.0)), Vec2::ZERO, "the middle");
    assert_eq!(edge_push(outer, pos2(500.0, 50.0 + 41.0)), Vec2::ZERO, "just inside the band");
    let right = edge_push(outer, pos2(899.0, 350.0));
    assert!(right.x > 0.9 && right.y == 0.0, "at the right edge: {right:?}");
    let gentle = edge_push(outer, pos2(900.0 - 30.0, 350.0));
    assert!(gentle.x > 0.0 && gentle.x < 0.2, "the start of the band is gentle: {gentle:?}");
    assert_eq!(edge_push(outer, pos2(80.0, 30.0)), vec2(-1.0, -1.0), "past the top-left corner");
    assert_eq!(edge_push(outer, pos2(1300.0, 350.0)), Vec2::ZERO, "far off to the right");

    // The view: holding at the right edge shows more of the right.
    let mut view = CanvasView { zoom: 1.0, ..Default::default() };
    let before = view.offset;
    assert!(view.edge_pan(outer, pos2(905.0, 350.0), 1.0 / 60.0));
    assert!(view.offset.x < before.x && view.offset.y == before.y, "{:?}", view.offset);
    assert!((before.x - view.offset.x - 15.0).abs() < 0.01, "900 points a second");
    assert!(!view.edge_pan(outer, pos2(500.0, 350.0), 1.0 / 60.0));
}

/// Zoomed out, a note's title and description grow on the canvas so they
/// stay readable on screen; zoomed in, they keep their own size.
#[test]
fn note_texts_stay_readable_zoomed_out() {
    use super::notes::text_sizes;
    assert_eq!(text_sizes(1.0), (15.0, 12.5));
    assert_eq!(text_sizes(2.0), (15.0, 12.5), "zoomed in: their own size");
    let (title, desc) = text_sizes(0.25);
    assert_eq!((title * 0.25, desc * 0.25), (14.0, 11.0), "at 25%: 14 and 11 on screen");
}
