//! A running bot: prepared and started on a background thread, its output
//! delivered as messages the editor polls each frame.

use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};
use std::thread;

use anyhow::Result;
use indexmap::IndexMap;
use serde_json::Value;

use crate::RUNTIME_SOURCE;
use crate::node::{check_node, ensure_packages, quiet};
use crate::protocol::{BotEvent, parse_line};

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
            match prepare_and_spawn(&cfg, &send) {
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
                                for line in BufReader::new(stream).lines().map_while(|l| l.ok()) {
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
                    send(Msg::Error(format!("{e:#}")));
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

    pub fn stop(&self) {
        *self.stopping.lock().unwrap() = true;
        if let Some(c) = self.child.lock().unwrap().as_mut() {
            let _ = c.kill();
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.stop();
    }
}

fn prepare_and_spawn(cfg: &SessionConfig, send: &dyn Fn(Msg)) -> Result<Child> {
    let version = check_node()?;
    send(Msg::Info(format!("Using Node.js {version}")));
    ensure_packages(&cfg.packages_dir, "xc-circuit-bot-packages", &cfg.dependencies, &mut |line| send(Msg::Info(line)))?;

    fs::create_dir_all(&cfg.run_dir)?;
    fs::create_dir_all(&cfg.data_dir)?;
    let runtime = cfg.run_dir.join("circuit-runtime.js");
    fs::write(&runtime, RUNTIME_SOURCE)?;
    let bundle = cfg.run_dir.join("bundle.json");
    fs::write(&bundle, serde_json::to_string(&cfg.bundle)?)?;

    let mut cmd = Command::new("node");
    cmd.arg(&runtime)
        .arg("--bundle")
        .arg(&bundle)
        .arg("--deps")
        .arg(&cfg.packages_dir)
        .arg("--data")
        .arg(&cfg.data_dir)
        .arg("--debug")
        .current_dir(&cfg.run_dir)
        .stdin(Stdio::null())
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
                "l": { "block": "core.log@1", "at": [0, 0], "values": { "message": "level {lvl}, {toNext} to go", "level": "info" } }
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
                "3": { "block": "core.log@1", "at": [0, 0], "values": { "message": "sum is {result}", "level": "success" } }
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
