# ADR-0016: Advanced layout — vias, push and shove, impedance, pairs, lengths, teardrops, stitching, channels, variants

- **Status:** accepted
- **Date:** 2026-11-16
- **Milestone:** M15

## Context

M15 brings the layout editor to the level of boards with fast signals and
dense parts: blind, buried and microvias; an interactive router that pushes
other tracks aside and drags tracks with their 45° neighbours; controlled
impedance with differential pairs; length matching; teardrops; via
stitching and shielding; repeated blocks (channels) and assembly variants.
The maintainer asked for an **impedance calculator** with two **working
modes**: a beginner mode that computes and applies the values, and an
advanced mode where everything is set by hand.

The guiding rules from earlier milestones stay: everything is one undo
step, the engine does the work and the app shows it, every feature is
reachable from scripts, routes bend by 45° unless the user allows 90°, and
the design rules check knows every new rule.

## Decision

### Working mode

- A **general app setting** (`workMode`: `beginner` by default, or
  `advanced`), not a per-dialog switch, so later milestones can use it
  too. Both modes edit the same data: switching never loses anything.
- Beginner screens hide settings and fill in values (impedance sizes, the
  usual skew of an interface, teardrop and stitching shapes); advanced
  screens show them.

### Blind, buried and microvias

- The via model already had `from`/`to`/`micro`; the **type follows from
  the layers** (`Via::via_type`), so files stay unchanged.
- Off until the design rules allow them (`allowBlindBuriedVias`,
  `allowMicroVias`), because many board makers do not make them. Microvias
  connect neighbouring layers only and have their own minimums.
- 3D, STEP, drill files, DSN and the KiCad export carry the spans.

### Push and shove

- Its own module (`shove.rs`) on top of the walkaround router: a track of
  another net is a **line** between joints; it is walked around the
  octagonal clearance hull of the new copper, in both directions, the
  shorter first. 90° bends that appear are chamfered into two 45° bends.
- Only the **new** segments are checked (boards often break rules already),
  pushes cascade (at most 64), and a final pairwise check rejects anything
  that still collides. Pads, vias, locked and curved tracks never move.
- The engine returns a **preview** (`tracks` and `hidden`), so the app can
  draw the pushed tracks while the user routes; the final call applies the
  same result.

### Impedance

- **Closed formulas**, as common board calculators use them:
  Hammerstad–Jensen for microstrip, Wheeler for stripline (an off-centre
  line as two centred ones in parallel), IPC-2141 coupling for edge-coupled
  pairs. Accurate to a few per cent, which is what a board maker's field
  solver then refines. A field solver in the app was not justified.
- **Profiles** (target, tolerance, single or pair, sizes set by hand per
  layer) live in the design rules; a **net class follows a profile**. The
  router takes the profile's size **on the layer it routes on**, so a
  route changes width with the layer.
- Integers in the model (Ω, %), because the rules derive `Eq`.

### Differential pairs

- Found **by name** (`P`/`+`/`H` against `N`/`M`/`-`/`L` at the end, exactly
  one of each), as most tools do; no extra pair object to keep in sync.
- Routed as a **centre line**: the walkaround router finds a path for one
  track as wide as both tracks and their gap (the partner's copper counting
  as own), and both tracks are that path moved sideways with mitred bends.
  Straight launch and landing pieces let both tracks join and leave at 45°.
  This reuses the tested router instead of a second one.
- A pair stays on its layer in this version; layer changes are made per net.

### Length matching

- **Rules:** a largest skew per net class (for its pairs) and length groups
  (net name patterns with a tolerance). The beginner mode sets the usual
  skew of the chosen interface.
- **Meanders** with 45° corners and an exact extra length (the bump height
  is solved, not stepped); placed on the side with room, on the longest
  straight tracks first, nets one after another so later meanders keep
  their distance to earlier ones.
- Lengths are the tracks' lengths; vias are not counted (documented).

### Teardrops

- **Zones** marked `teardrop`, as KiCad 7+ does: they are filled, checked,
  exported (Gerber, 3D, KiCad with the teardrop attribute) like any zone
  without new code in every output. The generator replaces all of them at
  once; settings live in the design rules.

### Stitching vias

- **Free vias** of a net (no track ends at them) — no new flag in the via
  model, the same as other tools. Removing them removes the free vias of
  the net in the zone or along the tracks.

### Channels

- Computed **from the schematic**: a sheet used more than once gives one
  channel per use; parts correspond by their symbol on the shared sheet,
  nets by the pads they reach. Copying a channel places the parts and
  copies the routing between them with each channel's nets.
- **No persistent rooms.** A room object would need its own editing,
  rendering, export and sync; the channels are fully known from the
  schematic, which keeps the board file unchanged.

### Assembly variants

- Stored in the **project manifest** (`variants`), edited with
  `project.setProperties` (undo included). A variant lists only the parts
  that differ: fitted, not fitted, another value or fields.
- Applied **when outputs are made**: a job made for a variant runs on a copy
  of the project with the variant applied (boards, symbols of sheets used
  once, bill of materials; `${VARIANT}` holds its name). The design itself
  never changes.

### Back drilling: not now

Back drilling removes the unused stub of a through via from the far side.
It matters for signals above roughly 5–10 Gbit/s on thick boards, needs a
board maker that offers it (none of the low-cost prototype services do),
and is specified per drill in fabrication notes. Blind and buried vias,
now available, avoid the stubs where needed. It stays a candidate after
v1.0 if users ask for it.

## Consequences

- New engine methods: `pcb.viaFits`, `pcb.editPreview`, `pcb.impedance`,
  `pcb.impedanceCalc`, `pcb.routePair`, `pcb.pairs`, `pcb.lengths`,
  `pcb.tuneLength`, `pcb.matchLengths`, `pcb.teardrops`, `pcb.stitch`,
  `pcb.channels`, `pcb.copyChannel`; extended `pcb.route`, `pcb.addTracks`,
  `pcb.routeStart`, `outputs.bom`, `project.setProperties`.
- New DRC checks: `via-type`, `via-layers`, `impedance`, `pair-gap`,
  `pair-skew`, `length-match` (the last four are warnings).
- File format additions are all optional and skipped when empty, so files
  of earlier versions read unchanged and new files stay readable for
  tools that ignore unknown fields.
- Board-wide DRC checks must stay linear: the length checks first ran a
  scan of all tracks per net, which broke the performance budget of a
  large board; they now sum lengths in one pass and are skipped when no
  length rule exists.
