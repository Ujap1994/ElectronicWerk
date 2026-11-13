# Project tree and properties

When a project is open, the window shows three areas:

1. the **toolbar** with Save, Undo, Redo, Check project, Archive and Close,
2. the **project tree** on the left, and
3. the **details** of the selected item on the right.

![The project workspace](/screenshots/workspace-light.png)

The status bar at the bottom shows the project folder and, while there are
unsaved changes, **Unsaved changes**.

![The status bar with an open project](/screenshots/status-bar-project.png)

## The project tree

The tree is the map of your project, like the _Projects_ panel in Altium
Designer.

![The schematic hierarchy in the project tree](/screenshots/tree-hierarchy.png)

| Group               | Contents                                                                                                                                                              |
| ------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Schematics**      | The sheet hierarchy. The **top sheet** comes first; sheets placed on it are shown below it, and so on. Sheets that are not placed anywhere are marked **Not placed**. |
| **Boards**          | The board of the project.                                                                                                                                             |
| **Libraries**       | The project library with the symbols and footprints the design uses.                                                                                                  |
| **Design rules**    | Manufacturing limits and net classes.                                                                                                                                 |
| **Output jobs**     | Which manufacturing files to generate.                                                                                                                                |
| **Other files**     | Every other file in the project folder (notes, datasheets, `.gitignore`).                                                                                             |
| **Generated files** | Files produced by output jobs.                                                                                                                                        |

The number next to a group is the number of documents in it. A dot **●** marks
documents with unsaved changes.

### Actions

Right-click an item, or click **⋯** at the end of its row, to see what you can
do with it:

![The context menu of a sheet](/screenshots/tree-menu.png)

| Item              | Actions                                                                              |
| ----------------- | ------------------------------------------------------------------------------------ |
| Project           | Add sheet, Add board, Add library, Add output job, Show in folder                    |
| A group           | Add a document of that kind (one board and one rule set per project), Show in folder |
| A schematic sheet | **Open**, **Add sub-sheet** (placed on this sheet), Rename, Show in folder, Remove   |
| A board           | **Open**, Rename, Show in folder, Remove                                             |
| Other documents   | Rename, Show in folder, Remove                                                       |
| A file            | Show in folder                                                                       |

New documents get a default name, which is selected right away so you can
type the real name:

![Renaming a new sheet in place](/screenshots/tree-rename.png)

Press <kbd>Enter</kbd> to confirm or <kbd>Esc</kbd> to keep the old name.

**Add sub-sheet** creates a new schematic document and places a sheet symbol
for it on the selected sheet, so it appears below that sheet in the hierarchy.

**Remove** asks first. Removing a sheet also removes the sheet symbols that
placed it. The file is deleted when you save; until then, **Undo** brings the
document back.

![Confirming the removal of a document](/screenshots/remove-dialog.png)

### Keyboard

| Key                              | Action                                            |
| -------------------------------- | ------------------------------------------------- |
| <kbd>↑</kbd> / <kbd>↓</kbd>      | Select the previous / next item                   |
| <kbd>→</kbd>                     | Expand the item, or go to its first child         |
| <kbd>←</kbd>                     | Collapse the item, or go to its parent            |
| <kbd>Home</kbd> / <kbd>End</kbd> | First / last item                                 |
| <kbd>Enter</kbd>                 | Open the selected sheet or board                  |
| <kbd>F2</kbd>                    | Rename the selected document                      |
| <kbd>Delete</kbd>                | Remove the selected document (after confirmation) |
| Context menu key                 | Open the actions of the selected item             |

Double-clicking a sheet or the board opens it in the
[2D view](./viewer); double-clicking a group expands or collapses it.

## Project properties

Select the project (the top item of the tree), or choose **File → Project
Properties**, to see and change the project's properties.

![Project properties](/screenshots/project-overview.png)

| Field       | Meaning                                                                                |
| ----------- | -------------------------------------------------------------------------------------- |
| Name        | The project name (`${PROJECT_NAME}`). Renaming the project does not rename the folder. |
| Revision    | For example `A` or `1.2` (`${REVISION}`).                                              |
| Description | Free text.                                                                             |
| Units       | Millimetres, mils or inches, used for every length shown in ElectronicWerk.            |
| Location    | The project file on disk; **Show in folder** reveals it in your file manager.          |

Changes are applied when you press <kbd>Enter</kbd> or leave the field, and
each change can be undone.

The **Overview** shows key figures: sheets, symbols, footprints, nets, track
segments, vias, copper layers, board size and the editor grids.

### Text variables

Text variables keep title blocks and texts consistent. Write `${NAME}` in a
text, and ElectronicWerk replaces it with the value of the variable:

![Text variables](/screenshots/project-variables.png)

| Built-in variable | Value                                        |
| ----------------- | -------------------------------------------- |
| `PROJECT_NAME`    | The project name                             |
| `REVISION`        | The project revision                         |
| `CURRENT_DATE`    | Today's date (`YYYY-MM-DD`)                  |
| `SHEET_NAME`      | The name of the sheet the text is on         |
| `FILENAME`        | The file name of the document the text is in |

Add your own variables in the last row of the table (the name is converted to
capital letters), change a value directly in the table, and remove a variable
with the bin icon. Names consist of letters, digits and `_` and do not start
with a digit. Built-in names cannot be used for your own variables. A variable
may refer to other variables, for example `${AUTHOR} (${PROJECT_NAME})`.

## Document details

Select a document in the tree to see its details. Every document shows its
**Name** (which you can change) and its **File** inside the project folder.

### Schematic sheets

![Details of a schematic sheet](/screenshots/details-schematic.png)

- **Paper:** the format (A5 to A0, Letter, Legal, Tabloid or custom) and the
  orientation. The resulting size is shown below.
- **Title block:** title, date, revision and company. Variables are allowed;
  a preview shows the resolved text. By default the title is `${SHEET_NAME}`,
  the date `${CURRENT_DATE}` and the revision `${REVISION}`.
- Counts of symbols, wires, labels and placed sub-sheets.

Double-click a sheet to open it in the [schematic editor](./schematic-editor).

### Boards

![Details of a board with its stack-up](/screenshots/details-pcb.png)

The board details show the outline size, the number of copper layers, the
thickness, the number of footprints, track segments, vias, zones and nets, and
the **layer stack-up** from top to bottom: silkscreen, solder mask, copper and
dielectric (core or prepreg) layers with material and thickness.

Double-click a board to open it in the [layout editor](./layout-editor);
**3D view** shows it in the [3D view](./view-3d).

### Design rules

![Design rules](/screenshots/details-rules.png)

- **Manufacturing limits:** minimum clearance, track width, via diameter and
  drill, annular ring, hole size, hole-to-hole distance, copper-to-edge
  distance and silkscreen line width. The defaults suit common low-cost board
  houses.
- **Net classes:** clearance, track width, via diameter and drill per class.
  New projects have a `Default` and a `Power` class.

The [design rules](./design-rules) are edited from the layout editor.

### Output jobs

![An output job](/screenshots/details-output-job.png)

An output job lists the manufacturing files to generate (Gerber X2, Excellon
drill, drill map, bill of materials, pick-and-place, IPC-D-356 netlist, PDF
prints and a ZIP package for the board maker) and the output folder
(`outputs/generated` by default). **Open** (or a double-click in the tree)
opens it in the [output job editor](./outputs), where **Generate all** writes
the files.
