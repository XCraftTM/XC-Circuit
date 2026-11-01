//! Running and exporting projects: builds the bundle the Node runtime reads,
//! installs npm packages, starts and stops the bot process, parses the debug
//! protocol, keeps secrets in the OS credential store, and writes exports.

pub mod bundle;
pub mod export;
pub mod node;
pub mod protocol;
pub mod secrets;
pub mod session;

pub use bundle::{Bundle, build_bundle};
pub use export::{ExportOptions, export};
pub use protocol::{BotEvent, parse_line};
pub use session::{Msg, Session, SessionConfig};

/// The runtime that executes block graphs, shipped inside the program.
pub const RUNTIME_SOURCE: &str = include_str!("../../../runtime/circuit-runtime.js");
