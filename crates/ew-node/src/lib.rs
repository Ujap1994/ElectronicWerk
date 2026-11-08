//! Node-API bridge for the ElectronicWerk desktop app.
//!
//! The desktop app loads this library inside an Electron `utilityProcess`, so
//! a crash in native code never takes the user interface down with it. The
//! engine (and the open project) lives as long as that process.

// The `#[napi]` macro generates the FFI glue, which needs `unsafe` blocks.
#![allow(unsafe_code)]

use std::sync::{LazyLock, Mutex, MutexGuard, PoisonError};

use ew_core::Engine;
use napi_derive::napi;

static ENGINE: LazyLock<Mutex<Engine>> = LazyLock::new(|| Mutex::new(Engine::new()));

fn engine() -> MutexGuard<'static, Engine> {
    ENGINE.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Executes one engine request (`{"method": ..., "params": ...}`) and returns
/// the JSON reply.
#[napi]
#[must_use]
#[allow(clippy::needless_pass_by_value)] // Node-API hands over an owned string.
pub fn dispatch(request: String) -> String {
    engine().dispatch(&request)
}

/// Returns the events produced since the last call as a JSON array.
#[napi]
#[must_use]
pub fn take_events() -> String {
    let events = engine().take_events();
    serde_json::to_string(&events).unwrap_or_else(|_| "[]".to_owned())
}

/// Returns the engine version.
#[napi]
#[must_use]
pub fn engine_version() -> String {
    ew_core::VERSION.to_owned()
}
