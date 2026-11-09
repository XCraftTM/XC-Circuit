//! The Stored Data panel: what the bot keeps with Set Stored Value and the
//! like (`data/store.json`, or the table xc_store of a database), and the
//! tables of the bot's databases. All of it can be read, added to, changed
//! and deleted. Databases are reached the way the bot reaches them: through
//! the runtime, with its npm packages (see `circuit_runner::database`).

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::time::{Duration, Instant};

use eframe::egui::{self, Frame, Margin, RichText, ScrollArea, Ui};
use serde_json::{Map, Value, json};

use crate::editor::{Editor, Level};
use crate::ui::inputs::{TextArea, TextLine};
use crate::ui::kit;
use crate::ui::theme::Theme;

/// Rows of a table shown at a time.
const PAGE: u64 = 50;

/// What the panel shows.
#[derive(Clone, Debug, Default, PartialEq)]
enum Show {
    /// The stored values, wherever they're kept.
    #[default]
    Values,
    /// A database's tables (its id).
    Tables(String),
}

/// A form open in the panel.
enum Form {
    /// A stored value: a new one (`new`), or one being changed (`was` its value).
    Value { bucket: String, name: String, text: String, new: bool, was: Value },
    /// A row of the table shown: added (no `key`), or changed (its key, and
    /// each column's value before).
    Row { key: Option<Map<String, Value>>, fields: Vec<(String, String, Value)> },
}

/// A page of a table's rows.
struct Page {
    table: String,
    columns: Vec<String>,
    /// The columns that tell rows apart; none: the rows can't be changed here.
    key: Vec<String>,
    rows: Vec<Map<String, Value>>,
    total: u64,
    offset: u64,
}

/// What a database's answer is for.
enum Then {
    Values,
    Tables,
    Rows,
    /// Something changed: show it again.
    Changed,
}

struct Job {
    rx: Receiver<Result<Value, String>>,
    then: Then,
}

/// A change to the stored values.
enum Change {
    Set(String, String, Value),
    Delete(String, Vec<String>),
}

/// A database kept connected, with what it was started for (its settings
/// and secrets, hashed): when those change, it starts again.
type Helper = std::sync::Arc<std::sync::Mutex<Option<(u64, circuit_runner::database::DbHelper)>>>;

#[derive(Default)]
pub struct StoredView {
    /// One connected helper per database (by id): Stored Data's requests go
    /// to it, without starting Node.js and connecting each time.
    helpers: std::collections::HashMap<String, Helper>,
    show: Show,
    filter: String,
    /// The stored values by bucket, and when they were read.
    data: Map<String, Value>,
    loaded: Option<Instant>,
    /// Where `data` came from: store.json (`Some(None)`) or a database.
    data_from: Option<Option<String>>,
    tables: Vec<String>,
    /// The database `tables` are of.
    tables_for: Option<String>,
    page: Option<Page>,
    job: Option<Job>,
    error: Option<String>,
    form: Option<Form>,
    /// A cell being changed in the grid: its row, column, text and value before.
    cell_edit: Option<(usize, String, String, Value)>,
    /// A row whose Delete was clicked once: a second click deletes it.
    confirm: Option<usize>,
    /// Why store.json couldn't be read: nothing is changed then, as saving
    /// would put what little is shown in place of the whole file.
    unreadable: Option<String>,
}

/// store.json's values by bucket: none when the file isn't there yet (or is
/// empty), and why when it is there and can't be read.
fn read_store(file: &std::path::Path) -> Result<Map<String, Value>, String> {
    match std::fs::read_to_string(file) {
        Ok(text) if text.trim().is_empty() => Ok(Map::new()),
        Ok(text) => serde_json::from_str(&text).map_err(|e| e.to_string()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Map::new()),
        Err(e) => Err(e.to_string()),
    }
}

fn file(ed: &Editor) -> Option<PathBuf> {
    ed.project.as_ref().map(|p| p.root.join(".circuit").join("data").join("store.json"))
}

/// The bot's database connections: (id, name).
fn databases(ed: &Editor) -> Vec<(String, String)> {
    let Some(p) = &ed.project else { return Vec::new() };
    let list = p.meta.settings.get("databases").and_then(Value::as_array).cloned().unwrap_or_default();
    list.iter()
        .filter_map(|d| {
            let id = d.get("id").and_then(Value::as_str)?.to_string();
            let name = d.get("name").and_then(Value::as_str).filter(|n| !n.is_empty()).unwrap_or(&id).to_string();
            Some((id, name))
        })
        .collect()
}

/// The connection with `id`, as in the project's settings.
fn spec(ed: &Editor, id: &str) -> Option<Value> {
    let list = ed.project.as_ref()?.meta.settings.get("databases")?.as_array()?;
    list.iter().find(|d| d.get("id").and_then(Value::as_str) == Some(id)).cloned()
}

