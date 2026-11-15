# Gerber files as a reference

Gerber and drill files can be drawn onto a board as a **reference
drawing**: filled areas on user layers, under your own design. Use it to

- **redraw a board** you only have the manufacturing files of: place the
  parts and route the tracks on top of the reference;
- **compare** a design with the files that were sent to the board maker.

The areas are drawings only — no copper, no nets — and do not end up in
your own Gerber files.

![The Gerber reference dialog](/screenshots/gerber-reference-dialog.png)

## Importing

1. Open the board in the [layout editor](./layout-editor).
2. Choose **Gerber reference** in the toolbar (or **Board: import Gerber
   files as a reference drawing** in the [command palette](./command-palette)).
3. **Choose Gerber and drill files…** and pick one or more files.
4. Check the list: each file shows its content (the X2 file function, for
   example `Copper,L1,Top`, or one guessed from the file name), its size
   and the **board layer** it goes to. Each file gets a free user layer;
   choose another user or mechanical layer if you like, or uncheck a file.
5. Choose **Import**.

| Option                                                          | Meaning                                                                                                                                       |
| --------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------- |
| **Move the lower left corner of the files to the board origin** | Moves all files together so that their lower left corner lies at the board origin (on by default). Off, the files keep their own coordinates. |
| **Replace the shapes already on these layers**                  | Removes what is on the chosen layers first, so a second import does not stack up (on by default).                                             |

The board's own Gerber files lie exactly on the board: import them with the
first option **off** to compare.

![The board's own top copper and drill files on user layers](/screenshots/gerber-reference-board.png)

## How files are drawn

- Every dark shape of a file (flashes, lines, arcs, regions) is united into
  areas; clear shapes cut holes into them, in the order of the file. Each
  area becomes one filled polygon.
- Drill files become one filled circle per hole (slots become their
  outline).
- Curves are approximated within 2 µm.

## Afterwards

The reference layers appear as **User 1**, **User 2**, … in the layer list:
hide them, change their opacity, or delete their shapes when you no longer
need them. The import is one step to undo.

Scripts use the engine methods
[`pcb.gerberPreview`](../reference/engine-api#pcb-gerberpreview) and
[`pcb.importGerbers`](../reference/engine-api#pcb-importgerbers).
