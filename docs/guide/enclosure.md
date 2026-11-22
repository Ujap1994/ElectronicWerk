# Enclosure and mechanical check

A board rarely lives on its own: it sits in an enclosure, behind a front
panel, on standoffs or on a carrier board. ElectronicWerk places the STEP
models of these **mechanical bodies** around the board in the
[3D view](./view-3d) and checks the board and every part against them:

- **Collisions** — a part or the board cuts into a body,
- **clearances** — a part comes closer to a body than the minimum
  distance of the [design rules](./design-rules),
- **mounting holes** — every boss of the enclosure has a hole over it,
  and the holes line up,
- **openings** — connectors that reach over the board edge have an
  opening in front of them,
- **height areas** — no part is taller than the room above (or below) it.

The findings appear in the enclosure panel of the 3D view, as markers in
3D and in the [design rules check](./drc) of the layout editor, and they
are updated while you move parts or bodies. The enclosure also helps the
layout: it gives the **board outline** and **keep-out and height areas**.
Finally the whole assembly — board, parts and enclosure — can be
[exported as STEP](#export-the-assembly).

![The blinky board in a box enclosure, with the enclosure panel](/screenshots/enclosure-box.png)

## Open the enclosure panel

Open the board's 3D view (**3D** in the layout editor's toolbar, or the
command palette's _Show the 3D view_) and click the **enclosure** button in
the 3D view's toolbar (the box with a lifted lid). The panel on the right
lists the board's bodies; it stays open when you switch tabs.

![The empty enclosure panel](/screenshots/enclosure-panel-empty.png)

The panel has four parts:

1. **Add STEP file…** and **Make box** — new bodies.
2. The **list of bodies**, each with an eye to show or hide it. Click a
   body to edit it below; click it again to close the editor. Clicking a
   solid body in the 3D view selects it here too.
3. **From the enclosure** — the [helpers](#helpers-from-the-enclosure)
   for the board outline and the keep-out and height areas.
4. **Check** — the results of the enclosure check.

## Add the enclosure's STEP files

Click **Add STEP file…** and choose a STEP file (`.step` or `.stp`) — an
enclosure shell, a lid, a front panel, a screw or a carrier board, as your
mechanical CAD program or the enclosure maker exports it. ElectronicWerk
**copies the file into the project's `mechanical/` folder**, so the project
stays complete when you move it, archive it or put it on GitHub. If a file
of that name with other contents is there already, the copy gets a number
(`Front panel-2.step`); the same file is used again.

Every body has:

| Setting                  | What it does                                                                                                                                                                      |
| ------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Name**                 | Shown in the list, the findings and the STEP export. The file name at first.                                                                                                      |
| **Kind**                 | Enclosure, lid, panel, screw, carrier board or other. Guessed from the name (`lid`, `cover`, `front`, `panel`, `screw`, `standoff` …); only used for names and for your overview. |
| **Position (mm)**        | Where the model's origin goes, in board coordinates. **z is measured from the bottom face of the board**: a body at z = −4 mm has its origin 4 mm below the board.                |
| **Rotation (°)**         | Turns about the x, y and z axes, in that order (as most CAD programs show it). 90° steps are exact.                                                                               |
| **Opacity**              | 5 to 100 %. See-through bodies let you look at the board inside, and clicks go through them to the parts.                                                                         |
| **Check for collisions** | Takes the body into the enclosure check (on by default). Turn it off for bodies that only show the surroundings.                                                                  |

The position fields take the new value when you press **Enter** or leave
the field; every change is one undo step (**Undo** in the project toolbar).
**Centre on the board** moves the body so that the middle of its box lies
over the middle of the board — a quick start for enclosures whose origin is
somewhere else.

::: tip Units and origins
STEP files carry their unit; ElectronicWerk reads millimetres, metres and
inches. Where a model's origin is depends on the program that made it —
often a corner or the middle of the bottom face. Use **Centre on the
board**, then move it in z until the board rests where it should (the
[section view](#look-inside) shows it best).
:::

**Remove** takes a body out of the board; its file stays in `mechanical/`
(so **Undo** can bring the body back).

## Make a simple box

No enclosure yet? **Make box** makes a simple box that fits the board and
adds it as two bodies, _Enclosure bottom_ and _Enclosure lid_:

- inside, the board's outline plus 0.5 mm on every side,
- walls, floor and lid 2 mm thick,
- the board rests 4 mm above the floor,
- a **boss** with a screw hole under every mounting hole of the board
  (unplated holes of at least 1.9 mm),
- the lid 2 mm above the tallest part, see-through (40 %).

The files are written to `mechanical/enclosure-bottom.step` and
`mechanical/enclosure-lid.step` — real STEP solids that any mechanical CAD
program opens, a useful start for your own enclosure design, and a quick way
to see how much room the board needs.

![The enclosure panel with a selected lid](/screenshots/enclosure-panel.png)

## Look inside

The 3D view's tools work on the enclosure too:

- **Opacity** of a lid or shell to see the board inside,
- the **eye** in the list to hide a body,
- the **section view** cuts the bodies with the board. The range of the
  section slider covers the enclosure, so you can cut right through a boss
  or a wall,
- **Measure** picks points on solid bodies as well (see-through bodies let
  the clicks through to the board).

![A section through the front mounting hole: the board rests on its boss](/screenshots/enclosure-section.png)

## The enclosure check

The check runs **by itself after every change** — a part moved in the
layout editor, a body moved in the panel, a new model — while **Check while
editing** is on (it is on by default). **Check now** runs it at once.

![The lid 5 mm too low: the pin header J1 collides with it](/screenshots/enclosure-collision.png)

It compares every **checked body** with:

- **the board** itself (its outline with cut-outs and the larger holes,
  through its whole thickness). The board may **touch** a body — it rests
  on bosses, screws clamp it — but it must not cut into one, and a gap that
  is there must not be smaller than the minimum clearance;
- **every populated part** (parts marked _do not populate_ are left out)
  with its 3D model. A part touching a body is a clearance of 0.

The results:

| Finding                | Severity | Meaning                                                                                                                      |
| ---------------------- | -------- | ---------------------------------------------------------------------------------------------------------------------------- |
| `mechanical-collision` | Error    | A part or the board **cuts into** a body, or lies inside it.                                                                 |
| `mechanical-clearance` | Warning  | Closer to a body than the **minimum clearance**, with the distance.                                                          |
| `mechanical-mounting`  | Warning  | A boss without a mounting hole, a hole without a boss, a hole beside its boss, or a hole smaller than the boss's screw hole. |
| `mechanical-opening`   | Warning  | A connector reaches over the board edge, but a body closes the way in front of it.                                           |
| `mechanical-height`    | Warning  | A part is taller than a height area allows.                                                                                  |

Each finding is a marker in 3D — red for errors, orange for warnings —
placed where the bodies meet. **Click a finding** in the list: the view
moves to the place and the part is selected (also in the layout editor).

![The view at a finding: J1 selected, the marker where it meets the lid](/screenshots/enclosure-finding.png)

### The minimum clearance

The minimum distance between the board or its parts and the enclosure is a
design rule: **Design rules → Minimums → Clearance to the enclosure**
(0.5 mm by default; the manufacturer presets keep it). Like every rule it
is stored in the project. The severities of the five checks can be changed
or turned off on the **Checks** tab, like any other check of the design
rules check.

![The minimum clearance to the enclosure in the design rules](/screenshots/enclosure-rule.png)

### Parts without a 3D model

A part without a model — or whose model has no mesh yet — is checked as a
**box**: its courtyard (else the box of its pads) with its height. The
height is the footprint property **Height** in millimetres (`1.2`,
`1.2 mm`, `1,2mm`), else 3 mm. The check names how many parts it checked
this way. Mounting holes (footprints with only unplated holes) and
fiducials have no body and are left out.

### In the layout editor

The same findings are part of the [design rules check](./drc) of the
board: they are listed with the other findings, marked on the board, and
can be **accepted** like any finding (they then stop counting). The online
check keeps them current while you move parts.

![Enclosure findings in the layout editor's design rules check](/screenshots/enclosure-drc.png)

### Mounting holes and bosses

The check cuts the bodies just below and just above the board. Small pieces
of these cuts (at most 15 mm across) are **bosses** — standoffs, screw
domes, pins. A boss under the board needs a **mounting hole** over it:

- a boss without a hole within its radius → _has no mounting hole_,
- a hole whose centre is more than 0.2 mm beside the boss's → _is … off
  the boss_,
- a hole smaller than the screw hole in the boss → _is smaller than the
  screw hole_,
- a mounting hole (round, at least 1.9 mm, not a via) without a boss →
  _has no boss_ (only when the enclosure has bosses at all).

![The bottom shell 0.3 mm to the side: the holes are beside their bosses](/screenshots/enclosure-mounting.png)

### Connectors and openings

Connectors usually reach over the board edge into an opening of the
enclosure: USB sockets, barrel jacks, terminal blocks. For every part that
reaches more than 0.3 mm over the board's box, the check casts rays from
its outer face outwards (a 3 × 3 grid over the face). If **all of them**
meet a body within 10 mm, there is no opening for the connector — the
plug could not get in. A connector that is in the opening but too tight is
a collision or clearance finding instead.

## Helpers from the enclosure

When the enclosure exists before the board — a bought enclosure, a design
from the mechanical team — it tells how the board must look. The two
buttons under **From the enclosure** read it:

### Board outline

**Board outline** cuts the checked bodies in the middle of the board's
thickness and takes the **room inside** that contains the board (else the
largest one), **less the minimum clearance**. It replaces the board's outer
outline with that shape; cut-outs inside the board stay. One undo step
brings the old outline back.

### Keep-out and height areas

**Keep-out and height areas** adds:

- **keep-out areas** (named _Enclosure keep-out_) where enclosure material
  crosses the board — ribs, posts, pins that need a hole or a free spot.
  Nothing may go there: no tracks, vias, pads, zones or parts;
- **height areas** (named _Height area 4.5 mm (top)_ and so on) for both
  sides: rays from the board's surface up to the lid and down to the floor,
  on a 1 mm grid, rounded down to 0.5 mm and **less the minimum
  clearance**. Places with the same free height become one area.

A height area is a keep-out area that forbids nothing but sets the **tallest
part** on its side (on the copper layer it is drawn on). You can draw and
change them yourself too: in the keep-out area's dialog, tick **Limit the
part height** and enter the height. The enclosure check reports parts that
are taller (`mechanical-height`). Running the helper again replaces the
areas it made before; areas you drew yourself stay.

![Height areas from the enclosure on the board (the hatched areas)](/screenshots/enclosure-areas.png)

## Export the assembly

The 3D view's **Export** menu has **With the enclosure (shown bodies)**
when the board has bodies: the STEP, GLB or VRML file then holds the board,
its parts and every shown body at its place. In STEP files the bodies are
the exact solids of their files, each a part of the assembly under its
name, ready for the mechanical CAD program. See
[3D models: STEP, GLB, VRML](./export-3d).

![The export menu with the enclosure option](/screenshots/enclosure-export.png)

## Command line and AI agents

Everything works without the app too:

```sh
ewerk mechanical add  MyBoard ~/Downloads/case.step --at 50,40,-4
ewerk mechanical list MyBoard
ewerk mechanical move MyBoard "case" --at 50,40,-3.5 --rotate 0,0,90
ewerk mechanical check MyBoard            # exit code 1 on collisions
ewerk mechanical enclosure MyBoard        # a box that fits
ewerk mechanical outline MyBoard --dry-run
ewerk mechanical areas MyBoard
ewerk export-3d MyBoard --mechanical -o assembly.step
```

See [`ewerk mechanical`](../reference/cli#ewerk-mechanical) for all
options, the [engine API](../reference/engine-api#enclosure) for the
`mechanical.*` methods, and [AI agents](./automation#the-enclosure) for
the MCP tools (`add_mechanical`, `edit_mechanical`, `check_enclosure`,
`make_enclosure`, `board_from_enclosure`).

::: info STEP files with curved faces
The check works on triangle meshes. The app makes them from any STEP file
(with OpenCASCADE, cached like the parts' models). The command line reads
STEP files whose faces are all flat itself — like the boxes ElectronicWerk
makes and the package models of the [part generator](./part-generator) —
but for curved enclosures it needs the meshes the app made: open the
board's 3D view once. Until then `ewerk mechanical check` names the files
and leaves those bodies out.
:::

## How it works

- Bodies are stored in the board file (`.ewpcb`) with their path, kind,
  placement, visibility, opacity and check switch; their STEP files are in
  `mechanical/`. See the [file format](../reference/file-format#mechanical-bodies).
- The check uses the same meshes as the 3D view. Triangles are sorted into
  a search tree per body; pairs of triangles are tested for crossing edges
  (an **overlap**, more than 1 µm deep) and otherwise measured. A body
  that lies wholly inside another is found by casting a ray from it.
  Touching faces count as touching, not as an overlap.
- Parts are checked in parallel; meshes and the bodies' trees are kept
  between checks, so a check after a small change takes a few
  milliseconds.
- The decisions behind the design are in
  [ADR-0018](https://github.com/Ujap1994/ElectronicWerk/blob/main/docs/adr/0018-enclosure-check.md).
