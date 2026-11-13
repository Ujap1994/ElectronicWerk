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
placed on the parent: at `at` (its top-left corner) with `size`, or in the
next free slot of a 6-column grid if they are missing. Returns `id` and
`path`. One undo step.

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

### `document.object`

`{"doc": "…", "id": "…"}`. One object of a document, with its `type`, in the
same form as `edit.apply` takes it. Unknown objects fail with `notFound`.

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

## Schematic

The schematic editor's methods. Edits work on one sheet (`doc`) and commit
one undo step with their own history label (in brackets); they return
`{"seq", "select", "changes", "history"}`: the sequence number of the change
set (`null` if nothing changed, in which case no undo step is made), the ids
to select afterwards, the number of changed objects and the history state.
Coordinates are nanometres (y up); the grid starts at the top-left corner of
the page.

Every edit keeps the junctions right: a junction is added where three or more
connections meet (a wire passing through counts twice, at least one of them
ends there) and removed where it is no longer needed. Crossing wires never
get one.

### `schematic.erc`

The electrical rules check of the whole project, with the settings of the
rules document. Returns `{"issues", "errors", "warnings", "nets", "pins"}`;
each issue has `code`, `severity` (`error` or `warning`), `message`
(English), `args` (values for a translated message: `pin`, `net`, `name`,
`kind`, …), `sheet` (document id), `path` (`/`, `/Power/`), `at` and `items`
(the objects concerned). Errors come first, then by sheet and position. The
codes are listed in [Electrical rules check](../guide/erc#what-is-checked).

### `schematic.ercSettings` / `schematic.setErcSettings`

`schematic.ercSettings` returns `{"checks": [{"code", "default", "value"}],
"kinds", "defaultMatrix", "matrix", "rules"}`: the severity of every check,
the pin types in matrix order and the 12 × 12 pin type matrix (`error`,
`warning` or `ignore` per cell), with the defaults, and the rules document
that stores them.

`schematic.setErcSettings` takes `{"severities": {"code": "warning"},
"matrix": [[…]]}` and stores only what differs from the defaults
(`erc-settings`).

### `schematic.net`

`{"doc", "id"?, "pin"?: {"symbol", "index"}, "at"?}`. The net of a wire,
label or other object, of a pin (its index in the unit's pin list) or of a
point, and what belongs to it on each sheet: `{"net": "/DIS", "sheets":
{"<doc>": {"items": [ids], "pins": ["<symbol id>/pin/<n>", "<sheet
id>/sheetPin/<n>"]}}, "places": […]}`. `net` is `null` if nothing is there.

`places` lists where the net shows up, for the
[net navigator](../guide/schematic-editor#following-a-net-across-sheets):
one entry per label, power port, power symbol, sheet pin and component pin,
sorted by sheet instance (top sheet first), then names before pins. Each has
`doc`, `sheet` (the document name), `path` (`/`, `/Power/`), `kind`
(`globalLabel`, `hierarchicalLabel`, `label`, `powerPort`, `powerSymbol`,
`sheetPin` or `pin`), `id` (the object; the placed symbol for pins), `at` and
`text` (`SDA`, `Power: VIN`, `U1.5`); pins also have `designator`, `pin` and
`name`. Wires and junctions are left out.

### `schematic.symbols` / `schematic.symbol`

`schematic.symbols` lists the symbols of the project libraries: `name`,
`description`, `keywords`, `prefix`, `value`, `footprint`, `power`, `units`,
`pins` and `library`, sorted by name. `schematic.symbol` (`{"name"}`) returns
one symbol definition.

`schematic.components` lists the [components](../guide/components) of the
project libraries: `name`, `description`, `keywords`, `symbol`, `footprint`,
`value`, `manufacturer`, `mpn` and `library`.

### `schematic.place`

`{"doc", "symbol", "at", "rotation"?, "mirror"?, "unit"?, "annotate"?}`.
Places a symbol (`place-symbol`). With `annotate`, it gets the next free
designator of its prefix across the project; otherwise `R?`.

With `"component": "<name>"` instead of `symbol`, the component's symbol is
placed with the component's value, footprint and fields, plus
`Manufacturer`, `MPN`, `Datasheet` and `Supplier <n>` / `Supplier <n> PN`
per supplier.

### `schematic.wire`

`{"doc", "points": [[x, y], …], "bus"?}`. A wire (or bus) through the points
(`draw-wire`). Repeated points are dropped and straight runs merged; inner
corners count as connection points.

### `schematic.add`

`{"doc", "item", "at", "rotation"?}` with `item` one of `{"kind": "label",
"label": "local" | "global" | "hierarchical", "text"}`, `{"kind":
"powerPort", "net", "style"?}`, `{"kind": "noConnect"}`, `{"kind":
"junction"}` or `{"kind": "text", "text"}` (`add-label`, `add-power-port`,
`add-no-connect`, `add-junction`, `add-text`). A junction placed this way is
kept even where it is not needed.

### `schematic.move`

`{"doc", "ids", "delta", "drag"?}` (`move`, `drag`). With `drag`, wires that
end on a connection point of a moved object follow: horizontal and vertical
wires stay straight and get a corner wire, others stretch; junctions, labels,
power ports and flags on those points move along. Wires that shrink to
nothing are removed.

### `schematic.dragInfo`

`{"doc", "ids"}`. What a drag of these objects drags along, for previews:
`{"wires": [{"id", "start", "end", "startMoves", "endMoves"}], "followers":
[ids]}`.

### `schematic.orient`

`{"doc", "ids", "orientation", "center"?}` with `orientation` `left`
(counterclockwise), `right`, `horizontal` (mirror left to right) or
`vertical` (top to bottom) (`rotate`, `mirror`). One object turns about its
anchor, several about the centre of their anchors on the grid.

### `schematic.delete`

`{"doc", "ids"}` (`delete`).

### `schematic.copy` / `schematic.paste`

`schematic.copy` (`{"doc", "ids"}`) returns `{"objects"}`, the objects as
stored. `schematic.paste` (`{"doc", "objects", "offset", "annotate"?}`,
`paste`) inserts them on any sheet with new ids, moved by `offset`; symbols
keep their designator if it is free and are numbered again otherwise
(`annotate`, default `true`) or left unannotated. Units of one copied part
stay one part.

### `schematic.annotate`

`{"sheet"?, "reset"?, "order"?, "bySheet"?}` (`annotate`). Numbers the
symbols whose designator ends in `?` (with `reset`: all of them) on all
sheets or only on `sheet`, sheet by sheet in hierarchy order and by position
(`order`: `rows` or `columns`), from 1 or (with `bySheet`) from the sheet
number × 100 + 1. Units of one part share a designator; sheets used more
than once get one per placement.

### `schematic.find` / `schematic.replace`

`schematic.find` takes `{"query", "matchCase"?, "wholeWord"?, "sheet"?}` and
returns `[{"doc", "id", "field", "text", "at"}]` with `field` one of
`designator`, `value`, `footprint`, `property:<name>`, `label`, `power`,
`text` and `sheet`. `schematic.replace` takes the same plus `"with"` and,
optionally, `"only": [{"doc", "id", "field"}]` (`replace`); labels, power
ports, sheet names and designators are never emptied.

### `schematic.syncSheetPins`

`{"doc", "id", "remove"?}` (`sync-sheet-pins`). Gives the sheet symbol `id` a
pin for every hierarchical label of its sheet that has none yet (down the
left edge, growing the symbol if needed); with `remove`, also removes pins
without a label.

### `schematic.snapToGrid`

`{"doc", "ids"?}` (`snap-to-grid`). Moves wire ends, symbols, labels,
junctions, flags, power ports, texts and sheet symbols of the sheet (or only
`ids`) to the nearest grid point.

### `schematic.netlist` / `schematic.bom`

`schematic.netlist` (`{"format": "kicad" | "csv"}`) and `schematic.bom`
return `{"text"}`: a KiCad netlist (version E), a CSV netlist
(`Net,Designator,Pin`) or the bill of materials as CSV. See
[Schematic exports](../guide/schematic-exports).

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

### `view.item`

`{"doc", "kind": "symbol" | "footprint", "name", "unit"?, "alternate"?,
"encoding"?}`. The scene of the library editor: one body of a symbol (unit,
1-based, normal or alternate) or a footprint, in the same form as
`view.scene`. Every pin, pad, graphic and text is an object of its own; its
id encodes the item: the high 64 bits are the kind (`1` pin, `2` graphic,
`3` text, `4` pad), the low 64 bits the index in its list
(`00000000-0000-0004-0000-000000000003` is pad 3). Hidden pins are drawn.

## Layout editor

The methods of the [layout editor](../guide/layout-editor). The logic lives
in the crate `ew-pcb`. Edits commit one undo step with the label in
brackets and return `{"seq", "history", …}`; `seq` is `null` when nothing
changed. Coordinates are nanometres; points are `[x, y]`.

### `pcb.drc`

`{"doc", "region"?: [x0, y0, x1, y1]}`. Runs the [design rules
check](../guide/drc). With `region`, only the items in that area (grown by
the largest clearance) are checked and the result is merged with the last
report of the board; without, the whole board is checked. Returns:

```json
{
  "findings": [
    {
      "code": "clearance",
      "severity": "error",
      "message": "Clearance 0.15 mm between track (GND) and pad U1-3 (VCC) is less than 0.2 mm (net class Default)",
      "args": {
        "a": "track (GND)",
        "b": "pad U1-3 (VCC)",
        "actual": "0.15 mm",
        "required": "0.2 mm",
        "source": "net class Default"
      },
      "at": [12000000, 8000000],
      "layer": "cu.top",
      "targets": [{ "id": "…" }, { "id": "…", "pad": 2 }],
      "key": "clearance:…/…#2:cu.top",
      "excluded": false
    }
  ],
  "counts": { "errors": 1, "warnings": 0, "excluded": 0, "unrouted": 3 },
  "airwires": [
    {
      "net": "…",
      "from": [0, 0],
      "to": [5000000, 0],
      "fromItem": { "kind": "pad", "footprint": "…", "index": 0 },
      "toItem": { "kind": "track", "id": "…" }
    }
  ],
  "ruleErrors": []
}
```

`targets` are board objects (a pad: its footprint and pad index). `key`
identifies the finding for `pcb.exclude`. `airwires` is the ratsnest.
Differences to the schematic (`missing-footprint`, `extra-footprint`,
`net-mismatch`) are part of every report.

### `pcb.exclude`

`{"doc", "keys": ["…"], "excluded"?: true}` (`drc-exclusions`). Accepts
findings (or reports them again with `"excluded": false`). The keys are
stored in the board.

### `pcb.ratsnest`

`{"doc"}` → `{"airwires": [...], "unrouted": 3}`: the unrouted connections
without running the checks.

### `pcb.fillZones` / `pcb.unfillZones`

`{"doc", "zones"?: ["…"]}` (`fill-zones` / `unfill-zones`). Fills (or
empties) all zones of the board, or the given ones. Returns `{"zones",
"areas"}`: the number of changed zones and of filled areas.

### `pcb.rules` / `pcb.setRules` / `pcb.validateRule`

`pcb.rules {"doc"?}` returns the project's design rules (`rules`: the
properties of the rules document), the DRC checks with their default and
current severity (`checks`), the nets of each net class on the board `doc`
(`nets`) and syntax errors of custom rules (`errors`).

`pcb.setRules {"props": {…}}` (`design-rules`) replaces the rules; it
creates a rules document if the project has none.

`pcb.validateRule {"condition": "A.net == 'GND'"}` → `{"ok": true}` or
`{"ok": false, "message", "position"}` (character position of the error).

### `pcb.syncPreview` / `pcb.syncApply`

`pcb.syncPreview {"doc"}` compares the schematic with the board and returns
`{"changes", "problems"}`. Each change has an `index`, a `kind` (`addNet`,
`renameNet`, `removeNet`, `addFootprint`, `removeFootprint`,
`changeFootprint`, `setDesignator`, `setValue`, `link`, `setPadNet`), its
fields and an English `description`. Problems name parts that cannot be
placed (`unannotated`, `no-footprint`, `unknown-footprint`,
`duplicate-designator`, `pin-without-pad`).

`pcb.syncApply {"doc", "changes"?: [0, 2, …]}` (`update-board`) applies the
given changes (all if missing) and returns the new footprints in `select`.

### `pcb.backPreview` / `pcb.backApply`

The same for back annotation: `setDesignator`, `setValue` and
`setFootprint` changes from the board to the schematic
(`update-schematic`).

### `pcb.crossProbe`

`{"doc", "symbols"?: ["…"], "footprints"?: ["…"]}` →
`{"footprints": ["…"], "symbols": [{"doc", "id"}]}`: the footprints of
placed symbols and the placed symbols (with their sheets) of footprints.

### `pcb.edit`

`{"doc", "op", …}`. Operations (label in brackets):

| `op`         | Parameters                | What it does                                                      |
| ------------ | ------------------------- | ----------------------------------------------------------------- |
| `move`       | `ids`, `offset`           | Moves objects (`move`); locked ones stay.                         |
| `rotate`     | `ids`, `angle`, `center`? | Turns objects (`rotate`) around the centre of the selection.      |
| `flip`       | `ids`, `centerX`?         | Puts objects on the other side (`flip`).                          |
| `delete`     | `ids`                     | Deletes unlocked objects (`delete`).                              |
| `lock`       | `ids`, `locked`           | Locks or unlocks (`lock`).                                        |
| `align`      | `ids`, `mode`             | `left`, `right`, `top`, `bottom`, `centerX`, `centerY` (`align`). |
| `distribute` | `ids`, `axis`             | `x` or `y`, equal gaps (`distribute`).                            |
| `setLayer`   | `ids`, `layer`            | Tracks to a copper layer (`set-layer`).                           |
| `setWidth`   | `ids`, `width`            | Track width (`set-width`).                                        |
| `setNet`     | `ids`, `net`?             | Net of tracks, vias and zones (`set-net`).                        |
| `dragTrack`  | `id`, `offset`            | Moves a segment, its neighbours follow (`drag-track`).            |

The reply has `select` and `changes`.

### `pcb.place`

`{"doc", "footprint": "SOIC-8", "at", "rotation"?, "side"?, "designator"?,
"value"?}` (`place-footprint`). Places a footprint of the project
libraries. Without a designator it gets the next free `X<n>`.

### `pcb.routeStart` / `pcb.route` / `pcb.addTracks`

`pcb.routeStart {"doc", "at", "layer"}` → `{"at", "net", "netName", "width",
"viaDiameter", "viaDrill", "clearance", "previous"}`: where a route that
begins at `at` starts (snapped to a pad or via centre or a track end) and the
sizes from the rules. `previous` is the direction of the track whose end the
route starts at (`null` for pads and vias).

`pcb.route {"doc", "layer", "net", "width", "start", "target",
"diagonalFirst"?, "corners"?, "previous"?, "via"?}` → `{"points": [[x, y], …],
"reached", "viaFits"?}`: the walkaround path. It changes nothing. `corners`
is `"45"` (the default: no bend sharper than 45°) or `"90"` (right angles
allowed); `previous` is the direction of the segment that ends at `start`,
so the first bend is limited too. With `via` (a diameter), `viaFits` says
whether a via fits at the end.

`pcb.addTracks {"doc", "net", "width", "segments": [{"layer", "points"}],
"vias": [{"at", "diameter", "drill", "from"?, "to"?}]}` (`route`) adds the
tracks between consecutive points and the vias.

### `pcb.presets` / `pcb.applyPreset`

`pcb.presets` lists the [board presets](../guide/board-setup#presets).
`pcb.applyPreset {"doc", "preset": "standard-4l"}` (`board-preset`) sets the
stack-up of the board and the minimums and `Default` class of the rules.

### `pcb.dxfPreview` / `pcb.importDxf`

`pcb.dxfPreview {"path", "scale"?}` → `{"layers", "units", "count", "bounds",
"skipped"}`. `pcb.importDxf {"doc", "path", "layer", "layers"?, "replace"?,
"scale"?, "origin"?, "width"?}` (`import-dxf`) imports the shapes of the
chosen DXF layers onto a board layer (`outline`, `mech.1`, …); `scale` is
nanometres per drawing unit, `origin` moves the lower left corner there.

## Output jobs

The manufacturing outputs (see [Output jobs](../guide/outputs)). An output
job is a document of kind `outputJob`; its properties (`document.properties`)
are `{"name", "directory", "outputs": [...]}` and every output is
`{"id", "name", "kind", "enabled", "source"?, "options"?}` (see the
[file format](./file-format#output-jobs)). Change outputs with
`document.setProperties {"doc", "props": {"outputs": [...]}}`.

### `outputs.presets`

Lists the [presets for board makers](../guide/ordering#presets):
`[{"id", "name", "description", "outputs": [[name, kind, options], ...]}]`.
Works without an open project.

### `outputs.create`

`outputs.create {"name", "preset"?}` (`add-output-job`) adds an output job
with the outputs of a preset (default `generic`) → `{"id"}`.

### `outputs.applyPreset`

`outputs.applyPreset {"job", "preset"}` (`output-preset`) replaces the outputs
of a job with the outputs of a preset.

### `outputs.run`

`outputs.run {"job", "outputs"?, "labels"?}` generates the outputs of a job
(or only the listed output ids) and writes them into the job's folder inside
the project. `labels` translates the title block of schematic prints, as in
`view.scene`. The reply:

```json
{
  "directory": "/home/me/Projects/Blinky/outputs/generated",
  "results": [
    {
      "id": "…",
      "name": "Gerber",
      "kind": "gerber",
      "status": "done",
      "files": ["gerber/Blinky-Copper_Top.gtl", "…"]
    },
    {
      "id": "…",
      "name": "3D model",
      "kind": "step",
      "status": "done",
      "message": "1 3D models not found: ${KIPRJMOD}/3d/relay.step",
      "files": ["3d/Blinky.step"]
    }
  ],
  "files": [{ "path": "gerber/Blinky-Copper_Top.gtl", "bytes": 3712 }],
  "elapsedMs": 168
}
```

`status` is `done` (a 3D model output may add a `message` with the models
that were not found), `skipped` (switched off or not selected) or `failed`
(with `message`). Paths are relative to `directory`. The job
folder and every subfolder must lie inside the project folder.

### `outputs.bom`

`outputs.bom {"source"?, "options"?}` → `{"headers", "rows", "parts",
"fields"}`: the bill of materials for the given BOM options (columns,
grouping, …) without writing a file — the preview of the editor. `fields` lists
the fields that columns can show.

### `outputs.files`

`outputs.files {"job"}` → `{"directory", "files"}`: the Gerber and drill files
in the job's output folder (absolute paths), for the Gerber viewer.

### `gerber.scene`

`gerber.scene {"paths", "name"?, "encoding"?}` reads Gerber and Excellon
files (absolute, or relative to the project folder) and returns a scene like
[`view.scene`](#view), one layer per file, plus `"files": [{"file",
"function"}]` (the X2 file function, or one guessed from the extension) and
`"problems": [{"file", "message"}]`. Works without an open project.

## 3D view

The 3D view and the 3D exports (see [The 3D view](../guide/view-3d) and
[3D models](../guide/export-3d)). Lengths are millimetres with z up; the
board's bottom face lies at z = 0. Matrices are 4 × 4, column-major (as
glTF and three.js store them).

### `board3d.scene`

`board3d.scene {"doc", "inner"?}` builds the 3D board of a board document:
the laminate (outline minus holes), the copper, solder mask, silkscreen and
paste of each side, the plated walls of holes and, with `inner: true`, the
inner copper layers. The reply:

```json
{
  "board": {
    "name": "Blinky",
    "levels": { "bodyTop": 1.5558, "topCopper": 0.035, "bottomCopper": 0.035,
                "topMask": 0.01, "bottomMask": 0.01, "copperLayers": 4 },
    "colors": { "body": […], "topMask": […], "copper": […], "finish": "ENIG", … },
    "bounds": [100, 80, -0.045, 140, 110, 1.6008],
    "holes": 21,
    "warnings": []
  },
  "meshes": [
    { "key": "body", "vertices": 1520, "triangles": 1748,
      "positions": 0, "normals": 18240, "indices": 36480 }
  ],
  "parts": [ … as in board3d.parts … ],
  "models": [ … as in board3d.resolve … ],
  "data": "…"
}
```

`meshes` names the meshes (`body`, `barrels`, `copper:top`, `mask:bottom`,
`silk:top`, `paste:top`, `copper:in1`, …) and where they lie in the binary
buffer: positions and normals as little-endian `f32` triples, indices as
`u32` triples. JSON clients get the buffer base64-encoded in `data`; the
desktop app receives it as binary next to the JSON.

### `board3d.parts`

`board3d.parts {"doc"}` → `{"parts", "models"}`: only the parts, for live
updates while parts move (about 7 ms for 5,000 parts). Every part:

```json
{
  "id": "…",
  "designator": "U1",
  "value": "NE555",
  "footprint": "Package_SO:SOIC-8_3.9x4.9mm_P1.27mm",
  "side": "top",
  "dnp": false,
  "matrix": [ … 16 numbers: the footprint placement … ],
  "pads": [-2.95, -2.105, 2.95, 2.105],
  "models": [
    { "path": "${KICAD10_3DMODEL_DIR}/Package_SO.3dshapes/SOIC-8_3.9x4.9mm_P1.27mm.step",
      "matrix": [ … model coordinates → board … ] }
  ]
}
```

A model's matrix applies KiCad's convention: the footprint's position and
rotation, a half turn about the footprint's y axis for parts on the bottom,
then the model's offset, its rotation by the negated angles (z, y, x) and its
scale. `pads` is the box of the pads in footprint coordinates (for parts
without a model).

### `board3d.resolve`

`board3d.resolve {"path"}` → `{"path", "file"?, "format"?, "entry"?,
"mesh"?}`: where a model path was found — a `file` on this computer
(`format` `step` or `vrml`), the base library's model `entry` with the hash
of its `mesh` in the mesh cache, both, or neither. See
[Parts and their 3D models](../guide/view-3d#parts-and-their-3d-models) for
the search order. Works without an open project (relative paths then find
nothing).

### `board3d.export`

`board3d.export {"doc", "path", "format"?, "origin"?, "includeDnp"?,
"units"?}` writes the board as a 3D model to `path` (absolute): `format`
`step` (default), `glb` or `vrml`; `origin` `drill` (default), `center` or
`absolute`; `units` (VRML) `mm` (default), `m` or `inch`. The reply:

```json
{
  "placed": 8,
  "exact": 5,
  "faceted": 0,
  "missing": [],
  "warnings": [],
  "exactBody": true,
  "bytes": 390553,
  "path": "/Users/me/Mechanics/blinky.step"
}
```

`placed` counts the parts written with a model, `exact` the STEP files
merged as they are, `faceted` the models written from meshes, `missing` the
model paths that were not found. `exactBody` tells whether the board body
uses the exact outline and holes (else polygons).

## Library editor

The methods of the [library editor](../guide/library-editor). Items are
addressed by library document (`doc`), `kind` (`symbol`, `footprint` or
`component`) and `name`. Edits commit one undo step with the label in
brackets and return their reply plus `history`.

### `libedit.list`

`{"doc"}`. The items of a library: `{"name", "symbols", "footprints",
"components"}`, each sorted by name, with `name`, `description` and
`checks` (`{"errors", "warnings", "infos"}`); symbols also `units`, `pins`,
`alternate` and `power`, footprints `pads` and `models`, components
`symbol`, `footprint` and `mpn`.

### `libedit.item`

`{"doc", "kind", "name"}`. `{"def", "usage", "findings"}`: the definition in
the [file format](./file-format), where it is used (`{"placed", "board",
"symbols", "components"}`) and the check findings (see `libedit.check`).

### `libedit.create`

`{"doc", "kind", "name", "copyOf"?, "def"?, "wizard"?}` (`create-library-item`).
An empty item, a copy of another item of the library (`copyOf`), a given
definition (`def`) or a footprint from a wizard (see `libedit.wizard`). Returns
`{"name"}`; a name in use fails with `conflict`.

### `libedit.delete`

`{"doc", "kind", "name", "force"?}` (`delete-library-item`). An item in use
fails with `conflict` unless `force` is set. Returns `{"usage"}`.

### `libedit.rename`

`{"doc", "kind", "name", "new"}` (`rename-library-item`). Renames the item and
every reference: placed symbols, board footprints, default footprints of
symbols, symbols and footprints of components. Returns `{"name",
"changes"}`.

### `libedit.setProps`

`{"doc", "kind", "name", "props"}` (`edit-library-item`). Changes the item's
own properties as a JSON merge patch: `{"description": "…",
"pinText": {"hideNumbers": true}, "properties": {"Tolerance": "1%", "Old":
null}}`. `name`, `id`, `type`, `units`, `pads`, `graphics`, `texts` and
`models` cannot be set here (`invalidParams`): use `libedit.rename` and the
edits below.

### `libedit.symbol`

`{"doc", "name", "body"?: {"unit", "alternate"}, "edit"}` (`edit-symbol`).
One edit of a symbol body (default: unit 1, normal). Returns `{"body",
"select"}`: the body to show afterwards (a new unit, the alternate body) and
the items to select (`[{"kind": "pin", "index": 3}]`). Edits (`op`):

| `op`                                       | Fields                            | Effect                                                                                        |
| ------------------------------------------ | --------------------------------- | --------------------------------------------------------------------------------------------- |
| `move`                                     | `items`, `delta`                  | Moves items                                                                                   |
| `duplicate`                                | `items`, `offset`                 | Copies items; pins get the next free numbers                                                  |
| `delete`                                   | `items`                           | Deletes items                                                                                 |
| `orient`                                   | `items`, `orientation`, `center`? | `left`/`right` turns by 90°, `horizontal`/`vertical` mirrors                                  |
| `addPin`                                   | `pin`                             | An empty number gets the next free one                                                        |
| `addPins`                                  | `pins`                            | Several pins (a pasted list), numbered likewise                                               |
| `addGraphic`                               | `graphic`                         | A line, rectangle, circle, arc, polyline or polygon                                           |
| `addText`                                  | `text`                            | A text                                                                                        |
| `updatePin`, `updateGraphic`, `updateText` | `index`, `pin`/`graphic`/`text`   | Replaces one item                                                                             |
| `setPins`                                  | `indices`, `patch`                | Sets `name`, `number`, `kind`, `shape`, `direction`, `length`, `hidden`, `at` of several pins |
| `renumber`                                 | `indices`, `start`, `step`?       | Numbers pins in the given order (`1`, `A1`)                                                   |
| `addUnit`                                  | `copy`?                           | A new unit, empty or a copy of the shown one                                                  |
| `removeUnit`                               |                                   | Removes the shown unit (not the last one)                                                     |
| `nameUnit`                                 | `name`                            | Names the shown unit                                                                          |
| `addAlternate`                             |                                   | A De Morgan body for the shown unit (a copy of the normal one)                                |
| `removeAlternate`                          |                                   | Removes it                                                                                    |

Items are `{"kind": "pin" | "graphic" | "text", "index"}`.

### `libedit.footprint`

`{"doc", "name", "edit"}` (`edit-footprint`). One edit of a footprint;
returns `{"select"}`. Besides `move`, `duplicate`, `delete`, `orient`,
`addGraphic`, `addText`, `updateGraphic`, `updateText` and `renumber`
(pads), which work as for symbols:

| `op`          | Fields                                                | Effect                                                                                                                                                             |
| ------------- | ----------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `addPad`      | `pad`                                                 | An empty number gets the next free one                                                                                                                             |
| `addPadArray` | `pad`, `columns`, `rows`, `pitchX`, `pitchY`, `start` | A grid of pads like `pad`, numbered row by row from `start`                                                                                                        |
| `updatePad`   | `index`, `pad`                                        | Replaces one pad                                                                                                                                                   |
| `setPads`     | `indices`, `patch`                                    | Sets `number`, `kind`, `shape`, `size`, `drill`, `layers`, `rotation`, `at`, `maskMargin`, `pasteMargin` of several pads; a new `kind` also fixes layers and drill |
| `addModel`    | `model`                                               | A 3D model (`path`, `offset`, `rotation`, `scale`, `hidden`)                                                                                                       |
| `updateModel` | `index`, `model`                                      | Replaces one model                                                                                                                                                 |
| `courtyard`   | `margin`                                              | Replaces the top courtyard by a rectangle around pads and fabrication outline                                                                                      |

Items are `{"kind": "pad" | "graphic" | "text" | "model", "index"}`.

### `libedit.check`

`{"doc", "kind"?, "name"?}`. The [library checks](../guide/library-checks) of
a whole library, one kind or one item: `[{"kind", "name", "findings"}]` for
the items with findings. A finding has `code`, `severity` (`error`,
`warning`, `info`), `message` (English), `args` (for translations) and
`target` (`{"kind": "pin" | "pad" | "unit", "unit"?, "alternate", "index"?}`).

### `libedit.wizard` / `libedit.presets`

`libedit.presets` returns the [wizard](../guide/footprint-wizard) presets:
`[{"name", "wizard"}]`. `libedit.wizard` (`{"wizard", "name"?}`) computes a
footprint without adding it: `{"name", "def", "findings"}`, with the
suggested name unless `name` is given. A wizard is one of:

```json
{ "kind": "chip", "bodyLength": 1600000, "bodyWidth": 800000, "terminal": 300000, "density": "nominal" }
{ "kind": "gullWing", "pins": 8, "quad": false, "pitch": 1270000, "leadSpan": 6000000,
  "leadLength": 835000, "leadWidth": 410000, "bodyWidth": 3900000, "bodyLength": 4900000 }
{ "kind": "noLead", "pins": 32, "quad": true, "pitch": 500000, "bodyWidth": 5000000,
  "bodyLength": 5000000, "terminalLength": 400000, "terminalWidth": 250000,
  "exposedPad": [3450000, 3450000] }
{ "kind": "bga", "rows": 8, "columns": 8, "pitch": 800000, "ball": 400000,
  "bodyWidth": 7000000, "bodyLength": 7000000 }
{ "kind": "header", "rows": 2, "pinsPerRow": 5, "pitch": 2540000, "drill": 1000000, "pad": 1700000 }
```

`density` is `least`, `nominal` (default) or `most`; lengths are nanometres.

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

### `import.archivePreview`

| Parameter  | Type   | Meaning                                                              |
| ---------- | ------ | -------------------------------------------------------------------- |
| `path`     | string | The download: a ZIP file, `.kicad_sym`, `.kicad_mod` or STEP file    |
| `nickname` | string | Prefix of the names (default: the provider, else the file name)      |
| `doc`      | id     | Target library, to mark what it has already (default: the first one) |

Reads a part download of an [online part library](../guide/online-parts)
without changing anything. Works without an open project. The reply:

```json
{
  "file": "NE555-DEMO.zip",
  "provider": "SnapEDA",
  "nickname": "SnapEDA",
  "modelFolder": "3d/SnapEDA",
  "symbols": [
    {
      "name": "SnapEDA:NE555-DEMO",
      "description": "…",
      "footprint": "SnapEDA:SOIC127P600X175-8N",
      "pins": 8,
      "exists": false
    }
  ],
  "footprints": [
    {
      "name": "SnapEDA:SOIC127P600X175-8N",
      "description": "…",
      "pins": 8,
      "exists": false,
      "models": ["${KIPRJMOD}/3d/SnapEDA/NE555-DEMO.step"],
      "duplicate": { "id": 412, "name": "Demo:SOIC-8", "class": "exact" }
    }
  ],
  "models": [{ "file": "NE555-DEMO.step", "bytes": 1960, "used": true }],
  "problems": []
}
```

`provider` is `SnapEDA`, `UltraLibrarian` or `null`. `duplicate` is the base
library's best match: `exact`, `pads` or `near` for footprints, `exact` or
`pins` for symbols. A ZIP may not unpack outside its folder, have more than
2,000 entries or more than 512 MB.

### `import.archive`

| Parameter    | Type     | Meaning                                                          |
| ------------ | -------- | ---------------------------------------------------------------- |
| `path`       | string   | The download                                                     |
| `nickname`   | string   | Prefix of the names                                              |
| `doc`        | id       | Target library (default: the first library of the project)       |
| `useLibrary` | string[] | Downloaded footprints to replace by their base library duplicate |

Imports the download as one undoable step (`Import part download`): the
symbols and footprints the library does not have yet, the used 3D models
copied into the project folder. Returns `{"symbols", "footprints",
"models", "replaced", "skipped", "nickname", "problems", "history"}`.

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

### `library.model`

`{"path": "${KICAD9_3DMODEL_DIR}/Package_SO.3dshapes/SOIC-8….step"}`. The
base library entry of a footprint's 3D model path: `{"key", "id"}`, where
`key` is `Package_SO/SOIC-8…` (variables and extension ignored) and `id` the
model entry for `library.get`, or `null` if the model is not in the base
library (or no library is set up). The footprint editor uses it for its 3D
preview.

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
  worker thread (OpenCASCADE) and caches it as `meshes/step-<hash>.glb`,
  named by the first 32 hex digits of the file's SHA-256 (the engine finds
  these meshes for [GLB and VRML exports](../guide/export-3d)); returns
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
