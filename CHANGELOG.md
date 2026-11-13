# Changelog

All notable changes to ElectronicWerk are documented in this file. The format
follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the
project uses [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.11.0] - 2026-11-13

The online part libraries: parts from **SnapEDA** and **Ultra Librarian**,
the two largest online part libraries, come into the project with their
symbol, footprint and 3D model, checked against the base library. As both
providers' terms require, ElectronicWerk does not search or download on
their websites by itself: the user does, in an isolated provider window or
in their own browser, and ElectronicWerk imports the downloads.

### Added

- **Online part libraries** dialog (**Find parts online** in the command
  palette, **Online parts** in the library manager, **Search online…** in
  the symbol chooser): a part number or keyword opens SnapEDA's or Ultra
  Librarian's search page, in a provider window or in the user's own
  browser. See
  [Online part libraries](https://ujap1994.github.io/ElectronicWerk/guide/online-parts).
- **Provider windows**: each provider in a separate, isolated browser with
  its own persistent profile (the sign-in stays there; ElectronicWerk never
  sees passwords), sandboxed, without access to the app, https only, sign-in
  pop-ups in the same profile. Files downloaded there are offered for
  import at once; other files (datasheets) are saved and the folder named.
- **Import of part downloads** (KiCad ZIPs of SnapEDA, Ultra Librarian or any
  other source, or single `.kicad_sym`, `.kicad_mod` and STEP files): a
  dialog shows the symbols, footprints and 3D models with their pins and
  pads, what the project library has already, and problems (for example a
  KiCad 5-only download). The library name (prefix of the imported names)
  and the target library can be chosen. The import is one undo step; the 3D
  models are copied into `3d/<library name>/` in the project folder and the
  footprints point at them with `${KIPRJMOD}`, so the 3D view and the 3D
  exports find them.
- **Duplicate check against the base library**: every downloaded footprint
  and symbol is compared with the base library (identical, the same pads,
  pads within 0.05 mm; symbols with the same drawing or pins). A footprint
  the base library has can be replaced by the library's (preselected for
  identical ones); the downloaded symbol then uses it.
