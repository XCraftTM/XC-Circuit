//! Bot › Settings: a page per category, like the app's Settings. The build
//! mode and plugins say what's on them (see circuit_plugins::bot_settings).
//! Changes wait for Save; secrets go to the system keyring.

use std::time::{Duration, Instant};

use circuit_plugins::Setting;
use circuit_runner::secrets;
use eframe::egui::{self, Align, Frame, Id, Layout, Margin, Rect, RichText, ScrollArea, Stroke, Ui, pos2, vec2};
use serde_json::Value;

use super::BotUi;
use super::databases::{self, DatabasesDraft};
use crate::editor::{Editor, Level};
use crate::pages::settings::{NAV_W, group, nav_item, row};
use crate::ui::dialog;
use crate::ui::theme::Theme;

/// The page of privileged intents, shown for build modes that have them.
const INTENTS: &str = "xc:intents";
/// How long the Copy button shows a check mark.
const COPIED_FOR: Duration = Duration::from_millis(1500);
/// The widest a page's settings get.
const CONTENT_MAX: f32 = 620.0;

#[derive(Clone, Debug, PartialEq)]
enum Kind {
    Text,
    Secret,
    Select(Vec<(String, String)>),
    Boolean,
    Number { min: Option<f64>, max: Option<f64> },
}

/// One setting being edited.
struct SettingField {
    key: String,
    page: String,
    /// A heading above it on its page.
    group: Option<String>,
    label: String,
    description: Option<String>,
    placeholder: String,
    kind: Kind,
    /// Typed like a password, with an eye button to show it (every secret is).
    masked: bool,
    required: bool,
    /// The value must match this, when not empty (`pattern`).
    pattern: Option<regex::Regex>,
    /// Shown when the value doesn't match (`patternHint`).
    pattern_hint: Option<String>,
    /// Text, a number as typed, or the chosen value of a select.
    value: String,
    /// A boolean's value.
    on: bool,
    /// What the project has, or else the default: saving leaves an
    /// unchanged default out of the project.
    default: Option<Value>,
    /// Secrets only: whether one is already stored.
    saved: bool,
    reveal: bool,
    remove: bool,
    /// When the Copy button was last used, to show that it worked.
    copied: Option<Instant>,
}

/// A field pattern must match the whole value.
pub(super) fn compile_pattern(pattern: &str) -> Result<regex::Regex, regex::Error> {
    regex::Regex::new(&format!("^(?:{pattern})$"))
}

/// Why a typed value isn't accepted, if it isn't. Empty values are left to `required`.
pub(super) fn pattern_problem(pattern: Option<&regex::Regex>, hint: Option<&str>, value: &str) -> Option<String> {
    let value = value.trim();
    match pattern {
        Some(re) if !value.is_empty() && !re.is_match(value) => Some(hint.unwrap_or(t!("This doesn't look right.")).to_string()),
        _ => None,
    }
}

/// Why a typed number isn't accepted, if it isn't.
fn number_problem(value: &str, min: Option<f64>, max: Option<f64>) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    let Ok(n) = value.parse::<f64>() else { return Some(t!("A number, such as 10.").to_string()) };
    match (min, max) {
        (Some(min), _) if n < min => Some(tf!("At least {min}.", min = min)),
        (_, Some(max)) if n > max => Some(tf!("At most {max}.", max = max)),
        _ => None,
    }
}

