//! `ewerk`: the ElectronicWerk command-line interface.
//!
//! Every engine method is reachable from the terminal, so scripts and AI
//! agents can drive ElectronicWerk without the desktop app.

mod commands;
mod library;

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
    /// Import KiCad projects, libraries and netlists.
    #[command(subcommand)]
    Import(ImportCommand),
    /// Build, search and distribute the base library.
    #[command(subcommand)]
    Library(LibraryCommand),
    /// Generate manufacturing outputs (Gerber, drill, BOM, prints, …).
    #[command(subcommand)]
    Outputs(OutputsCommand),
    /// Export the 3D model of a board (STEP, GLB or VRML).
    #[command(name = "export-3d")]
    Export3d(Export3dArgs),
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

/// Arguments of `export-3d`.
#[derive(Debug, Args)]
struct Export3dArgs {
    /// Project folder or `.ewproj` file.
    path: PathBuf,
    /// Output file (default: `<board>.<format>` in the current folder).
    #[arg(short, long)]
    output: Option<PathBuf>,
    /// `step`, `glb` or `vrml`.
    #[arg(long, default_value = "step")]
    format: String,
    /// Board name or id (default: the first board).
    #[arg(long)]
    board: Option<String>,
    /// Origin: `drill`, `center` or `absolute`.
    #[arg(long, default_value = "drill")]
    origin: String,
    /// Also export parts marked "do not populate".
    #[arg(long)]
    include_dnp: bool,
    /// Units of VRML files: `mm`, `m` or `inch`.
    #[arg(long, default_value = "mm")]
    units: String,
    /// Print the report as JSON.
    #[arg(long)]
    json: bool,
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

#[derive(Debug, Subcommand)]
enum OutputsCommand {
    /// Run an output job of a project (exit code 1 if an output fails).
    Run {
        /// Project folder or `.ewproj` file.
        path: PathBuf,
        /// Job name or id (default: the first job of the project).
        #[arg(long)]
        job: Option<String>,
        /// Only these outputs (names or ids; repeat the option).
        #[arg(long = "output")]
        outputs: Vec<String>,
        /// Print the report as JSON.
        #[arg(long)]
        json: bool,
    },
    /// List the output jobs of a project and their outputs.
    List {
        /// Project folder or `.ewproj` file.
        path: PathBuf,
        /// Print machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// List the presets for board makers.
    Presets {
        /// Print machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum ImportCommand {
    /// Import a KiCad project as a new ElectronicWerk project.
    Kicad {
        /// The `.kicad_pro` file (or its schematic, board or folder).
        source: PathBuf,
        /// Folder for the new project (must not exist or be empty).
        dir: PathBuf,
        /// Project name (default: the KiCad project name).
        #[arg(long)]
        name: Option<String>,
        /// Print the report as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Add the symbols and footprints of a KiCad library to a project's library.
    Library {
        /// `.kicad_sym`, `.kicad_symdir`, `.kicad_mod` or `.pretty`.
        source: PathBuf,
        /// Project folder or `.ewproj` file.
        #[arg(long)]
        project: PathBuf,
        /// Prefix of the names (default: the library file name).
        #[arg(long)]
        nickname: Option<String>,
        /// Print the result as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Compare a netlist with a project's board, or apply it (`--apply`).
    Netlist {
        /// KiCad `.net`, XML or CSV netlist.
        source: PathBuf,
        /// Project folder or `.ewproj` file.
        #[arg(long)]
        project: PathBuf,
        /// Assign the pads to the nets of the netlist and save the project.
        #[arg(long)]
        apply: bool,
        /// Print the result as JSON.
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum LibraryCommand {
    /// Build the library from KiCad libraries (replaces the current index).
    Build {
        /// A KiCad installation or a folder with the library repositories
        /// (`kicad-symbols`, `kicad-footprints`, `kicad-packages3D`).
        #[arg(long)]
        kicad: Option<PathBuf>,
        /// Folder with `.kicad_sym` files.
        #[arg(long)]
        symbols: Option<PathBuf>,
        /// Folder with `.pretty` footprint libraries.
        #[arg(long)]
        footprints: Option<PathBuf>,
        /// Folder with `.3dshapes` model libraries.
        #[arg(long)]
        models: Option<PathBuf>,
        /// Folder with tessellated meshes (`<Library>.3dshapes/<name>.glb`).
        #[arg(long)]
        meshes: Option<PathBuf>,
        /// Version of the libraries (for example `10.0.0`).
        #[arg(long, default_value = "")]
        version: String,
        /// License text of the library data, written as `LICENSE.md`.
        #[arg(long)]
        license_file: Option<PathBuf>,
        #[command(flatten)]
        target: LibraryDir,
    },
    /// Show what the library contains.
    Info {
        #[command(flatten)]
        target: LibraryDir,
    },
    /// Search the library.
    Search {
        /// Search text.
        text: String,
        /// Only `symbol`, `footprint` or `model`.
        #[arg(long)]
        kind: Option<String>,
        /// Only this library.
        #[arg(long)]
        library: Option<String>,
        /// Number of results.
        #[arg(long, default_value_t = 20)]
        limit: u32,
        #[command(flatten)]
        target: LibraryDir,
    },
    /// Print the deduplication report.
    Duplicates {
        /// Markdown instead of JSON.
        #[arg(long)]
        markdown: bool,
        #[command(flatten)]
        target: LibraryDir,
    },
    /// Write the library packs and their manifest (for a library release).
    Pack {
        /// Output folder.
        out: PathBuf,
        /// Start of every file name.
        #[arg(long, default_value = "ewerk-library")]
        prefix: String,
        /// Name of the release.
        #[arg(long, default_value = "KiCad")]
        name: String,
        /// Version of the release.
        #[arg(long, default_value = "")]
        version: String,
        #[command(flatten)]
        target: LibraryDir,
    },
    /// Install a library pack (index or meshes).
    Install {
        /// The pack (`.zip`).
        pack: PathBuf,
        #[command(flatten)]
        target: LibraryDir,
    },
}

/// Common options of the library commands.
#[derive(Debug, Args)]
struct LibraryDir {
    /// Library folder (default: the app's library folder or `EW_LIBRARY_DIR`).
    #[arg(long)]
    dir: Option<PathBuf>,
    /// Print machine-readable JSON.
    #[arg(long)]
    json: bool,
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