/// The database the stored values are kept in, when it isn't store.json.
fn store_db(ed: &Editor) -> Option<String> {
    let id = ed.project.as_ref()?.meta.settings.get("storeIn").and_then(Value::as_str).filter(|s| *s != "json")?;
    spec(ed, id).map(|_| id.to_string())
}

fn name_of(ed: &Editor, id: &str) -> String {
    databases(ed).into_iter().find(|(i, _)| i == id).map(|(_, n)| n).unwrap_or_else(|| id.to_string())
}

/// A value as it's edited: text as it is, anything else as JSON.
fn as_text(v: &Value, pretty: bool) -> String {
    match v {
        Value::String(s) => s.clone(),
        other if pretty => serde_json::to_string_pretty(other).unwrap_or_default(),
        other => other.to_string(),
    }
}

/// What was typed, back as a value: text stays text when it was text before;
/// otherwise JSON (a number, true, [1, 2], {"a": 1}), or text when it isn't.
fn from_text(text: &str, was: &Value) -> Value {
    if was.is_string() {
        return Value::String(text.to_string());
    }
    serde_json::from_str(text.trim()).unwrap_or_else(|_| Value::String(text.to_string()))
}

/// Stored values from a database, by bucket: their JSON read back.
fn buckets(values: &Value) -> Map<String, Value> {
    let mut out = Map::new();
    for v in values.as_array().into_iter().flatten() {
        let (Some(bucket), Some(name)) = (v.get("bucket").and_then(Value::as_str), v.get("name").and_then(Value::as_str)) else { continue };
        let text = v.get("value").and_then(Value::as_str).unwrap_or("null");
        let value = serde_json::from_str(text).unwrap_or_else(|_| Value::String(text.to_string()));
        if let Value::Object(m) = out.entry(bucket.to_string()).or_insert_with(|| Value::Object(Map::new())) {
            m.insert(name.to_string(), value);
        }
    }
    out
}

impl StoredView {
    fn read_json(&mut self, ed: &Editor) {
        match file(ed).map(|f| read_store(&f)) {
            Some(Ok(data)) => (self.data, self.unreadable) = (data, None),
            // Shown as it was last read; nothing is saved over it (see `unreadable`).
            Some(Err(why)) => self.unreadable = Some(why),
            None => (self.data, self.unreadable) = (Map::new(), None),
        }
        self.loaded = Some(Instant::now());
    }

    fn write_json(&self, ed: &mut Editor) {
        let Some(f) = file(ed) else { return };
        let _ = std::fs::create_dir_all(f.parent().unwrap_or(&f));
        // Through a temporary file, like the bot itself, so a crash can't leave half a file.
        let tmp = f.with_extension("json.tmp");
        let text = serde_json::to_string_pretty(&Value::Object(self.data.clone())).unwrap_or_default();
        match std::fs::write(&tmp, text).and_then(|_| std::fs::rename(&tmp, &f)) {
            Ok(()) => ed.log(Level::Success, t!("Stored data saved")),
            Err(e) => ed.log(Level::Error, tf!("Could not save stored data: {error}", error = e)),
        }
    }

