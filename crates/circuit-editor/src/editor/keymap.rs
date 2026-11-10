//! Keyboard shortcuts, changed in Settings › Shortcuts.
//!
//! A shortcut is any key (letters, digits, F1–F35, arrows, punctuation,
//! Home, Insert …) or the mouse's side buttons, with any mix of Ctrl, Shift
//! and Alt. Each action has up to two. They match exactly: Ctrl+Z doesn't
//! fire for Ctrl+Shift+Z. Copy, cut and paste stay the system's own keys.

use std::collections::BTreeMap;

use eframe::egui::{self, Event, Key, Modifiers, PointerButton};

/// What a shortcut does.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Action {
    CommandPalette,
    Settings,
    Save,
    NewProject,
    OpenProject,
    /// Closes the tab in front among the workspaces' (as a browser's Ctrl+W).
    CloseTab,
    Run,
    Stop,
    /// Start only (nothing when it runs already).
    Start,
    /// Restart only (nothing when it's stopped).
    Restart,
    BotSettings,
    SlashCommands,
    ExportBot,
    /// Upload to the bot's server.
    Upload,
    ServerConsole,
    ServerSettings,
    /// Share › Git…, and its two actions.
    Git,
    GitStore,
    GitFetch,
    /// Run, Restart and Stop between this computer and the server.
    ToggleRemote,
    Undo,
    Redo,
    Duplicate,
    SelectAll,
    Delete,
    AddBlock,
    AddNote,
    ToggleSnap,
    FitView,
    ZoomIn,
    ZoomOut,
    ZoomReset,
}

/// How many shortcuts an action can have.
pub const PER_ACTION: usize = 2;

impl Action {
    pub const ALL: [Action; 32] = [
        Action::CommandPalette,
        Action::Settings,
        Action::Save,
        Action::NewProject,
        Action::OpenProject,
        Action::CloseTab,
        Action::Run,
        Action::Stop,
        Action::Start,
        Action::Restart,
        Action::BotSettings,
        Action::SlashCommands,
        Action::ExportBot,
        Action::Upload,
        Action::ServerConsole,
        Action::ServerSettings,
        Action::ToggleRemote,
        Action::Git,
        Action::GitStore,
        Action::GitFetch,
        Action::Undo,
        Action::Redo,
        Action::Duplicate,
        Action::SelectAll,
        Action::Delete,
        Action::AddBlock,
        Action::AddNote,
        Action::ToggleSnap,
        Action::FitView,
        Action::ZoomIn,
        Action::ZoomOut,
        Action::ZoomReset,
    ];

