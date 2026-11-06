//! Bot › Settings › Databases: the connections the database blocks use, the
//! default one, and where Set/Get Stored Value keep their data. Kept in the
//! project's settings as `databases`, `databaseDefault` and `storeIn`;
//! passwords and connection URLs go to the system keyring as
//! `db_<id>_password` and `db_<id>_url`.

use std::sync::mpsc::{Receiver, TryRecvError, channel};

use circuit_runner::database::{TestConfig, TestResult, test_database};
use circuit_runner::secrets;
use eframe::egui::{self, Align, Id, Layout, RichText, Ui, vec2};
use serde_json::{Map, Value, json};

use crate::ui::kit;
use crate::editor::Editor;
use crate::ui::theme::Theme;

/// The page's id among Bot › Settings' pages.
pub const PAGE: &str = "xc:databases";

/// The kinds of database, as the runtime names them, with what the list shows.
pub const KINDS: [(&str, &str); 4] = [("mariadb", "MariaDB / MySQL"), ("postgres", "PostgreSQL"), ("sqlite", "SQLite"), ("mongodb", "MongoDB")];

fn default_port(kind: &str) -> &'static str {
    match kind {
        "mariadb" => "3306",
        "postgres" => "5432",
        "mongodb" => "27017",
        _ => "",
    }
}

/// A password or URL: typed now, or saved already.
#[derive(Default)]
struct Secret {
    value: String,
    saved: bool,
    remove: bool,
    reveal: bool,
}

impl Secret {
    fn load(root: &std::path::Path, key: &str) -> Secret {
        Secret { saved: secrets::get(root, key).is_some(), ..Default::default() }
    }

    /// What the bot would get: what's typed, or else what's saved.
    fn current(&self, root: &std::path::Path, key: &str) -> Option<String> {
        if !self.value.trim().is_empty() {
            Some(self.value.trim().to_string())
        } else if self.saved && !self.remove {
            secrets::get(root, key)
        } else {
            None
        }
    }
}

enum Test {
    Running(Receiver<Result<TestResult, String>>),
    Done(Result<TestResult, String>),
}

/// One connection being edited.
struct Connection {
    id: String,
    name: String,
    kind: String,
    host: String,
    port: String,
    user: String,
    database: String,
    file: String,
    ssl: bool,
    password: Secret,
    url: Secret,
    test: Option<Test>,
}

impl Connection {
    fn from_value(v: &Value, root: &std::path::Path) -> Option<Connection> {
        let text = |k: &str| match v.get(k) {
            Some(Value::String(s)) => s.clone(),
            Some(Value::Number(n)) => n.to_string(),
            _ => String::new(),
        };
        let id = text("id");
        if id.is_empty() {
            return None;
        }
        Some(Connection {
            name: text("name"),
            kind: Some(text("kind")).filter(|k| KINDS.iter().any(|(x, _)| x == k)).unwrap_or_else(|| "mariadb".into()),
            host: text("host"),
            port: text("port"),
            user: text("user"),
            database: text("database"),
            file: text("file"),
            ssl: v.get("ssl").and_then(Value::as_bool).unwrap_or(false),
            password: Secret::load(root, &secret_key(&id, "password")),
            url: Secret::load(root, &secret_key(&id, "url")),
            test: None,
            id,
        })
    }

    /// As the project keeps it: only what this kind uses.
    fn to_value(&self) -> Value {
        let mut m = Map::new();
        m.insert("id".into(), json!(self.id));
        m.insert("name".into(), json!(self.name.trim()));
        m.insert("kind".into(), json!(self.kind));
        let put = |m: &mut Map<String, Value>, k: &str, v: &str| {
            if !v.trim().is_empty() {
                m.insert(k.into(), json!(v.trim()));
            }
        };
        if self.kind == "sqlite" {
            put(&mut m, "file", &self.file);
        } else {
            put(&mut m, "host", &self.host);
            if let Ok(port) = self.port.trim().parse::<u16>() {
                m.insert("port".into(), json!(port));
            }
            put(&mut m, "user", &self.user);
            put(&mut m, "database", &self.database);
            if self.ssl && self.kind != "mongodb" {
                m.insert("ssl".into(), json!(true));
            }
        }
        Value::Object(m)
    }

