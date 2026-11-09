# ADR-0006: Schematic editor and electrical rules check

- **Status:** accepted
- **Date:** 2026-11-09
- **Milestone:** M5

## Context

M5 turns the read-only sheet view of M2 into a schematic editor and adds the
electrical rules check (ERC). The roadmap asks for placing and editing
symbols, wires, buses, labels, power ports and flags, hierarchical sheets,
annotation, smart editing (rubber-banding, automatic junctions, copy and
paste across sheets, find and replace, net highlighting), a configurable ERC
with a results panel, beginner aids and exports.

Three constraints shape the design:

- The engine owns the data (ADR-0001). The UI must not grow a second model
  of the schematic that can drift from the engine's.
- KiCad projects are the main source of real designs (ADR-0004). The
  connectivity and the ERC must give the same answers as KiCad, or imported
  designs would show errors that are not there.
- The 2D view (ADR-0003) draws a binary scene built by the engine. Editing
  must stay fluid without rebuilding that scene on every mouse move.

## Decision

### Logic in a crate of its own, edits as change requests

The new crate `ew-schematic` holds everything about schematics that is not
drawing: connectivity, the ERC, editing operations, annotation, find and
replace and exports. It depends only on the data model. Every editing
operation works on a copy of one sheet and returns a list of changes
(insert, update, delete); `ew-core` turns them into ordinary change requests
and commits them as one undo step with its own history label
(`place-symbol`, `drag`, `annotate`, …). Undo, the autosave journal, crash
recovery and change events therefore work for the editor without any new
mechanism, and scripts can use the same operations through the
`schematic.*` methods of the engine API.

The working copy also makes the junction rule easy to keep: after each edit,
the points the edit touched are checked, and a junction is added where three
or more connections meet (a wire passing through counts twice, at least one
of them ends there) and removed where it is no longer needed. Crossing
wires never get one; junctions placed on purpose stay.

### Connectivity like KiCad

`ew-schematic::analyze` builds the nets of the whole hierarchy. Each
placement of a sheet is its own instance (a path of sheet symbol ids), so a
sheet used twice has two sets of nets and two sets of designators
(`PlacedSymbol::instances`, keyed by that path). Within an instance, points
connect when they coincide or when a point lies on a wire (union-find over
exact integer coordinates; horizontal and vertical wires are indexed).
Pieces then merge across the hierarchy by local labels (per instance),
global labels, power ports and power symbols (globally), hidden power pins
(by pin name) and hierarchical labels (to the sheet pin above). Nets are
named the way KiCad names them, including `Net-(R1-Pad2)`.

The rules were checked against KiCad itself: on the synthetic demo project
and on a real 15-sheet project, all connected nets match the netlists KiCad
exports (480 of 480), and the KiCad import was fixed where it disagreed
(mirrored symbols turned by 90° or 270° had swapped pins). `kicad-cli sch
erc` reports the same findings as the ERC on both projects. The grid of a
sheet starts at the top-left page corner, as in KiCad, so imported drawings
stay on it.

### A configurable ERC with KiCad's defaults

The checks and the 12 × 12 pin type matrix default to KiCad's values; the
defaults live in the data model (`ERC_CHECKS`, `ERC_PIN_MATRIX`). A project
stores only what differs, as an optional `erc` section of its rules document
(no schema change). The KiCad import maps the `.kicad_pro` settings onto it.
Every finding carries a code, the sheet, the instance path, a position, the
objects concerned and the values for a translated message, so the UI can
zoom to it and show it in either language.

### Interaction in the renderer, previews without rebuilding the scene

The renderer's `SchematicEditor` implements an interaction interface of the
viewer controller: it receives pointer and key events, snaps points to the
grid and calls the engine. Previews never touch the scene:

- wires being drawn, stretched wires, the symbol being placed and new sheets
  are drawn as SVG over the canvas;
- objects being moved keep their GPU buffers: a per-object state bit makes
  the vertex shader add an offset uniform, and another bit hides the wires
  that the SVG preview draws stretched.

When the edit is committed, the engine's change event reloads the scene
(one sheet builds in a few milliseconds); the offset stays until the new
scene arrives, so nothing jumps.

Rubber-banding keeps horizontal and vertical wires straight: the end that
follows the drag gets a new corner wire, as in KiCad. `schematic.dragInfo`
tells the preview which wires stretch and which labels, junctions, flags
and power ports move along, using the same rule as the edit.

### Beginner aids

New projects bring eleven starter symbols drawn like KiCad's (resistor,
capacitors, inductor, diodes, LED, NPN transistor, connectors, push button,
`PWR_FLAG`), so the guide for a first schematic works before the base
library is downloaded. The guide's steps tick themselves off from the sheet
contents; every tool explains itself in a tooltip and the status line.

### Exports from the same data

Netlists (KiCad format and CSV) and the bill of materials come from the
connectivity in `ew-schematic`; the KiCad netlist reads back through the
KiCad importer and matches KiCad's own. The SVG is written from the scene
the view draws (so it looks exactly like the screen, in the light theme),
and the PDF prints one SVG page per sheet in a hidden, script-free window
(`printToPDF` with CSS page sizes).

## Consequences

- Every editing feature is available to scripts and tests through the engine
  API, and the UI stays a thin layer over it.
- Editing a large sheet stays fluid: previews cost one uniform or a few SVG
  elements; only committed edits rebuild a sheet's scene.
- The ERC and the netlists of imported KiCad projects match KiCad, so users
  can move between the tools.
- Buses are drawn but not yet expanded into member nets; bus connectivity
  (`D[0..7]`) is a later step. The BOM is a plain CSV until the online part
  search adds ordering data.
