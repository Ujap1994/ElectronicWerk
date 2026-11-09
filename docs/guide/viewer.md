# Viewing sheets and boards

ElectronicWerk draws schematic sheets and boards in a fast 2D view. You can
pan and zoom smoothly through large designs, select objects, read their
properties, and choose which layers you see. Sheets are edited right in this
view (see [Drawing schematics](./schematic-editor)); boards are read-only
until the layout editor arrives in version 0.8.

![A board in the 2D view](/screenshots/viewer-board.png)

## Opening a document

Sheets and boards open in **tabs** next to the **Details** tab. There are four
ways to open one:

- Double-click it in the [project tree](./project-tree).
- Select it in the tree and press <kbd>Enter</kbd>.
- Right-click it in the tree and choose **Open**.
- Select it in the tree and click **Open** at the top of its details.

A document that is already open is brought to the front instead of being
opened twice. Click **Details** to go back to the properties of the item
selected in the tree, and click **×** on a tab to close it. Tabs of documents
you remove from the project close by themselves.

Each tab remembers its view while the project is open: zoom, position, layer
visibility, active layer and selection tool are restored when you return to
it.

::: tip Changes appear immediately
The view follows every change to the project, including changes from
**Undo**, **Redo** and scripts that use [`ewerk rpc`](../reference/cli#ewerk-rpc).
The selection is kept when the document is redrawn.
:::

## The window

| Area                 | Purpose                                                                                                                                                       |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **View toolbar**     | Zoom in and out, fit the document to the window, zoom to the selection, choose box or lasso selection, and (boards) highlight layers or view from the bottom. |
| **Drawing area**     | The sheet or board. The dotted grid follows the grid of the project settings and becomes coarser when you zoom out.                                           |
| **Status line**      | The cursor position in the project's units, the object under the cursor, the current grid spacing and the zoom level.                                         |
| **Layers / Display** | What is shown and how (see [Layers](#layers)).                                                                                                                |
| **Properties**       | The properties of the selected object (see [Inspector](#inspector)).                                                                                          |

The zoom level is given in percent of the real size: at **100 %**, a 10 mm
part is about 10 mm wide on a typical screen.

## Navigating

### Mouse

| Action                               | Effect                                                  |
| ------------------------------------ | ------------------------------------------------------- |
| Turn the wheel                       | Zoom in or out around the cursor                        |
| Drag with the middle or right button | Pan                                                     |
| Hold <kbd>Space</kbd> and drag       | Pan                                                     |
| <kbd>Shift</kbd> + wheel             | Pan sideways                                            |
| <kbd>Ctrl</kbd>/<kbd>⌘</kbd> + wheel | Zoom continuously (for mice with a free-spinning wheel) |

### Trackpad

| Gesture                 | Effect                  |
| ----------------------- | ----------------------- |
| Scroll with two fingers | Pan in every direction  |
| Pinch                   | Zoom around the fingers |

### Keyboard

Click the drawing area first so that it has the keyboard focus (a blue frame
appears when you move there with <kbd>Tab</kbd>).

| Key                                                 | Effect                                                                          |
| --------------------------------------------------- | ------------------------------------------------------------------------------- |
| <kbd>←</kbd> <kbd>→</kbd> <kbd>↑</kbd> <kbd>↓</kbd> | Pan (with <kbd>Shift</kbd>: farther)                                            |
| <kbd>+</kbd> / <kbd>-</kbd>                         | Zoom in / out around the centre                                                 |
| <kbd>Home</kbd>                                     | Fit the whole sheet or board                                                    |
| <kbd>F</kbd>                                        | Zoom to the selection                                                           |
| <kbd>B</kbd>                                        | View the board from the bottom (and back); on a sheet, <kbd>B</kbd> draws a bus |
| <kbd>Ctrl</kbd>/<kbd>⌘</kbd> + <kbd>A</kbd>         | Select every visible object                                                     |
| <kbd>Esc</kbd>                                      | Clear the selection or cancel a drag                                            |

Zooming is animated and always keeps the point under the cursor in place.
You can zoom out until the document is a tenth of the window size, and in
until a pixel is 20 nm wide.

## Selecting objects

### Click

Click an object to select it. You do not have to hit thin lines exactly: an
object within a few pixels of the cursor counts. When several objects are
there, objects directly under the cursor win over objects that are only near
it, and zones come last. Among the rest, the most specific one wins:

1. pads, pins and sheet pins,
2. vias, junctions, no-connect flags and net labels,
3. tracks, wires, buses and power ports,
4. texts, designators and values,
5. graphics,
6. footprints, symbols and sheet symbols (anywhere inside their outline),
7. zones.

Among objects of the same kind, objects on the [active layer](#active-layer)
come first, then the closest one. So a click on a pad selects the pad, a click
on the body of a part next to a pad selects the part, and a click next to a
track inside a copper zone selects the track. Clicking empty space clears the
selection.

| Modifier                             | Effect                             |
| ------------------------------------ | ---------------------------------- |
| <kbd>Shift</kbd> + click             | Add to the selection               |
| <kbd>Ctrl</kbd>/<kbd>⌘</kbd> + click | Toggle the object in the selection |

Selected objects are highlighted; a selected footprint or symbol also
highlights its pads, pins, designator and value. The object under the cursor
is lightly highlighted, and the status line names it (with its net on
boards).

![A selected pad with its properties](/screenshots/viewer-pad-selected.png)

### Box selection

Drag on empty space to draw a box:

- **Left to right** (blue, solid): selects the objects that are completely
  inside the box.
- **Right to left** (green, dashed): selects every object the box touches.

![A crossing box selection while dragging](/screenshots/viewer-box-selection.png)

Box and lasso selection pick whole footprints, symbols and sheets, not their
pads or pins. Hold <kbd>Shift</kbd> to add to the selection, or
<kbd>Ctrl</kbd>/<kbd>⌘</kbd> to toggle.

### Lasso selection

Choose the **lasso** in the view toolbar, or hold <kbd>Alt</kbd> while you start
dragging, and draw around the objects. Everything completely inside the lasso
is selected.

![Lasso selection](/screenshots/viewer-lasso.png)

Objects on hidden layers are never selected.

## Inspector

The **Properties** panel shows the selected object. Lengths and positions use
the units of the [project settings](./project-tree#project-properties)
(millimetres, mil or inches); positions of pads and pins are absolute.

![The inspector](/screenshots/viewer-inspector.png)

| Object             | Properties                                                                                         |
| ------------------ | -------------------------------------------------------------------------------------------------- |
| Track              | Layer, net, width, start, end (arc midpoint for arcs), length, locked                              |
| Via                | Net, position, diameter, drill, from and to layer, microvia, locked                                |
| Footprint          | Designator, value, footprint, position, rotation, side, pads, mounting, locked, assembly flags     |
| Pad                | Number, footprint, net, pad type, shape, position, size, rotation, layers, drill                   |
| Zone               | Name, net, layers, priority, corners, keep-out, clearance                                          |
| Symbol             | Designator, value, symbol, footprint, position, rotation, mirrored, unit, pins, description, flags |
| Pin                | Number, name, symbol, electrical type, graphic style, position, length                             |
| Wire, bus          | Start, end, length                                                                                 |
| Net label          | Text, label type, shape, position, rotation, text size                                             |
| Texts and graphics | Text or shape, layer, position, size, width                                                        |

With several objects selected, the panel lists how many of each kind are
selected.

![Several objects selected](/screenshots/viewer-multiple-selected.png)

## Layers

On boards, the **Layers** panel lists the copper layers from top to bottom,
followed by silkscreen, solder mask, paste, fabrication and courtyard layers,
the board outline and the drill holes. Copper layers with a name in the
[stack-up](./project-tree#boards) show that name.

![The layer panel](/screenshots/viewer-layers.png)

| Control            | Effect                                                             |
| ------------------ | ------------------------------------------------------------------ |
| Eye                | Show or hide the layer                                             |
| Layer name         | Make it the [active layer](#active-layer) (click again to clear)   |
| **Opacity** slider | Transparency of the active layer                                   |
| **Show all**       | Show every layer                                                   |
| **Only active**    | Hide everything except the active layer, the outline and the holes |

Solder mask and paste are hidden by default. Layers without anything on them
are marked **Empty**. Overlapping shapes on one layer never darken each other,
so a track that runs over a pad looks like one piece of copper.

### Active layer

The active layer is drawn on top of the other copper and side layers, and
objects on it win when you click. On a new board, the top copper is active.
**Highlight the active layer** dims every other layer, which makes inner
layers easy to follow.

![Bottom copper highlighted](/screenshots/viewer-high-contrast.png)

### View from the bottom

**View from the bottom** (<kbd>B</kbd>) mirrors the board as if you turned it
over: bottom layers come to the front, and texts on the bottom side become
readable. If the top copper was the active layer, the bottom copper becomes
active (and back when you turn the board again). The outline and drill holes
stay on top.

![The board seen from the bottom](/screenshots/viewer-bottom-view.png)

### Sheets

On sheets, the **Display** panel lists the kinds of objects instead of
layers: wires, buses, junctions, net labels, power ports, symbol bodies and
fill, pins, pin names and numbers, designators and values, sheet symbols,
no-connect flags, texts, graphics, the frame with the title block, and the
paper.

![A sheet in the 2D view](/screenshots/viewer-sheet-light.png)

The title block shows the sheet name, the file, the page size, the date and
the revision. Text variables such as `${REVISION}` are replaced with their
values (see [Text variables](./project-tree#text-variables)). Symbols whose
definition is missing from the project library appear as a box with a
question mark.

![A selected symbol](/screenshots/viewer-sheet-symbol.png)

Sheets follow the colour theme of the app; boards always use a dark
background, which makes copper colours easier to tell apart.

![A sheet in the dark theme](/screenshots/viewer-sheet-dark.png)

## Text

Texts on sheets and boards use the monospace font **JetBrains Mono**, which is
included in the app, so every computer shows the same text widths. Very small
texts are left out until you zoom in far enough to read them.

## Performance

The view is drawn by the graphics card (WebGL 2). The engine prepares every
shape once when a document opens; panning and zooming then only redraw. On a
laptop with integrated graphics (Apple M3), the synthetic
[L reference design](../development/benchmarks) with 5,000 parts and about
590,000 shapes opens in 0.2 s and pans and zooms at 60 frames per second. See
[Benchmarks](../development/benchmarks#viewer) for details.

::: warning WebGL 2 is required
If the drawing area shows **The 2D view needs WebGL 2**, the graphics driver
does not support it. Update the driver; on computers without a usable
graphics card, ElectronicWerk falls back to a slower software renderer. See
[Troubleshooting](./troubleshooting#the-2d-view-stays-empty-or-is-slow).
:::
