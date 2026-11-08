//! How the app draws, from the best choice to the most widely supported:
//!
//! 1. `auto`: Vulkan or DirectX 12 (on Windows, DirectX 12 includes WARP, a
//!    software renderer every Windows 10 and 11 has, for virtual machines,
//!    Remote Desktop and missing drivers). Elsewhere wgpu picks among Vulkan,
//!    Metal and OpenGL.
//! 2. `opengl`: OpenGL through wgpu (OpenGL 3.3 or ES 3.0).
//! 3. `glow`: eframe's own OpenGL renderer, which also runs on older OpenGL.
//! 4. `software`: like `auto`, for when no OpenGL worked either: on Windows
//!    that ends with WARP.
//!
//! When a stage can't start, the app starts itself again with the next one.
//!
//! Graphics cards too old for DirectX 12 and Vulkan (such as a Radeon HD 5000)
//! don't make `auto` fail: Windows draws with WARP instead. So when `auto`
//! ends up drawing in software, the app asks OpenGL whether it has a graphics
//! card; if so, it starts itself again with `opengl` (the window stays hidden
//! meanwhile) and remembers that in `graphics.txt` in its data folder, so later
//! starts begin there. Only such computers pay for the question.
//!
//! XC_RENDERER picks a stage by hand, before `graphics.txt`.

use std::sync::OnceLock;

use eframe::egui;
use eframe::egui_wgpu::WgpuSetup;
use eframe::wgpu;

pub const RENDERER_VAR: &str = "XC_RENDERER";
/// Why an earlier stage didn't start, for the new one to log.
pub const REASON_VAR: &str = "XC_RENDERER_REASON";
const STAGES: [&str; 4] = ["auto", "opengl", "glow", "software"];
/// The stage the app chose itself at an earlier start, in its data folder.
const REMEMBERED: &str = "graphics.txt";

/// Why this start uses the stage remembered in graphics.txt, for the Console.
static FROM_FILE: OnceLock<String> = OnceLock::new();
/// Set when `auto` drew in software and OpenGL has a card: why, for the restart.
static SWITCH: OnceLock<String> = OnceLock::new();

/// The stage to start with: XC_RENDERER, the one remembered in graphics.txt,
/// or the first.
pub fn stage() -> &'static str {
    let (stage, note) = pick_stage(std::env::var(RENDERER_VAR).ok().as_deref(), &crate::bot::app_data_dir());
    if let Some(note) = note {
        let _ = FROM_FILE.set(note);
    }
    stage
}

/// The stage for XC_RENDERER's value and the data folder, and a note for the
/// Console when it came from graphics.txt.
fn pick_stage(wanted: Option<&str>, data_dir: &std::path::Path) -> (&'static str, Option<String>) {
    let find = |s: &str| STAGES.iter().copied().find(|x| *x == s.trim().to_lowercase());
    if let Some(s) = wanted.and_then(find) {
        return (s, None);
    }
    let file = data_dir.join(REMEMBERED);
    match std::fs::read_to_string(&file).ok().as_deref().and_then(find) {
        Some(s) => (s, Some(format!("Drawing with the graphics chosen at an earlier start ({s}); delete {} to let the app choose again", file.display()))),
        None => (STAGES[0], None),
    }
}

/// Why this start uses the stage remembered in graphics.txt, if it does.
pub fn remembered() -> Option<&'static str> {
    FROM_FILE.get().map(String::as_str)
}

/// After the window was made: when `auto` draws in software on Windows but
/// OpenGL has a graphics card, a stand-in app that closes the still hidden
/// window, so `main` can start again with OpenGL. None otherwise (and
/// OpenGL is only asked when drawing in software).
pub fn opengl_instead(cc: &eframe::CreationContext<'_>, stage: &str) -> Option<Box<dyn eframe::App>> {
    if !cfg!(windows) || stage != "auto" || std::env::var_os("WGPU_BACKEND").is_some() || !describe(cc).1 {
        return None;
    }
    let card = graphics_card(adapters(wgpu::Backends::GL))?;
    let _ = SWITCH.set(format!("DirectX 12 and Vulkan could only draw in software, but OpenGL has {card}"));
    // Later starts begin with OpenGL. If it stops working, the stages after it still follow.
    let dir = crate::bot::app_data_dir();
    let _ = std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(dir.join(REMEMBERED), "opengl\n"));
    Some(Box::new(Switching { _hidden: crate::app::reveal::Reveal::new(cc) }))
}

