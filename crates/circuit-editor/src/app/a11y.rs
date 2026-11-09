//! Every control a user can act on has a name a screen reader can say (and
//! a test can find it by): the main screens are walked with egui_kittest's
//! accessibility check.

use super::*;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

/// The plugins next to this crate.
fn plugins_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins")
}

/// The whole window around a fresh editor in English; with a small project
/// (in a scratch folder) when `project` is set, else the start page shows.
fn window(name: &str, project: bool) -> Harness<'static, CircuitApp> {
    let mut ed = Editor::new(vec![plugins_dir()], None).unwrap();
    ed.prefs.language = crate::i18n::ENGLISH.into();
    ed.prefs.setup_done = true;
    ed.apply_language();
    ed.restyle();
    if project {
        let dir = std::env::temp_dir().join(format!("xc-a11y-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut p = Project::create(&dir, "Ping Bot", "discord-bot").unwrap();
        let ws = p.workspaces.values_mut().next().unwrap();
        let node = |block: &str, x: f32, values: serde_json::Value| circuit_core::Node { block: block.into(), at: [x, 0.0], values: values.as_object().cloned().unwrap_or_default(), ..Default::default() };
        ws.nodes.insert("a".into(), node("discord.on_message@1", 0.0, serde_json::Value::Null));
        // An embed with each of its parts, so the builder shows all of its inputs.
        let embed = serde_json::json!({ "embed": {
            "color": "#00D8F0", "title": "Hello", "description": "Text", "url": "https://example.com",
            "author": { "name": "Me", "url": "https://example.com", "icon_url": "https://example.com/a.png" },
            "footer": { "text": "Footer", "icon_url": "https://example.com/f.png" },
            "image": { "url": "https://example.com/i.png" }, "thumbnail": { "url": "https://example.com/t.png" },
            "fields": [{ "name": "Field", "value": "Value", "inline": true }],
        } });
        ws.nodes.insert("e".into(), node("discord.build_embed@2", 320.0, embed));
        ws.nodes.insert("l".into(), node("discord.build_layout@1", 640.0, serde_json::Value::Null));
        ws.nodes.insert("c".into(), node("core.build_canvas@1", 960.0, serde_json::Value::Null));
        // Two databases, for Bot › Settings › Databases (no passwords: nothing is connected).
        p.meta.settings.insert("databases".into(), serde_json::json!([
            { "id": "db1", "name": "Main", "kind": "mariadb", "host": "db.example.com", "user": "bot", "database": "levels", "ssl": true },
            { "id": "db2", "name": "Local", "kind": "sqlite", "file": "bot.sqlite" },
        ]));
        // A slash command with a translation and options of each sort.
        p.meta.data.insert(crate::builders::slash::DATA_KEY.into(), serde_json::json!([{
            "name": "rank", "description": "Shows your rank", "locales": { "de": { "name": "rang", "description": "Zeigt deinen Rang" } },
            "contexts": ["guild"], "installs": ["guild"],
            "options": [
                { "name": "user", "type": "string", "description": "Whose", "choices": [{ "name": "Me", "value": "me" }] },
                { "name": "count", "type": "integer", "description": "How many", "min": 1, "max": 10 },
                { "name": "where", "type": "channel", "description": "Where" },
            ],
        }]));
        p.save().unwrap();
        ed.open_project(&dir);
    }
    let bundled = Some(plugins_dir());
    let mut h = Harness::builder().with_accessibility_check(false).with_size(egui::vec2(1440.0, 900.0)).build_eframe(move |cc| {
        egui_extras::install_image_loaders(&cc.egui_ctx);
        let _ = crate::ui::fonts::install_with(&cc.egui_ctx, None);
        ed.theme.apply(&cc.egui_ctx);
        CircuitApp::with_editor(cc, ed, default_layout(), bundled)
    });
    settle(&mut h);
    h
}

fn settle(h: &mut Harness<'_, CircuitApp>) {
    h.run_steps(6);
}

/// What `where_` shows without a name, gathered over the walk.
fn look(h: &Harness<'_, CircuitApp>, where_: &str, missing: &mut Vec<String>) {
    for w in h.unnamed_widgets() {
        missing.push(format!("{where_}: {w}"));
    }
}

fn click(h: &mut Harness<'_, CircuitApp>, label: &str) {
    h.get_by_label(label).click();
    settle(h);
}

fn report(missing: Vec<String>) {
    assert!(missing.is_empty(), "{} control(s) without a name:\n{}", missing.len(), missing.join("\n"));
}

#[test]
fn the_first_start_page_names_its_controls() {
    let mut h = window("setup", false);
    let mut missing = Vec::new();
    h.state_mut().setup = Some(crate::pages::setup::Setup::new());
    settle(&mut h);
    for step in ["Welcome", "Node.js", "Look", "Tour"] {
        look(&h, step, &mut missing);
        if step != "Tour" {
            let next = if step == "Welcome" { "Get started" } else { "Next" };
            click(&mut h, next);
        }
    }
    report(missing);
}

#[test]
fn the_start_page_names_its_controls() {
    let h = window("start", false);
    let mut missing = Vec::new();
    look(&h, "Start page", &mut missing);
    report(missing);
}

#[test]
fn the_editor_names_its_controls() {
    let mut h = window("editor", true);
    let mut missing = Vec::new();
    look(&h, "Editor", &mut missing);
    // The side bar's other views.
    for view in [SideView::StoredData, SideView::Plugins] {
        h.state_mut().ed.prefs.side_shown = Some(view.key().into());
        settle(&mut h);
        look(&h, view.key(), &mut missing);
    }
    // The command palette, then the block picker.
    h.state_mut().palette = Some(Default::default());
    settle(&mut h);
    look(&h, "Command palette", &mut missing);
    h.state_mut().palette = None;
    let ws = h.state().ed.project.as_ref().unwrap().workspaces.keys().next().unwrap().clone();
    h.state_mut().ed.picker = Some(picker::Picker::new(&ws, egui::pos2(400.0, 300.0), egui::pos2(0.0, 0.0), None));
    settle(&mut h);
    look(&h, "Block picker", &mut missing);
    h.state_mut().ed.picker = None;
    // Backups and Export.
    open_center(&mut h.state_mut().dock, Tab::Backups);
    settle(&mut h);
    look(&h, "Backups", &mut missing);
    {
        let app = h.state_mut();
        app.bot.open_export(&app.ed);
    }
    settle(&mut h);
    look(&h, "Export", &mut missing);
    h.state_mut().new_project_dialog();
    settle(&mut h);
    look(&h, "New project", &mut missing);
    report(missing);
}

/// Controls drawn by hand say what they are, so a screen reader (and a
/// test) finds them by role and name.
#[test]
fn hand_drawn_controls_are_found_by_name() {
    use egui::Role;
    let mut h = window("found", true);
    let mut lost = Vec::new();
    let mut find = |h: &Harness<'_, CircuitApp>, role: Role, name: &str| {
        if h.query_all_by_role_and_label(role, name).next().is_none() {
            lost.push(format!("{role:?} {name:?}"));
        }
    };
    // The title bar, the activity bar, the dock's tabs, a block on the canvas.
    for name in ["Minimize", "Maximize", "Close", "Search commands, workspaces and blocks", "Workspaces", "Settings (Ctrl+,)"] {
        find(&h, Role::Button, name);
    }
    find(&h, Role::Tab, "Inspector");
    find(&h, Role::Button, "Close Console");
    find(&h, Role::Button, "Message Sent");
    // Settings: its list of pages, a theme, a switch named by its row.
    h.state_mut().open_settings();
    settle(&mut h);
    find(&h, Role::Button, "Appearance");
    find(&h, Role::Tab, "Settings");
    h.state_mut().settings.section = crate::pages::settings::Section::Editor;
    settle(&mut h);
    find(&h, Role::Switch, "Activity bar");
    assert!(lost.is_empty(), "{} control(s) not found by role and name:\n{}", lost.len(), lost.join("\n"));
}

