# Command-line tool (`ewerk`)

`ewerk` is ElectronicWerk's command-line interface. It contains the same engine
as the desktop app, so everything the engine can do is available in a
terminal, in scripts, in CI pipelines and to AI agents. The command set grows
with every milestone.

## Installation

Download the archive for your system from the
[latest release](https://github.com/Ujap1994/ElectronicWerk/releases/latest):

| System              | Archive                                                     |
| ------------------- | ----------------------------------------------------------- |
| Windows x64 / arm64 | `ewerk-<version>-win-x64.zip` / `…-win-arm64.zip`           |
| macOS Apple Silicon | `ewerk-<version>-mac-arm64.tar.gz`                          |
| macOS Intel         | `ewerk-<version>-mac-x64.tar.gz`                            |
| Linux x64 / arm64   | `ewerk-<version>-linux-x64.tar.gz` / `…-linux-arm64.tar.gz` |

Unpack it and move `ewerk` (`ewerk.exe` on Windows) to a folder on your `PATH`,
for example:

```sh
tar -xzf ewerk-0.2.0-linux-x64.tar.gz
sudo mv ewerk-0.2.0-linux-x64/ewerk /usr/local/bin/
ewerk --version
```

::: tip macOS
macOS quarantines downloaded programs. Run
`xattr -d com.apple.quarantine ewerk` once after unpacking.
:::

## Overview

| Command                                                 | Purpose                                              |
| ------------------------------------------------------- | ---------------------------------------------------- |
| [`ewerk project new`](#ewerk-project-new)               | Create a project from a template                     |
| [`ewerk project info`](#ewerk-project-info)             | Show a summary of a project                          |
| [`ewerk project validate`](#ewerk-project-validate)     | Check a project for errors                           |
| [`ewerk project archive`](#ewerk-project-archive)       | Write a ZIP archive of a project                     |
| [`ewerk project upgrade`](#ewerk-project-upgrade)       | Rewrite a project in the current file format         |
| [`ewerk project templates`](#ewerk-project-templates)   | List the project templates                           |
| [`ewerk schematic …`](#schematics)                      | Place parts, connect pins, run the ERC               |
| [`ewerk board …`](#boards)                              | Update, place, route and check a board               |
| [`ewerk script`](#ewerk-script)                         | Run engine calls from a file                         |
| [`ewerk mcp`](#ewerk-mcp)                               | MCP server for AI agents                             |
| [`ewerk import kicad`](#ewerk-import-kicad)             | Import a KiCad project as a new project              |
| [`ewerk import library`](#ewerk-import-library)         | Add a KiCad library to a project library             |
| [`ewerk import archive`](#ewerk-import-archive)         | Import a SnapEDA or Ultra Librarian download         |
| [`ewerk import netlist`](#ewerk-import-netlist)         | Compare a netlist with a board, or apply it          |
| [`ewerk outputs run`](#ewerk-outputs-run)               | Generate the manufacturing files of an output job    |
| [`ewerk outputs list`](#ewerk-outputs-list)             | List the output jobs of a project                    |
| [`ewerk outputs presets`](#ewerk-outputs-presets)       | List the presets for board makers                    |
| [`ewerk export-3d`](#ewerk-export-3d)                   | Export a board as a STEP, GLB or VRML model          |
| [`ewerk library build`](#ewerk-library-build)           | Build the base library from KiCad libraries          |
| [`ewerk library info`](#ewerk-library-info)             | Show what the base library contains                  |
| [`ewerk library search`](#ewerk-library-search)         | Search the base library                              |
| [`ewerk library duplicates`](#ewerk-library-duplicates) | Print the deduplication report                       |
| [`ewerk library pack`](#ewerk-library-pack)             | Write library packs for a library release            |
| [`ewerk library install`](#ewerk-library-install)       | Install a library pack                               |
| [`ewerk rpc`](#ewerk-rpc)                               | Run an engine session over standard input and output |
| [`ewerk call`](#ewerk-call-method-params)               | Call one engine method                               |
| [`ewerk methods`](#ewerk-methods)                       | List every engine method (with summaries)            |
| [`ewerk synth`](#ewerk-synth)                           | Generate a synthetic reference design                |
| [`ewerk bench`](#ewerk-bench)                           | Measure opening and saving a synthetic design        |
| [`ewerk info`](#ewerk-info)                             | Show information about the engine build              |

Global options: `-h`, `--help` (also per command, for example
`ewerk project new --help`) and `-V`, `--version`.

## Projects

`<project>` is a project folder or its `.ewproj` file.

### `ewerk project new`

```text
ewerk project new <dir> [--name <name>] [--template <id>] [--width <mm>] [--height <mm>]
                        [--layers <n>] [--thickness <mm>] [--units mm|mil|inch] [--json]
```

Creates the folder `<dir>` (it must not exist or be empty) with a new project.
The name defaults to the folder name, the template to `board-2l`.

```console
$ ewerk project new "LED Blinker" --template board-2l --width 80 --height 50
Created /home/lab/LED Blinker/LED Blinker.ewproj
Project:   LED Blinker
Revision:  A
Folder:    /home/lab/LED Blinker
Documents:
  schematic  schematics/root.ewsch              0 symbols, 0 wires
  pcb        pcb/led-blinker.ewpcb              80.000 mm × 50.000 mm, 2 layers, 0 footprints, 0 tracks, 0 vias
  library    library/project.ewlib              0 symbols, 0 footprints
  rules      rules/design.ewrules               2 net classes
  outputJob  outputs/fabrication.ewout          6 outputs
```

The templates are described in [Projects](../guide/projects#step-2-template).

### `ewerk project info`

```text
ewerk project info <project> [--json]
```

Prints the summary shown above. With `--json`, the complete project snapshot
(the result of the engine method `project.open`) is printed. If the project
has unsaved work from a session that ended unexpectedly, a note says so.

### `ewerk project validate`

```text
ewerk project validate <project> [--json]
```

Checks every file and reference and lists the findings. The exit code is `1`
if there are errors, so the command fits CI pipelines:

```console
$ ewerk project validate "LED Blinker"
5 documents, 1 objects checked: 0 errors, 0 warnings.

$ ewerk project validate Broken
error   rules/design.ewrules: …: expected value at line 1 column 2 [unreadable-document]
warning schematics/stray.ewsch: `schematics/stray.ewsch` is not part of the project [unlisted-file]
3 documents, 0 objects checked: 1 errors, 1 warnings.
```

| Code (examples)             | Severity | Meaning                                                |
| --------------------------- | -------- | ------------------------------------------------------ |
| `unreadable-document`       | error    | A file is missing, unreadable or not valid             |
| `duplicate-id`              | error    | Two objects share an id                                |
| `sheet-cycle`               | error    | A sheet includes itself (directly or indirectly)       |
| `sheet-target-missing`      | error    | A sheet symbol refers to a missing schematic           |
| `unknown-net`               | error    | A track, via, zone or pad refers to a missing net      |
| `layer-not-in-stackup`      | error    | An object is on a copper layer the board does not have |
| `invalid-via`               | error    | A via's drill is not smaller than its diameter         |
| `coordinate-out-of-range`   | error    | A coordinate lies outside ±10 m                        |
| `symbol-not-in-library`     | warning  | A placed symbol is not in a project library            |
| `no-board-outline`          | warning  | The board has no outline                               |
| `builtin-variable-shadowed` | warning  | A user variable uses a built-in name                   |
| `unlisted-file`             | warning  | A native file in the folder is not part of the project |
| `unplaced-sheet`            | info     | A sheet is not placed in the hierarchy                 |

### `ewerk project archive`

```text
ewerk project archive <project> [--output <file.zip>]
```

Writes a ZIP archive with one folder containing the project file, every
document and the other files of the project folder (without `.ewerk/` and
generated outputs). The default output is `<name>.zip` next to the project
folder.

```console
$ ewerk project archive "LED Blinker"
Archived 7 files (2945 bytes) to /home/lab/LED Blinker.zip
```

### `ewerk project upgrade`

```text
ewerk project upgrade <project>
```

Rewrites every file in the current file format. Files from older versions are
backed up to `.ewerk/backup/schema-v<N>/` first.

```console
$ ewerk project upgrade "LED Blinker"
Already current; rewrote 6 files in schema version 1.
```

### `ewerk project templates`

```console
$ ewerk project templates
empty            no board
board-2l         100.000 mm × 100.000 mm, 2 layers
board-4l         100.000 mm × 100.000 mm, 4 layers
arduino-shield   68.580 mm × 53.340 mm, 2 layers
demo             no board
```

## Schematics

The `schematic` commands edit a project's schematic with designators, pin
names and millimetres; see [Scripts and AI agents](../guide/automation). Each
command opens the project, makes its change, saves it and closes it. All of
them take `--json` for the full result.

### `ewerk schematic place`

```text
ewerk schematic place <project> <symbol> [--component] [--ref <designator>]
    [--value <text>] [--footprint <name>] [--sheet <name>] [--at <x,y>]
    [--rotate <degrees>] [--mirror] [--unit <n>]
```

Places a symbol of the project library (`R`, `LED`, …) or of the base
library (`Device:R`, copied into the project library first). With
`--component`, the name is a component of the project library (with its
value, footprint and part numbers).

| Option        | Meaning                                                                                                     |
| ------------- | ----------------------------------------------------------------------------------------------------------- |
| `--ref`       | Designator (default: the next free one, `R4`)                                                               |
| `--value`     | Value (default: the symbol's)                                                                               |
| `--footprint` | Project footprint, base library footprint, or wizard preset (`0805`, `SOIC-8`; `ewerk call design.presets`) |
| `--sheet`     | Sheet name (default: the top sheet)                                                                         |
| `--at`        | Position in millimetres, `x,y` (y up); snapped to the sheet grid. Default: a free place                     |
| `--rotate`    | Rotation in degrees, counter-clockwise                                                                      |

```console
$ ewerk schematic place "Status LED" R --value 330 --footprint 0805
Placed R1 (330) at [30.48,179.52] mm on sheet Root
```

### `ewerk schematic connect`

```text
ewerk schematic connect <project> <net> <pin>… [--style auto|label|global|power]
```

Connects pins to a net: a 2.54 mm wire from each pin and a net label at its
end (`auto`: a local label, a global label when the net spans sheets), or a
power port (`power`; nets starting with GND or VSS get the ground symbol). A
pin is `designator.pin` with the pin number or name: `R1.2`, `U1.VCC`; a name
shared by several pins (`U1.GND`) connects all of them. Pins on the net
already are skipped; a pin on another named net is an error.

```console
$ ewerk schematic connect "Status LED" LED_A R1.2 D1.A
Connected R1.2, D1.2 to LED_A
```

### `ewerk schematic set`

```text
ewerk schematic set <project> <designator> [--value <text>] [--footprint <name>]
    [--rename <designator>] [--property KEY=VALUE]…
```

Changes a part on every sheet (all its units). `--property MPN=…` sets a
property; an empty value (`--property MPN=`) removes it.

### `ewerk schematic list` / `nets`

`list` prints every part with its value, footprint and the net of each pin;
`nets` prints every net with its pins. Local nets carry the sheet path
(`/LED_A`); power and global nets do not.

```console
$ ewerk schematic list "Status LED"
C1       100n         Chip_0603_1608Metric         1=VCC 2=GND
D1       green        Chip_0805_2013Metric         1=GND 2=/LED_A
R1       330          Chip_0805_2013Metric         1=VCC 2=/LED_A
```

### `ewerk schematic no-connect`

```text
ewerk schematic no-connect <project> <pin>…
```

Puts no-connect flags on pins that stay open on purpose (the ERC then accepts
them).

### `ewerk schematic annotate`

Numbers the parts whose designator ends in `?`; `--all` numbers every part
again.

### `ewerk schematic erc`

Runs the [electrical rules check](../guide/erc). Exit code `1` if there are
errors.

```console
$ ewerk schematic erc "Status LED"
ERC: 0 errors, 0 warnings
```

### `ewerk schematic netlist`

```text
ewerk schematic netlist <project> [--format kicad|csv] [-o <file>]
```

Writes the netlist to standard output or a file.

## Boards

The `board` commands work on the first board of a project (`--board <name>`
for another one). Positions are millimetres, rotations degrees.

### `ewerk board update`

Brings the board up to date with the schematic, like
[Update board from schematic](../guide/schematic-to-board): new footprints
(below the board outline), nets, values and footprint changes.

### `ewerk board place`

```text
ewerk board place <project> <designator> --at <x,y> [--rotate <degrees>] [--side top|bottom]
```

Puts a footprint at an exact position, rotation and side.

### `ewerk board arrange`

```text
ewerk board arrange <project> [<designator>…] [--gap <mm>]
```

Puts footprints in rows inside the board outline, in designator order:
the given ones, or all that are not on the board yet.

### `ewerk board route`

```text
ewerk board route <project> [--net <name>]… [--layer top|bottom]… [--width <mm>]
```

Routes the unrouted connections one by one with the
[walkaround router](../guide/routing): 45° bends, one copper layer per
connection (in the order of `--layer`, default top then bottom), no vias,
the track width of the design rules (or `--width`). `--net LED_A` also finds
the local net `/LED_A`. Connections without a path are listed; the routing is
one undo step.

```console
$ ewerk board route "Status LED"
Routed 3 connections; 0 still unrouted
```

### `ewerk board info`

The board outline, copper layers, every footprint with position, rotation and
side, and the number of tracks, vias and unrouted connections.

### `ewerk board fill-zones` / `drc`

`fill-zones` fills the copper zones. `drc` runs the
[design rules check](../guide/drc); exit code `1` if there are errors.

```console
$ ewerk board drc "Status LED"
warning silk-width       Silkscreen line width 0.12 mm is less than 0.15 mm (board constraints)
…
DRC: 0 errors, 3 warnings, 0 unrouted
```

## Import

The import commands read KiCad files and netlists; they never change them.
See [Importing KiCad projects](../guide/import-kicad) and
[Netlists](../guide/netlists) for what is imported and how.

### `ewerk import kicad`

```text
ewerk import kicad <source> <dir> [--name <name>] [--json]
```

Imports the KiCad project `<source>` (the `.kicad_pro` file, its schematic or
board, or a folder with exactly one KiCad project) into the new project
folder `<dir>` (it must not exist or be empty). The name defaults to the name
of the KiCad project. The command prints the counts and every finding of the
import report, and writes the report into the project as
`kicad-import-report.md`:

```console
$ ewerk import kicad blinky/blinky.kicad_pro Blinky
Imported blinky/blinky.kicad_pro into /home/lab/Blinky
3 board graphics, 8 footprints, 10 labels, 1 library footprints, 2 library symbols, 7 nets, 22 pads, 3 sheet texts and graphics, 2 sheets, 17 symbols, 14 tracks, 2 vias, 19 wires, 1 zones
warning blinky.kicad_dru: 1 custom design rules are not imported; write them again as custom rules in the design rules [custom-rules]
info    sym-lib-table: the symbol library `Device` is outside the project; the symbols the design uses were imported from the design itself [external-library]
info    blinky.kicad_pcb: `dimension` items are not imported [unsupported-item]
Report: /home/lab/Blinky/kicad-import-report.md
```

With `--json`, the result of the engine method
[`import.kicad`](./engine-api#import-kicad) is printed: the project snapshot,
the report and the report file. The exit code is `1` if the report contains
errors (the project is created anyway).

### `ewerk import library`

```text
ewerk import library <source> --project <project> [--nickname <name>] [--json]
```

Adds the symbols and footprints of a KiCad library to the first library of
the project and saves it. `<source>` is a symbol library (`.kicad_sym`), a
KiCad 10 symbol library folder (`.kicad_symdir`), a footprint library folder
(`.pretty`) or a single footprint (`.kicad_mod`). The names get the prefix
`<nickname>:`, which defaults to the library's file name without extension.
Names that the library has already are skipped:

```console
$ ewerk import library ~/kicad/MyParts.pretty --project Blinky
Added 0 symbols and 12 footprints to the project library.
```

### `ewerk import archive`

```text
ewerk import archive <source> --project <project> [--nickname <name>] [--use-library] [--dry-run] [--json]
```

Imports a part downloaded from an [online part library](../guide/online-parts)
(SnapEDA, Ultra Librarian, or any KiCad ZIP; also a single `.kicad_sym`,
`.kicad_mod` or STEP file) into the first library of the project, copies its
3D models into `3d/<nickname>/` and saves the project. The names get the
prefix `<nickname>:` (default: the provider, else the file name).
`--use-library` takes the base library's footprint instead of a downloaded
one that the base library has already (identical, the same pads or pads
within 0.05 mm); `--dry-run` only lists the content:

```console
$ ewerk import archive ~/Downloads/ul_OPAMP-DEMO.zip --project Amp
ul_OPAMP-DEMO.zip (UltraLibrarian)
  symbol    UltraLibrarian:OPAMP-DEMO
  footprint UltraLibrarian:DBV0005A
  3D model  DBV0005A.step
Added 1 symbols, 1 footprints and 1 3D models (0 replaced by the base library, 0 already there).
```

With `--json` the result of [`import.archive`](./engine-api#import-archive)
is printed (with `--dry-run`, that of `import.archivePreview`).

### `ewerk import netlist`

```text
ewerk import netlist <source> --project <project> [--apply] [--json]
```

Compares the netlist `<source>` (KiCad `.net`, KiCad XML or CSV) with the
first board of the project. The exit code is `0` if the board connects
exactly like the netlist and `1` if it does not, so the command fits CI
pipelines. With `--apply`, the pads get the nets of the netlist, the project
is saved and the exit code is `0`:

```console
$ ewerk import netlist blinky/blinky.net --project Blinky
kicad netlist: 7 nets, 8 components. 7 of 7 connected nets match the board (7 with the same name).

$ ewerk import netlist changed.csv --project Blinky
csv netlist: 8 nets, 8 components. 6 of 6 connected nets match the board (6 with the same name).
  D1-2: netlist /LED_B (not connected) / board /LED_A
  R3-2: netlist /LED_A (not connected) / board /LED_A
Run with --apply to change 1 pads.
```

With `--json`, the result of the engine method
[`import.netlist`](./engine-api#import-netlist) is printed.

## Outputs

The output jobs of a project generate the manufacturing files: Gerber, drill,
drill map, bill of materials, placement, PDF and SVG prints, IPC-D-356,
IPC-2581, netlists and ZIP packages (see [Output jobs](../guide/outputs)).

### `ewerk outputs run`

```sh
ewerk outputs run <project> [--job <name or id>] [--output <name or id>]… [--json]
```

Runs an output job and writes its files into the job's output folder inside
the project (`outputs/generated` by default). Without `--job` the first job of
the project runs. `--output` (repeatable) runs only these outputs; the others
are reported as skipped.

```text
$ ewerk outputs run ~/Projects/Blinky
Fabrication → /Users/me/Projects/Blinky/outputs/generated
  done     Gerber                   10 files
  done     Drill                    2 files
  done     Drill map                1 file
  done     Bill of materials        1 file
  done     Pick and place           1 file
  done     IPC-D-356 netlist        1 file
  done     Schematic PDF            1 file
  done     Assembly drawing         1 file
  done     Fabrication package      1 file
```

The exit code is 1 when an output fails (for example a board output in a
project without a board). `--json` prints the report of
[`outputs.run`](./engine-api#outputs-run).

### `ewerk outputs list`

```sh
ewerk outputs list <project> [--json]
```

Lists the output jobs with their folder and outputs (`+` switched on, `-`
switched off).

### `ewerk outputs presets`

```sh
ewerk outputs presets [--json]
```

Lists the [presets for board makers](../guide/ordering#presets) with what they
contain.

## 3D export

### `ewerk export-3d`

```sh
ewerk export-3d <project> [-o <file>] [--format step|glb|vrml] [--board <name or id>]
                [--origin drill|center|absolute] [--include-dnp] [--units mm|m|inch] [--json]
```

Exports a board with its parts as a 3D model (see
[3D models: STEP, GLB and VRML](../guide/export-3d)). Without `--board` the
first board of the project is exported; without `-o` the file is written to
the current folder as `<board>.step` (`.glb`, `.wrl`). `--origin` chooses the
origin (the drill and placement origin by default), `--include-dnp` also
exports parts marked “do not populate”, `--units` sets the units of VRML
files.

```text
$ ewerk export-3d ~/Projects/Blinky -o ~/Mechanics/blinky.step
Blinky → /Users/me/Mechanics/blinky.step (390553 bytes)
  8 parts placed, 5 exact models, 0 from meshes
```

The 3D models are looked up like in the [3D view](../guide/view-3d#parts-and-their-3d-models).
Models that were not found are listed (`not found: …`); the board and the
other parts are still written. GLB and VRML need meshes: models of the base
library have them, STEP models get one when the board is first shown in the
app's 3D view (the command line cannot convert STEP files). `--json` prints
the report of [`board3d.export`](./engine-api#board3d-export).

## Library

The library commands work on the [base library](../guide/library). Every
command takes these options:

| Option        | Meaning                                                                                                                                                   |
| ------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `--dir <dir>` | The library folder. Default: `EW_LIBRARY_DIR`, or the app's library folder (`<config>/ElectronicWerk/library`), so the CLI and the app share one library. |
| `--json`      | Print machine-readable JSON (the result of the engine method).                                                                                            |

### `ewerk library build`

```text
ewerk library build [--kicad <dir>] [--symbols <dir>] [--footprints <dir>] [--models <dir>]
                    [--meshes <dir>] [--version <text>] [--license-file <file>] [--dir <dir>] [--json]
```

Builds the library from KiCad libraries and replaces the current index (your
[duplicate decisions](../guide/duplicates) are kept and applied again).
`--kicad` takes a KiCad installation or a folder with the library
repositories and finds the symbol, footprint and 3D model folders in it;
`--symbols`, `--footprints` and `--models` name them one by one. `--meshes`
adds tessellated 3D models (`<Library>.3dshapes/<name>.glb`, made by the
[tessellator](../development/library#tessellating-the-3d-models)).

```console
$ ewerk library build --kicad kicad --meshes meshes --version demo
Built in 0.0 s
Library:  /home/lab/.config/ElectronicWerk/library
Source:   KiCad demo (CC-BY-SA-4.0 WITH KiCad-libraries-exception)
symbols:  6 (5 shown, the others merged into duplicates)
footprints:6 (5 shown, the others merged into duplicates)
models:   4 (3 shown, the others merged into duplicates)
Groups:   3 exact, 3 near (3 to review), 1 alias
Meshes:   4
```

Problems while reading the libraries are listed below the summary, grouped
by code (for example custom pads that are simplified).

### `ewerk library info`

```text
ewerk library info [--dir <dir>] [--json]
```

Prints the same summary as `build`. The exit code is `1` if there is no
library.

### `ewerk library search`

```text
ewerk library search <text> [--kind symbol|footprint|model] [--library <name>] [--limit <n>]
                     [--dir <dir>] [--json]
```

Searches like the [library manager](../guide/library#searching): every word
must match the start of a word, words with punctuation are phrases.

```console
$ ewerk library search 0805 --kind footprint
footprint Demo:C_0805  Capacitor SMD 0805 (demo)
footprint Demo:R_0805  Resistor SMD 0805 (demo)
footprint Demo:R_0805_Wide  Resistor SMD 0805, wider pad spacing (demo)
3 matches in 0.2 ms
```

### `ewerk library duplicates`

```text
ewerk library duplicates [--markdown] [--dir <dir>]
```

Prints the [deduplication report](../guide/duplicates#the-deduplication-report)
with the current decisions, as JSON or (with `--markdown`) as Markdown:

```console
$ ewerk library duplicates --markdown
# Library deduplication report

Tolerance for near duplicates: 0.050 mm

| Kind | Entries | Exact groups | Near groups | Alias groups | Merged | Pending review |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| symbol | 6 | 1 | 1 | 1 | 1 | 1 |
| footprint | 6 | 1 | 1 | 0 | 1 | 1 |
| model | 4 | 1 | 1 | 0 | 1 | 1 |

## Exact duplicates

- **Demo:R** (symbol, merge): same name and drawing
  - Demo_Copy:R
…
```

### `ewerk library pack`

```text
ewerk library pack <out> [--prefix <text>] [--name <text>] [--version <text>] [--dir <dir>] [--json]
```

Writes the [library packs](../development/library#packs) of the library to
`<out>`: the index pack, one mesh pack per 3D library and `manifest.json`
with the size and SHA-256 checksum of every pack. File names start with
`--prefix` (default `ewerk-library`).

```console
$ ewerk library pack packs --prefix ewerk-library-demo --name "Demo library" --version demo
Wrote 2 packs (0.0 MB) and manifest.json to packs
```

### `ewerk library install`

```text
ewerk library install <pack> [--dir <dir>] [--json]
```

Installs a pack: an index pack replaces the index (after checking that it
opens), a mesh pack adds its meshes to the mesh cache (each checked against
its name, which is its content hash). A damaged pack is refused and the
library stays as it was.

```console
$ ewerk library install packs/ewerk-library-demo-meshes-Demo.zip
Installed: index unchanged, 3 meshes
```

## Engine access

### `ewerk rpc`

```text
ewerk rpc [--events] [--no-journal]
```

Runs a **session** with one open project: read one JSON request per line from
standard input, write one JSON reply per line to standard output. This is how
scripts and AI agents edit projects step by step, with undo and the autosave
journal, exactly like the desktop app.

- **JSON-RPC 2.0**: requests with `"jsonrpc": "2.0"` get JSON-RPC 2.0 replies.
  Errors have numeric codes (`-32700` parse error, `-32600` invalid request,
  `-32601` method not found, `-32602` invalid params, `-32603` internal,
  `-32000` every other engine error) and the engine's code in
  `error.data.code` (for example `noProject`). Notifications (no `id`) get no
  reply; a batch (a JSON array) gets an array of replies.
- **Short form**: `{"id": …, "method": "…", "params": …}`; the reply carries
  the same `id` plus `result` or `error` with the engine's error code (see the
  [Engine API](./engine-api)).
- `--events` also writes the engine's events as `{"event": …}` lines.
- `--no-journal` disables the autosave journal (useful for throw-away edits).

```console
$ ewerk rpc --events
{"id":1,"method":"project.open","params":{"path":"LED Blinker"}}
{"id":1,"result":{"name":"LED Blinker","revision":"A", …}}
{"event":{"type":"project","action":"opened","path":"/home/lab/LED Blinker/LED Blinker.ewproj"}}
{"id":2,"method":"project.setProperties","params":{"revision":"B"}}
{"id":2,"result":{"dirty":true,"canUndo":true,"undoLabel":"set-project-properties", …}}
{"event":{"type":"changed","changeSet":{"cause":"commit", …}}}
{"id":3,"method":"project.save"}
{"id":3,"result":{"written":["LED Blinker.ewproj"],"deleted":[], …}}
```

Example in Python:

```python
import json, subprocess

engine = subprocess.Popen(["ewerk", "rpc"], stdin=subprocess.PIPE,
                          stdout=subprocess.PIPE, text=True)

def call(method, params=None, _id=[0]):
    _id[0] += 1
    engine.stdin.write(json.dumps({"id": _id[0], "method": method, "params": params}) + "\n")
    engine.stdin.flush()
    reply = json.loads(engine.stdout.readline())
    if "error" in reply:
        raise RuntimeError(reply["error"]["message"])
    return reply["result"]

project = call("project.create", {"path": "Sensor", "name": "Sensor", "template": "board-4l"})
call("project.setProperties", {"variables": {"AUTHOR": "Hardware Lab"}})
call("project.save")
```

### `ewerk call <method> [params]`

Calls one engine method on a fresh engine and prints the JSON reply. `params`
is a JSON value and defaults to `null`. Because every call starts a new
engine, use [`ewerk rpc`](#ewerk-rpc) for anything that needs an open project.

```console
$ ewerk call engine.ping '{"hello": "world"}'
{"result":{"echo":{"hello":"world"},"pong":true}}

$ ewerk call project.probe '{"path": "LED Blinker"}'
{"result":{"exists":true,"hasRecovery":false,"manifestPath":"…/LED Blinker.ewproj","name":"LED Blinker","path":"…","valid":true}}
```

::: tip Quoting on Windows
In PowerShell, wrap JSON in single quotes and escape inner double quotes:
`ewerk call engine.ping '{\"hello\": \"world\"}'`.
:::

### `ewerk methods`

Lists every engine method, one per line (see the [Engine API](./engine-api)).
With `--json`, every method with a one-line summary:

```console
$ ewerk methods --json | jq '.methods[] | select(.name == "design.route")'
{
  "name": "design.route",
  "summary": "{\"nets\"?, \"layers\"?: [\"top\", \"bottom\"], \"width\"? (mm), \"board\"?}. Routes the unrouted connections one by one …"
}
```

### `ewerk script`

```text
ewerk script <file> [--project <folder>] [--keep-going] [--dry-run] [--json]
```

Runs engine calls from a file in one session. The file is JSON Lines (one
`{"method", "params"}` per line; empty lines and lines starting with `#` or
`//` are skipped), a JSON list of steps, or `{"project", "steps"}` (the
project relative to the script). `-` reads standard input.

- `--project` opens a project first and saves it at the end (if something
  changed).
- The first failing step stops the script and the exit code is `1`;
  `--keep-going` runs the rest (and still saves).
- `--dry-run` does not save; `--json` prints every step's result.

```console
$ ewerk script status-led.jsonl --project "Status LED"
ok    design.place             R1 330
…
ok    design.route             3 routed, 0 unrouted
11 steps, 0 failed, project saved
```

See the [scripting cookbook](./cookbook) for complete scripts.

### `ewerk mcp`

```text
ewerk mcp [--project <folder>] [--no-autosave]
```

A [Model Context Protocol](https://modelcontextprotocol.io) server on standard
input and output (JSON-RPC 2.0, one message per line) for AI agents. It
offers tools to open and create projects, read and draw schematics, update,
place, route and check boards and generate outputs, plus `call_engine` for
any engine method; see [AI agents (MCP)](../guide/automation#ai-agents-mcp).

- Changes are saved after every tool call; `--no-autosave` keeps them in
  memory until the agent calls `save_project`.
- `--project` opens a project at the start.
- Tool failures (an unknown pin, a missing project) are returned as tool
  results with `isError`, so the agent can correct itself.
- Protocol versions 2025-11-25, 2025-06-18, 2025-03-26 and 2024-11-05.

## Development tools

### `ewerk synth`

```text
ewerk synth <S|M|L|XL> <dir> [--seed <n>] [--json]
```

Generates a synthetic, reproducible reference design (see
[Benchmarks](../development/benchmarks)). The same size and seed always
produce the same project.

```console
$ ewerk synth M demo-m
Created synthetic M design: /home/lab/demo-m/Synthetic M.ewproj
500 symbols on 6 sheets, 500 footprints (2642 pads), 1000 nets, 20000 tracks, 2440 vias, 2 zones, 4 copper layers (247 ms)
```

### `ewerk bench`

```text
ewerk bench [--size S|M|L|XL] [--json]
```

Generates a design in a temporary folder and measures opening, saving and building the board scene
(best of three runs).

```console
$ ewerk bench --size L
L design: open 142 ms, save 304 ms, board scene 40 ms (587730 primitives) (best of 3)
```

The board scene is what the [2D view](../guide/viewer) gets from the engine
(see [`view.scene`](./engine-api#view-scene)).

### `ewerk info`

```console
$ ewerk info
ElectronicWerk Engine 0.2.0
Target:  linux-x86_64
Threads: 16
Build:   release
```

With `--json`, the same information is printed as JSON.

## Exit codes

| Code | Meaning                                                                                               |
| ---- | ----------------------------------------------------------------------------------------------------- |
| `0`  | Success                                                                                               |
| `1`  | A check found errors: project validation, ERC, DRC, import findings, a failed output or a script step |
| `2`  | Wrong command-line usage (unknown command or option, invalid value)                                   |
| `3`  | The command could not run: an engine error (unknown part, missing project, …), a file or JSON problem |

Before version 0.13, errors of the third kind also had the exit code `1`.

## Using `ewerk` in scripts

```sh
# Fail a CI job if the project has errors.
ewerk project validate hardware/ || exit 1

# Read values with jq.
ewerk project info hardware/ --json | jq -r '.documents[] | select(.kind=="pcb") | .stats.copperLayers'
```