    /// Sends `request` to database `db` in the background; the answer is
    /// picked up by [`Self::poll`].
    fn ask(&mut self, ed: &Editor, db: &str, request: Value, then: Then, ctx: &egui::Context) {
        let Some(project) = &ed.project else { return };
        let Some(spec) = spec(ed, db) else { return };
        let mut secrets = Vec::new();
        for part in ["password", "url"] {
            let key = crate::bot::databases::secret_key(db, part);
            if let Some(v) = circuit_runner::secrets::get(&project.root, &key) {
                secrets.push((format!("XC_SECRET_{}", key.to_uppercase()), v));
            }
        }
        // Every package the bot needs: installing only the driver would remove the others.
        let deps = match circuit_runner::build_bundle(project, &ed.reg, ed.mode()) {
            Ok(b) => b.dependencies,
            Err(_) => circuit_runner::bundle::database_dependencies(&project.meta.settings),
        };
        let packages = crate::bot::app_data_dir().join("packages");
        let run = project.root.join(".circuit").join("run");
        let data = project.root.join(".circuit").join("data");
        // What the helper was started for: other settings or secrets start another.
        let signature = {
            use std::hash::{Hash, Hasher};
            let mut h = std::collections::hash_map::DefaultHasher::new();
            spec.to_string().hash(&mut h);
            secrets.hash(&mut h);
            h.finish()
        };
        let slot = self.helpers.entry(db.to_string()).or_default().clone();
        let (tx, rx) = channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let cfg = circuit_runner::database::TestConfig { spec: &spec, secrets, dependencies: &deps, packages_dir: &packages, run_dir: &run, data_dir: &data };
            let answer = (|| -> Result<Value, String> {
                let mut held = slot.lock().map_err(|_| t!("The database gave no answer.").to_string())?;
                // One that stopped (ten minutes without a request, say) starts again, once.
                for attempt in 0..2 {
                    let usable = held.as_mut().is_some_and(|(sig, h)| *sig == signature && h.alive());
                    if !usable {
                        *held = None;
                        let helper = circuit_runner::database::DbHelper::start(&cfg, &mut |_| {}).map_err(|e| format!("{e:#}"))?;
                        *held = Some((signature, helper));
                    }
                    let Some((_, helper)) = held.as_mut() else { break };
                    match helper.ask(&request) {
                        Ok(answer) => return answer,
                        Err(e) if attempt == 1 => return Err(format!("{e:#}")),
                        Err(_) => *held = None,
                    }
                }
                Err(t!("The database gave no answer.").to_string())
            })();
            let _ = tx.send(answer);
            ctx.request_repaint();
        });
        self.job = Some(Job { rx, then });
        self.error = None;
    }

    /// Picks up a database's answer.
    fn poll(&mut self, ed: &mut Editor, ctx: &egui::Context) {
        let Some(job) = &self.job else { return };
        let answer = match job.rx.try_recv() {
            Ok(a) => a,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => Err(t!("The database gave no answer.").to_string()),
        };
        let Some(job) = self.job.take() else { return };
        let a = match answer {
            Ok(a) => a,
            Err(e) => {
                self.error = Some(e);
                return;
            }
        };
        match job.then {
            Then::Values => {
                self.data = buckets(&a["values"]);
                self.loaded = Some(Instant::now());
            }
            Then::Tables => {
                self.tables = a["tables"].as_array().into_iter().flatten().filter_map(|t| t.as_str().map(String::from)).collect();
                // The first table, unless one is shown already.
                let first = self.tables.first().cloned();
                if let (Show::Tables(db), Some(table)) = (self.show.clone(), first)
                    && self.page.as_ref().is_none_or(|p| !self.tables.contains(&p.table))
                {
                    self.ask(ed, &db, json!({ "op": "rows", "table": table, "offset": 0, "limit": PAGE }), Then::Rows, ctx);
                }
            }
            Then::Rows => {
                let strings = |v: &Value| v.as_array().into_iter().flatten().filter_map(|c| c.as_str().map(String::from)).collect::<Vec<_>>();
                let rows = a["rows"].as_array().into_iter().flatten().filter_map(|r| r.as_object().cloned()).collect();
                let offset = a.get("offset").and_then(Value::as_u64).or(self.page.as_ref().map(|p| p.offset)).unwrap_or(0);
                let table = a.get("table").and_then(Value::as_str).unwrap_or_default().to_string();
                self.page = Some(Page { table, columns: strings(&a["columns"]), key: strings(&a["key"]), rows, total: a["total"].as_u64().unwrap_or(0), offset });
                self.cell_edit = None;
                self.confirm = None;
            }
            Then::Changed => {
                ed.log(Level::Success, t!("Saved in the database"));
                self.refresh(ed, ctx);
            }
        }
    }

    /// Reads again what's shown.
    fn refresh(&mut self, ed: &Editor, ctx: &egui::Context) {
        match self.show.clone() {
            Show::Values => match store_db(ed) {
                None => self.read_json(ed),
                Some(db) => self.ask(ed, &db, json!({ "op": "store" }), Then::Values, ctx),
            },
            Show::Tables(db) => match &self.page {
                Some(p) => {
                    let request = json!({ "op": "rows", "table": p.table, "offset": p.offset, "limit": PAGE });
                    self.ask(ed, &db, request, Then::Rows, ctx);
                }
                None => self.ask(ed, &db, json!({ "op": "tables" }), Then::Tables, ctx),
            },
        }
    }

    fn rows(&mut self, ed: &Editor, table: &str, offset: u64, ctx: &egui::Context) {
        if let Show::Tables(db) = self.show.clone() {
            self.ask(ed, &db, json!({ "op": "rows", "table": table, "offset": offset, "limit": PAGE }), Then::Rows, ctx);
        }
    }

    /// Changes the stored values: in store.json right away, or in the database.
    fn change(&mut self, ed: &mut Editor, change: Change, ctx: &egui::Context) {
        match store_db(ed) {
            None if self.unreadable.is_some() => {}
            None => {
                match change {
                    Change::Set(bucket, name, value) => {
                        if let Value::Object(m) = self.data.entry(bucket).or_insert_with(|| Value::Object(Map::new())) {
                            m.insert(name, value);
                        }
                    }
                    Change::Delete(bucket, names) => {
                        if let Some(Value::Object(m)) = self.data.get_mut(&bucket) {
                            for n in &names {
                                m.shift_remove(n);
                            }
                        }
                    }
                }
                self.data.retain(|_, v| v.as_object().is_none_or(|m| !m.is_empty()));
                self.write_json(ed);
            }
            Some(db) => {
                let request = match change {
                    Change::Set(bucket, name, value) => json!({ "op": "store-set", "bucket": bucket, "name": name, "value": value }),
                    Change::Delete(bucket, names) => json!({ "op": "store-delete", "bucket": bucket, "names": names }),
                };
                self.ask(ed, &db, request, Then::Changed, ctx);
            }
        }
    }
}

