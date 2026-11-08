# ADR-0002: Data model, file format and transactions

- **Status:** accepted
- **Date:** 2026-11-08
- **Milestone:** M1

## Context

Every later milestone (viewer, importers, editors, outputs, 3D) builds on the
data model and the project files. The roadmap asks for open, git-friendly
files with a schema version (principle 5), never losing work (principle 2),
undo for everything (principle 3) and one engine API for every client
(principle 4). Large designs (L: 5,000 parts, 200,000 track segments) must
open in under 3 s and save in under 1 s.

## Decision

### Crates

| Crate       | Responsibility                                                                                       |
| ----------- | ---------------------------------------------------------------------------------------------------- |
| `ew-model`  | Pure data types: project, documents, objects, units, layers. No I/O.                                 |
| `ew-format` | Files: deterministic JSON layout, schema versions and migrations, atomic saves, validation, archives |
| `ew-synth`  | Synthetic reference designs for tests and benchmarks                                                 |
| `ew-core`   | Sessions, transactions, undo/redo, journal, change events, RPC                                       |

### Model

- Lengths are integer nanometres (`i64`), angles integer millidegrees,
  serialized as degrees. +x right, **+y up** (as Gerber and pick-and-place
  files), counter-clockwise rotation.
- Every editable object has a UUID. Objects of one kind live in a
  `Collection<T>` (a `BTreeMap<Id, T>`), serialized as an array sorted by id.
- A project is a manifest plus documents of five kinds: schematic, board,
  library, rules and output job. The manifest lists the documents, so later
  releases can add several boards, variants or harness documents without a
  format break.
- Schematic symbols and board footprints are linked by a `component` id. The
  units of a multi-unit part share it. Sheets used several times keep their
  designators per instance path (`instances`).
- Boards store their own copy of every footprint's geometry (as seen from the
  top; the side mirrors it), so a library change never alters a board
  silently.

### File format

- One JSON file per document, plus the `.ewproj` manifest. Every file starts
  with `format` and `schemaVersion`.
- A custom layout makes the output deterministic and diff-friendly: one
  object per line inside arrays, containers up to 100 characters on one line,
  everything else expanded. Fields follow the schema order; defaults are left
  out.
- Files are read with a fast path for the current version. Older versions run
  through a registered chain of JSON migrations; originals are backed up to
  `.ewerk/backup/schema-v<N>/` on the next save. Newer versions are refused.
- Saves stage every file as a synced temporary file first and only then
  rename them into place (encoded and written in parallel). Only changed
  documents are written.

### Transactions, undo and recovery

- Every edit is a list of `Change`s, each holding the state before and after
  (object insert/update/delete, document properties, document add/remove,
  project properties). A transaction applies all changes or none.
- Undo applies a transaction backwards, redo forwards. The history is
  unlimited in steps and limited by memory (256 MB by default; the oldest
  steps are dropped first). "Dirty" means the history state differs from the
  saved state, so undoing back to the saved state clears it.
- Every commit, undo and redo is appended to `.ewerk/journal.jsonl` (flushed
  to the OS immediately, synced to disk at least once a second). Saving
  deletes the journal. Opening a project with a journal offers recovery;
  replaying the entries reproduces the project **and** its history exactly.
  Editing and saving are blocked until the user decides, so the journal is
  never overwritten by accident.
- After every call the engine emits change sets (with the changed objects)
  and project events. The engine host pushes them to every window; views will
  use them to update incrementally (2D canvas in M2, 3D in M9).

## Consequences

- Integer coordinates and ids make importers (M3) and exporters (M8)
  deterministic and testable with golden files.
- The JSON files are larger than a binary format (the L board is about
  55 MB), but load in ≈ 0.14 s and save in ≈ 0.31 s, well within budget, and
  diff cleanly in git.
- Every future format change needs a migration step and a fixture of the old
  version.
- One open project per engine keeps the API simple. Several projects or
  windows can be added later with a project handle in the API.

## Verification

- Property tests: random edit/undo/redo sequences restore the original
  project when undone, and journal replay reproduces state and history.
- A kill test terminates a real `ewerk rpc` process with SIGKILL during
  editing and recovers every change.
- Round-trip tests: save → load → save produces identical bytes for random
  synthetic designs.
- The L budgets run in CI on every push.
