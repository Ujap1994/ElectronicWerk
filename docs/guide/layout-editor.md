# The layout editor

A board opens in the layout editor: the same 2D view you know from
[viewing boards](./viewer), with tools to place footprints, route tracks,
draw copper zones and keep-out areas, and to check the board against its
design rules while you work.

![The layout editor with the online design rules check and the ratsnest](/screenshots/layout-editor.png)

The editor is built for the usual way of designing a board:

1. Draw the schematic and check it with the [ERC](./erc).
2. [Update the board from the schematic](./schematic-to-board): the parts
   arrive below the board outline, with their pads connected to the nets.
3. [Set up the board](./board-setup): pick a preset for your board maker,
   check the layer stack-up and draw or import the outline.
4. Place the footprints. The **ratsnest** (thin dashed lines) shows the
   connections still to make.
5. [Route the tracks](./routing) and [pour copper zones](./zones).
6. Fix what the [design rules check](./drc) reports until it is clean.

## The screen

| Area                   | What it holds                                                                                                                                                                                   |
| ---------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **View toolbar** (top) | Zoom, fit, box and lasso selection, the **active copper layer**, the [corner mode](./routing#45-and-90-corners) of the router, edit actions, board commands, high contrast and the bottom view. |
| **Tools** (left)       | The layout tools, one column (see below).                                                                                                                                                       |
| **Board**              | The drawing. Previews of the tool in use, the ratsnest and the markers of the design rules check are drawn over it.                                                                             |
| **DRC panel** (bottom) | The findings of the [design rules check](./drc), opened with the check button.                                                                                                                  |
| **Layers** (right)     | Visibility, opacity and the active layer of every board layer, as in the [viewer](./viewer#layers).                                                                                             |
| **Properties** (right) | The selected object: its details, and fields to edit footprints, tracks, vias and texts ([below](#editing-in-the-properties-panel)).                                                            |
| **Status line**        | Cursor position, the object under the cursor, a hint for the tool in use, the net and width while routing, the grid and the zoom.                                                               |

![The layout tools](/screenshots/layout-tools.png)

## Tools

Pick a tool in the column on the left or with its key. **Esc** ends what is
in progress, and a second **Esc** returns to the select tool.

| Tool                | Key | What it does                                                                                                                |
| ------------------- | --- | --------------------------------------------------------------------------------------------------------------------------- |
| **Select**          | S   | Click to select, drag to move. Dragging a single track keeps it connected (see [dragging](./routing#dragging-tracks)).      |
| **Place footprint** | A   | Opens the footprint chooser; each click places the chosen footprint. **R** turns the next one.                              |
| **Route tracks**    | X   | [Interactive routing](./routing) from a pad, track or via. The route walks around copper of other nets.                     |
| **Place via**       | V   | Each click places a via. It takes the net of the copper below the click and the sizes of that net's class.                  |
| **Copper zone**     | Z   | Click the corners of a [copper zone](./zones); **Enter** or a double-click closes it and opens its settings.                |
| **Keep-out area**   | K   | Click the corners of an area where tracks, vias, pads, zones or footprints are not allowed.                                 |
| **Board outline**   | O   | Two clicks draw a rectangular outline. **/** switches to polygon outlines (click the corners, **Enter** closes).            |
| **Text**            | T   | Click where a text goes and type it. It goes on the silkscreen of the side of the active layer (mirrored on the bottom).    |
| **Highlight net**   | N   | Click copper to highlight every pad, track, via and zone of its net. **`** highlights the net under the cursor in any tool. |

## The active layer

The **layer selector** in the toolbar names the copper layer that new tracks,
zones and keep-out areas use. **Page Up** and **Page Down** step through the
copper layers. While routing, changing the layer places a via (see
[routing](./routing#vias-and-layer-changes)). Footprints placed while a
bottom layer is active go on the bottom side.

## Selecting and editing

Click an object to select it; **Shift**, **Ctrl/⌘** and box or lasso
selection work as in the [viewer](./viewer#selecting-objects). Clicking a pad
selects the pad (its details show in the properties panel); moving or
turning it moves or turns its footprint.

| Action                      | Key            | Toolbar                       |
| --------------------------- | -------------- | ----------------------------- |
| Move with the mouse         | drag           |                               |
| Move with the cursor        | M, then click  |                               |
| Drag a track (keeps joints) | D, then click  |                               |
| Turn by 90°                 | R / Shift+R    | Turn                          |
| Flip to the other side      | Shift+F        | Flip to the other side        |
| Lock / unlock               | L / Shift+L    | Lock                          |
| Align and distribute        |                | Align and distribute (menu)   |
| Delete                      | Del, Backspace | Delete                        |
| Undo / redo                 | Ctrl/⌘+Z       | Undo, Redo in the project bar |

- **Flipping** puts footprints on the other side of the board: they are
  mirrored, their pads and graphics move to the layers of that side, and the
  rotation is mirrored too. Tracks change to the mirrored copper layer, and
  texts are mirrored. The mirror line is the vertical line through the
  middle of the selection.
- **Locked** objects stay where they are: moving, turning, flipping and
  deleting skip them. Lock connectors and mounting holes once they are
  placed.
- **Align and distribute** line up the selected footprints by their left,
  right, top or bottom edges or centres, or spread three or more footprints
  with equal gaps between them.

Every edit is one undo step, with a name in the **Undo** button (`move`,
`rotate`, `flip`, `route`, …).

### Editing in the properties panel

With one footprint, track, via or board text selected, the properties panel
shows fields to change it:

![A footprint in the properties panel](/screenshots/layout-properties.png)

| Object    | Fields                                                              |
| --------- | ------------------------------------------------------------------- |
| Footprint | Designator, value, rotation, side (with a flip button), locked      |
| Track     | Width, copper layer, locked                                         |
| Via       | Diameter, drill, locked                                             |
| Text      | Text                                                                |
| Zone      | Double-click the zone to open its [settings](./zones#zone-settings) |

Changes apply when you press **Enter** or leave the field. Designators and
values changed here can go back to the schematic with
[back annotation](./schematic-to-board#back-annotation).

## Placing footprints

Footprints normally come from the schematic: the
[board update](./schematic-to-board) places every part of the schematic
below the board, and you move them into place. Parts that exist only on the
board — mounting holes, fiducials, logos, test points — are placed with the
**Place footprint** tool (**A**): pick a footprint of the project libraries
in the chooser, then click on the board. They get designators `X1`, `X2`, …
which you can change in the properties panel.

## The ratsnest

Thin dashed lines connect pads of the same net that are not yet joined by
copper. They form the shortest tree between the separate pieces of each
net, so they always point to the nearest place to connect. Each finished
connection removes a line. The **ratsnest** button in the toolbar hides and
shows them; the number of open connections is in the summary of the
[DRC panel](./drc).

## Cross-probing

Select a symbol in the schematic, switch to the board, and its footprint is
selected there — and the other way round: select footprints on the board,
and their symbols are selected on their sheets. This works across tabs, so
you can keep both open and switch with a click. The **cross-probe** button
in the toolbar switches it off.

## Board commands

| Command                   | Toolbar button            | Command palette                     |
| ------------------------- | ------------------------- | ----------------------------------- |
| Update from the schematic | Update from the schematic | Board: update from the schematic    |
| Fill all zones (Shift+B)  | Fill all zones            | Board: fill all zones               |
| Design rules check        | Design rules check        | Board: run the design rules check   |
| Board setup               | Board setup               | Board: set up the board             |
| Design rules              | Design rules              | Board: design rules and net classes |
| Import a DXF drawing      | Import a DXF drawing      | Board: import a DXF drawing         |
| Place a footprint         | Place footprint (tool)    | Board: place a footprint            |

The [keyboard shortcuts](./keyboard-shortcuts#layout-editor) page lists all
keys of the layout editor.
