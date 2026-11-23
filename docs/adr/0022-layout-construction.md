# ADR-0022: Layout construction

- **Status:** accepted
- **Date:** 2026-11-23
- **Milestone:** M21

## Context

M21 brings the construction aids of professional layout tools: dimensions
and a fabrication drawing, rooms, a creepage check for high voltages,
padstacks with via filling after IPC-4761, and design blocks that carry a
circuit with its layout from one project to another. Each needed a decision
about where its data lives and what the engine computes.

## Decision

### Dimensions are board objects that measure themselves

A dimension stores its kind, its two points, the distance of its line and a
text template (`{value}`), not a measured number. The value is computed
whenever it is drawn (`ew_model::dimension::draw`), in the project's units
or the dimension's own. So a dimension stays right when it is moved, turned
or flipped, and the screen, the prints, the Gerber files and the
fabrication drawing all draw the same lines from one function.

KiCad's sign convention for the dimension line (`height`) was measured with
`kicad-cli`, not taken from its documentation: positive moves a horizontal
or aligned dimension down and a vertical one to the right in KiCad's y-down
page. The import takes the side from the position of KiCad's text, which is
unambiguous.

### The fabrication drawing is generated, not drawn

The drawing is an output (`fabricationDrawing`) made from the board: the
outline, the holes with the drill map's symbols, every dimension, the
overall size, the drill table, the stack-up and numbered notes. Notes from
the board data come first (size, material, copper, finish, colours,
smallest features, tolerances, IPC class), the job's own notes after them.
Its texts are English, the language of board makers. A drawing that is
generated cannot go stale; a hand-drawn sheet (as in Altium's draftsman)
would need to be kept up to date by hand. Users who need more put
dimensions onto the board and notes into the job.

### Rooms are board objects; `inRoom` reuses the zone lookup

A room is a named polygon in the board (`rooms`). The rule language's
`A.inRoom('X')` goes through the same area lookup as `A.insideZone('X')`
with a `room:` prefix, so rules, the online DRC and the evaluation cache
needed no second mechanism. Moving a room carries the unlocked parts inside
it (they are added to the move); turning a room does not turn them, because
turning a group of parts about the room's centre is rarely what is wanted.
Parts are packed into a room in rows below its name; rooms for the sheets
of the schematic go below the board, where new parts from the board update
go too.

### Creepage: the user enters the distances, the engine measures the path

The distances come from the standard of the product (IEC 60664-1, IEC
62368-1, UL 840 …) and depend on voltage, pollution degree, material group
and kind of insulation. ElectronicWerk does not ship the tables: they are
copyrighted, and choosing the right line is the designer's decision. A
custom rule asks for a `creepage` distance; the DRC measures the shortest
path along the surface of the outer layers. The path is found on a
visibility graph of the corners of the obstacles near the two items (board
outline from the inside; cut-outs, slots and non-plated holes from the
outside), with a straight line as the shortcut when nothing is in the way.
The check runs only for pairs that a creepage rule covers and whose
straight distance is below it, so boards without high voltage pay nothing.

### Padstacks are optional per-layer shapes of a pad

A pad keeps its shape and size; `stack` adds entries for the inner layers
and for the back. `Pad::on_copper(layer, bottom)` gives the pad as it is on
one board layer, and every consumer (copper model for DRC, filler and
router, scene, outputs) asks for that. A pad without a stack costs nothing.
Only inner and back are offered because KiCad 9's `front_inner_back` mode
covers the needs that come up in practice; a shape per inner layer
(KiCad's custom mode) is imported with its first inner shape.

### Via filling follows IPC-4761 by type

A via has an optional `fill`: the seven IPC-4761 types from tented (I) to
filled and capped (VII). None means "as the output settings say", so old
boards keep their output. The type decides the mask opening (covered types
get none), groups the drill table and adds notes to the fabrication drawing.
A DRC check (`via-in-pad`) warns about vias in SMD pads that are not type
VII. KiCad 9's separate front/back flags for tenting, covering and plugging
are mapped to the types; ElectronicWerk treats both sides alike.

### Design blocks are files outside the project

A block is a JSON file in a folder beside the base library, not part of a
project, so every project on the computer sees it and blocks can be shared
by copying files or keeping the folder in Git. It holds the schematic
objects, the library items they use, and the footprints, tracks and vias of
their layout.

Placing a block does not invent nets. It pastes the schematic (designators
numbered on, units of one part kept together), then runs the normal board
update restricted to the new parts on a staged copy of the project, so the
footprints get exactly the nets the schematic gives them. It then moves
them to the block's positions and copies the tracks and vias, mapping each
block net to the new net through the pads (designator and pad number). All
of it is one transaction and one undo step, and the board part becomes a
group so it can be moved onto the board as one piece.

## Consequences

- KiCad exchange covers dimensions, padstacks and via protection; boards
  that use padstacks or via fills need KiCad 9 or newer.
- The rule language has one more function, `inRoom`, and one more
  constraint, `creepage`; the DRC has `creepage` and `via-in-pad`.
- Design blocks depend on the schematic's connectivity for their nets: a
  local label of a block joins a label of the same name on the target sheet.
  The guide says so; a later version may offer to rename them on placing.
- The default fabrication job has ten outputs (the fabrication drawing after
  the drill map).
