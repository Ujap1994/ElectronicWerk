# Teardrops

A teardrop widens a track smoothly where it meets a pad or via. Without it,
the thin track meets the round or square copper of the pad at a sharp
corner. Teardrops help in three ways:

- The joint is stronger: heat cycles and a bent board crack a sharp joint
  sooner.
- A drill that hits a via or a through-hole pad a little off-centre does not
  cut the track off the pad.
- For fast signals the width changes gently instead of in one step.

ElectronicWerk makes teardrops as small copper zones of the track's net,
marked as teardrops: they start across the pad (at most as wide as the pad)
and narrow down to the track's width some way beyond the pad's edge, with
curved or straight sides.

![Teardrops at U1 and R3](/screenshots/teardrops-board.png)

## Adding and removing them

1. Click **Teardrops** in the layout editor's tool bar (or **Board: add or
   remove teardrops** in the [command palette](./command-palette)).
2. Click **Add teardrops**. Every end of a straight track that lies in a pad
   or via of its net gets a teardrop, on the track's layer; the teardrops
   added before are replaced. They are filled at once.
3. **Remove teardrops** removes all of them.

Both are one undo step each.

A teardrop is left out where it would come too close to copper of another
net (it is first made shorter and narrower, then left out), where the track
is about as wide as the pad, and at curved tracks.

## Their shape

In the [advanced working mode](./settings#working-mode) the dialog shows the
shape of the teardrops; it is kept in the design rules of the project and
used whenever teardrops are added:

![The teardrop settings in the advanced mode](/screenshots/teardrops-dialog.png)

| Setting                                | Default | Meaning                                                                   |
| -------------------------------------- | ------- | ------------------------------------------------------------------------- |
| **At pads**                            | on      | Teardrops where tracks end in pads.                                       |
| **At vias**                            | on      | Teardrops where tracks end in vias.                                       |
| **Curved sides**                       | on      | Curved (concave) sides; off: straight sides.                              |
| **Length (of the pad size)**           | 50 %    | How far beyond the pad's edge the teardrop reaches, in % of the pad size. |
| **Longest**                            | 1 mm    | The longest teardrop.                                                     |
| **Width at the pad (of the pad size)** | 100 %   | How wide the teardrop is at the pad, in % of the pad size.                |
| **Widest**                             | 2 mm    | The widest teardrop.                                                      |

The **pad size** is the width of the pad across the track.

## After changes

Teardrops are zones of the highest priority. They do not follow when you
move a track or footprint: add them again (which replaces them all), or
**Fill all zones** (Shift+B) to fill them again around the new copper. The
[DRC](./drc) checks them like all copper; an unfilled teardrop is reported
as an unfilled zone.

Teardrops go into the [Gerber files](./gerber-drill), the
[3D view](./view-3d) and the [KiCad export](./export-kicad) (as teardrop
zones, which KiCad 7 and later recognise). Teardrop zones of an imported
KiCad board are recognised as teardrops too.

## For scripts and AI agents

[`pcb.teardrops`](../reference/engine-api#pcb-teardrops) adds, replaces or
removes the teardrops of a board.
