# Drawing schematics

ElectronicWerk's schematic editor works in the same 2D view you use to look at
a sheet: open a sheet and start drawing. Every action is one step in the
[undo history](./undo-and-recovery), every change is saved in the autosave
journal, and the [electrical rules check](./erc) tells you what is still
missing.

![The schematic editor with the tool palette on the left](/screenshots/schematic-editor.png)

## The editor at a glance

| Area             | What it does                                                                                                                                                                                  |
| ---------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Tool palette** | The drawing tools in a column at the left edge of the drawing. The pressed tool is active. Hover a tool to see its name, its key and a short explanation.                                     |
| **View toolbar** | Zoom and selection tools (see [Viewing sheets and boards](./viewer)), then **Rotate**, **Mirror**, **Delete**, **Annotate**, the **check**, **Find**, **Export**, **Grid** and the **Guide**. |
| **Drawing area** | The sheet. The cursor snaps to the grid; drawing tools show a small blue cross where the next point lands, and a dashed blue preview of what you are drawing.                                 |
| **Status line**  | The cursor position, the object under the cursor, a hint for the active tool and, while a net is highlighted, its name.                                                                       |
| **Properties**   | The selected object. Symbols, labels, power ports, texts and sheets can be edited right there (see [Editing properties](#editing-properties)).                                                |

A new project starts with an empty top sheet. Double-click the sheet in the
[project tree](./project-tree) to open it. On an empty sheet,
[the guide](#the-guide-for-a-first-schematic) explains the first steps.

## Tools

![The tool palette](/screenshots/schematic-tools.png){width=48}

| Tool               | Key                           | What it does                                                                                    |
| ------------------ | ----------------------------- | ----------------------------------------------------------------------------------------------- |
| Select and move    | <kbd>Esc</kbd>                | Select objects, drag them, edit them. The default tool.                                         |
| Add symbol         | <kbd>A</kbd>                  | Opens the [symbol chooser](#placing-symbols) and places the chosen part.                        |
| Wire               | <kbd>W</kbd>                  | Draws [wires](#drawing-wires) that connect pins.                                                |
| Bus                | <kbd>B</kbd>                  | Draws a bus line. Buses are drawn only for now; bus members follow in a later version.          |
| Net label          | <kbd>L</kbd>                  | Names a wire. Wires with the same label on one sheet are connected.                             |
| Global label       | <kbd>Shift</kbd>+<kbd>L</kbd> | Connects wires with the same name on every sheet.                                               |
| Hierarchical label | <kbd>H</kbd>                  | Connects to the pin of the same name on the sheet symbol one level up.                          |
| Power port         | <kbd>P</kbd>                  | Connects to a supply net such as `GND` or `+5V` everywhere in the project.                      |
| No-connect flag    | <kbd>Q</kbd>                  | Marks a pin that is left open on purpose.                                                       |
| Junction           | <kbd>J</kbd>                  | Connects wires that cross. You rarely need it: tees get a junction automatically.               |
| Text               | <kbd>T</kbd>                  | A note on the sheet. Text variables such as `${REVISION}` are replaced when the sheet is drawn. |
| Sheet              | <kbd>S</kbd>                  | Draws a [sheet symbol](#hierarchical-sheets) for a new sub-sheet.                               |
| Highlight net      | <kbd>N</kbd>                  | Click a wire, label or pin to [highlight its net](#highlighting-a-net) on every sheet.          |

Press <kbd>Esc</kbd> to end the current action; press it again to go back to
**Select and move**. Click the drawing area first so it has the keyboard
focus.

## Placing symbols

Press <kbd>A</kbd> or click the chip button. The **Add symbol** dialog lists the
symbols of the project library and, once you type two letters, matching
symbols of the [base library](./library).

![Choosing a resistor](/screenshots/schematic-chooser.png)

- Type to filter: every word must occur in the name, description or keywords
  (`res`, `cap 100n`, `led`).
- Click a symbol to see a preview, double-click it or press <kbd>Enter</kbd> to
  place it.
- A symbol of the base library is copied into the project library first
  (**Add to the project and place**), together with its default footprint.
  This is one undo step of its own.

Every new project brings **starter symbols**, so you can draw a first circuit
without setting up the base library: `R`, `C`, `C_Polarized`, `L`, `D`, `LED`,
`Q_NPN_BCE`, `Conn_01x02`, `Conn_01x03`, `SW_Push` and `PWR_FLAG`. Their pins
lie on the 1.27 mm (50 mil) grid, like KiCad's.

After you choose a symbol, it hangs at the cursor as a blue outline:

![Placing a resistor](/screenshots/schematic-place.png)

| Key or action  | Effect                                                        |
| -------------- | ------------------------------------------------------------- |
| Click          | Places the symbol; you can place the next one right away      |
| <kbd>R</kbd>   | Rotates by 90° counterclockwise (<kbd>Shift</kbd>: clockwise) |
| <kbd>Y</kbd>   | Mirrors left to right                                         |
| <kbd>X</kbd>   | Mirrors top to bottom                                         |
| <kbd>Esc</kbd> | Stops placing                                                 |

A placed symbol gets the next free designator at once (`R1`, `R2`, … per
prefix, across all sheets). Power symbols and flags count separately (`#PWR1`,
`#FLG1`). You can number everything again later with
[Annotate](#annotation).

## Drawing wires

Press <kbd>W</kbd>, then:

1. Click where the wire starts, usually on a pin end.
2. Move the mouse: the preview runs horizontally first, then vertically.
   Press <kbd>/</kbd> to bend the other way round.
3. Click to fix a corner and go on.
4. Click on a pin, another wire or a label to end the wire there. You can also
   double-click, press <kbd>Enter</kbd>, or click the last point again.

![Drawing a wire from a resistor to a capacitor](/screenshots/schematic-wire.png)

<kbd>Backspace</kbd> removes the last corner, <kbd>Esc</kbd> throws the wire
away. Straight runs are merged: three points on a line give one wire.

Two wires are connected where an end of one meets the other, at its end or in
its middle. Wires that only cross are **not** connected. ElectronicWerk adds a
junction dot wherever three or more connections meet (a wire passing through
counts twice) and removes it when it is no longer needed, for example after a
wire is deleted.

::: tip Pins connect only when they meet exactly
A wire end must lie exactly on the pin end. Keep the
[grid](#grid-and-snapping) at 1.27 mm, the grid of nearly every library, and
let the cursor snap.
:::

## Labels, power ports and flags

Choose a label tool and click where the label connects: on a wire end, on a
wire, or on a pin end. A small field asks for the name:

![Naming a net](/screenshots/schematic-label.png)

| Object             | Connects                                                                                   |
| ------------------ | ------------------------------------------------------------------------------------------ |
| Net label          | every wire with the same label **on this sheet** (and in each placement of a reused sheet) |
| Global label       | every wire with the same global label **on every sheet**                                   |
| Hierarchical label | the pin with the same name **on the sheet symbol one level up**                            |
| Power port         | every power port and power symbol with the same net name in the project (`GND`, `+3V3`, …) |

The name of the next label counts up when it ends in a number: after `D0` the
field suggests `D1`. Power ports suggest common supply names (`GND`, `VCC`,
`+5V`, `+3V3`, …); names that start with `GND`, `VSS` or `0V` get the ground
symbol, all others a bar. Press <kbd>R</kbd> before clicking to turn the next
label or power port.

**No-connect flags** (<kbd>Q</kbd>) mark pins that stay open on purpose, so the
check does not report them. Put them on the pin end.

**Junctions** (<kbd>J</kbd>) connect two wires that cross. Junctions you place
yourself stay where they are.

## Selecting, moving and dragging

Click an object to select it. A click on a pin or on a designator selects the
whole symbol. Hold <kbd>Shift</kbd> to add objects, <kbd>Ctrl</kbd>/<kbd>⌘</kbd>
to toggle them, or drag on empty space for a box selection (see
[Selecting objects](./viewer#selecting-objects)).

**Drag** a selected object to move it. Wires that end on its pins follow
("rubber-banding"): horizontal and vertical wires stay straight and get a new
corner, like in KiCad, and labels, junctions, flags and power ports sitting on
those pins move along. Hold <kbd>Alt</kbd> when you release the mouse to leave
the wires where they are.

![Dragging a capacitor: the wire gets a new corner](/screenshots/schematic-drag.png)

From the keyboard:

| Key                                          | Action                                                                      |
| -------------------------------------------- | --------------------------------------------------------------------------- |
| <kbd>G</kbd>                                 | Drag the selection with its wires; it follows the cursor, a click places it |
| <kbd>M</kbd>                                 | Move the selection without its wires                                        |
| <kbd>R</kbd> (<kbd>Shift</kbd>+<kbd>R</kbd>) | Rotate counterclockwise (clockwise)                                         |
| <kbd>Y</kbd> / <kbd>X</kbd>                  | Mirror left to right / top to bottom                                        |
| <kbd>Delete</kbd> or <kbd>Backspace</kbd>    | Delete                                                                      |
| <kbd>Ctrl</kbd>/<kbd>⌘</kbd>+<kbd>C</kbd>    | Copy                                                                        |
| <kbd>Ctrl</kbd>/<kbd>⌘</kbd>+<kbd>X</kbd>    | Cut                                                                         |
| <kbd>Ctrl</kbd>/<kbd>⌘</kbd>+<kbd>V</kbd>    | Paste at the cursor                                                         |
| <kbd>Ctrl</kbd>/<kbd>⌘</kbd>+<kbd>D</kbd>    | Duplicate (a copy next to the original)                                     |

A single object turns about its anchor; several objects turn about their
common centre on the grid, so everything on the grid stays there. Moving
always goes in whole grid steps.

### Copy and paste across sheets

Copied objects stay on the clipboard while the project is open. Open another
sheet and press <kbd>Ctrl</kbd>/<kbd>⌘</kbd>+<kbd>V</kbd>: the copy appears with
its centre at the cursor. Pasted symbols keep their designator if it is still
free; otherwise they get the next free number. Pasted parts are new parts:
they are not linked to the footprints of the originals.

## Editing properties

Select one symbol, label, power port, text or sheet: the **Properties** panel
shows fields you can change. A change is saved when you press
<kbd>Enter</kbd> or leave the field, and it is one undo step.

![Editing the value of a capacitor](/screenshots/schematic-properties.png)

| Object     | Editable                                                                                                                            |
| ---------- | ----------------------------------------------------------------------------------------------------------------------------------- |
| Symbol     | Designator, value, footprint, parameters (add, change, remove), **Do not place**, **Leave out of the BOM**, **Leave off the board** |
| Label      | Name, kind (net, global or hierarchical label) and, for global and hierarchical labels, the direction shown on the label            |
| Power port | Net name and symbol (bar, arrow, circle, wave, ground, signal ground, earth)                                                        |
| Text       | The text (several lines)                                                                                                            |
| Sheet      | Name; **Add pins from the sheet**; **Open the sheet**                                                                               |

Double-click a label, power port, text or sheet to rename it right in the
drawing.

Parameters are free name–value pairs such as `MPN` or `Manufacturer`. The
[bill of materials](./schematic-exports#bill-of-materials) has columns for
`Manufacturer` and `MPN`.

## Hierarchical sheets

Large designs are split into sheets. A **sheet symbol** on a sheet stands for
a sub-sheet; its **sheet pins** connect to the **hierarchical labels** of the
same name inside the sub-sheet.

1. Press <kbd>S</kbd> and click two opposite corners. Name the new sheet. It
   appears in the project tree below its parent.
2. Open the sub-sheet (double-click it in the tree, or select the sheet symbol
   and click **Open the sheet**) and draw its circuit. Put a hierarchical label
   (<kbd>H</kbd>) on every signal that leaves the sheet.
3. Back on the parent, select the sheet symbol and click **Add pins from the
   sheet**. Every hierarchical label without a pin gets one on the left edge;
   drag wires to them.

A sheet can be placed more than once (copy and paste its sheet symbol). Each
placement is a separate copy of the circuit with its own nets and its own
designators (for example `R1` in the first placement and `R5` in the second).

The [check](./erc) reports hierarchical labels without a sheet pin and sheet
pins without a hierarchical label.

## Highlighting a net

Press <kbd>N</kbd> (or click the highlight tool) and click a wire, label or pin.
The whole net turns blue, on this sheet and on every other sheet you open,
and its name appears in the status line. Click the name there, or press
<kbd>Esc</kbd>, to clear the highlight. In the select tool, <kbd>`</kbd>
highlights the net under the cursor.

![The highlighted net LED_CTRL](/screenshots/schematic-net.png)

## Find and replace

Press <kbd>Ctrl</kbd>/<kbd>⌘</kbd>+<kbd>F</kbd> or click the magnifier. The search
covers every sheet: designators, values, footprints, parameters, labels,
power ports, texts and sheet names.

![Finding a net name](/screenshots/schematic-find.png)

- **Match case** and **Whole words** narrow the search (`SDA` with whole words
  finds `SDA` but not `SDA1`).
- Click a match to show it: the view opens its sheet, zooms to it and selects
  it.
- **Replace all** replaces the text in every match in one undo step. Labels,
  power ports, sheet names and designators are never emptied.

## Annotation

Designators are numbered as you place symbols. **Annotate** (the `#` button)
numbers them again or fills in the ones that end in `?` (for example after
importing symbols without numbers).

![The annotation dialog](/screenshots/schematic-annotate.png)

| Option                                   | Effect                                               |
| ---------------------------------------- | ---------------------------------------------------- |
| **All sheets** / **This sheet only**     | Which symbols are numbered                           |
| **Only unannotated ones**                | Keeps every existing number and fills in the rest    |
| **All of them, numbered again**          | Numbers every symbol from 1                          |
| **Row by row** / **Column by column**    | Order on each sheet: top to bottom, or left to right |
| **Start each sheet at its number × 100** | Sheet 1 gets `R101`, sheet 2 `R201`, …               |

Sheets are numbered in hierarchy order. Units of a multi-unit part (for
example the four gates of a 74HC00) share one designator: a new unit joins a
part with the same symbol and value on the same sheet that does not have this
unit yet.

## Grid and snapping

Click the dotted grid button for the grid assistant:

![The grid assistant](/screenshots/schematic-grid.png){width=360}

- **Snap to grid** (on by default): the cursor, symbols, wire corners and
  labels land on grid points. Turn it off for free placement of texts.
- **Grid size**: the grid of the project, 1.27 mm (50 mil) by default. KiCad,
  and the starter and base libraries, put pins on this grid. The grid starts
  at the top-left corner of the page, so imported KiCad drawings stay on it.
- The assistant counts wire ends that are not on the grid and offers **Snap
  everything to the grid**, which moves wire ends, symbols, labels and other
  objects of this sheet to the nearest grid point (one undo step).

## The guide for a first schematic

On an empty sheet, a small guide opens next to the tool palette. It has five
steps that tick themselves off as you work: place two parts, connect them,
add power, number the parts and run the check. Close it with **×**; the
lightbulb button brings it back.

![The guide on an empty sheet (dark theme)](/screenshots/schematic-guide-dark.png)

Every tool also explains itself: hover a tool to see its key and what it
does, and read the status line for the next step of the active tool.

## Commands in the command palette

The [command palette](./command-palette) offers the schematic commands while a
sheet is open: **Schematic: add symbol**, **… run the electrical rules check**,
**… annotate**, **… find and replace**, **… export**, **… grid and snapping** and
**… show the guide**.

## See also

- [Electrical rules check](./erc)
- [Exports: netlists, bill of materials, SVG and PDF](./schematic-exports)
- [Keyboard shortcuts](./keyboard-shortcuts#in-the-schematic-editor)