impl SettingField {
    fn new(s: &Setting, stored: Option<&Value>, saved_secret: bool) -> (SettingField, Option<String>) {
        let tr = |s: &str| crate::i18n::tr(s).into_owned();
        let text = |key: &str| s.text(key).map(str::to_string);
        let number = |key: &str| s.spec.get(key).and_then(Value::as_f64);
        let kind = match s.kind() {
            "secret" => Kind::Secret,
            "boolean" => Kind::Boolean,
            "number" => Kind::Number { min: number("min"), max: number("max") },
            "select" => Kind::Select(match s.spec.get("choices") {
                Some(Value::Object(map)) => map.iter().map(|(v, l)| (v.clone(), l.as_str().map(tr).unwrap_or_else(|| v.clone()))).collect(),
                Some(Value::Array(list)) => list.iter().filter_map(Value::as_str).map(|v| (v.to_string(), tr(v))).collect(),
                _ => Vec::new(),
            }),
            _ => Kind::Text,
        };
        let mut bad = None;
        let pattern = text("pattern").and_then(|p| match compile_pattern(&p) {
            Ok(re) => Some(re),
            Err(e) => {
                bad = Some(format!("The pattern of setting {} ({}) doesn't work, so it isn't checked: {e}", s.key, s.plugin));
                None
            }
        });
        let default = s.spec.get("default").cloned();
        let current = if kind == Kind::Secret { None } else { stored.or(default.as_ref()) };
        let value = match current {
            Some(Value::String(v)) => v.clone(),
            Some(Value::Number(n)) => n.to_string(),
            Some(Value::Bool(b)) => b.to_string(),
            _ => String::new(),
        };
        let on = current.is_some_and(|v| v.as_bool() == Some(true) || v.as_str() == Some("true"));
        let field = SettingField {
            key: s.key.clone(),
            page: s.page.clone(),
            group: s.group.as_deref().map(tr),
            label: text("label").map(|l| tr(&l)).unwrap_or_else(|| s.key.clone()),
            description: text("description").map(|d| tr(&d)),
            placeholder: text("placeholder").map(|p| tr(&p)).unwrap_or_default(),
            masked: kind == Kind::Secret || s.spec.get("sensitive").and_then(Value::as_bool).unwrap_or(false),
            required: s.spec.get("required").and_then(Value::as_bool).unwrap_or(false),
            kind,
            pattern,
            pattern_hint: text("patternHint").map(|h| tr(&h)),
            value,
            on,
            default,
            saved: saved_secret,
            reveal: false,
            remove: false,
            copied: None,
        };
        (field, bad)
    }

    /// What has to change before saving, if anything.
    fn problem(&self) -> Option<String> {
        match &self.kind {
            Kind::Number { min, max } => number_problem(&self.value, *min, *max),
            Kind::Text | Kind::Secret => pattern_problem(self.pattern.as_ref(), self.pattern_hint.as_deref(), &self.value),
            _ => None,
        }
    }

    /// A required setting that has no value (and won't have one after saving).
    fn missing(&self) -> bool {
        self.required
            && match self.kind {
                Kind::Secret => self.value.trim().is_empty() && (!self.saved || self.remove),
                Kind::Boolean => false,
                _ => self.value.trim().is_empty(),
            }
    }

    /// The value to keep in the project; None to leave it out.
    fn project_value(&self) -> Option<Value> {
        match &self.kind {
            Kind::Secret => None,
            Kind::Boolean => Some(Value::Bool(self.on)),
            Kind::Number { .. } => {
                let n: f64 = self.value.trim().parse().ok()?;
                Some(if n.fract() == 0.0 && n.abs() < 9e15 { Value::from(n as i64) } else { Value::from(n) })
            }
            _ => Some(Value::String(self.value.trim().to_string())),
        }
    }
}

/// A page in the list on the left.
struct PageInfo {
    id: String,
    name: String,
    icon: Option<String>,
    description: Option<String>,
}

pub struct SettingsDraft {
    fields: Vec<SettingField>,
    pages: Vec<PageInfo>,
    /// The page shown.
    page: String,
    project_name: String,
    /// Privileged intents the project's blocks need: what the platform calls
    /// each, and the blocks that need it.
    intents: Vec<(String, Vec<String>)>,
    /// Where to turn them on, for this bot.
    intents_page: Option<String>,
    /// The Databases page.
    databases: DatabasesDraft,
}

impl SettingsDraft {
    /// Problems and missing required values on a page, for its mark in the list.
    fn page_marks(&self, page: &str) -> (usize, usize) {
        if page == databases::PAGE {
            return (self.databases.problems(), 0);
        }
        let on = || self.fields.iter().filter(move |f| f.page == page);
        (on().filter(|f| f.problem().is_some()).count(), on().filter(|f| f.missing()).count())
    }
}