pub fn panel(ui: &mut Ui, ed: &mut Editor, view: &mut StoredView, bot_running: bool) {
    let theme = ed.theme.clone();
    let ctx = ui.ctx().clone();
    view.poll(ed, &ctx);
    let dbs = databases(ed);
    // A database that's gone: back to the stored values.
    if let Show::Tables(id) = &view.show
        && !dbs.iter().any(|(i, _)| i == id)
    {
        view.show = Show::Values;
        view.page = None;
    }
    let store = store_db(ed);
    // The stored values moved (to a database, or back to store.json): read them there.
    if view.show == Show::Values && view.data_from.as_ref() != Some(&store) {
        view.data.clear();
        view.loaded = None;
        view.data_from = Some(store.clone());
        view.form = None;
        view.error = None;
    }
    let busy = view.job.is_some();
    match (&view.show, &store) {
        // While the bot runs it owns store.json, so the panel only shows it and refreshes.
        (Show::Values, None) => {
            let stale = view.loaded.is_none_or(|t| t.elapsed() > Duration::from_secs(if bot_running { 2 } else { 30 }));
            if stale && view.form.is_none() {
                view.read_json(ed);
            }
            if bot_running {
                ctx.request_repaint_after(Duration::from_secs(2));
            }
        }
        (Show::Values, Some(_)) if view.loaded.is_none() && !busy && view.error.is_none() => view.refresh(ed, &ctx),
        (Show::Tables(db), _) if view.tables_for.as_ref() != Some(db) && !busy => {
            view.tables_for = Some(db.clone());
            view.tables.clear();
            view.page = None;
            view.refresh(ed, &ctx);
        }
        _ => {}
    }
    // Stored values are read when the bot starts: changed while it runs, they'd be lost.
    let unreadable = if view.show == Show::Values && store.is_none() { view.unreadable.clone() } else { None };
    let values_locked = bot_running || unreadable.is_some();

    Frame::new().inner_margin(Margin::symmetric(12, 8)).show(ui, |ui| {
        if !dbs.is_empty() {
            let label = match &view.show {
                Show::Values => t!("Stored values").to_string(),
                Show::Tables(id) => tf!("{name} › tables", name = name_of(ed, id)),
            };
            let mut pick = None;
            kit::dropdown(ui, egui::Id::unique("stored-show"), ui.available_width().min(320.0), label, |ui| {
                if kit::option(ui, view.show == Show::Values, t!("Stored values")).clicked() {
                    pick = Some(Show::Values);
                }
                for (id, name) in &dbs {
                    if kit::option(ui, view.show == Show::Tables(id.clone()), tf!("{name} › tables", name = name)).clicked() {
                        pick = Some(Show::Tables(id.clone()));
                    }
                }
            })
            .accessible_name(t!("Show"));
            if let Some(p) = pick
                && p != view.show
            {
                view.show = p;
                view.form = None;
                view.error = None;
                view.tables_for = None;
            }
            ui.add_space(6.0);
        }
        match view.show.clone() {
            Show::Values => {
                let mut refresh = false;
                let mut add = false;
                kit::field_with_button(
                    ui,
                    |ui| {
                        ui.add(TextLine::new(&mut view.filter).hint(t!("Filter keys")));
                    },
                    |ui| {
                        refresh = ui.add_enabled_ui(!busy, |ui| kit::button(ui, &theme, t!("Refresh"))).inner.clicked();
                        add = ui.add_enabled_ui(!busy && !values_locked, |ui| kit::button(ui, &theme, t!("Add"))).inner.on_hover_text(t!("Add a stored value")).clicked();
                    },
                );
                if refresh {
                    view.refresh(ed, &ctx);
                }
                if add {
                    view.form = Some(Form::Value { bucket: "global".into(), name: String::new(), text: String::new(), new: true, was: Value::Null });
                }
                let place = match &store {
                    None => t!("Kept in the bot's data folder, in store.json.").to_string(),
                    Some(db) => tf!("Kept in the database {name}, in the table xc_store.", name = name_of(ed, db)),
                };
                let state = if bot_running { t!("Bot running: read-only.") } else { t!("Changes apply on the next run.") };
                kit::hint(ui, &theme, format!("{place} {state}"));
                if let Some(why) = &unreadable {
                    ui.add(egui::Label::new(RichText::new(tf!("store.json can't be read right now, so nothing can be changed here (saving would replace the whole file): {why}", why = why)).size(12.0).color(theme.error)).wrap());
                }
            }
            Show::Tables(_) => {
                let mut pick = None;
                let mut refresh = false;
                let shown = view.page.as_ref().map(|p| p.table.clone()).unwrap_or_else(|| t!("Tables").to_string());
                kit::field_with_button(
                    ui,
                    |ui| {
                        kit::dropdown(ui, egui::Id::unique("stored-table"), ui.available_width(), shown, |ui| {
                            for t in &view.tables {
                                if kit::option(ui, view.page.as_ref().is_some_and(|p| &p.table == t), t).clicked() {
                                    pick = Some(t.clone());
                                }
                            }
                        })
                        .accessible_name(t!("Tables"));
                    },
                    |ui| refresh = ui.add_enabled_ui(!busy, |ui| kit::button(ui, &theme, t!("Refresh"))).inner.clicked(),
                );
                if let Some(t) = pick {
                    view.form = None;
                    view.rows(ed, &t, 0, &ctx);
                }
                if refresh {
                    view.refresh(ed, &ctx);
                }
                if let Some(p) = &view.page {
                    let (offset, total, table) = (p.offset, p.total, p.table.clone());
                    let mut go = None;
                    let mut add = false;
                    ui.horizontal_wrapped(|ui| {
                        let to = (offset + p.rows.len() as u64).min(total);
                        let count = if total == 0 { t!("No rows").to_string() } else { tf!("Rows {from}–{to} of {total}", from = offset + 1, to = to, total = total) };
                        ui.label(RichText::new(count).size(12.0).color(theme.text_muted));
                        ui.add_enabled_ui(!busy && offset > 0, |ui| {
                            if kit::button(ui, &theme, "‹").on_hover_text(t!("Previous rows")).clicked() {
                                go = Some(offset.saturating_sub(PAGE));
                            }
                        });
                        ui.add_enabled_ui(!busy && to < total, |ui| {
                            if kit::button(ui, &theme, "›").on_hover_text(t!("Next rows")).clicked() {
                                go = Some(offset + PAGE);
                            }
                        });
                        add = ui.add_enabled_ui(!busy, |ui| kit::button(ui, &theme, t!("Add row"))).inner.clicked();
                    });
                    if p.key.is_empty() && !p.columns.is_empty() {
                        kit::hint(ui, &theme, t!("This table has no primary key: its rows can be read here, not changed."));
                    }
                    let fields: Vec<(String, String, Value)> = p.columns.iter().filter(|c| c.as_str() != "rowid").map(|c| (c.clone(), String::new(), Value::Null)).collect();
                    if add {
                        view.form = Some(Form::Row { key: None, fields });
                    }
                    if let Some(offset) = go {
                        view.rows(ed, &table, offset, &ctx);
                    }
                }
            }
        }
        if busy {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(RichText::new(t!("Asking the database…")).size(12.0).color(theme.text_muted));
            });
        }
        if let Some(e) = &view.error {
            ui.add(egui::Label::new(RichText::new(e).size(12.0).color(theme.error)).wrap());
        }
    });

    match view.show {
        Show::Values => {
            ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                Frame::new().inner_margin(Margin { left: 12, right: 12, top: 0, bottom: 12 }).show(ui, |ui| {
                    form_ui(ui, &theme, ed, view, &ctx);
                    values_ui(ui, &theme, ed, view, values_locked || busy, &ctx);
                });
            });
        }
        // The grid scrolls on its own, both ways; a form goes above it.
        Show::Tables(_) => {
            Frame::new().inner_margin(Margin { left: 12, right: 12, top: 0, bottom: 8 }).show(ui, |ui| {
                if view.form.is_some() {
                    let most = ui.available_height() * 0.6;
                    ScrollArea::vertical().id_salt("stored-row-form").max_height(most).show(ui, |ui| form_ui(ui, &theme, ed, view, &ctx));
                }
                rows_ui(ui, &theme, ed, view, busy, &ctx);
            });
        }
    }
}

