//! The tabs closed lately, to bring back with Ctrl+Shift+T as a browser
//! does: where each was among its area's tabs, and for a builder or the
//! bot's settings what was being edited, so it comes back as it was.

use super::*;
use crate::builders::Kind;

/// What a closed builder or settings tab was editing.
pub(super) enum Draft {
    None,
    Embed(crate::builders::embed::EmbedEdit),
    Layout(crate::builders::layout::LayoutEdit),
    Modal(crate::builders::modal::ModalEdit),
    Canvas(crate::builders::canvas::CanvasEdit),
    Slash(crate::builders::slash::SlashEdit),
    BotSettings(crate::bot::SettingsDraft),
}

pub(super) struct Closed {
    pub(super) tab: Tab,
    pub(super) draft: Draft,
    /// Another tab of the area it was in, and its place among that area's tabs.
    pub(super) place: Option<(Tab, usize)>,
}

/// How many closed tabs are remembered.
pub(super) const MOST: usize = 25;

/// Where every tab of the main area is: another tab of its area (none when
/// it is alone there) and its place among them.
pub(super) fn places(dock: &Dock<Tab>) -> std::collections::HashMap<Tab, (Option<Tab>, usize)> {
    let mut out = std::collections::HashMap::new();
    for leaf in dock.main.iter().flat_map(|m| m.leaves()) {
        for (i, tab) in leaf.tabs.iter().enumerate() {
            out.insert(tab.clone(), (leaf.tabs.iter().find(|t| *t != tab).cloned(), i));
        }
    }
    out
}

/// Whether a closed tab of this kind is remembered. Builders and the bot's
/// settings only with what they were editing (see [`Draft`]); what opens in
/// a window of its own and the remote's console are not.
pub(super) fn remembered(tab: &Tab, with_draft: bool) -> bool {
    match tab {
        Tab::Canvas(_) | Tab::Settings | Tab::Backups | Tab::PluginPage(_) | Tab::Inspector | Tab::Console | Tab::Problems | Tab::Side(_) | Tab::Plugin(..) | Tab::Git => true,
        Tab::Builder(..) | Tab::SlashCommands | Tab::BotSettings => with_draft,
        _ => false,
    }
}

impl CircuitApp {
    /// Takes in what was closed this frame.
    pub(super) fn keep_closed(&mut self, closed: Vec<Closed>) {
        self.closed_tabs.extend(closed);
        if self.closed_tabs.len() > MOST {
            let cut = self.closed_tabs.len() - MOST;
            self.closed_tabs.drain(..cut);
        }
    }

    /// Brings back the tab closed last that can still be shown: where it
    /// was, and a builder with what it was editing. Returns whether one was.
    pub(super) fn reopen_tab(&mut self) -> bool {
        while let Some(closed) = self.closed_tabs.pop() {
            let Closed { tab, draft, place } = closed;
            // Open again meanwhile, or about something that is gone.
            let gone = match &tab {
                Tab::Canvas(key) => self.ed.workspace(key).is_none(),
                Tab::Plugin(plugin, panel) => self.plugin_host.panel_title(plugin, panel).is_none(),
                Tab::PluginPage(id) => !self.ed.reg.plugins.iter().chain(self.ed.reg.disabled.iter()).any(|p| p.manifest.id == *id),
                _ => false,
            };
            // A builder for the same field was opened again meanwhile: that one stays.
            let field_open = match &draft {
                Draft::Embed(d) => self.ed.builder_open(Kind::Embed, &d.ws, &d.node, &d.key),
                Draft::Layout(d) => self.ed.builder_open(Kind::Layout, &d.ws, &d.node, &d.key),
                Draft::Modal(d) => self.ed.builder_open(Kind::Modal, &d.ws, &d.node, &d.key),
                Draft::Canvas(d) => self.ed.builder_open(Kind::Canvas, &d.ws, &d.node, &d.key),
                _ => false,
            };
            if gone || field_open || self.dock.contains(&tab) || self.ed.project.is_none() {
                continue;
            }
            // An icon in the activity bar now: shown there.
            if tab.side_key().is_some_and(|k| self.ed.prefs.activity.contains(&k)) {
                self.show_panel(tab);
                return true;
            }
            // A builder comes back with what it was editing (and its number, so its tab).
            let taken = match draft {
                Draft::None => false,
                Draft::Embed(d) => {
                    self.ed.embed_edits.push(d);
                    false
                }
                Draft::Layout(d) => {
                    self.ed.layout_edits.push(d);
                    false
                }
                Draft::Modal(d) => {
                    self.ed.modal_edits.push(d);
                    false
                }
                Draft::Canvas(d) => {
                    self.ed.canvas_edits.push(d);
                    false
                }
                Draft::Slash(d) => self.ed.slash_edit.replace(d).is_some(),
                Draft::BotSettings(d) => self.bot.settings.replace(d).is_some(),
            };
            debug_assert!(!taken, "its tab would have been open");
            match place {
                Some((anchor, index)) if self.dock.contains(&anchor) => self.dock.put(tab.clone(), &crate::ui::dock::Target::Tabs { anchor, index }),
                _ => place_panel(&mut self.dock, tab.clone()),
            }
            self.dock.focus(&tab);
            if let Tab::Canvas(key) = &tab {
                self.ed.active_workspace = Some(key.clone());
            }
            self.drop_placeholder();
            return true;
        }
        false
    }
}
