//! What egui remembers between starts (which sections were open, a text
//! field's height, the console's level …), saved by the app itself.
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