/// The form for a value or a row, when one is open.
fn form_ui(ui: &mut Ui, theme: &Theme, ed: &mut Editor, view: &mut StoredView, ctx: &egui::Context) {
    let Some(form) = &mut view.form else { return };
    let mut done: Option<bool> = None;
    Frame::new().fill(theme.panel_header).corner_radius(8).inner_margin(Margin::same(10)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        match form {
            Form::Value { bucket, name, text, new, .. } => {
                if *new {
                    kit::field(ui, theme, t!("Bucket"), Some(t!("global, or user:<id>, server:<id> …")), |ui| ui.add(TextLine::new(bucket).hint("global")));
                    kit::field(ui, theme, t!("Name"), None, |ui| ui.add(TextLine::new(name).hint(t!("score"))));
                } else {
                    ui.label(RichText::new(format!("{bucket} › {name}")).monospace().color(theme.text));
                }
                kit::field(ui, theme, t!("Value"), Some(t!("A number, true, [1, 2] or {\"a\": 1} is kept as that; anything else as text.")), |ui| {
                    ui.add(TextArea::new(("stored-value", &*bucket, &*name), text).rows(3))
                });
            }
            Form::Row { key, fields } => {
                ui.label(RichText::new(if key.is_some() { t!("Change row") } else { t!("New row") }).strong().color(theme.text));
                let keys: Vec<String> = view.page.as_ref().map(|p| p.key.clone()).unwrap_or_default();
                for (column, text, _) in fields.iter_mut() {
                    // A row's key stays: it's what the row is found by.
                    let fixed = key.is_some() && keys.contains(column);
                    kit::field(ui, theme, column, None, |ui| ui.add_enabled(!fixed, TextLine::new(text).monospace()));
                }
                if key.is_none() {
                    kit::hint(ui, theme, t!("Leave a column empty for its default."));
                }
            }
        }
        ui.horizontal(|ui| {
            let ok = match form {
                Form::Value { bucket, name, .. } => !bucket.trim().is_empty() && !name.trim().is_empty(),
                Form::Row { .. } => true,
            };
            if ui.add_enabled_ui(ok, |ui| kit::primary_button(ui, theme, t!("Save"))).inner.clicked() {
                done = Some(true);
            }
            if kit::secondary_button(ui, theme, t!("Cancel")).clicked() {
                done = Some(false);
            }
        });
    });
    ui.add_space(8.0);
    match done {
        Some(true) => {}
        Some(false) => {
            view.form = None;
            return;
        }
        None => return,
    }
    match view.form.take() {
        Some(Form::Value { bucket, name, text, was, .. }) => {
            let value = from_text(&text, &was);
            view.change(ed, Change::Set(bucket.trim().to_string(), name.trim().to_string(), value), ctx);
        }
        Some(Form::Row { key, fields }) => {
            let (Show::Tables(db), Some(table)) = (view.show.clone(), view.page.as_ref().map(|p| p.table.clone())) else { return };
            let request = match key {
                // Only what changed; columns left empty get their default.
                Some(key) => {
                    let values: Map<String, Value> = fields.into_iter().filter(|(_, text, was)| *text != as_text(was, false)).map(|(c, text, was)| (c, from_text(&text, &was))).collect();
                    if values.is_empty() {
                        return;
                    }
                    json!({ "op": "row-update", "table": table, "key": key, "values": values })
                }
                None => {
                    let values: Map<String, Value> = fields.into_iter().filter(|(_, text, _)| !text.is_empty()).map(|(c, text, was)| (c, from_text(&text, &was))).collect();
                    json!({ "op": "row-insert", "table": table, "values": values })
                }
            };
            view.ask(ed, &db, request, Then::Changed, ctx);
        }
        None => {}
    }
}

