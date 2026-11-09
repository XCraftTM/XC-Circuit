//! A running bot: prepared and started on a background thread, its output
//! delivered as messages the editor polls each frame.

use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Child, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use anyhow::Result;
use indexmap::IndexMap;
use serde_json::Value;

use crate::node::{check_node, ensure_shared_packages, lines_lossy, quiet};
use crate::protocol::{BotEvent, parse_line};

/// How long a bot gets to stop by itself before it is ended.
const STOP_GRACE: Duration = Duration::from_millis(1500);

pub struct SessionConfig {
    pub bundle: Value,
    pub dependencies: IndexMap<String, String>,
    /// Shared folder where npm packages for running bots are installed.
    pub packages_dir: PathBuf,
    /// Where the runtime file and bundle are written (`<project>/.circuit/run`).
    pub run_dir: PathBuf,
    /// Where `ctx.store` keeps its data.
    pub data_dir: PathBuf,
    /// Extra environment, such as `XC_SECRET_TOKEN`.
    pub env: Vec<(String, String)>,
}

#[derive(Debug)]
pub enum Msg {
    /// A message from XC Circuit itself (preparing, installing, errors).
    Info(String),
    Error(String),
    /// The bot process started.
    Started,
    /// A line from the bot.
    Bot(BotEvent),
    /// The bot process ended.
    Exited(Option<i32>),
}

pub struct Session {
    rx: Receiver<Msg>,
    child: Arc<Mutex<Option<Child>>>,
    stopping: Arc<Mutex<bool>>,
}

impl Session {
    /// Starts preparing and running in the background. `wake` is called whenever a
    /// message arrives, so the UI can repaint.
    pub fn start(cfg: SessionConfig, wake: impl Fn() + Send + Sync + 'static) -> Session {
        let (tx, rx) = channel();
        let child = Arc::new(Mutex::new(None));
        let stopping = Arc::new(Mutex::new(false));
        let wake: Arc<dyn Fn() + Send + Sync> = Arc::new(wake);
        let (c, s) = (child.clone(), stopping.clone());
        thread::spawn(move || {
            let send = |m: Msg| {
                let _ = tx.send(m);
                wake();
            };
            let stopping = || *s.lock().unwrap_or_else(|e| e.into_inner());
            match prepare_and_spawn(&cfg, &send, &stopping) {
                Ok(mut proc) => {
                    if *s.lock().unwrap() {
                        let _ = proc.kill();
                        send(Msg::Exited(None));
                        return;
                    }
                    let out = proc.stdout.take();
                    let err = proc.stderr.take();
                    *c.lock().unwrap() = Some(proc);
                    send(Msg::Started);
                    let readers: Vec<_> = [out.map(|o| Box::new(o) as Box<dyn Read + Send>), err.map(|e| Box::new(e) as Box<dyn Read + Send>)]
                        .into_iter()
                        .flatten()
                        .map(|stream| {
                            let tx = tx.clone();
                            let wake = wake.clone();
                            thread::spawn(move || {
                                for line in lines_lossy(stream) {
                                    let _ = tx.send(Msg::Bot(parse_line(&line)));
                                    wake();
                                }
                            })
                        })
                        .collect();
                    for r in readers {
                        let _ = r.join();
                    }
                    let code = c.lock().unwrap().as_mut().and_then(|p| p.wait().ok()).and_then(|s| s.code());
                    *c.lock().unwrap() = None;
                    send(Msg::Exited(code));
                }
                Err(e) => {
                    // Stopped while preparing: not an error.
                    send(if stopping() { Msg::Info(format!("{e:#}")) } else { Msg::Error(format!("{e:#}")) });
                    send(Msg::Exited(None));
                }
            }
        });
        Session { rx, child, stopping }
    }

    /// Messages that arrived since the last call.
    pub fn poll(&self) -> Vec<Msg> {
        self.rx.try_iter().collect()
    }

