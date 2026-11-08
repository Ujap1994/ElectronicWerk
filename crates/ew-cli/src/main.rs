//! `ewerk`: the ElectronicWerk command-line interface.
//!
//! Every engine method is reachable from the terminal, so scripts and AI
//! agents can drive ElectronicWerk without the desktop app.

mod commands;

use std::io::Write as _;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand};

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
    /// Call an engine method once and print the JSON reply.
    Call {
        /// Method name, for example `engine.ping`.
        method: String,
        /// Parameters as a JSON value (default: `null`).
        #[arg(default_value = "null")]
        params: String,
    },
    /// List all engine methods.
    Methods,
    /// Run a session: read one JSON request per line from standard input and
    /// write one JSON reply per line.
    Rpc {
        /// Also write engine events (`{"event": …}` lines).
        #[arg(long)]
        events: bool,
        /// Disable the autosave journal.
        #[arg(long)]
        no_journal: bool,
    },
    /// Create, inspect, check and archive projects.
    #[command(subcommand)]
    Project(ProjectCommand),
    /// Generate a synthetic reference design (for tests and benchmarks).
    Synth {
        /// Size class: S, M, L or XL.
        size: String,
        /// Target folder (must not exist or be empty).
        dir: PathBuf,
        /// Random seed.
        #[arg(long, default_value_t = ew_synth::DEFAULT_SEED)]
        seed: u64,
        /// Print machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// Measure how long opening and saving a synthetic design takes.
    Bench {
        /// Size class: S, M, L or XL.
        #[arg(long, default_value = "L")]
        size: String,
        /// Print machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum ProjectCommand {
    /// Create a new project from a template.
    New(NewArgs),
    /// Show a summary of a project.
    Info {
        /// Project folder or `.ewproj` file.
        path: PathBuf,
        /// Print machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// Check a project for errors (exit code 1 if there are any).
    Validate {
        /// Project folder or `.ewproj` file.
        path: PathBuf,
        /// Print machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// Write a ZIP archive of a project.
    Archive {
        /// Project folder or `.ewproj` file.
        path: PathBuf,
        /// Archive file (default: `<name>.zip` next to the project folder).
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Rewrite every file in the current format (older files are backed up).
    Upgrade {
        /// Project folder or `.ewproj` file.
        path: PathBuf,
    },
    /// List the project templates.
    Templates,
}

#[derive(Debug, Args)]
struct NewArgs {
    /// Folder to create (must not exist or be empty).
    dir: PathBuf,
    /// Project name (default: the folder name).
    #[arg(long)]
    name: Option<String>,
    /// Template id (see `ewerk project templates`).
    #[arg(long, default_value = "board-2l")]
    template: String,
    /// Board width in millimetres.
    #[arg(long)]
    width: Option<f64>,
    /// Board height in millimetres.
    #[arg(long)]
    height: Option<f64>,
    /// Number of copper layers.
    #[arg(long)]
    layers: Option<u8>,
    /// Board thickness in millimetres.
    #[arg(long)]
    thickness: Option<f64>,
    /// Display units: mm, mil or inch.
    #[arg(long, default_value = "mm")]
    units: String,
    /// Print machine-readable JSON.
    #[arg(long)]
    json: bool,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let mut out = std::io::stdout().lock();
    match commands::run(cli.command, &mut out) {
        Ok(code) => code,
        Err(err) => {
            let _ = out.flush();
            let _ = writeln!(std::io::stderr().lock(), "ewerk: {err}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory as _;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }
}
