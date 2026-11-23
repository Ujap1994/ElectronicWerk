# Selecting and editing many objects

A board has hundreds of tracks, vias and parts, and a change often concerns
many of them at once: all 0.2 mm tracks of a bus should become 0.25 mm, all
vias of the power nets should get a larger drill, two resistors and their
capacitor should stay together while you place them. This page describes
the tools for that:

- the **selection filter** decides which kinds of objects a click, a box or
  a lasso picks;
- **Select similar** selects every object that shares chosen properties
  with the one you clicked;
- the **properties panel** edits many objects of one kind together;
- **groups** keep objects together, so that one click selects all of them.

The selection filter and **Select similar** work in schematics and on
boards; editing many objects at once and groups are part of the
[layout editor](./layout-editor).

## The selection filter

The **Selection filter** section on the right, below the layers, lists the
kinds of objects of the document. Clear a box and that kind can no longer
be selected: a click goes through it to what lies below, a box or lasso
leaves it out, and **Select all** (<kbd>Ctrl</kbd>/<kbd>⌘</kbd>+<kbd>A</kbd>)
skips it.

![The selection filter with tracks and vias switched off](/screenshots/selection-filter.png)

| Document  | Kinds                                                                                                                             |
| --------- | --------------------------------------------------------------------------------------------------------------------------------- |
| Board     | Parts (with their pads and texts), tracks, vias, zones, texts, graphics                                                           |
| Schematic | Symbols (with their pins and fields), wires (with buses and junctions), labels (with power ports), sheets, flags, texts, graphics |

Typical uses:

- **Only parts** while placing: a box around an area takes the footprints,
  not the tracks and zones below them.
- **No zones** while routing on a board with ground planes: a click on a
  track never picks the plane underneath.
- **Only tracks and vias** to delete the routing of an area and keep its
  parts.

The section header shows how many kinds are switched off, so a forgotten
filter does not surprise you. **All** switches every kind on again. The
filter is kept for the session, separately for boards and for schematics;
it is not saved with the project.

## Select similar

Select one object — on a board a track, via, footprint, zone or text, on a
sheet a symbol, wire, label or text — and click **Select similar …** in the
properties panel. A small list shows the
properties of the object that can be compared, with their values:

![Select similar for a track: same layer and width](/screenshots/select-similar.png)

Tick the properties that must be equal and click **Select**. Every object of
the same kind in the document that has the same values is selected. The
list starts with the usual choices ticked:

| Object     | Ticked at first  |
| ---------- | ---------------- |
| Track      | Layer, width     |
| Via        | Diameter, drill  |
| Footprint  | Footprint, value |
| Symbol     | Symbol, value    |
| Zone, text | Layer            |

Every other simple property of the object (its net, side, rotation, locked
state …) can be ticked too; positions and names that identify one object
(designator, position, start and end) are not offered. Lengths show in the
units of the project; the net shows its name.

## Editing many objects at once

When the selection holds several objects of one kind — after
**Select similar**, a box selection with the [selection filter](#the-selection-filter)
or <kbd>Shift</kbd>-clicks — the properties panel shows the fields of that
kind for all of them:

![The properties of 12 tracks: one width for all](/screenshots/multi-edit.png)

- A field whose value is the same for all objects shows that value.
- A field marked **· differs** has different values; it shows the value of
  the first object.
- A change goes to **every** selected object and is **one** undo step
  (**Change properties**).

![Fields that differ are marked](/screenshots/multi-edit-panel.png)

The designator is left out for several parts — each part keeps its own. The
panel edits up to 2,000 objects at once; for more, narrow the selection
first. A selection of different kinds (tracks and vias, say) lists how many
there are of each kind; select similar objects or use the selection filter
to edit one kind.

## Groups

A group keeps objects together. Clicking any member selects the whole
group, so the members move, turn, flip and lock together, and a box that
touches one member takes all of them.

Select the objects — parts, tracks, vias, zones, texts and graphics can be
mixed — and click **Group** in the properties panel or press
<kbd>Ctrl</kbd>/<kbd>⌘</kbd>+<kbd>G</kbd>:

![Two resistors selected: Group](/screenshots/group-actions.png)

![A click on R1 selects the whole group](/screenshots/group-selected.png)

| Action          | Key                                                        | Properties panel |
| --------------- | ---------------------------------------------------------- | ---------------- |
| Group           | <kbd>Ctrl</kbd>/<kbd>⌘</kbd>+<kbd>G</kbd>                  | **Group**        |
| Dissolve groups | <kbd>Ctrl</kbd>/<kbd>⌘</kbd>+<kbd>Shift</kbd>+<kbd>G</kbd> | **Ungroup**      |

Rules:

- An object belongs to at most one group. Grouping objects that are already
  in a group takes them out of it; a group that keeps only one member is
  dissolved.
- Deleting members removes them from their group.
- Groups are saved with the board (see the
  [file format](../reference/file-format#boards-ewpcb)) and every change to them
  is one undo step (**Group**, **Ungroup**).
- [Logos and QR codes](./logos-and-qr-codes) are grouped when they are
  added, so the many polygons of a picture move as one.

To change one member of a group, ungroup it, change it and group again —
or edit the whole group in the properties panel, which changes every
member of the kind shown.
