# ADR-0008: Layout editor, design rules and DRC

- **Status:** accepted
- **Date:** 2026-11-12
- **Milestone:** M7

## Context

M7 turns the board viewer into a layout editor: placing footprints,
interactive routing, vias, copper zones and keep-out areas, board setup
(stack-up, presets, outline, DXF import), design rules with net classes and
custom rules, a design rules check that runs online while editing, and the
synchronisation between schematic and board (forward and back annotation,
cross-probing).

The constraints of the earlier editors hold (ADR-0001, ADR-0002, ADR-0006,
ADR-0007): the engine owns the data, every change is a transaction with
undo, and the 2D view draws scenes built by the engine. Two further
requirements are specific to boards:

- **Exact geometry.** A DRC that reports 0.199 mm where there is 0.2 mm, or
  misses a short because of sampling, is worse than none. The same geometry
  must serve the DRC, the router and the zone filler, so that a fresh fill
  or a fresh route never shows up as a violation.
- **Speed.** The online DRC must keep up with editing (the roadmap asks for
  < 100 ms), and routing previews must follow the pointer.

## Decision

### One crate for board logic

The new crate `ew-pcb` holds everything about boards that is not storage:
geometry, connectivity, rules, DRC, zone filling, routing, edits, the
schematic diff, presets and the DXF reader. `ew-core` only parses
parameters, finds documents and commits change requests (`pcb.*` methods),
as `schematic.rs` and `libedit.rs` do for their crates.

### Copper as cores with a radius

Every piece of copper is a **core** — a point, a segment or a polygon (with
holes) — grown by a **radius**: a via is a disc, a track a stadium, an oval
pad a stadium, a rounded-rectangle pad a smaller rectangle with the corner
radius, a rectangle or custom pad a polygon with radius 0, a zone fill a
polygon. The distance between two shapes is the distance between their
cores minus both radii, which is exact for all of them. Polygons with many
edges carry an R-tree of their edges, so large zone fills are cheap to test
against. Arc tracks and curved outlines are split into chords with an
error below 1 µm.

All copper items of a board go into an R-tree (`rstar`); pairs are found by
envelope queries grown by the largest clearance. The pair searches of the
connectivity and the clearance check run in parallel (`rayon`).

### Rules: classes, minimums, custom rules with a query language

The rules document gains `custom` rules and `drc` severities (both optional,
so older files read unchanged). Resolution for a pair or an item:

1. The **last** enabled custom rule whose condition matches (for pairs in
   either order) and that sets the constraint wins.
2. Otherwise the net classes decide (the larger clearance of the two).
3. The board minimums are a floor for everything except the copper-to-edge
   distance of a custom rule (edge connectors need less).

Conditions use a small expression language over properties of `A` and `B`
(`type`, `net`, `netclass`, `layer`, `designator`, `footprint`, `pad`,
`side`, `width`, `diameter`, `drill`, `insideZone('name')`), with wildcards
for text and units for numbers. It is parsed once per check; a syntax error
carries its character position, skips the rule and becomes a `rule-error`
finding. The language is deliberately close to KiCad's custom rules, so
users can carry rules over with few changes, but it is our own grammar
(no S-expressions in the project file).

### DRC: full and by region, with a tolerance

`drc::check` runs every check; with a region it only checks items whose
boxes meet the region grown by the largest clearance, and `drc::merge`
replaces the local findings of the region in the previous report while the
board-wide checks (unrouted, outline, duplicates, parity) come fresh. The
engine keeps the last report per board for this. Findings have a stable
**key** (check, objects, layer); accepted keys are stored in the board
(`drcExclusions`).

Values within **500 nm** of a limit pass. The zone filler approximates
curves from outside and the polygon library snaps to an integer grid; the
tolerance keeps these effects out of the report without hiding real
violations (board makers work in tens of micrometres).

Differences to the schematic (missing or extra footprints, pad nets) come
from the schematic diff and are part of every report.

### Zone filling with a polygon library

