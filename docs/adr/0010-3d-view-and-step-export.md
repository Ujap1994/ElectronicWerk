# ADR-0010: The live 3D view and the STEP export

- **Status:** accepted
- **Date:** 2026-11-13
- **Milestone:** M9

## Context

M9 shows the board in 3D and exports it for mechanical CAD. The roadmap
asks for:

- a board body from the outline (holes, cut-outs, thickness from the
  stack-up) with copper, mask, silkscreen and paste;
- the parts' 3D models from the mesh cache, placed with the footprints'
  offsets;
- **live updates**: a part moved in the layout moves in 3D within 250 ms;
- an optional **3D window** for a second monitor with a synchronized
  selection;
- camera presets, sections, measurements and screenshots;
- a STEP export of the whole assembly, and VRML/GLB exports.

ADR-0001 chose three.js for the 3D view and OpenCASCADE for STEP, first
through its WebAssembly build `occt-import-js` (reading and tessellating,
ADR-0005) and "later natively". The roadmap names a STEP export "with native
OpenCASCADE".

The constraints of the earlier milestones hold: the engine owns the data
and computes everything that the CLI and the scripts need too; the board
geometry of `ew-pcb` and the board art of `ew-output` (the features of the
Gerber files, ADR-0009) are the single source of truth for copper, mask and
silkscreen; and every output is verified against an independent reference.

## Decision

### One crate for the 3D board

A new crate `ew-board3d` builds the 3D board and its exports. It depends on
`ew-output` (board art) and `ew-pcb` (geometry, polygon booleans); the
engine (`ew-core`) plugs it into the output jobs, so `ew-output` stays free
of 3D code. Modules: the heights and colours of the stack-up, triangle
meshes, the board meshes, the placement of models, the binary scene for the
3D view, GLB reading and writing, STEP and VRML.

### Heights: the laminate between the outer coppers

The laminate spans the dielectrics and the inner copper layers, from z = 0
at the bottom face; the outer copper lies on its faces, the mask on the
copper, the silkscreen on the mask. This is KiCad's STEP convention, so
boards exported by both tools line up. Parts stand on the outer copper
(their models' z = 0 is the copper surface). KiCad 10 lifts parts by a
further 0.05 mm of "pad thickness" for the solder; ElectronicWerk does not,
and the reference test accounts for it.

### Board meshes from the Gerber features

Every layer is built from `BoardArt::features`, the features of the Gerber
files, so the 3D view shows exactly what is manufactured. The body is the
outline minus the holes, capped and walled. The mask is the board minus its
openings and holes (one boolean operation: the mask is transparent, so
overlaps would show). Opaque layers (copper, silkscreen, paste) skip the
global union: every feature becomes its own polygon, and only features near
a hole, a mask opening or the board edge (found with an R-tree) are cut,
one by one. Polygons with many holes (the body of a board with thousands of
vias) are triangulated in tiles, because ear clipping slows down
quadratically with the number of holes. The layers are built in parallel.

With this, the synthetic L board (5,000 parts, 200,000 track segments,
356,000 silkscreen strokes) builds in about 0.6 s instead of 17 s with one
union per layer, the M board in about 60 ms. A budget test in CI checks
both.

### Placement: KiCad's convention, verified against KiCad

A model is placed like KiCad does: the footprint's position and rotation,
for parts on the bottom a half turn about the footprint's y axis, then the
model's offset, its rotation by the negated angles (z, y, x) and its scale.
The engine computes the matrices; the 3D view and the exports use them
unchanged.

KiCad stores bottom-side footprints mirrored in y, ElectronicWerk mirrors
them in x, which turns the imported local coordinates by a half turn. The
KiCad import now gives the 3D models of bottom footprints the same half
turn, so they sit on their pads.

Verification: `scripts/make-kicad-3d-reference.py` exports the blinky
fixture with `kicad-cli pcb export glb` and records every part's position
and rotation; a test compares ElectronicWerk's matrices with it (all eight
parts, including the bottom-side capacitor and parts turned by 90° and
270°).

### STEP written in Rust, not with native OpenCASCADE

The STEP export is written in Rust (ISO 10303-21, AP214), with the
structure OpenCASCADE writes: an assembly product for the board, the board
body as a product, one product per 3D model, placed per part by a
`NEXT_ASSEMBLY_USAGE_OCCURRENCE` with an `ITEM_DEFINED_TRANSFORMATION`.

