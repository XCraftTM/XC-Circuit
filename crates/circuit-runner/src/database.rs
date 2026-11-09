//! The editor's way into the bot's databases, connecting the way the bot will
//! (the same runtime and npm packages): Bot › Settings › Databases › Test
//! says what it found there, and Stored Data reads and changes stored values
//! and tables.

use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use indexmap::IndexMap;
use serde::Deserialize;
use serde_json::Value;

use crate::node::{check_node, ensure_shared_packages, quiet};

/// What a test found.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct TestResult {
    pub ok: bool,
    /// The server's version (SQLite's own for SQLite).
    #[serde(default)]
    pub version: Option<String>,
    /// Its tables (MongoDB: collections).
    #[serde(default)]
    pub tables: Vec<String>,
    /// SQLite: the file it opened.
    #[serde(default)]
    pub file: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
}

/// How long a test may take, packages already installed.
const TIMEOUT: Duration = Duration::from_secs(30);

pub struct TestConfig<'a> {
    /// The connection, as in the project's `databases` setting.
    pub spec: &'a Value,
    /// Its password and URL, as `XC_SECRET_DB_<ID>_PASSWORD` / `_URL`.
    pub secrets: Vec<(String, String)>,
    /// Every npm package the bot needs (installing only some would remove the others).
    pub dependencies: &'a IndexMap<String, String>,
    pub packages_dir: &'a Path,
    pub run_dir: &'a Path,
    /// Where a SQLite file without a full path is.
    pub data_dir: &'a Path,
}

/// Connects with `cfg.spec`; `log` gets what happens on the way (npm installs).
pub fn test_database(cfg: &TestConfig, log: &mut dyn FnMut(String)) -> Result<TestResult> {
    let (out, err) = run_runtime(cfg, "--db-test", ("XC_DB_TEST", cfg.spec.to_string()), log)?;
    parse(&out).with_context(|| format!("the test gave no answer: {}", err.trim()))
}

/// One request to the database in `cfg.spec` from Stored Data, such as
/// `{"op": "rows", "table": "levels", "offset": 0, "limit": 50}` (see
/// `browse` in the runtime). Returns the answer, or what went wrong.
pub fn browse_database(cfg: &TestConfig, request: &Value, log: &mut dyn FnMut(String)) -> Result<Value> {
    let mut request = request.clone();
    if let Some(r) = request.as_object_mut() {
        r.insert("spec".into(), cfg.spec.clone());
    }
    let (out, err) = run_runtime(cfg, "--db-browse", ("XC_DB_REQUEST", request.to_string()), log)?;
    let answer: Value = out.lines().rev().find_map(|l| serde_json::from_str(l.trim()).ok()).with_context(|| format!("no answer: {}", err.trim()))?;
    if answer.get("ok").and_then(Value::as_bool) == Some(true) {
        Ok(answer)
    } else {
        bail!("{}", answer.get("error").and_then(Value::as_str).unwrap_or("it failed"))
    }
}

/// A database kept connected for Stored Data: the runtime in `--db-serve`
/// mode, answering one request after another without starting again (npm
/// packages checked, Node.js started and the connection made once). It ends
/// when dropped, or by itself after ten minutes without a request.
pub struct DbHelper {
    child: Child,
    stdin: ChildStdin,
    lines: Receiver<String>,
    errors: std::sync::Arc<std::sync::Mutex<String>>,
    next: u64,
}

