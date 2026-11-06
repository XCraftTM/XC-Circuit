//! Bot › Settings › Databases › Test: connects the way the bot will (the same
//! runtime and npm packages) and says what it found there.

use std::fs;
use std::io::Read;
use std::path::Path;
use std::process::Stdio;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use indexmap::IndexMap;
use serde::Deserialize;
use serde_json::Value;

use crate::RUNTIME_SOURCE;
use crate::node::{check_node, ensure_packages, quiet};

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
    check_node()?;
    ensure_packages(cfg.packages_dir, "xc-circuit-bot-packages", cfg.dependencies, log, &|| false)?;
    fs::create_dir_all(cfg.run_dir)?;
    fs::create_dir_all(cfg.data_dir)?;
    let runtime = cfg.run_dir.join("circuit-runtime.js");
    fs::write(&runtime, RUNTIME_SOURCE)?;
    let mut cmd = crate::node::node_command();
    cmd.arg(&runtime)
        .arg("--db-test")
        .arg("--deps")
        .arg(cfg.packages_dir)
        .arg("--data")
        .arg(cfg.data_dir)
        .env("XC_DB_TEST", cfg.spec.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (k, v) in &cfg.secrets {
        cmd.env(k, v);
    }
    let mut child = quiet(&mut cmd).spawn().context("starting Node.js")?;
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
    let mut out = String::new();
    if let Some(mut s) = child.stdout.take() {
        s.read_to_string(&mut out)?;
    }
    let mut err = String::new();
    if let Some(mut s) = child.stderr.take() {
        s.read_to_string(&mut err)?;
    }
    parse(&out).with_context(|| format!("the test gave no answer: {}", err.trim()))
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
}
