<p align="center">
  <img src="assets/icon/icon.svg" width="128" height="128" alt="ElectronicWerk icon">
</p>

# ElectronicWerk

An open-source ECAD suite for beginners and advanced hobbyists. It covers
schematic, symbol, footprint and layout editing, a live 3D view, ERC/DRC,
manufacturing outputs, online part search, GitHub integration and a full CLI.

> **Status:** early development. Version 0.4 imports KiCad projects,
> libraries and netlists. It builds on the fast 2D view (0.3) and on projects
> (0.2: wizard, project tree, native file format, undo, autosave and crash
> recovery). The editors follow milestone by milestone. See the
> [roadmap](ROADMAP.md) for milestones and progress.

<p align="center">
  <img src="docs/public/screenshots/viewer-pad-selected.png" width="800" alt="A board in the ElectronicWerk 2D view with a selected pad and its properties">
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
- **KiCad import:** KiCad 6 to 10 projects with schematics, hierarchy,
  symbols, board, footprints, zones, rules and project libraries become an
  ElectronicWerk project in one step, with a detailed import report; KiCad
  libraries can be added to a project at any time
- **Netlists** from KiCad (`.net`, XML) and CSV: compare them with a board by
  connectivity and apply them
- An open, **git-friendly file format**: readable JSON, one object per line,
  with schema versions and migrations
- A **CLI** (`ewerk`) to create, inspect, check and archive projects, import
  KiCad projects, libraries and netlists, and a JSON-RPC session for scripts
  and AI agents
- A German and English UI, light and dark appearance, a command palette and
  an update notice
- Builds for Windows, macOS and Linux

## Planned highlights

- Open standard formats (DXF, Gerber, STEP, …)
- A **unified base library** with deduplication, and **online part search**
- A **live 3D view**, optionally in its own window
- **ERC, DRC** and a rule system
- Manufacturing outputs: Gerber X2, Excellon, BOM, pick-and-place, PDF, STEP
- **GitHub integration**: save a project straight to a new or existing
  repository
- A complete CLI and MCP server for scripting and AI agents
- An **assembly assistant**

## License

The application code is licensed under the [MIT License](LICENSE). Library
data derived from third-party libraries keeps its original license (for
example, the KiCad libraries are CC-BY-SA 4.0 with a design exception).
