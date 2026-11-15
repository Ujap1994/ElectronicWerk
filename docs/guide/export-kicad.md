# Exporting to KiCad

A design made in ElectronicWerk is never locked in: **Export as KiCad
Project…** writes the whole project as a KiCad project that KiCad 8 and
every later version open — the schematic with all its sheets, the board,
the project libraries and the design rules.

## Exporting

1. Open the project.
2. Choose **File → Export as KiCad Project…**, or **Export as a KiCad
   project** in the [command palette](./command-palette).
3. Choose an **empty folder** (or a new one) for the KiCad project.

![The export in the command palette](/screenshots/export-kicad-command.png)

A notice names the `.kicad_pro` file; open it in KiCad.

![The notice after the export](/screenshots/export-kicad-notice.png)

## What is written

| File                  | Content                                                                                                                             |
| --------------------- | ----------------------------------------------------------------------------------------------------------------------------------- |
| `<project>.kicad_pro` | Net classes with their patterns, the board minimums of the [design rules](./design-rules) and the text variables.                   |
| `<project>.kicad_sch` | The top sheet. Every other sheet is a file of its own (`Power.kicad_sch`, …), placed through its sheet symbol with its page number. |
| `<project>.kicad_pcb` | The board: stack-up, footprints, tracks, arcs, vias, zones (with their fill), drawings, texts and the drill and placement origin.   |
| `<project>.kicad_sym` | The project's own symbols (those without a library name), with the library table `sym-lib-table`.                                   |
| `<project>.pretty/`   | The project's own footprints, with the library table `fp-lib-table`.                                                                |

Everything links up in KiCad:

- **Symbols and footprints.** Each placed symbol carries its footprint
  (`Library:Name`) and the board footprint the path of its symbol, so
  KiCad's _Update PCB from Schematic_ and the schematic parity check find
  nothing to change.
- **Power ports** become KiCad power symbols (`power:GND`, …) with the
  same nets.
- **Symbols are embedded** in the schematic, as KiCad does: the schematic
  opens even without the libraries. Symbols that came from KiCad libraries
  keep their library names (`Device:R`), so KiCad can update them from its
  own libraries.
- **Bottom-side parts** are written the way KiCad stores them (flipped), so
  they land at the same place with the same pads.

The files are written in the **KiCad 8 format** (board `20240108`,
schematic `20231120`). KiCad 9 and 10 open them and save them in their own
format when you save.

## What KiCad cannot hold

Some things have no place in a KiCad project. They are left out and listed
in a notice after the export:

| Left out                              | Why                                                                                               |
| ------------------------------------- | ------------------------------------------------------------------------------------------------- |
| Drawings on mechanical layers 2 to 32 | KiCad has one mechanical layer (_Margin_, for mechanical layer 1).                                |
| Drawings on user layers above 13      | KiCad 8 has the user layers `User.1` to `User.9` besides the four classic ones.                   |
| A second or third board               | A KiCad project has one board; the others are written as their own `.kicad_pcb` files next to it. |
| Sheets not placed in the hierarchy    | They are written, but KiCad only shows them once they are placed.                                 |

Zone fills with holes are not written; KiCad fills those zones again when
you press <kbd>B</kbd> or run the design rules check.

## How the export is tested

The export is checked in both directions: a KiCad project is imported,
exported and imported again, and the netlist, every part, wire, label and
sheet and every board item must be the same. KiCad itself reads the
exported files: its netlist is the same as the netlist of the original
project, and its design rules check finds the same results.

## From the command line

```sh
ewerk project export-kicad "Sensor Board" -o "Sensor Board KiCad"
```

See [`ewerk project export-kicad`](../reference/cli#ewerk-project-export-kicad)
and the engine method
[`project.exportKicad`](../reference/engine-api#project-exportkicad).
