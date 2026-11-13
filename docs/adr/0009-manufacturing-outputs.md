# ADR-0009: Manufacturing outputs and output jobs

- **Status:** accepted
- **Date:** 2026-11-13
- **Milestone:** M8

## Context

M8 produces the files a board maker and an assembly house need: Gerber
files with X2 attributes, Excellon drill files and a drill map, the bill of
materials, pick-and-place files, IPC-D-356 and IPC-2581 netlists, PDF and
SVG prints of schematics and boards, assembly drawings and a ZIP package.
An **output job** document (`.ewout`, introduced with projects in M1)
describes which of these to generate; one click must generate all of them,
and presets must set them up for common board makers. A built-in Gerber
viewer shows the generated files, and the roadmap asks for the Gerber
output to be verified against reference renders.

The constraints of the earlier milestones hold (ADR-0001, ADR-0002,
ADR-0008): the engine owns the data, everything is scriptable through the
engine API and the CLI, and the board geometry of `ew-pcb` (cores with a
radius) is the single source of truth for copper. Outputs add two
requirements of their own:

- **Correctness over features.** A Gerber file with a missing pad or a
  wrongly mirrored layer costs a board order. Every output must be checked
  against an independent reference, not only against itself.
- **Reproducibility.** The same project must give byte-identical files
  (apart from the creation date), so that generated files can be committed
  and compared in git.

## Decision

### One crate for outputs

The new crate `ew-output` holds every file format and the job runner. It
depends on `ew-model`, `ew-pcb`, `ew-scene` and `ew-schematic`, and on
`flate2` (PDF streams, ZIP) and `zip`. It has no I/O: every function returns
`OutputFile { name, data }` values in memory, and only the engine
(`ew-core::outputs`) writes them to disk. This keeps the formats testable
without temporary folders and lets the CLI, the engine and the tests share
one implementation.

### Board art: one list of features per layer

`art::BoardArt` turns a board into **features** per fabrication layer —
flashes (pads, vias), lines and arcs (tracks, graphics, text strokes) and
regions (zone fills, filled shapes) — each with its function (`SMDPad`,
`ViaPad`, `Conductor`, …) and its net, pin and component. Every output that
draws the board uses these features: Gerber, IPC-2581, board prints,
assembly drawings and the drill map. Options decide what a layer contains:
tented vias, paste for parts that are not placed, values and designators on
the silkscreen.

Pads come from the shapes of `ew-pcb`, the same shapes the DRC and the zone
filler use (ADR-0008), so a pad in the Gerber file is exactly the pad the DRC
checked.

### Gerber X2: standard apertures and macros from cores

Files are written in **millimetres with six decimals** (`%FSLAX46Y46*%`), so
one file unit is one nanometre, the unit of the model: coordinates are
written without rounding. The model's y axis points up like Gerber's; no
mirroring is needed.

- Circles, rectangles and ovals use the standard apertures `C`, `R` and `O`.
- Every other pad shape (rounded rectangles, chamfered rectangles,
  trapezoids, custom pads) becomes an **aperture macro** built from the
  shape's core and radius: an outline primitive for the core polygon,
  vector lines along its edges and circles at its corners, each with the
  width of twice the radius. The union of these primitives is exactly the
  core grown by the radius. Macros have no parameters: the shape is written
  in its final orientation relative to the flash point, and identical shapes
  share one macro, so readers that handle macro rotation or variables
  poorly still draw every pad correctly. Rectangles, ovals and circles that
  are not axis-aligned stay standard apertures where the orientation
  allows, and become macros otherwise.
- Zone fills become `G36`/`G37` regions. Holes are joined to their outline
  by zero-width **cut-ins** (`fracture.rs`), the representation Gerber
  readers support best.
- X2 attributes: `.FileFunction`, `.FilePolarity` (solder mask negative),
  `.GenerationSoftware`, `.CreationDate`, `.ProjectId` and `.SameCoordinates`
  on files; `.AperFunction` on apertures; `.N`, `.P` and `.C` object
  attributes for nets, pins and components. Values are escaped (`\uXXXX`)
  where the specification requires it. A Gerber job file (`.gbrjob`) lists
  the layers, the stack-up and the board size.
- Attributes and the job file can be switched off for CAM systems that
  reject them, and Protel extensions (`.gtl`, `.gbs`, …) can be chosen.

### Excellon: one file per hole kind

Drill files are written in metric decimal format with `; #@! TF/TA`
comments for the file and tool functions, as KiCad does. Plated and
non-plated holes go into separate files (or one, for makers that want it);
blind and buried vias get one file per layer pair. Slots are written with
`G85`. Tools are sorted by diameter and numbered from 1.

### Verification: a reader, a rasterizer and KiCad references

`read.rs` is a Gerber (RS-274X with X2) and Excellon reader: standard
apertures, macros with variables and arithmetic expressions, lines, arcs
(`G74`/`G75`), regions with several contours, polarity and step-and-repeat.
It produces an image of dark and clear shapes in `ew-pcb` geometry, which
`raster.rs` fills into a bitmap with a non-zero scanline rule.

