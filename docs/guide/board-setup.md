# Board setup and outline

**Board setup** (button in the board toolbar, or **Board: set up the
board** in the command palette) collects what describes the board itself:
presets for common kinds of boards, the layer stack-up and a rectangular
outline. Outlines of any shape are drawn with the outline tool or
[imported from DXF](#importing-a-dxf-drawing).

## Presets

![Board presets](/screenshots/layout-setup-presets.png)

A preset sets the layer stack-up, the [board minimums](./design-rules#board-minimums)
and the sizes of the `Default` net class in one step. Other net classes are
raised to the new minimums where they fall below them. Applying a preset is
one undo step (`board-preset`).

| Preset                  | Layers | Thickness | Clearance / track | Via (diameter / drill) | Annular ring | Finish |
| ----------------------- | ------ | --------- | ----------------- | ---------------------- | ------------ | ------ |
| Home etching, 1 layer   | 1      | 1.6 mm    | 0.4 / 0.5 mm      | 1.8 / 0.8 mm           | 0.4 mm       | None   |
| Standard, 2 layers      | 2      | 1.6 mm    | 0.2 / 0.2 mm      | 0.6 / 0.3 mm           | 0.13 mm      | HASL   |
| Standard, 4 layers      | 4      | 1.6 mm    | 0.127 / 0.127 mm  | 0.45 / 0.2 mm          | 0.125 mm     | ENIG   |
| Fine pitch, 6 layers    | 6      | 1.6 mm    | 0.1 / 0.1 mm      | 0.4 / 0.2 mm           | 0.1 mm       | ENIG   |
| Thin, 2 layers (0.8 mm) | 2      | 0.8 mm    | 0.2 / 0.2 mm      | 0.55 / 0.25 mm         | 0.13 mm      | ENIG   |

The values are the minimums; the `Default` class uses slightly more
comfortable sizes (for example 0.25 mm tracks on the standard 2-layer
board). Always check the capabilities of your board maker and adjust the
[design rules](./design-rules) to them.

## Stack-up

![The layer stack-up](/screenshots/layout-setup-stackup.png)

The stack-up lists the layers from top to bottom: silkscreen, solder mask,
the copper layers and the insulating layers (core and prepreg) between
them.

- **Copper layers** rebuilds a standard FR4 stack with 1, 2, 4, 6, 8 or 10
  copper layers and the current board thickness. Copper is 35 µm (1 oz),
  solder mask 10 µm; the insulating layers share the rest equally,
  alternating prepreg and core on multilayer boards.
- **Board thickness** rebuilds the stack with the same number of copper
  layers and the new thickness.
- In the table, change the **thickness** of every layer, the **material**
  of insulating layers (FR4, Rogers, polyimide …) and their relative
  permittivity **εr**. The total thickness is shown below the table.
- **Surface finish** (HASL, ENIG, OSP, …) is stored for the manufacturing
  outputs.

**Save the stack-up** applies the changes as one undo step. Removing copper
layers that carry tracks or zones leaves those items on layers the board no
longer has; the design rules check reports them, so move them first.

## Board outline

![A rectangular outline](/screenshots/layout-setup-outline.png)

The **Outline** tab sets a rectangular board outline from the origin with
the given width and height. It replaces the shapes of the current outline.

For other shapes, use the **Board outline** tool (**O**):

- Two clicks draw a rectangle.
- **/** switches to polygon mode: click the corners, **Enter** or a
  double-click closes the outline.

Outline shapes are ordinary board shapes on the outline layer: select,
move, turn or delete them like any other object. Lines, arcs and polylines
that meet end to end (within 10 µm) form a closed outline; the largest
closed shape is the board, smaller ones inside it are cut-outs. The
[design rules check](./drc) reports a missing or open outline.

## Importing a DXF drawing

Mechanical outlines usually come from the enclosure design as a DXF file.
Click **Import a DXF drawing** in the board toolbar (or **Board: import a
DXF drawing** in the command palette) and choose the file.

![The DXF import](/screenshots/layout-dxf.png)

The dialog shows what the file contains: the number of shapes, the units
and the size, the DXF layers and any entities that cannot be imported. Then
choose:

| Option                   | Meaning                                                                                             |
| ------------------------ | --------------------------------------------------------------------------------------------------- |
| DXF layers               | Which layers of the drawing to import (for example only the board edge, not the dimensions).        |
| Drawing units            | As given in the file (`$INSUNITS`), or millimetres, inches or mils if the file says nothing useful. |
| Board layer              | Board outline, a mechanical layer or a user drawing layer.                                          |
| Replace                  | Removes the shapes already on that layer first (the old outline).                                   |
| Move to the board origin | Moves the lower left corner of the drawing to the origin; otherwise the DXF coordinates are kept.   |

What is imported:

| DXF entity               | Becomes                                                         |
| ------------------------ | --------------------------------------------------------------- |
| `LINE`                   | Line                                                            |
| `CIRCLE`                 | Circle                                                          |
| `ARC`                    | Arc (a full-circle arc becomes a circle)                        |
| `LWPOLYLINE`, `POLYLINE` | Polygon (closed) or polyline; segments with a bulge become arcs |
| `SPLINE`                 | Polyline through the fit points (or the control points)         |
| `ELLIPSE`                | Polygon or polyline with 64 segments                            |

Texts, dimensions, hatches and block inserts are skipped and listed in the
dialog. DXF's y axis points up and the board's down, so the drawing is
mirrored vertically on import to look the same as in the CAD program. The
import is one undo step (`import-dxf`).