    fn problem(&self, all: &[Connection]) -> Option<String> {
        let name = self.name.trim();
        if name.is_empty() {
            return Some(t!("Give it a name; blocks list it by that.").into());
        }
        if all.iter().filter(|c| c.name.trim().eq_ignore_ascii_case(name)).count() > 1 {
            return Some(t!("Another database has this name.").into());
        }
        if self.kind != "sqlite" && !self.port.trim().is_empty() && self.port.trim().parse::<u16>().map_or(true, |p| p == 0) {
            return Some(t!("The port is a number from 1 to 65535.").into());
        }
        None
    }
}

/// The keyring entry of a connection's password or URL.
pub fn secret_key(id: &str, part: &str) -> String {
    format!("db_{id}_{part}")
}

/// The project's connections, from its settings.
fn listed(settings: &Map<String, Value>) -> impl Iterator<Item = &Value> {
    settings.get("databases").and_then(Value::as_array).into_iter().flatten()
}

/// The keyring entries of the project's databases, with what to call them in an export's .env.
pub fn secret_keys(settings: &Map<String, Value>) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for d in listed(settings) {
        let Some(id) = d.get("id").and_then(Value::as_str) else { continue };
        let name = d.get("name").and_then(Value::as_str).unwrap_or(id);
        out.push((secret_key(id, "password"), format!("{name}: password")));
        out.push((secret_key(id, "url"), format!("{name}: connection URL")));
    }
    out
}

pub struct DatabasesDraft {
    list: Vec<Connection>,
    /// The default connection's id, or empty.
    default: String,
    /// "json", or a connection's id.
    store_in: String,
    /// Connections removed since opening: their saved secrets go too.
    removed: Vec<String>,
}

/// What the page asks for.
pub enum Ask {
    Test(usize),
}

impl DatabasesDraft {
    pub fn load(settings: &Map<String, Value>, root: &std::path::Path) -> DatabasesDraft {
        let text = |k: &str| settings.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
        DatabasesDraft {
            list: listed(settings).filter_map(|v| Connection::from_value(v, root)).collect(),
            default: text("databaseDefault"),
            store_in: Some(text("storeIn")).filter(|s| !s.is_empty()).unwrap_or_else(|| "json".into()),
            removed: Vec::new(),
        }
    }

    pub fn problems(&self) -> usize {
        self.list.iter().filter(|c| c.problem(&self.list).is_some()).count()
    }

