//! Project Stats: an example native plugin.
//!
//! - a dockable "Project Stats" panel drawn with egui: counts, a bar chart of
//!   the most used blocks, and what the running bot has been doing
//! - a "Log project summary" command
//! - a check that lists blocks with no wires at all

use std::collections::BTreeMap;

use serde_json::Value;
use xc_circuit_plugin::egui::{self, Align2, Color32, FontId, Sense, Stroke, vec2};
use xc_circuit_plugin::{BotEvent, Command, Host, Level, PanelInfo, Plugin, Problem};

#[derive(Default)]
struct Stats {
    /// Runs and failures of the current bot session.
    runs: u64,
    errors: Vec<(String, String, String)>,
    slowest: Option<(String, String, f64)>,
}

/// Counts for the open project.
#[derive(Default)]
struct Counts {
    workspaces: usize,
    blocks: usize,
    wires: usize,
    zones: usize,
    /// Block type → (uses, first workspace and node using it).
    by_type: BTreeMap<String, (usize, String, String)>,
}

fn workspaces(host: &dyn Host) -> Vec<(String, Value)> {
    host.workspaces()
        .into_iter()
        .filter_map(|w| Some((w.key.clone(), serde_json::from_str(&host.workspace_json(&w.key)?).ok()?)))
        .collect()
}

/// Block type without its `@major` suffix.
fn block_type(node: &Value) -> String {
    node["block"].as_str().unwrap_or_default().split('@').next().unwrap_or_default().to_string()
}

fn count(host: &dyn Host) -> Counts {
    let mut c = Counts::default();
    for (key, ws) in workspaces(host) {
        c.workspaces += 1;
        c.wires += ws["links"].as_array().map_or(0, |l| l.len());
        c.zones += ws["zones"].as_object().map_or(0, |z| z.len());
        for (id, node) in ws["nodes"].as_object().into_iter().flatten() {
            c.blocks += 1;
            let entry = c.by_type.entry(block_type(node)).or_insert((0, key.clone(), id.clone()));
            entry.0 += 1;
        }
    }
    c
}

fn stat(ui: &mut egui::Ui, label: &str, value: String) {
    ui.vertical(|ui| {
        ui.label(egui::RichText::new(value).size(22.0).strong());
        ui.label(egui::RichText::new(label).size(11.0).weak());
    });
}

impl Plugin for Stats {
    fn commands(&self) -> Vec<Command> {
        vec![Command::new("stats.summary", "Log project summary").description("Writes block, wire and workspace counts to the Console")]
    }

    fn run_command(&mut self, _id: &str, host: &mut dyn Host) {
        let c = count(host);
        let name = host.project_name().unwrap_or_else(|| "No project".into());
        host.log(Level::Info, &format!("{name}: {} workspaces, {} blocks, {} wires, {} Try Zones", c.workspaces, c.blocks, c.wires, c.zones));
    }

    fn panels(&self) -> Vec<PanelInfo> {
        vec![PanelInfo::new("stats", "Project Stats")]
    }