    /// Saved in the settings under this name.
    pub fn id(self) -> &'static str {
        match self {
            Action::CommandPalette => "palette",
            Action::Settings => "settings",
            Action::Save => "save",
            Action::CloseTab => "close_tab",
            Action::NewProject => "new_project",
            Action::OpenProject => "open_project",
            Action::Run => "run",
            Action::Stop => "stop",
            Action::Start => "start",
            Action::Restart => "restart",
            Action::BotSettings => "bot_settings",
            Action::SlashCommands => "slash_commands",
            Action::ExportBot => "export_bot",
            Action::Upload => "upload",
            Action::ServerConsole => "server_console",
            Action::ServerSettings => "server_settings",
            Action::ToggleRemote => "toggle_remote",
            Action::Git => "git",
            Action::GitStore => "git_store",
            Action::GitFetch => "git_fetch",
            Action::Undo => "undo",
            Action::Redo => "redo",
            Action::Duplicate => "duplicate",
            Action::SelectAll => "select_all",
            Action::Delete => "delete",
            Action::AddBlock => "add_block",
            Action::AddNote => "add_note",
            Action::ToggleSnap => "toggle_snap",
            Action::FitView => "fit_view",
            Action::ZoomIn => "zoom_in",
            Action::ZoomOut => "zoom_out",
            Action::ZoomReset => "zoom_reset",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Action::CommandPalette => t!("Command palette"),
            Action::CloseTab => t!("Close the tab in front"),
            Action::Settings => t!("Settings"),
            Action::Save => t!("Save (restarts a running bot)"),
            Action::NewProject => t!("New project"),
            Action::OpenProject => t!("Open project"),
            Action::Run => t!("Run or restart the bot"),
            Action::Stop => t!("Stop the bot"),
            Action::Start => t!("Start the bot (when it's stopped)"),
            Action::Restart => t!("Restart the bot (when it runs)"),
            Action::BotSettings => t!("Bot settings"),
            Action::SlashCommands => t!("Slash Commands"),
            Action::ExportBot => t!("Export the bot"),
            Action::Upload => t!("Upload to the remote"),
            Action::ServerConsole => t!("Remote console"),
            Action::ServerSettings => t!("Remote settings (Bot › Remote…)"),
            Action::ToggleRemote => t!("Switch Run between this computer and the remote"),
            Action::Git => t!("Git (Share › Git…)"),
            Action::GitStore => t!("Store the project in its repository"),
            Action::GitFetch => t!("Fetch what is stored in the repository"),
            Action::Undo => t!("Undo"),
            Action::Redo => t!("Redo"),
            Action::Duplicate => t!("Duplicate the selected blocks"),
            Action::SelectAll => t!("Select all blocks"),
            Action::Delete => t!("Delete the selected blocks or wire (or workspaces selected in the Workspaces list)"),
            Action::AddBlock => t!("Add a block at the mouse"),
            Action::AddNote => t!("Add a note"),
            Action::ToggleSnap => t!("Snap to grid on or off"),
            Action::FitView => t!("Show every block"),
            Action::ZoomIn => t!("Zoom in"),
            Action::ZoomOut => t!("Zoom out"),
            Action::ZoomReset => t!("Zoom to 100%"),
        }
    }

    /// The Settings page's group.
    pub fn group(self) -> &'static str {
        match self {
            Action::CommandPalette | Action::Settings | Action::Save | Action::NewProject | Action::OpenProject | Action::CloseTab => t!("General"),
            Action::Run | Action::Stop | Action::Start | Action::Restart | Action::BotSettings | Action::SlashCommands | Action::ExportBot => t!("Bot"),
            Action::Upload | Action::ServerConsole | Action::ServerSettings | Action::ToggleRemote => t!("Remote"),
            Action::Git | Action::GitStore | Action::GitFetch => "Git",
            Action::Undo | Action::Redo | Action::Duplicate | Action::SelectAll | Action::Delete => t!("Editing"),
            _ => t!("Canvas"),
        }
    }

    /// Only while the mouse is over a canvas (and nothing is typed).
    pub fn canvas_only(self) -> bool {
        matches!(self, Action::AddBlock | Action::AddNote | Action::ToggleSnap | Action::FitView | Action::ZoomIn | Action::ZoomOut | Action::ZoomReset)
    }

    pub fn defaults(self) -> Vec<Shortcut> {
        let s = |text: &str| Shortcut::parse(text).expect("default shortcut");
        match self {
            Action::CommandPalette => vec![s("Ctrl+P"), s("Ctrl+Shift+P")],
            Action::Settings => vec![s("Ctrl+,")],
            Action::Save => vec![s("Ctrl+S")],
            Action::NewProject => vec![s("Ctrl+N")],
            Action::OpenProject => vec![s("Ctrl+O")],
            Action::CloseTab => vec![s("Ctrl+W")],
            Action::Run => vec![s("F5")],
            Action::Stop => vec![s("Shift+F5")],
            Action::Undo => vec![s("Ctrl+Z")],
            Action::Redo => vec![s("Ctrl+Y"), s("Ctrl+Shift+Z")],
            Action::Duplicate => vec![s("Ctrl+D")],
            Action::SelectAll => vec![s("Ctrl+A")],
            Action::Delete => vec![s("Delete")],
            Action::AddBlock => vec![s("Space")],
            Action::AddNote => vec![s("N")],
            Action::ToggleSnap => vec![s("G")],
            Action::FitView => vec![s("F")],
            Action::ZoomIn => vec![s("Ctrl+=")],
            Action::ZoomOut => vec![s("Ctrl+-")],
            Action::ZoomReset => vec![s("Ctrl+0")],
            // None to start with: set in Settings › Shortcuts.
            Action::Start | Action::Restart | Action::BotSettings | Action::SlashCommands | Action::ExportBot | Action::Upload | Action::ServerConsole | Action::ServerSettings | Action::ToggleRemote | Action::Git | Action::GitStore | Action::GitFetch => Vec::new(),
        }
    }
}

/// What triggers a shortcut besides the modifiers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trigger {
    Key(Key),
    /// A mouse's side buttons (back and forward).
    Mouse(PointerButton),
}

/// A key or side button with Ctrl, Shift and Alt as needed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shortcut {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub trigger: Trigger,
}

