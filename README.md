<p align="center">
  <img src="assets/icon/icon.svg" width="128" height="128" alt="ElectronicWerk icon">
</p>

# ElectronicWerk

An open-source ECAD suite for beginners and advanced hobbyists. It covers
schematic, symbol, footprint and layout editing, a live 3D view, ERC/DRC,
manufacturing outputs, online part search, GitHub integration and a full CLI.

> **Status:** early development. Version 0.8 adds the layout editor:
> interactive routing around obstacles, copper zones, design rules with net
> classes and custom rules, an online design rules check, and updating the
> board from the schematic and back. It builds on the library editors (0.7),
> the schematic editor (0.6), the base library (0.5), the KiCad import (0.4),
> the fast 2D view (0.3) and projects (0.2). Manufacturing outputs come next.
> See the [roadmap](ROADMAP.md) for milestones and progress.

<p align="center">
  <img src="docs/public/screenshots/layout-editor.png" width="800" alt="The ElectronicWerk layout editor with a board, the ratsnest and the design rules check">
</p>

**[Documentation](https://ujap1994.github.io/ElectronicWerk/)** ·
**[Download](https://github.com/Ujap1994/ElectronicWerk/releases/latest)** ·
**[Changelog](CHANGELOG.md)** · **[Contributing](CONTRIBUTING.md)**

## Available now

- An Altium/KiCad-style **project** that holds schematics, PCB, libraries,
  rules and output jobs, with a new-project wizard and templates, a project
  tree, project properties and text variables
- **Never lose work:** unlimited undo/redo, an autosave journal of every
  change, crash recovery and atomic saves
- A fast **2D view** for schematic sheets and boards, drawn by the graphics
  card: smooth pan and zoom (60 fps on a board with 5,000 parts), layers with
  colours and opacity, view from the bottom, click/box/lasso selection and a
  property inspector
- A **schematic editor** in the same view: symbols from the project and base
  libraries, wires with automatic junctions, buses, net, global and
  hierarchical labels, power ports, hierarchical sheets, dragging with
  rubber-banding, copy and paste across sheets, find and replace, net
  highlighting, annotation and an editable inspector; new projects bring
  starter symbols and a guide for a first schematic
- **Library editors** for symbols (pins, units, De Morgan bodies, a pin table
  with bulk edits and pasted pin lists), footprints (all pad kinds and
  shapes, pad arrays, silkscreen, fabrication and courtyard layers, 3D model
  alignment) and components (symbol, footprint, manufacturer, part number,
  suppliers), with **IPC-7351 footprint wizards** (chip, SOIC/QFP, QFN, BGA,
  headers) and KLC-like **library checks**
- A **net navigator**: Ctrl/⌘ + double-click on a net lists every place of it
  on every sheet and jumps there
- A **layout editor**: footprints, **interactive routing** that walks around
  copper of other nets with the clearances of the rules, vias and layer
  changes, dragging tracks, copper **zones** with thermal reliefs, priorities
  and island removal, keep-out areas, board outlines (drawn or **imported
  from DXF**), board presets and a stack-up editor, turning, flipping,
  aligning and an editable inspector
- **Design rules** with net classes, name patterns, board minimums and
  **custom rules** in a small query language (`A.netclass == 'HV' &&
B.netclass != 'HV'`)
- A **design rules check** that runs online while you edit (about 20 ms on a
  500-part board): clearances, shorts, unrouted connections, widths, vias,
  annular rings, holes, board edge, keep-outs, courtyards, silkscreen and
  parity with the schematic, with markers on the board and accepted findings
- **Schematic ↔ board:** update the board from the schematic (new parts,
  footprint and value changes, nets, renamed nets), back annotation of
  designators, values and footprints, and **cross-probing** between sheets
  and board
- An **electrical rules check** with KiCad's configurable pin type matrix,
  unconnected pins, conflicting drivers, missing power and more, a results
  panel that zooms to each problem, and the same results as KiCad on
  imported projects
- **Manufacturing outputs** in **output jobs** (like Altium's OutJobs): one
  click writes **Gerber X2** with net, pad and component attributes and a job
  file, **Excellon** drill files (plated, non-plated, blind and buried, slots),
  a drill map, a configurable **bill of materials** (CSV, XLSX, HTML),
  **pick-and-place** files, **IPC-D-356** and **IPC-2581** netlists, **PDF and
  SVG prints** of schematics, board layers and **assembly drawings**, and a
  ZIP package; **presets** for JLCPCB, PCBWay, AISLER, Eurocircuits and OSH
  Park; the Gerber output is verified pixel by pixel against KiCad 10
- A built-in **Gerber viewer** for Gerber X2/RS-274X and Excellon files from
  any tool
- A **live 3D view** of the board — laminate, copper with its surface finish,
  solder mask, silkscreen and the parts' 3D models — that follows every edit
  at once, in a tab or **in its own window** for a second monitor, with
  camera presets, sections, measuring, screenshots and a selection shared
  with the layout editor; models come from the project, a KiCad installation
  or the base library
- **3D exports:** **STEP** assemblies for mechanical CAD (the board as an exact
  solid with true cylinders for holes, the parts' original STEP models),
  **GLB** and **VRML**, from the 3D view, output jobs or the CLI; part
  placement verified against KiCad 10, the STEP file read back with
  OpenCASCADE in every CI run
- **Exports:** KiCad and CSV netlists, a bill of materials, SVG and a PDF of
  all sheets
- **KiCad import:** KiCad 6 to 10 projects with schematics, hierarchy,
  symbols, board, footprints, zones, rules and project libraries become an
  ElectronicWerk project in one step, with a detailed import report; KiCad
  libraries can be added to a project at any time
- **Netlists** from KiCad (`.net`, XML) and CSV: compare them with a board by
  connectivity and apply them
- A **base library** with the KiCad symbols, footprints and 3D models (about
  45,000 entries): full-text search in milliseconds, filters, 2D and 3D
  previews, adding parts to a project; downloaded on demand or built from a
  local KiCad installation
- **Online part libraries:** parts downloaded from **SnapEDA** and **Ultra
  Librarian** (in an isolated provider window or any browser) are imported
  with symbol, footprint and 3D model, and checked against the base library
- **GitHub and source control** without installing git: new projects on
  GitHub (new or existing empty repository), commit, push, pull, branches,
  history and "Open from GitHub"; sign-in with the browser or a token kept in
  the system keychain. Pulls never merge inside a file, so a design is never
  half merged
- **Duplicate detection** independent of the source: exact duplicates are
  merged, near duplicates (pads within 0.05 mm, same mesh in other colours, …)
  are reviewed side by side, with a reproducible report
- An open, **git-friendly file format**: readable JSON, one object per line,
  with schema versions and migrations
- A **CLI** (`ewerk`) for everything: create projects, place parts and
  connect pins by name (`R1.2`, `U1.VCC`), update, place and route boards, run
  the ERC and DRC with exit codes for CI, generate outputs, import KiCad
  projects and libraries, export 3D models and manage the base library;
  scripts in JSON Lines, a JSON-RPC 2.0 session and an **MCP server** so AI
  agents (Claude, Cursor, VS Code, …) can design boards
- A German and English UI, light and dark appearance, a command palette and
  an update notice
- Builds for Windows, macOS and Linux

## Planned highlights

- Further open standard formats (KiCad export, Specctra, DXF export)
- Push-and-shove routing, differential pairs and length tuning
- **GitHub integration**: save a project straight to a new or existing
  repository
- A complete CLI and MCP server for scripting and AI agents
- An **assembly assistant**
- Later: a **part generator** that reads a datasheet and draws symbol,
  footprint and 3D model

## License

The application code is licensed under the [MIT License](LICENSE). Library
data derived from third-party libraries keeps its original license (for
example, the KiCad libraries are CC-BY-SA 4.0 with a design exception).
