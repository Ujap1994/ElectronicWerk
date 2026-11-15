# ADR-0013: Assembly assistant, panels, statistics and documentation

- **Status:** accepted
- **Date:** 2026-11-15
- **Milestone:** M13

## Context

M13 asks for an **assembly assistant** for soldering boards by hand (board
view and BOM, the next parts highlighted, grouped by value and side,
checkboxes, a large-display mode for the workbench, progress saved in the
project) and for extras: a **panelization helper**, **design statistics**
and a **documentation generator**.

All four read the design; only the assistant keeps state of its own, and
only the panel changes what the manufacturing outputs contain.

## Decision

### Assembly assistant

- **Groups from the BOM.** The engine (`ew-core::assembly`) groups the
  fitted parts of a board by side, mounting type, value, footprint and part
  number — the parts of one group come from one reel or bag. DNP parts and
  schematic parts without a footprint are listed, not grouped.
- **Order.** SMD parts on both sides before through-hole parts (these keep
  the board from lying flat), the chosen first side before the other, and
  within that by footprint size (default), value, group size or designator.
- **Progress outside the documents.** Ticking a part off is not a design
  change: it must not create undo steps, mark the project as unsaved or
  conflict with a teammate's edits. The progress lives in `assembly.json` in
  the project folder (per board: placed designators and finished builds)
  and is written atomically at once. An `assembly` engine event keeps every
  window in step.
- **View.** The assistant reuses the board viewer (`view.scene`): only the
  current side's layers, the current group highlighted, parts of later
  groups dimmed (a new shader state bit), the board flipped for the bottom
  side. The same component runs in a tab or in its own window (for a second
  monitor; full screen with F11), with a workbench mode that enlarges text
  and buttons and keyboard control (Space, arrow keys).

### Panels

- **A panel is a board in memory.** `ew-pcb::panel::panelize` copies the
  board (`edit::translated`) into a grid, adds rails, tabs with mouse-bite
  holes or V-score lines, fiducials and tooling holes, and computes the
  outline as the union of boards, rails and tabs. Every board output (Gerber,
  drill, drill map, pick and place) then runs unchanged on that board — no
  output needs to know about panels.
- **Part of the output job.** The settings are stored in the `.ewout` file
  (`panel`, optional): a panel is a manufacturing decision, often different
  per board maker, not part of the design. Outputs that describe the design
  (BOM, prints, netlists, STEP, documentation) stay for one board.
- **Special footprints.** Fiducials, tooling holes and mouse bites are
  locked footprints with mounting _other_, excluded from BOM and placement;
  V-score lines go to the user layer 1 (`User_1` Gerber file).
- **Limits.** V-score needs rectangular boards; for other outlines, mouse
  bites put the tabs on the bounding box and say so. Warnings and problems
  carry codes, so the app shows them translated.

### Statistics and documentation

- **Statistics** are computed in `ew-output::statistics` from the model
  (schematic connectivity, BOM parts, board items, the ratsnest for routing
  completion) and shared by the project details, `ewerk project stats`,
  `project.statistics` and the documentation.
- **Documentation** is an output kind (`documentation`) that writes one
  self-contained HTML file: overview, sheets, board views, assembly drawings,
  stack-up, BOM, holes, statistics. Drawings are the SVG prints of the PDF
  output, embedded as data URIs; the file needs nothing else and prints with
  a page per section. It is in English, like the other generated files.

## Consequences

- The assistant works on any board without preparation, and its progress
  can be committed or ignored in source control.
- Panels need no editor of their own; the price is that a panel cannot be
  edited by hand (custom tab positions, a mixed panel of different boards).
  That can come later as a panel document if users need it.
- The documentation follows every improvement of the prints automatically.
- Making the documentation exposed that schematic prints did not resolve
  `${SHEET_NAME}`; the print pages now resolve variables per sheet.
- The synthetic demo design was improved on the way: parts stay inside the
  board outline and pin headers are through-hole parts.
