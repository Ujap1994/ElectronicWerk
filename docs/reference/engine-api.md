# Engine API

Every operation of the ElectronicWerk engine is a **method** with a name and
JSON parameters. The desktop app, the [`ewerk` command-line tool](./cli) and
the automated tests all use this one API. A feature that exists in the engine
is therefore automatically scriptable.

The engine keeps **one open project** between calls. In the desktop app the
engine lives as long as the window; on the command line use
[`ewerk rpc`](./cli#ewerk-rpc) for a session.

## Request and reply

A request is a JSON object:

```json
{ "method": "project.setProperties", "params": { "revision": "B" } }
```

- `method` (string, required): the method name.
- `params` (any JSON value, optional): the parameters; `null` or omitted when
  a method takes none. Unknown fields are rejected.

The reply is either a result or an error:

```json
{ "result": { "dirty": true, "canUndo": true, "undoLabel": "set-project-properties" } }
```

```json
{ "error": { "code": "noProject", "message": "no project is open" } }
```

## Error codes

| Code              | Meaning                                                                  |
| ----------------- | ------------------------------------------------------------------------ |
| `invalidRequest`  | The request is not valid JSON or does not match the format above         |
| `methodNotFound`  | There is no method with this name                                        |
| `invalidParams`   | The parameters do not match what the method expects                      |
| `noProject`       | The method needs an open project                                         |
| `projectOpen`     | A project is already open; close it first                                |
| `unsavedChanges`  | `project.close` without `discard` while there are unsaved changes        |
| `recoveryPending` | Unsaved work from a crashed session must be recovered or discarded first |
| `conflict`        | The edit does not fit the current state (for example a duplicate id)     |
| `notFound`        | A file, document or object does not exist                                |
| `alreadyExists`   | The target folder exists and is not empty                                |
| `ioError`         | A file could not be read or written                                      |
| `formatError`     | A file is not a valid ElectronicWerk file                                |
| `newerFormat`     | A file was written by a newer ElectronicWerk                             |
| `internal`        | An unexpected internal error                                             |

The desktop app adds `hostError` (the engine process reported an exception),
`disconnected` (the engine restarted while a request was pending) and
`timeout`.

## Events

After every call, the engine reports what happened. The desktop app pushes
these events to every window; `ewerk rpc --events` prints them.

```json
{ "type": "project", "action": "opened", "path": "/home/lab/LED Blinker/LED Blinker.ewproj" }
```

`action` is `opened`, `saved`, `closed`, `recovered` or `recoveryDiscarded`.

```json
{
  "type": "changed",
  "changeSet": {
    "seq": 7,
    "cause": "commit",
    "label": "rename-document",
    "changes": [{ "doc": "…", "op": "props" }],
    "truncated": false,
    "history": { "dirty": true, "canUndo": true, "canRedo": false, "undoLabel": "rename-document", … }
  }
}
```

- `cause`: `commit`, `undo`, `redo` or `recover`.
- `changes`: one entry per changed object: `op` (`insert`, `update`,
  `delete`, `props`, `addDocument`, `removeDocument`, `project`), the document
  `doc`, the object `id` and `type`, and the new `object` for inserts and
  updates. Views use this to update without reloading.
- `truncated`: `true` if the change set was too large to list (more than
  1,000 changes); views reload instead.

## Projects

### `project.templates`

Lists the templates: `id`, default `board` settings (`width`, `height`,
`layers`, `thickness` in nanometres, or `null`), `fixedSize`, `example`.

### `project.create`

| Parameter  | Type   | Meaning                                                                                                                                                                                      |
| ---------- | ------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `path`     | string | Project folder to create (must not exist or be empty)                                                                                                                                        |
| `name`     | string | Project name                                                                                                                                                                                 |
| `template` | string | Template id (default `empty`)                                                                                                                                                                |
| `board`    | object | `width`, `height`, `layers`, `thickness` (default: template values)                                                                                                                          |
| `units`    | string | `mm`, `mil` or `inch`                                                                                                                                                                        |
| `names`    | object | Optional names for the created documents: `rootSchematic`, `library`, `rules`, `outputJob`, and `outputs` by kind (`{"bom": "Stückliste"}`). The desktop app passes them in the UI language. |

Creates the project on disk, opens it and returns the
[project snapshot](#project-snapshot).

### `project.open`

`{"path": "…"}` (folder or `.ewproj` file). Opens the project and returns the
snapshot. If unsaved work from a crashed session exists, `recovery` describes
it; call `project.recover` or `project.discardRecovery` before editing.

### `project.probe`

`{"path": "…"}`. A quick check without opening:
`{"exists", "valid", "name", "path", "manifestPath", "hasRecovery"}`.

### `project.info`

Returns the snapshot of the open project.

### `project.setProperties`

Changes project properties with a JSON merge patch: `name`, `revision`,
`description`, `variables` (`null` removes a variable), `rootSchematic`,
`settings` (`units`, `schematicGrid`, `boardGrid`). One undo step. Returns the
history state.

```json
{ "method": "project.setProperties", "params": { "variables": { "AUTHOR": "Lab", "OLD": null } } }
```

### `project.save`

`{"all": false}`. Writes changed files (every file with `all`), deletes files
of removed documents and resets the autosave journal. Returns `written`,
`deleted` (project-relative paths) and `history`.

### `project.saveAs` / `project.saveCopy`

`{"path": "…"}` (a new or empty folder outside the project). `saveAs` writes
the project there and continues working in the new folder (returns the
snapshot); `saveCopy` only writes a copy (returns its `manifestPath`). Other
files of the project folder are copied too.

### `project.archive`

`{"path": "….zip"}`. Writes a ZIP archive of the current state. Returns
`path`, `files`, `bytes`.

### `project.close`

`{"discard": false}`. Closes the project. With unsaved changes this fails with
`unsavedChanges` unless `discard` is `true`.

### `project.recover` / `project.discardRecovery`

Restores or discards the unsaved work of a crashed session (see
[Undo, autosave and recovery](../guide/undo-and-recovery)). Both return the
snapshot. `project.recover` fails with `conflict` (and changes nothing) if the
journal no longer fits the saved files.

### `project.files`

Lists the files in the project folder: `path`, `size`, `category`
(`manifest`, `document`, `generated`, `other`).

### `project.validate`

`{"path": "…"}` checks a project folder without opening it; without `path` the
open project is checked. Returns `issues` (`severity`, `code`, `message`,
`path`, `object`, `params`), `errors`, `warnings`, `documents`, `objects`.
`message` is English; `params` holds the values in it (for example
`{"name": "REVISION"}`), so clients can show a translated message for the
`code`.

### `project.resolveText`

`{"text": "${PROJECT_NAME} rev ${REVISION}", "doc": "…"}` → `{"text": "LED Blinker rev B"}`.
`doc` provides `SHEET_NAME` and `FILENAME`.

### Project snapshot

| Field                                                | Content                                                                                                                   |
| ---------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------- |
| `id`, `name`, `revision`, `description`, `variables` | Project properties                                                                                                        |
| `path`, `manifestPath`                               | Project folder and project file (absolute)                                                                                |
| `rootSchematic`, `settings`                          | Top sheet and editor settings                                                                                             |
| `documents`                                          | Per document: `id`, `kind`, `name`, `path`, `dirty`, `stats` (counts, page or board size, stack-up summary, child sheets) |
| `history`                                            | `dirty`, `canUndo`, `canRedo`, `undoLabel`, `redoLabel`, `undoCount`, `redoCount`                                         |
| `needsUpgrade`                                       | Files were read in an older format and are upgraded on save                                                               |
| `recovery`                                           | `null`, or `commits`, `entries`, `modified`, `matchesFiles` of unsaved work                                               |
| `journalError`                                       | `null`, or why the autosave journal could not be written                                                                  |

## Documents

### `document.add`

`{"kind": "schematic", "name": "Power Supply", "parent": "…"}`. Adds an empty
document. With `parent` (a schematic), a sheet symbol for the new sheet is
placed on the parent. Returns `id` and `path`. One undo step.

### `document.rename`

`{"doc": "…", "name": "…"}`. One undo step.

### `document.remove`

`{"doc": "…"}`. Removes the document and the sheet symbols that place it; the
file is deleted on the next save. One undo step.

### `document.properties` / `document.setProperties`

`document.properties` returns the editable properties of a document (`name`,
`page` and `titleBlock` of a sheet, `stackup` and `origin` of a board, the
whole content of rules and output jobs) plus `docKind` and `path`.
`document.setProperties` changes them with a JSON merge patch:

```json
{
  "method": "document.setProperties",
  "params": { "doc": "…", "props": { "page": { "format": "a3" } } }
}
```

### `document.get`

`{"doc": "…"}`. Returns the complete document, as in its file, plus `path`.

## Editing

### `edit.apply`

Applies a list of changes as **one transaction**: either all succeed or
nothing changes.

```json
{
  "method": "edit.apply",
  "params": {
    "label": "add-test-points",
    "changes": [
      { "op": "insert", "doc": "…", "object": { "type": "junction", "at": [1000000, 2000000] } },
      { "op": "patch", "doc": "…", "id": "…", "fields": { "end": [5000000, 0] } },
      { "op": "delete", "doc": "…", "id": "…" }
    ]
  }
}
```

| `op`             | Fields                                | Effect                                                            |
| ---------------- | ------------------------------------- | ----------------------------------------------------------------- |
| `insert`         | `doc`, `object`                       | Adds an object (an `id` is assigned if missing)                   |
| `update`         | `doc`, `object`                       | Replaces an object with the same `id` and `type`                  |
| `patch`          | `doc`, `id`, `fields`                 | Changes some fields (JSON merge patch; `id` and `type` are fixed) |
| `delete`         | `doc`, `id`                           | Removes an object                                                 |
| `setProps`       | `doc`, `props`                        | Changes document properties (merge patch)                         |
| `setProject`     | `props`                               | Changes project properties (merge patch)                          |
| `addDocument`    | `kind`, `name`, optional `path`, `id` | Adds an empty document                                            |
| `removeDocument` | `doc`                                 | Removes a document                                                |

Object types are `symbol`, `wire`, `bus`, `busEntry`, `junction`, `label`,
`powerPort`, `noConnect`, `sheet`, `text`, `shape` (schematics); `net`,
`footprint`, `track`, `via`, `zone`, `boardText`, `boardShape` (boards);
`symbolDef`, `footprintDef` (libraries). Their fields are described in the
[file format](./file-format). The result contains `seq`, the ids of
`inserted` objects and `history`.

### `edit.undo` / `edit.redo`

Undo or redo one transaction. Result: `applied` (`false` if there was nothing
to undo/redo), `label` and `history`.

### `edit.history`

Returns `state` (the history state), `undo` and `redo` (labels, next step
first) and `bytes` (memory used by the history).

Labels of built-in commands: `add-document`, `add-sheet`, `remove-document`,
`rename-document`, `set-document-properties`, `set-project-properties`,
`apply` (default of `edit.apply`), `recover`.

## View

The methods behind the [2D view](../guide/viewer). They only read the
project.

### `view.scene`

| Parameter  | Type   | Meaning                                                                                                                    |
| ---------- | ------ | -------------------------------------------------------------------------------------------------------------------------- |
| `doc`      | UUID   | A schematic sheet or board                                                                                                 |
| `encoding` | string | `base64` (default): `data` is the buffer in base64; `none`: `data` is `null` (metadata only)                               |
| `labels`   | object | Optional title block labels in the UI language: `file`, `size`, `date`, `revision` (default `File`, `Size`, `Date`, `Rev`) |

Returns everything the view draws for the document. The metadata is JSON;
the primitives are one little-endian binary buffer (`data`). The desktop app
receives the buffer as raw bytes instead of base64.

```console
$ ewerk rpc
{"id":1,"method":"project.open","params":{"path":"LED Blinker"}}
{"id":2,"method":"view.scene","params":{"doc":"0a64e4a4-…","encoding":"none"}}
{"id":2,"result":{"kind":"pcb","name":"LED Blinker","origin":[40000000,25000000],"stats":{"objects":412,"segments":731,…},…}}
```

| Field         | Content                                                                                                                  |
| ------------- | ------------------------------------------------------------------------------------------------------------------------ |
| `doc`, `kind` | Document id; `pcb` or `schematic`                                                                                        |
| `origin`      | Every coordinate in the buffer is relative to this point (nm), so f32 keeps full precision near the design               |
| `bounds`      | Bounding box of everything drawn `[minX, minY, maxX, maxY]` (absolute nm)                                                |
| `home`        | The area to fit on open: the board outline or the page                                                                   |
| `page`        | Page size of a sheet `[width, height]`                                                                                   |
| `grid`        | Grid spacing from the project settings                                                                                   |
| `chunk`       | Instances per culling chunk (1,024)                                                                                      |
| `font`        | Text metrics relative to the text size: `advance` per character, `linePitch`                                             |
| `layers`      | One entry per layer, in panel order (see below)                                                                          |
| `objectTypes` | Names of the object types (`track`, `via`, `footprint`, `pad`, `symbol`, `pin`, …), indexed by type code                 |
| `objects`     | Byte offsets of the object table: `types` (u8), `parents`, `children`, `nets` (u32), `boxes` (4 × f32), `ids` (16 bytes) |
| `nets`        | Net names, indexed by net number                                                                                         |
| `stats`       | Counts of objects and primitives, and the buffer size `bytes`                                                            |
| `data`        | The buffer (base64) or `null`                                                                                            |

A layer has a `key` (`cu.top`, `silk.bottom`, `holes`, `sch.wires`, …), a
`group`, a `side`, an optional stack-up `name`, its draw order `z` in the top
view and whether it is `visible` by default. Its primitives are sections of
the buffer with `offset`, `count` and the byte offset of their `chunks`
(bounding boxes of four f32 per 1,024 instances):

| Section    | Layout per instance (f32 unless noted)                                          |
| ---------- | ------------------------------------------------------------------------------- |
| `segments` | start x, y, end x, y, half width, object (u32)                                  |
| `arcs`     | centre x, y, radius, half width, start angle, sweep (radians), object           |
| `circles`  | centre x, y, outer radius, inner radius (0 for a disc), object                  |
| `rects`    | centre x, y, half width, half height, rotation (radians), corner radius, object |
| `mesh`     | vertices: x, y, object (u32), flags (u32; 1 = hatched); u32 triangle indices    |

`texts` lists the layer's texts as `[text, x, y, size, rotation°, hAlign,
vAlign, flags, object]`, with alignment −1/0/1 (left/centre/right,
bottom/centre/top) and flags 1 (mirrored), 2 (bold), 4 (italic). Texts use a
monospace font: each character advances `font.advance × size`, where `size` is
the cap height.

The object table lists every selectable object, including pads, pins, sheet
pins and symbol fields as children of their footprint, symbol or sheet.
Children carry the id of their parent and their index in `children`; the
value `0xFFFFFFFF` means "none" in the u32 tables. Primitives without an
object (the page and the title block) also use `0xFFFFFFFF`.

### `view.object`

`{"doc": "…", "id": "…", "child": {"kind": "pad", "index": 0}}` (`child` only
for pads, footprint texts, pins, fields and sheet pins). Returns the
properties shown in the inspector:

```json
{
  "type": "pad",
  "id": "4f0c3b2e-…",
  "title": "U8-1",
  "rows": [
    { "key": "number", "value": "1", "kind": "text" },
    { "key": "net", "value": "N00045", "kind": "net" },
    { "key": "position", "value": [15238000, 37700000], "kind": "point" },
    { "key": "size", "value": [1550000, 600000], "kind": "size" },
    { "key": "layers", "value": ["cu.top", "mask.top", "paste.top"], "kind": "layers" }
  ]
}
```

`kind` tells clients how to show the value: `text`, `length` (nm), `point`,
`size`, `angle` (degrees), `bool`, `layer`, `layers`, `net`, `enum` (a
camelCase name such as `throughHole`) or `count`. Positions of pads and pins
are absolute. An unknown object fails with `notFound`.

## Import

These methods read KiCad files and netlists; they never change them. See
[Importing KiCad projects](../guide/import-kicad) and
[Netlists](../guide/netlists).

### `import.kicadProbe`

`{"source": "…"}`. Checks a KiCad project without importing it: the
`.kicad_pro` file, its schematic or board, or a folder with exactly one KiCad
project. Returns `{"name", "folder", "project", "schematic", "board"}` (the
last three tell which files exist). Fails with `invalidParams` if `source` is
not a KiCad project.

### `import.kicad`

| Parameter | Type   | Meaning                                                           |
| --------- | ------ | ----------------------------------------------------------------- |
| `source`  | string | The KiCad project (as for `import.kicadProbe`)                    |
| `path`    | string | Project folder to create (must not exist or be empty)             |
| `name`    | string | Project name (default: the name of the KiCad project)             |
| `names`   | object | Optional names for the created documents, as for `project.create` |

Imports the KiCad project into a new project, opens it and returns
`{"project", "report", "reportFile"}`: the [project snapshot](#project-snapshot),
the import report and the path of the Markdown report inside the project
(`kicad-import-report.md`). No project may be open.

The report:

```json
{
  "source": "/home/lab/blinky/blinky.kicad_pro",
  "versions": { "board": "20260206", "schematic": "20260306" },
  "counts": { "footprints": 8, "nets": 7, "sheets": 2, "tracks": 12 },
  "findings": [
    {
      "severity": "warning",
      "code": "custom-rules",
      "message": "1 custom design rules are not imported; …",
      "file": "blinky.kicad_dru",
      "count": 1
    }
  ]
}
```

`severity` is `error`, `warning` or `info`, most serious first. The codes are
listed in [Report codes](../guide/import-kicad#report-codes).

### `import.library`

| Parameter  | Type   | Meaning                                                                  |
| ---------- | ------ | ------------------------------------------------------------------------ |
| `source`   | string | `.kicad_sym`, `.kicad_symdir`, `.pretty` or `.kicad_mod`                 |
| `doc`      | id     | Target library document (default: the first library of the project)      |
| `nickname` | string | Prefix of the names (default: the library's file name without extension) |

Adds the symbols and footprints that the library does not have yet, as one
undoable step (`Import library`). Returns `{"symbols", "footprints",
"report", "history"}`: the numbers added, a report (with `already-in-library`
for skipped names) and the history state.

### `import.netlist`

| Parameter | Type    | Meaning                                                        |
| --------- | ------- | -------------------------------------------------------------- |
| `source`  | string  | KiCad `.net`, KiCad XML or CSV netlist                         |
| `doc`     | id      | The board (default: the first board of the project)            |
| `apply`   | boolean | Apply the netlist to the board (default `false`: only compare) |

Returns:

| Field               | Meaning                                                                                                             |
| ------------------- | ------------------------------------------------------------------------------------------------------------------- |
| `format`            | `kicad`, `xml` or `csv`                                                                                             |
| `nets`              | Nets in the netlist                                                                                                 |
| `components`        | Components in the netlist                                                                                           |
| `comparison`        | `nets` and `nodes` (connected nets and their pins), `matchingNets`, `sameNames`, `missingComponents`, `differences` |
| `changedPads`       | Pads that applying the netlist changes                                                                              |
| `newNets`           | Nets that applying the netlist creates                                                                              |
| `missingFootprints` | Components of the netlist without a footprint on the board                                                          |
| `extraFootprints`   | Footprints on the board that the netlist does not mention                                                           |
| `applied`           | `true` if the netlist was applied (one undoable step, `Import netlist`)                                             |

Each entry of `differences` is `{"reference", "pin", "expected", "actual",
"unconnected"}`: the net of the pin in the netlist and on the board (`null`
if none), and whether the netlist leaves the pin unconnected while the board
connects it.

## Library

The [base library](../guide/library) lives in its own folder: `EW_LIBRARY_DIR`
if set (the desktop app sets it to `<userData>/library`), otherwise
`<config>/ElectronicWerk/library`. Methods other than `library.status`,
`library.open`, `library.detect` and `library.build` fail with `notFound`
until a library is installed.

### `library.status`

Returns `{"dir", "installed", "building", "status"}`. `building` is
`{"stage", "done", "total"}` while a build runs (stages `symbols`,
`footprints`, `models`, `index`), else `null`. `status` describes the index:
`path`, `format`, `sources` (`name`, `version`, `license`, `url`), `counts`
and `visible` entries per kind (`symbol`, `footprint`, `model`), `groups` per
class (`exact`, `near`, `alias`), `pending` near groups, `meshes`, `bytes`
and the `release` the index was downloaded from.

### `library.open`

`{"dir": "…"}`. Uses another library folder (closes the current index).

### `library.detect`

Lists KiCad installations of this computer with libraries: `[{"path",
"symbols", "footprints", "models"}]`.

### `library.build`

| Parameter | Type    | Meaning                                                                                                                                                                                                            |
| --------- | ------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `sources` | array   | Sources, highest priority first: `{"kicad"}` (an installation or a folder with the library repositories) and/or `{"symbols", "footprints", "models", "meshes"}`, plus optional `name`, `version`, `license`, `url` |
| `license` | string  | License text, written as `LICENSE.md`                                                                                                                                                                              |
| `wait`    | boolean | Build within the request (scripts). Default: build in the background                                                                                                                                               |

A background build returns `{"started": true}` at once and reports through
events (see below); searches keep using the old index until the new one is
ready. With `wait`, the result is `{"status", "problems"}`. Review decisions
are kept.

### `library.search`

| Parameter         | Type     | Meaning                                         |
| ----------------- | -------- | ----------------------------------------------- |
| `text`            | string   | Search text (empty: browse by library and name) |
| `kind`            | string   | `symbol`, `footprint` or `model`                |
| `libraries`       | string[] | Only these libraries                            |
| `mounting`        | string   | Footprints: `smd`, `tht` or `other`             |
| `withModel`       | boolean  | Footprints with a 3D model only                 |
| `duplicates`      | boolean  | Entries in a duplicate group only               |
| `includeMerged`   | boolean  | Also entries merged into another one            |
| `offset`, `limit` | number   | Paging (`limit` at most 500)                    |

Returns `{"total", "items", "elapsedMs"}`. Each item: `id`, `kind`,
`library`, `name`, `description`, `value`, `package`, `pins`, `mounting`,
`mergedInto` (the id of the entry it is merged into, or `null`) and `groups`
(the number of duplicate groups it belongs to).

### `library.facets`

`{"kind": "footprint"}`. The libraries of a kind with their number of
visible entries: `[{"library", "count"}]`.

### `library.get`

`{"id": 42}`. Everything about an entry: the search fields plus `keywords`,
`mpn`, `manufacturer`, `datasheet`, `params`, `units`, `size` (mm), `model`
(the model key of a footprint, `Library/Name`), `modelId`, `file`,
`sourceFile` (the STEP file on this computer, if known), `mesh` (the GLB
content hash of a model), `aliases` (merged entries), `duplicateGroups`,
`source` and `definition` (the full symbol or footprint in the
[file format](./file-format)).

### `library.preview`

`{"id": 42, "unit": 1}`. The 2D scene of a symbol (one unit) or footprint, in
the same form as `view.scene` (the desktop app receives the
buffer separately). 3D models have no 2D preview (`invalidParams`).

### `library.groups`

| Parameter         | Type   | Meaning                                 |
| ----------------- | ------ | --------------------------------------- |
| `kind`            | string | Only this kind                          |
| `class`           | string | `exact`, `near` or `alias`              |
| `decision`        | string | `merge`, `separate` or `pending`        |
| `offset`, `limit` | number | Paging (default limit 100, at most 500) |

Returns `{"total", "groups"}`; each group has `key`, `kind`, `class`,
`reason`, `decision`, `canonical` (the id of the shown entry) and `members`
(`id`, `name`), best first.

### `library.decide`

`{"key": "…", "decision": "merge", "canonical": 42}`. Records a review
decision (also in `decisions.json`) and returns the group. `canonical` sets
the shown entry. Emits a `changed` event.

### `library.report`

`{"format": "markdown"}` or JSON (default). The deduplication report as a
string.

### `library.install`

`{"file": "…", "release": "library-…", "remove": true}`. Installs a pack (see
[Building the base library](../development/library#packs)). `release` is
stored with an index pack; `remove` deletes the pack afterwards. Returns
`{"installed": {"index", "meshes"}, "status"}` and emits `changed`.

### `library.mesh`

`{"hash": "…"}`. Where the GLB file of a mesh hash is cached:
`{"path", "exists"}`.

### `library.add`

`{"ids": [42, 43], "doc": "…", "footprints": true}`. Copies symbols and
footprints into a library of the open project (default: the first one), as
one undoable step (`add-from-library`). With `footprints` (default), the
default footprints of the symbols are copied too, if the base library has
them. Returns `{"symbols", "footprints", "skipped", "history"}`.

### Library events

| Event                                                                     | When                                               |
| ------------------------------------------------------------------------- | -------------------------------------------------- |
| `{"type": "library", "action": "progress", "stage", "done", "total"}`     | During a background build (at most every 200 ms)   |
| `{"type": "library", "action": "built", "seconds", "status", "problems"}` | A background build finished; the new index is open |
| `{"type": "library", "action": "failed", "message"}`                      | A background build failed; the old index stays     |
| `{"type": "library", "action": "changed"}`                                | A decision or an installation changed the index    |

### Meshes in the desktop app

The desktop app's engine host answers two more methods itself (they are not
part of the Rust engine):

- `mesh.load` `{"hash"}` returns the cached GLB file of a mesh hash as
  `{"data"}` (bytes), or `notFound`.
- `mesh.tessellate` `{"file"}` converts an absolute STEP path to GLB in a
  worker thread (OpenCASCADE) and caches it by path, size and time; returns
  `{"data", "cached"}`.

## Engine

### `engine.info`

| Field     | Type    | Description                                                             |
| --------- | ------- | ----------------------------------------------------------------------- |
| `name`    | string  | Always `ElectronicWerk Engine`                                          |
| `version` | string  | Engine version, for example `0.2.0`                                     |
| `os`      | string  | Operating system the engine was built for (`windows`, `macos`, `linux`) |
| `arch`    | string  | Processor architecture (`x86_64`, `aarch64`)                            |
| `threads` | number  | Number of worker threads the engine may use                             |
| `debug`   | boolean | `true` for development builds                                           |

### `engine.methods`

Returns the names of all methods, sorted alphabetically.

### `engine.ping`

Returns `{"pong": true, "echo": <params>}`.

## Versioning

Method names and result fields are stable within a major version. New methods
and new optional fields can appear in any release; they are listed in the
[changelog](https://github.com/Ujap1994/ElectronicWerk/blob/main/CHANGELOG.md).
