//! `ewerk`: the ElectronicWerk command-line interface.
//!
//! Every engine method is reachable from the terminal, so scripts and AI
//! agents can drive ElectronicWerk without the desktop app.

use std::io::Write as _;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use ew_core::info::EngineInfo;

/// ElectronicWerk command-line interface.
#[derive(Debug, Parser)]
#[command(name = "ewerk", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Show information about the engine build.
    Info {
        /// Print machine-readable JSON instead of text.
        #[arg(long)]
        json: bool,
    },
    /// Call an engine method directly and print the JSON reply.
    Call {
        /// Method name, for example `engine.ping`.
        method: String,
        /// Parameters as a JSON value (default: `null`).
        #[arg(default_value = "null")]
        params: String,
    },
    /// List all engine methods.
    Methods,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let mut out = std::io::stdout().lock();
    let result = run(cli.command, &mut out);
    match result {
        Ok(code) => code,
        Err(err) => {
            let _ = writeln!(std::io::stderr().lock(), "ewerk: {err}");
            ExitCode::FAILURE
        }
    }
}

fn run(command: Command, out: &mut impl std::io::Write) -> std::io::Result<ExitCode> {
    match command {
        Command::Info { json } => {
            let info = EngineInfo::current();
            if json {
                writeln!(out, "{}", serde_json::to_string_pretty(&info)?)?;
            } else {
                writeln!(out, "{} {}", info.name, info.version)?;
                writeln!(out, "Target:  {}-{}", info.os, info.arch)?;
                writeln!(out, "Threads: {}", info.threads)?;
                writeln!(
                    out,
                    "Build:   {}",
                    if info.debug { "debug" } else { "release" }
                )?;
            }
            Ok(ExitCode::SUCCESS)
        }
        Command::Call { method, params } => {
            let params: serde_json::Value = serde_json::from_str(&params).map_err(|err| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("params are not valid JSON: {err}"),
                )
            })?;
            let request = serde_json::json!({ "method": method, "params": params });
            let reply = ew_core::rpc::dispatch(&request.to_string());
            writeln!(out, "{reply}")?;
            let failed = serde_json::from_str::<serde_json::Value>(&reply)
                .map_or(true, |value| value.get("error").is_some());
            Ok(if failed {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            })
        }
        Command::Methods => {
            for method in ew_core::rpc::METHODS {
                writeln!(out, "{method}")?;
            }
            Ok(ExitCode::SUCCESS)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory as _;

    fn run_to_string(command: Command) -> (ExitCode, String) {
        let mut buffer = Vec::new();
        let code = run(command, &mut buffer).unwrap();
        (code, String::from_utf8(buffer).unwrap())
    }

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn info_text_contains_version() {
        let (code, output) = run_to_string(Command::Info { json: false });
        assert_eq!(code, ExitCode::SUCCESS);
        assert!(output.contains(ew_core::VERSION));
    }

    #[test]
    fn info_json_is_valid_json() {
        let (_, output) = run_to_string(Command::Info { json: true });
        let value: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(value["version"], ew_core::VERSION);
    }

    #[test]
    fn call_ping_succeeds() {
        let (code, output) = run_to_string(Command::Call {
            method: "engine.ping".into(),
            params: "[1,2]".into(),
        });
        assert_eq!(code, ExitCode::SUCCESS);
        assert!(output.contains(r#""echo":[1,2]"#));
    }

    #[test]
    fn call_unknown_method_fails() {
        let (code, output) = run_to_string(Command::Call {
            method: "nope".into(),
            params: "null".into(),
        });
        assert_eq!(code, ExitCode::FAILURE);
        assert!(output.contains("methodNotFound"));
    }

    #[test]
    fn call_with_invalid_params_is_an_error() {
        let mut buffer = Vec::new();
        let err = run(
            Command::Call {
                method: "engine.ping".into(),
                params: "{".into(),
            },
            &mut buffer,
        )
        .unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    }

    #[test]
    fn methods_lists_all() {
        let (_, output) = run_to_string(Command::Methods);
        assert_eq!(output.lines().count(), ew_core::rpc::METHODS.len());
    }
}