/// The stored values, by bucket: each with Edit and Delete.
fn values_ui(ui: &mut Ui, theme: &Theme, ed: &mut Editor, view: &mut StoredView, locked: bool, ctx: &egui::Context) {
    if view.data.is_empty() && view.loaded.is_some() && view.unreadable.is_none() {
        ui.label(RichText::new(t!("Nothing stored yet. Blocks like Set Stored Value write here when the bot runs.")).color(theme.text_muted));
    }
    let filter = view.filter.to_lowercase();
    let mut change = None;
    let mut edit = None;
    let pencil = ed.reg.icons.get("pencil").cloned();
    let trash = ed.reg.icons.get("trash").cloned();
    for (bucket, entries) in &view.data {
        let Value::Object(entries) = entries else { continue };
        let names: Vec<&String> = entries.keys().filter(|k| filter.is_empty() || k.to_lowercase().contains(&filter) || bucket.to_lowercase().contains(&filter)).collect();
        if names.is_empty() {
            continue;
        }
        let header = egui::CollapsingHeader::new(RichText::new(format!("{bucket}  ·  {}", names.len())).monospace()).default_open(true).id_salt(("stored", bucket)).show(ui, |ui| {
            for name in &names {
                let value = &entries[*name];
                ui.horizontal(|ui| {
                    ui.add(egui::Label::new(RichText::new(*name).monospace().color(theme.text)).truncate());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_enabled_ui(!locked, |ui| {
                            if kit::row_icon(ui, theme, trash.as_deref(), "×", t!("Delete")).clicked() {
                                change = Some(Change::Delete(bucket.clone(), vec![(*name).clone()]));
                            }
                            if kit::row_icon(ui, theme, pencil.as_deref(), "✎", t!("Edit")).clicked() {
                                edit = Some((bucket.clone(), (*name).clone(), value.clone()));
                            }
                        });
                    });
                });
                let shown = ui.add(egui::Label::new(RichText::new(as_text(value, false)).monospace().size(11.5).color(theme.text_muted)).truncate().selectable(false).sense(egui::Sense::click()));
                if shown.double_clicked() && !locked {
                    edit = Some((bucket.clone(), (*name).clone(), value.clone()));
                }
                ui.add_space(4.0);
            }
        });
        header.header_response.context_menu(|ui| {
            let all: Vec<String> = entries.keys().cloned().collect();
            if ui.add_enabled_ui(!locked, |ui| kit::menu_item(ui, tf!("Delete all {count} in {bucket}", count = all.len(), bucket = bucket), None, true)).inner.clicked() {
                change = Some(Change::Delete(bucket.clone(), all));
                ui.close();
            }
        });
    }
    if let Some((bucket, name, value)) = edit {
        view.form = Some(Form::Value { bucket, name, text: as_text(&value, true), new: false, was: value });
    }
    if let Some(c) = change {
        view.change(ed, c, ctx);
    }
}

