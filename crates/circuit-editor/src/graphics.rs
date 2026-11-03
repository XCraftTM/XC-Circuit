//! How the app draws, from the best choice to the most widely supported:
//!
//! 1. `auto`: Vulkan or DirectX 12 (on Windows, DirectX 12 includes WARP, a
//!    software renderer every Windows 10 and 11 has, for virtual machines,
//!    Remote Desktop and missing drivers). Elsewhere wgpu picks among Vulkan,
//!    Metal and OpenGL.
//! 2. `opengl`: OpenGL through wgpu (OpenGL 3.3 or ES 3.0).
//! 3. `glow`: eframe's own OpenGL renderer, which also runs on older OpenGL.
//!
//! When a stage can't start, the app starts itself again with the next one.
//! XC_RENDERER picks a stage by hand.

use eframe::egui_wgpu::WgpuSetup;
use eframe::wgpu;

pub const RENDERER_VAR: &str = "XC_RENDERER";
/// Why an earlier stage didn't start, for the new one to log.
pub const REASON_VAR: &str = "XC_RENDERER_REASON";
const STAGES: [&str; 3] = ["auto", "opengl", "glow"];

/// The stage to start with: XC_RENDERER, or the first.
pub fn stage() -> &'static str {
    let wanted = std::env::var(RENDERER_VAR).unwrap_or_default().trim().to_lowercase();
    STAGES.iter().copied().find(|s| *s == wanted).unwrap_or(STAGES[0])
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
    let exe = std::env::current_exe().ok()?;
    let status = std::process::Command::new(exe)
        .args(std::env::args_os().skip(1))
        .env(RENDERER_VAR, next)
        .env(REASON_VAR, format!("{} couldn't start: {why}", describe_stage(stage)))
        .status()
        .ok()?;
    Some(status.code().unwrap_or(1))
}

fn describe_stage(stage: &str) -> &'static str {
    match stage {
        "opengl" => "OpenGL",
        "glow" => "OpenGL (glow)",
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
    (format!("{api} · {}", info.name), info.device_type == wgpu::DeviceType::Cpu)
}
