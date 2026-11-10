//! The pictures of the window in the README and the tutorials, made from
//! the app itself: the Ping Bot tutorial's project is opened in a window
//! that is drawn without a screen, and what a tutorial points at gets a
//! numbered mark. Run by hand after the window changes, then look at them:
//!
//! ```text
//! cargo test -p circuit-editor docs_screenshots -- --ignored
//! ```
//!
//! They are written to `docs/public/screens/`. It needs graphics (a card,
//! or a software renderer such as WARP or lavapipe).

use std::path::{Path, PathBuf};

use circuit_core::Project;
use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

use super::*;

fn plugins_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins")
}

fn screens_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/public/screens")
}

/// The tutorials' project: a /ping and a /count command, each in its workspace.
fn ping_bot(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
    let mut p = Project::create(dir, "Ping Bot", "discord-bot").unwrap();
    let first = p.workspaces.keys().next().cloned().unwrap();
    p.workspaces.get_mut(&first).unwrap().name = "Start here".into();
    let count: circuit_core::Workspace = serde_json::from_value(serde_json::json!({
        "name": "Count command",
        "nodes": {
            "1": { "block": "discord.on_slash_command@2", "at": [60, 150], "values": { "command": "count" } },
            "2": { "block": "core.increase_stored@1", "at": [380, 150], "values": { "scope": "user", "key": "counts" } },
            "3": { "block": "discord.send_message@3", "at": [700, 150], "values": { "mode": "reply", "text": "You've counted ${value} times, ${user.username}!", "private": false } },
        },
        "links": ["1.next > 2.in", "2.next > 3.in", "1.interaction > 3.interaction", "1.user > 2.target", "2.value > 3.value"],
        "notes": { "n1": { "title": "/count", "text": "Counts how often each member used the command and answers with the number. Stored per user, so it survives restarts.", "rect": [30, 50, 1010, 520], "color": "#3DFF6E" } },
    }))
    .unwrap();
    let ping: circuit_core::Workspace = serde_json::from_value(serde_json::json!({
        "name": "Ping command",
        "nodes": {
            "1": { "block": "discord.on_slash_command@2", "at": [60, 80], "values": { "command": "ping" } },
            "2": { "block": "discord.send_message@3", "at": [380, 80], "values": { "mode": "reply", "text": "Pong! ${Date.now() - interaction.createdTimestamp} ms", "private": false } },
        },
        "links": ["1.next > 2.in", "1.interaction > 2.interaction"],
    }))
    .unwrap();
    p.workspaces.insert("count-command".into(), count);
    p.workspaces.insert("ping-command".into(), ping);
    p.meta.data.insert(
        crate::builders::slash::DATA_KEY.into(),
        serde_json::json!([
            { "name": "ping", "description": "Checks that the bot answers", "contexts": ["guild"], "installs": ["guild"], "options": [] },
            { "name": "count", "description": "Counts how often you used it", "contexts": ["guild"], "installs": ["guild"], "options": [] },
        ]),
    );
    p.save().unwrap();
}

/// The tutorial's own plugin (Your first editor plugin), for the Plugins menu.
fn block_counter(plugins: &Path) {
    let dir = plugins.join("block.counter");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("plugin.toml"), "id = \"block.counter\"\nname = \"Block Counter\"\nversion = \"1.0.0\"\n\n[editor]\nscript = \"editor.js\"\n").unwrap();
    std::fs::write(
        dir.join("editor.js"),
        "xc.command({ id: 'counter.total', title: 'Count all blocks', run() { xc.log(`${xc.project().name} has ${xc.blocks().length} blocks`); } });\nxc.panel({ id: 'counts', title: 'Block Counter', render(ui) { ui.heading('Blocks per workspace'); } });\n",
    )
    .unwrap();
}