impl Shortcut {
    /// `Ctrl+Shift+K`, `F5`, `Alt+Mouse4`, `Ctrl+,`. Modifier names in any
    /// case and order; Cmd means Ctrl.
    pub fn parse(text: &str) -> Option<Shortcut> {
        let mut s = Shortcut { ctrl: false, shift: false, alt: false, trigger: Trigger::Key(Key::A) };
        let text = text.trim();
        // The last part is the key; it may itself be "+" (as in Ctrl++).
        let (mods, key) = match text.strip_suffix("++") {
            Some(rest) => (rest, "+"),
            None => text.rsplit_once('+').unwrap_or(("", text)),
        };
        for m in mods.split('+').map(str::trim).filter(|m| !m.is_empty()) {
            match m.to_ascii_lowercase().as_str() {
                "ctrl" | "control" | "cmd" | "command" => s.ctrl = true,
                "shift" => s.shift = true,
                "alt" | "option" => s.alt = true,
                _ => return None,
            }
        }
        s.trigger = match key.to_ascii_lowercase().as_str() {
            "mouse4" | "back" => Trigger::Mouse(PointerButton::Extra1),
            "mouse5" | "forward" => Trigger::Mouse(PointerButton::Extra2),
            "del" => Trigger::Key(Key::Delete),
            "esc" => Trigger::Key(Key::Escape),
            _ => Trigger::Key(Key::from_name(key).or_else(|| Key::from_name(&key.to_uppercase()))?),
        };
        Some(s)
    }

    /// As people read it: `Ctrl+Shift+K`.
    pub fn text(&self) -> String {
        let mut out = String::new();
        for (on, name) in [(self.ctrl, "Ctrl"), (self.shift, "Shift"), (self.alt, "Alt")] {
            if on {
                out.push_str(name);
                out.push('+');
            }
        }
        out.push_str(&match self.trigger {
            // Punctuation as itself (Ctrl+,), everything else by name (Delete, Up, Space).
            Trigger::Key(Key::Minus) => "-".to_string(),
            Trigger::Key(k) => match k.symbol_or_name() {
                s if s.len() == 1 && s.chars().all(|c| c.is_ascii_punctuation()) => s.to_string(),
                _ => k.name().to_string(),
            },
            Trigger::Mouse(PointerButton::Extra1) => "Mouse4".into(),
            Trigger::Mouse(_) => "Mouse5".into(),
        });
        out
    }

    fn modifiers(&self) -> Modifiers {
        let mut m = Modifiers::NONE;
        if self.ctrl {
            m |= Modifiers::COMMAND;
        }
        if self.shift {
            m |= Modifiers::SHIFT;
        }
        if self.alt {
            m |= Modifiers::ALT;
        }
        m
    }

    /// Whether `event` presses this shortcut.
    pub fn matches(&self, event: &Event) -> bool {
        match (event, self.trigger) {
            (Event::Key { key, pressed: true, modifiers, .. }, Trigger::Key(k)) => *key == k && modifiers.matches_exact(self.modifiers()),
            (Event::PointerButton { button, pressed: true, modifiers, .. }, Trigger::Mouse(b)) => *button == b && modifiers.matches_exact(self.modifiers()),
            _ => false,
        }
    }

    /// Ctrl or Alt, a function key or a side button: not something typed.
    pub fn works_while_typing(&self) -> bool {
        self.ctrl
            || self.alt
            || match self.trigger {
                Trigger::Mouse(_) => true,
                Trigger::Key(k) => k.name().starts_with('F') && k.name().len() > 1 && k.name()[1..].chars().all(|c| c.is_ascii_digit()),
            }
    }

    /// The shortcut a key or side-button press makes, for recording one.
    /// Escape cancels recording, so it isn't one.
    pub fn from_event(event: &Event) -> Option<Shortcut> {
        let (trigger, m) = match event {
            Event::Key { key: Key::Escape, .. } => return None,
            Event::Key { key, pressed: true, modifiers, .. } => (Trigger::Key(*key), modifiers),
            Event::PointerButton { button: b @ (PointerButton::Extra1 | PointerButton::Extra2), pressed: true, modifiers, .. } => (Trigger::Mouse(*b), modifiers),
            _ => return None,
        };
        Some(Shortcut { ctrl: m.command || m.ctrl, shift: m.shift, alt: m.alt, trigger })
    }
}

/// Shortcuts changed by the user, by action id: their text (`Ctrl+K`). An
/// empty list means none. Actions not in it have their defaults.
pub type Custom = BTreeMap<String, Vec<String>>;

/// An action's shortcuts now.
pub fn shortcuts(custom: &Custom, action: Action) -> Vec<Shortcut> {
    match custom.get(action.id()) {
        Some(list) => list.iter().filter_map(|s| Shortcut::parse(s)).collect(),
        None => action.defaults(),
    }
}

