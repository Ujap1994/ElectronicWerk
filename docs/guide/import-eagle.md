# Importing Eagle projects

ElectronicWerk reads projects of **Autodesk Eagle** (versions 6 to 9, and
the Eagle files that Fusion electronics exports): the schematic (`.sch`),
the board (`.brd`) and libraries (`.lbr`). An Eagle project becomes a new
ElectronicWerk project; the Eagle files are only read, never changed.

Eagle files before version 6 are binary. Open them once in Eagle 6 or later
(or the free Eagle viewer) and save them again to get the XML files.

## Importing a project

1. On the start screen, choose **Import Eagle project** (or **File → Import
   Eagle Project…**, or **Import Eagle project** in the
   [command palette](./command-palette)).
2. Choose the schematic or the board. The other file of the same name next
   to it is read too, so a schematic and its board come in together.
3. Check the name and the location of the new project and choose
   **Import**.

![Importing an Eagle project](/screenshots/import-eagle-dialog.png)

**Contains** lists what was found: _Schematic_, _Board_ or both. A board
without a schematic, or a schematic without a board, can be imported as
well.

After the import, the [import report](#the-import-report) lists what came in
and what could not be carried over. The new project opens.

![The Timer sheet of the imported Eagle project](/screenshots/import-eagle-sheet.png)

![The board of the imported Eagle project](/screenshots/import-eagle-board.png)

## What is imported

| Eagle                                     | ElectronicWerk                                                                                                                                     |
| ----------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| Sheets                                    | One sheet each. With several sheets, a top sheet with a sheet symbol for each Eagle sheet holds them together.                                     |
| Parts and gates                           | Placed symbols; each gate is a unit of the symbol. Part attributes (`MPN`, …) become properties.                                                   |
| Device sets and devices                   | One symbol per device (`R` + `0805` = `R0805`) in the project library, its pins numbered with the pads of the device.                              |
| Packages                                  | Footprints in the project library: SMD pads (with roundness), through-hole pads (round, square, octagon, long, offset), holes, drawings and texts. |
| Supply symbols (`GND`, `VCC`, …)          | Power symbols: they connect their net everywhere and are not on the board or the bill of materials.                                                |
| Nets, wires, junctions, labels            | Wires, junctions and global net labels (Eagle's net names are global). See [How nets are kept](#how-nets-are-kept).                                |
| Buses                                     | Drawn as bus lines; their member nets connect through their labels.                                                                                |
| Texts and drawings on sheets              | Texts and graphics.                                                                                                                                |
| Elements                                  | Footprints on the board; mirrored elements (`MR90`, …) on the bottom side.                                                                         |
| Signals                                   | Nets with their tracks (straight and curved), vias and copper pours.                                                                               |
| Board drawings (layer 20 _Dimension_)     | The board outline.                                                                                                                                 |
| Holes on the board                        | Mounting holes (`H1`, `H2`, …).                                                                                                                    |
| Net classes                               | Net classes with their track width, clearance and via drill; nets keep their class.                                                                |
| Design rules (`mdWireWire`, `msWidth`, …) | The board minimums: clearance, track width, smallest hole and copper to edge.                                                                      |

Board layers are mapped like this:

| Eagle layer                                  | ElectronicWerk layer   |
| -------------------------------------------- | ---------------------- |
| 1 Top, 16 Bottom, 2–15 inner layers in use   | Copper layers          |
| 20 Dimension, 46 Milling                     | Board outline          |
| 21 tPlace, 25 tNames (22, 26 for the bottom) | Silkscreen             |
| 27 tValues, 51 tDocu (28, 52)                | Fabrication            |
| 29 tStop (30)                                | Solder mask            |
| 31 tCream (32)                               | Paste                  |
| 35 tGlue (36)                                | Adhesive               |
| 39 tKeepout (40)                             | Courtyard              |
| 47 Measures, 48 Document, 49 Reference       | User 1, User 2, User 3 |
| 100–127 (your own layers)                    | User 5 to User 32      |

The board and the schematic stay linked: after the import, **Update from
the schematic** in the [layout editor](./layout-editor) finds nothing to
change.

## How nets are kept

Eagle connects what a net lists, by its name; ElectronicWerk connects what
touches on the sheet. The import keeps every net of the Eagle file:

- **Labels sit on their wires.** An Eagle label is placed near its wire;
  in ElectronicWerk it moves onto the nearest point of that wire.
- **Pins in the middle of a wire** split the wire there, so the pin
  connects.
- **Every segment of a net with several segments** (or on several sheets)
  gets a global label with the net's name, so the segments connect as in
  Eagle.
- **Supply symbols** connect their net through their power pin.

Wires that only run across a pin of another net without a connection in
Eagle would connect in ElectronicWerk. Eagle designs rarely have them; the
[electrical rules check](./erc) and the [board update](./schematic-to-board)
show them.

## The import report

![The import report of an Eagle project](/screenshots/import-eagle-report.png)

The report counts what was imported and lists everything that was not
carried over. It is also saved as `eagle-import-report.md` in the new
project folder.

| Note                                               | What it means                                                                                           |
| -------------------------------------------------- | ------------------------------------------------------------------------------------------------------- |
| _Copper pours are imported unfilled_               | Fill the zones on the board (**Fill all zones**). Eagle files hold the outline of a pour, not its fill. |
| _Drawing frames are not imported_                  | Sheets get the ElectronicWerk frame and title block.                                                    |
| _Buses are drawn as lines_                         | Bus member nets connect through their labels.                                                           |
| _Drawings on an Eagle layer without a counterpart_ | Drawings on layers such as 41 tRestrict or 44 Drills are left out.                                      |
| _A pin connects several pads_                      | Only the first pad is linked to the pin; the other pads keep their net on the board.                    |
| _Item kind not supported yet_                      | Dimensions, frames on boards and other items are left out.                                              |

## Eagle libraries

An Eagle library (`.lbr`) is added to a project library like a KiCad
library file: in the [project tree](./project-tree), right-click a library
(or the **Libraries** group), choose **Import KiCad library file…** and pick
the `.lbr` file (the dialog offers KiCad and Eagle libraries). Each device
becomes a symbol and each package a footprint, named `library:name`; names
the library has already are skipped (see
[Importing KiCad libraries](./import-kicad#importing-kicad-libraries)).

## From the command line

```sh
ewerk import eagle blinky.sch "Blinky"
ewerk import library MyParts.lbr --project "Blinky"
```

See [`ewerk import eagle`](../reference/cli#ewerk-import-eagle). Scripts
call the engine method [`import.kicad`](../reference/engine-api#import-kicad)
with an Eagle file; it recognizes the format.