    fn panel_ui(&mut self, _id: &str, ui: &mut egui::Ui, host: &mut dyn Host) {
        let Some(name) = host.project_name() else {
            ui.label("Open a project to see its stats.");
            return;
        };
        let c = count(host);
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            egui::Frame::new().inner_margin(12).show(ui, |ui| {
                ui.label(egui::RichText::new(name).size(15.0).strong());
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 22.0;
                    stat(ui, "workspaces", c.workspaces.to_string());
                    stat(ui, "blocks", c.blocks.to_string());
                    stat(ui, "wires", c.wires.to_string());
                    stat(ui, "Try Zones", c.zones.to_string());
                });

                ui.add_space(14.0);
                ui.label(egui::RichText::new("MOST USED BLOCKS").size(10.5).weak());
                ui.add_space(4.0);
                let mut top: Vec<_> = c.by_type.iter().collect();
                top.sort_by(|a, b| b.1.0.cmp(&a.1.0).then(a.0.cmp(b.0)));
                let max = top.first().map_or(1, |t| t.1.0).max(1) as f32;
                let accent = ui.visuals().selection.bg_fill;
                for (ty, (uses, ws, node)) in top.into_iter().take(8) {
                    let width = ui.available_width().min(420.0);
                    let (rect, resp) = ui.allocate_exact_size(vec2(width, 22.0), Sense::click());
                    let label = host.block_name(ty).unwrap_or_else(|| ty.clone());
                    let bar_w = (rect.width() - 150.0).max(20.0) * (*uses as f32 / max);
                    let bar = egui::Rect::from_min_size(rect.min + vec2(140.0, 5.0), vec2(bar_w, 12.0));
                    let p = ui.painter();
                    if resp.hovered() {
                        p.rect_filled(rect, 4.0, ui.visuals().faint_bg_color);
                    }
                    p.text(rect.left_center() + vec2(4.0, 0.0), Align2::LEFT_CENTER, &label, FontId::proportional(12.5), ui.visuals().text_color());
                    p.rect_filled(bar, 3.0, accent);
                    p.text(bar.right_center() + vec2(6.0, 0.0), Align2::LEFT_CENTER, uses.to_string(), FontId::monospace(11.0), ui.visuals().weak_text_color());
                    if resp.on_hover_text("Show the first one").clicked() {
                        host.reveal(ws, node);
                    }
                }

                ui.add_space(14.0);
                ui.label(egui::RichText::new("RUNNING BOT").size(10.5).weak());
                ui.add_space(4.0);
                if !host.bot_running() && self.runs == 0 {
                    ui.label(egui::RichText::new("Run the bot to see block activity here.").weak());
                } else {
                    let total: u64 = host.storage_get("allTimeRuns").and_then(|s| s.parse().ok()).unwrap_or(0);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 22.0;
                        stat(ui, "block runs", self.runs.to_string());
                        stat(ui, "errors", self.errors.len().to_string());
                        stat(ui, "runs, all time", total.to_string());
                    });
                    if let Some((ws, node, ms)) = self.slowest.clone() {
                        ui.add_space(6.0);
                        if ui.link(format!("Slowest block: #{node} took {ms:.1} ms")).clicked() {
                            host.reveal(&ws, &node);
                        }
                    }
                    for (ws, node, error) in self.errors.iter().rev().take(5) {
                        ui.horizontal(|ui| {
                            ui.colored_label(Color32::from_rgb(0xFF, 0x6B, 0x6B), "●");
                            if ui.link(format!("#{node}")).clicked() {
                                host.reveal(ws, node);
                            }
                            ui.label(egui::RichText::new(error).weak());
                        });
                    }
                }
                ui.add_space(10.0);
                ui.painter().hline(ui.min_rect().x_range(), ui.cursor().top(), Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color));
                ui.add_space(6.0);
                ui.label(egui::RichText::new("Native plugin · plugins/xc.stats").size(10.5).weak());
            });
        });
        // Keep the bot section live while it runs.
        if host.bot_running() {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(500));
        }
    }

    fn check(&mut self, host: &mut dyn Host) -> Vec<Problem> {
        let mut problems = Vec::new();
        for (key, ws) in workspaces(host) {
            let Some(nodes) = ws["nodes"].as_object() else { continue };
            if nodes.len() < 2 {
                continue;
            }
            let links: Vec<&str> = ws["links"].as_array().into_iter().flatten().filter_map(|l| l.as_str()).collect();
            for (id, node) in nodes {
                let wired = links.iter().any(|l| l.split(" > ").any(|end| end.split('.').next() == Some(id.as_str())));
                if !wired {
                    let name = host.block_name(&block_type(node)).unwrap_or_else(|| block_type(node));
                    problems.push(Problem::new(format!("{name} (#{id}) isn't connected to anything")).at(&key, id));
                }
            }
        }
        problems
    }

    fn on_bot_event(&mut self, event: &BotEvent, host: &mut dyn Host) {
        match event {
            BotEvent::Started => *self = Stats::default(),
            BotEvent::Block { workspace, node, phase, ms, error } => {
                if let Some(e) = error {
                    self.errors.push((workspace.clone(), node.clone(), e.clone()));
                } else if phase == "exit" {
                    self.runs += 1;
                    let total: u64 = host.storage_get("allTimeRuns").and_then(|s| s.parse().ok()).unwrap_or(0);
                    host.storage_set("allTimeRuns", &(total + 1).to_string());
                    if let Some(ms) = ms
                        && self.slowest.as_ref().is_none_or(|s| *ms > s.2)
                    {
                        self.slowest = Some((workspace.clone(), node.clone(), *ms));
                    }
                }
            }
            _ => {}
        }
    }
}

xc_circuit_plugin::export_plugin!(Stats);
