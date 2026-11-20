# Parts from datasheets

A part that is in no library does not have to be drawn by hand. The **part
generator** reads the part's PDF datasheet and makes everything a part needs:

- the **schematic symbol**, with the pins grouped by function,
- the **footprint**, with IPC-7351 lands calculated from the package
  dimensions of the datasheet,
- a **3D model** as a STEP file (body and terminals in their real sizes),
  already assigned to the footprint,
- the **component** that joins them, with manufacturer, part number and the
  link to the datasheet.

Everything read from the datasheet is shown next to the datasheet itself
before anything is made. Every value has a button that shows the place on
the page it was read from, so you can check it at a glance.

The datasheet is read on your computer; nothing is sent anywhere. For
datasheets the generator cannot read well, your own AI agent can help
([below](#help-from-your-ai-agent)) — no API key is stored in
ElectronicWerk.

![The part generator: the datasheet on the left, the part read from it on the right](/screenshots/partgen-part.png)

## Opening a datasheet

Open the generator in one of these ways while a project is open:

- In the [library editor](./library-editor), click **Create from a
  datasheet** above the list of symbols, footprints or components. The part
  goes into this library.
- In the [command palette](./command-palette), run **Generate a part from a
  datasheet**.
- Drag a PDF file from your file manager onto the ElectronicWerk window.

![Create from a datasheet in the library editor](/screenshots/partgen-entry.png)

Click **Open datasheet…** and choose the PDF file. Reading takes a moment
for long datasheets (a 650-page microcontroller datasheet takes about half a
second); then the generator shows four steps on the right and the datasheet
on the left.

![Choosing the datasheet](/screenshots/partgen-start.png)

### The datasheet view

The left side shows the datasheet one page at a time. Use the arrows to
turn pages and **Fit the width**, **−** and **+** to zoom. Whenever you
click a page button such as **p. 4** next to a value, the view turns to
that page and frames the place the value came from.

Datasheets protected against copying (a password only for copying and
printing, as many manufacturers use) open normally. Datasheets that need a
password to open cannot be read. A page that is a scanned image without
text is shown, but its values cannot be read; enter them by hand or ask
your AI agent.

## Step 1: the part

The first step shows what was found about the part itself:

| Field                        | Where it comes from                                                                                                                                                                                          |
| ---------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **Part name**                | The part number that the title, the first page and the page headers name most often. For a datasheet that covers a family (for example LM158, LM258, LM358), the file name decides: `lm358.pdf` gives LM358. |
| **Manufacturer**             | The author of the PDF file, or a known manufacturer name on the first pages.                                                                                                                                 |
| **Manufacturer part number** | The part name, until you enter the ordering code.                                                                                                                                                            |
| **Description**              | The subject of the PDF file or its title without the part numbers.                                                                                                                                           |

Change any of them; the names go into the symbol, footprint and component.

**Notes** list what the recognition wants you to know, for example that the
pins were read from a pinout drawing rather than a table, or that a page is
a scanned image.

## Step 2: the package

Most parts come in several packages (SOIC-8, MSOP-8, DFN-8 …). The
generator lists every package it found with its **family** and the number
of pins it has. **Choose the package you want** — pins that differ between
packages (an exposed pad, other pad numbers) follow your choice.

![The packages and the dimensions of SOIC-8, with the D value framed in the datasheet](/screenshots/partgen-package.png)

### Dimensions

The table shows the package dimensions in millimetres with their minimum,
nominal and maximum values, using the JEDEC symbols most datasheets use:

| Symbol     | Meaning                                                 |
| ---------- | ------------------------------------------------------- |
| `e`        | Pitch (distance between pin centres)                    |
| `D`        | Body length along the pin rows (QFP: lead span along y) |
| `E`        | Lead span from tip to tip (QFN, DFN, BGA: body width)   |
| `E1`, `D1` | Body width and length (without the leads)               |
| `D2`, `E2` | Size of the exposed pad under the body                  |
| `b`        | Lead, terminal or ball width                            |
| `b1`, `b2` | Shoulder width of DIP leads, width of a tab             |
| `L`        | Length of the lead foot or terminal on the board        |
| `A`, `A1`  | Height of the package and its stand-off above the board |
| `c`        | Lead thickness                                          |

Each value is either read from a **dimension table of the datasheet** —
then it has a page button that frames its row — or it is the **standard**
value of that package (JEDEC outlines and typical manufacturer values),
marked _standard_. Check standard values against the package drawing of
the datasheet; **Show the package drawing** turns the view to the table the
dimensions came from.

How the tables are read:

- Tables with **millimetres and inches** side by side: the millimetre
  columns are used. Tables only in inches are converted.
- **TYP** and **BSC** values count as nominal values.
- Manufacturers that use **their own letters** (A, B, C, … instead of the
  JEDEC symbols) are matched by value against the standard dimensions of the
  package; NXP's `HE`, `bp` and `Lp` are understood too.

In the **beginner mode** (see [Settings](./settings)) the table shows the
main dimensions; **Show all dimensions** shows the rest. In the **advanced
mode** every value can be changed, the **package family** can be chosen and
the **exposed pad** switched on or off:

![The advanced mode: every dimension of DFN-8 can be changed](/screenshots/partgen-advanced.png)

A typed value replaces the one from the datasheet (it loses its page
button). If a package's family is _Other_ — a package the generator does not
know — choose the family that fits, so the footprint can be made.

![All dimensions of SOIC-8](/screenshots/partgen-dimensions.png)

## Step 3: the pins

The pin table of the chosen package lists every pin with its **pad**
numbers, **name**, electrical **type** and **function**. The page button of
a row frames the row of the datasheet's pin table.

![The pins of SOIC-8, the first row framed in the datasheet's pin table](/screenshots/partgen-pins.png)

- **Pad** — one or more pad numbers (`6, 7` or `9-11`); BGA balls as `A1`.
  An empty pad cell removes the pin from this package.
- **Name** — as the datasheet writes it. Correct names here; they become the
  symbol's pin names.
- **Type** — input, output, bidirectional, tri-state, passive, power input,
  power output, open collector or drain, not connected, unspecified. The
  type comes from the datasheet's type column (I, O, I/O, P, G, OD …) or,
  without one, from the name (VCC and GND are power inputs, NC is not
  connected, OUT… an output and so on). The [electrical rules
  check](./erc) uses these types.
- **×** removes a pin, **Add pin** adds one with the next free pad.

Pads of the package without a pin are listed below the table; they become
not connected pins (NC) unless you add them.

Where the pins come from:

- **Pin tables**, with or without drawn lines, also when they go on over
  several pages, with a column of pad numbers per package (for example
  _SOIC-8, MSOP-8_ and _DFN-8_). Package variants in a cell (`3 (R5)`) become
  packages of their own.
- **Pinout drawings** of two-row packages (DIP, SOIC) when the datasheet has
  no pin table: the numbers down both sides and the names beside them. The
  package is taken from the caption above the drawing. Check these names:
  names drawn inside the package outline are not read.

## Step 4: the result

The last step shows what will be made:

![The result: symbol, footprint and 3D model of SOIC-8, the library checks and the target library](/screenshots/partgen-result.png)

- **Symbol** — a box with the supply pins on top, ground and unconnected
  pins at the bottom, inputs on the left and outputs on the right. Pins of
  one group (`PA0` … `PA7`, `D0` … `D7`, `IN1+`/`IN1−`) stay together, and
  bidirectional or passive groups go to the side with fewer pins. Parts with
  more than 64 pins are split into units: one with the supply pins, then
  units of whole groups (choose a unit above the preview).
- **Footprint** — made by the [footprint wizard](./footprint-wizard) of the
  package family with the dimensions of step 2, with silkscreen, assembly
  outline and courtyard. Its name follows the KiCad libraries, for example
  `SOIC-8_3.9x4.9mm_P1.27mm` or `QFN-32-1EP_5x5mm_P0.5mm_EP3.45x3.45mm`.
- **3D model** — the body and terminals of the package in their real sizes:
  gull-wing leads, terminals under the edge, balls, through-hole pins, tabs
  and exposed pads, with a pin-1 mark on top. Drag to turn it.
- **Library checks** — the [library checks](./library-checks) of symbol,
  footprint and component. **Generate part** stays off while a check finds
  an error.
- **Notes** — for example a pad that had no pin in the datasheet (it was
  added as `NC`, or as `EP` for an exposed pad or tab) or a pin without a
  pad in this package (it is left out).

**Names and options**: the **part name** (the symbol and component name;
by default the part and package name, such as `EW7021_SOIC-8`). In the
advanced mode also the **footprint name**, the **land size** (IPC-7351
density: least for dense boards, nominal, most for hand soldering) and the
pin count above which the symbol is **split into units** (0: never).

Choose the library under **Into the library** and click **Generate part**.
In one undo step the symbol, footprint and component are added to the
library, and the 3D model is written to `models/<footprint name>.step` in
the project folder. Names that are already taken get a suffix (`_2`). The
library opens, and the part can be placed in the schematic like any other
component.

![The new component in the library](/screenshots/partgen-done.png)

### A QFN with an exposed pad

The second synthetic datasheet below has a pin table without lines over two
pages, a composite font and AES encryption — the generator reads it the same
way. Its QFN-32 package gets 32 terminals and the exposed pad 33, as the
pin table says (_GND: 16, 33_):

![A QFN-32 with its exposed pad](/screenshots/partgen-qfn.png)

## Already in a library?

Before anything is made, the generator looks for the part in the project
libraries and in the installed [base library](./library). A part counts as
**already there** only when **both** are true:

1. its name or manufacturer part number is the part's (an ordering suffix is
   allowed: `LM358DR` is LM358), and
2. its footprint has **nearly the same pads**: the same pad numbers with
   their centres within 0.15 mm — the same package geometry. The land sizes
   are not compared, since they depend on the IPC density.

Then the generator names the part and its library, and **Generate part**
stays off: choose **Use …** to keep the existing part (the library opens),
or **Make it anyway**. The same part in **another package** is only
mentioned below.

![The duplicate check names the existing part](/screenshots/partgen-duplicate.png)

## Help from your AI agent

Some datasheets are hard to read automatically: pins only in a four-sided
pinout drawing, unusual tables, scanned pages. Your own AI agent (for
example Claude) can read the datasheet and correct the draft through
ElectronicWerk's [MCP server](./automation). Nothing in the app talks to an
AI service, and no API key is stored.

1. In step 1, open **Help from your AI agent** and click **Save draft for
   the agent**. The draft is saved as `parts/<part name>.part.json` in the
   project folder.
2. Click **Copy the request** and paste it into your agent. It asks the
   agent to read the datasheet with the `datasheet_text` tool, compare it
   with the draft (`part_draft_read`) and write the corrected draft back
   (`part_draft_write`).
3. When the agent is done, click **Load the corrected draft** and check the
   result in the steps as usual.

![Help from your AI agent](/screenshots/partgen-agent.png)

An agent can also make parts on its own with the tools `datasheet_extract`,
`part_preview` (which reports duplicates too) and `part_generate`; see
[AI agents and scripts](./automation).

## From the command line

```console
ewerk part extract datasheet.pdf --out part.json
ewerk part generate part.json --project my-board --package SOIC-8
```

`ewerk part text datasheet.pdf --from 3 --to 4` prints the text of pages.
See the [command-line reference](../reference/cli#ewerk-part).

## Trying it out

The command-line tool writes two synthetic datasheets of invented parts —
the EW7021 dual comparator in SOIC-8, MSOP-8 and DFN-8, and the EW4410 I/O
expander in QFN-32 and LQFP-32:

```console
ewerk synth datasheets my-datasheets
```

## Limits

- Datasheets that need a password to open cannot be read.
- **Scanned pages** have no text to read (there is no text recognition, OCR);
  enter their values by hand or ask your AI agent.
- **Pinout drawings** are read for two-row packages only; four-sided QFN and
  QFP drawings are not. Most such datasheets have a pin table as well.
- **Package drawings** without a dimension table (values written on the
  drawing only) are not read; the standard values of the package are used —
  check them.
- Package families the generator has no footprint wizard for (connectors,
  modules, LGA, ceramic carriers) are listed as _Other_; choose the nearest
  family or draw the footprint in the [footprint editor](./footprint-editor).
- The 3D model shows the body and terminals; it has no lettering or
  rounded edges.
