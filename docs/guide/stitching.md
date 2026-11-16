# Stitching vias and via fences

Stitching vias are vias of one net that no track ends at. They come in two
uses:

| Use                | What it does                                                                                                                                                                                               |
| ------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Zone stitching** | A grid of vias in a copper zone (usually ground) connects it to the copper of the same net on the other layers with many short paths: a low-impedance ground, less noise, and heat spread over the layers. |
| **Via fence**      | A row of ground vias along both sides of a track shields it: RF lines, antenna feeds and fast clocks radiate and pick up less.                                                                             |

Click **Stitching vias** in the layout editor's tool bar (or **Board:
stitching vias and via fences** in the [command palette](./command-palette)).

![The stitching dialog with a via fence along the selected tracks](/screenshots/stitch-dialog.png)

## Stitching a zone

1. Fill the zones first (**Fill all zones**, Shift+B): vias go only into
   filled copper.
2. Select the zone on the board, or choose it in the dialog under **Zone**.
3. Click **Add vias**.

ElectronicWerk places a grid of vias over the zone, centred in it. A via
goes only where:

- it lies completely in the zone's copper,
- another layer has copper of the same net at that point too (another zone
  of the net), so it actually connects something,
- it keeps the clearance to copper of other nets on every layer, the hole
  distance to every other hole, and the distance to the board edge,
- it stays off the pads, tracks and vias of its own net, and out of
  keep-out areas that forbid vias.

The zones are filled again afterwards (other nets' zones now keep their
clearance to the new vias).

## A via fence along tracks

1. Select the tracks to shield on the board (Shift+click adds more).
2. Open the dialog: it shows **Along tracks** with the number of selected
   tracks. Choose the **Net of the vias** (ground is preselected when the
   board has a net called `GND`).
3. Click **Add vias**.

The vias go along both sides of every straight selected track, at the gap
of the net's class between the track's edge and the via's edge. Where a
fence of one track meets the fence of the next (at bends), vias closer
than three quarters of the pitch are left out. The same rules as for zone stitching apply,
so near pads and other tracks some vias are left out.

The fence vias should connect to a zone of their net (a ground zone on an
inner layer or below the track); otherwise the [DRC](./drc) reports them as
unconnected vias.

![A ground via fence along the /OUT tracks](/screenshots/stitch-fence.png)

## Settings

| Setting                         | Default                         | Mode     |
| ------------------------------- | ------------------------------- | -------- |
| **Pitch**                       | 3 mm (zone), 2 mm (fence)       | Advanced |
| **Staggered rows** (zone)       | on                              | Advanced |
| **Gap** (fence)                 | the clearance of the net        | Advanced |
| **Via diameter**, **Via drill** | the via size of the net's class | Advanced |

In the [beginner working mode](./settings#working-mode) the defaults apply.
For a fence, keep the pitch below a twentieth of the wavelength of the
highest frequency on the track (on FR4 about 3 mm at 2.4 GHz).

## Removing them

**Remove vias** removes the vias of the net that no track ends at: inside
the zone, or along the selected tracks. Vias you placed by hand without a
track are free vias too and are removed with them. Both adding and removing
are one undo step.

## For scripts and AI agents

[`pcb.stitch`](../reference/engine-api#pcb-stitch) stitches a zone, fences
tracks, or removes stitching vias.
