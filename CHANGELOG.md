# Changelog

All notable changes to ElectronicWerk are documented in this file. The format
follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the
project uses [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.8.0] - 2026-11-12

The layout release: boards are edited in ElectronicWerk itself. Tracks are
routed interactively around obstacles, copper zones fill with thermal
reliefs, the design rules (net classes, board minimums and custom rules in
a small query language) are checked online while you edit, and schematic
and board stay in sync in both directions.

### Added

- **Layout editor**: the board view becomes an editor with a tool column
  (select, place footprint, route, via, copper zone, keep-out area, board
  outline, text, highlight net), an **active copper layer** selector (Page
  Up / Page Down), turning (R), flipping to the other side (Shift+F),
  locking (L), aligning and distributing footprints, moving (M) and deleting,
  each as one undo step. Clicking a pad keeps the pad in the inspector while
  a drag moves its footprint. See [The layout editor](https://ujap1994.github.io/ElectronicWerk/guide/layout-editor).
- **Inspector editing** of board objects: designator, value, rotation, side
  and lock of footprints, width and layer of tracks, diameter and drill of
  vias, the text of board texts.
- **Interactive routing** in walkaround mode (X): from a pad, track end or
  via, segments at 0°, 45° and 90° follow the cursor and go around copper
  of other nets, non-plated holes, keep-out areas and the board edge with
  the clearances of the rules. Clicks fix segments, V places a via and
  changes to the opposite layer, / swaps the corner, Backspace takes back a
  segment; the target pad snaps to its centre. Track width and via sizes
  come from the net class (or a custom rule). The whole route is one undo
  step.
- **45° corners by default**: routes never bend at right angles (signal
  integrity and EMC), also where they continue a fixed segment or an
  existing track; **Allow 90° corners** in the toolbar permits right angles
  (the direct path is then an L).
- **Dragging tracks** (drag or D): a segment moves sideways and the tracks
  and vias at its ends follow.
- **Copper zones** (Z) with net, layers, priority, clearance, minimum width,
  thermal reliefs (gap and spoke width) or solid pads, island removal and
  hatched fill, edited by double-click; **keep-out areas** (K) that forbid
  tracks, vias, pads, zones or footprints. **Fill all zones** (Shift+B).
- **Ratsnest**: the shortest connections still to route, as dashed lines,
  updated after every change.
- **Board setup**: five **presets** (home etching 1 layer, standard 2 and 4
  layers, fine pitch 6 layers, thin 0.8 mm) that set the stack-up, the board
  minimums and the Default net class; a **stack-up editor** (copper layer
  count, board thickness, layer thickness, material, εr, finish); a
  rectangular **outline** from width and height. The **outline tool** (O)
  draws rectangles or polygons.
- **DXF import** for board outlines and mechanical layers: lines, arcs,
  circles, polylines with bulges, splines and ellipses, units from the file
  or chosen, a choice of DXF layers, replacing the old outline and moving
  the drawing to the origin.
- **Design rules** dialog: **net classes** (clearance, track width, via
  size, differential pair sizes) with name **patterns** (`+*V*` → Power),
  **board minimums** (clearance, track width, via diameter and drill,
  annular ring, hole, hole to hole, copper to edge, silkscreen width),
  **custom rules** with conditions such as `A.netclass == 'HV' &&
B.netclass != 'HV'` (properties of the items A and B, wildcards, units,
  `insideZone('name')`) checked for syntax as you type, and the
  **severity** of every check. Later custom rules win; nothing goes below
  the board minimums.
- **Design rules check** with an online mode (it runs again after every
  change), 25 checks — short, clearance, unrouted, track width, via
  diameter and drill, annular ring, hole size, hole to hole, copper to
  edge, missing or open outline, outside the board, keep-out, courtyard
  overlap, silkscreen on pads, silkscreen width, dangling tracks and vias,
  unfilled zones, duplicate designators, missing and extra footprints,
  pad nets that differ from the schematic, custom rule errors — markers on
  the board, a panel that zooms to each finding and **accepted findings**
  stored with the board. Every finding names where its limit comes from
  (net class, rule, board minimum).
- **Update the board from the schematic** (forward annotation): an ECO
  dialog lists new, removed and changed footprints, designators, values,
  links, new and removed nets, **renamed nets** (tracks keep their
  connection) and pad nets; changes can be left out. New footprints are
  placed in rows below the board. Parts that cannot be placed are listed
  with the reason.
- **Back annotation**: designators, values and footprints changed on the
  board go back to the schematic (also for sheets used more than once).
- **Cross-probing**: selecting symbols selects their footprints on the
  board and the other way round, across tabs.
- Command palette entries for the board commands; the DXF file dialog.
- **Engine API**: `pcb.drc` (full or by region), `pcb.exclude`,
  `pcb.ratsnest`, `pcb.fillZones`, `pcb.unfillZones`, `pcb.rules`,
  `pcb.setRules`, `pcb.validateRule`, `pcb.syncPreview`, `pcb.syncApply`,
  `pcb.backPreview`, `pcb.backApply`, `pcb.crossProbe`, `pcb.edit`,
  `pcb.place`, `pcb.routeStart`, `pcb.route`, `pcb.addTracks`,
  `pcb.presets`, `pcb.applyPreset`, `pcb.dxfPreview`, `pcb.importDxf` and
  `document.object`.
- **File format**: custom rules and DRC severities in the rules document,
  accepted DRC findings in the board, holes in filled zone areas (all
  optional; older files read unchanged).
- New crate `ew-pcb` with exact copper geometry (discs, stadiums, rounded
  polygons), an R-tree of the board, connectivity, the rule engine, the
  DRC, the zone filler (polygon operations with `i_overlay`), the router,
  board edits, the schematic diff, presets and the DXF reader; ADR-0008
  records the design.
- Performance budgets in CI: the full DRC of a 5,000-part board (200,000
  track segments) in 1.5 s, the online check of a 500-part board in 20 ms.
- A **Release snap** workflow adds a missing snap package to a published
  release (built from the tagged source, smoke-tested, with updated
  checksums). The release workflow skips the snap when the Snap Store is
  unreachable, as it was for the Linux x64 build of 0.7.1.
- Documentation: seven new guide pages (layout editor, schematic and board,
  board setup and outline, routing, zones, design rules, DRC) with
  screenshots, keyboard shortcuts of the layout editor, the engine API and
  file format references.

### Changed

- The view toolbar wraps onto a second row when it does not fit, instead of
  covering the side panels.
- Boards are no longer read-only in the inspector.

### Fixed

- Tests of the command-line tool ignore exit codes explicitly (a new lint
  of Rust 1.100 failed the build).

## [0.7.1] - 2026-11-12

The library release: symbols, footprints and components are drawn and
maintained in ElectronicWerk itself, with footprint wizards after IPC-7351
and library checks in the spirit of KiCad's library conventions. The
schematic editor gains a net navigator.

Version 0.7.0 was tagged but never published: its Linux builds could not
reach the Snap Store (see **Fixed**). 0.7.1 is the same release.

### Added

- **Library editor**: a project library opens in a tab next to sheets and
  boards, with **Symbols**, **Footprints** and **Components**, a search,
  **New**, **Duplicate**, **Rename** (every reference in the project follows)
  and **Delete** (it tells you where the item is still used). Every change is
  one undo step.
- **Symbol editor**:
  - drawing tools for pins, lines, rectangles, circles, polylines and texts,
    with snapping, moving, rotating, mirroring, duplicating and deleting;
  - pins with number, name, all twelve electrical types, nine graphic styles,
    direction, length and a hidden state; new pins count up by themselves
    and <kbd>R</kbd> turns the next pin;
  - a **pin table** that shares the selection with the drawing: edit cells in
    place, change type, style, direction, length or visibility of all
    selected pins at once, renumber them (`1, 2, 3 …`, `A1, A2 …`, with a
    step), delete them;
  - **Paste pin list**: pins from a datasheet table or a spreadsheet (number,
    name, type; KiCad's type letters work too);
  - **units** (add a copy, remove, choose) and **De Morgan bodies** per unit;
  - symbol properties: description, keywords, prefix, default value and
    footprint, datasheet, power symbol, pin number and name visibility, pin
    name offset, custom fields.
- **Footprint editor**:
  - pads of every kind (SMD, through-hole, non-plated hole, edge connector)
    and shape (circle, rectangle, oval, rounded rectangle, trapezoid,
    custom), with size, rotation, drill and slot, layers, mask and paste
    margins; a new pad copies the selected one;
  - a **pad table** with bulk edits and renumbering, and **pad arrays**
    (rows × columns with pitches and the first number);
  - graphics and texts on the silkscreen, fabrication, courtyard and copper
    layers with the usual line widths;
  - **Draw a courtyard around pads and body** with a margin;
  - **3D models**: path, offset, rotation, scale, hidden, and a **3D preview**
    for models of the base library that follows every change of the values;
  - footprint attributes (mounting, BOM, placement file, do not populate).
- **Footprint wizard** after the IPC-7351 land equations with three density
  levels (least, nominal, most) and a live preview: chips (0402 to 2512),
  gull-wing packages (SOIC, SSOP, TSSOP, LQFP), no-lead packages (DFN, QFN
  with exposed pad and reduced paste), BGAs (JEDEC row letters) and pin
  headers; 23 presets and KiCad-style names.
- **Component editor**: symbol, footprint, value, manufacturer, part number,
  datasheet, suppliers with order numbers and links, custom fields, a
  preview of symbol and footprint and a pin-to-pad table. Components appear
  in the schematic's symbol chooser and are placed with their value,
  footprint and order data as fields.
- **Library checks**: 30 checks for symbols, footprints and components
  (pins off the grid, duplicate pin numbers, hidden power pins, pads outside
  the courtyard, silkscreen over pads, SMD pads without paste, pins without
  pads, …) as errors, warnings and notes, in the item list, a **Checks** tab
  with **Show**, the wizard and the component editor, and for the whole
  library.
- **Net navigator** in the schematic editor: <kbd>Ctrl</kbd>/<kbd>⌘</kbd> +
  double-click on a label, power port, wire or pin lists every place of its
  net on every sheet (labels, power ports and symbols, sheet pins, component
  pins), grouped by sheet; a click jumps there, also to another sheet, and
  previous/next step through the list. **Show all places** in the status
  line reopens it for the highlighted net.
- Alternate body styles (De Morgan) in the data model: a symbol unit can have
  a second body, and a placed symbol chooses it (`alternate`). The KiCad
  import reads body style 2 of symbols and placed symbols instead of
  dropping it.
- Library components in the data model (`components` in a library).
- Engine API: `libedit.list`, `libedit.item`, `libedit.create`,
  `libedit.delete`, `libedit.rename`, `libedit.setProps`, `libedit.symbol`,
  `libedit.footprint`, `libedit.check`, `libedit.wizard`, `libedit.presets`,
  `view.item`, `library.model`, `schematic.components`; `schematic.place`
  takes a `component`, and `schematic.net` replies with the net's `places`.
- Documentation: new guide pages for the library editor, symbols,
  footprints, the footprint wizard, components and library checks, the net
  navigator, new keyboard shortcuts, the engine API and the file format, and
  ADR-0007.

### Fixed

- Release builds: the Linux builds wait up to about ten minutes for the Snap
  Store and, if it stays unreachable, publish every other Linux package
  without the snap instead of stopping the whole release.

## [0.6.2] - 2026-11-09

The schematic release: sheets become editable, with an electrical rules
check that gives the same answers as KiCad, net highlighting, find and
replace, annotation, exports and a guide for a first schematic.

Versions 0.6.0 and 0.6.1 were tagged but never published: their release
smoke tests failed on Windows and on Intel Macs (see **Fixed**). 0.6.2 is
the same release with both fixes.

### Added

- **Schematic editor** in the sheet view: a tool palette for symbols, wires,
  buses, net, global and hierarchical labels, power ports, no-connect flags,
  junctions, texts and sheets, with grid snapping and live previews;
  selecting, dragging with rubber-banding (straight wires get a corner,
  labels and power ports move along), moving, rotating, mirroring, deleting,
  copy, cut, paste and duplicate across sheets; an editable inspector
  (designator, value, footprint, parameters, flags, label kinds, power port
  symbols, texts, sheet names); renaming by double-click; every edit is one
  undo step. See [Drawing schematics](https://ujap1994.github.io/ElectronicWerk/guide/schematic-editor).
- Automatic junctions: added where three or more connections meet and
  removed when no longer needed; crossing wires never get one.
- The symbol chooser lists the project library and searches the base
  library; a chosen base-library symbol is copied into the project.
- Starter symbols in every new project (R, C, C_Polarized, L, D, LED,
  Q_NPN_BCE, Conn_01x02, Conn_01x03, SW_Push, PWR_FLAG).
- Hierarchical sheets: draw a sheet symbol, add its pins from the
  hierarchical labels of the sheet, open it from the inspector.
- Annotation: all sheets or one, only unannotated or all, rows or columns,
  optionally starting each sheet at its number × 100; multi-unit parts and
  reused sheets are handled.
- Find and replace across all sheets; net highlighting across sheets.
- The ERC panel below the drawing: filters, automatic updates after changes,
  click to zoom to a problem on any sheet, fixes for unannotated symbols and
  wire ends off the grid; check settings with severities and a clickable pin
  type matrix. See [Electrical rules check](https://ujap1994.github.io/ElectronicWerk/guide/erc).
- Exports: KiCad netlist, CSV netlist, bill of materials (CSV), SVG of a
  sheet and a PDF of all sheets. See
  [Schematic exports](https://ujap1994.github.io/ElectronicWerk/guide/schematic-exports).
- Beginner aids: a guide for the first schematic that ticks itself off,
  tooltips and status hints for every tool, and a grid and snapping
  assistant that finds and fixes wire ends off the grid.
- Schematic commands in the command palette.
- Engine methods `schematic.*` (erc, ercSettings, setErcSettings, net,
  symbols, symbol, place, wire, add, move, dragInfo, orient, delete, copy,
  paste, annotate, find, replace, syncSheetPins, snapToGrid, netlist, bom);
  `document.add` takes the position and size of a new sheet symbol.
- New crate `ew-schematic`: the connectivity of all schematics across sheets
  and the hierarchy (wires, junctions, local, global and hierarchical labels,
  power symbols, sheet pins, one instance per sheet placement) with KiCad's
  net naming. On a real 15-sheet project, all 480 connected nets match the
  netlist KiCad exports.
- Electrical rules check (ERC) in `ew-schematic`: KiCad's pin type matrix,
  unconnected pins, conflicting drivers, power inputs without a source,
  undriven inputs, dangling wires, labels and no-connect flags, unmatched
  hierarchical labels and sheet pins, net name conflicts, duplicate and
  missing designators (multi-unit aware), missing symbols and sheets, and
  wire ends off the grid. Every check's severity and every matrix cell can be
  changed in the rules document (`erc`, stored only where it differs from
  the defaults). On the demo project and a real project it reports the same
  errors as KiCad 10.
- The KiCad import takes over the ERC settings of the `.kicad_pro` file
  (check severities and pin type matrix).
- Placed symbols resolve their designator per sheet instance
  (`instances`, keyed by the path of sheet symbol ids).

### Fixed

- The release smoke test starts the 3D mesh check only once and gives it
  three minutes: under Rosetta, several checks started at the same time
  and compiled OpenCASCADE in parallel until the test timed out.
- 3D previews in packaged builds: OpenCASCADE is always loaded as CommonJS,
  also when the app folder lies below a `package.json` that declares ES
  modules (this made the Windows release smoke test fail).
- A long name of the hovered object no longer makes the drawing area wider,
  so fitting the view gives the same zoom wherever the mouse is.
- KiCad import: symbols that are mirrored and turned by 90° or 270° were
  placed with their pins swapped (KiCad turns before it mirrors).

### Changed

- The smoke test of packaged builds also tessellates a STEP model with the
  bundled OpenCASCADE, so every release checks the 3D pipeline on every
  platform.
- Sheets are no longer read-only; boards stay read-only until the layout
  editor.
- The dotted grid of a sheet starts at the top-left page corner (as in
  KiCad), so imported drawings lie on it.
- CI caches the Electron download and retries it when GitHub's release
  downloads time out.

## [0.5.0] - 2026-11-08

The library release: the symbols, footprints and 3D models of the KiCad
libraries in one searchable base library, with 2D and 3D previews and a
deduplication engine that works for any source.

### Added

- **Library manager** (start screen, project toolbar, **View → Library…**,
  <kbd>Ctrl</kbd>/<kbd>⌘</kbd>+<kbd>Shift</kbd>+<kbd>L</kbd>, command
  palette): full-text search over symbols, footprints and 3D models with
  filters (kind, library, mounting, with 3D model, duplicates, merged
  entries), a virtualized result list for tens of thousands of entries, and
  the details of every entry.
- **Previews:** symbols (with unit selection) and footprints in 2D with the
  sheet and board engine; 3D models and footprints with their model on a
  board in 3D (three.js), placed with the footprint's offset, rotation and
  scale.
- **Add to project library:** copies symbols (with their default footprint)
  and footprints into the project library as one undoable step.
- **Deduplication engine** (`ew-library`), independent of the source:
  canonical fingerprints of footprints (pads and graphics, centred and
  turned, 1 µm), symbols (pins and body) and 3D meshes; exact duplicates
  (merged), near duplicates (pads within 0.05 mm, same pads with other
  graphics, same name with another drawing, same mesh in other colours) and
  aliases; a **Duplicates** tab to review groups side by side; decisions
  kept in `decisions.json` across rebuilds; a reproducible report
  (`dedup-report.json`, `dedup-report.md`).
- **Library setup:** download the KiCad library from a GitHub library
  release (index first, 3D models per 3D library on demand, every file
  checked against its SHA-256 checksum) or build it from a local KiCad
  installation or library checkouts in a few seconds.
- **3D models:** STEP models are tessellated to GLB with OpenCASCADE
  (WebAssembly) — in bulk for library releases, on demand in a worker thread
  for local libraries — and cached.
- Commands `ewerk library build`, `info`, `search`, `duplicates`, `pack` and
  `install`; engine methods `library.*`.
- **Library** workflow: builds the library from the official KiCad
  libraries (GitLab) and publishes it as a separate release
  (`library-kicad-<version>-r<run>`, CC-BY-SA 4.0 with the KiCad libraries
  exception).
- Documentation: The base library, Duplicates, Building the base library,
  the `ewerk library` commands and `library.*` methods, ADR-0005.

### Performance

- Every search over the KiCad 10 libraries (45,643 entries) answers in
  0.1–15 ms (Apple M3); the CI budget checks 50,000 synthetic entries.
- The index builds in 6.6 s; all 7,257 STEP models tessellate in 7.4 minutes
  on eight cores. The packs are 42 MB (index) and 220 MB (3D models, 105
  packs).

## [0.4.0] - 2026-11-08

The KiCad release: KiCad projects become ElectronicWerk projects in one step,
with a detailed import report, and netlists from KiCad and other tools can be
compared with a board and applied.

### Added

- **KiCad project import** (KiCad 6 to 10; KiCad 10 is the reference) from
  the start screen, **File → Import KiCad Project…** or the command palette.
  The import dialog takes a project name and location; the KiCad files are
  only read.
  - Schematics with their hierarchy, placed symbols (fields, units,
    mirroring), wires, buses, bus entries, junctions, no-connect flags, local,
    global and hierarchical labels, texts, graphics and title blocks.
  - Symbols from the design and from the project's own libraries, with units,
    pins, pin name and number settings, and derived symbols.
  - The board: stackup, footprints (every pad shape, drills and slots,
    clearances, graphics, texts, attributes, 3D model references), tracks and
    arc tracks, vias, zones with their filled areas, keep-outs, graphics,
    texts and the outline.
  - Net classes, assignments, constraints and text variables from the
    `.kicad_pro` file; project libraries from the library tables.
  - Symbols and footprints are linked as components.
- **Import report** with counts, findings by severity (error, warning, note),
  21 stable codes and translated titles, also written into the project as
  `kicad-import-report.md`.
- **KiCad library import** into a project library from the project tree:
  `.kicad_sym`, `.kicad_symdir`, `.pretty` and `.kicad_mod`; existing names
  are skipped, and the import is one undoable step.
- **Netlists**: KiCad `.net`, KiCad XML and CSV (with header detection) are
  compared with a board by connectivity, with a table of differences, and
  can be applied to the board as one undoable step (**Import netlist…** in
  the board's context menu or the command palette).
- Commands **`ewerk import kicad`**, **`ewerk import library`** and
  **`ewerk import netlist`** (exit code 1 if the board differs from the
  netlist), and engine methods `import.kicad`, `import.kicadProbe`,
  `import.library` and `import.netlist`.
- The file format keeps the **filled areas of zones** (`filled`) and the
  **pin text settings of symbols** (`pinText`); both are optional, so the
  format stays at version 1.
- A synthetic KiCad 10 test project (`crates/ew-kicad/tests/fixtures/blinky`,
  generated by `scripts/make-kicad-fixture.py`), used by the tests and the
  documentation, and local-only regression tests for real projects and the
  official libraries.
- Documentation: [Importing KiCad projects](https://ujap1994.github.io/ElectronicWerk/guide/import-kicad),
  [Netlists](https://ujap1994.github.io/ElectronicWerk/guide/netlists),
  the import commands and methods in the references, and
  ADR-0004 (KiCad import).

### Changed

- The 2D view draws filled zones solid (and unfilled ones hatched), honours
  hidden pin names and numbers, puts pin names above the pin line when their
  offset is 0, and keeps texts upright when a symbol or footprint is turned
  by 180°.

### Performance

- A real 4-layer, multi-sheet KiCad 10 project (480 nets) imports in 0.08 s,
  and all official KiCad libraries (22,916 symbols, 15,470 footprints) are
  read in 5.6 s (Apple M3).

## [0.3.0] - 2026-11-08

The viewer release: schematic sheets and boards open in a fast 2D view drawn
by the graphics card, with layers, selection and a property inspector.

### Added

- **2D view** for schematic sheets and boards. Documents open in tabs from the
  project tree (double-click, <kbd>Enter</kbd>, **Open** in the context menu)
  or the details panel, and remember their view while the project is open.
- **Navigation** with mouse (wheel zoom around the cursor, middle/right-drag
  or <kbd>Space</kbd>+drag to pan), trackpad (two-finger pan, pinch zoom) and
  keyboard (arrows, <kbd>+</kbd>/<kbd>-</kbd>, <kbd>Home</kbd> to fit,
  <kbd>F</kbd> to zoom to the selection); animated zoom, a grid that adapts
  to the zoom level, and the cursor position in the project's units.
- **Selection** by click (pads before tracks before footprints before zones,
  objects on the active layer first), <kbd>Shift</kbd>/<kbd>Ctrl</kbd>/<kbd>⌘</kbd>
  to add or toggle, **box selection** (left to right: inside; right to left:
  touching), **lasso selection**, select all and hover highlighting.
- **Inspector** with the read-only properties of tracks, vias, footprints,
  pads, zones, texts, graphics, symbols, pins, fields, wires, buses, labels,
  power ports and sheets, in the project's units.
- **Layer panel** for boards: visibility, colours, active layer, opacity,
  _Show all_ / _Only active_, highlighting of the active layer, and a **view
  from the bottom**. Sheets get a **Display** panel for wires, labels,
  symbols, pins, fields, the title block and more.
- Sheets show the paper, a frame and a title block with resolved text
  variables, and follow the light or dark theme.
- Engine methods **`view.scene`** (everything the view draws, as JSON
  metadata plus one binary buffer) and **`view.object`** (inspector
  properties); `ewerk bench` also measures the board scene.
- A viewer benchmark (end-to-end, in CI on macOS) and a scene budget test.

### Changed

- The synthetic designs (and the demo project template) point net labels
  away from their symbols and keep every row on the sheet.
- Without a usable graphics card, the app falls back to Chromium's software
  WebGL renderer.

### Performance

- The L reference design (5,000 parts, about 590,000 primitives) opens in the
  view in 0.2 s and pans and zooms at 60 fps with about 0.2 ms of CPU time per
  frame (Apple M3); the XL design (2.3 million primitives) also keeps 60 fps.

## [0.2.0] - 2026-11-08

The project release: create, organise, save and archive projects with
schematics, a board, a library, design rules and output jobs, stored in an
open, git-friendly file format, with unlimited undo, an autosave journal and
crash recovery.

### Added

- **Projects** (Altium/KiCad style): a project folder with a `.ewproj` file,
  schematic sheets, a board, a project library, design rules and output jobs.
- **New-project wizard** with name, location and storage, five templates
  (empty, 2-layer board, 4-layer board, Arduino-style shield, demo project)
  and board settings (size, copper layers, thickness, units).
- **Project workspace** with toolbar, **project tree** (sheet hierarchy,
  documents by kind, other and generated files, context menus, in-place
  rename, keyboard navigation) and details for the project, sheets (paper,
  title block), the board (stack-up), design rules, output jobs and files.
- **Project properties** and **text variables** (`${NAME}`, with the built-in
  `PROJECT_NAME`, `REVISION`, `CURRENT_DATE`, `SHEET_NAME`, `FILENAME`).
- **Open**, **Open Recent** (with "Not found" and "Unsaved changes" badges),
  **Save**, **Save As**, **Save a Copy**, **Archive** (ZIP), **Close** and
  **Check project**, in the menus, the toolbar and the command palette.
  Opening `.ewproj` files from the file manager or the command line.
- **Unlimited undo and redo** (limited only by memory) for every change.
- **Autosave journal** and **crash recovery**: after an engine crash the
  project is reopened and restored automatically; after a crash of the whole
  app, opening the project offers to restore the unsaved work.
- A prompt about unsaved changes when closing a project or the window.
- Documents created from templates are named in the language of the user
  interface, and the findings of **Check project** are shown in German or
  English.
- **Native file format**: deterministic, diff-friendly JSON with integer
  nanometre coordinates, UUIDs, schema versions, a migration framework,
  atomic multi-file saves and validation.
- `ewerk project new|info|validate|archive|upgrade|templates`, `ewerk rpc`
  (line-based JSON-RPC session), `ewerk synth` (synthetic S/M/L/XL reference
  designs) and `ewerk bench`.
- Engine API: 26 new methods for projects, documents and editing, plus
  change-set events (see the Engine API reference).
- Performance budgets in CI: the synthetic L design (5,000 parts, 200,000
  track segments) opens in well under 3 s and saves in under 1 s.
- Documentation: Projects, Project tree and properties, Undo/autosave/recovery,
  File format, Benchmarks, and an updated CLI and Engine API reference;
  ADR-0002 (data model, file format and transactions).

## [0.1.0] - 2026-11-07

The foundation release: the secure application shell, the Rust engine, the
quality gates and the delivery pipeline that every later feature builds on.

### Added

- Desktop app shell (Electron) with secure defaults: sandboxed renderer,
  context isolation, a strict Content Security Policy and a custom `app://`
  scheme.
- Single-instance lock; closing the window quits the app on every platform.
- Rust engine (`ew-core`) running in a crash-isolated `utilityProcess`, with
  automatic restart and a direct MessagePort channel to the UI.
- `ewerk` command-line interface with `info`, `methods` and `call`.
- Start screen, status bar with live engine status, and a settings dialog
  (language and appearance).
- German and English UI, following the system language by default and
  switchable at runtime.
- Light and dark appearance, following the system by default.
- Lint gates with zero warnings (ESLint strict type-checked, TypeScript strict,
  Stylelint, markdownlint, Prettier, rustfmt, Clippy pedantic, cargo-deny,
  actionlint, commitlint) and a custom rule against hard-coded UI text.
- Test stack: Rust unit tests, Vitest unit and integration tests, and
  Playwright end-to-end tests for Electron.
- CI on Windows, macOS and Linux.
- Project icon (SVG master) with generated raster files for the window, Dock,
  taskbar, installers, favicon, README, documentation and release pages.
- Update notice: compares the installed version with the newest GitHub release,
  shows the release notes, offers "Skip this version" and opens the download
  page. Nothing is ever installed automatically. Can be turned off in the
  settings or run manually from the menu.
- Command palette (Cmd/Ctrl+K or Cmd/Ctrl+Shift+P) with commands for settings,
  language, appearance, updates and help.
- Packaged-app smoke test (`--smoke-test`) for the release pipeline.
- Release packages for Windows (installer and portable, x64 and arm64), macOS
  (dmg and zip for universal, Apple Silicon and Intel) and Linux (AppImage,
  deb, rpm, pacman, tar.gz, Snap and Flatpak, x64 and arm64), plus `ewerk`
  command-line archives and `SHA256SUMS.txt`.
- Documentation site with guides for installation, first start, settings,
  command palette, updates, keyboard shortcuts and troubleshooting, and
  references for the CLI, the engine API, the architecture and the release
  process ([ujap1994.github.io/ElectronicWerk](https://ujap1994.github.io/ElectronicWerk/)).
  **Help → Documentation** opens it.

[Unreleased]: https://github.com/Ujap1994/ElectronicWerk/compare/v0.7.1...HEAD
[0.7.1]: https://github.com/Ujap1994/ElectronicWerk/compare/v0.6.2...v0.7.1
[0.6.2]: https://github.com/Ujap1994/ElectronicWerk/compare/v0.5.0...v0.6.2
[0.5.0]: https://github.com/Ujap1994/ElectronicWerk/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/Ujap1994/ElectronicWerk/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/Ujap1994/ElectronicWerk/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/Ujap1994/ElectronicWerk/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/Ujap1994/ElectronicWerk/releases/tag/v0.1.0