impl DbHelper {
    /// Starts it for the connection in `cfg.spec`.
    pub fn start(cfg: &TestConfig, log: &mut dyn FnMut(String)) -> Result<DbHelper> {
        check_node()?;
        ensure_shared_packages(cfg.packages_dir, cfg.dependencies, log, &|| false)?;
        fs::create_dir_all(cfg.data_dir)?;
        let runtime = crate::write_runtime(cfg.run_dir)?;
        let mut cmd = crate::node::node_command();
        cmd.arg(&runtime)
            .arg("--db-serve")
            .arg("--deps")
            .arg(cfg.packages_dir)
            .arg("--data")
            .arg(cfg.data_dir)
            .env("XC_DB_SPEC", cfg.spec.to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (k, v) in &cfg.secrets {
            cmd.env(k, v);
        }
        let mut child = quiet(&mut cmd).spawn().context("starting Node.js")?;
        let stdin = child.stdin.take().context("no input to the database helper")?;
        let (tx, lines) = channel();
        if let Some(out) = child.stdout.take() {
            std::thread::spawn(move || {
                for line in crate::node::lines_lossy(out) {
                    if tx.send(line).is_err() {
                        break;
                    }
                }
            });
        }
        // What it says on stderr, for when it stops: the last few lines.
        let errors = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        if let Some(err) = child.stderr.take() {
            let keep = errors.clone();
            std::thread::spawn(move || {
                for line in crate::node::lines_lossy(err) {
                    if let Ok(mut e) = keep.lock() {
                        e.push_str(&line);
                        e.push('\n');
                        let cut = e.len().saturating_sub(2000);
                        if cut > 0 {
                            e.drain(..cut);
                        }
                    }
                }
            });
        }
        Ok(DbHelper { child, stdin, lines, errors, next: 0 })
    }

    /// Whether it still runs.
    pub fn alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    /// Sends one request (see `browse` in the runtime). The outer error: the
    /// helper itself failed (start another); the inner one: what the
    /// database said went wrong.
    pub fn ask(&mut self, request: &Value) -> Result<std::result::Result<Value, String>> {
        self.next += 1;
        let id = self.next;
        writeln!(self.stdin, "{}", serde_json::json!({ "id": id, "request": request })).context("the database helper stopped")?;
        self.stdin.flush().context("the database helper stopped")?;
        let deadline = Instant::now() + TIMEOUT;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.lines.recv_timeout(left) {
                Ok(line) => {
                    let Ok(answer) = serde_json::from_str::<Value>(line.trim()) else { continue };
                    if answer.get("id").and_then(Value::as_u64) != Some(id) {
                        continue;
                    }
                    return Ok(if answer.get("ok").and_then(Value::as_bool) == Some(true) {
                        Ok(answer)
                    } else {
                        Err(answer.get("error").and_then(Value::as_str).unwrap_or("it failed").to_string())
                    });
                }
                Err(RecvTimeoutError::Timeout) => {
                    let _ = self.child.kill();
                    bail!("no answer after {} seconds", TIMEOUT.as_secs());
                }
                Err(RecvTimeoutError::Disconnected) => {
                    let said = self.errors.lock().map(|e| e.trim().to_string()).unwrap_or_default();
                    bail!("the database helper stopped{}", if said.is_empty() { String::new() } else { format!(": {said}") });
                }
            }
        }
    }
}

