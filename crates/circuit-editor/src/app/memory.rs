//! Saving the window's layout and settings: a moment after the last change
//! ([`Autosave`]), and what egui remembers between starts (which sections
//! were open, a text field's height, the console's level …), by the app itself.
//!
//! eframe would save all of egui's memory, every 30 seconds, while holding
//! egui's lock, so the window stops answering for as long as that takes. And
//! that memory only grows: every popup, menu and window ever shown is kept,
//! start after start (thousands after a while), so the save got slower the
//! longer the app was used. Here only the options and the remembered values
//! are kept, with less of each kind, and they're written after the lock is let
//! go: a copy is taken under it, which is quick.

/// eframe's key for egui's memory, which it still reads at start.
const KEY: &str = "egui";

/// How much of each kind of remembered value is kept (the most recently
/// used first). egui keeps 256 KiB.
const BYTES_PER_KIND: usize = 64 * 1024;

/// The part of egui's memory worth keeping: no windows' and popups' places.
fn worth_keeping(ctx: &egui::Context) -> egui::Memory {
    let (options, mut data) = ctx.memory(|m| (m.options.clone(), m.data.clone()));
    data.set_max_bytes_per_type(BYTES_PER_KIND);
    let mut kept = egui::Memory::default();
    kept.options = options;
    kept.data = data;
    kept
}

/// Saves what egui remembers into `storage`.
pub(super) fn save(ctx: &egui::Context, storage: &mut dyn eframe::Storage) {
    eframe::set_value(storage, KEY, &worth_keeping(ctx));
}

/// When to save the layout and settings: a while after the last change
/// (input, a window moved or resized, a plugin storing a value), each
/// change in the meantime starting the wait again. Without changes, never
/// (and always when the app closes, which eframe does).
#[derive(Default)]
pub(super) struct Autosave {
    /// The last change not saved yet.
    changed: Option<std::time::Instant>,
    /// The windows' places last frame, to see them move.
    windows: Vec<Place>,
}

/// A window's place: which, outside and inside, and whether maximized.
type Place = (egui::ViewportId, Option<egui::Rect>, Option<egui::Rect>, Option<bool>);

/// As good as never: eframe asks for an interval.
const NEVER: std::time::Duration = std::time::Duration::from_secs(60 * 60 * 24 * 365);

impl Autosave {
    /// Looks at this frame for changes (`stored`: a plugin stored a value),
    /// and asks for a frame when the wait is over, so the save happens then.
    pub(super) fn look(&mut self, ctx: &egui::Context, stored: bool, wait: std::time::Duration) {
        let input = ctx.input(|i| i.events.iter().any(changes_something));
        let mut windows: Vec<_> = ctx.input(|i| i.raw.viewports.iter().map(|(id, v)| (*id, v.outer_rect, v.inner_rect, v.maximized)).collect());
        windows.sort_by_key(|(id, ..)| format!("{id:?}"));
        let moved = windows != self.windows;
        self.windows = windows;
        self.note(std::time::Instant::now(), input || moved || stored);
        if let Some(at) = self.changed {
            // A little after, so the wait is surely over.
            ctx.request_repaint_after(wait.saturating_sub(at.elapsed()) + std::time::Duration::from_millis(50));
        }
    }

    fn note(&mut self, now: std::time::Instant, changed: bool) {
        if changed {
            self.changed = Some(now);
        }
    }

    /// How long eframe waits between saves, from its last one: none once
    /// `wait` has passed since the last change.
    pub(super) fn interval(&self, now: std::time::Instant, wait: std::time::Duration) -> std::time::Duration {
        match self.changed {
            Some(at) if now.saturating_duration_since(at) >= wait => std::time::Duration::ZERO,
            _ => NEVER,
        }
    }

    /// Saved: nothing left to save.
    pub(super) fn saved(&mut self) {
        self.changed = None;
    }
}

