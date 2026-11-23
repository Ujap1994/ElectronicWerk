# Dimensions and leaders

Dimensions put measured sizes onto the board: the size of the board, the
distance between two mounting holes, the radius of a corner. A leader points
at something with a note ("Timer IC", "Do not populate", "Fiducial"). Both
are drawing objects on a layer of their own choice — usually **User 1** or a
fabrication layer — and they go into every output that shows that layer:
prints, PDF and SVG drawings, Gerber files of user layers and the
[fabrication drawing](./fabrication-drawing).

![The board's height on the left, the distance of R1 and D1 above, the width from the KiCad file below and a leader pointing at the timer IC](/screenshots/dimensions.png)

## Drawing a dimension

1. Choose the **dimension** tool in the tool palette (<kbd>Shift</kbd>+<kbd>D</kbd>).
2. Choose what it measures in the tool bar:

   | Kind                   | Measures                                                        |
   | ---------------------- | --------------------------------------------------------------- |
   | **Aligned**            | The straight distance between two points, along their direction |
   | **Horizontal**         | Only the difference in X                                        |
   | **Vertical**           | Only the difference in Y                                        |
   | **Radius**             | The radius of a circle or arc: centre, then a point on it       |
   | **Diameter**           | The diameter of a circle: centre, then a point on it            |
   | **Leader with a note** | Nothing: an arrow with a text                                   |

3. Click the first point, then the second point. The points snap to the
   grid of the layout editor; choose a fine grid to measure off-grid objects.
4. Move the mouse to where the dimension line should be — the preview shows
   the line, the arrows and the value — and click. For a radius or diameter
   the third click places the label.

<kbd>Backspace</kbd> takes back the last click, <kbd>Esc</kbd> ends the tool.

For a **leader**, click the tip of the arrow, then where the note goes, and
type the note.

## Editing a dimension

![The properties of a dimension](/screenshots/dimension-properties.png)

Select a dimension to change it in the properties:

| Field                              | Meaning                                                                                                         |
| ---------------------------------- | --------------------------------------------------------------------------------------------------------------- |
| **Text**                           | What is written. `{value}` stands for the measured value: `{value} max` gives "30.00 mm max". Empty: the value. |
| **Units**                          | Millimetres, mils or inches; **As the project** follows the [project's units](./settings).                      |
| **Decimals**                       | The number of decimals; **Automatic** uses two for millimetres, one for mils and three for inches.              |
| **Without the unit**               | Writes only the number.                                                                                         |
| **Distance of the dimension line** | How far the line is from the measured points (negative: the other side).                                        |
| **Text size**, **Line width**      | The look of the dimension.                                                                                      |
| **Layer**                          | Any drawing layer: user, fabrication, silkscreen or courtyard layers.                                           |
| **Locked**                         | It cannot be moved by accident.                                                                                 |

The value is always measured from the dimension's two points, so it stays
right when the dimension is moved, turned or flipped with the rest of a
selection. Turning a horizontal dimension by 90° makes it a vertical one.
The [inspector](./layout-editor) lists the kind, the value and both points.

## KiCad

Dimensions and leaders come along when a [KiCad project is imported](./import-kicad)
and are written when [exporting to KiCad](./export-kicad) as KiCad 8 and
newer have them: aligned, orthogonal (horizontal and vertical), radial and
leader dimensions. KiCad has no diameter dimension, so a diameter becomes an
aligned dimension across the circle. The side of the dimension line, the
text, the units and the number of decimals are kept; KiCad then measures the
value itself.

## Scripts

[`pcb.addDimension`](../reference/engine-api#dimensions-and-rooms) adds a
dimension from a script or an AI agent:

```json
{
  "method": "pcb.addDimension",
  "params": {
    "doc": "…",
    "kind": "vertical",
    "start": [100000000, 80000000],
    "end": [100000000, 110000000],
    "offset": -4000000
  }
}
```