    /// The settings as the project keeps them.
    fn values(&self) -> [(&'static str, Option<Value>); 3] {
        let ids: Vec<&str> = self.list.iter().map(|c| c.id.as_str()).collect();
        [
            ("databases", (!self.list.is_empty()).then(|| Value::Array(self.list.iter().map(Connection::to_value).collect()))),
            ("databaseDefault", Some(self.default.clone()).filter(|d| ids.contains(&d.as_str())).map(Value::String)),
            ("storeIn", Some(self.store_in.clone()).filter(|s| ids.contains(&s.as_str())).map(Value::String)),
        ]
    }

    /// Writes the settings into the project and the secrets into the
    /// keyring. Gives whether the project changed, and errors.
    pub fn save(&mut self, settings: &mut Map<String, Value>, root: &std::path::Path) -> (bool, Vec<String>) {
        let mut changed = false;
        for (key, value) in self.values() {
            match value {
                Some(v) if settings.get(key) != Some(&v) => {
                    settings.insert(key.into(), v);
                    changed = true;
                }
                None if settings.contains_key(key) => {
                    settings.remove(key);
                    changed = true;
                }
                _ => {}
            }
        }
        let mut errors = Vec::new();
        for c in &self.list {
            for (part, s) in [("password", &c.password), ("url", &c.url)] {
                let key = secret_key(&c.id, part);
                let result = if !s.value.trim().is_empty() {
                    secrets::set(root, &key, s.value.trim())
                } else if s.remove {
                    secrets::set(root, &key, "")
                } else {
                    Ok(())
                };
                if let Err(e) = result {
                    errors.push(format!("{}: {e:#}", c.name));
                }
            }
        }
        for id in std::mem::take(&mut self.removed) {
            for part in ["password", "url"] {
                let _ = secrets::set(root, &secret_key(&id, part), "");
            }
        }
        (changed, errors)
    }

    /// Starts Test for connection `i`: connects the way the bot will, on its own thread.
    pub fn start_test(&mut self, i: usize, ed: &Editor, packages_dir: std::path::PathBuf, repaint: egui::Context) {
        let (Some(project), Some(c)) = (&ed.project, self.list.get(i)) else { return };
        let spec = c.to_value();
        let mut secrets_env = Vec::new();
        for part in ["password", "url"] {
            let s = if part == "password" { &c.password } else { &c.url };
            if let Some(v) = s.current(&project.root, &secret_key(&c.id, part)) {
                secrets_env.push((format!("XC_SECRET_{}", secret_key(&c.id, part).to_uppercase()), v));
            }
        }
        // Every package the bot needs with these connections: installing only
        // the driver would remove the others.
        let mut draft = project.clone();
        for (key, value) in self.values() {
            match value {
                Some(v) => draft.meta.settings.insert(key.into(), v),
                None => draft.meta.settings.remove(key),
            };
        }
        let deps = match circuit_runner::build_bundle(&draft, &ed.reg, ed.mode()) {
            Ok(b) => b.dependencies,
            Err(_) => circuit_runner::bundle::database_dependencies(&draft.meta.settings),
        };
        let run_dir = project.root.join(".circuit").join("run");
        let data_dir = project.root.join(".circuit").join("data");
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let cfg = TestConfig { spec: &spec, secrets: secrets_env, dependencies: &deps, packages_dir: &packages_dir, run_dir: &run_dir, data_dir: &data_dir };
            let _ = tx.send(test_database(&cfg, &mut |_| {}).map_err(|e| format!("{e:#}")));
            repaint.request_repaint();
        });
        if let Some(c) = self.list.get_mut(i) {
            c.test = Some(Test::Running(rx));
        }
    }

    /// Picks up finished tests.
    fn poll(&mut self) {
        for c in &mut self.list {
            if let Some(Test::Running(rx)) = &c.test {
                match rx.try_recv() {
                    Ok(r) => c.test = Some(Test::Done(r)),
                    Err(TryRecvError::Disconnected) => c.test = Some(Test::Done(Err(t!("The test stopped without an answer.").into()))),
                    Err(TryRecvError::Empty) => {}
                }
            }
        }
    }

    fn add(&mut self) {
        let id = (1..).map(|i| format!("db{i}")).find(|id| !self.list.iter().any(|c| c.id == *id) && !self.removed.contains(id)).expect("unbounded");
        let name = (1..).map(|i| if i == 1 { t!("Database").to_string() } else { format!("{} {i}", t!("Database")) }).find(|n| !self.list.iter().any(|c| c.name == *n)).expect("unbounded");
        if self.list.is_empty() {
            self.default = id.clone();
        }
        self.list.push(Connection {
            id,
            name,
            kind: "mariadb".into(),
            host: "localhost".into(),
            port: String::new(),
            user: String::new(),
            database: String::new(),
            file: String::new(),
            ssl: false,
            password: Secret::default(),
            url: Secret::default(),
            test: None,
        });
    }

    /// The page. Returns what it asks for (a test).
    pub fn ui(&mut self, ui: &mut Ui, ed: &Editor, theme: &Theme) -> Option<Ask> {
        self.poll();
        let mut ask = None;
        kit::group(ui, theme, t!("Connections"));
        if self.list.is_empty() {
            ui.label(RichText::new(t!("No database yet. Add one, then use it with Get Rows, Insert Row, Update Rows, Delete Rows and SQL Query.")).size(12.5).color(theme.text_muted));
            ui.add_space(8.0);
        }
        let mut remove = None;
        let problems: Vec<Option<String>> = self.list.iter().map(|c| c.problem(&self.list)).collect();
        for (i, c) in self.list.iter_mut().enumerate() {
            let action = connection_ui(ui, ed, theme, c, problems[i].as_deref());
            match action {
                Some(Action::Remove) => remove = Some(i),
                Some(Action::Test) => ask = Some(Ask::Test(i)),
                None => {}
            }
        }
        if let Some(i) = remove {
            let c = self.list.remove(i);
            self.removed.push(c.id.clone());
            if self.default == c.id {
                self.default = self.list.first().map(|c| c.id.clone()).unwrap_or_default();
            }
            if self.store_in == c.id {
                self.store_in = "json".into();
            }
        }
        if kit::button(ui, theme, t!("+ Add database")).clicked() {
            self.add();
        }

        kit::group(ui, theme, t!("Defaults"));
        let choices: Vec<(String, String)> = self.list.iter().map(|c| (c.id.clone(), c.name.clone())).collect();
        let name_of = |id: &str| choices.iter().find(|(x, _)| x == id).map(|(_, n)| n.clone());
        let none = t!("None").to_string();
        let shown = name_of(&self.default).unwrap_or_else(|| none.clone());
        kit::setting_row(ui, theme, t!("Default database"), t!("What the database blocks use when their Database is Default."), |ui| {
            crate::ui::kit::dropdown(ui, Id::new("db-default"), 200.0, shown, |ui| {
                if kit::option(ui, self.default.is_empty(), &none).clicked() {
                    self.default.clear();
                }
                for (id, name) in &choices {
                    if kit::option(ui, self.default == *id, name).clicked() {
                        self.default = id.clone();
                    }
                }
            });
        });
        let json_name = t!("JSON file (store.json)").to_string();
        let shown = name_of(&self.store_in).unwrap_or_else(|| json_name.clone());
        kit::setting_row(
            ui,
            theme,
            t!("Keep stored values in"),
            t!("Where Set Stored Value, Get Stored Value and the other stored-data blocks keep their data (in the table xc_store). The first time a database is used, what's in store.json is copied there; store.json stays as it was."),
            |ui| {
                crate::ui::kit::dropdown(ui, Id::new("db-store-in"), 200.0, shown, |ui| {
                    if kit::option(ui, self.store_in == "json", &json_name).clicked() {
                        self.store_in = "json".into();
                    }
                    for (id, name) in &choices {
                        if kit::option(ui, self.store_in == *id, name).clicked() {
                            self.store_in = id.clone();
                        }
                    }
                });
            },
        );
        ask
    }

    #[cfg(test)]
    fn names(&self) -> Vec<&str> {
        self.list.iter().map(|c| c.name.as_str()).collect()
    }
}

