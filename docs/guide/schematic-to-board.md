# Schematic and board

The schematic and the board describe the same circuit. ElectronicWerk keeps
them together in three ways:

- **Update the board** (forward annotation) brings new parts, changed
  footprints and values and the nets of the schematic onto the board.
- **Update the schematic** (back annotation) takes designators, values and
  footprints changed on the board back into the schematic.
- **Cross-probing** selects a part on one side when you select it on the
  other.

The [design rules check](./drc) reports the differences that are left
(`missing-footprint`, `extra-footprint`, `net-mismatch`).

## Updating the board

Click **Update from the schematic** in the board toolbar (or **Board:
update from the schematic** in the command palette). The dialog compares
schematic and board and lists every change it would make:

![The changes of an update](/screenshots/layout-sync.png)

| Change           | When                                                                                                             |
| ---------------- | ---------------------------------------------------------------------------------------------------------------- |
| Add net          | A net of the schematic has no board net yet.                                                                     |
| Rename net       | All pads of a board net moved to one new net name, for example after renaming a label. Tracks and zones keep it. |
| Remove net       | No pad uses the net any more. Its tracks, vias and zones keep their copper but lose the net.                     |
| Add footprint    | A part has no footprint on the board. It is placed in rows below the board outline.                              |
| Remove footprint | A footprint linked to the schematic has no part there any more.                                                  |
| Change footprint | The part now uses another footprint. The new one takes the place, rotation and side of the old one.              |
| Rename           | The designator of the part changed (after re-annotation).                                                        |
| Change value     | The value of the part changed.                                                                                   |
| Link             | A footprint matched its part by designator only; it is linked to the part from now on.                           |
| Connect pad      | A pad is on another net than its pin.                                                                            |

Untick changes you do not want, then **Apply**. The update is one undo step
(`update-board`), and the new footprints are selected so you can move them
straight away.

### How parts and footprints are matched

Every part (a symbol with all its units, once per use of its sheet) has a
link id. A footprint placed by an update carries that id, so it stays
linked when you rename either side. Footprints without a link (from an
older import, for example) are matched by designator and then linked.
Footprints that were never linked to the schematic — mounting holes, logos
you placed on the board — are left alone.

The footprint of a part comes from the **Footprint** field of the symbol,
or from the default footprint of its symbol in the library. Pins and pads
are joined by number: pad `3` gets the net of pin `3`. Several pads with
the same number (an exposed pad split into parts) all get that net; pads
without a pin (mounting tabs) get no net.

A pin connected to nothing gets no net, so its pad stays free.

### What cannot be updated

The dialog lists parts it cannot place, with the reason:

| Problem                               | Fix                                                                   |
| ------------------------------------- | --------------------------------------------------------------------- |
| A part is not annotated (`R?`)        | [Annotate](./schematic-editor#annotation) the schematic.              |
| A part has no footprint               | Set the footprint field, or a default footprint in the symbol.        |
| The footprint is not in the libraries | Create it in the [footprint editor](./footprint-editor) or import it. |
| A designator is used twice            | Annotate again, or rename one of them.                                |
| A pin has no pad                      | The footprint has fewer pads than the symbol has pins: check both.    |

## Back annotation

Changes made on the board can go back to the schematic: switch the dialog
to **Update the schematic**. It lists designators, values and footprints
that differ on linked footprints. Applying them changes every unit of the
part on its sheet (and the designator of the right sheet instance for sheets
used more than once). This is one undo step (`update-schematic`).

A typical use: renumber the designators on the board in placement order,
then take them back to the schematic.

## Cross-probing

With cross-probing on (the schematic button in the board toolbar), selecting
symbols on a sheet selects their footprints on the board, and selecting
footprints on the board selects their symbols on the sheets. Switch tabs to
see the other side; the selection is already there.
