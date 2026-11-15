# Design statistics

The design statistics count what your design consists of: sheets, parts,
nets, BOM lines, pads, holes, vias, track length and how much of the board
is routed. Use them to get a price quote (board makers ask for the size,
the layer count and the number of holes and SMD pads), to check the
progress of a layout, or to compare two variants of a design.

![The design statistics in the project details](/screenshots/statistics.png)

## Where to find them

- **Project details:** select the project (the top entry of the
  [project tree](./project-tree)). The statistics are the last panel of the
  project overview. They follow every change: place a part or route a track
  and the numbers update a moment later.
- **Project documentation:** the [documentation output](./documentation)
  can include them as a section.
- **Command line:** `ewerk project stats` prints them, `--json` for
  scripts (see [below](#from-the-command-line)).
- **Engine:** scripts and AI agents call
  [`project.statistics`](../reference/engine-api#project-statistics).

Lengths are shown in the project's units (millimetres, mil or inches; see
[project settings](./project-tree#project-properties)).

## Schematic

| Figure                | Meaning                                                                                                                                                |
| --------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **Sheets**            | Schematic sheets, the root sheet included.                                                                                                             |
| **Parts**             | Placed symbols with a designator (power symbols and graphics are not counted). Multi-unit parts count once.                                            |
| **Pins**              | Pins of all parts.                                                                                                                                     |
| **Nets**              | Nets with at least one pin.                                                                                                                            |
| **Named nets**        | Nets with a name from a label, a power port or a sheet pin. The others get a generated name.                                                           |
| **Nets with one pin** | Nets that connect a single pin to nothing. Most of them are mistakes — a missing wire or a mistyped label — unless the pin is marked as not connected. |

## Bill of materials

The figures are counted as in the [bill of materials](./bom-placement)
output, for the first board of the project:

| Figure                  | Meaning                                                                                                       |
| ----------------------- | ------------------------------------------------------------------------------------------------------------- |
| **BOM lines**           | Different parts: one line per value, footprint and part number. This is the number of reels or bags you need. |
| **Parts to place**      | Every part that is fitted (not marked _Do not populate_).                                                     |
| **Not populated**       | Parts marked _Do not populate_.                                                                               |
| **Without footprint**   | Parts that have no footprint yet — they cannot go on the board.                                               |
| **Without part number** | Parts without an `MPN` field. An assembly service needs a part number for each part it buys.                  |

## Boards

One block per board of the project:

| Figure                                                        | Meaning                                                                    |
| ------------------------------------------------------------- | -------------------------------------------------------------------------- |
| **Size**                                                      | Width and height of the board outline.                                     |
| **Area**                                                      | Area of the board, cut-outs subtracted, in cm².                            |
| **Copper layers**, **Thickness**                              | From the [layer stack-up](./board-setup#stack-up).                         |
| **Footprints**, **Top side**, **Bottom side**                 | Footprints on the board, and on each side.                                 |
| **SMD**, **Through-hole**                                     | Footprints by mounting type (the footprint attribute).                     |
| **Pads**, **SMD pads**, **Through-hole pads**                 | All pads, and the pads by kind. Assembly services often price by SMD pads. |
| **Holes**                                                     | Drilled holes: pads and vias.                                              |
| **Vias**                                                      | Vias.                                                                      |
| **Zones**                                                     | Copper zones and keep-outs.                                                |
| **Track segments**, **Track length**                          | Straight pieces and arcs, and their total length.                          |
| **Thinnest track**, **Smallest via drill**, **Smallest hole** | The values a board maker checks against its capabilities.                  |
| **Parts per cm²**, **Pads per cm²**                           | How dense the board is.                                                    |

Below the figures:

- **Routing**: how many of the connections the board needs are already
  routed, as a percentage and a bar, with the number of unrouted
  connections. A net with _n_ pads needs _n_ − 1 connections; the unrouted
  ones are the lines of the [ratsnest](./layout-editor).
- **Holes by diameter**: plated and non-plated holes per drill size —
  the same table a drill file has.
- **Track length per layer**: how much copper is routed on each layer.

## From the command line

```sh
ewerk project stats "Sensor Board"
```

```text
Schematic: sheets 1, parts 50, pins 297, nets 57 (named 57, with one pin 0)
BOM:       lines 6, parts to place 50, not populated 0, without footprint 0, without part number 50
Board Sensor Board: 80.0 × 60.0 mm, 48.0 cm², copper layers 2
  footprints 50 (top 45, bottom 5; SMD 48, THT 2), pads 297, vias 242
  tracks: segments 600, length 962.2 mm; thinnest 0.25 mm, smallest hole 0.3 mm
  hole 0.3 mm: 242 plated, 0 non-plated
  hole 1.0 mm: 20 plated, 0 non-plated
  routing 99.2 % (unrouted 2 of 237 connections)
```

With `--json`, the same figures as the engine method
[`project.statistics`](../reference/engine-api#project-statistics) — in
millimetres, square centimetres and fractions (routing completion `0.992`).
See [`ewerk project stats`](../reference/cli#ewerk-project-stats).
