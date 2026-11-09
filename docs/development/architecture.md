# Architecture

This page summarizes how ElectronicWerk is built. The binding decisions are
recorded as Architecture Decision Records (ADRs) in the repository, starting
with [ADR-0001: Application architecture](https://github.com/Ujap1994/ElectronicWerk/blob/main/docs/adr/0001-architecture.md).

## Overview

```text
┌──────────────────────── ElectronicWerk desktop app ────────────────────────┐
│                                                                            │
│  Renderer (React UI, WebGL2) ◄── MessagePort (JSON, buffers) ─► Engine host │
│        ▲                                                       (utility    │
│        │ IPC: settings, status, updates                         process)   │
│        ▼                                                          │        │
│  Main process (Electron): windows, menus, settings,           Node-API     │
│  single instance, security, update check, engine lifecycle        │        │
│                                                                   ▼        │
│                                                   Rust engine (ew-core)    │
└────────────────────────────────────────────────────────────────────────────┘
                     ewerk CLI ──────────► Rust engine (ew-core)
```

| Part           | Technology                | Responsibility                                                                                                                        |
| -------------- | ------------------------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| Main process   | Electron, TypeScript      | Windows, menus, settings, security policy, update check, starting and restarting the engine                                           |
| Renderer       | React, TypeScript, WebGL2 | The user interface and the 2D view. It has no access to Node.js or the file system                                                    |
| Preload script | TypeScript                | The only bridge: a small, typed API (`window.ew`)                                                                                     |
| Engine host    | Electron `utilityProcess` | Loads the native engine and serves requests                                                                                           |
| Engine         | Rust (`crates/ew-*`)      | Data model (`ew-model`), file format (`ew-format`), sessions, transactions, undo and journal (`ew-core`); later all design algorithms |
| CLI            | Rust (`crates/ew-cli`)    | The same engine for terminals, scripts and AI agents                                                                                  |

## Projects, transactions and recovery

The engine holds the open project. Every edit is a transaction of reversible
changes: it can be undone and redone, is appended to an autosave journal on
disk, and is announced to every window as a change set. If the engine
process crashes, the app restarts it, reopens the project and replays the
journal, so no work is lost. The design is recorded in
[ADR-0002: Data model, file format and transactions](https://github.com/Ujap1994/ElectronicWerk/blob/main/docs/adr/0002-data-model-and-file-format.md);
the files are described in the [file format reference](../reference/file-format).

| Crate          | Responsibility                                                                  |
| -------------- | ------------------------------------------------------------------------------- |
| `ew-model`     | Data types: project, documents, objects, units, layers                          |
| `ew-format`    | JSON layout, schema versions and migrations, atomic saves, validation, archives |
| `ew-synth`     | Synthetic reference designs for tests and [benchmarks](./benchmarks)            |
| `ew-scene`     | Render scenes for the 2D view: primitives, texts and the object table           |
| `ew-kicad`     | KiCad import: S-expression parser, projects, libraries, netlists, import report |
| `ew-library`   | Base library: SQLite/FTS5 index, fingerprints, deduplication, library packs     |
| `ew-schematic` | Schematic logic: connectivity, ERC, editing operations, annotation, exports     |
| `ew-core`      | Sessions, transactions, undo/redo, journal, events, RPC                         |
| `ew-node`      | Node-API bridge for the desktop app                                             |
| `ew-cli`       | The `ewerk` command-line tool                                                   |

## 2D rendering

The [2D view](../guide/viewer) splits the work between the engine and the
GPU, as recorded in
[ADR-0003: 2D rendering engine](https://github.com/Ujap1994/ElectronicWerk/blob/main/docs/adr/0003-2d-rendering.md):

1. `view.scene` (crate `ew-scene`) converts a sheet or board into a **scene**:
   per layer, instanced primitives (line segments, arcs, discs and rings,
   rounded rectangles, triangle meshes) and texts, plus a table of every
   selectable object with its bounding box, parent, net and id. Footprints and
   symbols are placed, pads expanded, polygons triangulated, and the
   primitives of each layer sorted along a Z-order curve and grouped into
   chunks of 1,024 with bounding boxes.
2. The scene arrives as one binary buffer next to the JSON metadata. The
   renderer uploads it to the GPU as it is.
3. Every frame, the renderer draws the visible chunks of each layer with one
   instanced draw call per kind. Fragment shaders compute the exact shape
   (signed distance fields), so edges stay sharp at every zoom level.
   Multisampling turns the edge coverage into anti-aliasing; a depth value
   per layer makes each pixel blend only once per layer, so layers can be
   transparent without overlapping shapes darkening each other.
4. Hit-testing runs in the renderer: a packed R-tree (Flatbush) over the
   object boxes finds candidates, and exact distance tests on the drawn
   primitives decide. Selection and hover are a per-object flag texture that
   the shaders read, so changing the selection never re-uploads geometry.
5. When the project changes, the view asks for a new scene. Incremental
   updates follow with the editors.

## Why a separate engine process?

- **Stability:** if native code crashes, only the engine process ends. The main
  process starts a new one (at most three times within a minute), the window
  stays open and shows a notice.
- **Performance:** the engine is native Rust and can use every processor core.
  The renderer talks to it directly over a `MessagePort`, without a detour
  through the main process.
- **One API:** the app and the CLI call the same engine methods, so every
  feature is scriptable ([Engine API](../reference/engine-api)).

## Security

- The renderer runs in Chromium's sandbox with context isolation and without
  Node.js integration.
- The user interface is loaded from a custom `app://` scheme with a strict
  Content Security Policy instead of from `file://`.
- Every IPC handler checks that the request comes from ElectronicWerk's own
  user interface.
- New windows, embedded web views, navigation to other sites and permission
  requests (camera, microphone, location, …) are denied.
- Links open in the browser only if they point to this project on GitHub.
- Packaged apps use hardened Electron fuses (for example, no "run as Node.js"
  mode and integrity checks for the application archive).

## Code quality

Every change passes the same gates locally (Git hooks) and in CI on Windows,
macOS and Linux:

- **Lint with zero warnings:** ESLint (strict, type-checked), TypeScript strict
  mode, Stylelint, markdownlint, Prettier, rustfmt, Clippy (pedantic),
  cargo-deny, actionlint and commitlint. A custom rule forbids hard-coded UI
  text, so every text exists in German and English.
- **Tests:** Rust unit tests, Vitest unit and integration tests (including the
  real native engine) and Playwright end-to-end tests of the running app.

## KiCad import

The [KiCad import](../guide/import-kicad) is a one-way conversion in the crate
`ew-kicad`, as recorded in
[ADR-0004: KiCad import](https://github.com/Ujap1994/ElectronicWerk/blob/main/docs/adr/0004-kicad-import.md):

1. A non-recursive S-expression parser reads each file in one pass and
   borrows the text instead of copying it.
2. Converters map sheets, symbols, boards, footprints, project settings and
   library tables to the data model: exact nanometre coordinates, layers by
   name, KiCad's symbol transforms, bottom-side footprints stored as seen from
   the top, and stable ids (KiCad's UUIDs or ids derived from the content).
3. Everything that cannot be mapped exactly is collected in an import report
   with severities and stable codes.
4. The engine creates a new project from the result (`import.kicad`), adds
   libraries (`import.library`) or compares and applies netlists
   (`import.netlist`) as undoable transactions.

## Base library

The [base library](../guide/library) is recorded in
[ADR-0005: Base library and deduplication](https://github.com/Ujap1994/ElectronicWerk/blob/main/docs/adr/0005-base-library.md):

1. `ew-library` reads sources (KiCad folders and tessellated meshes) into
   source-independent entries with fingerprints, groups exact duplicates,
   near duplicates and aliases, and writes a SQLite index with an FTS5
   full-text index.
2. The engine's `library.*` methods search the index, build preview scenes
   with `ew-scene`, record review decisions and copy parts into a project.
   Builds run in a background thread and report progress as events.
3. STEP models are tessellated to GLB with OpenCASCADE (WebAssembly): in
   bulk by `apps/desktop/scripts/tessellate.ts` for library releases, and on
   demand in a worker thread of the engine host. The renderer shows them with
   three.js.
4. Library releases are built by the **Library** workflow and downloaded by
   the main process (see [Building the base library](./library)).

## Schematic editor

The [schematic editor](../guide/schematic-editor) and the
[electrical rules check](../guide/erc) are recorded in
[ADR-0006: Schematic editor and electrical rules check](https://github.com/Ujap1994/ElectronicWerk/blob/main/docs/adr/0006-schematic-editor.md):

1. `ew-schematic` works out the nets of all sheets and placements the way
   KiCad does, runs the ERC with a configurable pin type matrix, and turns
   every editing action (placing, wiring, dragging, annotating, replacing, …)
   into a list of changes, keeping the junctions right.
2. The engine's `schematic.*` methods commit those changes as one undo step
   each, and serve the ERC, net highlighting, netlists and the bill of
   materials.
3. In the renderer, `SchematicEditor` plugs into the viewer controller as its
   interaction: it snaps to the grid, draws previews as SVG over the canvas,
   moves selected objects with a shader offset until the edit is committed,
   and calls the engine.
