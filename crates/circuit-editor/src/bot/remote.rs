//! Running the bot on its server: the Local / Server switch beside Run. With
//! Server picked, Run, Restart and Stop act on the project's Calagopus server
//! (Run and Restart upload what changed first, so it runs this version) and
//! the status bar shows the server's state, read every few seconds.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::thread;
use std::time::{Duration, Instant};

use circuit_runner::hosting::calagopus;
use eframe::egui::{self, Color32, RichText, Stroke, vec2};

use super::BotState;
use crate::ui::theme::Theme;

/// How often the server's state is read.
const EVERY: Duration = Duration::from_secs(3);
/// After Run or Stop, what was asked shows at least this long (the panel may
/// still say the old state for a moment) …
const AT_LEAST: Duration = Duration::from_secs(4);
/// … and at most this long, if the server never gets there.
const AT_MOST: Duration = Duration::from_secs(120);

/// The server's state, read in the background while Server is picked.
pub struct Watch {
    pub server: String,
    pub name: String,
    rx: Receiver<Result<calagopus::Resources, String>>,
    stop: Arc<AtomicBool>,
    pub last: Option<calagopus::Resources>,
    pub problem: Option<String>,
    expect: Option<Expect>,
}

/// What Run or Stop asked for, shown until the server gets there.
struct Expect {
    shown: &'static str,
    until: &'static str,
    since: Instant,
}

impl Drop for Watch {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

impl Watch {
    pub fn new(panel: calagopus::Panel, server: String, name: String) -> Watch {
        let (tx, rx) = channel();
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let id = server.clone();
        thread::spawn(move || {
            while !stopped.load(Ordering::Relaxed) {
                if tx.send(panel.resources(&id).map_err(|e| format!("{e:#}"))).is_err() {
                    return;
                }
                let next = Instant::now() + EVERY;
                while Instant::now() < next {
                    if stopped.load(Ordering::Relaxed) {
                        return;
                    }
                    thread::sleep(Duration::from_millis(100));
                }
            }
        });
        Watch { server, name, rx, stop, last: None, problem: None, expect: None }
    }

    /// Takes in what the panel said.
    pub fn poll(&mut self, ctx: &egui::Context) {
        while let Ok(read) = self.rx.try_recv() {
            match read {
                Ok(r) => {
                    self.take(r);
                    self.problem = None;
                }
                Err(e) => self.problem = Some(e),
            }
        }
        if self.expect.as_ref().is_some_and(|e| e.since.elapsed() > AT_MOST) {
            self.expect = None;
        }
        ctx.request_repaint_after(Duration::from_millis(500));
    }

    fn take(&mut self, r: calagopus::Resources) {
        if self.expect.as_ref().is_some_and(|e| e.until == r.state && e.since.elapsed() > AT_LEAST) {
            self.expect = None;
        }
        self.last = Some(r);
    }

    /// Run (start, restart) or Stop (stop, kill) was pressed: that shows
    /// until the server is there.
    pub fn expect(&mut self, action: &str) {
        let (shown, until) = match action {
            "stop" | "kill" => ("stopping", "offline"),
            _ => ("starting", "running"),
        };
        self.expect = Some(Expect { shown, until, since: Instant::now() });
    }

    /// It didn't happen (the upload before it failed).
    pub fn forget_expect(&mut self) {
        self.expect = None;
    }

    /// The server's state as the panel names it: running, starting,
    /// stopping, offline; empty before the first answer.
    pub fn state(&self) -> &str {
        self.expect.as_ref().map(|e| e.shown).or(self.last.as_ref().map(|r| r.state.as_str())).unwrap_or("")
    }

    /// The state as the local bot's, for the Run button.
    pub fn bot_state(&self) -> BotState {
        match self.state() {
            "running" => BotState::Running,
            "starting" | "stopping" => BotState::Starting,
            _ => BotState::Stopped,
        }
    }

