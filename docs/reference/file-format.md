# Native file format

ElectronicWerk stores projects as plain, readable JSON files. This page
describes the format, so you can read the files, review them in git, and write
tools that process them. The format is defined by the Rust types in
[`crates/ew-model`](https://github.com/Ujap1994/ElectronicWerk/tree/main/crates/ew-model)
and written by
[`crates/ew-format`](https://github.com/Ujap1994/ElectronicWerk/tree/main/crates/ew-format).

## Files

| Extension  | Content                          | Format tag                 |
| ---------- | -------------------------------- | -------------------------- |
| `.ewproj`  | Project manifest                 | `electronicwerk.project`   |
| `.ewsch`   | Schematic sheet                  | `electronicwerk.schematic` |
| `.ewpcb`   | Board                            | `electronicwerk.pcb`       |
| `.ewlib`   | Library (symbols and footprints) | `electronicwerk.library`   |
| `.ewrules` | Design rules                     | `electronicwerk.rules`     |
| `.ewout`   | Output job                       | `electronicwerk.outputJob` |

Every file starts with the same header:

```json
{
  "format": "electronicwerk.pcb",
  "schemaVersion": 1,
  "id": "4f0c3b2e-7f39-4d5e-9a51-0d2c8e1a9b77",
  "name": "Board",
  …
}
```

- `format` names the kind of file.
- `schemaVersion` is the version of the format. ElectronicWerk reads every
  earlier version and upgrades it on save (see [Versions](#versions-and-migrations)).
- `id` is the document id; it must match the id in the manifest.

## Conventions

| Topic         | Rule                                                                                                                                                                                                                   |
| ------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Lengths       | Integer **nanometres** (1 mm = 1 000 000). No rounding errors, exact comparisons.                                                                                                                                      |
| Points, sizes | Arrays: a point is `[x, y]`, a size `[width, height]`.                                                                                                                                                                 |
| Axes          | x points right, **y points up** (as in Gerber), origin at the board or sheet origin.                                                                                                                                   |
| Angles        | Degrees, counter-clockwise, `0` to `<360`, with up to three decimals (`90`, `22.5`).                                                                                                                                   |
| Ids           | Every object has a UUID (`"id": "…"`), stable for its whole life. References use ids, never names or positions.                                                                                                        |
| Field names   | `camelCase`. Default values are left out to keep files small.                                                                                                                                                          |
| Layers        | Short names: `cu.top`, `cu.in1` … `cu.in62`, `cu.bottom`, `silk.top`, `mask.bottom`, `paste.top`, `fab.top`, `courtyard.top`, `adhesive.top`, `outline`, `mech.1`, `user.1`. Pads may use `cu.*`, `mask.*`, `paste.*`. |
| Text          | UTF-8, `\n` line endings, two-space indentation, a final newline.                                                                                                                                                      |

## Deterministic layout

The same project always produces exactly the same bytes, and small changes
produce small diffs:

- Fields appear in a fixed order (the order of the schema).
- Objects of one kind are stored as an array **sorted by id**, so creating
  objects in a different order does not change the file.
- A container that fits into 100 characters stays on one line.
- An object inside an array (one track, one wire, one symbol) is written on
  **one line**, as long as none of its values is itself long. Moving a track
  therefore changes exactly one line.

Example (`pcb/board.ewpcb`, shortened):

```json
{
  "format": "electronicwerk.pcb",
  "schemaVersion": 1,
  "id": "0a64e4a4-91f7-4c05-9e44-1c5c9b8c7d2e",
  "name": "Board",
  "stackup": {
    "layers": [
      { "type": "silkscreen", "side": "top", "color": "white" },
      { "type": "solderMask", "side": "top", "thickness": 10000, "color": "green" },
      { "type": "copper", "layer": "cu.top", "thickness": 35000 },
      {
        "type": "dielectric",
        "kind": "core",
        "thickness": 1510000,
        "material": "FR4",
        "epsilonR": 4.5,
        "lossTangent": 0.02
      },
      { "type": "copper", "layer": "cu.bottom", "thickness": 35000 },
      { "type": "solderMask", "side": "bottom", "thickness": 10000, "color": "green" },
      { "type": "silkscreen", "side": "bottom", "color": "white" }
    ],
    "finish": "HASL"
  },
  "nets": [{ "id": "1b6f…", "name": "GND", "class": "Power" }],
  "tracks": [
    {
      "id": "05c1…",
      "layer": "cu.top",
      "net": "1b6f…",
      "width": 250000,
      "start": [12700000, 5080000],
      "end": [25400000, 5080000]
    }
  ],
  "shapes": [
    {
      "id": "9e2d…",
      "layer": "outline",
      "geometry": { "rect": { "start": [0, 0], "end": [100000000, 100000000] } },
      "width": 100000
    }
  ]
}
```

## The manifest (`.ewproj`)

| Field           | Type   | Meaning                                                     |
| --------------- | ------ | ----------------------------------------------------------- |
| `id`            | UUID   | Project id                                                  |
| `name`          | string | Project name                                                |
| `revision`      | string | Revision (optional)                                         |
| `description`   | string | Description (optional)                                      |
| `variables`     | object | User text variables, `{"AUTHOR": "Lab"}` (optional)         |
| `documents`     | array  | `{"id", "kind", "path"}` per document, in tree order        |
| `rootSchematic` | UUID   | The top sheet (optional)                                    |
| `settings`      | object | `units` (`mm`, `mil`, `inch`), `schematicGrid`, `boardGrid` |

`kind` is one of `schematic`, `pcb`, `library`, `rules`, `outputJob`. `path`
is relative to the project folder, uses `/` and must not leave the folder.

## Schematic sheets (`.ewsch`)

| Field                                   | Content                                                                                                                                                                                                                                                                                                 |
| --------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `page`                                  | `format` (`a5` … `a0`, `letter`, `legal`, `tabloid`, `custom`), `orientation`, custom `size`                                                                                                                                                                                                            |
| `titleBlock`                            | `title`, `date`, `revision`, `company`, `comments` (text variables allowed)                                                                                                                                                                                                                             |
| `symbols`                               | Placed symbols: `symbol` (library name), `unit`, `at`, `rotation`, `mirror`, `designator`, `value`, `footprint`, `component` (links the units of a part and its footprint), `properties`, `fields`, `flags` (`dnp`, `excludeFromBom`, `excludeFromBoard`), `instances` (designators per sheet instance) |
| `wires`, `buses`                        | Segments with `start` and `end`                                                                                                                                                                                                                                                                         |
| `busEntries`, `junctions`, `noConnects` | Points                                                                                                                                                                                                                                                                                                  |
| `labels`                                | Net labels: `kind` (`local`, `global`, `hierarchical`), `text`, `at`, `shape`                                                                                                                                                                                                                           |
| `powerPorts`                            | Power ports: `net`, `at`, `style` (`bar`, `ground`, …)                                                                                                                                                                                                                                                  |
| `sheets`                                | Sheet symbols: `name`, `schematic` (child document id), `at`, `size`, `pins`                                                                                                                                                                                                                            |
| `texts`, `shapes`                       | Free text and graphics                                                                                                                                                                                                                                                                                  |

## Boards (`.ewpcb`)

| Field             | Content                                                                                                                                                                                                          |
| ----------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `stackup`         | Physical layers from top to bottom (silkscreen, solder mask, copper, dielectric) and the surface finish                                                                                                          |
| `origin`          | Drill and placement origin                                                                                                                                                                                       |
| `nets`            | `name` and net `class`                                                                                                                                                                                           |
| `footprints`      | Placed footprints with their own copy of pads, graphics, texts and 3D models (as seen from the top; `side: "bottom"` mirrors them), `component`, `designator`, `value`, `at`, `rotation`, `locked`, `attributes` |
| `tracks`          | `layer`, `net`, `width`, `start`, `end`, and `mid` for arcs                                                                                                                                                      |
| `vias`            | `at`, `diameter`, `drill`, `net`, `from`/`to` layers (default through), `micro`                                                                                                                                  |
| `zones`           | Copper pours and keep-outs: `net`, `layers`, `outline`, `holes`, `priority`, `keepout`, `fill` settings, and `filled` (the computed copper per layer: `layer`, `points`; for example from a KiCad import)        |
| `texts`, `shapes` | Free text and graphics; the board outline is drawn on the `outline` layer                                                                                                                                        |

## Libraries, rules and output jobs

- **Library** (`.ewlib`): `symbols` (with `units`, `pins`, `graphics`, and
  `pinText`: `hideNumbers`, `hideNames` and `nameOffset`, where an offset of
  0 puts pin names above the pin line instead of inside the body) and
  `footprints` (with `pads`, `graphics`, `texts`, `models`, `attributes`).
- **Rules** (`.ewrules`): `constraints` (minimum clearance, track width, via,
  drill, annular ring, hole-to-hole, copper-to-edge, silkscreen width),
  `netClasses` and pattern-based `assignments` (`GND*` → `Power`).
- **Output job** (`.ewout`): `directory` and the ordered list of `outputs`
  (`kind`: `gerber`, `drill`, `bom`, `pickAndPlace`, `pdf`, `step`, …).

## Versions and migrations

Each file kind has its own chain of upgrade steps. When ElectronicWerk reads a
file with an older `schemaVersion`, it upgrades the data step by step in
memory. The next save writes the current version and first copies the
original files to `.ewerk/backup/schema-v<N>/`. Files with a newer version than
the app supports are refused with a clear message instead of being
misinterpreted.

`ewerk project upgrade <project>` rewrites every file in the current version
in one go.

Version 1 is the first version of the format (ElectronicWerk 0.2).
ElectronicWerk 0.4 added the optional fields `filled` (zones) and `pinText`
(symbols) without a new version: they are left out when they are empty or
have their default value, and older versions of the app ignore them.

## Local state (`.ewerk/`)

| File                             | Purpose                                                                   |
| -------------------------------- | ------------------------------------------------------------------------- |
| `.ewerk/journal.jsonl`           | [Autosave journal](../guide/undo-and-recovery) (one JSON line per change) |
| `.ewerk/journal.discarded.jsonl` | The last journal you chose to discard                                     |
| `.ewerk/backup/schema-v<N>/`     | Originals of files upgraded from format version N                         |

The folder is machine-local and listed in the project's `.gitignore`.
