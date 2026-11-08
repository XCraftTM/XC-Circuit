//! Keeps the window off the screen until it has drawn, so Windows doesn't show
//! it white for a moment at startup. eframe makes the window visible right
//! after its first frame, before that frame has reached the screen; a cloaked
//! window is still drawn by Windows (unlike a hidden one) but not shown, so it
//! appears with its first frames already there.
//!
//! A window that was maximized last time is a second case: eframe creates it
//! maximized, and Windows shows a window it maximizes, empty and white, long
//! before anything is drawn. So it is created at its normal size instead and
//! maximized on the first frame, while cloaked.

use std::sync::atomic::{AtomicBool, Ordering};

use eframe::egui;

/// The window was maximized last time and is maximized once it is cloaked.
static MAXIMIZE: AtomicBool = AtomicBool::new(false);

/// For eframe's window builder hook: creates the window unmaximized and
/// remembers to maximize it (on Windows; elsewhere the builder stays as it is).
pub fn defer_maximize(builder: egui::ViewportBuilder) -> egui::ViewportBuilder {
    if cfg!(windows) && builder.maximized == Some(true) {
        MAXIMIZE.store(true, Ordering::Relaxed);
        return builder.with_maximized(false);
    }
    builder
}

/// Frames drawn before the window is shown (one at its maximized size too).
const FRAMES: u32 = 4;
/// Shown after this long in any case, should frames stop while it is cloaked.
#[cfg(windows)]
const AT_THE_LATEST: std::time::Duration = std::time::Duration::from_secs(2);

pub struct Reveal {
    /// The window (as a number, to share with the fallback thread) while it is
    /// still cloaked.
    #[cfg(windows)]
    hwnd: Option<isize>,
    /// The window, for [`Reveal::is_foreground`].
    #[cfg(windows)]
    window: Option<isize>,
    frames: u32,
}

impl Reveal {
    /// Cloaks the window (Windows only; elsewhere this does nothing).
    pub fn new(window: &impl raw_window_handle::HasWindowHandle) -> Self {
        #[cfg(windows)]
        {
            use raw_window_handle::RawWindowHandle;
            let hwnd = match window.window_handle().map(|h| h.as_raw()) {
                Ok(RawWindowHandle::Win32(h)) => Some(h.hwnd.get()),
                _ => None,
            };
            let window = hwnd;
            let hwnd = hwnd.filter(|&h| cloak(h, true));
            if let Some(h) = hwnd {
                std::thread::spawn(move || {
                    std::thread::sleep(AT_THE_LATEST);
                    cloak(h, false);
                });
            }
            Reveal { hwnd, window, frames: 0 }
        }
        #[cfg(not(windows))]
        {
            let _ = window;
            Reveal { frames: 0 }
        }
    }

    /// Call once per frame: maximizes the window if it was, and shows it after
    /// a few drawn frames.
    pub fn frame(&mut self, ctx: &egui::Context) {
        if MAXIMIZE.swap(false, Ordering::Relaxed) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(true));
        }
        #[cfg(windows)]
        if let Some(hwnd) = self.hwnd {
            self.frames += 1;
            if self.frames > FRAMES {
                cloak(hwnd, false);
                self.hwnd = None;
            } else {
                ctx.request_repaint();
            }
        }
        #[cfg(not(windows))]
        let _ = (ctx, self.frames, FRAMES);
    }
}

impl Reveal {
    /// Whether the window is the one in front, taking the keyboard: asked of
    /// Windows itself (None where that isn't known). egui only draws the text
    /// cursor while it thinks the window has the focus, and learns that from
    /// focus events, which a window cloaked at startup can miss.
    /// The main window's handle (Windows only).
    pub fn window(&self) -> Option<isize> {
        #[cfg(windows)]
        return self.window;
        #[cfg(not(windows))]
        None
    }

    pub fn is_foreground(&self) -> Option<bool> {
        #[cfg(windows)]
        {
            // SAFETY: no arguments; it returns the foreground window's handle, or null.
            let front = unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow() };
            self.window.map(|w| front as isize == w)
        }
        #[cfg(not(windows))]
        None
    }
}

/// Cloaks or uncloaks a window through the desktop window manager. Returns
/// whether it worked. Uncloaking twice is harmless.
#[cfg(windows)]
fn cloak(hwnd: isize, on: bool) -> bool {
    use windows_sys::Win32::Graphics::Dwm::{DWMWA_CLOAK, DwmSetWindowAttribute};
    let value: i32 = on.into();
    // SAFETY: the handle is the app's own window from eframe; the attribute is
    // a BOOL, passed with its size. A handle of a closed window only makes the
    // call fail.
    let result = unsafe { DwmSetWindowAttribute(hwnd as windows_sys::Win32::Foundation::HWND, DWMWA_CLOAK as u32, (&raw const value).cast(), size_of::<i32>() as u32) };
    result >= 0
}
