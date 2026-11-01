//! Plugins: manifests, their contributions (blocks, pin types, themes, build
//! modes, icons) and the QuickJS sandbox that reads block files.

pub mod engine;
pub mod manifest;
pub mod mode;
pub mod registry;
pub mod theme;
pub mod ts;
pub mod types_def;

pub use engine::BlockEngine;
pub use manifest::PluginManifest;
pub use mode::{BuildMode, Category};
pub use registry::{Problem, Registry};
pub use theme::ThemeDef;
pub use types_def::PinTypeDef;