The reference is **KiCad 10**: the Blinky test project is imported, its
Gerber and drill files are generated by ElectronicWerk and by `kicad-cli`
from the original KiCad board, and both sets are rasterized at 20 µm per
pixel and compared pixel by pixel. Copper, solder mask, paste, board
outline and drill files of the two tools are identical apart from 2 pixels
of arc approximation on the bottom copper (out of 34,000); the test allows
1 % for curve approximation. Silkscreen layers only get a coarse check,
because the fonts differ. The KiCad files are committed as
golden files (`crates/ew-output/tests/fixtures/blinky-kicad`), so CI does
not need KiCad. A second test writes every pad kind, shape and rotation and
compares the reader's image of the file with the model's geometry.

The IPC-D-356 netlist is compared record by record with KiCad's; the
IPC-2581 file is checked for completeness (every layer, net, pad and
component) and for well-formed XML.

### Prints: the scene as vector drawing

Schematic prints draw exactly what the viewer shows. The scene builder of
`ew-scene` (ADR-0003) gains a **recording mode**: next to the triangles for
the graphics card it keeps the vector primitives — lines, arcs, circles and
polygons as outlines — as a `Plot`. Prints convert the plot into pages of a
small canvas model (`canvas.rs`: paths of lines and cubic curves, fills and
strokes, text labels) with print colours per layer. Board prints and
assembly drawings use the board features instead.

Two back ends write the canvas: `pdf.rs` (PDF 1.7 with compressed streams,
a bookmark per page and UTF-16 document information) and `svg.rs` (one file
per page in millimetres). Both are written from scratch; a general PDF
library would add far more than the few operators prints need.

### An original stroke font

Plotted text — silkscreen in Gerber files, texts in prints — needs a stroke
font. Instead of copying an existing one, ElectronicWerk has its own
monospace stroke font (`font.rs`) with the metrics of the viewer's font
(advance 0.6 of the size, cap height equal to the size), so a text covers
the same area on paper and on the board as on screen. It covers ASCII, the
German letters and common electronics symbols (`µ`, `Ω`, `°`, `±`, …).
PDF prints add each text as invisible text (render mode 3), so designators
and nets can be searched and copied.

### Output jobs: a pure runner with presets

`job::run(job, only, context)` generates the outputs of a job in memory and
returns the files with a result per output (done, skipped, failed, with
messages). Each output kind has an options struct with defaults, read from
the job's option map; unknown options are ignored and missing ones take
their default, so older and newer job files work. An output that needs a
board is skipped in a project without one.

The engine (`outputs.run`) resolves text variables, runs the job and writes
the files into the job's output folder with atomic writes. It rejects every
path that leaves the project folder (`..`, absolute and rooted paths, drive
prefixes).

Presets (`generic`, `jlcpcb`, `pcbway`, `aisler`, `eurocircuits`,
`oshpark`) are plain lists of outputs with options. Applying a preset
replaces the outputs of a job in one undoable change. The generic preset is
also the job of new projects; a test keeps both in sync.

### Gerber viewer: the reader plus the scene builder

The Gerber viewer reads files with the same reader as the tests and draws
them with the scene builder of the 2D view, one layer per file (colour from
the X2 file function or the extension). Clear polarity is resolved with the
polygon operations of `ew-pcb` before drawing. It runs in the engine
(`gerber.scene`) and reuses the viewer's renderer, layer panel and
navigation unchanged.

### Not in this milestone

- **ODB++** is not written. Its specification is published by Siemens under
  a licence agreement, the format is a directory tree of many files with a
  proprietary feature set, and every maker that accepts ODB++ also accepts
  Gerber X2 or IPC-2581, which carry the same information (layers, nets,
  components, stack-up) in an open standard. The effort is not justified
  for the users of ElectronicWerk; it can be added as another output kind
  if makers ask for it.
- **IPC-2581** is written as a subset of revision C (content, logistic
  header, BOM, ECAD with stack-up, layers, profiles, packages, components,
  nets and layer features). The XSD schema could not be obtained for
  automated validation; the structure follows the specification and is
  checked by the tests.
- **STEP** export of the board needs the 3D view and model handling of M9
  and moves there.

## Consequences

- One click generates every manufacturing file; the same jobs run from the
  CLI (`ewerk outputs run`) and through the engine API, for scripts and CI.
- The Gerber and drill output is verified against KiCad 10 on every CI run;
  regressions in pads, tracks, zones, mask or drills fail the build.
- Generated files are reproducible: the same project gives the same files,
  apart from the creation date.
- The scene recording ties prints to the viewer: a change of the schematic
  drawing changes screen and print together.
- Not yet covered: ODB++, STEP, panelisation, fabrication drawings with
  dimensions and stack-up tables, and per-maker validation of uploads. They
  fit as new output kinds of the job runner.
