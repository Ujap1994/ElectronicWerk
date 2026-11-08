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