/// The privileged intents `mode` lists that the project's blocks need, with
/// the names of those blocks.
pub(super) fn privileged_intents(project: &circuit_core::Project, reg: &circuit_plugins::Registry, mode: &circuit_plugins::BuildMode) -> Vec<(String, Vec<String>)> {
    let mut out: Vec<(String, Vec<String>)> = mode.privileged_intents.values().map(|label| (label.clone(), Vec::new())).collect();
    for ws in project.workspaces.values() {
        for node in ws.nodes.values() {
            let Some(def) = reg.block(&node.block) else { continue };
            for (i, intent) in mode.privileged_intents.keys().enumerate() {
                if def.intents.contains(intent) && !out[i].1.contains(&def.name) {
                    out[i].1.push(def.name.clone());
                }
            }
        }
    }
    out.retain(|(_, blocks)| !blocks.is_empty());
    out
}

/// A Discord bot's application id: the first part of its token is the id,
/// in base64.
pub(super) fn app_id_from_token(token: &str) -> Option<String> {
    let first = token.trim().split('.').next()?;
    let mut bits: u32 = 0;
    let mut count = 0;
    let mut out = Vec::new();
    for c in first.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' => break,
            _ => return None,
        } as u32;
        bits = (bits << 6) | v;
        count += 6;
        if count >= 8 {
            count -= 8;
            out.push((bits >> count) as u8);
            bits &= (1 << count) - 1;
        }
    }
    let id = String::from_utf8(out).ok()?;
    (id.len() >= 15 && id.bytes().all(|b| b.is_ascii_digit())).then_some(id)
}

/// The secrets of the project's build mode, plugins and databases, with
/// what to call them: they go to the bot as `XC_SECRET_<KEY>`.
pub(super) fn secret_keys(ed: &Editor) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = ed
        .mode()
        .map(|m| ed.reg.bot_settings(m).settings.into_iter().filter(Setting::is_secret).map(|s| (s.key.clone(), s.text("label").unwrap_or(&s.key).to_string())).collect())
        .unwrap_or_default();
    if let Some(p) = &ed.project {
        out.extend(databases::secret_keys(&p.meta.settings));
    }
    out
}

impl BotUi {
    pub fn open_settings(&mut self, ed: &mut Editor) {
        let (Some(project), Some(mode)) = (&ed.project, ed.mode()) else { return };
        let tr = |s: &str| crate::i18n::tr(s).into_owned();
        let all = ed.reg.bot_settings(mode);
        let mut warnings = all.problems.clone();
        let fields: Vec<SettingField> = all
            .settings
            .iter()
            .map(|s| {
                let saved = s.is_secret() && secrets::get(&project.root, &s.key).is_some();
                let (field, bad) = SettingField::new(s, project.meta.settings.get(&s.key), saved);
                warnings.extend(bad);
                field
            })
            .collect();
        let mut pages: Vec<PageInfo> = all.pages.iter().map(|p| PageInfo { id: p.id.clone(), name: tr(&p.name), icon: p.icon.clone(), description: p.description.as_deref().map(tr) }).collect();
        pages.push(PageInfo {
            id: databases::PAGE.into(),
            name: t!("Databases").into(),
            icon: Some("database".into()),
            description: Some(t!("Connections for the database blocks (MariaDB, MySQL, PostgreSQL, SQLite, MongoDB), and where stored values are kept.").into()),
        });
        if !mode.privileged_intents.is_empty() {
            pages.push(PageInfo { id: INTENTS.into(), name: t!("Privileged intents").into(), icon: Some("shield".into()), description: None });
        }
        let intents = privileged_intents(project, &ed.reg, mode);
        let app_id = secrets::get(&project.root, "token").and_then(|t| app_id_from_token(&t));
        let intents_page = mode.intents_page.as_ref().map(|page| match &app_id {
            Some(id) => page.replace("{appId}", id),
            // Without the bot's id: the list of the account's applications.
            None => page.split("{appId}").next().unwrap_or(page).to_string(),
        });
        // Reopening (after Save, or Undo) stays on the page shown.
        let page = self.settings.as_ref().map(|d| d.page.clone()).filter(|p| pages.iter().any(|x| x.id == *p)).or_else(|| pages.first().map(|p| p.id.clone())).unwrap_or_default();
        let databases = DatabasesDraft::load(&project.meta.settings, &project.root);
        self.settings = Some(SettingsDraft { fields, pages, page, project_name: project.meta.name.clone(), intents, intents_page, databases });
        for line in warnings {
            ed.log(Level::Warn, line);
        }
    }

