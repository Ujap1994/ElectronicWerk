# Bill of materials and pick and place

Assembly houses need two files besides the Gerber files: the **bill of
materials** (which parts to buy and where they go) and the **pick-and-place**
file (where the machine puts each part). Both are
[outputs of an output job](./outputs).

## Bill of materials

The bill of materials lists the parts of the project. It takes them from the
schematic — every symbol with a designator; power symbols and parts marked
**Exclude from the bill of materials** are left out — and adds the side and
mounting type from the board. A project without a schematic takes its parts
from the board.

![The settings of the bill of materials with a preview](/screenshots/outputs-bom.png)

### Format

| Format                    | File                 | Notes                                                                                    |
| ------------------------- | -------------------- | ---------------------------------------------------------------------------------------- |
| **CSV**                   | `<project>-bom.csv`  | UTF-8 with a byte order mark (spreadsheets read µ and umlauts correctly), CRLF line ends |
| **Excel workbook (XLSX)** | `<project>-bom.xlsx` | One sheet with a bold, frozen header row, a filter and fitted column widths              |
| **HTML page**             | `<project>-bom.html` | A stand-alone page with the project name and revision; prints well; light and dark       |

The **CSV separator** can be a comma, a semicolon (for spreadsheets set to
German and other languages that use the comma as decimal separator) or a tab.

### Columns

Every column shows one **field**, and its **heading** can be changed (an
empty heading uses the field's name). Add columns with **Add column**, move
them with the arrow and remove them with the bin.

| Field       | Content                                                           |
| ----------- | ----------------------------------------------------------------- |
| Item        | Line number                                                       |
| Quantity    | Number of parts on the line                                       |
| Designators | The designators of the line, in natural order (`R2, R10`)         |
| Value       | Value of the part                                                 |
| Footprint   | Footprint name                                                    |
| Symbol      | Symbol name                                                       |
| Description | Description of the symbol                                         |
| Side        | `Top` or `Bottom` on the board                                    |
| Mounting    | `SMD` or `THT`                                                    |
| DNP         | `DNP` for parts that are not placed                               |
| Any other   | A field of the symbols, for example `Manufacturer`, `MPN`, `LCSC` |

The list of fields offers every field name used by a symbol of the project,
plus `Manufacturer`, `MPN` and `LCSC`. Field names compare without case
(`mpn` and `MPN` are the same field).

### Grouping

Parts share a line when the ticked fields are equal — by default the value,
footprint, manufacturer, part number and the DNP mark. Untick every field to
list each part on its own line. Lines are sorted by their first designator.

Parts marked **Do not place** (in the schematic or on the board) are listed
with `DNP` by default; untick **List parts that are not placed** to leave them
out.

The **Preview** below the settings shows the first 50 lines exactly as they
will be written and updates with every change.

## Pick and place

The **Pick and place** output (also called _centroid_, _CPL_ or _XY file_)
lists the position, rotation and side of every part.

![The settings of the placement file](/screenshots/outputs-placement.png)

| Setting                               | Meaning                                                                                                                                                                  |
| ------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **Columns**                           | **Generic** `Designator,Value,Footprint,X,Y,Rotation,Side`; **JLCPCB** `Designator,Mid X,Mid Y,Layer,Rotation` with `mm`; **KiCad** `Ref,Val,Package,PosX,PosY,Rot,Side` |
| **Unit**                              | Millimetres or inches                                                                                                                                                    |
| **Relative to the board origin**      | Coordinates from the board origin (the drill and placement origin of an imported KiCad board); otherwise from the design origin                                          |
| **Include parts that are not placed** | DNP parts are left out by default                                                                                                                                        |
| **One file per side**                 | `<board>-placement-top.csv` and `…-bottom.csv` instead of one file                                                                                                       |
| **Mirror x of parts on the bottom**   | For machines that expect bottom coordinates as seen from the bottom                                                                                                      |

The position is the origin of the footprint, which is the centre of the part
for footprints from the base library and KiCad. The rotation is the
footprint's rotation counter-clockwise in degrees (0 to 360), as on the board.
Footprints marked **Exclude from placement files** are left out (mounting
holes, fiducials, logos).

::: tip Rotations of assembly houses
Some assembly houses use their own zero orientation for certain packages and
correct the rotation in their order form. Check the placement preview of the
order before you confirm it.
:::
