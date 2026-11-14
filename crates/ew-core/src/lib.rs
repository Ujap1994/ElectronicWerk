//! ElectronicWerk engine core.
//!
//! The engine owns all design data and exposes every operation as a method
//! through one RPC entry point ([`rpc::Engine::dispatch`]). The desktop app,
//! the `ewerk` CLI and automated tests all talk to the engine through this
//! API.
//!
//! - [`session`]: an open project with transactions, undo/redo, saving and
//!   crash recovery.
//! - [`change`]: reversible changes and edit requests.
//! - [`history`]: undo/redo stacks with a memory limit.
//! - [`journal`]: the autosave journal.
//! - [`templates`]: new-project templates.
//! - [`import`]: KiCad projects, libraries and netlists.
//! - [`summary`]: project snapshots for clients.
//! - [`schematic`]: the schematic editor and ERC methods.
//! - [`libedit`]: the symbol, footprint and component editors.

pub mod board3d;
pub mod change;
pub mod design;
pub mod docs;
pub mod downloads;
pub mod error;
pub mod history;
pub mod import;
pub mod info;
pub mod journal;
pub mod libedit;
pub mod library;
pub mod outputs;
pub mod pcb;
pub mod rpc;
pub mod schematic;
pub mod session;
pub mod summary;
pub mod templates;

pub use error::{EngineError, ErrorCode};
pub use rpc::Engine;

/// Version of the engine crate (identical to the workspace version).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
