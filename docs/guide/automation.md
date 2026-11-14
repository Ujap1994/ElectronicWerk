# Scripts and AI agents

Everything you do in ElectronicWerk can also be done without the window:
from a terminal, in a script, in a CI pipeline or by an **AI agent** such as
Claude. All of them use the same engine as the app, through the command-line
tool `ewerk`.

![A board designed by a script: three parts, placed and routed](/screenshots/automation-board.png)

This board was made without opening the app, by this
[script](../reference/cookbook#a-status-led-as-a-script): three parts, three
nets, four footprint commands and the router. The app opens the result like
any other project.

## Three ways to automate

| Way                                                 | For                                                                |
| --------------------------------------------------- | ------------------------------------------------------------------ |
| **Commands** (`ewerk schematic …`, `ewerk board …`) | Shell scripts, CI checks, quick edits from a terminal              |
| **Scripts** (`ewerk script`)                        | A list of engine calls in a file, run in one go                    |
| **Engine sessions** (`ewerk rpc`, `ewerk mcp`)      | Programs (Python, Node.js, …) and AI agents that work step by step |

All three address what a designer names: parts by **designator** (`R1`),
pins as **`R1.2`** or **`U1.VCC`** (number or name), nets by **name**, sheets
and boards by name, lengths in **millimetres** and angles in **degrees**
(counter-clockwise, y points up, as in the app). The
[design methods](../reference/engine-api#design) behind them are described
in the engine API.

## Install the command-line tool

Download `ewerk` for your system from the
[release page](https://github.com/Ujap1994/ElectronicWerk/releases/latest) and
put it on your `PATH`, see [Command-line tool](../reference/cli#installation).
Check it with:

```sh
ewerk --version
```

The [settings](./settings#ai-agents-and-scripts) of the app show the
download link and the configuration for AI agents.

![AI agents and scripts in the settings](/screenshots/automation-settings.png)

## A design from the terminal

The commands open the project, make their change, save and close it. This is
the whole status LED of the picture above:

```sh
ewerk project new "Status LED" --width 30 --height 20

ewerk schematic place "Status LED" R --value 330 --footprint 0805
ewerk schematic place "Status LED" LED --value green --footprint 0805
ewerk schematic place "Status LED" C --value 100n --footprint 0603
ewerk schematic connect "Status LED" VCC R1.1 C1.1 --style power
ewerk schematic connect "Status LED" LED_A R1.2 D1.A
ewerk schematic connect "Status LED" GND D1.K C1.2 --style power
ewerk schematic erc "Status LED"

ewerk board update "Status LED"
ewerk board place "Status LED" R1 --at 10,8
ewerk board place "Status LED" D1 --at 20,8 --rotate 180
ewerk board place "Status LED" C1 --at 15,14
ewerk board route "Status LED"
ewerk board drc "Status LED"

ewerk outputs run "Status LED"
```

What happens in the schematic:

![The schematic the script drew](/screenshots/automation-schematic.png)

- **Parts.** `schematic place` takes a symbol of the project library (new
  projects have resistors, capacitors, diodes, LEDs, a transistor and a push
  button) or of the [base library](./library) by its full name, such as
  `Device:R` or `MCU_Microchip_ATmega:ATmega328P-P`; a symbol of the base
  library is copied into the project library first. Without `--at`, the part
  goes to a free place on the sheet, on the grid. Without `--ref`, it gets
  the next free designator.
- **Footprints.** `--footprint` takes a footprint of the project library, a
  footprint of the base library (`Resistor_SMD:R_0805_2012Metric`) or a
  **wizard preset**: `0402` to `2512`, `SOIC-8`, `TSSOP-20`, `QFN-32 5x5`,
  `pin header 1x04 2.54 mm` and more (`ewerk call design.presets` lists
  them). Presets work without any library.
- **Nets.** `schematic connect` draws a short wire from each pin and puts a
  **net label** at its end: a local label, or a global label when the net
  spans several sheets. With `--style power`, a power port (GND gets the
  ground symbol). Pins already on the net are skipped; a pin on another
  named net is an error.

And on the board:

- `board update` adds the footprints (below the board outline) and the nets,
  like [Update board from schematic](./schematic-to-board).
- `board place` puts a footprint at a position, rotation and side;
  `board arrange` puts all footprints that are not on the board in rows
  inside the outline.
- `board route` routes the unrouted connections one by one with the
  [walkaround router](./routing): 45° bends, one copper layer per connection
  (top first, then bottom), no vias, the track width of the design rules.
  Connections it cannot route are listed; route them in the app.

Every command prints a short report; with `--json` it prints the full result
for other programs.

## Exit codes for CI

| Code | Meaning                                                                       |
| ---- | ----------------------------------------------------------------------------- |
| `0`  | Success                                                                       |
| `1`  | A check found errors: ERC, DRC, project validation, a failed output or step   |
| `2`  | Wrong command-line usage                                                      |
| `3`  | The command could not run: unknown part or pin, missing project, invalid JSON |

So a pipeline can fail on design errors:

```sh
ewerk schematic erc hardware/ && ewerk board drc hardware/ && ewerk outputs run hardware/
```

The [cookbook](../reference/cookbook#checks-in-github-actions) has a
complete GitHub Actions workflow.

## Scripts

`ewerk script` runs engine calls from a file in **one session**, which is
faster than one command per step and is undone as a whole when you close the
project without saving. A script is a JSON list or **JSON Lines** (one call
per line; `#` starts a comment):

```text
# status-led.jsonl
{"method": "design.place", "params": {"symbol": "R", "value": "330", "footprint": "0805"}}
{"method": "design.place", "params": {"symbol": "LED", "value": "green", "footprint": "0805"}}
{"method": "design.connect", "params": {"net": "LED_A", "pins": ["R1.2", "D1.A"]}}
```

```console
$ ewerk script status-led.jsonl --project "Status LED"
ok    design.place             R1 330
ok    design.place             D1 green
ok    design.connect           LED_A → R1.2, D1.2
3 steps, 0 failed, project saved
```

- `--project` opens the project first and saves it at the end.
- The first failing step stops the script (exit code `1`); `--keep-going`
  runs the remaining steps.
- `--dry-run` saves nothing; `--json` prints every result.

Any [engine method](../reference/engine-api) can be a step.

## AI agents (MCP)

`ewerk mcp` is a [Model Context Protocol](https://modelcontextprotocol.io)
server: AI agents that speak MCP (Claude Desktop, Claude Code, VS Code,
Cursor and others) get tools to design with ElectronicWerk. Add it to the
agent's MCP configuration:

```json
{
  "mcpServers": {
    "electronicwerk": { "command": "ewerk", "args": ["mcp"] }
  }
}
```

| Agent          | Where                                                                                        |
| -------------- | -------------------------------------------------------------------------------------------- |
| Claude Desktop | _Settings → Developer → Edit Config_ (`claude_desktop_config.json`), as above                |
| Claude Code    | `claude mcp add electronicwerk -- ewerk mcp`                                                 |
| VS Code        | `.vscode/mcp.json`: `{"servers": {"electronicwerk": {"command": "ewerk", "args": ["mcp"]}}}` |
| Cursor         | `~/.cursor/mcp.json` (or `.cursor/mcp.json` in a project), as above                          |

::: tip Full path
Desktop apps do not always see your shell's `PATH`. If the agent reports that
it cannot start `ewerk`, give the full path, for example
`"command": "/usr/local/bin/ewerk"` or `"C:\\Tools\\ewerk.exe"`.
:::

Then ask the agent in plain words, for example: _"Create a project Status LED
in my Documents folder with a 30 × 20 mm board: a green 0805 LED with a 330 Ω
resistor from VCC and a 100 nF decoupling capacitor. Route it and generate the
Gerber files."_ The agent opens or creates the project and works with these
tools:

| Tool                                             | What it does                                     |
| ------------------------------------------------ | ------------------------------------------------ |
| `open_project`, `create_project`, `project_info` | Open, create and describe a project              |
| `list_parts`, `list_nets`, `list_symbols`        | Read the schematic                               |
| `search_library`, `list_footprint_presets`       | Find symbols and footprints                      |
| `place_part`, `edit_part`, `connect_pins`        | Draw the schematic                               |
| `mark_no_connect`, `run_erc`                     | Finish and check the schematic                   |
| `update_board`, `board_info`                     | Bring the board up to date and read it           |
| `place_footprint`, `arrange_footprints`          | Place footprints                                 |
| `route_board`, `fill_zones`, `run_drc`           | Route, fill zones and check the board            |
| `generate_outputs`                               | Write the manufacturing files of the output job  |
| `undo`, `redo`, `save_project`                   | Undo and redo (each tool call is one step), save |
| `list_engine_methods`, `call_engine`             | Any other engine method, for advanced agents     |

Changes are **saved after every tool call**, so you can look at them in the
app at any time (reopen the project). `ewerk mcp --no-autosave` keeps them in
memory until the agent calls `save_project`. `ewerk mcp --project <folder>`
opens a project at the start.

## Programs (JSON-RPC)

`ewerk rpc` keeps one engine session open and speaks
[JSON-RPC 2.0](https://www.jsonrpc.org/specification) over standard input
and output, one message per line. Any language that can start a process can
use it; the [cookbook](../reference/cookbook#python) shows Python and
Node.js. Engine lengths are nanometres there, except in the `design.*`
methods, which take millimetres.

## The app and the agent at the same time

Agents and scripts change the project files directly. The app does not see
those changes while the project is open in it, and it **never overwrites
them**: if you save in the app after another program changed a file you also
changed, the app refuses with _Not saved: another program … changed …_ and
keeps your work in memory. Save it with **File → Save Copy**, then reopen the
project to see the agent's work. The same check protects the agent from your
saves.

The simplest way: let the agent work on a project that is closed in the app,
and open it when the agent is done.

## Command reference

Every command and option is in the
[command-line reference](../reference/cli); recipes are in the
[scripting cookbook](../reference/cookbook).