    /// The status bar's words for it.
    pub fn label(&self) -> &'static str {
        match self.state() {
            "running" => t!("Server running"),
            "starting" => t!("Server starting"),
            "stopping" => t!("Server stopping"),
            "offline" => t!("Server stopped"),
            _ if self.problem.is_some() => t!("Server unreachable"),
            _ => t!("Server …"),
        }
    }

    /// The status bar's tooltip: which server, CPU and memory, or what went wrong.
    pub fn tip(&self) -> String {
        let mut tip = tf!("On the server: {server}", server = self.name);
        if let Some(p) = &self.problem {
            tip.push_str(&format!("\n{p}"));
        } else if let Some(r) = &self.last {
            let mb = |b: u64| b as f64 / 1_048_576.0;
            let memory = if r.memory_limit > 0 { format!("{:.0} / {:.0} MB", mb(r.memory), mb(r.memory_limit)) } else { format!("{:.0} MB", mb(r.memory)) };
            tip.push('\n');
            tip.push_str(&tf!("CPU {cpu}% · RAM {memory}", cpu = format!("{:.0}", r.cpu), memory = memory));
        }
        tip
    }
}

/// The Local | Server switch beside Run, for a right-to-left row. Returns
/// the side clicked this frame, if it isn't the one picked: true for Server.
pub fn switch(ui: &mut egui::Ui, theme: &Theme, remote: bool) -> Option<bool> {
    let mut picked = None;
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        let h = ui.spacing().interact_size.y.max(24.0);
        // Right to left: Server is drawn first, so it reads Local | Server.
        for (server, label, tip) in [(true, t!("Server"), t!("Run, Restart and Stop act on the bot's server, which the status bar shows")), (false, t!("Local"), t!("Run, Restart and Stop act on the bot on this computer"))] {
            let on = server == remote;
            let ink = if on { if theme.dark { Color32::from_rgb(0, 24, 28) } else { Color32::WHITE } } else { theme.text };
            let corner = if server { egui::CornerRadius { nw: 0, sw: 0, ne: 7, se: 7 } } else { egui::CornerRadius { nw: 7, sw: 7, ne: 0, se: 0 } };
            let b = egui::Button::new(RichText::new(label).size(12.5).color(ink)).corner_radius(corner).min_size(vec2(56.0, h));
            let fill = if on { theme.accent } else { theme.panel };
            if crate::ui::kit::styled_button(ui, theme, b, fill, Stroke::new(1.0, if on { theme.accent } else { theme.node_border })).on_hover_text(tip).clicked() && !on {
                picked = Some(server);
            }
        }
    });
    picked
}

#[cfg(test)]
mod tests {
    use super::*;

    fn res(state: &str) -> calagopus::Resources {
        calagopus::Resources { state: state.into(), memory: 0, memory_limit: 0, cpu: 0.0, uptime: 0 }
    }

    /// The state shown: the panel's, and after Run or Stop what was asked,
    /// until the server is there (not before a few seconds passed, as the
    /// panel may still say the old state).
    #[test]
    fn what_was_asked_shows_until_the_server_is_there() {
        let mut w = Watch::new(calagopus::Panel::new("http://127.0.0.1:9", "c7sp_x"), "u-1".into(), "Bot".into());
        assert_eq!((w.state(), w.label()), ("", "Server …"));
        w.take(res("offline"));
        assert_eq!((w.bot_state(), w.label()), (BotState::Stopped, "Server stopped"));
        w.expect("start");
        w.take(res("offline"));
        assert_eq!((w.bot_state(), w.label()), (BotState::Starting, "Server starting"), "still starting");
        w.take(res("running"));
        assert_eq!(w.label(), "Server starting", "too soon to believe it");
        w.expect.as_mut().unwrap().since -= AT_LEAST + Duration::from_secs(1);
        w.take(res("running"));
        assert_eq!((w.bot_state(), w.label()), (BotState::Running, "Server running"));
        w.expect("stop");
        assert_eq!((w.bot_state(), w.label()), (BotState::Starting, "Server stopping"));
        w.forget_expect();
        assert_eq!(w.label(), "Server running");
    }
}
