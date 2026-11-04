//! Mode-independent model of an XC Circuit project: pin types, block
//! definitions, workspace graphs and the on-disk project format.

pub mod backup;
pub mod block;
pub mod graph;
pub mod project;
pub mod template;
pub mod types;

pub use block::{BlockDef, BlockKind, BlockShape, DefineResult, FieldDef, OutputDef, PinMode};
pub use graph::{CLIP_FORMAT, Clip, Flow, Link, Node, Note, PinRef, Workspace, Zone};
pub use project::{Project, ProjectMeta};
pub use types::{Compat, PinType, TypeInfo};