Zones are filled with `i_overlay` (MIT/Apache, pure Rust): boolean
operations on many overlapping polygons and polygon offsets. The steps are
those of KiCad: clip to the board outline shrunk by the edge clearance,
subtract other-net copper grown by the clearance (and higher-priority fills,
non-plated holes, keep-outs), cut thermal gaps, remove necks below the
minimum width by an opening (shrink, grow), add spokes, drop islands
without a connection. Inputs are normalised (outlines counter-clockwise,
holes clockwise) and combined with the non-zero rule, so overlapping
obstacles unite. Curves are approximated from outside, so a fill is always
legal. Layers are filled in parallel; on one layer zones fill in priority
order. Fills keep their holes as holes (`FilledArea.holes`); imported KiCad
fills keep their slits.

### Routing in walkaround mode

The router answers one question per pointer move: a path from the start to
the cursor. It first tries the two two-segment octilinear paths; if neither
keeps the rules, an A\* search on a grid of half the track width (25–200
µm) around start and target, with octilinear steps and a bend penalty, finds
a way, which is then straightened into the longest octilinear segments that
keep the rules exactly. When the target is unreachable, the path ends at
the explored cell nearest to it. The search is limited to a window around
start and target and to 200,000 expansions, so a preview stays interactive.

Bends are limited by a **corner mode**: by default no bend is sharper than
45° (no right angles, for signal integrity and EMC), optionally 90°. The
limit applies to the two-segment candidates, to the A\* transitions (the
search state includes the direction a cell was entered from, and the
previous segment's direction seeds the start), to straightening (a merged
segment must bend within the limit towards the segment before and the grid
step after it) and to the final join at the target.

Shoving other tracks (push-and-shove) is left for later; walkaround covers
the common case and never changes existing copper.

The renderer asks for one preview at a time and always for the latest
cursor position; fixed segments stay local until the route ends and are
then committed together (one undo step).

### Schematic ↔ board: link ids, then designators

Parts are taken from the schematic connectivity: one part per designator
and sheet instance, with the net of every pin (single-pin nets without a
name count as unconnected). A part's **link id** is the component id of its
symbol (or the symbol id), and for repeated sheets an id derived from that
and the instance path. Footprints carry the link id in `component`.
Matching uses the link first and the designator second (for imported or
older boards), then proposes `link` changes.

Net renames are detected (all pads of an old net move to one new net that
the board does not have yet) and applied as a rename of the net object, so
tracks and zones keep their connection. New footprints are placed in rows
below the board outline.

### Editing and UI

Board edits (`move`, `rotate`, `flip`, `align`, `distribute`, `dragTrack`, …)
are one `BoardEdit` enum in `ew-pcb::edit` that returns the changed
objects; they reuse the `Edit`/`Change` types of `ew-schematic` so the
engine turns them into requests the same way. The renderer's `BoardEditor`
plugs into the viewer controller like the schematic editor. Selecting a pad
keeps the pad in the inspector while dragging moves its footprint; the
inspector edits footprints, tracks, vias and texts through `edit.apply`
patches.

## Consequences

- Undo, autosave and recovery work for every board edit without new
  mechanisms; every feature is scriptable through the engine API.
- The DRC, the router and the filler agree on geometry, so fills and routes
  are legal by construction; measured against KiCad 10 on the test project,
  the shorts, clearances and unconnected items agree.
- Performance (release builds, measured in CI): a full DRC of the synthetic
  L board (5,000 parts, 200,000 segments, 256,000 copper items) takes
  about 1.5 s; an online check of a region of the M board (500 parts,
  20,000 segments) about 20 ms. The online check of very large boards still
  rebuilds the copper index (about 50 ms on L) and the connectivity; caching
  them between edits is the next step if real boards of that size need it.
- Not yet covered: push-and-shove routing, differential pair routing,
  length tuning, solder mask bridge and hole-clearance checks, rooms. They
  fit into the same structure (new modules in `ew-pcb`, new checks in
  `drc`).
