# Drawing symbols

A symbol is what a part looks like on a schematic: **pins** that wires
connect to, and **graphics and texts** that show what the part is. Symbols
can have several **units** (the four gates of a 74HC00) and an **alternate
body** per unit (the De Morgan form of a gate). Open the
[library editor](./library-editor), choose **Symbols** and click a symbol or
**New**.

![Drawing a level shifter: pins from a pasted list on the left, pins from the pin tool on the right](/screenshots/libedit-symbol.png)

## Drawing tools

| Tool            | Key            | What it does                                                                                                                         |
| --------------- | -------------- | ------------------------------------------------------------------------------------------------------------------------------------ |
| Select and move | <kbd>Esc</kbd> | Click to select, <kbd>Shift</kbd>/<kbd>Ctrl</kbd>-click to add, drag a box to select several, drag to move.                          |
| Pin             | <kbd>P</kbd>   | Each click places a [pin](#pins). Numbers count up by themselves.                                                                    |
| Line            | <kbd>L</kbd>   | Click the start, then every next point; each click ends one line and starts the next. Double-click or <kbd>Esc</kbd> ends the chain. |
| Rectangle       | <kbd>B</kbd>   | Click two opposite corners. The usual body of an IC.                                                                                 |
| Circle          | <kbd>C</kbd>   | Click the centre, then a point on the circle.                                                                                        |
| Polyline        | <kbd>K</kbd>   | Click the points; <kbd>Enter</kbd>, a double-click or a second click on the last point ends it. Triangles of op amps and diodes.     |
| Text            | <kbd>T</kbd>   | Click where the text goes and type it.                                                                                               |

While a tool draws, a dashed blue preview follows the cursor and a small
blue cross marks the grid point where the next click lands. New graphics use
the default line width of schematics (shown as `0`); change the width, the
points and the fill in the side panel.

Selected items can be moved by dragging, turned with <kbd>R</kbd>, mirrored
with <kbd>Y</kbd> and <kbd>X</kbd>, copied with <kbd>Ctrl</kbd>+<kbd>D</kbd>
and deleted with <kbd>Del</kbd>, also through the toolbar buttons.

## Pins

A pin has a **number** (`1`, `A3`, which pad it connects to), a **name**
(`VCC`, `~{RESET}`, what it does), an **electrical type** for the
[electrical rules check](./erc), a **graphic style**, a **direction** and a
**length**. The end of the pin away from the body, marked by its position,
is where wires connect.

With the pin tool active, the options row shows the settings for the next
pin:

![Settings for new pins: direction, type and length](/screenshots/libedit-pin-options.png)

- **Direction**: where the pin points from its connection end towards the
  body. Pins on the left side of a body point **right**, pins on the right
  side point **left**. <kbd>R</kbd> turns the next pin by 90°.
- **Electrical type**: input, output, bidirectional, tri-state, passive, not
  internally connected, unspecified, power input, power output, open
  collector, open emitter, not connected.
- **Length**: 2.54 mm (100 mil), 3.81 mm, 5.08 mm or 7.62 mm.

Each click places one pin with the next free number. Pins should lie on the
1.27 mm grid so that wires can reach them; the
[library checks](./library-checks) warn about pins off the grid.

### Graphic styles

| Style              | Meaning                                          |
| ------------------ | ------------------------------------------------ |
| Line               | A plain pin                                      |
| Inverted           | A bubble at the body: active low                 |
| Clock              | A triangle at the body: clock input              |
| Inverted clock     | Bubble and triangle                              |
| Input low          | A low-active input in IEEE notation              |
| Clock low          | A low-active clock in IEEE notation              |
| Output low         | A low-active output in IEEE notation             |
| Falling edge clock | A clock that triggers on the falling edge        |
| Non-logic          | A cross: an analog or otherwise non-logic signal |

## The pin table

The **Pins** tab below the drawing lists every pin of the shown body, one
row per pin. Every cell can be edited in place: type a new number or name
and press <kbd>Enter</kbd> (or just leave the cell), or choose a type, style
or direction from the lists.

![The pin table with two rows selected for a bulk edit](/screenshots/libedit-pin-table.png)

**Selecting rows.** The table and the drawing share one selection: click a
row (outside its fields) to select its pin in the drawing, and pins you
click in the drawing are ticked in the table.

- Click selects one row, <kbd>Ctrl</kbd>/<kbd>⌘</kbd>-click adds or removes a
  row, <kbd>Shift</kbd>-click selects a range.
- The check box in the header selects all rows or none.

**Bulk edits.** When you change the **type**, **style**, **direction**,
**length** or **hidden** state in a selected row, the change applies to every
selected row at once: select all address pins and make them inputs in one
step. Numbers, names and positions always change only the row you edit.

**Renumbering.** With rows selected, the bar above the table shows
**Start at**, a step and **Renumber**. The selected pins get consecutive
numbers from top to bottom: `1, 2, 3 …`, or with a letter prefix
`A1, A2, A3 …`. A step of `2` numbers every other pad of a two-row header
(`1, 3, 5 …`).

**Deleting.** **Delete** removes the selected pins.

### Pasting a pin list

The fastest way to a large symbol is the pin table of the datasheet. Click
**Paste pin list** and paste one pin per line:

```text
1;VCCA;W
2;A1;B
3;A2;B
4;GND;W
```

![Pasting a pin list](/screenshots/libedit-paste-pins.png)

- Fields are separated by tabs (as copied from a spreadsheet or a PDF
  table), semicolons or commas: **number**, **name**, **type**.
- An empty number gets the next free one; a line with only a word is a name.
- A header line starting with `Pin`, `No`, `Number` or `#` is skipped.
- The type may be written out (`input`, `power in`, `open collector`,
  `tristate`, `I/O`) or as KiCad's letter: `I` input, `O` output,
  `B` bidirectional, `T` tri-state, `P` passive, `U` unspecified,
  `W` power input, `w` power output, `C` open collector, `E` open emitter,
  `N` not connected, `F` free. Pins without a type become passive; unknown
  types are reported line by line.

The pins are added in a column at the left of the existing pins, 2.54 mm
apart and pointing right. Move them into place afterwards, for example
select half of them, mirror them with <kbd>Y</kbd> and drag them to the right
side.

## Units and alternate bodies

The options row shows the **unit** and the **body style** of the drawing:

![Unit A with its De Morgan body](/screenshots/libedit-units.png)

- **Unit**: a symbol with several units (gates, op amps, the power unit of
  a logic chip) shows one unit at a time; choose it in the list. Units are
  called A, B, C … unless you name them.
- **+** adds a copy of the shown unit: the same drawing, and its pins get
  the next free numbers, ready to renumber. The trash button removes the
  shown unit (a symbol keeps at least one).
- **Normal** / **De Morgan**: the body style. **+ De Morgan** creates an
  alternate body for the shown unit as a copy of the normal one; then change
  the graphics (an AND gate drawn as an OR gate with inverted inputs). The
  alternate body must have the same pin numbers as the normal one; the
  [checks](./library-checks) report differences. The **×** next to the
  switch removes the alternate body.

On a sheet, every placed symbol remembers its unit and body style. Symbols
imported from KiCad keep their alternate bodies (`body style 2`).

## Properties

The side panel shows the selected item at the top and the symbol's
properties below.

![A selected pin in the side panel](/screenshots/libedit-pin-properties.png)

**Selected pin**: number, name, type, style, direction, length, position and
**Hidden**. **Selected graphic**: start and end point (or centre and radius),
line width and **Fill**: _None_, _Solid_ (the line colour) or _Body colour_
(the light fill of IC bodies). **Selected text**: text, position, size and
rotation. With several items selected, the panel says so; use the table for
bulk edits.

**Symbol properties:**

| Property                                 | Meaning                                                                                                                                                              |
| ---------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Name**                                 | Change it with **Rename**; references follow (see [the library editor](./library-editor#creating-copying-renaming-and-deleting)).                                    |
| **Description**                          | Shown in the symbol chooser and searched. The [checks](./library-checks) note symbols without one.                                                                   |
| **Keywords**                             | Extra search words (`opamp amplifier`).                                                                                                                              |
| **Designator prefix**                    | `R`, `C`, `U`, `Q` … [Annotation](./schematic-editor#annotation) numbers placed parts with it. `#` starts the prefix of symbols that are not parts (`#PWR`, `#FLG`). |
| **Default value**                        | The value of a newly placed symbol.                                                                                                                                  |
| **Default footprint**                    | The footprint a placed symbol gets; suggestions come from the project's footprints.                                                                                  |
| **Datasheet**                            | A link or file name.                                                                                                                                                 |
| **Power symbol**                         | The symbol is a power port such as `GND`: its power-input pin joins the net named like the placed symbol's value, everywhere in the project.                         |
| **Hide pin numbers**, **Hide pin names** | For passives, whose pin numbers say nothing on the sheet.                                                                                                            |
| **Pin name offset**                      | Distance of pin names from the body edge. `0` puts names above the pin line, outside the body.                                                                       |
| **Fields**                               | Extra fields that every placed symbol inherits (`Tolerance`, `Voltage`). Add one by name, change its value, remove it with the trash button.                         |

The line below the name tells you where the symbol is used: on sheets, by
components.