/// Whether input may have changed what's saved: anything but the pointer
/// moving over the window, or leaving it, or the focus.
fn changes_something(event: &egui::Event) -> bool {
    !matches!(event, egui::Event::PointerMoved(_) | egui::Event::MouseMoved(_) | egui::Event::PointerGone | egui::Event::WindowFocused(_))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Kv(std::collections::HashMap<String, String>);
    impl eframe::Storage for Kv {
        fn get_string(&self, key: &str) -> Option<String> {
            self.0.get(key).cloned()
        }
        fn set_string(&mut self, key: &str, value: String) {
            self.0.insert(key.into(), value);
        }
        fn remove_string(&mut self, key: &str) {
            self.0.remove(key);
        }
        fn flush(&mut self) {}
    }

    fn frame(ctx: &egui::Context, ui: impl FnMut(&mut egui::Ui)) {
        ctx.run_ui(Default::default(), ui).textures_delta.clear();
    }

    /// Remembered values and the options come back at the next start; the
    /// places of the popups and windows shown don't, however many there were.
    #[test]
    fn remembered_values_are_kept_and_popups_are_not() {
        let ctx = egui::Context::default();
        let open = egui::Id::unique("section");
        for i in 0..2000 {
            frame(&ctx, |ui| {
                ui.data_mut(|d| d.insert_persisted(open, true));
                // A popup with an id of its own each time, as egui keeps them.
                egui::Area::new(egui::Id::unique(("popup", i))).show(ui.ctx(), |ui| ui.label("hi"));
            });
        }
        ctx.memory_mut(|m| m.options.input_options.line_scroll_speed = 77.0);
        let everything = ctx.memory(|m| ron::to_string(m).unwrap()).len();

        let mut kv = Kv::default();
        save(&ctx, &mut kv);
        let saved = &kv.0[KEY];
        assert!(saved.len() * 20 < everything, "{} bytes saved of {everything}", saved.len());

        let mut back: egui::Memory = eframe::get_value(&kv, KEY).expect("eframe reads it at start");
        assert_eq!(back.data.get_persisted::<bool>(open), Some(true));
        assert_eq!(back.options.input_options.line_scroll_speed, 77.0);
        assert!(!saved.contains("pivot_pos"), "no places of windows or popups");
    }

    /// It saves once the wait has passed since the last change; each change
    /// in the meantime starts the wait again. Without changes, it doesn't.
    #[test]
    fn it_saves_a_moment_after_the_last_change() {
        use std::time::{Duration, Instant};
        let wait = Duration::from_secs(5);
        let start = Instant::now();
        let at = |s: u64| start + Duration::from_secs(s);
        let mut a = Autosave::default();
        assert_eq!(a.interval(at(100), wait), NEVER, "nothing changed");
        a.note(at(0), true);
        assert_eq!(a.interval(at(3), wait), NEVER, "still waiting");
        a.note(at(3), true);
        assert_eq!(a.interval(at(6), wait), NEVER, "a new change starts the wait again");
        a.note(at(6), false);
        assert_eq!(a.interval(at(8), wait), Duration::ZERO, "five seconds after the last change");
        a.saved();
        assert_eq!(a.interval(at(60), wait), NEVER, "saved: nothing to save");
        assert!(!changes_something(&egui::Event::PointerMoved(egui::Pos2::ZERO)));
        assert!(changes_something(&egui::Event::Text("a".into())));
    }

    /// The copy is taken under egui's lock, and quickly: a frame waiting for
    /// it doesn't notice.
    #[test]
    fn keeping_takes_a_moment() {
        let ctx = egui::Context::default();
        frame(&ctx, |ui| {
            ui.data_mut(|d| {
                for i in 0..20_000 {
                    d.insert_persisted(egui::Id::unique(("open", i)), i % 2 == 0);
                }
            });
        });
        let start = std::time::Instant::now();
        let _ = worth_keeping(&ctx);
        assert!(start.elapsed() < std::time::Duration::from_millis(500), "{:?}", start.elapsed());
    }
}