    /// The Bot settings tab: the pages on the left, the one shown on the right,
    /// and Save at the bottom.
    pub fn settings_tab(&mut self, ui: &mut Ui, ed: &mut Editor) {
        let Some(draft) = &mut self.settings else { return };
        let theme = ed.theme.clone();
        let problems = draft.fields.iter().filter(|f| f.problem().is_some()).count() + draft.databases.problems();
        let (mut save, mut close, mut cancel) = (false, false, false);
        let mut test = None;

        egui::Panel::bottom(Id::new("bot-settings-footer"))
            .resizable(false)
            .frame(Frame::new().fill(theme.panel_header).stroke(Stroke::new(1.0, theme.line)).inner_margin(Margin::symmetric(16, 12)))
            .show(ui, |ui| {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    ui.add_enabled_ui(problems == 0, |ui| {
                        if dialog::primary_button(ui, &theme, t!("Save and close")).clicked() {
                            (save, close) = (true, true);
                        }
                        if dialog::secondary_button(ui, &theme, t!("Save")).clicked() {
                            save = true;
                        }
                    });
                    cancel = dialog::secondary_button(ui, &theme, t!("Cancel")).clicked();
                    if problems > 0 {
                        ui.label(RichText::new(tf!("{count} to fix", count = problems)).size(12.0).color(theme.warning));
                    }
                });
            });

        egui::CentralPanel::default().frame(Frame::new().fill(theme.panel)).show(ui, |ui| {
            let full = ui.max_rect();
            // The pages.
            let nav = Rect::from_min_size(full.min, vec2(NAV_W, full.height()));
            ui.painter().rect_filled(nav, 0.0, theme.frame);
            ui.painter().vline(nav.right(), nav.y_range(), Stroke::new(1.0, theme.line));
            let mut nav_ui = ui.new_child(egui::UiBuilder::new().max_rect(nav.shrink2(vec2(8.0, 14.0))).layout(Layout::top_down(Align::Min)));
            nav_ui.label(RichText::new(t!("BOT SETTINGS")).size(11.0).strong().color(theme.text_muted));
            nav_ui.add_space(2.0);
            nav_ui.add(egui::Label::new(RichText::new(tf!("{project} · stored per project", project = draft.project_name)).size(11.5).color(theme.text_muted)).truncate());
            nav_ui.add_space(8.0);
            let mut picked = None;
            for p in &draft.pages {
                let (bad, missing) = draft.page_marks(&p.id);
                let badge = if bad > 0 {
                    Some((bad.to_string(), theme.error))
                } else if missing > 0 {
                    Some((missing.to_string(), theme.warning))
                } else if p.id == INTENTS && !draft.intents.is_empty() {
                    Some((draft.intents.len().to_string(), theme.text_muted))
                } else {
                    None
                };
                let icon = p.icon.as_deref().and_then(|i| ed.reg.icons.get(i)).map(|p| p.as_path());
                let resp = nav_item(&mut nav_ui, &theme, icon, &p.name, draft.page == p.id, badge);
                let resp = if bad > 0 {
                    resp.on_hover_text(tf!("{count} to fix", count = bad))
                } else if missing > 0 {
                    resp.on_hover_text(t!("Something required isn't set yet"))
                } else {
                    resp
                };
                if resp.clicked() {
                    picked = Some(p.id.clone());
                }
            }
            if let Some(p) = picked {
                draft.page = p;
            }

            // The page shown.
            let content = Rect::from_min_max(pos2(nav.right() + 1.0, full.top()), full.max);
            let mut content_ui = ui.new_child(egui::UiBuilder::new().max_rect(content).layout(Layout::top_down(Align::Min)));
            ScrollArea::vertical().id_salt(("bot-settings", draft.page.as_str())).auto_shrink([false, false]).show(&mut content_ui, |ui| {
                Frame::new().inner_margin(Margin { left: 28, right: 28, top: 22, bottom: 28 }).show(ui, |ui| {
                    ui.set_max_width(ui.available_width().min(CONTENT_MAX));
                    let Some(page) = draft.pages.iter().find(|p| p.id == draft.page) else { return };
                    ui.label(RichText::new(&page.name).size(20.0).strong().color(theme.text));
                    if let Some(d) = &page.description {
                        ui.add_space(2.0);
                        ui.label(RichText::new(d).size(12.5).color(theme.text_muted));
                    }
                    ui.add_space(10.0);
                    if draft.page == INTENTS {
                        intents_page(ui, &theme, &draft.intents, draft.intents_page.as_deref());
                        return;
                    }
                    if draft.page == databases::PAGE {
                        if let Some(databases::Ask::Test(i)) = draft.databases.ui(ui, ed, &theme) {
                            test = Some(i);
                        }
                        return;
                    }
                    let root = ed.project.as_ref().map(|p| p.root.clone());
                    let mut heading: Option<String> = None;
                    let shown = draft.page.clone();
                    for f in draft.fields.iter_mut().filter(|f| f.page == shown) {
                        if f.group.is_some() && f.group != heading {
                            group(ui, &theme, f.group.as_deref().unwrap_or_default());
                        }
                        heading = f.group.clone();
                        setting_ui(ui, ed, &theme, f, root.as_deref());
                    }
                });
            });
        });

        if let Some(i) = test {
            draft.databases.start_test(i, ed, super::app_data_dir().join("packages"), ui.ctx().clone());
        }
        if save && problems == 0 {
            self.save_settings(ed, close);
        } else if cancel {
            self.settings = None;
        }
    }

    fn save_settings(&mut self, ed: &mut Editor, close: bool) {
        let Some(draft) = &mut self.settings else { return };
        let mut token_changed = false;
        if let Some(project) = ed.project.as_mut() {
            let mut errors = Vec::new();
            for f in &draft.fields {
                if f.kind == Kind::Secret {
                    let result = if f.remove && f.value.is_empty() {
                        secrets::set(&project.root, &f.key, "")
                    } else if !f.value.is_empty() {
                        secrets::set(&project.root, &f.key, f.value.trim())
                    } else {
                        Ok(())
                    };
                    match result {
                        Err(e) => errors.push(format!("{}: {e:#}", f.label)),
                        Ok(()) if f.key == "token" && (f.remove || !f.value.is_empty()) => token_changed = true,
                        Ok(()) => {}
                    }
                    continue;
                }
                let old = project.meta.settings.get(&f.key);
                match f.project_value() {
                    // An unchanged default stays out of the project.
                    Some(v) if old.is_none() && f.default.as_ref() == Some(&v) => {}
                    Some(v) if old != Some(&v) => {
                        project.meta.settings.insert(f.key.clone(), v);
                        ed.edited = true;
                    }
                    None if old.is_some() => {
                        project.meta.settings.remove(&f.key);
                        ed.edited = true;
                    }
                    _ => {}
                }
            }
            let (changed, db_errors) = draft.databases.save(&mut project.meta.settings, &project.root);
            ed.edited |= changed;
            for e in errors.into_iter().chain(db_errors) {
                ed.log(Level::Error, e);
            }
        }
        // Blocks list the databases (and other settings) in their fields.
        ed.refresh_define_ctx();
        ed.log(Level::Success, t!("Settings saved"));
        // The previews show the bot of the token saved now.
        if token_changed {
            ed.fetch_bot_profile();
        }
        if close {
            self.settings = None;
        } else {
            // Stay open, showing what is stored now (a typed token becomes "Saved").
            self.open_settings(ed);
        }
    }
}

