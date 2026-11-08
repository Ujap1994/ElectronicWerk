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

| Command                                               | Purpose                                              |
| ----------------------------------------------------- | ---------------------------------------------------- |
| [`ewerk project new`](#ewerk-project-new)             | Create a project from a template                     |
| [`ewerk project info`](#ewerk-project-info)           | Show a summary of a project                          |
| [`ewerk project validate`](#ewerk-project-validate)   | Check a project for errors                           |
| [`ewerk project archive`](#ewerk-project-archive)     | Write a ZIP archive of a project                     |
| [`ewerk project upgrade`](#ewerk-project-upgrade)     | Rewrite a project in the current file format         |
| [`ewerk project templates`](#ewerk-project-templates) | List the project templates                           |
| [`ewerk import kicad`](#ewerk-import-kicad)           | Import a KiCad project as a new project              |
| [`ewerk import library`](#ewerk-import-library)       | Add a KiCad library to a project library             |
| [`ewerk import netlist`](#ewerk-import-netlist)       | Compare a netlist with a board, or apply it          |
| [`ewerk rpc`](#ewerk-rpc)                             | Run an engine session over standard input and output |
| [`ewerk call`](#ewerk-call-method-params)             | Call one engine method                               |
| [`ewerk methods`](#ewerk-methods)                     | List every engine method                             |
| [`ewerk synth`](#ewerk-synth)                         | Generate a synthetic reference design                |
| [`ewerk bench`](#ewerk-bench)                         | Measure opening and saving a synthetic design        |
| [`ewerk info`](#ewerk-info)                           | Show information about the engine build              |

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
3 board graphics, 8 footprints, 10 labels, 1 library footprints, 2 library symbols, 7 nets, 22 pads, 3 sheet texts and graphics, 2 sheets, 17 symbols, 12 tracks, 2 vias, 19 wires, 1 zones
warning blinky.kicad_dru: 1 custom design rules are not imported; the rule editor arrives with the layout editor [custom-rules]
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

## Engine access

### `ewerk rpc`

```text
ewerk rpc [--events] [--no-journal]
```

Runs a **session** with one open project: read one JSON request per line from
standard input, write one JSON reply per line to standard output. This is how
scripts and AI agents edit projects step by step, with undo and the autosave
journal, exactly like the desktop app.

- A request is `{"id": …, "method": "…", "params": …}`; the reply carries the
  same `id` plus `result` or `error` (see the [Engine API](./engine-api)).
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

| Code | Meaning                                                                               |
| ---- | ------------------------------------------------------------------------------------- |
| `0`  | Success                                                                               |
| `1`  | The engine reported an error, the project has validation errors, or input was invalid |
| `2`  | Wrong command-line usage (unknown command or option)                                  |

## Using `ewerk` in scripts

```sh
# Fail a CI job if the project has errors.
ewerk project validate hardware/ || exit 1

# Read values with jq.
ewerk project info hardware/ --json | jq -r '.documents[] | select(.kind=="pcb") | .stats.copperLayers'
```
