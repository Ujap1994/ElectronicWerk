//! Node-API bridge for the ElectronicWerk desktop app.
//!
//! The desktop app loads this library inside an Electron `utilityProcess`, so
//! a crash in native code never takes the user interface down with it. The
//! engine (and the open project) lives as long as that process.

// The `#[napi]` macro generates the FFI glue, which needs `unsafe` blocks.
#![allow(unsafe_code)]

use std::sync::{LazyLock, Mutex, MutexGuard, PoisonError};

use ew_core::Engine;
use napi::bindgen_prelude::Buffer;
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

/// Reply of [`dispatch_binary`]: the JSON reply plus an optional binary
/// buffer (the primitives of `view.scene`).
#[napi(object)]
pub struct BinaryReply {
    /// The JSON reply.
    pub reply: String,
    /// The binary buffer, if the method produced one.
    pub data: Option<Buffer>,
}

/// Like [`dispatch`], but returns the buffer of `view.scene` as a Node.js
/// `Buffer` instead of base64 inside the JSON.
#[napi]
#[must_use]
#[allow(clippy::needless_pass_by_value)] // Node-API hands over an owned string.
pub fn dispatch_binary(request: String) -> BinaryReply {
    let (reply, data) = engine().dispatch_binary(&request);
    BinaryReply {
        reply,
        data: data.map(Buffer::from),
    }
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