/// Why the app should start again with OpenGL, once its window has closed.
pub fn switching() -> Option<&'static str> {
    SWITCH.get().map(String::as_str)
}

/// Keeps the window cloaked and closes it at once.
struct Switching {
    _hidden: crate::app::reveal::Reveal,
}

impl eframe::App for Switching {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
    }
}

/// The first adapter that is a graphics card (not software), by name.
fn graphics_card(adapters: Vec<(String, bool)>) -> Option<String> {
    adapters.into_iter().find(|(_, software)| !software).map(|(name, _)| name)
}

/// The adapters wgpu finds with these backends: name, and whether it draws in software.
fn adapters(backends: wgpu::Backends) -> Vec<(String, bool)> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor { backends, ..wgpu::InstanceDescriptor::new_without_display_handle() });
    // Native backends answer at once; the future only waits on the web.
    let mut list = std::pin::pin!(instance.enumerate_adapters(backends));
    match list.as_mut().poll(&mut std::task::Context::from_waker(std::task::Waker::noop())) {
        std::task::Poll::Ready(list) => list
            .iter()
            .map(|a| {
                let info = a.get_info();
                (info.name, info.device_type == wgpu::DeviceType::Cpu)
            })
            .collect(),
        std::task::Poll::Pending => Vec::new(),
    }
}

/// Sets up `options` for a stage.
pub fn configure(options: &mut eframe::NativeOptions, stage: &str) {
    let backends = |options: &mut eframe::NativeOptions, b: wgpu::Backends| {
        if let WgpuSetup::CreateNew(setup) = &mut options.wgpu_options.wgpu_setup {
            setup.instance_descriptor.backends = b;
        }
    };
    match stage {
        "opengl" => backends(options, wgpu::Backends::GL),
        "glow" => options.renderer = eframe::Renderer::Glow,
        // "auto" and "software" (the same, as the last resort).
        _ => {
            // wgpu would also try OpenGL, and NVIDIA's OpenGL driver opens
            // screen-sized helper windows for that. It is a later stage here;
            // WGPU_BACKEND still chooses by hand.
            if cfg!(windows) && std::env::var_os("WGPU_BACKEND").is_none() {
                backends(options, wgpu::Backends::PRIMARY);
            }
        }
    }
}

/// Whether the app couldn't start because of its graphics (not, say, a
/// broken project), so the next stage may work.
pub fn is_graphics_error(e: &eframe::Error) -> bool {
    matches!(e, eframe::Error::Wgpu(_) | eframe::Error::Glutin(_) | eframe::Error::NoGlutinConfigs(..) | eframe::Error::OpenGL(_))
}

/// Starts the app again with the stage after `stage`, waits for it and returns
/// its exit code. None when there is no later stage or it couldn't start.
pub fn restart_with_next(stage: &str, why: &eframe::Error) -> Option<i32> {
    let next = STAGES.iter().position(|s| *s == stage).and_then(|i| STAGES.get(i + 1))?;
    restart_with(next, &format!("{} couldn't start: {why}", describe_stage(stage)))
}

/// Starts the app again with `stage`, waits for it and returns its exit code.
pub fn restart_with(stage: &str, why: &str) -> Option<i32> {
    let exe = std::env::current_exe().ok()?;
    let status = std::process::Command::new(exe).args(std::env::args_os().skip(1)).env(RENDERER_VAR, stage).env(REASON_VAR, why).status().ok()?;
    Some(status.code().unwrap_or(1))
}

fn describe_stage(stage: &str) -> &'static str {
    match stage {
        "opengl" => "OpenGL",
        "glow" => "OpenGL (glow)",
        "software" => "Software drawing",
        _ if cfg!(windows) => "Vulkan and DirectX 12",
        _ => "Vulkan",
    }
}

/// What draws the app, for the console: "Vulkan · NVIDIA GeForce RTX 4070
/// SUPER". `software` is true when no graphics card does it.
pub fn describe(cc: &eframe::CreationContext<'_>) -> (String, bool) {
    let Some(state) = cc.wgpu_render_state.as_ref() else {
        return ("OpenGL (glow)".to_string(), false);
    };
    let info = state.adapter.get_info();
    let api = match info.backend {
        wgpu::Backend::Vulkan => "Vulkan",
        wgpu::Backend::Dx12 => "DirectX 12",
        wgpu::Backend::Metal => "Metal",
        wgpu::Backend::Gl => "OpenGL",
        _ => "WebGPU",
    };
    let text = format!("{api} · {}", info.name);
    let _ = DESCRIBED.set(text.clone());
    (text, info.device_type == wgpu::DeviceType::Cpu)
}

