# Changelog

All notable changes to ElectronicWerk are documented in this file. The format
follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the
project uses [Semantic Versioning](https://semver.org/).

## [Unreleased]

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

[Unreleased]: https://github.com/Ujap1994/ElectronicWerk/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/Ujap1994/ElectronicWerk/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/Ujap1994/ElectronicWerk/releases/tag/v0.1.0
