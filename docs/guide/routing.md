# Routing tracks

The routing tool (**X**) draws tracks from pad to pad. It works in
**walkaround** mode: the route follows the cursor with segments at 0°, 45°
and 90° and goes around copper of other nets, holes, keep-out areas and the
board edge, always keeping the clearance the [design rules](./design-rules)
ask for. Tracks get the width of their net's class, vias the sizes of that
class.

![Routing from a pad: the route goes around the pads of other nets](/screenshots/layout-routing.png)

## Routing a connection

1. Press **X** (or pick **Route tracks** in the tool column) and choose the
   copper layer in the toolbar (**Page Up** / **Page Down**).
2. **Click a pad, track end or via.** The route starts at the centre of the
   pad or via, or at the nearer end of the track, and takes its net. The
   status line shows `Routing <net> · <width> mm`, and the net is
   highlighted.
3. **Move the cursor.** The live path follows it. It is drawn faint when the
   cursor cannot be reached; the route then ends at the reachable point
   nearest to the cursor.
4. **Click** to fix the path so far. The route continues from its end.
5. **Click the target pad** (or a track or via of the same net) to finish,
   or **double-click** or press **Enter** anywhere.

The cursor snaps to pads and vias of the routed net: hovering over the
target pad routes to its centre exactly, whatever the grid.

| Key or action      | While routing                                                      |
| ------------------ | ------------------------------------------------------------------ |
| Click              | Fix the path to the cursor; on copper of the net: finish           |
| Double-click/Enter | Finish (fixed segments go into the board)                          |
| V                  | Place a via at the end and continue on the opposite outer layer    |
| Page Up/Page Down  | Place a via and continue on the previous/next copper layer         |
| /                  | Start with the diagonal instead of the straight segment (and back) |
| Backspace          | Take back the last fixed segment (and its via)                     |
| Esc                | End: fixed segments go into the board, the live path is dropped    |

The whole route is **one undo step** (`route`), with all its tracks and vias.

## 45° and 90° corners

By default, routes **only bend by 45°**: segments run at 0°, 45° or 90°,
but two segments never meet at a right angle (or a sharper one). Right-angled
corners are a classic signal-integrity and EMC concern (they make the track
wider at the corner and are etched less evenly), so the safe choice needs no
setting. This also holds where a route continues: at a fixed segment, and at
the end of an existing track the route starts from. To turn by 90°, the
route makes two 45° bends.

