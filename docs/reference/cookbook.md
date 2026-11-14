# Scripting cookbook

Recipes for the command-line tool `ewerk`: designs from scripts, checks in
CI, batch jobs and programs that drive the engine. The basics are in
[Scripts and AI agents](../guide/automation); every command is in the
[command-line reference](./cli).

The status LED examples are in the repository under
[`examples/automation`](https://github.com/Ujap1994/ElectronicWerk/tree/main/examples/automation)
and run in the automated tests of every release, so they always work with
the current version.

## A status LED with commands

Shell script (`status-led.sh`): a project with a 30 × 20 mm board, three
parts, three nets, the board placed and routed, the checks and the Gerber
files.

```sh
#!/bin/sh
set -e
P="${1:-Status LED}"

ewerk project new "$P" --width 30 --height 20

# Schematic: parts from the project library, footprints from the wizard.
ewerk schematic place "$P" R --value 330 --footprint 0805
ewerk schematic place "$P" LED --value green --footprint 0805
ewerk schematic place "$P" C --value 100n --footprint 0603
ewerk schematic connect "$P" VCC R1.1 C1.1 --style power
ewerk schematic connect "$P" LED_A R1.2 D1.A
ewerk schematic connect "$P" GND D1.K C1.2 --style power
ewerk schematic erc "$P"

# Board: footprints onto the board (positions in mm), routed, checked.
ewerk board update "$P"
ewerk board place "$P" R1 --at 10,8
ewerk board place "$P" D1 --at 20,8 --rotate 180
ewerk board place "$P" C1 --at 15,14
ewerk board route "$P"
ewerk board drc "$P"

# Manufacturing files.
ewerk outputs run "$P"
```

```console
$ sh status-led.sh
Created /home/lab/Status LED/Status LED.ewproj
…
Placed R1 (330) at [30.48,179.52] mm on sheet Root
Placed D1 (green) at [55.88,179.52] mm on sheet Root
Placed C1 (100n) at [81.28,179.52] mm on sheet Root
Connected R1.1, C1.1 to VCC
Connected R1.2, D1.2 to LED_A
Connected D1.1, C1.2 to GND
ERC: 0 errors, 0 warnings
Applied 6 changes from the schematic
Placed R1 at [10.0,8.0] mm, 0.0°, top
Placed D1 at [20.0,8.0] mm, 180.0°, top
Placed C1 at [15.0,14.0] mm, 0.0°, top
Routed 3 connections; 0 still unrouted
warning silk-width       Silkscreen line width 0.12 mm is less than 0.15 mm (board constraints)
…
DRC: 0 errors, 3 warnings, 0 unrouted
```

![The routed status LED](/screenshots/automation-board.png)

`set -e` stops the script at the first command that fails, including the
ERC and the DRC when they find errors (exit code `1`).

## A status LED as a script

The same design as one `ewerk script` (`status-led.jsonl`), in one engine
session:

```text
# A status LED as an ewerk script (one engine call per line).
{"method": "design.place", "params": {"symbol": "R", "value": "330", "footprint": "0805"}}
{"method": "design.place", "params": {"symbol": "LED", "value": "green", "footprint": "0805"}}
{"method": "design.place", "params": {"symbol": "C", "value": "100n", "footprint": "0603"}}
{"method": "design.connect", "params": {"net": "VCC", "pins": ["R1.1", "C1.1"], "style": "power"}}
{"method": "design.connect", "params": {"net": "LED_A", "pins": ["R1.2", "D1.A"]}}
{"method": "design.connect", "params": {"net": "GND", "pins": ["D1.K", "C1.2"], "style": "power"}}
{"method": "design.updateBoard"}
{"method": "design.placeFootprint", "params": {"designator": "R1", "at": [10, 8]}}
{"method": "design.placeFootprint", "params": {"designator": "D1", "at": [20, 8], "rotation": 180}}
{"method": "design.placeFootprint", "params": {"designator": "C1", "at": [15, 14]}}
{"method": "design.route"}
```

```sh
ewerk project new "Status LED" --width 30 --height 20
ewerk script status-led.jsonl --project "Status LED"
```

A JSON file works too, with the project named in the file:

```json
{
  "project": "Status LED",
  "steps": [
    { "method": "design.place", "params": { "symbol": "R", "footprint": "0805" } },
    { "method": "design.route" }
  ]
}
```

## Parts from the base library

With the [base library](../guide/library) installed (in the app, or with
`ewerk library build`), any KiCad symbol and footprint can be used by its
full name. Search first:

```console
$ ewerk library search ne555 --kind symbol
symbol    Timer:NE555P  Precision Timers, 555 compatible, PDIP-8
symbol    Timer:NE555D  Precision Timers, 555 compatible, SOIC-8
2 matches in 0.8 ms
$ ewerk schematic place Timer Timer:NE555P --footprint Package_DIP:DIP-8_W7.62mm
Placed U1 (NE555P) at [30.48,179.52] mm on sheet Root
$ ewerk schematic connect Timer VCC U1.VCC U1.RST --style power
Connected U1.8, U1.4 to VCC
```

The symbol (and its default footprint) is copied into the project library;
pins can be named (`U1.VCC`; `U1.RST` also finds KiCad's overlined `~{RST}`) or numbered (`U1.8`).

## Checks in GitHub Actions

Check every push and keep the manufacturing files of each run:

```yaml
name: Hardware
on: [push, pull_request]

jobs:
  check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Install ewerk
        run: |
          curl -sSL -o ewerk.tar.gz \
            "https://github.com/Ujap1994/ElectronicWerk/releases/download/v0.13.0/ewerk-0.13.0-linux-x64.tar.gz"
          tar -xzf ewerk.tar.gz
          echo "$PWD/ewerk-0.13.0-linux-x64" >> "$GITHUB_PATH"
      - name: Validate, ERC and DRC
        run: |
          ewerk project validate hardware/
          ewerk schematic erc hardware/
          ewerk board drc hardware/
      - name: Manufacturing files
        run: ewerk outputs run hardware/
      - uses: actions/upload-artifact@v4
        with:
          name: manufacturing
          path: hardware/outputs/
```

A design error makes the job fail (exit code `1`); a broken setup shows as
exit code `3`, so the two are easy to tell apart.

## All projects of a folder

Generate the outputs of every project below a folder and report the ones
with errors:

```sh
#!/bin/sh
status=0
for manifest in $(find projects -name '*.ewproj'); do
  folder=$(dirname "$manifest")
  if ewerk board drc "$folder" > /dev/null; then
    ewerk outputs run "$folder" > /dev/null && echo "ok      $folder"
  else
    echo "errors  $folder"
    status=1
  fi
done
exit $status
```

In PowerShell:

```powershell
Get-ChildItem projects -Recurse -Filter *.ewproj | ForEach-Object {
  ewerk board drc $_.DirectoryName | Out-Null
  if ($LASTEXITCODE -eq 0) { ewerk outputs run $_.DirectoryName | Out-Null }
  else { Write-Host "errors  $($_.DirectoryName)" }
}
```

## Reading results with jq

Every command prints JSON with `--json`:

```sh
# Parts without a footprint.
ewerk schematic list hardware/ --json | jq -r '.components[] | select(.footprint == null) | .designator'

# Footprints that are not on the board yet.
ewerk board info hardware/ --json | jq -r '.footprints[] | select(.onBoard | not) | .designator'

# The nets of a part.
ewerk schematic list hardware/ --json | jq '.components[] | select(.designator == "U1") | .pins'

# Number of DRC errors.
ewerk board drc hardware/ --json | jq '[.findings[] | select(.severity == "error")] | length'
```

## Python

`ewerk rpc` keeps the project open between calls and speaks JSON-RPC 2.0
(`status-led.py`):

```python
import json, os, subprocess

class Engine:
    def __init__(self):
        self.process = subprocess.Popen(
            [os.environ.get("EWERK", "ewerk"), "rpc", "--no-journal"],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
        self.next_id = 0

    def call(self, method, params=None):
        self.next_id += 1
        request = {"jsonrpc": "2.0", "id": self.next_id, "method": method, "params": params}
        self.process.stdin.write(json.dumps(request) + "\n")
        self.process.stdin.flush()
        reply = json.loads(self.process.stdout.readline())
        if "error" in reply:
            raise RuntimeError(f"{method}: {reply['error']['message']}")
        return reply["result"]

engine = Engine()
engine.call("project.create", {
    "path": "Status LED", "name": "Status LED", "template": "board-2l",
    # Engine lengths are nanometres; design.* methods take millimetres.
    "board": {"width": 30_000_000, "height": 20_000_000, "layers": 2, "thickness": 1_600_000},
})
for symbol, value, footprint in [("R", "330", "0805"), ("LED", "green", "0805"), ("C", "100n", "0603")]:
    part = engine.call("design.place", {"symbol": symbol, "value": value, "footprint": footprint})
    print("placed", part["designator"], part["value"])
engine.call("design.connect", {"net": "VCC", "pins": ["R1.1", "C1.1"], "style": "power"})
engine.call("design.connect", {"net": "LED_A", "pins": ["R1.2", "D1.A"]})
engine.call("design.connect", {"net": "GND", "pins": ["D1.K", "C1.2"], "style": "power"})
print("ERC errors:", engine.call("schematic.erc")["errors"])
engine.call("design.updateBoard")
engine.call("design.arrange")
print(engine.call("design.route"))
engine.call("project.save")
```

## Node.js

```js
import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';

const engine = spawn('ewerk', ['rpc', '--no-journal']);
const lines = createInterface({ input: engine.stdout });
const waiting = new Map();
let nextId = 0;
lines.on('line', (line) => {
  const reply = JSON.parse(line);
  const { resolve, reject } = waiting.get(reply.id);
  waiting.delete(reply.id);
  if (reply.error) reject(new Error(reply.error.message));
  else resolve(reply.result);
});
const call = (method, params = null) =>
  new Promise((resolve, reject) => {
    const id = ++nextId;
    waiting.set(id, { resolve, reject });
    engine.stdin.write(`${JSON.stringify({ jsonrpc: '2.0', id, method, params })}\n`);
  });

await call('project.open', { path: 'Status LED' });
const { components } = await call('design.components');
for (const part of components) console.log(part.designator, part.value, part.footprint);
engine.stdin.end();
```

## Undo inside a session

Every changing call is one undo step, also `design.route` with all its
tracks:

```text
{"jsonrpc": "2.0", "id": 1, "method": "design.route"}
{"jsonrpc": "2.0", "id": 2, "method": "design.board"}
{"jsonrpc": "2.0", "id": 3, "method": "edit.undo"}
```

## AI agents

Configure `ewerk mcp` in your agent as described in
[AI agents (MCP)](../guide/automation#ai-agents-mcp). Useful prompts:

- _"Open the project in ~/Documents/ElectronicWerk/Sensor and list the parts
  without a footprint. Give the resistors and capacitors 0603 footprints."_
- _"Run the ERC and fix the unconnected pins: the NC pins of U3 stay open."_
- _"Update the board, put the connector J1 at the left edge, arrange the
  rest, route everything you can and tell me what is left."_
- _"Generate the manufacturing files and list the written files."_
