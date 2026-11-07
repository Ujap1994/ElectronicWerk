//! ElectronicWerk engine core.
//!
//! The engine owns all design data and exposes every operation as a command
//! through a single RPC entry point ([`rpc::dispatch`]). The desktop app, the
//! `ewerk` CLI and automated tests all talk to the engine through this API.

pub mod info;
pub mod rpc;

/// Version of the engine crate (identical to the workspace version).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
