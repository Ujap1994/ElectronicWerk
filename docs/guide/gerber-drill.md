# Gerber and drill files

Board makers build a board from **Gerber files** (one image per layer:
copper, solder mask, silkscreen, solder paste, outline) and **drill files**
(where to drill which hole). ElectronicWerk writes both as
[outputs of an output job](./outputs), plus a drill map, a Gerber job file
and the netlists that some makers use for the electrical test.

## Gerber files

The **Gerber X2** output writes one file per layer in the Gerber format
(RS-274X) with the X2 extensions:

| Layer                                              | File name                                         | File function (`.FileFunction`)     |
| -------------------------------------------------- | ------------------------------------------------- | ----------------------------------- |
| Top copper                                         | `<board>-Copper_Top.gtl`                          | `Copper,L1,Top`                     |
| Inner copper _n_                                   | `<board>-Copper_In<n>.g<n>`                       | `Copper,L<n+1>,Inr`                 |
| Bottom copper                                      | `<board>-Copper_Bottom.gbl`                       | `Copper,L<count>,Bot`               |
| Top / bottom solder mask                           | `…-Soldermask_Top.gts`, `…-Soldermask_Bottom.gbs` | `Soldermask,Top` / `Bot` (negative) |
| Top / bottom silkscreen                            | `…-Silkscreen_Top.gto`, `…-Silkscreen_Bottom.gbo` | `Legend,Top` / `Bot`                |
| Top / bottom solder paste                          | `…-Paste_Top.gtp`, `…-Paste_Bottom.gbp`           | `Paste,Top` / `Bot`                 |
| Board outline                                      | `<board>-Outline.gm1`                             | `Profile,NP`                        |
| Adhesive, fabrication, courtyard, mechanical, user | `.gta`/`.gba` or `.gbr`                           | `Glue`, `AssemblyDrawing`, `Other`  |

By default the output writes the **fabrication layers**: every copper layer
of the stack-up, both solder masks, silkscreens and pastes, and the outline.
Untick **Fabrication layers** in its settings to choose the layers yourself.

### What the files contain

- **Coordinates** are in millimetres with six decimals (format 4.6). One unit
  of the file is one nanometre of the design, so nothing is rounded.
- **Pads and vias** are _flashed_ with apertures: circles, rectangles and
  obrounds where the pad has that shape, and exact aperture macros for
  rotated, rounded-rectangle, trapezoid and custom pads. CAM tools recognise
  every pad as a pad, not as a drawing.
- **Tracks** are lines and arcs (`G02`/`G03`) with round apertures.
- **Copper zones** are regions; their holes are joined to the outline by
  cut-ins, as the Gerber specification describes.
- **Texts** on silkscreen and copper are drawn with ElectronicWerk's stroke
  font, with the same size and position as on screen.
- **Solder mask** openings follow each pad's mask expansion; **solder paste**
  follows its paste margin (parts marked _do not place_ get no paste).
- **Vias** are covered by solder mask ("tented") unless you untick **Cover vias
  with solder mask**.

### X2 attributes

Gerber X2 adds _attributes_ that tell the board maker what everything is:

| Attribute       | Example                               | Meaning                            |
| --------------- | ------------------------------------- | ---------------------------------- |
| `.FileFunction` | `Copper,L1,Top`                       | Which layer the file is            |
| `.FilePolarity` | `Negative` (solder mask)              | Dark means "no mask here"          |
| `.ProjectId`    | `Blinky,<guid>,A`                     | Project name, id and revision      |
| `.AperFunction` | `SMDPad,CuDef`, `ViaPad`, `Conductor` | What a pad, via or track is        |
| `.N`            | `/TRIG`                               | The net of a pad, track or zone    |
| `.P`            | `U1,3`                                | Designator and pad number of a pad |
| `.C`            | `R1`                                  | The component a drawing belongs to |

With the net information the board maker can compare the copper with your
netlist. Some older CAM systems do not understand attributes: untick
**Gerber X2 attributes** to write plain RS-274X ("X1") files. The
[JLCPCB, PCBWay and OSH Park presets](./ordering) do this.

### Gerber job file

With **Gerber job file** ticked, `<board>-job.gbrjob` (JSON) describes the
whole set: board size, layer count, thickness, surface finish, the material
stack-up (from [board setup](./board-setup#stack-up)) and the function of
every file. Makers that read it need no further explanation of the files.

### File name extensions

**Protel file extensions** (`.gtl`, `.gbl`, `.gts`, …) are what most board
makers expect and recognise without help. Without them every file ends in
`.gbr`, and the X2 file function or the name tells the layers apart.

## Drill files

The **Excellon drill** output writes the holes in the Excellon format
(metric, decimal coordinates, absolute):

| File                   | Holes                                                            |
| ---------------------- | ---------------------------------------------------------------- |
| `<board>-PTH.drl`      | Plated holes through the whole board: vias and through-hole pads |
| `<board>-NPTH.drl`     | Non-plated holes (mounting holes)                                |
| `<board>-L1-L2.drl`, … | Blind and buried vias, one file per pair of copper layers        |

Tick **Plated and non-plated holes in one file** for makers that want a
single file (`<board>.drl`). Each file has one tool per drill size; slots
(oval holes) are written with `G85`. Like KiCad, the files carry their X2
attributes as comments (`; #@! TF.FileFunction,Plated,1,4,PTH`,
`; #@! TA.AperFunction,Plated,PTH,ViaDrill`), so CAM tools can tell vias from
component holes.

## Drill map

The **Drill map** draws the board outline and every hole with a symbol for
its drill size, and a legend with the size, count and plating of each drill
(slots counted separately). Choose PDF or SVG in its settings.

![A drill map](/screenshots/output-drill-map.png)

## IPC-D-356 netlist for the electrical test

Board makers test bare boards with flying probes: every net must be
connected, and no two nets may touch. The **IPC-D-356 netlist** tells the
tester which pads and vias belong to which net. ElectronicWerk writes
IPC-D-356A in the layout that KiCad uses (units of 0.0001 inch, coordinates
from the board origin — the drill and placement origin of an imported KiCad
board, otherwise the design origin — and net names longer than 14 characters
as `NNAME` aliases). Each record has the pad or via, its net, position, size,
drill and which sides are covered by solder mask; tented vias are marked as
covered, so the tester does not probe them.

## IPC-2581

**IPC-2581** (revision C) puts the whole board into one XML file: the
stack-up, the board outline, packages and components, the logical nets, the
copper, mask, paste and silkscreen of every layer, the drill holes and the
bill of materials. Some makers (Eurocircuits, many assembly houses) accept it
instead of separate Gerber, drill and placement files. ODB++, a similar
format, is not supported.

## How the output is checked

Every change to the Gerber and drill writers is tested automatically:

- The files of a KiCad example board are compared with the files KiCad 10
  writes for the same board. Both are read back by ElectronicWerk's Gerber
  reader and drawn into bitmaps with a resolution of 20 µm: copper, solder
  mask, paste, outline and drill holes match pixel for pixel. Only
  silkscreen texts differ, because KiCad uses another font.
- A test board with every kind of pad (rotated, rounded, trapezoid, custom),
  arc tracks, zone fills with holes, slots and parts on the bottom is written
  and read back, and compared with the exact copper of the board.
- The IPC-D-356 records of the example board match KiCad's record by record.