/// The page of privileged intents: which ones the blocks need, and where to turn them on.
fn intents_page(ui: &mut Ui, theme: &Theme, intents: &[(String, Vec<String>)], portal: Option<&str>) {
    let text = if intents.is_empty() {
        t!("None of your blocks needs one at the moment. Blocks that do, such as reading message text, are listed here once you use them.")
    } else {
        t!("Your blocks need these. Turn them on for your bot in the developer portal, under Bot › Privileged Gateway Intents, or it can't log in.")
    };
    ui.label(RichText::new(text).size(12.5).color(theme.text_muted));
    ui.add_space(8.0);
    for (intent, blocks) in intents {
        card(ui, theme, |ui| {
            ui.label(RichText::new(intent).size(13.5).color(theme.text));
            ui.label(RichText::new(tf!("for {blocks}", blocks = blocks.join(", "))).size(11.5).color(theme.text_muted));
        });
    }
    if let Some(page) = portal {
        ui.add_space(4.0);
        if dialog::button(ui, theme, t!("Open the developer portal")).on_hover_text(page).clicked() {
            let _ = open::that(page);
        }
    }
}

/// A setting's box: its name, description and control stacked.
fn card(ui: &mut Ui, theme: &Theme, add: impl FnOnce(&mut Ui)) {
    Frame::new().fill(theme.panel_header).stroke(Stroke::new(1.0, theme.line)).corner_radius(8).inner_margin(Margin::symmetric(16, 12)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = 3.0;
        add(ui);
    });
    ui.add_space(8.0);
}