    /// Replies to a `BotEvent::Ask`.
    pub fn answer(&self, id: &str, value: &str) {
        let line = format!("answer {} {}\n", id.replace(char::is_whitespace, ""), value.replace(['\r', '\n'], " "));
        if let Some(c) = self.child.lock().unwrap().as_mut()
            && let Some(input) = c.stdin.as_mut()
        {
            let _ = input.write_all(line.as_bytes()).and_then(|_| input.flush());
        }
    }

    /// Asks the bot to stop, so it can write its saved data, and ends it after
    /// a moment if it hasn't: with the processes it started (on Windows), so
    /// none of them (a player, a converter) stays behind. A bot that can't be
    /// asked is ended at once.
    pub fn stop(&self) {
        *self.stopping.lock().unwrap() = true;
        let asked = {
            let mut guard = self.child.lock().unwrap();
            let Some(c) = guard.as_mut() else { return };
            c.stdin.as_mut().is_some_and(|i| i.write_all(b"stop\n").and_then(|_| i.flush()).is_ok())
        };
        // On a thread of its own: ending the processes takes a moment too.
        let child = self.child.clone();
        thread::spawn(move || {
            if asked {
                thread::sleep(STOP_GRACE);
            }
            if let Some(c) = child.lock().unwrap().as_mut() {
                crate::node::end_tree(c);
            }
        });
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.stop();
    }
}