If you do want right angles (for example on a simple one-sided board, or to
match an existing layout), click **Allow 90° corners** in the board toolbar
(the right-angle icon). The direct path is then an L (across, then up or
down; **/** swaps it), and the router may bend by up to 90°. Sharper bends
are never made. The setting is remembered on this computer; the status line
shows the current mode while routing (`45° bends` or `90° bends`).

## How the route finds its way

For every cursor position the router first tries the simplest path: one
straight and one diagonal segment (or the other way round; in 90° mode an L
first). If that path keeps every rule and every bend is allowed, it is used
— this is what you see in open areas. If not, the router searches a grid
around the obstacles (an A\* search with octilinear steps that only takes
allowed bends and avoids needless ones) and then straightens the result
into as few segments as possible, again without sharper bends. The search covers the area between
start and cursor plus a margin of at least 5 mm, so it stays fast: a preview
on a typical board takes a few milliseconds.

What counts as an obstacle:

| Obstacle                                                       | Distance kept                                                                              |
| -------------------------------------------------------------- | ------------------------------------------------------------------------------------------ |
| Pads, tracks and vias of other nets (and copper without a net) | The clearance between the two net classes, or a [custom rule](./design-rules#custom-rules) |
| Non-plated holes                                               | The largest clearance of the rules                                                         |
| Keep-out areas that forbid tracks                              | None: the track must stay outside                                                          |
| The board edge                                                 | The copper-to-edge minimum (or a custom rule)                                              |

Copper of the routed net is never an obstacle, so routes may run over pads
and tracks of their own net. Zone fills are no obstacle either: routes and
vias may cross a filled zone of another net, and the zone keeps its
clearance again when it is [filled](./zones#filling) the next time.

## Push and shove

The **Router** in the toolbar chooses what a new track does with the tracks
of other nets in its way:

| Router                | What happens                                                                                                  |
| --------------------- | ------------------------------------------------------------------------------------------------------------- |
| Walk around (default) | The new track goes around them, as described above. Nothing else on the board changes.                        |
| Push and shove        | The new track goes the direct way and **pushes them aside**; the tracks it pushes push the next ones in turn. |

![A new track pushes the /OUT track aside](/screenshots/shove-routing.png)

While you route, the pushed tracks are drawn in their new place. They give
way along the outline the new track keeps free around itself: at the
clearance of the two net classes, with bends of 45° only (right angles
where a pushed track meets that outline are cut to two 45° bends). When you
finish the route, the route and the pushed tracks go into the board in one
step: **Undo** takes both back.

![The pushed track after the route](/screenshots/shove-result.png)

What stays where it is:

- **Pads, vias, locked tracks and curved tracks** are never pushed, and a
  pushed track keeps its ends on its pads and vias. A pushed track whose
  new path would come too close to one of them cannot give way.
- **Keep-out areas and the board edge** limit how far a track can move.
- Tracks of the **routed net** are never pushed (the route may run over
  them).
- The parts of a pushed track that stay where they were are left as they
  are, even if they already broke a rule.

When the tracks in the way cannot make room, the route **walks around**
them instead, like in the other mode. Vias are placed only where they fit
without pushing.

## Differential pairs

A differential pair is two nets that carry one signal with opposite
polarity: the data lines of USB, Ethernet, HDMI, PCI Express or LVDS, or a
CAN bus. Both tracks must run side by side at a constant gap, with the
same length. ElectronicWerk finds pairs by the **end of their net names**:

| Positive                   | Negative                        | Example                                                                    |
| -------------------------- | ------------------------------- | -------------------------------------------------------------------------- |
| `P`, `+` or `H` at the end | `N`, `M`, `-` or `L` at the end | `USB_DP` / `USB_DN`, `USB_D+` / `USB_D-`, `TX_P` / `TX_N`, `CANH` / `CANL` |

The rest of both names must be the same, and there must be exactly one net
of each polarity (`VP`, `VN` and `VM` together are no pair).

To route a pair:

1. Choose **Route differential pair** in the tool bar or press **6**.
2. Click on a pad (or a track end, or a via) of one net of the pair. The
   other net's nearest pad within 10 mm is the start of its track. The
   status line shows `Routing the pair USB_DP / USB_DN` with the width and
   the gap.
3. Move the cursor: the router finds a way for both tracks together, as if
   they were one track as wide as both and their gap. Both tracks leave
   their pads straight and join each other at 45°.
4. Click to fix the route up to the cursor and go on, as with a single
   track. **Backspace** takes back the last fixed part, **/** switches
   between the diagonal and the straight segment first.
5. Click on a pad of either net at the other end: both tracks end on their
   pads (each net on its nearest pad there) and the pair is put into the
   board. Otherwise **Enter**, a double-click or **Esc** ends the pair where
   it is.

![Routing a USB pair from pins 2 and 3 of U1](/screenshots/pairs-routing.png)

![The finished pair](/screenshots/pairs-result.png)

The **width and gap** come from the net class: its
[impedance profile](./impedance) on the active layer when it has one,
otherwise its **Diff. pair width** and **Diff. pair gap**
([net classes](./design-rules#net-classes)). Keep the gap at least as large
as the clearance of the class, or the DRC reports the two tracks.

Good to know:

- Both tracks of a pair are one undo step.
- A pair stays on its layer: finish it, then change the layer of each net
  with a single route and a via. Pairs are routed around other copper
  (walkaround), even when the router is set to push and shove.
- The [design rules check](./drc) measures the gap of coupled tracks
  (`pair-gap`) for classes with a differential impedance profile.
- [`pcb.pairs`](../reference/engine-api#pcb-routepair-pcb-pairs) lists the
  pairs of a board with the routed length of each net and their difference;
  matching the lengths follows with length tuning.

## Vias and layer changes

Press **V** while routing to place a via at the end of the live path and
continue on the opposite outer layer (top ↔ bottom; from an inner layer to
the top). **Page Up** and **Page Down** place a via too, and continue on the
previous or next copper layer. The via outline at the cursor turns red when
no via fits there; move the end of the route first.

Vias get the diameter and drill of the net's class ([design
rules](./design-rules#net-classes)), but never less than the board minimums.
The **Via type** in the toolbar chooses which via a layer change places:

| Via type              | The via goes                                                                                       |
| --------------------- | -------------------------------------------------------------------------------------------------- |
| Through via (default) | From the top to the bottom layer, whatever layers the route changes between.                       |
| Blind / buried via    | Only from the layer the route leaves to the layer it continues on.                                 |
| Microvia              | Only between neighbouring layers, with the microvia sizes of the net class. Choose the next layer. |

Blind, buried and microvias must be allowed in the design rules first; see
[Blind, buried and microvias](./vias).

The **Place via** tool (**V** outside routing) places single vias, for
example stitching vias in a ground zone: each click places a via with the
net of the copper below it.

## Track widths

New tracks take the track width of their net's class, unless a custom rule
sets a **track width for routing** for them. When the class follows an
[impedance profile](./impedance), the width comes from the profile and
**changes with the layer**: after a layer change the route goes on with the
width that gives the target impedance on the new layer. The width is shown in the
status line while routing. To change the width of tracks afterwards, select
them and edit the width in the [properties panel](./layout-editor#editing-in-the-properties-panel);
the [design rules check](./drc) reports tracks thinner than the minimum.

## Dragging tracks

With the select tool, **drag a single straight track segment** to move it
sideways (only the part of the movement across the segment counts). The
tracks attached to its ends **keep their directions**: their ends slide
along their own lines to the moved segment, so a route of 45° bends stays a
route of 45° bends. While you drag, the result is drawn over the board.

| At an end of the segment                                                              | What happens                                                    |
| ------------------------------------------------------------------------------------- | --------------------------------------------------------------- |
| One straight track                                                                    | Its end slides along it to the moved segment.                   |
| Nothing                                                                               | The end moves with the segment.                                 |
| A pad, a via, a branch (several tracks), a locked or curved track, or a track in line | The end stays; a short 45° track joins it to the moved segment. |

A drag that would turn the segment round (its ends would pass each other)
does nothing. Curved tracks are moved as a whole, and the tracks and vias at
their ends follow.

Press **D** with a track selected to do the same with the cursor, and click
to drop it. Press **M** instead to move the segment alone, without its
neighbours.

Moving a whole selection (**M**, or dragging footprints) moves everything
selected by the same offset; tracks attached to moved footprints are not
dragged along, so the ratsnest and the [DRC](./drc) show what needs to be
re-routed.

## Tips

- Route the critical nets first (supply, clocks, differential pairs), then
  the rest.
- Use [net classes](./design-rules#net-classes) to give power nets wider
  tracks: the router then uses the right width automatically.
- If the router cannot get through a gap, the clearance is probably larger
  than you think: hover over the pads and check their net classes, or look
  at the finding in the [DRC panel](./drc) after routing a test track.
- Routing over the board edge, through keep-out areas or into copper of
  other nets is impossible by design; draw a different path with fixed
  segments (click) instead.