/// What draws the app this time, as [`describe`] said.
static DESCRIBED: OnceLock<String> = OnceLock::new();

pub fn in_use() -> Option<&'static str> {
    DESCRIBED.get().map(String::as_str)
}

/// The way to draw picked in Settings for the next start: `auto`, `opengl`
/// or `glow` (kept in graphics.txt, as a fallback that worked is).
pub fn chosen() -> &'static str {
    chosen_in(&crate::bot::app_data_dir())
}

fn chosen_in(data_dir: &std::path::Path) -> &'static str {
    let text = std::fs::read_to_string(data_dir.join(REMEMBERED)).unwrap_or_default();
    ["opengl", "glow"].into_iter().find(|s| text.trim().eq_ignore_ascii_case(s)).unwrap_or("auto")
}

/// Picks the way to draw from the next start on: `auto` lets the app choose
/// (and fall back) again.
pub fn choose(stage: &str) -> std::io::Result<()> {
    choose_in(&crate::bot::app_data_dir(), stage)
}

fn choose_in(data_dir: &std::path::Path, stage: &str) -> std::io::Result<()> {
    let file = data_dir.join(REMEMBERED);
    if stage == "auto" {
        return match std::fs::remove_file(&file) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        };
    }
    std::fs::create_dir_all(data_dir)?;
    std::fs::write(file, format!("{stage}\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The way to draw picked in Settings: kept for the next start, and
    /// Automatic takes the choice away again.
    #[test]
    fn a_picked_way_to_draw_is_kept() {
        let dir = std::env::temp_dir().join(format!("xc-graphics-pick-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(chosen_in(&dir), "auto");
        choose_in(&dir, "glow").unwrap();
        assert_eq!(chosen_in(&dir), "glow");
        assert_eq!(pick_stage(None, &dir).0, "glow", "the next start uses it");
        choose_in(&dir, "auto").unwrap();
        assert_eq!((chosen_in(&dir), pick_stage(None, &dir).0), ("auto", "auto"));
        choose_in(&dir, "auto").unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// OpenGL's graphics card, not its software drawing.
    #[test]
    fn finds_the_graphics_card() {
        let card = |n: &str| (n.to_string(), false);
        let soft = |n: &str| (n.to_string(), true);
        assert_eq!(graphics_card(vec![soft("GDI Generic"), card("AMD Radeon HD 5700 Series")]).as_deref(), Some("AMD Radeon HD 5700 Series"));
        assert_eq!(graphics_card(vec![soft("llvmpipe")]), None, "no card anywhere: software, as before");
        assert_eq!(graphics_card(Vec::new()), None);
    }

    /// Every stage has a next one until the last, which is software drawing.
    #[test]
    fn stages_end_in_software() {
        assert_eq!(STAGES.last(), Some(&"software"));
        assert_eq!(describe_stage("software"), "Software drawing");
    }

    /// On this computer: OpenGL finds the card without a window of its own (a card is needed).
    #[test]
    #[ignore = "needs a graphics card"]
    fn live_opengl_card() {
        let gl = adapters(wgpu::Backends::GL);
        eprintln!("opengl: {gl:?}");
        assert!(graphics_card(gl).is_some());
    }

    /// A stage remembered in graphics.txt is used; XC_RENDERER comes first.
    #[test]
    fn remembered_stage() {
        let dir = std::env::temp_dir().join(format!("xc-graphics-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(pick_stage(None, &dir), ("auto", None), "nothing remembered");
        std::fs::write(dir.join(REMEMBERED), "opengl\n").unwrap();
        let (stage, note) = pick_stage(None, &dir);
        assert_eq!(stage, "opengl");
        assert!(note.is_some_and(|m| m.contains("graphics.txt")));
        assert_eq!(pick_stage(Some("Glow"), &dir), ("glow", None), "XC_RENDERER comes first");
        std::fs::write(dir.join(REMEMBERED), "nonsense").unwrap();
        assert_eq!(pick_stage(None, &dir).0, "auto");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
