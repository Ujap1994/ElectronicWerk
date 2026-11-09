# Changelog

All notable changes to ElectronicWerk are documented in this file. The format
follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the
project uses [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Fixed

- A long name of the hovered object no longer makes the drawing area wider,
  so fitting the view gives the same zoom wherever the mouse is.

### Changed

- The smoke test of packaged builds also tessellates a STEP model with the
  bundled OpenCASCADE, so every release checks the 3D pipeline on every
  platform.

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

[Unreleased]: https://github.com/Ujap1994/ElectronicWerk/compare/v0.5.0...HEAD
[0.5.0]: https://github.com/Ujap1994/ElectronicWerk/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/Ujap1994/ElectronicWerk/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/Ujap1994/ElectronicWerk/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/Ujap1994/ElectronicWerk/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/Ujap1994/ElectronicWerk/releases/tag/v0.1.0