enum Action {
    Remove,
    Test,
}

/// One connection's box.
fn connection_ui(ui: &mut Ui, ed: &Editor, theme: &Theme, c: &mut Connection, problem: Option<&str>) -> Option<Action> {
    let mut action = None;
    let icon = |name: &str| ed.reg.icons.get(name).map(|p| p.as_path());
    egui::Frame::new().fill(theme.panel_header).stroke(egui::Stroke::new(1.0, theme.line)).corner_radius(8).inner_margin(egui::Margin::symmetric(16, 12)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = 6.0;
        let width = ui.available_width();
        // Name, kind and remove.
        crate::ui::kit::line(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            ui.allocate_ui(vec2((width - 260.0).clamp(120.0, 300.0), 34.0), |ui| kit::text_input(ui, &mut c.name, t!("Name"), false));
            let kind = KINDS.iter().find(|(k, _)| *k == c.kind).map(|(_, n)| *n).unwrap_or("?");
            crate::ui::kit::dropdown(ui, Id::new(("db-kind", c.id.as_str())), 170.0, kind, |ui| {
                for (k, name) in KINDS {
                    if kit::option(ui, c.kind == k, name).clicked() && c.kind != k {
                        c.kind = k.into();
                        c.test = None;
                    }
                }
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if kit::icon_button(ui, theme, icon("trash"), "×", t!("Remove this database")).clicked() {
                    action = Some(Action::Remove);
                }
            });
        });
        if let Some(p) = problem {
            ui.label(RichText::new(p).size(11.5).color(theme.error));
        }
        let field = |ui: &mut Ui, label: &str, value: &mut String, hint: &str, w: f32| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                kit::hint(ui, theme, label);
                ui.allocate_ui(vec2(w, 34.0), |ui| kit::text_input(ui, value, hint, false));
            });
        };
        let inner = width.min(520.0);
        if c.kind == "sqlite" {
            field(ui, t!("File"), &mut c.file, "bot.sqlite", inner);
            kit::hint(ui, theme, t!("In the bot's data folder, or a full path. Made when it's first used."));
        } else {
            crate::ui::kit::line(ui, |ui| {
                field(ui, t!("Host"), &mut c.host, "localhost", inner - 110.0);
                field(ui, t!("Port"), &mut c.port, default_port(&c.kind), 96.0);
            });
            crate::ui::kit::line(ui, |ui| {
                field(ui, t!("User"), &mut c.user, "", (inner - 8.0) / 2.0);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    kit::hint(ui, theme, t!("Password"));
                    secret_ui(ui, ed, theme, &mut c.password, (inner - 8.0) / 2.0, ("db-pw", &c.id));
                });
            });
            field(ui, t!("Database name"), &mut c.database, if c.kind == "postgres" { "postgres" } else { "" }, inner);
            if c.kind != "mongodb" {
                crate::ui::kit::line(ui, |ui| {
                    crate::ui::kit::toggle(ui, &mut c.ssl, theme);
                    ui.label(RichText::new(t!("Use SSL/TLS (for databases on the internet)")).size(12.5).color(theme.text));
                });
            }
            let example = match c.kind.as_str() {
                "postgres" => "postgres://user:password@host:5432/name",
                "mongodb" => "mongodb+srv://… (MongoDB Atlas)",
                _ => "mysql://user:password@host:3306/name",
            };
            kit::hint(ui, theme, tf!("Or a connection URL, used instead of the boxes above: {example}", example = example));
            secret_ui(ui, ed, theme, &mut c.url, inner, ("db-url", &c.id));
        }
        // Test, and what it found.
        ui.add_space(2.0);
        ui.horizontal_wrapped(|ui| {
            let running = matches!(c.test, Some(Test::Running(_)));
            if ui.add_enabled_ui(!running, |ui| kit::button(ui, theme, t!("Test connection"))).inner.clicked() {
                action = Some(Action::Test);
            }
            match &c.test {
                Some(Test::Running(_)) => {
                    ui.spinner();
                    ui.label(RichText::new(t!("Connecting … (the first time, its npm package is installed)")).size(12.0).color(theme.text_muted));
                }
                Some(Test::Done(Ok(r))) if r.ok => {
                    crate::ui::kit::done_label(ui, RichText::new(found(r)).size(12.0), theme.action);
                }
                Some(Test::Done(Ok(r))) => {
                    ui.label(RichText::new(r.error.clone().unwrap_or_default()).size(12.0).color(theme.error));
                }
                Some(Test::Done(Err(e))) => {
                    ui.label(RichText::new(e).size(12.0).color(theme.error));
                }
                None => {}
            }
        });
    });
    ui.add_space(8.0);
    action
}

