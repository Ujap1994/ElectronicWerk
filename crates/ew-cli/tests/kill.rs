//! Kill test: a real `ewerk rpc` session is killed hard (SIGKILL / process
//! termination) in the middle of editing. Reopening the project must offer
//! the unsaved work, and recovering it must restore every edit.

#![allow(
    clippy::unwrap_used,
    clippy::needless_pass_by_value,
    reason = "test helpers may panic and take owned JSON for brevity"
)]

use std::io::{BufRead as _, BufReader, Write as _};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{Value, json};

struct Session {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next: u64,
}

impl Session {
    fn start() -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_ewerk"))
            .arg("rpc")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        Self {
            child,
            stdin,
            stdout,
            next: 1,
        }
    }

    fn call(&mut self, method: &str, params: Value) -> Value {
        let id = self.next;
        self.next += 1;
        let request = json!({"id": id, "method": method, "params": params});
        writeln!(self.stdin, "{request}").unwrap();
        self.stdin.flush().unwrap();
        let mut line = String::new();
        self.stdout.read_line(&mut line).unwrap();
        let reply: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(reply["id"], id);
        assert!(reply.get("error").is_none(), "{method}: {reply}");
        reply["result"].clone()
    }
}

#[test]
fn killed_session_is_recovered() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("kill-test");

    let mut session = Session::start();
    let snapshot = session.call(
        "project.create",
        json!({"path": path, "name": "Kill test", "template": "board-2l"}),
    );
    let root = snapshot["rootSchematic"].as_str().unwrap().to_owned();
    for index in 0..25 {
        session.call(
            "edit.apply",
            json!({"label": "add", "changes": [{"op": "insert", "doc": root, "object": {"type": "wire", "start": [index * 1000, 0], "end": [index * 1000, 5000]}}]}),
        );
    }
    session.call("project.setProperties", json!({"revision": "K"}));
    session.call("edit.undo", Value::Null);
    session.call("edit.undo", Value::Null);
    let expected = session.call("document.get", json!({"doc": root}));

    // Hard kill: no chance to save or clean up.
    session.child.kill().unwrap();
    session.child.wait().unwrap();

    let mut session = Session::start();
    let probe = session.call("project.probe", json!({"path": path}));
    assert_eq!(probe["hasRecovery"], true);
    let opened = session.call("project.open", json!({"path": path}));
    assert_eq!(opened["recovery"]["commits"], 26);
    let recovered = session.call("project.recover", Value::Null);
    assert_eq!(recovered["revision"], "A", "the property change was undone");
    assert_eq!(session.call("document.get", json!({"doc": root})), expected);
    let history = session.call("edit.history", Value::Null);
    assert_eq!(history["undo"].as_array().unwrap().len(), 24);
    assert_eq!(history["redo"].as_array().unwrap().len(), 2);
    session.call("project.save", Value::Null);
    session.call("project.close", Value::Null);
    drop(session.stdin);
    assert!(session.child.wait().unwrap().success());
    assert!(!path.join(".ewerk/journal.jsonl").exists());
}
