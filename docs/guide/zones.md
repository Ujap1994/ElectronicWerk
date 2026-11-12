# Copper zones and keep-out areas

A **copper zone** (also called a pour or plane) fills an area of a copper
layer with copper of one net — typically a ground plane — keeping clear of
everything that belongs to other nets. A **keep-out area** is the opposite:
an area where tracks, vias, pads, zones or footprints are not allowed, for
example under an antenna or around a mounting hole.

![A zone drawn over the board, before its settings](/screenshots/layout-zone-drawing.png)

## Drawing a zone

1. Choose the copper layer in the toolbar (the zone starts on that layer).
2. Press **Z** (or pick **Copper zone**) and click the corners of the area.
   The outline may be any polygon; it does not need to stay inside the board
   (the fill does).
3. Press **Enter** or double-click to close the outline. **Backspace**
   removes the last corner, **Esc** cancels.
4. The zone settings open. Pick the **net** (usually GND), check the
   layers and click **Create**. The zone is filled at once.

![The settings of a new zone](/screenshots/layout-zone-dialog.png)

To change a zone later, **double-click** it with the select tool.

## Zone settings

| Setting                    | Default         | Meaning                                                                                                                                                                                                                                               |
| -------------------------- | --------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Name                       | empty           | Shown in DRC findings; custom rules can refer to it with `A.insideZone('name')`.                                                                                                                                                                      |
| Net                        | none            | The net of the copper. A zone without a net is filled, but its islands are kept (nothing connects them).                                                                                                                                              |
| Layers                     | active layer    | One zone can fill several copper layers with the same outline.                                                                                                                                                                                        |
| Priority                   | 0               | Zones fill from the highest priority down; a zone of another net keeps its clearance from the copper of every zone with a higher priority.                                                                                                            |
| Clearance                  | 0.3 mm          | Distance to copper of other nets. The fill uses the larger of this and the [design rules](./design-rules) clearance for each item.                                                                                                                    |
| Minimum width              | 0.25 mm         | Copper necks thinner than this are removed, so the fill has no thin slivers that break off in production.                                                                                                                                             |
| Pads of the net            | Thermal reliefs | **Thermal reliefs** keep a gap around each pad of the zone's net and connect it with four spokes, which makes soldering easier. **Solid** joins the pads to the zone directly. **Not connected** keeps the clearance around them like for other nets. |
| Thermal gap, spoke width   | 0.5 mm, 0.5 mm  | Size of the gap around the pad and width of the spokes.                                                                                                                                                                                               |
| Remove unconnected islands | on              | Pieces of copper that touch nothing of the zone's net (no pad, track or via) are removed.                                                                                                                                                             |
| Hatched fill               | off             | Fills with a grid of 0.25 mm copper lines and 0.75 mm openings, with a solid border. Used for flex boards and some RF ground planes.                                                                                                                  |

Tracks and vias of the zone's net always connect solidly.

## Filling

Zones are filled when they are created or changed. After moving parts or
routing tracks, fill them again: **Shift+B**, the **Fill all zones** button
or **Board: fill all zones** in the command palette. Filling is one undo
step (`fill-zones`). The [design rules check](./drc) warns about zones
without fill.

![The board after filling the new zone](/screenshots/layout-zone-filled.png)

How a zone is filled, layer by layer:

1. The zone outline is cut to the board outline, shrunk by the
   copper-to-edge distance.
2. All copper of other nets is taken out, each item grown by its clearance;
   so are the fills of higher-priority zones of other nets, non-plated
   holes and keep-out areas that forbid zones.
3. Pads of the zone's net get their thermal relief gap.
4. Necks thinner than the minimum width are removed (the fill shrinks by
   half the width and grows back).
5. Thermal spokes join the pads to the rest of the fill.
6. Islands without a connection to the net are removed.

Curves are approximated with a chord error below 5 µm, and every
approximation lies outside the true clearance, so a fresh fill never
violates the rules. Layers fill in parallel; a 100 × 80 mm board with a
few hundred parts fills in well under a second.

## Keep-out areas

Press **K** (or pick **Keep-out area**) and draw the area like a zone. The
settings ask for the layers and what is forbidden inside:

| Forbidden    | Effect                                                                |
| ------------ | --------------------------------------------------------------------- |
| Tracks       | The router does not enter the area; the DRC reports tracks inside it. |
| Vias         | The DRC reports vias inside it.                                       |
| Pads         | The DRC reports pads inside it.                                       |
| Copper zones | Zones are not filled inside the area.                                 |
| Footprints   | The DRC reports footprints whose pads lie inside it.                  |

Keep-out areas have no net and are never filled. They show as a hatched
area on their layers.