/// "Connected · version 11.4 · 3 tables: levels, users, xc_store".
fn found(r: &TestResult) -> String {
    let tables = match r.tables.len() {
        0 => t!("no tables yet").to_string(),
        n => {
            let shown: Vec<&str> = r.tables.iter().take(6).map(String::as_str).collect();
            let more = if n > 6 { " …" } else { "" };
            tf!("{count} tables: {names}", count = n, names = format!("{}{more}", shown.join(", ")))
        }
    };
    format!("{} · {} {} · {tables}", t!("Connected"), t!("version"), r.version.as_deref().unwrap_or("?"))
}

/// A password or URL box, with the eye button and whether one is saved.
fn secret_ui(ui: &mut Ui, ed: &Editor, theme: &Theme, s: &mut Secret, width: f32, id: (&str, &str)) {
    let icon = |name: &str| ed.reg.icons.get(name).map(|p| p.as_path());
    ui.push_id(id, |ui| {
        crate::ui::kit::line(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            let hint = if s.saved && !s.remove { t!("•••••••• saved · type to replace") } else { "" };
            ui.allocate_ui(vec2((width - 80.0).max(80.0), 34.0), |ui| kit::text_input(ui, &mut s.value, hint, !s.reveal));
            let (eye, tip) = if s.reveal { (icon("eye-off"), t!("Hide")) } else { (icon("eye"), t!("Show what you typed")) };
            if kit::icon_button(ui, theme, eye, if s.reveal { t!("Hide") } else { "Show" }, tip).clicked() {
                s.reveal = !s.reveal;
            }
            if s.saved && kit::icon_button(ui, theme, icon(if s.remove { "check" } else { "x" }), "×", if s.remove { t!("Keep it") } else { t!("Remove saved value") }).clicked() {
                s.remove = !s.remove;
            }
        });
        if s.remove {
            ui.label(RichText::new(t!("Will be removed")).size(11.5).color(theme.error));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> std::path::PathBuf {
        std::env::temp_dir().join(format!("xc-db-draft-{}", std::process::id()))
    }

    /// Connections are kept with only what their kind uses; the default and
    /// the store follow removals; an empty page leaves the project as it was.
    #[test]
    fn saves_what_each_kind_uses() {
        let mut settings = Map::new();
        let mut d = DatabasesDraft::load(&settings, &root());
        assert_eq!(d.save(&mut settings, &root()), (false, vec![]), "nothing to save");
        d.add();
        d.add();
        assert_eq!(d.names(), ["Database", "Database 2"]);
        assert_eq!(d.default, "db1", "the first is the default");
        d.list[0].kind = "mariadb".into();
        d.list[0].port = "3307".into();
        d.list[0].file = "ignored.sqlite".into();
        d.list[0].ssl = true;
        d.list[1].kind = "sqlite".into();
        d.list[1].host = "ignored".into();
        d.list[1].file = "levels.sqlite".into();
        d.store_in = "db2".into();
        let (changed, errors) = d.save(&mut settings, &root());
        assert!(changed && errors.is_empty());
        assert_eq!(settings["databases"], json!([
            { "id": "db1", "name": "Database", "kind": "mariadb", "host": "localhost", "port": 3307, "ssl": true },
            { "id": "db2", "name": "Database 2", "kind": "sqlite", "file": "levels.sqlite" },
        ]));
        assert_eq!((settings["databaseDefault"].as_str(), settings["storeIn"].as_str()), (Some("db1"), Some("db2")));
        // Read back as it was.
        let mut again = DatabasesDraft::load(&settings, &root());
        assert_eq!(again.list[0].port, "3307");
        assert_eq!(again.store_in, "db2");
        // Removing one: the store goes back to the JSON file, which isn't written.
        again.list.remove(1);
        again.removed.push("db2".into());
        again.store_in = "json".into();
        again.save(&mut settings, &root());
        assert!(!settings.contains_key("storeIn"));
        assert_eq!(settings["databases"].as_array().unwrap().len(), 1);
        assert_eq!(secret_keys(&settings), [("db_db1_password".to_string(), "Database: password".to_string()), ("db_db1_url".to_string(), "Database: connection URL".to_string())]);
    }

    #[test]
    fn names_and_ports_are_checked() {
        let mut d = DatabasesDraft::load(&Map::new(), &root());
        d.add();
        d.add();
        assert_eq!(d.problems(), 0);
        d.list[1].name = " database ".into();
        assert_eq!(d.problems(), 2, "the same name twice");
        d.list[1].name = "Other".into();
        d.list[1].port = "99999".into();
        assert_eq!(d.problems(), 1);
        d.list[1].kind = "sqlite".into();
        assert_eq!(d.problems(), 0, "SQLite has no port");
        d.list[0].name.clear();
        assert_eq!(d.problems(), 1);
    }

    #[test]
    fn describes_what_a_test_found() {
        let r = TestResult { ok: true, version: Some("11.4.2".into()), tables: (1..=8).map(|i| format!("t{i}")).collect(), ..Default::default() };
        assert_eq!(found(&r), "Connected · version 11.4.2 · 8 tables: t1, t2, t3, t4, t5, t6 …");
        assert!(found(&TestResult { ok: true, ..Default::default() }).ends_with("no tables yet"));
    }
}
