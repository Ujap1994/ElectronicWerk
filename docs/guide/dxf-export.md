# DXF drawings

The **DXF drawing** output writes the board outline and the mechanical and
user layers as a DXF file, for mechanical CAD: design a case or a front
panel around the board, or send the outline to a board maker that wants it
as a drawing.

![The DXF output in an output job](/screenshots/output-dxf.png)

## Adding the output

1. Open the output job (**Outputs** in the toolbar).
2. Choose **DXF drawing (outline, mechanical layers)** under **Add output**.
3. Choose the layers, the units and whether each layer gets a file of its
   own.
4. **Generate this output**, or **Generate all**.

| Setting                | Default              | Meaning                                                                                                                                                                                                      |
| ---------------------- | -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **Layers**             | outline and drawings | With **Board outline and every mechanical and user layer with drawings** checked, exactly those layers. Unchecked, choose them from the outline, mechanical 1–4, user 1–4, fabrication and courtyard layers. |
| **Units**              | Millimetres          | Millimetres or inches.                                                                                                                                                                                       |
| **One file per layer** | off                  | Off: one file `<board>.dxf` with a DXF layer per board layer (`Board_outline`, `Mechanical_1`, …). On: a file per layer, `<board>-Board_outline.dxf`, …                                                      |
| **Subfolder**          | `mechanical`         | The folder in the job's output folder.                                                                                                                                                                       |

## What the file contains

- **Board drawings** on the chosen layers (lines, arcs, circles,
  rectangles, polygons, curves), the **drawings of the parts** on those
  layers (as placed, bottom parts mirrored) and **board texts** (one line,
  as DXF `TEXT`).
- Shapes are written as their **centre lines**; line widths and fills are
  not drawn. Arcs and circles stay exact; Bézier curves become short
  segments.
- Coordinates are relative to the board's **drill and placement origin**
  (see [Board setup](./board-setup)); DXF's y axis points up like the
  board's.
- The format is ASCII **DXF R12** (`AC1009`), which every CAD program reads.
  The units are in the header (`$INSUNITS`).

With a [panel](./panels), the drawing shows the whole panel: the copies of
the board, the rails and — on _User 1_ — the V-score lines.

The DXF import of the [board outline](./board-setup#board-outline) reads
these files back, so a drawing can travel between ElectronicWerk and
mechanical CAD in both directions.

## In the job file

The output has the kind `dxf`; its options are `layers`, `units` (`mm` or
`inch`), `separate` and `folder`. See the
[file format reference](../reference/file-format#output-jobs).