/// A page of a table's rows as a grid, as database tools show them: a
/// header with the columns (a key on the primary key's), a row each, and
/// Edit and Delete in front of each row. Double-click a cell to change it
/// right there (Enter saves, Escape doesn't).
fn rows_ui(ui: &mut Ui, theme: &Theme, ed: &mut Editor, view: &mut StoredView, busy: bool, ctx: &egui::Context) {
    use egui_extras::{Column, TableBuilder};
    let Some(page) = &view.page else {
        if view.tables.is_empty() && view.tables_for.is_some() && !busy && view.error.is_none() {
            ui.label(RichText::new(t!("This database has no tables yet.")).color(theme.text_muted));
        }
        return;
    };
    let icon = |name: &str| ed.reg.icons.get(name).cloned();
    let (pencil, trash, check, cross, key_icon) = (icon("pencil"), icon("trash"), icon("check"), icon("x"), icon("key"));
    let editable = !page.key.is_empty();
    let cell_edit = &mut view.cell_edit;
    let confirm = &mut view.confirm;
    // What the grid asks for, done once it's drawn.
    let mut edit_row = None;
    let mut delete_row = None;
    let mut save_cell: Option<(usize, String, Value)> = None;
    let mono = egui::FontId::monospace(12.0);
    let char_w = ui.fonts_mut(|f| f.glyph_width(&mono, '0')).max(6.0);
    // Wide enough for the longest value (or the column's name), within reason.
    let widths: Vec<f32> = page
        .columns
        .iter()
        .map(|c| {
            let longest = page.rows.iter().map(|r| as_text(r.get(c).unwrap_or(&Value::Null), false).chars().count()).max().unwrap_or(0).max(c.chars().count() + 2);
            (longest as f32 * char_w + 24.0).clamp(64.0, 300.0)
        })
        .collect();
    let height = ui.available_height();
    ScrollArea::horizontal().id_salt(("stored-grid-x", &page.table)).auto_shrink([false, false]).show(ui, |ui| {
        let mut table = TableBuilder::new(ui)
            .id_salt(("stored-grid", &page.table))
            .striped(true)
            .resizable(true)
            .vscroll(true)
            .min_scrolled_height(0.0)
            .max_scroll_height((height - 34.0).max(80.0))
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center));
        if editable {
            table = table.column(Column::exact(58.0));
        }
        for w in &widths {
            table = table.column(Column::initial(*w).at_least(40.0).clip(true));
        }
        table
            .header(30.0, |mut header| {
                let head = |ui: &mut Ui| {
                    let r = ui.max_rect().expand2(egui::vec2(4.0, 0.0));
                    ui.painter().rect_filled(r, 0.0, theme.panel_header);
                    ui.painter().hline(r.x_range(), r.bottom(), egui::Stroke::new(1.0, theme.line));
                };
                if editable {
                    header.col(|ui| head(ui));
                }
                for c in &page.columns {
                    header.col(|ui| {
                        head(ui);
                        if page.key.contains(c) {
                            match &key_icon {
                                Some(p) => {
                                    let (r, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
                                    egui::Image::from_uri(crate::editor::file_uri(p)).tint(theme.warning).paint_at(ui, r);
                                }
                                None => {
                                    ui.label(RichText::new("🔑").size(10.0));
                                }
                            }
                        }
                        ui.add(egui::Label::new(RichText::new(c).strong().color(theme.text)).truncate()).on_hover_text(if page.key.contains(c) { t!("Primary key: how a row is found") } else { "" });
                    });
                }
            })
            .body(|body| {
                body.rows(28.0, page.rows.len(), |mut row| {
                    let i = row.index();
                    let r = &page.rows[i];
                    if editable {
                        row.col(|ui| {
                            ui.add_enabled_ui(!busy, |ui| {
                                if *confirm == Some(i) {
                                    // Delete only on a second click: there's no undo.
                                    if kit::row_icon(ui, theme, check.as_deref(), "✓", t!("Delete this row")).clicked() {
                                        delete_row = Some(i);
                                    }
                                    if kit::row_icon(ui, theme, cross.as_deref(), "×", t!("Keep it")).clicked() {
                                        *confirm = None;
                                    }
                                } else {
                                    if kit::row_icon(ui, theme, pencil.as_deref(), "✎", t!("Change row")).clicked() {
                                        edit_row = Some(i);
                                    }
                                    if kit::row_icon(ui, theme, trash.as_deref(), "×", t!("Delete row")).clicked() {
                                        *confirm = Some(i);
                                    }
                                }
                            });
                        });
                    }
                    for c in &page.columns {
                        let value = r.get(c).cloned().unwrap_or(Value::Null);
                        row.col(|ui| {
                            let editing = cell_edit.as_ref().is_some_and(|(ri, col, ..)| *ri == i && col == c);
                            if editing {
                                let Some((_, _, text, was)) = cell_edit.as_mut() else { return };
                                let resp = ui.add(TextLine::new(text).monospace().compact().width(ui.available_width())).accessible_name(c.as_str());
                                let (enter, escape) = ui.input(|inp| (inp.key_pressed(egui::Key::Enter), inp.key_pressed(egui::Key::Escape)));
                                if escape {
                                    *cell_edit = None;
                                } else if enter || resp.lost_focus() {
                                    save_cell = Some((i, c.clone(), from_text(text, was)));
                                    *cell_edit = None;
                                } else {
                                    resp.request_focus();
                                }
                                return;
                            }
                            let shown = if value.is_null() {
                                RichText::new("NULL").italics().size(11.5).color(theme.text_muted)
                            } else {
                                RichText::new(as_text(&value, false)).font(mono.clone()).color(theme.text)
                            };
                            // Not selectable: a double-click changes the cell instead of selecting a word.
                            let resp = ui.add(egui::Label::new(shown).truncate().selectable(false).sense(egui::Sense::click()));
                            let can = editable && !busy && !page.key.contains(c);
                            // A quick third click is still a double-click to whoever made it.
                            if can && (resp.double_clicked() || resp.triple_clicked()) {
                                *cell_edit = Some((i, c.clone(), as_text(&value, false), value.clone()));
                            }
                            if can {
                                resp.on_hover_text(t!("Double-click to change"));
                            }
                        });
                    }
                });
            });
    });

    let key_of = |row: &Map<String, Value>| -> Map<String, Value> { page.key.iter().map(|k| (k.clone(), row.get(k).cloned().unwrap_or(Value::Null))).collect() };
    let table = page.table.clone();
    let Show::Tables(db) = view.show.clone() else { return };
    if let Some(i) = edit_row {
        let row = &page.rows[i];
        let fields = page.columns.iter().map(|c| {
            let v = row.get(c).cloned().unwrap_or(Value::Null);
            (c.clone(), as_text(&v, false), v)
        });
        let form = Form::Row { key: Some(key_of(row)), fields: fields.collect() };
        view.form = Some(form);
    } else if let Some(i) = delete_row {
        let request = json!({ "op": "row-delete", "table": table, "key": key_of(&page.rows[i]) });
        view.confirm = None;
        view.ask(ed, &db, request, Then::Changed, ctx);
    } else if let Some((i, column, value)) = save_cell {
        let unchanged = page.rows[i].get(&column).is_some_and(|v| *v == value);
        if !unchanged {
            let request = json!({ "op": "row-update", "table": table, "key": key_of(&page.rows[i]), "values": { column: value } });
            view.ask(ed, &db, request, Then::Changed, ctx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A store.json that is there but can't be read is told apart from one
    /// that isn't there yet: only the second counts as "nothing stored", so
    /// adding a value can't save an almost empty file over the bot's data.
    #[test]
    fn an_unreadable_store_is_not_an_empty_one() {
        let dir = std::env::temp_dir().join(format!("xc-stored-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("store.json");
        assert_eq!(read_store(&file), Ok(Map::new()), "not there yet");
        std::fs::write(&file, "").unwrap();
        assert_eq!(read_store(&file), Ok(Map::new()), "empty");
        std::fs::write(&file, r#"{"global":{"score":3}}"#).unwrap();
        assert_eq!(read_store(&file).unwrap()["global"]["score"], 3);
        std::fs::write(&file, r#"{"global":{"score":3"#).unwrap();
        assert!(read_store(&file).is_err(), "cut off");
        // A folder in its place: there, and not readable.
        std::fs::remove_file(&file).unwrap();
        std::fs::create_dir_all(&file).unwrap();
        assert!(read_store(&file).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