#[test]
fn every_settings_page_names_its_controls() {
    let mut h = window("settings", true);
    let mut missing = Vec::new();
    h.state_mut().open_settings();
    settle(&mut h);
    for (section, name, _) in crate::pages::settings::Section::ALL {
        h.state_mut().settings.section = section;
        settle(&mut h);
        look(&h, &format!("Settings › {name}"), &mut missing);
    }
    report(missing);
}

#[test]
fn bot_settings_name_their_controls() {
    let mut h = window("bot", true);
    let mut missing = Vec::new();
    {
        let app = h.state_mut();
        app.bot.open_settings(&mut app.ed);
    }
    settle(&mut h);
    let pages = h.state().bot.settings.as_ref().unwrap().page_ids();
    for page in pages {
        h.state_mut().bot.settings.as_mut().unwrap().show_page(&page);
        settle(&mut h);
        look(&h, &format!("Bot settings › {page}"), &mut missing);
    }
    report(missing);
}

/// Connection offers the bot's invite link once a token is there, not before.
#[test]
fn bot_settings_offer_the_invite_link_with_a_token() {
    use egui::Role;
    let mut h = window("invite", true);
    {
        let app = h.state_mut();
        app.bot.open_settings(&mut app.ed);
        app.bot.settings.as_mut().unwrap().show_page("connection");
    }
    settle(&mut h);
    assert!(h.query_all_by_role_and_label(Role::Button, "Open invite page").next().is_none(), "no token, no invite link");
    let fake_token = ["MTIzNDU2Nzg5MDEyMzQ1Njc4OQ", "GhIjKl", "abcdefghijklmnopqrstuvwxyz0123456789AB"].join(".");
    h.state_mut().bot.settings.as_mut().unwrap().type_into("token", &fake_token);
    settle(&mut h);
    assert!(h.query_all_by_role_and_label(Role::Button, "Open invite page").next().is_some(), "a token: its invite link");
    assert!(h.query_all_by_role_and_label(Role::Button, "Copy link").next().is_some());
    assert!(h.query_by_label_contains("Invite your bot").is_some());
}