/// One setting, drawn by its kind: texts and secrets in a box with the
/// field under the name, switches, choices and numbers in a row.
fn setting_ui(ui: &mut Ui, ed: &Editor, theme: &Theme, f: &mut SettingField, root: Option<&std::path::Path>) {
    let label = if f.required { format!("{} *", f.label) } else { f.label.clone() };
    let description = f.description.clone().unwrap_or_default();
    match f.kind.clone() {
        Kind::Boolean => row(ui, theme, &label, &description, |ui| {
            crate::ui::widgets::toggle(ui, &mut f.on, theme);
        }),
        Kind::Select(choices) => {
            let shown = choices.iter().find(|(v, _)| *v == f.value).map(|(_, l)| l.clone()).unwrap_or_else(|| if f.value.is_empty() { f.placeholder.clone() } else { f.value.clone() });
            row(ui, theme, &label, &description, |ui| {
                crate::ui::widgets::dropdown(ui, Id::new(("bot-setting", f.key.as_str())), 200.0, shown, |ui| {
                    for (value, name) in &choices {
                        if ui.selectable_label(f.value == *value, name).clicked() {
                            f.value = value.clone();
                        }
                    }
                });
            });
        }
        Kind::Number { .. } => {
            row(ui, theme, &label, &description, |ui| {
                ui.allocate_ui(vec2(120.0, 34.0), |ui| dialog::text_input(ui, &mut f.value, &f.placeholder, false));
            });
            if let Some(p) = f.problem() {
                ui.add_space(-4.0);
                ui.label(RichText::new(p).size(11.5).color(theme.error));
                ui.add_space(6.0);
            }
        }
        Kind::Text | Kind::Secret => card(ui, theme, |ui| {
            ui.label(RichText::new(&label).size(13.5).color(theme.text));
            if !description.is_empty() {
                ui.label(RichText::new(&description).size(11.5).color(theme.text_muted));
            }
            ui.add_space(5.0);
            let width = ui.available_width().min(460.0);
            if !f.masked {
                ui.allocate_ui(vec2(width, 34.0), |ui| dialog::text_input(ui, &mut f.value, &f.placeholder, false));
            } else {
                secret_input(ui, ed, theme, f, root, width);
            }
            if let Some(p) = f.problem() {
                ui.add_space(2.0);
                ui.label(RichText::new(p).size(11.5).color(theme.error));
            }
            if f.kind != Kind::Secret {
                return;
            }
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if f.remove {
                    dialog::pill(ui, t!("Will be removed"), theme.error);
                } else if f.saved {
                    dialog::pill(ui, t!("Saved"), theme.action);
                } else {
                    dialog::pill(ui, t!("Not set"), if f.required { theme.warning } else { theme.text_muted });
                }
                if f.saved && dialog::button(ui, theme, if f.remove { t!("Keep it") } else { t!("Remove saved value") }).clicked() {
                    f.remove = !f.remove;
                }
            });
            let store = if cfg!(windows) { t!("Windows Credential Manager") } else { t!("the system keyring") };
            ui.label(RichText::new(tf!("Kept in {store}, never in project files.", store = store)).size(11.5).color(theme.text_muted));
        }),
    }
}

