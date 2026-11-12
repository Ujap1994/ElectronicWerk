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

## How the route finds its way

For every cursor position the router first tries the simplest path: one
straight and one diagonal segment (or the other way round). If that path
keeps every rule, it is used — this is what you see in open areas. If not,
the router searches a grid around the obstacles (an A\* search with
octilinear steps that avoids needless bends) and then straightens the
result into as few segments as possible. The search covers the area between
start and cursor plus a margin of at least 5 mm, so it stays fast: a preview
on a typical board takes a few milliseconds.

What counts as an obstacle:

| Obstacle                                                                   | Distance kept                                                                              |
| -------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------ |
| Pads, tracks, vias and zone fills of other nets (and copper without a net) | The clearance between the two net classes, or a [custom rule](./design-rules#custom-rules) |
| Non-plated holes                                                           | The largest clearance of the rules                                                         |
| Keep-out areas that forbid tracks                                          | None: the track must stay outside                                                          |
| The board edge                                                             | The copper-to-edge minimum (or a custom rule)                                              |

Copper of the routed net is never an obstacle, so routes may run over pads
and tracks of their own net.

## Vias and layer changes

Press **V** while routing to place a via at the end of the live path and
continue on the opposite outer layer (top ↔ bottom; from an inner layer to
the top). **Page Up** and **Page Down** place a via too, and continue on the
previous or next copper layer. The via outline at the cursor turns red when
no via fits there; move the end of the route first.

Vias get the diameter and drill of the net's class ([design
rules](./design-rules#net-classes)), but never less than the board minimums.
They are through-hole vias from the top to the bottom layer.

The **Place via** tool (**V** outside routing) places single vias, for
example stitching vias in a ground zone: each click places a via with the
net of the copper below it.

## Track widths

New tracks take the track width of their net's class, unless a custom rule
sets a **track width for routing** for them. The width is shown in the
status line while routing. To change the width of tracks afterwards, select
them and edit the width in the [properties panel](./layout-editor#editing-in-the-properties-panel);
the [design rules check](./drc) reports tracks thinner than the minimum.

## Dragging tracks

With the select tool, **drag a single track segment** to move it sideways:
the tracks attached to its ends stretch to follow, and vias at its ends move
with it (together with the tracks on other layers that end at those vias).
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
