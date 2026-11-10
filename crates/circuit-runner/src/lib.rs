//! Running and exporting projects: builds the bundle the Node runtime reads,
//! installs npm packages, starts and stops the bot process, parses the debug
//! protocol, keeps secrets in the OS credential store, and writes exports.

pub mod bundle;
pub mod database;
pub mod export;
pub mod host_link;
pub mod hosting;
pub mod node;
pub mod node_setup;
pub mod protocol;
pub mod secrets;
pub mod session;

pub use bundle::{Bundle, build_bundle};
pub use export::{ExportOptions, ExportSecret, export};
pub use protocol::{BotEvent, parse_line};
pub use session::{Msg, Session, SessionConfig};

/// The runtime that executes block graphs, shipped inside the program.
pub const RUNTIME_SOURCE: &str = include_str!("../../../runtime/circuit-runtime.js");

/// Writes the runtime into `run_dir` to run it from there. A package.json
/// next to it says it's CommonJS: Node otherwise goes by the nearest one
/// above, and a project kept inside a folder whose package.json says
/// `"type": "module"` couldn't run its bot.
pub fn write_runtime(run_dir: &std::path::Path) -> std::io::Result<std::path::PathBuf> {
    std::fs::create_dir_all(run_dir)?;
    std::fs::write(run_dir.join("package.json"), "{ \"private\": true, \"type\": \"commonjs\" }
")?;
    let runtime = run_dir.join("circuit-runtime.js");
    std::fs::write(&runtime, RUNTIME_SOURCE)?;
    Ok(runtime)
}
