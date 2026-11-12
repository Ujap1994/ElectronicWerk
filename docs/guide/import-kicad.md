# Importing KiCad projects

ElectronicWerk imports KiCad projects with their schematics, board, libraries
and design rules, and turns them into a new ElectronicWerk project. The
KiCad files are only read, never changed. After the import a report shows what was
imported and everything that could not be carried over exactly.

![An imported KiCad board](/screenshots/import-board.png)

KiCad 10 is the reference. Files from KiCad 6, 7, 8 and 9 are read as well
(all versions with the S-expression file format). Schematics from KiCad 5 and
earlier (`.sch`) use a different file format and are not read: open the
project once in a current KiCad and save it to convert it.

::: tip Your KiCad project stays as it is
The import creates a separate ElectronicWerk project in a new folder. Your
KiCad project is never modified, so you can import it again at any time, for
example after changing it in KiCad.
:::

## Starting the import

There are three ways to start:

- On the start screen, click **Import KiCad project**.
- Choose **File → Import KiCad Project…** in the menu bar.
- Open the [command palette](./command-palette) and run **Import KiCad
  project**.

![The import card on the start screen](/screenshots/import-start-action.png)

A file dialog opens. Choose the KiCad project file (`.kicad_pro`). You can
also choose the schematic (`.kicad_sch`) or the board (`.kicad_pcb`) of the
project; ElectronicWerk then finds the other files of the same project in the
same folder. A project that has only a board or only a schematic is
imported as well.

## The import dialog

![The import dialog](/screenshots/import-dialog.png)

