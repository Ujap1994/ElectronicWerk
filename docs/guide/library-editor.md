# The library editor

Every project has its own **project library** with the symbols, footprints
and components that its sheets and boards use. The library editor is where
you draw and maintain them. It has three parts that share one window:

- the [symbol editor](./symbol-editor): pins, body graphics, units and
  De Morgan bodies,
- the [footprint editor](./footprint-editor): pads, silkscreen, fabrication
  and courtyard outlines, 3D models, plus the
  [footprint wizard](./footprint-wizard) for standard packages,
- the [component editor](./components): a symbol, its footprint and the
  order data of a real part, ready to place in one go.

[Library checks](./library-checks) watch over all three while you work.

![The library editor with the starter symbols of a new project](/screenshots/libedit-library.png)

## Opening a library

Libraries are documents of the project, listed under **Libraries** in the
[project tree](./project-tree). Double-click a library, or select it and
click **Open** in the details panel, or choose **Open** in its context menu
(<kbd>…</kbd>). The library opens in a tab next to your sheets and boards, so
you can switch between a sheet and the symbol you are fixing with one click.

A new project already contains a library called **Project library** with the
starter symbols (`R`, `C`, `C_Polarized`, `L`, `D`, `LED`, `Q_NPN_BCE`,
`Conn_01x02`, `Conn_01x03`, `SW_Push`, `PWR_FLAG`). Symbols and footprints
that you take from the [base library](./library) are copied into the project
library as well, so you can adapt them here without touching the base
library. Add more libraries with **Add library** in the project tree.

## The window at a glance

| Area             | What it does                                                                                                                                                                                                                                               |
| ---------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Item list**    | Left column. **Symbols**, **Footprints** and **Components** with the number of items, the buttons **New**, **Wizard** (footprints), **Duplicate** and **Delete**, a search field and the items. The chosen item is highlighted.                            |
| **Toolbar**      | Above the drawing: fit to window, the drawing tools, **Rotate**, **Mirror left to right**, **Mirror top to bottom**, **Duplicate** and **Delete** for the selection.                                                                                       |
| **Options**      | The second row: the grid, the layer for new footprint graphics, the unit and body style of a symbol, and the settings for new pins while the pin tool is active.                                                                                           |
| **Drawing area** | The item in its own coordinates. The small cross is the item's **origin**; symbols are placed and footprints are positioned by it. The cursor snaps to the grid.                                                                                           |
| **Status line**  | The cursor position, a hint for the active tool and the zoom.                                                                                                                                                                                              |
| **Tables**       | Below the drawing: the [pin table](./symbol-editor#the-pin-table) or the [pad table](./footprint-editor#the-pad-table), and the **Checks** tab with the [findings](./library-checks) of the item. The tab names show how many rows and findings there are. |
| **Side panel**   | Right column: the selected pin, pad, graphic or text, then the properties of the whole item.                                                                                                                                                               |

## Finding items

Type into **Search by name or description**: every item whose name or
description contains the text stays in the list. Each entry shows a short
summary below its name: the number of pins and units, `De Morgan` when a unit
has an alternate body, `Power` for power symbols, the number of pads of a
footprint, or the symbol, footprint and part number of a component.

A coloured number next to a name counts the errors (red) and warnings
(yellow) of the [library checks](./library-checks). Notes do not show up
here.

## Creating, copying, renaming and deleting

| Button        | What it does                                                                                                                                                                                                                                                   |
| ------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **New**       | Asks for a name and creates an empty symbol (prefix `U`), an empty footprint, or a component without symbol. Names must be unique per kind within the library.                                                                                                 |
| **Wizard**    | Footprints only: opens the [footprint wizard](./footprint-wizard).                                                                                                                                                                                             |
| **Duplicate** | Copies the chosen item under a new name (the dialog suggests `<name>_copy`). Use it to start a variant: a 16-pin version of an 8-pin footprint, a symbol with an extra pin.                                                                                    |
| **Rename**    | In the side panel, next to the item's name. Every reference in the project follows: placed symbols on sheets, footprints on boards, the default footprint of symbols and the symbol and footprint of components. A message tells you how many objects changed. |
| **Delete**    | Asks first. If the item is still in use, the dialog says where. Placed parts keep the name, so restoring the item (undo, or a new item of the same name) brings their drawing back.                                                                            |

![Naming a new symbol](/screenshots/libedit-new.png)

## Undo and saving

Every change in the library editor is one step in the project's
[undo history](./undo-and-recovery): drawing a pin, a bulk edit in a table,
a changed property, a renamed item. **Undo** and **Redo** in the project
toolbar (<kbd>Ctrl</kbd>+<kbd>Z</kbd> / <kbd>⌘</kbd>+<kbd>Z</kbd>) name the
step, for example _Undo Edit symbol_. Like everything else, library changes
are kept in the autosave journal and written to the library file when you
save the project.

Changes show up in open sheets and boards at once: a moved pin moves on every
placed symbol, and a renamed footprint is renamed on the board.

## Grid and units

The **Grid** list in the options row sets the snapping grid of the drawing:

- symbols: 0.254 mm, 0.635 mm, **1.27 mm** (the default, 50 mil) and 2.54 mm,
- footprints: 0.01 mm to 1.27 mm, **0.05 mm** by default.

Lengths in tables and fields use the units of the project (see
[project properties](./project-tree)): millimetres, mil or inch. Type a plain
number in those units; a comma works as decimal separator too.

## Keyboard

Click the drawing area first so it has the keyboard focus.

| Key                                                      | Effect                                                        |
| -------------------------------------------------------- | ------------------------------------------------------------- |
| <kbd>Esc</kbd>                                           | Ends the current shape; pressed again, back to **Select**     |
| <kbd>P</kbd>                                             | Pin tool (symbols) or pad tool (footprints)                   |
| <kbd>L</kbd>                                             | Line                                                          |
| <kbd>B</kbd>                                             | Rectangle                                                     |
| <kbd>C</kbd>                                             | Circle                                                        |
| <kbd>K</kbd>                                             | Polyline                                                      |
| <kbd>T</kbd>                                             | Text                                                          |
| <kbd>R</kbd>                                             | Rotates the selection (with the pin tool: turns the next pin) |
| <kbd>Shift</kbd>+<kbd>R</kbd>                            | Rotates the other way                                         |
| <kbd>Y</kbd> / <kbd>X</kbd>                              | Mirrors the selection left to right / top to bottom           |
| <kbd>Ctrl</kbd>+<kbd>D</kbd> / <kbd>⌘</kbd>+<kbd>D</kbd> | Duplicates the selection                                      |
| <kbd>Del</kbd> / <kbd>Backspace</kbd>                    | Deletes the selection                                         |
| <kbd>Enter</kbd>                                         | Finishes a polyline                                           |
| <kbd>Home</kbd>                                          | Fits the item into the window                                 |

Zooming and panning work like in the [viewer](./viewer): mouse wheel or
pinch to zoom, middle mouse button or <kbd>Space</kbd>+drag to pan.