impl Drop for DbHelper {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Runs the runtime with `flag` and the request in an environment variable;
/// gives what it printed and its errors.
fn run_runtime(cfg: &TestConfig, flag: &str, (env_key, env_value): (&str, String), log: &mut dyn FnMut(String)) -> Result<(String, String)> {
    check_node()?;
    ensure_shared_packages(cfg.packages_dir, cfg.dependencies, log, &|| false)?;
    fs::create_dir_all(cfg.run_dir)?;
    fs::create_dir_all(cfg.data_dir)?;
    let runtime = crate::write_runtime(cfg.run_dir)?;
    let mut cmd = crate::node::node_command();
    cmd.arg(&runtime)
        .arg(flag)
        .arg("--deps")
        .arg(cfg.packages_dir)
        .arg("--data")
        .arg(cfg.data_dir)
        .env(env_key, env_value)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (k, v) in &cfg.secrets {
        cmd.env(k, v);
    }
    let mut child = quiet(&mut cmd).spawn().context("starting Node.js")?;
    // Read while it runs: an answer bigger than the pipe would otherwise
    // keep it from finishing.
    let read = |pipe: Option<Box<dyn Read + Send>>| {
        std::thread::spawn(move || {
            let mut text = String::new();
            if let Some(mut p) = pipe {
                let _ = p.read_to_string(&mut text);
            }
            text
        })
    };
    let out = read(child.stdout.take().map(|p| Box::new(p) as Box<dyn Read + Send>));
    let err = read(child.stderr.take().map(|p| Box::new(p) as Box<dyn Read + Send>));
    let started = Instant::now();
    loop {
        if child.try_wait()?.is_some() {
            break;
        }
        if started.elapsed() > TIMEOUT {
            let _ = child.kill();
            bail!("no answer after {} seconds", TIMEOUT.as_secs());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Ok((out.join().unwrap_or_default(), err.join().unwrap_or_default()))
}

/// The answer: the last line of JSON the runtime printed.
fn parse(out: &str) -> Option<TestResult> {
    out.lines().rev().find_map(|l| serde_json::from_str(l.trim()).ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_the_answer() {
        let ok = parse("noise\n{\"ok\":true,\"version\":\"3.46.0\",\"tables\":[\"users\"]}\n").unwrap();
        assert_eq!((ok.ok, ok.version.as_deref(), ok.tables.as_slice()), (true, Some("3.46.0"), &["users".to_string()][..]));
        let bad = parse("{\"ok\":false,\"error\":\"Main: the user name or password is wrong.\"}").unwrap();
        assert!(!bad.ok && bad.error.unwrap().contains("password"));
        assert!(parse("nothing").is_none());
    }

    /// A SQLite file, through the real runtime.
    #[test]
    fn tests_a_sqlite_file() {
        if check_node().is_err() {
            return;
        }
        let dir = std::env::temp_dir().join(format!("xc-db-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let spec = json!({ "id": "db1", "name": "Main", "kind": "sqlite", "file": "bot.sqlite" });
        let deps = IndexMap::new();
        let cfg = TestConfig { spec: &spec, secrets: vec![], dependencies: &deps, packages_dir: &dir.join("packages"), run_dir: &dir.join("run"), data_dir: &dir.join("data") };
        let r = test_database(&cfg, &mut |_| {}).unwrap();
        assert!(r.ok, "{r:?}");
        assert!(r.version.is_some() && r.tables.is_empty());
        assert!(dir.join("data/bot.sqlite").is_file());
        // A kind it doesn't know is an answer too, not an error.
        let spec = json!({ "id": "db2", "name": "Odd", "kind": "oracle" });
        let r = test_database(&TestConfig { spec: &spec, ..cfg }, &mut |_| {}).unwrap();
        assert!(!r.ok && r.error.unwrap().contains("oracle"));
        let _ = fs::remove_dir_all(&dir);
    }

    /// The kept-connected helper answers request after request, and says a
    /// database's error as an answer, not as its own failure.
    #[test]
    fn a_helper_answers_one_request_after_another() {
        if check_node().is_err() {
            return;
        }
        let dir = std::env::temp_dir().join(format!("xc-db-helper-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let spec = json!({ "id": "db1", "name": "Main", "kind": "sqlite", "file": "bot.sqlite" });
        let deps = IndexMap::new();
        let cfg = TestConfig { spec: &spec, secrets: vec![], dependencies: &deps, packages_dir: &dir.join("packages"), run_dir: &dir.join("run"), data_dir: &dir.join("data") };
        let mut helper = DbHelper::start(&cfg, &mut |_| {}).unwrap();
        let started = Instant::now();
        for n in 0..5 {
            helper.ask(&json!({ "op": "store-set", "bucket": "global", "name": format!("v{n}"), "value": n })).unwrap().unwrap();
        }
        let store = helper.ask(&json!({ "op": "store" })).unwrap().unwrap();
        assert_eq!(store["values"].as_array().unwrap().len(), 5);
        assert!(started.elapsed() < Duration::from_secs(5), "one Node.js for all of them: {:?}", started.elapsed());
        let err = helper.ask(&json!({ "op": "rows", "table": "nope" })).unwrap().unwrap_err();
        assert!(err.contains("nope"), "{err}");
        assert!(helper.alive(), "an error is an answer, not the end");
        drop(helper);
        let _ = fs::remove_dir_all(&dir);
    }

    /// Stored Data's requests on a SQLite file: stored values and a table's rows,
    /// read, added, changed and deleted.
    #[test]
    fn browses_and_changes_a_sqlite_file() {
        if check_node().is_err() {
            return;
        }
        let dir = std::env::temp_dir().join(format!("xc-db-browse-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        // Inside a folder of ES modules: the runtime still runs as CommonJS.
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("package.json"), r#"{ "type": "module" }"#).unwrap();
        let spec = json!({ "id": "db1", "name": "Main", "kind": "sqlite", "file": "bot.sqlite" });
        let deps = IndexMap::new();
        let cfg = TestConfig { spec: &spec, secrets: vec![], dependencies: &deps, packages_dir: &dir.join("packages"), run_dir: &dir.join("run"), data_dir: &dir.join("data") };
        let ask = |req: Value| browse_database(&cfg, &req, &mut |_| {});

        ask(json!({ "op": "store-set", "bucket": "global", "name": "starts", "value": 3 })).unwrap();
        ask(json!({ "op": "store-set", "bucket": "user:1", "name": "xp", "value": { "level": 2 } })).unwrap();
        let store = ask(json!({ "op": "store" })).unwrap();
        let values = store["values"].as_array().unwrap();
        assert_eq!(values.len(), 2, "{store}");
        assert!(values.iter().any(|v| v["bucket"] == "user:1" && v["value"] == "{\"level\":2}"), "{store}");
        ask(json!({ "op": "store-delete", "bucket": "global", "name": "starts" })).unwrap();
        assert_eq!(ask(json!({ "op": "store" })).unwrap()["values"].as_array().unwrap().len(), 1);

        // A table the bot made, with a primary key.
        let tables = ask(json!({ "op": "tables" })).unwrap();
        assert_eq!(tables["tables"], json!(["xc_store"]));
        let key = json!({ "bucket": "user:1", "name": "xp" });
        let rows = ask(json!({ "op": "rows", "table": "xc_store", "offset": 0, "limit": 10 })).unwrap();
        assert_eq!((rows["total"].as_u64(), &rows["key"]), (Some(1), &json!(["bucket", "name"])), "{rows}");
        assert_eq!(rows["columns"], json!(["bucket", "name", "value"]));
        ask(json!({ "op": "row-update", "table": "xc_store", "key": key, "values": { "value": "5" } })).unwrap();
        assert_eq!(ask(json!({ "op": "rows", "table": "xc_store" })).unwrap()["rows"][0]["value"], "5");
        ask(json!({ "op": "row-insert", "table": "xc_store", "values": { "bucket": "global", "name": "new", "value": "1" } })).unwrap();
        ask(json!({ "op": "row-delete", "table": "xc_store", "key": key })).unwrap();
        let rows = ask(json!({ "op": "rows", "table": "xc_store" })).unwrap();
        assert_eq!(rows["rows"].as_array().unwrap().len(), 1);
        assert_eq!(rows["rows"][0]["name"], "new");

        // Without a key, nothing changes: not every row.
        let err = ask(json!({ "op": "row-delete", "table": "xc_store", "key": {} })).unwrap_err();
        assert!(err.to_string().contains("primary key"), "{err}");
        assert!(ask(json!({ "op": "rows", "table": "nope; DROP TABLE xc_store" })).is_err(), "names are checked");
        assert_eq!(ask(json!({ "op": "rows", "table": "xc_store" })).unwrap()["total"], 1);
        let _ = fs::remove_dir_all(&dir);
    }
}
