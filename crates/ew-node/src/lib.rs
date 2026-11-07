//! Node-API bridge for the ElectronicWerk desktop app.
//!
//! The desktop app loads this library inside an Electron `utilityProcess`, so
//! a crash in native code never takes the user interface down with it.

// The `#[napi]` macro generates the FFI glue, which needs `unsafe` blocks.
#![allow(unsafe_code)]

use napi_derive::napi;

/// Executes one engine request (`{"method": ..., "params": ...}`) and returns
/// the JSON reply.
#[napi]
#[must_use]
#[allow(clippy::needless_pass_by_value)] // Node-API hands over an owned string.
pub fn dispatch(request: String) -> String {
    ew_core::rpc::dispatch(&request)
}

/// Returns the engine version.
#[napi]
#[must_use]
pub fn engine_version() -> String {
    ew_core::VERSION.to_owned()
}
