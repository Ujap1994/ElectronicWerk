//! JSON-based RPC dispatch.
//!
//! A request is a JSON object `{"method": "...", "params": ...}`. The reply is
//! either `{"result": ...}` or `{"error": {"code": "...", "message": "..."}}`.
//! Request IDs and transport are the caller's business (Electron, CLI, tests).

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::info::EngineInfo;

/// A single engine request.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    /// Fully qualified method name, for example `engine.info`.
    pub method: String,
    /// Method parameters; `null` when omitted.
    #[serde(default)]
    pub params: Value,
}

/// Machine-readable error category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ErrorCode {
    /// The request was not valid JSON or did not match the request schema.
    InvalidRequest,
    /// No method with the given name exists.
    MethodNotFound,
    /// The parameters did not match what the method expects.
    InvalidParams,
}

/// Error returned to the caller.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RpcError {
    /// Error category.
    pub code: ErrorCode,
    /// Human-readable description (English, for logs and developers).
    pub message: String,
}

impl RpcError {
    fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

/// Reply to a [`Request`].
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Response {
    /// The method succeeded.
    Result(Value),
    /// The method failed.
    Error(RpcError),
}

/// All methods the engine understands, in alphabetical order.
pub const METHODS: &[&str] = &["engine.info", "engine.methods", "engine.ping"];

/// Executes a parsed request.
#[must_use]
pub fn handle(request: &Request) -> Response {
    match request.method.as_str() {
        "engine.info" => to_result(&EngineInfo::current()),
        "engine.methods" => to_result(&METHODS),
        "engine.ping" => Response::Result(serde_json::json!({
            "pong": true,
            "echo": request.params,
        })),
        other => Response::Error(RpcError::new(
            ErrorCode::MethodNotFound,
            format!("unknown method `{other}`"),
        )),
    }
}

/// Parses a JSON request, executes it and returns the JSON reply.
///
/// This function never panics on bad input; malformed requests produce an
/// `invalidRequest` error reply.
#[must_use]
pub fn dispatch(request_json: &str) -> String {
    let response = match serde_json::from_str::<Request>(request_json) {
        Ok(request) => handle(&request),
        Err(err) => Response::Error(RpcError::new(
            ErrorCode::InvalidRequest,
            format!("malformed request: {err}"),
        )),
    };
    encode(&response)
}

fn to_result<T: Serialize>(value: &T) -> Response {
    match serde_json::to_value(value) {
        Ok(value) => Response::Result(value),
        Err(err) => Response::Error(RpcError::new(
            ErrorCode::InvalidParams,
            format!("could not encode result: {err}"),
        )),
    }
}

fn encode(response: &Response) -> String {
    // Serializing these plain types cannot fail; fall back to a static error
    // instead of panicking so the host process always gets valid JSON.
    serde_json::to_string(response).unwrap_or_else(|_| {
        r#"{"error":{"code":"invalidRequest","message":"could not encode response"}}"#.to_owned()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn call(raw: &str) -> Value {
        serde_json::from_str(&dispatch(raw)).unwrap()
    }

    #[test]
    fn info_returns_version() {
        let reply = call(r#"{"method":"engine.info"}"#);
        assert_eq!(reply["result"]["version"], crate::VERSION);
    }

    #[test]
    fn ping_echoes_params() {
        let reply = call(r#"{"method":"engine.ping","params":{"n":42}}"#);
        assert_eq!(reply["result"], json!({"pong": true, "echo": {"n": 42}}));
    }

    #[test]
    fn ping_without_params_echoes_null() {
        let reply = call(r#"{"method":"engine.ping"}"#);
        assert_eq!(reply["result"]["echo"], Value::Null);
    }

    #[test]
    fn methods_lists_every_method_sorted() {
        let reply = call(r#"{"method":"engine.methods"}"#);
        let listed: Vec<String> = serde_json::from_value(reply["result"].clone()).unwrap();
        let mut sorted = listed.clone();
        sorted.sort();
        assert_eq!(listed, sorted);
        assert_eq!(listed, METHODS);
    }

    #[test]
    fn unknown_method_is_reported() {
        let reply = call(r#"{"method":"does.not.exist"}"#);
        assert_eq!(reply["error"]["code"], "methodNotFound");
        assert!(
            reply["error"]["message"]
                .as_str()
                .unwrap()
                .contains("does.not.exist")
        );
    }

    #[test]
    fn malformed_json_is_reported() {
        let reply = call("{not json");
        assert_eq!(reply["error"]["code"], "invalidRequest");
    }

    #[test]
    fn unknown_fields_are_rejected() {
        let reply = call(r#"{"method":"engine.info","extra":1}"#);
        assert_eq!(reply["error"]["code"], "invalidRequest");
    }

    #[test]
    fn every_listed_method_is_handled() {
        for method in METHODS {
            let request = Request {
                method: (*method).to_owned(),
                params: Value::Null,
            };
            assert!(
                matches!(handle(&request), Response::Result(_)),
                "{method} is listed but not handled"
            );
        }
    }
}
