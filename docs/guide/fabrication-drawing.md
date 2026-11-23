# Fabrication drawing

The Gerber and drill files say where copper, mask and holes are. A board
maker also wants to know the rest: the size of the board, the material, the
layer stack-up, the copper weight, the finish, the colours, the tolerances
and the quality class. The **fabrication drawing** puts all of it on one
page, made from the board data.

![The fabrication drawing of the example board](/screenshots/output-fabrication-drawing.png)

## What is on the drawing

- **The board** — its outline with the cut-outs and slots, every hole marked
  by the symbol of its drill, and every [dimension](./dimensions) of the
  board, whatever its layer. The overall width and height are added below
  and left of the board.
- **The scale** — 1:1 up to 200 mm, then 1:2, 1:2.5, 1:4, 1:5, 1:10 … so the
  board fits the page.
- **The drill table** — symbol, finished diameter (mm and inches), count and
  kind of every drill: plated, non-plated, via, slots, and the
  [IPC-4761 filling](./padstacks-and-via-filling#via-filling-and-covering-ipc-4761)
  of filled or covered vias.
- **The stack-up** — every layer from top to bottom: silkscreen, solder mask,
  copper (top, inner 1, … bottom) and dielectric with material, Dk and
  thickness, and the total thickness.
- **The notes** — numbered, first the notes made from the board data, then
  your own.

The notes from the board data are:

1. _Board: 40 × 30 mm, 1.6 mm thick, 4 copper layers._
2. _Base material:_ the dielectric materials of the stack-up.
3. _Finished copper:_ outer and inner copper in µm and oz.
4. _Surface finish_, _Solder mask_ and _Silkscreen_ colours from the
   [board setup](./board-setup).
5. _Smallest features:_ the narrowest track and the smallest drill.
6. _Hole sizes are finished sizes (after plating)._
7. One note per [via filling](./padstacks-and-via-filling), if any.
8. _Tolerances unless noted: outline ±0.1 mm, plated holes ±0.075 mm,
   non-plated holes ±0.05 mm._
9. _Manufacture and inspect to IPC-6012 and IPC-A-600 class 2._
10. _All dimensions in millimetres._

The texts of the drawing are English, the language board makers read.

## In the output job

![The settings of the fabrication drawing in the output job](/screenshots/outputs-fabrication-drawing.png)

The default **Fabrication** [output job](./outputs) has a **Fabrication
drawing** after the drill map; it writes
`gerber/<board>-fabrication-drawing.pdf` next to the Gerber files. The
**Fabrication package** (ZIP) holds the Gerber and drill files by default;
tick **Fabrication drawing** in its settings to send the drawing along.
Its settings:

| Setting                       | Meaning                                                                                                                            |
| ----------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- |
| **Format**                    | PDF (one file) or SVG.                                                                                                             |
| **Notes from the board data** | The notes listed above (on by default). Untick to write only your own.                                                             |
| **IPC class**                 | Class 1 (general electronics), 2 (dedicated service, the default) or 3 (high reliability), or not named at all.                    |
| **Further notes**             | One note per line, after the generated ones. [Text variables](./project-tree#text-variables) like `${PROJECT_NAME}` are filled in. |

Add a fabrication drawing to another job with **Add output → Fabrication
drawing**. From a script, the output kind is `fabricationDrawing` (see the
[file format](../reference/file-format#output-jobs)).

## Tips

- Put the important sizes onto the board as [dimensions](./dimensions) —
  mounting holes, connector positions, the position of a cut-out. They
  appear on the drawing as drawn.
- Set the stack-up, the finish and the colours in the
  [board setup](./board-setup) before generating; the drawing shows what is
  set there.
- Many board makers take the order details from their web form, not from
  the drawing. The drawing is still the record of what was ordered and the
  reference for a dispute — keep it with the order.
