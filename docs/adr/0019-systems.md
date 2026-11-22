# ADR-0019: Systems of several boards

- **Status:** accepted
- **Date:** 2026-11-22
- **Milestone:** M18

## Context

Many devices are several boards: a shield stacked on a carrier, a main board
with a front panel board, boards joined by cables, a board built onto a
development kit whose design files are open. Mistakes between the boards —
signals swapped on a header, a supply on a signal pin, a pin open on one
side — were found only when the boards were built and plugged together. The
roadmap asked for a system that lists the boards (of this project, of other
projects, reference boards from open design files) and how they connect
(stacked or by cables); a connection check across the boards with renaming
a signal on the others; the system in 3D with stacking by mating connectors,
collisions between the boards and with their enclosures, and one STEP file;
wiring lists and harness drawings for cables; and a system bill of
materials, a connection report, command line and engine API.

## Decision

### A system is a document of the project

A system is a new document kind (`.ewsys` in `systems/`, tag
`electronicwerk.system`) beside schematics, boards, libraries, rules and
output jobs: boards, links (connectors that mate), cables with their ends
and wires, signal aliases and accepted findings.

- **Why a document, not a project of projects**: it is saved, versioned,
  archived and undone like everything else, and a project can hold several
  systems (a product and its test fixture). A board belongs to one project;
  the system refers to it.
- **Edits are property changes** of the whole document (`SetProps`). A
  system is small (a few boards, links and cables), so replacing it per edit
  is cheap, and undo, redo and change events come for free.

### Boards are references

A board of the system is `{kind: local, doc}` (a board document of this
project) or `{kind: project, path, board}` (another project, stored relative
to this project so both can move together). Other projects are read from
their files and kept in a cache keyed by the modification times of the
manifest and the documents; they do not need to be open and are never
written, except by an explicit rename.

- **Reference designs** (open KiCad or Eagle files) are imported once with
  the existing importers as their own project in `systems/references/`. The
  system then treats them like any other project; the import report stays
  beside them.
- **Placement** is a position (nm, z = the board's bottom face) and a
  rotation per board in system coordinates — the same convention as
  mechanical bodies (ADR-0018), so the 3D code is shared.

### Connectors are footprints, pins are pads

A connector is a footprint of the board (listed when its designator, name
or value looks like one), its pins are its pads with their numbers and
nets. No new connector object or library field was needed; any footprint
can be chosen.

- **Pairing** says which pin meets which: by number, mirrored rows (a
  two-row connector turned over) or by position in 3D (the pads' places
  after both boards' transforms, matched within 0.5 mm). Position is the
  truth for stacked boards, whose bottom-side headers are mirrored.
- **Cables** are explicit wires from an end and pin to an end and pin; an
  end may be free (a switch or sensor not modelled as a board). A crossed
  UART is just two wires.

### The check compares signal names

Signals are compared by their net names without the sheet path, ignoring
case, with user aliases (`BUTTON = BTN`). Names the tools make themselves
(`Net-(…)`, `unconnected-(…)`, `N$…`) are not compared; a supply is
recognised by its name (GND, VCC, +5V, 3V3 …) or a net class containing
"power".

- **Why names**: they are what the designer chose and what the schematics
  show; matching them is what a reviewer does by hand. Electrical types of
  pins (input, output) would need complete symbol data on every board,
  which reference designs and imports often lack.
- **Swapped pairs** are found as one finding: two pins whose signals are
  exchanged on the other side. Other differences are one finding per pin.
- **Findings** have codes like design rules check findings (`system-*`),
  severities, keys for accepting them and arguments for translated
  messages. The check runs in the engine; the editor refreshes it after
  every change of the system or any of its boards.
- **Renaming** follows the signal through links and cables (a breadth-first
  walk over pin edges) and renames the net on every reached board of this
  project and the labels in its schematics, as one undo step. Boards of
  other projects are named, not changed: writing into another project
  behind the user's back would surprise them.

### 3D reuses the board's scene

The system scene is the board scenes of all shown boards, their meshes
transformed and merged into one buffer, their parts and mechanical bodies
with the board's matrix applied. Models of other projects are resolved
from their own folder. The collision check reuses the mechanical check's
meshes and R-trees (ADR-0018): every pair of boards is compared, mating
connectors may touch. The STEP export writes every board's assembly with
its bodies at its place and joins them in one assembly file.

- **Stacking** fits the second board's connector pads onto the first's (a
  2D Kabsch fit, tried with both pairings and turned over if asked) and
  lifts it by the lower connector's model height (8.5 mm, a 2.54 mm socket
  strip, without a model).

### Outputs are plain files

The bill of materials (CSV), the connection report (CSV and HTML), the
wiring lists (CSV) and the harness drawings (PDF and SVG, drawn on the
output canvas of M8, A4 landscape) go into `outputs/<system>/`. A harness
drawing shows both ends as pin tables and the wires in their colours; it is
not a full harness layout (no bundle routing, no branch lengths), which
needs a dedicated tool.

## Consequences

- New model types in `ew-model::system`, codec and validation in
  `ew-format`; engine modules `system` (check, rename, references),
  `system3d` (scene, stacking, collisions, STEP) and `system_outputs`.
- New engine methods `system.*` (info, boards, links, cables, check,
  counterparts, renameSignal, aliases, exclusions, references, stack, scene,
  export, bom, wiring, outputs); `step::assembly` in `ew-board3d`.
- `ewerk system …`, `ewerk synth system`, MCP tools `system_info`,
  `system_check`.
- The desktop app has a Systems group in the project tree and a system
  editor (boards, connections, cables, check, 3D).
- Limits: signals are matched by name only; boards of other projects are
  read, not renamed; harness drawings are wiring diagrams, not
  manufacturing layouts; flexible cables are not shown in 3D.