/// The app's window, 1440 by 900, with the tutorial's project open and its
/// Count command in view.
fn window(name: &str) -> Harness<'static, CircuitApp> {
    let tmp = std::env::temp_dir().join(format!("xc-shots-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let own_plugins = tmp.join("plugins");
    block_counter(&own_plugins);
    let project = tmp.join("Ping Bot");
    ping_bot(&project);
    let mut ed = Editor::new(vec![plugins_dir(), own_plugins], Some("graphite")).unwrap();
    ed.prefs.language = crate::i18n::ENGLISH.into();
    ed.prefs.setup_done = true;
    ed.apply_language();
    ed.restyle();
    ed.open_project(&project);
    let bundled = Some(plugins_dir());
    let mut h = Harness::builder().with_size(egui::vec2(1440.0, 900.0)).with_pixels_per_point(1.0).wgpu().build_eframe(move |cc| {
        egui_extras::install_image_loaders(&cc.egui_ctx);
        let _ = crate::ui::fonts::install_with(&cc.egui_ctx, None);
        ed.theme.apply(&cc.egui_ctx);
        let mut app = CircuitApp::with_editor(cc, ed, default_layout(), bundled);
        // Nothing of the first frames is news for the corner.
        app.ed.actions.push(Action::OpenWorkspace("count-command".into()));
        app
    });
    h.run_steps(12);
    h
}

/// Draws the window and writes `crop` of it (all of it without) as `name`.png.
fn save(h: &mut Harness<'_, CircuitApp>, name: &str, crop: Option<egui::Rect>) {
    h.run_steps(4);
    let image = h.render().expect("the window drawn (this needs graphics, see the top of this file)");
    let image = match crop {
        Some(r) => {
            let (x, y) = (r.left().max(0.0) as u32, r.top().max(0.0) as u32);
            let (w, hgt) = ((r.width() as u32).min(image.width() - x), (r.height() as u32).min(image.height() - y));
            image::imageops::crop_imm(&image, x, y, w, hgt).to_image()
        }
        None => image,
    };
    let file = screens_dir().join(format!("{name}.png"));
    image.save(&file).unwrap();
    eprintln!("wrote {} ({}×{})", file.display(), image.width(), image.height());
}

/// Where the control with this name is; of several, the one `pick` likes best (the smallest it gives).
fn rect_of(h: &Harness<'_, CircuitApp>, label: &str, pick: impl Fn(egui::Rect) -> f32) -> egui::Rect {
    let mut all: Vec<egui::Rect> = h.query_all_by_label(label).map(|n| n.rect()).collect();
    assert!(!all.is_empty(), "nothing is named {label:?}");
    all.sort_by(|a, b| pick(*a).total_cmp(&pick(*b)));
    all[0]
}

#[test]
#[ignore]
fn docs_screenshots() {
    // The whole window, for the README.
    let mut h = window("hero");
    save(&mut h, "hero", None);

    // The Run button.
    let run = h.state().ed.ui_rects.get("run").or_else(|| h.state().ed.ui_rects_before.get("run")).copied().expect("the Run button");
    h.state_mut().shot_marks = vec![(run, 1)];
    save(&mut h, "run", Some(egui::Rect::from_min_size(egui::pos2(1440.0 - 680.0, 0.0), egui::vec2(680.0, 110.0))));
    h.state_mut().shot_marks.clear();

    // The Plugins menu, with the tutorial's plugin in it.
    let menu = rect_of(&h, "Plugins", |r| r.top());
    h.query_all_by_label("Plugins").find(|n| n.rect() == menu).unwrap().click();
    h.run_steps(6);
    let command = rect_of(&h, "Count all blocks", |r| r.top());
    let panel = rect_of(&h, "Block Counter panel", |r| r.top());
    h.state_mut().shot_marks = vec![(command, 1), (panel, 2)];
    save(&mut h, "plugins-menu", Some(egui::Rect::from_min_size(egui::pos2((menu.left() - 110.0).max(0.0), 0.0), egui::vec2(500.0, 260.0))));
    h.state_mut().shot_marks.clear();

    new_project();
}

/// Draws the marks over a picture's contents: an orange frame around what
/// a tutorial points at, and its number in a circle at the frame's corner.
pub(super) fn marks(ctx: &egui::Context, list: &[(egui::Rect, u8)]) {
    let mark = egui::Color32::from_rgb(0xFF, 0xA5, 0x1F);
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Debug, egui::Id::unique("shot-marks")));
    for (rect, number) in list {
        let frame = rect.expand(3.0);
        painter.rect_stroke(frame, 5.0, egui::Stroke::new(3.0, mark), egui::StrokeKind::Outside);
        // The number at the frame's top left corner; at the very top of the
        // picture, where that would be cut off, at its bottom left one.
        let at = if frame.top() < 16.0 { frame.left_bottom() + egui::vec2(-2.0, 2.0) } else { frame.left_top() + egui::vec2(-2.0, -2.0) };
        painter.circle_filled(at, 12.0, mark);
        painter.circle_stroke(at, 12.0, egui::Stroke::new(1.5, egui::Color32::from_rgb(0x4A, 0x2C, 0x00)));
        painter.text(at, egui::Align2::CENTER_CENTER, number.to_string(), crate::ui::fonts::bold(14.0), egui::Color32::from_rgb(0x2A, 0x18, 0x00));
    }
}

/// The New Project window as it opens beside the app's: its own title bar,
/// and in it what the tutorial fills in.
fn new_project() {
    use crate::pages::new_project::{NewProject, WINDOW, content, templates};
    let ed = Editor::new(vec![plugins_dir()], Some("graphite")).unwrap();
    let theme = ed.theme.clone();
    let examples = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let starters = templates(Some(&examples), &ed.reg);
    let mut state = NewProject::new(&ed.reg, PathBuf::from(r"C:\Users\You\Documents\XC Circuit"));
    state.name = "Ping Bot".into();
    let icon = ed.reg.icons.get("folder-plus").cloned();
    let shown: std::rc::Rc<std::cell::RefCell<Vec<(egui::Rect, u8)>>> = Default::default();
    let to_draw = shown.clone();
    let mut ready = false;
    let mut h = Harness::builder().with_size(WINDOW).with_pixels_per_point(1.0).wgpu().build_ui(move |ui| {
        if !ready {
            ready = true;
            egui_extras::install_image_loaders(ui.ctx());
            let _ = crate::ui::fonts::install_with(ui.ctx(), None);
            theme.apply(ui.ctx());
        }
        state.as_if_the_folder_is_free();
        crate::ui::dialog::own_window_contents(ui, &theme, "New project", icon.as_deref(), false, |ui| {
            content(ui, &theme, &mut state, &ed.reg, &starters);
        });
        marks(ui.ctx(), &to_draw.borrow());
    });
    h.run_steps(8);
    let create = h.get_by_label("Create").rect();
    let kind = h.query_all_by_label("Discord Bot").map(|n| n.rect()).min_by(|a, b| a.left().total_cmp(&b.left())).expect("the kinds of project");
    let empty = h.query_all_by_label("Empty project").map(|n| n.rect()).next().expect("the empty starter");
    let name = h.query_all_by_label("Name").map(|n| n.rect()).max_by(|a, b| a.width().total_cmp(&b.width())).expect("the name's field");
    *shown.borrow_mut() = vec![(kind, 1), (name, 2), (empty, 3), (create, 4)];
    h.run_steps(4);
    let image = h.render().expect("the window drawn");
    let file = screens_dir().join("new-project.png");
    image.save(&file).unwrap();
    eprintln!("wrote {}", file.display());
}