- **The body is an exact B-rep**: the outline's lines and arcs and every
  drilled hole as a true cylinder (full circles split into two halves),
  with a top and a bottom face, one side face per segment and every edge
  shared by two faces. If holes overlap or cut the outline (found by a
  boolean operation: the ring count and the area must match), the body is
  written from the polygons of the 3D view instead.
- **Component STEP files are merged as they are**: a small Part 21 reader
  splits the records, renumbers the references (never inside strings),
  maps the file's application context to the assembly's, finds the root
  products (those that are no other product's part) and links them. A
  model file is written once, however many parts use it.
- **Mesh-only models** (the downloaded base library ships meshes, not the
  3 GB of STEP files) become faceted surface models, one per colour.

Native OpenCASCADE was not used: it is a large C++ build for five targets
(including cross-compiled Windows and Linux arm64), it is LGPL (static
linking into the MIT app would need relinking support), and it would add
tens of megabytes to every package. The STEP structure needed for an
assembly of existing models is small; OpenCASCADE stays in its role as
reader and tessellator (WebAssembly).

Verification: the STEP file of the blinky fixture (with synthetic STEP
boxes and a mesh from the library fixture) is a reference file; a vitest
reads it back with `occt-import-js` — the same OpenCASCADE kernel FreeCAD
and others use — and checks the body (40 × 30 × 1.5558 mm at the drill
origin), the parts and their sides. Locally, with the models of a KiCad 10
installation, the exported parts match `kicad-cli pcb export step`
triangle for triangle.

### GLB and VRML from meshes

GLB and VRML carry meshes: the board meshes with their materials and one
shared mesh group per model (glTF nodes, VRML `DEF`/`USE`). The meshes come
from the mesh cache. Tessellated STEP files are now cached by content hash
(`step-<sha256>.glb`), so the engine finds the meshes the desktop app made;
the command line cannot tessellate and reports such models.

### Model lookup

Footprints name models like KiCad. The engine looks for a file first:
`${KIPRJMOD}` and relative paths in the project folder, other variables in
the environment, KiCad's model variables also in the 3D folders of KiCad
installations; a `.wrl` path also finds a `.step` file of the same name.
Then the base library (by library and model name), which gives a mesh hash
and, for libraries built from local folders, the STEP file. The footprint
editor's 3D preview uses the same lookup (`board3d.resolve`).

### The 3D view: three.js, fed by the engine

`board3d.scene` returns the board meshes as one binary buffer next to the
JSON metadata (as `view.scene` does for the 2D view) and the parts with
their matrices and found models. The renderer draws them with three.js
(loaded on demand), z up, with a room environment for the metallic copper.
Models are loaded once per path and instanced per part.

**Live updates:** every change event of the board triggers
`board3d.parts` (placements only, about 7 ms for 5,000 parts) when parts
changed, and a full `board3d.scene` 200 ms after the edits pause. A moved
part is placed again well within the 250 ms of the requirement.

### The 3D window and the shared selection

The 3D window is a second `BrowserWindow` with the same page and a URL
fragment (`#view3d=<board>`); it gets its own engine port, so it receives
the engine's change events like the main window. It has no project state of
its own and closes when the project or the main window closes.

The selection travels as before through the `crossSelection` store, now
with a third source (`3d`) and across windows: each window sends its
changes to the main process, which forwards them to the other windows; a
new window asks the others for the current selection. The layout editor
publishes its selection always (the schematic still follows only with
cross-probing on).

## Consequences

- The board can be checked in 3D while it is edited, on a second monitor,
  and handed to mechanical CAD as an assembly with exact board geometry and
  the parts' original STEP solids.
- The STEP writer has no C++ dependency; packages do not grow. Features
  that need a geometry kernel (copper as solids, fused shapes, STEP import
  of enclosures) would need more of it, or native OpenCASCADE, later.
- Board meshes and Gerber files come from the same features: what the 3D
  view shows is what the Gerber files contain.
- The KiCad import changed for bottom-side models; projects imported before
  keep their (half-turned) models until they are imported again or the
  model's z rotation is corrected by 180°.
- Mesh exports depend on the mesh cache; a STEP model needs to be shown in
  the app once before the command line can export it as GLB or VRML.
