# ADR-0018: Enclosure and mechanical check

- **Status:** accepted
- **Date:** 2026-11-22
- **Milestone:** M17

## Context

Boards go into enclosures, behind front panels, onto standoffs and carrier
boards. Until now a collision showed up only in the mechanical CAD program,
after the STEP export, or when the first boards came back. The roadmap asked
for the enclosure and other mechanical parts as STEP files in a 3D assembly
of the board, a collision and clearance check with a minimum distance (as a
design rule, listed like DRC findings and kept current while parts move),
helpers that read the board outline, keep-out and height areas, mounting
holes and connector openings from the enclosure, and the whole assembly as
STEP — in the app, the command line and the engine API.

## Decision

### Bodies are board objects

A mechanical body is a new kind of board object (`MechanicalBody`, `type:
"mechanical"`) in the board file: name, path of the STEP file, kind,
position (nm, board coordinates with z from the board's bottom face),
rotation (degrees about x, y, z), hidden, opacity, check switch. As a board
object it gets undo, redo, the change events of the live views and the
file format's migrations for free.

- **Files are copied into the project** (`mechanical/`), so a project stays
  complete when it is moved, archived or pushed. A file of the same name
  with other contents gets a number; the same file is used again.
- **Placement is a transform of the whole file.** Enclosure STEP files are
  assemblies of several solids; splitting them into parts was not needed for
  the check and would tie the board to the file's inner structure.
- The minimum clearance is a new board minimum of the design rules
  (`minMechanicalClearance`, 0.5 mm), the findings are design rules check
  codes (`mechanical-collision`, `-clearance`, `-mounting`, `-opening`,
  `-height`) with the usual severities, exclusions and online updates.

### The check works on triangle meshes

The geometry is the meshes the 3D view already shows: the board body from
the board model (only the larger holes — vias do not matter here), part
models from the mesh cache, bodies from their STEP files.

- **Why not exact solids**: an exact boolean of B-reps needs a geometry
  kernel (OpenCASCADE) in the engine — native code for five targets and
  LGPL linking (see ADR-0010). Meshes are exact enough: the models'
  tessellation error is below a few hundredths of a millimetre, far below
  any useful clearance.
- **Overlaps against touching**: a board resting on bosses touches them, a
  connector pressed against a panel touches it. The check therefore tells
  _overlap_ (an edge of one body passes through a face of the other by more
  than 1 µm on both sides, or one body lies inside the other — a ray-parity
  test) from _distance_ (the closest points of two triangles, with touching
  as 0). The board may touch a body; gaps smaller than the minimum are
  reported. Parts touching a body are a clearance of 0.
- **Speed**: each body's triangles go into an R-tree (`rstar`, already
  used); a part is compared only with the triangles near its box, the parts
  in parallel (`rayon`). Meshes and the bodies' trees are kept between
  checks, so the online check after a small edit costs milliseconds.
- **Parts without a model** are boxes of their courtyard (else the pads'
  box) and the footprint's `Height` property (else 3 mm). Mounting holes and
  fiducials have no body.
- **Meshes in the engine**: the desktop app tessellates STEP files with
  OpenCASCADE (occt-import-js, ADR-0010) into the mesh cache. The engine
  reads the cache, and reads STEP files whose faces are all planes itself
  (`step::tessellate`: planar faces bounded by lines, triangulated with
  earcut) — the models ElectronicWerk writes, so the command line and CI can
  check generated enclosures and package models without the app. Curved
  files without a mesh are named in the reply (`tessellate`); the app's
  engine host makes their meshes and runs the check once more.

### Helpers read cross-sections and rays

- **Cross-sections** of the bodies with a horizontal plane give closed
  loops (outlines counter-clockwise, holes clockwise, from the triangles'
  orientation), united with the polygon library of the layout editor.
- **Board outline**: the hole of the section at the board's mid-plane that
  contains the board (the room inside), shrunk by the minimum clearance,
  replaces the outer outline; cut-outs stay.
- **Keep-out areas**: section material inside the board area.
- **Height areas**: rays up from the top face and down from the bottom face
  on a grid (1 mm), free heights rounded down (0.5 mm) less the clearance,
  equal cells united. A height area is a keep-out zone with a new
  `maxHeight`; a new check compares the parts' model heights with it. Zones
  were chosen over a new object kind, so the layout editor draws, edits and
  exports them already.
- **Mounting holes**: small pieces of the sections just below and above the
  board (at most 15 mm across) are bosses; they are matched with the
  board's round holes of at least 1.9 mm (offset more than 0.2 mm, hole
  smaller than the boss's screw hole, boss or hole alone).
- **Openings**: parts reaching over the board's box cast a 3 × 3 grid of
  rays outwards from their face; all of them hitting a body within 10 mm
  means there is no opening. A connector inside an opening that is too tight
  is already a collision or clearance finding.

### A box enclosure

`mechanical.enclosure` makes a simple box (bottom shell with walls, floor,
bosses under the mounting holes and an optional front opening; a flat lid)
as exact STEP solids written by the prism writer of M16, now with holes in
prisms. It is a quick fit check for users without an enclosure, a start for
their own design, and the synthetic enclosure of the tests and the
documentation (no real enclosure is in the repository).

### Export

`board3d.export` with `mechanical: true` adds the shown bodies as parts of
the assembly: their STEP files merged as exact solids like part models,
their meshes in GLB and VRML.

## Consequences

- New modules in `ew-board3d`: `collide` (meshes, R-tree, triangle tests),
  `mechanical` (placements, part shapes, check), `section`, `assembly`
  (helpers), `enclosure` (box), `step::tessellate`; prisms with holes; a
  body-only board build.
- New engine methods `mechanical.list`, `.add`, `.update`, `.remove`,
  `.check`, `.enclosure`, `.section`, `.outline`, `.areas`; `pcb.drc`
  includes the findings; `board3d.export` takes `mechanical`.
- `ewerk mechanical …`, `ewerk export-3d --mechanical`, MCP tools
  `list_mechanical`, `add_mechanical`, `edit_mechanical`, `check_enclosure`,
  `make_enclosure`, `board_from_enclosure`.
- The 3D view has an enclosure panel; the keep-out dialog a height limit;
  the rules dialog the new minimum.
- Limits: curved enclosures need the app once for their meshes; the check
  is as exact as the meshes; bodies are rigid (no flexible cables or hinged
  lids in other positions — add the lid twice if both positions matter).