fn prepare_and_spawn(cfg: &SessionConfig, send: &dyn Fn(Msg), stopping: &dyn Fn() -> bool) -> Result<Child> {
    let version = check_node()?;
    send(Msg::Info(format!("Using Node.js {version}")));
    // Stopping while packages install ends npm instead of waiting for it.
    ensure_shared_packages(&cfg.packages_dir, &cfg.dependencies, &mut |line| send(Msg::Info(line)), stopping)?;

    fs::create_dir_all(&cfg.run_dir)?;
    fs::create_dir_all(&cfg.data_dir)?;
    let runtime = crate::write_runtime(&cfg.run_dir)?;
    let bundle = cfg.run_dir.join("bundle.json");
    fs::write(&bundle, serde_json::to_string(&cfg.bundle)?)?;

    let mut cmd = crate::node::node_command();
    cmd.arg(&runtime)
        .arg("--bundle")
        .arg(&bundle)
        .arg("--deps")
        .arg(&cfg.packages_dir)
        .arg("--data")
        .arg(&cfg.data_dir)
        .arg("--debug")
        .arg("--stop-on-input")
        .current_dir(&cfg.run_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (k, v) in &cfg.env {
        cmd.env(k, v);
    }
    Ok(quiet(&mut cmd).spawn()?)
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::time::{Duration, Instant};

    use circuit_core::{Project, Workspace};
    use circuit_plugins::{BlockEngine, Registry};

    use super::*;
    use crate::bundle::build_bundle;

    /// Runs a project and returns once a log line equal to `want` appears.
    fn run_until(reg: &Registry, ws: Workspace, name: &str, want: &str) {
        let dir = std::env::temp_dir().join(format!("xc-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let mut project = Project::create(&dir, name, "none").unwrap();
        project.workspaces.insert("main".into(), ws);
        let bundle = build_bundle(&project, reg, None).unwrap();
        let session = Session::start(
            SessionConfig {
                bundle: bundle.value,
                dependencies: bundle.dependencies,
                packages_dir: dir.join("packages"),
                run_dir: dir.join(".circuit/run"),
                data_dir: dir.join(".circuit/data"),
                env: vec![],
            },
            || {},
        );
        let deadline = Instant::now() + Duration::from_secs(20);
        let mut seen = Vec::new();
        while Instant::now() < deadline {
            for m in session.poll() {
                if let Msg::Bot(BotEvent::Log { text, .. }) = &m
                    && text == want
                {
                    session.stop();
                    return;
                }
                seen.push(format!("{m:?}"));
            }
            thread::sleep(Duration::from_millis(50));
        }
        panic!("never saw {want:?}; got:\n{}", seen.join("\n"));
    }

    /// Stopping the bot lets it write its saved data first: a value stored just
    /// before Stop is on disk afterwards.
    #[test]
    fn stopping_keeps_saved_data() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let engine = BlockEngine::new().unwrap();
        let reg = Registry::load(&[root.join("plugins")], &engine);
        let dir = std::env::temp_dir().join(format!("xc-stop-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let mut project = Project::create(&dir, "Stop Test", "none").unwrap();
        let ws: Workspace = serde_json::from_value(serde_json::json!({
            "name": "Main",
            "nodes": {
                "1": { "block": "core.on_start@1", "at": [0, 0] },
                "2": { "block": "core.increase_stored@1", "at": [0, 0], "values": { "scope": "global", "key": "starts", "by": 1 } },
                "3": { "block": "core.log@1", "at": [0, 0], "values": { "message": "counted", "level": "info" } }
            },
            "links": ["1.next > 2.in", "2.next > 3.in"]
        }))
        .unwrap();
        project.workspaces.insert("main".into(), ws);
        let bundle = build_bundle(&project, &reg, None).unwrap();
        let data_dir = dir.join(".circuit/data");
        let session = Session::start(
            SessionConfig { bundle: bundle.value, dependencies: bundle.dependencies, packages_dir: dir.join("packages"), run_dir: dir.join(".circuit/run"), data_dir: data_dir.clone(), env: vec![] },
            || {},
        );
        let deadline = Instant::now() + Duration::from_secs(20);
        let mut stopped = false;
        let mut exited = false;
        while Instant::now() < deadline && !exited {
            for m in session.poll() {
                match m {
                    Msg::Bot(BotEvent::Log { text, .. }) if text == "counted" && !stopped => {
                        // At once: the store's batched write hasn't happened yet.
                        session.stop();
                        stopped = true;
                    }
                    Msg::Exited(_) => exited = true,
                    _ => {}
                }
            }
            thread::sleep(Duration::from_millis(10));
        }
        assert!(stopped && exited, "the bot ran and stopped");
        let saved: serde_json::Value = serde_json::from_str(&fs::read_to_string(data_dir.join("store.json")).expect("store.json written on stop")).unwrap();
        assert_eq!(saved["global"]["starts"], 1);
    }

    /// An empty store.json makes the bot ask; answering picks a backup.
    #[test]
    fn empty_saved_data_asks_and_restores() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let engine = BlockEngine::new().unwrap();
        let reg = Registry::load(&[root.join("plugins")], &engine);
        let dir = std::env::temp_dir().join(format!("xc-ask-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let mut project = Project::create(&dir, "Ask Test", "none").unwrap();
        let ws: Workspace = serde_json::from_value(serde_json::json!({
            "name": "Main",
            "nodes": {
                "1": { "block": "core.on_start@1", "at": [0, 0] },
                "2": { "block": "core.get_stored@1", "at": [0, 0], "values": { "scope": "global", "key": "level", "fallback": 0 } },
                "3": { "block": "core.log@1", "at": [0, 0], "values": { "message": "level ${value}", "level": "info" } }
            },
            "links": ["1.next > 3.in", "2.value > 3.value"]
        }))
        .unwrap();
        project.workspaces.insert("main".into(), ws);
        let data_dir = dir.join(".circuit/data");
        fs::create_dir_all(data_dir.join("backups")).unwrap();
        fs::write(data_dir.join("backups/store-2026-01-01T00-00-00-000Z.json"), r#"{"global":{"level":7}}"#).unwrap();
        fs::write(data_dir.join("store.json"), "").unwrap();

        let bundle = build_bundle(&project, &reg, None).unwrap();
        let session = Session::start(
            SessionConfig { bundle: bundle.value, dependencies: bundle.dependencies, packages_dir: dir.join("packages"), run_dir: dir.join(".circuit/run"), data_dir: data_dir.clone(), env: vec![] },
            || {},
        );
        let deadline = Instant::now() + Duration::from_secs(20);
        let mut asked = false;
        let mut seen = Vec::new();
        while Instant::now() < deadline {
            for m in session.poll() {
                match &m {
                    Msg::Bot(BotEvent::Ask { id, choices, .. }) => {
                        assert_eq!(choices.len(), 2, "the backup, then the empty template");
                        assert!(choices[0].value.starts_with("backup:"));
                        session.answer(id, &choices[0].value);
                        asked = true;
                    }
                    Msg::Bot(BotEvent::Log { text, .. }) if text == "level 7" => {
                        assert!(asked);
                        session.stop();
                        return;
                    }
                    _ => {}
                }
                seen.push(format!("{m:?}"));
            }
            thread::sleep(Duration::from_millis(20));
        }
        panic!("never restored: {}", seen.join("\n"));
    }

    /// A TypeScript block from a project's own plugins/ folder runs in the bot.
    #[test]
    fn runs_a_typescript_block() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let engine = BlockEngine::new().unwrap();
        let reg = Registry::load(&[root.join("plugins"), root.join("examples/DemoBot/plugins")], &engine);
        assert!(reg.problems.is_empty(), "{:?}", reg.problems);
        assert!(reg.block("demo.level_from_xp").and_then(|d| d.compiled.as_ref()).is_some());
        let ws: Workspace = serde_json::from_value(serde_json::json!({
            "name": "Main",
            "nodes": {
                "a": { "block": "core.on_start@1", "at": [0, 0] },
                "lv": { "block": "demo.level_from_xp@1", "at": [0, 0], "values": { "xp": 250, "perLevel": 100 } },
                "l": { "block": "core.log@1", "at": [0, 0], "values": { "message": "level ${lvl}, ${toNext} to go", "level": "info" } }
            },
            "links": ["a.next > l.in", "lv.level > l.lvl", "lv.toNext > l.toNext"]
        }))
        .unwrap();
        run_until(&reg, ws, "ts", "level 3, 50 to go");
    }

    /// Loads the bundled plugins, builds a project in a temp folder, runs it with
    /// Node and waits for the log line the graph produces.
    #[test]
    fn runs_a_project_end_to_end() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let engine = BlockEngine::new().unwrap();
        let reg = Registry::load(&[root.join("plugins")], &engine);

        let dir = std::env::temp_dir().join(format!("xc-session-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let mut project = Project::create(&dir, "Session Test", "none").unwrap();
        let ws: Workspace = serde_json::from_value(serde_json::json!({
            "name": "Main",
            "nodes": {
                "1": { "block": "core.on_start@1", "at": [0, 0] },
                "2": { "block": "core.math@1", "at": [0, 0], "values": { "a": 20, "op": "add", "b": 22 } },
                "3": { "block": "core.log@1", "at": [0, 0], "values": { "message": "sum is ${result}", "level": "success" } }
            },
            "links": ["1.next > 3.in", "2.result > 3.result"]
        }))
        .unwrap();
        project.workspaces.insert("main".into(), ws);

        let bundle = build_bundle(&project, &reg, None).unwrap();
        assert_eq!(bundle.value["blocks"].as_object().unwrap().len(), 3);
        let session = Session::start(
            SessionConfig {
                bundle: bundle.value,
                dependencies: bundle.dependencies,
                packages_dir: dir.join("packages"),
                run_dir: dir.join(".circuit/run"),
                data_dir: dir.join(".circuit/data"),
                env: vec![],
            },
            || {},
        );

        let deadline = Instant::now() + Duration::from_secs(20);
        let mut seen = Vec::new();
        while Instant::now() < deadline {
            for m in session.poll() {
                if let Msg::Bot(BotEvent::Log { text, .. }) = &m
                    && text == "sum is 42"
                {
                    session.stop();
                    return;
                }
                seen.push(format!("{m:?}"));
            }
            thread::sleep(Duration::from_millis(50));
        }
        panic!("never saw the log line; got:\n{}", seen.join("\n"));
    }
}