/// A hidden text with buttons to show it and to copy it (or the saved one) without showing it.
fn secret_input(ui: &mut Ui, ed: &Editor, theme: &Theme, f: &mut SettingField, root: Option<&std::path::Path>, width: f32) {
    let icon = |name: &str| ed.reg.icons.get(name).map(|p| p.as_path());
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let hint = if f.saved && !f.remove { t!("•••••••• saved · type to replace") } else { f.placeholder.as_str() };
        ui.allocate_ui(vec2(width - 80.0, 34.0), |ui| {
            dialog::text_input(ui, &mut f.value, hint, !f.reveal);
        });
        let (eye, tip) = if f.reveal { (icon("eye-off"), t!("Hide")) } else { (icon("eye"), t!("Show what you typed")) };
        if dialog::icon_button(ui, theme, eye, if f.reveal { t!("Hide") } else { "Show" }, tip).clicked() {
            f.reveal = !f.reveal;
        }
        let saved_value = || if f.kind == Kind::Secret && f.saved { root.and_then(|r| secrets::get(r, &f.key)) } else { None };
        let can_copy = !f.value.is_empty() || (f.saved && !f.remove);
        let copied = f.copied.is_some_and(|t| t.elapsed() < COPIED_FOR);
        let (copy, tip) = if copied { (icon("check"), t!("Copied")) } else { (icon("copy"), t!("Copy without showing it")) };
        if ui.add_enabled_ui(can_copy, |ui| dialog::icon_button(ui, theme, copy, "Copy", tip)).inner.clicked() {
            let value = if f.value.is_empty() { saved_value() } else { Some(f.value.trim().to_string()) };
            if let Some(v) = value {
                ui.ctx().copy_text(v);
                f.copied = Some(Instant::now());
                ui.ctx().request_repaint_after(COPIED_FOR);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn patterns_match_the_whole_value() {
        let re = compile_pattern(r"\d{3}").unwrap();
        assert_eq!(pattern_problem(Some(&re), None, "123"), None);
        assert_eq!(pattern_problem(Some(&re), None, " 123 "), None, "surrounding spaces are trimmed, as on save");
        assert!(pattern_problem(Some(&re), None, "1234").is_some(), "anchored at both ends");
        assert_eq!(pattern_problem(Some(&re), Some("Three digits"), "12").as_deref(), Some("Three digits"));
        assert_eq!(pattern_problem(Some(&re), None, ""), None, "empty is left to required");
        assert!(compile_pattern("(").is_err());
    }

    #[test]
    fn numbers_are_checked() {
        assert_eq!(number_problem("", Some(1.0), None), None);
        assert_eq!(number_problem("12", Some(1.0), Some(100.0)), None);
        assert!(number_problem("twelve", None, None).is_some());
        assert!(number_problem("0", Some(1.0), None).is_some());
        assert!(number_problem("101", None, Some(100.0)).is_some());
    }

    /// Bot settings names the privileged intents the blocks need, and the
    /// developer portal page of the bot (its id is the token's first part).
    #[test]
    fn lists_privileged_intents_and_the_bot_page() {
        let fake_token = ["MTIzNDU2Nzg5MDEyMzQ1Njc4OQ", "GhIjKl", "abcdefghijklmnopqrstuvwxyz0123456789AB"].join(".");
        assert_eq!(app_id_from_token(&fake_token).as_deref(), Some("1234567890123456789"));
        assert_eq!(app_id_from_token("not a token"), None);

        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let ed = Editor::new(vec![root.join("plugins")], None).unwrap();
        let mode = ed.reg.modes.iter().find(|m| m.id == "discord-bot").unwrap();
        let dir = std::env::temp_dir().join(format!("xc-intents-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut project = circuit_core::Project::create(&dir, "Intents", "discord-bot").unwrap();
        let ws = project.workspaces.values_mut().next().unwrap();
        for (id, block) in [("a", "discord.on_member_join@1"), ("b", "discord.on_message@1"), ("c", "core.log@1")] {
            ws.nodes.insert(id.into(), circuit_core::Node { block: block.into(), ..Default::default() });
        }
        let found = privileged_intents(&project, &ed.reg, mode);
        let names: Vec<&str> = found.iter().map(|(i, _)| i.as_str()).collect();
        assert_eq!(names, ["Server Members Intent", "Message Content Intent"], "{found:?}");
        assert_eq!(found[0].1, ["Member Joins"], "{found:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn discord_settings_check_their_values() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../plugins/xc.discord/modes/discord.toml");
        let mode: circuit_plugins::BuildMode = toml::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let check = |key: &str, value: &str| {
            let spec = &mode.settings[key];
            let re = compile_pattern(spec["pattern"].as_str().unwrap()).unwrap();
            pattern_problem(Some(&re), spec.get("patternHint").and_then(Value::as_str), value)
        };
        // Made-up values in the shape Discord uses.
        // Put together here, so secret scanners don't take it for a real token.
        let fake_token = ["MTIzNDU2Nzg5MDEyMzQ1Njc4OQ", "GhIjKl", "abcdefghijklmnopqrstuvwxyz0123456789AB"].join(".");
        assert_eq!(check("token", &fake_token), None);
        assert!(check("token", "not a token").is_some());
        assert!(check("token", "abc.def").is_some());
        assert_eq!(check("devServer", "123456789012345678"), None);
        assert!(check("devServer", "12345").is_some());
        assert!(check("devServer", "my server").is_some());
        assert_eq!(check("prefix", "!"), None);
        assert!(check("prefix", "a b").is_some());
    }

    /// Each kind reads what the project has (or the default) and saves it
    /// back as the right JSON; an unchanged default isn't written.
    #[test]
    fn kinds_read_and_save_their_values() {
        let setting = |spec: Value| Setting { key: "k".into(), spec, page: "p".into(), group: None, plugin: "t".into() };
        let field = |spec: Value, stored: Option<Value>| SettingField::new(&setting(spec), stored.as_ref(), false).0;

        let f = field(serde_json::json!({ "type": "number", "default": 5, "min": 1 }), None);
        assert_eq!((f.value.as_str(), f.project_value()), ("5", Some(Value::from(5))));
        let mut f = field(serde_json::json!({ "type": "number" }), Some(Value::from(2.5)));
        assert_eq!(f.project_value(), Some(Value::from(2.5)));
        f.value.clear();
        assert_eq!(f.project_value(), None, "an emptied number is left out, so the default applies");

        let f = field(serde_json::json!({ "type": "boolean", "default": true }), None);
        assert!(f.on);
        let f = field(serde_json::json!({ "type": "boolean", "default": true }), Some(Value::Bool(false)));
        assert!(!f.on);

        let f = field(serde_json::json!({ "type": "select", "choices": { "json": "JSON file", "sqlite": "SQLite" }, "default": "json" }), None);
        assert_eq!(f.kind, Kind::Select(vec![("json".into(), "JSON file".into()), ("sqlite".into(), "SQLite".into())]));
        assert_eq!(f.project_value(), Some(Value::from("json")));

        let f = field(serde_json::json!({ "type": "secret", "required": true }), None);
        assert!(f.missing() && f.masked && f.project_value().is_none());
        let f = SettingField::new(&setting(serde_json::json!({ "type": "secret", "required": true })), None, true).0;
        assert!(!f.missing(), "saved already");
    }

    /// The tab draws every page of the Discord mode without growing past
    /// its window, at the size it opens with and in a narrow dock.
    #[test]
    fn draws_every_page() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut ed = Editor::new(vec![root.join("plugins")], None).unwrap();
        let dir = std::env::temp_dir().join(format!("xc-bot-settings-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut project = circuit_core::Project::create(&dir, "Settings", "discord-bot").unwrap();
        // Databases of each kind, for their page.
        let kinds = ["mariadb", "postgres", "sqlite", "mongodb"];
        let list: Vec<Value> = kinds.iter().enumerate().map(|(i, k)| serde_json::json!({ "id": format!("db{i}"), "name": format!("Database {i}"), "kind": k })).collect();
        project.meta.settings.insert("databases".into(), Value::Array(list));
        ed.project = Some(project);
        let mut bot = BotUi::new();
        bot.open_settings(&mut ed);
        assert!(bot.settings.as_ref().unwrap().pages.iter().any(|p| p.id == databases::PAGE));
        let pages: Vec<String> = bot.settings.as_ref().unwrap().pages.iter().map(|p| p.id.clone()).collect();
        assert!(pages.len() >= 5 && pages.last().map(String::as_str) == Some(INTENTS), "{pages:?}");
        let ctx = egui::Context::default();
        crate::ui::fonts::install(&ctx);
        for width in [820.0, 480.0] {
            for page in &pages {
                bot.settings.as_mut().unwrap().page = page.clone();
                for _ in 0..3 {
                    let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(width, 620.0))), ..Default::default() };
                    let mut out = ctx.run_ui(input, |ui| bot.settings_tab(ui, &mut ed));
                    out.textures_delta.clear();
                }
                assert!(bot.settings.is_some(), "still open on {page}");
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
