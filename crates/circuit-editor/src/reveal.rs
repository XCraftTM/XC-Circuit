//! Keeps the window off the screen until it has drawn, so Windows doesn't show
//! it white for a moment at startup. eframe makes the window visible right
//! after its first frame, before that frame has reached the screen; a cloaked
//! window is still drawn by Windows (unlike a hidden one) but not shown, so it
//! appears with its first frames already there.

use eframe::egui;

/// Frames drawn before the window is shown.
const FRAMES: u32 = 3;
/// Shown after this long in any case, should frames stop while it is cloaked.
#[cfg(windows)]
const AT_THE_LATEST: std::time::Duration = std::time::Duration::from_secs(2);

pub struct Reveal {
    /// The window (as a number, to share with the fallback thread) while it is
    /// still cloaked.
    #[cfg(windows)]
    hwnd: Option<isize>,
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
            let hwnd = hwnd.filter(|&h| cloak(h, true));
            if let Some(h) = hwnd {
                std::thread::spawn(move || {
                    std::thread::sleep(AT_THE_LATEST);
                    cloak(h, false);
                });
            }
            Reveal { hwnd, frames: 0 }
        }
        #[cfg(not(windows))]
        {
            let _ = window;
            Reveal { frames: 0 }
        }
    }

    /// Call once per frame: shows the window after a few drawn frames.
    pub fn frame(&mut self, ctx: &egui::Context) {
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