- **Accounts** in the settings: sign in, sign out (deletes the provider
  window's sign-in and stored data) and the terms of use of each provider.
- `ewerk import archive <file> --project <project> [--use-library]
[--dry-run]` imports a download on the command line; engine methods
  `import.archivePreview` and `import.archive`.

### Changed

- The roadmap's milestone M10 covers SnapEDA and Ultra Librarian only, as
  decided by the maintainer; distributor APIs (Octopart, Digi-Key, Mouser,
  LCSC) are not planned.

### Security

- A download may not unpack outside its folder, have more than 2,000
  entries or unpack to more than 512 MB.

### Tests

- Synthetic downloads laid out like SnapEDA's and Ultra Librarian's
  (`scripts/make-parts-fixture.py`): preview, duplicate check against a
  test base library, import with models, undo and archives with paths
  outside their folder.
- End-to-end tests against a local stand-in website: search opens the
  provider window (without the app's API), a download there is imported,
  a download from the own browser too, and the settings list the accounts.

## [0.10.0] - 2026-11-13

The 3D release: every board can now be looked at in 3D while it is edited —
in a tab or in its own window on a second monitor — and handed to
mechanical CAD as a STEP assembly. The parts' placement is verified against
KiCad 10 and the STEP export is read back with OpenCASCADE in every CI run.

### Added

- **3D view** of every board (the cube in the layout editor's toolbar,
  **3D view** in the board's details, **Show the 3D view** in the command
  palette): the laminate with its holes and cut-outs, the outer copper in
  the colour of the surface finish (gold for ENIG, bright copper for OSP,
  silver for HASL), the transparent solder mask and the silkscreen in the
  stack-up's colours, the solder paste, the plated walls of holes and the
  3D models of the parts. The heights come from the stack-up; parts stand on
  the outer copper of their side. See
  [The 3D view](https://ujap1994.github.io/ElectronicWerk/guide/view-3d).
- **Live updates:** the 3D view follows every edit of its board, undo and
  redo included. Moved, turned and flipped parts are placed again at once
  (about 7 ms for a board with 5,000 parts); the copper, mask and
  silkscreen follow as soon as the edits pause.
- **3D view in its own window** for a second monitor; it closes with the
  project or the main window.
- **Shared selection** between the layout editor, the schematic and the 3D
  view, in every window: a part clicked in 3D is selected in the layout
  editor (and its symbol, with cross-probing on), a part selected in the
  layout is framed in 3D. Shift-, Ctrl- or ⌘-click adds parts.
- **Camera:** isometric view, top, bottom, front, back, left and right,
  fit (**Home**), perspective or orthographic projection, turning, panning
  and zooming towards the pointer.
- **Show** switches for the parts of each side, parts marked “do not
  populate”, silkscreen, solder mask, copper, solder paste, the board and
  the inner copper layers (remembered for all boards).
- **Sections** along x, along y or horizontal, with a position slider and
  the choice of the side to keep; cut solids show their inside.
- **Measuring** between two points on any surface, with the distance and
  its x, y and z parts.
- **Screenshots** of the 3D view as PNG images.
- **3D models of the parts** are found like KiCad finds them: `${KIPRJMOD}`
  and relative paths in the project folder, environment variables, KiCad's
  model variables also in the 3D folders of KiCad installations (macOS,
  Windows, Linux, Flatpak), `.step` files for `.wrl` paths, then the base
  library (downloading its mesh pack the first time). STEP files are
  converted with OpenCASCADE once and cached. The status line counts the
  loaded models and lists the ones that were not found.
- **STEP export** of the board assembly (AP214) for mechanical CAD: the
  board as an exact solid (outline lines and arcs, holes as true cylinders,
  cut-outs; polygons if holes overlap), every part as an instance of its
  model's original STEP file (written once per model), mesh-only models
  (from the downloaded library) as faceted surfaces with their colours.
  See [3D models: STEP, GLB and VRML](https://ujap1994.github.io/ElectronicWerk/guide/export-3d).
- **GLB** (binary glTF 2.0) and **VRML 2.0** exports with the board's
  materials and one shared mesh per model.
- The exports run from the 3D view (**Export**), as a **3D model** output in
  output jobs (format, origin — drill and placement origin, board centre or
  board coordinates —, VRML units, parts not placed; missing models are
  reported in the results) and on the command line with
  `ewerk export-3d <project> [--format step|glb|vrml] [-o <file>]`.
- Engine methods `board3d.scene` (the board meshes as a binary buffer with
  the parts and the found models), `board3d.parts` (placements only, for
  live updates), `board3d.resolve` and `board3d.export`.

### Changed

- The footprint editor's **3D preview** looks models up like the 3D view: a
  STEP file in the project folder or a KiCad installation is shown too, not
  only base library models.
- **KiCad import:** the 3D models of bottom-side footprints get the same half
  turn as their pads, so they sit on their pads (they were turned by 180°
  before). Projects imported earlier keep the old model rotation until they
  are imported again.
- The layout editor always shares its selection with the 3D view; the
  schematic still follows it only with cross-probing on.
- Tessellated STEP files are cached by their content (`step-<hash>.glb`), so
  a changed file is converted again and the exports find the meshes.
- The STEP output kind of output jobs, skipped until now, is the new
  **3D model (STEP, GLB, VRML)** output (`3d/<board>.step` by default).

### Tests

- The placement of every part of the blinky fixture (positions, rotations,
  the bottom side) is compared with KiCad 10's export of the same board
  (`scripts/make-kicad-3d-reference.py`).
- The STEP assembly of the blinky fixture is a reference file; a vitest reads
  it back with OpenCASCADE and checks the board body, its size and origin
  and every part. Locally, with KiCad's own models, the exported parts match
  `kicad-cli pcb export step` triangle for triangle.
- The board body's volume (laminate minus holes), tiled triangulation, GLB
  round trips, VRML transforms and the Part 21 reader are unit-tested.
- Performance budgets in CI: the 3D board of the M board below 1 s, of the L
  board below 5 s, the placements of the L board below 50 ms.
- End-to-end tests open the 3D view from the layout editor, the details and
  the command palette, switch views, projection, layers and sections,
  measure, export STEP, save a screenshot, open the 3D window and check that
  it follows a part turned in the layout editor and shares the selection.

## [0.9.0] - 2026-11-13

The manufacturing release: a board designed in ElectronicWerk can now be
ordered. **Output jobs** generate Gerber X2 and Excellon files, a drill map,
the bill of materials, pick-and-place files, IPC-D-356 and IPC-2581
netlists, PDF and SVG prints and a ZIP package in one click, with presets
for common board makers, and a built-in **Gerber viewer** shows the files as
a board maker reads them. The Gerber and drill output is verified against
KiCad 10 in every CI run.

### Added

- **Output job editor**: output jobs (`outputs/*.ewout`) open in a tab of
  their own from the new **Outputs** button of the project toolbar, the
  project tree or the command palette. The list on the left holds the
  outputs in the order they run, each with a switch and the badge of its
  last result; the settings on the right edit the selected output (name,
  subfolder and every option of its kind); **Add output**, the arrows and
  the bin add, move and remove outputs. Every change is one undo step. See
  [Output jobs](https://ujap1994.github.io/ElectronicWerk/guide/outputs).
- **Generate all** writes every enabled output into the job's output folder
  (`outputs/generated` by default) and lists the results — generated,
  skipped or failed with the reason — with every file; **Open** opens a file
  with its application, the arrow shows it in the file manager.
  **Generate this output** runs a single output. Paths that leave the
  project folder are rejected.
- **Gerber X2** files for every fabrication layer (copper, solder mask,
  paste, silkscreen, board outline): millimetres with six decimals, so
  coordinates are written exactly; standard apertures for circles,
  rectangles and ovals and parameterless aperture macros for every other
  pad shape (rounded and chamfered rectangles, trapezoids, custom pads);
  zone fills as regions with their holes; texts in ElectronicWerk's stroke
  font. **X2 attributes**: file function and polarity, generation software,
  project id, aperture functions (`SMDPad`, `ComponentPad`, `ViaPad`,
  `Conductor`, …) and the net, pin and component of every object. A
  **Gerber job file** (`.gbrjob`) lists the layers, the stack-up and the
  board size. Attributes and the job file can be switched off, and Protel
  file extensions (`.gtl`, `.gbs`, …) chosen, for older CAM systems. Options
  for tented vias, paste for parts that are not placed and values and
  designators on the silkscreen. See
  [Gerber and drill files](https://ujap1994.github.io/ElectronicWerk/guide/gerber-drill).
- **Excellon drill files** in metric decimal format with file and tool
  functions: plated and non-plated holes in separate files or one file,
  blind and buried vias in one file per layer pair, slots as `G85`.
- **Drill map** as PDF or SVG: the board outline and every hole drawn with
  a symbol per drill size, with a legend of the drills (size, count, plated
  or non-plated, slots).
- **Bill of materials** as CSV (UTF-8 with byte order mark, comma,
  semicolon or tab), **Excel workbook** (bold frozen header, filter, fitted
  columns) or **HTML page** (light and dark, prints well): configurable
  columns (item, quantity, designators, value, footprint, symbol,
  description, side, mounting, DNP and any symbol field such as
  `Manufacturer`, `MPN` or `LCSC`) with their own headings, grouping by any
  fields, designators in natural order and an optional list of parts that
  are not placed. The settings show a **live preview** of the first 50
  lines. See [Bill of materials and pick and place](https://ujap1994.github.io/ElectronicWerk/guide/bom-placement).
- **Pick-and-place files** (CPL) with generic, JLCPCB or KiCad columns, in
  millimetres or inches, relative to the board origin or the design origin,
  in one file or one per side, optionally with mirrored x on the bottom;
  footprints marked _Exclude from placement files_ are left out.
- **IPC-D-356** netlist for the electrical test of the bare board (pads,
  vias and their access sides, tented vias), matching KiCad's record by
  record on the test project.
- **IPC-2581** (revision C subset): content, logistic header, bill of
  materials, stack-up, layers, profile, packages, components, nets and the
  features of every layer in one XML file.
- **PDF and SVG prints** of the schematic (every sheet in hierarchy order,
  as the viewer draws it, with frame and title block), of chosen **board
  layers** (on one page or one page per layer, optionally seen from the
  bottom) and **assembly drawings** (one page per side with the outline,
  pads, part outlines and fitted designators, optionally with values), in
  colour or black and white, on paper of the drawing's size (1:1), A4, A3
  or Letter. PDFs have a bookmark per page, and their texts can be searched
  and copied. See [Prints and drawings](https://ujap1994.github.io/ElectronicWerk/guide/prints).
- **ZIP packages** of the files of chosen outputs for upload
  (`{board}-fabrication.zip`).
- **Presets** for board makers — Generic, JLCPCB, PCBWay, AISLER,
  Eurocircuits and OSH Park — that replace the outputs of a job in one
  undoable step (**Apply a preset…**). New projects and new output jobs
  start with the Generic preset. See [Ordering boards](https://ujap1994.github.io/ElectronicWerk/guide/ordering).
- **Gerber viewer**: opens the files of a job (**Gerber viewer** in the
  toolbar of the job) or any Gerber and drill files (**Gerber viewer: open
  Gerber and drill files…** in the command palette) in a read-only tab with
  the navigation, layer panel and **view from the bottom** of the 2D view.
  It reads RS-274X with or without X2 attributes (standard apertures, macros
  with expressions, arcs, regions, polarity, step and repeat) and Excellon
  (metric and inch, decimal and implicit coordinates, slots); layers are
  coloured by file function or extension, and problems are listed in the
  side panel. See [The Gerber viewer](https://ujap1994.github.io/ElectronicWerk/guide/gerber-viewer).
- **Stroke font**: an original monospace stroke font with the metrics of the
  viewer's font, covering ASCII, German letters and common electronics
  symbols (`µ`, `Ω`, `°`, `±`, `²`, …), for silkscreen texts and prints.
- **Command line**: `ewerk outputs run <project> [--job NAME] [--output
NAME …] [--json]` runs an output job, `ewerk outputs list` lists the jobs
  of a project and `ewerk outputs presets` the presets. See
  [Command-line tool](https://ujap1994.github.io/ElectronicWerk/reference/cli#outputs).
- **Engine API**: `outputs.presets`, `outputs.create`, `outputs.applyPreset`,
  `outputs.run`, `outputs.bom`, `outputs.files` and `gerber.scene`. See
  [Engine API](https://ujap1994.github.io/ElectronicWerk/reference/engine-api#output-jobs).
- New crate `ew-output` with every file format, a Gerber and Excellon
  reader and a rasterizer; a recording mode of the scene builder that keeps
  the vector drawing for prints. See
  [ADR-0009](https://github.com/Ujap1994/ElectronicWerk/blob/main/docs/adr/0009-manufacturing-outputs.md).

### Changed

- The fabrication job of new projects now holds the generic preset: Gerber
  X2, drill files, drill map, bill of materials (XLSX), pick and place,
  IPC-D-356, schematic PDF, assembly drawing and a ZIP package. Projects
  without a board get a schematic PDF and a bill of materials. The planned
  STEP output was removed from new jobs; existing STEP outputs are skipped
  until the 3D export arrives.
- Output jobs open from their details (**Open**) and with a double-click in
  the project tree, like sheets and boards; the notice about version 0.9 is
  gone.
- The demo project and the synthetic test boards route with 45° corners
  (straight and diagonal pieces) instead of right angles, like routes drawn
  in the layout editor; the Blinky test board chamfers its supply track.
  The documentation screenshots show no right-angled tracks.
- The KiCad import report suggests writing custom rules from `.kicad_dru`
  again as custom rules, instead of the old notice that the rule editor is
  still to come.

### Tests

- Gerber and drill files of the Blinky test project are compared pixel by
  pixel (20 µm) with the files KiCad 10 writes for the same board: copper,
  solder mask, paste, outline and drill files are identical apart from 2
  pixels of arc approximation. Every pad kind, shape and rotation is
  written, read back and compared with the model's geometry.
- The IPC-D-356 netlist is compared with KiCad's; every preset runs on the
  test project; the engine writes a job into a project folder; end-to-end
  tests generate the outputs of the demo project, check the BOM preview and
  apply a preset with undo.
- The performance budget of the full design rules check of the L board is
  5 s (half the 10 s of the requirement) instead of 3 s, measured as the
  best of two runs after a warm-up: shared CI runners took between 2.1 s and
  3.5 s for the same code, so the old budget failed at random.

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

[Unreleased]: https://github.com/Ujap1994/ElectronicWerk/compare/v0.11.0...HEAD
[0.11.0]: https://github.com/Ujap1994/ElectronicWerk/compare/v0.10.0...v0.11.0
[0.10.0]: https://github.com/Ujap1994/ElectronicWerk/compare/v0.9.0...v0.10.0
[0.9.0]: https://github.com/Ujap1994/ElectronicWerk/compare/v0.8.0...v0.9.0
[0.8.0]: https://github.com/Ujap1994/ElectronicWerk/compare/v0.7.1...v0.8.0
[0.7.1]: https://github.com/Ujap1994/ElectronicWerk/compare/v0.6.2...v0.7.1
[0.6.2]: https://github.com/Ujap1994/ElectronicWerk/compare/v0.5.0...v0.6.2
[0.5.0]: https://github.com/Ujap1994/ElectronicWerk/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/Ujap1994/ElectronicWerk/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/Ujap1994/ElectronicWerk/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/Ujap1994/ElectronicWerk/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/Ujap1994/ElectronicWerk/releases/tag/v0.1.0