#[test]
fn every_builder_names_its_controls() {
    let mut h = window("builders", true);
    let mut missing = Vec::new();
    let ws = h.state().ed.project.as_ref().unwrap().workspaces.keys().next().unwrap().clone();
    {
        let ed = &mut h.state_mut().ed;
        crate::builders::embed::open(ed, &ws, "e", "embed");
    }
    settle(&mut h);
    look(&h, "Embed Builder", &mut missing);
    {
        let ed = &mut h.state_mut().ed;
        ed.embed_edit = None;
        crate::builders::layout::open(ed, &ws, "l", "layout");
    }
    settle(&mut h);
    look(&h, "Layout Builder", &mut missing);
    {
        // A form with every kind of part, so each one's controls are looked at.
        let ed = &mut h.state_mut().ed;
        ed.layout_edit = None;
        let form = serde_json::json!({ "title": "Apply", "custom_id": "apply", "components": [
            { "type": 10, "content": "Read this first" },
            { "type": 18, "label": "Name", "component": { "type": 4, "custom_id": "name", "style": 1 } },
            { "type": 18, "label": "Team", "component": { "type": 3, "custom_id": "team", "options": [{ "label": "Red", "value": "red" }] } },
            { "type": 18, "label": "Who", "component": { "type": 8, "custom_id": "where" } },
            { "type": 18, "label": "Proof", "component": { "type": 19, "custom_id": "proof" } }
        ] });
        ed.modal_edit = Some(crate::builders::modal::ModalEdit::new(&ws, "m", "form", &form));
    }
    settle(&mut h);
    look(&h, "Modal Builder", &mut missing);
    {
        let ed = &mut h.state_mut().ed;
        ed.modal_edit = None;
        crate::builders::canvas::open(ed, &ws, "c", "canvas");
    }
    settle(&mut h);
    look(&h, "Canvas Builder", &mut missing);
    {
        let ed = &mut h.state_mut().ed;
        ed.canvas_edit = None;
        crate::builders::slash::open(ed);
    }
    settle(&mut h);
    look(&h, "Slash Commands", &mut missing);
    report(missing);
}

/// Settings › Window › Accessibility off: the app turns AccessKit off just
/// before each frame begins (`raw_input_hook`; eframe turns it on when a
/// program asks for the window's contents), and then the frame builds
/// nothing for it.
#[test]
fn accessibility_can_be_turned_off() {
    let frame = |ctx: &egui::Context, off: bool| {
        if off {
            ctx.disable_accesskit();
        }
        let mut out = ctx.run_ui(Default::default(), |ui| {
            ui.label("Hello");
        });
        out.textures_delta.clear();
        out.platform_output.accesskit_update.is_some()
    };
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    assert!(frame(&ctx, false), "on: the window's contents are built");
    // A program reading the window asks again (eframe turns it on), but it's off.
    ctx.enable_accesskit();
    assert!(!frame(&ctx, true), "off: nothing built, though it was asked for");
    assert!(crate::editor::prefs::Prefs::default().accessibility, "on to start with");
}