| Field             | Meaning                                                                                                                                                                |
| ----------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **KiCad project** | The chosen KiCad project, and which parts it contains: schematic, board and project settings.                                                                          |
| **Project name**  | The name of the new ElectronicWerk project. It defaults to the name of the KiCad project and follows the same rules as a [new project](./projects#creating-a-project). |
| **Location**      | The folder in which the project folder is created. Click **Change…** to choose another one. The hint below shows the full path.                                        |

Click **Import** to start. The project is created, opened, and the import
report appears. The documents of the new project get names in the language
of the user interface (for example _Design rules_ or _Entwurfsregeln_).

## The import report

![The import report](/screenshots/import-report.png)

The report has two parts:

- **Imported** counts what was imported: sheets, placed symbols, wires, net
  labels, footprints, pads, tracks, vias, zones, nets, texts and graphics,
  and symbols and footprints from the project's own libraries.
- **Findings** lists everything that could not be imported exactly. Each
  finding has a severity, a short title, the file it concerns, a detailed
  message and a code (see the [table below](#report-codes)). Findings that occur
  more than once are shown once with a count (for example **12×**).

| Severity    | Meaning                                                                                        |
| ----------- | ---------------------------------------------------------------------------------------------- |
| **Error**   | Part of the project could not be imported (for example a sheet file is missing or unreadable). |
| **Warning** | Something was imported in a simplified form or left out, and you should check it.              |
| **Note**    | Information about how something was imported; usually nothing to do.                           |

The report is also written into the new project as `kicad-import-report.md`
(in Markdown, with the KiCad file versions that were read). Click **Show the
report file** to reveal it in your file manager. It appears in the project
tree under **Other files**.

![The imported project](/screenshots/import-workspace.png)

## What is imported

| KiCad                                            | ElectronicWerk                                                                                                                                                                                                       |
| ------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Project file (`.kicad_pro`)                      | Project name, text variables (such as `${AUTHOR}`), net classes, net class assignments by pattern and the board constraints. These go into the **Design rules** document.                                            |
| Schematic hierarchy                              | One schematic document per sheet file, linked by sheet symbols, with the top sheet set as the project's top sheet. A sheet file used by several sheet symbols is imported once.                                      |
| Sheets                                           | Placed symbols with all fields, units and mirroring; wires, buses and bus entries; junctions; no-connect flags; local, global and hierarchical labels; texts and graphics; title block.                              |
| Symbols                                          | The symbols the design uses (from the schematic's symbol cache) and the symbols of the project's own libraries, with units, pins, pin names and numbers, and derived symbols resolved.                               |
| Board (`.kicad_pcb`)                             | Layer stackup (copper layers, dielectrics, board thickness, surface finish), footprints, tracks and arc tracks, vias, zones with their filled areas and keep-out areas, board graphics, texts and the board outline. |
| Footprints                                       | Pads of every shape (round, oval, rectangle, rounded rectangle, trapezoid, custom), drills and slots, pad clearances, graphics, texts, attributes, properties and 3D model references.                               |
| Library tables (`sym-lib-table`, `fp-lib-table`) | Libraries inside the project folder (`${KIPRJMOD}`) are imported into the project library. Libraries outside the project are not copied; the symbols and footprints the design uses come from the design itself.     |
| Components                                       | A placed symbol and its footprint are linked as one component, so you can find a part on the sheet and on the board.                                                                                                 |
| Nets                                             | All net names of the board, from every KiCad version (names in KiCad 10, net codes before).                                                                                                                          |

Footprints on the bottom side are stored as seen from the top and flipped
when they are drawn, as in every ElectronicWerk board. Coordinates are
converted exactly to ElectronicWerk's internal unit (1 nm); nothing is
rounded to a grid.

The new project also gets a default fabrication [output job](./project-tree#output-jobs)
and a `.gitignore`, as a [new project](./projects) does.

## What is not imported (yet)

| KiCad                                          | What happens                                                                                                                          |
| ---------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| Custom design rules (`.kicad_dru`)             | Not imported; the report counts them (`custom-rules`). Write them again as [custom rules](./design-rules#custom-rules).               |
| Dimensions, text boxes, images, tables         | Not imported; the report names each kind (`unsupported-item`).                                                                        |
| Alternate body styles (De Morgan)              | Only the standard body style is imported (`alternate-body-style`).                                                                    |
| Custom pads made of several shapes             | Simplified to their first polygon (`custom-pad`).                                                                                     |
| Chamfered pads                                 | Imported as rounded rectangles (`chamfered-pad`).                                                                                     |
| Zones inside footprints                        | Not imported (`footprint-zone`).                                                                                                      |
| Libraries outside the project folder           | Not copied (`external-library`); use [Importing KiCad libraries](#importing-kicad-libraries) to add symbols and footprints from them. |
| Dashed and dotted lines                        | Imported, but drawn as solid lines for now.                                                                                           |
| Page frame reference zones                     | The title block is imported; the letters and numbers along the page frame are not drawn yet.                                          |
| Simulation models, ERC settings, plot settings | Not imported.                                                                                                                         |

## Report codes

| Code                   | Severity           | Meaning                                                                                                                                           |
| ---------------------- | ------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------- |
| `missing-sheet`        | error              | A sheet symbol refers to a sheet file that does not exist.                                                                                        |
| `unreadable-file`      | error/warning/note | A file could not be read or is not valid. Sheets and the board are errors; library files and library tables are warnings or notes.                |
| `custom-rules`         | warning            | Custom design rules from `.kicad_dru` are not imported.                                                                                           |
| `custom-pad`           | warning            | A custom pad made of several shapes was simplified.                                                                                               |
| `footprint-zone`       | warning            | A zone inside a footprint was not imported.                                                                                                       |
| `missing-parent`       | warning            | A derived symbol (`extends`) refers to a symbol that is not in its library.                                                                       |
| `missing-symbol`       | warning            | A placed symbol is missing from the schematic's symbol cache.                                                                                     |
| `unknown-layer`        | warning            | A track or graphic is on a layer that ElectronicWerk does not know; it was left out.                                                              |
| `unsupported-item`     | note/warning       | An item kind that is not imported yet (dimensions, text boxes, images and so on).                                                                 |
| `alternate-body-style` | note               | Alternate body styles of a symbol are not imported.                                                                                               |
| `already-in-library`   | note               | A library import skipped symbols or footprints that the library already has.                                                                      |
| `chamfered-pad`        | note               | A chamfered pad was imported as a rounded rectangle.                                                                                              |
| `external-library`     | note               | A library table entry points outside the project; it was not copied.                                                                              |
| `no-board`             | note               | The KiCad project has no board.                                                                                                                   |
| `no-designator`        | note               | A footprint on the board has no designator.                                                                                                       |
| `no-project-file`      | note               | There is no `.kicad_pro` file; default settings were used.                                                                                        |
| `no-schematic`         | note               | The KiCad project has no schematic.                                                                                                               |
| `reused-sheet`         | note               | A sheet file is used by several sheet symbols. It is imported once; each use keeps its designators, and the link to the board uses the first use. |
| `text-box`             | note               | Text boxes in symbols are imported as their frame; text boxes in footprints are left out.                                                         |
| `vertical-slot`        | note               | A vertical slot in an oval pad is drawn along the pad's width for now.                                                                            |
| `zone-layer`           | note               | Zones and rule areas on non-copper layers are not imported.                                                                                       |

## Checking the result

After the import:

1. Open the board and the sheets in the [viewer](./viewer) and compare them
   with KiCad.
2. Run **Check project** in the toolbar (see [Checking a project](./projects#checking-a-project)). It reports, for example, placed
   symbols that are not in a project library.
3. Compare the board with the netlist that KiCad exports (**File → Export →
   Netlist…** in the KiCad schematic editor). See
   [Netlists](./netlists). If the board connects every pin like the netlist,
   the import is electrically complete.

![An imported sheet](/screenshots/import-sheet.png)

## Importing KiCad libraries

You can add symbols and footprints from KiCad libraries to a project library
at any time, for example from the official KiCad libraries or from your own.

1. In the project tree, right-click a library, or the **Libraries** group.
2. Choose **Import KiCad library file…** for a symbol library (`.kicad_sym`)
   or a single footprint (`.kicad_mod`). Choose **Import KiCad library folder…**
   for a footprint library (`.pretty`) or a KiCad 10 symbol library folder
   (`.kicad_symdir`).

![The library import menu](/screenshots/library-import-menu.png)

The symbols and footprints get the library name as a prefix, as in KiCad:
`Device:R` or `Resistor_SMD:R_0805_2012Metric`. Names that the library already
has are skipped, so you can import a library again without duplicates. A
notice shows how many symbols and footprints were added. Use **Undo** to
take the import back.

The import reads large libraries quickly: all official KiCad libraries
(about 23,000 symbols and 15,000 footprints) read in a few seconds.

## Command line

The command-line tool imports projects, libraries and netlists without the
desktop app, for example in scripts:

```sh
ewerk import kicad blinky/blinky.kicad_pro Blinky
ewerk import library MyParts.pretty --project Blinky
ewerk import netlist blinky/blinky.net --project Blinky
```

See [`ewerk import`](../reference/cli#import) for all options.

## Privacy

The import runs entirely on your computer. Nothing is uploaded.
