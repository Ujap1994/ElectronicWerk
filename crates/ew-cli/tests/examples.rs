//! The examples of the scripting cookbook (`examples/automation`) run as
//! they are documented: as an ewerk script, a shell script and a Python
//! program.

#![allow(clippy::unwrap_used, reason = "test code")]

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn examples() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/automation")
}

fn ewerk() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ewerk"));
    command.env("EW_LIBRARY_DIR", std::env::temp_dir().join("ew-no-library"));
    command
}

/// The routed board of a project.
fn board(project: &Path) -> Value {
    let output = ewerk()
        .args(["board", "info", project.to_str().unwrap(), "--json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn the_script_example_routes_the_board() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("Status LED");
    let status = ewerk()
        .args([
            "project",
            "new",
            project.to_str().unwrap(),
            "--width",
            "30",
            "--height",
            "20",
        ])
        .status()
        .unwrap();
    assert!(status.success());
    let output = ewerk()
        .args(["script"])
        .arg(examples().join("status-led.jsonl"))
        .args(["--project", project.to_str().unwrap()])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "{text}");
    assert!(text.contains("11 steps, 0 failed, project saved"), "{text}");
    let info = board(&project);
    assert_eq!(info["unrouted"], 0);
    assert_eq!(info["footprints"].as_array().unwrap().len(), 3);
}

/// `PATH` with the freshly built ewerk first.
#[cfg(unix)]
fn path_with_ewerk() -> std::ffi::OsString {
    let dir = Path::new(env!("CARGO_BIN_EXE_ewerk"))
        .parent()
        .unwrap()
        .to_path_buf();
    let mut paths = vec![dir];
    paths.extend(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    ));
    std::env::join_paths(paths).unwrap()
}

#[cfg(unix)]
#[test]
fn the_shell_example_makes_gerber_files() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("Status LED");
    let output = Command::new("sh")
        .arg(examples().join("status-led.sh"))
        .arg(&project)
        .env("PATH", path_with_ewerk())
        .env("EW_LIBRARY_DIR", std::env::temp_dir().join("ew-no-library"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("DRC: 0 errors"), "{text}");
    assert!(project.join("outputs").exists());
    assert_eq!(board(&project)["unrouted"], 0);
}

#[cfg(unix)]
#[test]
fn the_python_example_routes_the_board() {
    // Runners without Python skip this example.
    if Command::new("python3").arg("--version").output().is_err() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let output = Command::new("python3")
        .arg(examples().join("status-led.py"))
        .arg(dir.path())
        .env("EWERK", env!("CARGO_BIN_EXE_ewerk"))
        .env("EW_LIBRARY_DIR", std::env::temp_dir().join("ew-no-library"))
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "{text}{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(text.contains("ERC: 0 errors"), "{text}");
    assert!(text.contains("0 left"), "{text}");
    assert_eq!(board(&dir.path().join("Status LED"))["unrouted"], 0);
}