/// An action's first shortcut as text, for menus; empty when it has none.
pub fn label(custom: &Custom, action: Action) -> String {
    shortcuts(custom, action).first().map(Shortcut::text).unwrap_or_default()
}

/// Gives `action` these shortcuts, and takes them away from any other action
/// that had them. Returns the actions they were taken from.
pub fn set(custom: &mut Custom, action: Action, list: Vec<Shortcut>) -> Vec<Action> {
    let mut taken = Vec::new();
    for other in Action::ALL.into_iter().filter(|a| *a != action) {
        let mine = shortcuts(custom, other);
        let kept: Vec<Shortcut> = mine.iter().copied().filter(|s| !list.contains(s)).collect();
        if kept.len() != mine.len() {
            taken.push(other);
            store(custom, other, kept);
        }
    }
    store(custom, action, list);
    taken
}

fn store(custom: &mut Custom, action: Action, list: Vec<Shortcut>) {
    if list == action.defaults() {
        custom.remove(action.id());
    } else {
        custom.insert(action.id().to_string(), list.iter().map(Shortcut::text).collect());
    }
}

/// Whether a key press in another of the app's windows (a tab in a window
/// of its own) is passed on to the main one for its shortcuts: only presses
/// that can't be typing (with Ctrl or Alt, or a function key).
pub fn passed_on(event: &Event) -> bool {
    Shortcut::from_event(event).is_some_and(|s| s.works_while_typing() && matches!(s.trigger, Trigger::Key(_)))
}

/// Whether an action's shortcut was pressed this frame; takes the press, so
/// nothing else reacts to it.
pub fn consume(ctx: &egui::Context, custom: &Custom, action: Action) -> bool {
    let list = shortcuts(custom, action);
    ctx.input_mut(|i| {
        let Some(at) = i.events.iter().position(|e| list.iter().any(|s| s.matches(e))) else { return false };
        i.events.remove(at);
        true
    })
}

/// Like `consume`, but while `typing` only shortcuts with Ctrl or Alt, or
/// on a function key, count: plain keys belong to the text box.
pub fn consume_while(ctx: &egui::Context, custom: &Custom, action: Action, typing: bool) -> bool {
    let list: Vec<Shortcut> = shortcuts(custom, action).into_iter().filter(|s| !typing || s.works_while_typing()).collect();
    ctx.input_mut(|i| {
        let Some(at) = i.events.iter().position(|e| list.iter().any(|s| s.matches(e))) else { return false };
        i.events.remove(at);
        true
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(key: Key, modifiers: Modifiers) -> Event {
        Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers }
    }

    #[test]
    fn reads_and_writes_shortcuts() {
        for text in ["Ctrl+S", "Ctrl+Shift+Z", "Alt+F4", "F5", "Shift+F12", "Ctrl+,", "Ctrl+=", "Ctrl+-", "Space", "Delete", "Ctrl+Shift+Alt+Home", "Mouse4", "Alt+Mouse5", "Ctrl+[", "F24"] {
            let s = Shortcut::parse(text).unwrap_or_else(|| panic!("{text}"));
            assert_eq!(s.text(), text, "round trip");
        }
        assert_eq!(Shortcut::parse("cmd+shift+p").unwrap().text(), "Ctrl+Shift+P");
        assert_eq!(Shortcut::parse("Alt+Ctrl+Home").unwrap().text(), "Ctrl+Alt+Home", "any order in, one order out");
        assert_eq!(Shortcut::parse("Del").unwrap().text(), "Delete");
        assert!(Shortcut::parse("Hyper+K").is_none() && Shortcut::parse("Ctrl+NoSuchKey").is_none());
        // Every default is readable (the newer actions have none to start
        // with), and every key egui knows reads back.
        for a in Action::ALL {
            assert!(!a.defaults().is_empty() || a.group() == "Remote" || a.group() == "Git" || matches!(a, Action::Start | Action::Restart | Action::BotSettings | Action::SlashCommands | Action::ExportBot), "{a:?}");
            assert!(!a.id().is_empty() && Action::ALL.iter().filter(|b| b.id() == a.id()).count() == 1, "{a:?}: one id each");
        }
        for k in Key::ALL {
            let s = Shortcut { ctrl: true, shift: false, alt: true, trigger: Trigger::Key(*k) };
            assert_eq!(Shortcut::parse(&s.text()), Some(s), "{k:?} as {}", s.text());
        }
    }

    #[test]
    fn matches_exactly() {
        let undo = Shortcut::parse("Ctrl+Z").unwrap();
        assert!(undo.matches(&key(Key::Z, Modifiers::COMMAND)));
        assert!(!undo.matches(&key(Key::Z, Modifiers::COMMAND | Modifiers::SHIFT)), "Ctrl+Shift+Z is redo, not undo");
        assert!(!undo.matches(&key(Key::Z, Modifiers::NONE)));
        let back = Shortcut::parse("Mouse4").unwrap();
        assert!(back.matches(&Event::PointerButton { pos: Default::default(), button: PointerButton::Extra1, pressed: true, modifiers: Modifiers::NONE }));
        // Recording: the press becomes the shortcut; Escape cancels.
        assert_eq!(Shortcut::from_event(&key(Key::K, Modifiers::COMMAND | Modifiers::ALT)).unwrap().text(), "Ctrl+Alt+K");
        assert!(Shortcut::from_event(&key(Key::Escape, Modifiers::NONE)).is_none());
    }

    #[test]
    fn plain_keys_wait_while_typing() {
        let mut custom = Custom::new();
        set(&mut custom, Action::Save, vec![Shortcut::parse("S").unwrap(), Shortcut::parse("Ctrl+S").unwrap()]);
        let ctx = egui::Context::default();
        let press = |k: Key, m: Modifiers, typing: bool| {
            let mut hit = false;
            let raw = egui::RawInput { events: vec![key(k, m)], ..Default::default() };
            ctx.run_ui(raw, |ui| hit = consume_while(ui.ctx(), &custom, Action::Save, typing)).textures_delta.clear();
            hit
        };
        assert!(press(Key::S, Modifiers::NONE, false));
        assert!(!press(Key::S, Modifiers::NONE, true), "an S typed into a text box is just an S");
        assert!(press(Key::S, Modifiers::COMMAND, true));
        assert!(Shortcut::parse("F5").unwrap().works_while_typing() && Shortcut::parse("Mouse4").unwrap().works_while_typing());
        assert!(!Shortcut::parse("F").unwrap().works_while_typing() && !Shortcut::parse("Shift+Space").unwrap().works_while_typing());
    }

    #[test]
    fn a_shortcut_belongs_to_one_action() {
        let mut custom = Custom::new();
        let ctrl_s = Shortcut::parse("Ctrl+S").unwrap();
        // Moving Ctrl+S to Run takes it from Save.
        let taken = set(&mut custom, Action::Run, vec![ctrl_s]);
        assert_eq!(taken, vec![Action::Save]);
        assert!(shortcuts(&custom, Action::Save).is_empty());
        assert_eq!(label(&custom, Action::Run), "Ctrl+S");
        // Back to the defaults: nothing stays stored for it.
        set(&mut custom, Action::Run, Action::Run.defaults());
        assert!(!custom.contains_key("run"));
    }

    /// The newer actions (Upload, Start, Restart …) have no shortcut to
    /// start with, and take one like the others.
    #[test]
    fn newer_actions_take_a_shortcut() {
        let mut custom = Custom::new();
        assert!(shortcuts(&custom, Action::Upload).is_empty() && label(&custom, Action::Upload).is_empty());
        set(&mut custom, Action::Upload, vec![Shortcut::parse("Ctrl+U").unwrap()]);
        assert_eq!(label(&custom, Action::Upload), "Ctrl+U");
        let ctx = egui::Context::default();
        let press = key(Key::U, Modifiers::COMMAND);
        let raw = egui::RawInput { events: vec![press], ..Default::default() };
        let mut fired = false;
        let mut out = ctx.run_ui(raw, |_| fired = consume(&ctx, &custom, Action::Upload));
        out.textures_delta.clear();
        assert!(fired);
        // Taken back to none, nothing stays stored.
        set(&mut custom, Action::Upload, Vec::new());
        assert!(custom.is_empty());
    }

    /// From a tab's window of its own, only presses that can't be typing
    /// reach the main window's shortcuts.
    #[test]
    fn other_windows_pass_on_only_real_shortcuts() {
        assert!(passed_on(&key(Key::S, Modifiers::COMMAND)));
        assert!(passed_on(&key(Key::F5, Modifiers::NONE)));
        assert!(passed_on(&key(Key::F5, Modifiers::SHIFT)));
        assert!(!passed_on(&key(Key::N, Modifiers::NONE)), "typed");
        assert!(!passed_on(&key(Key::Delete, Modifiers::NONE)), "deleting text");
        assert!(!passed_on(&key(Key::Escape, Modifiers::NONE)));
    }
}
