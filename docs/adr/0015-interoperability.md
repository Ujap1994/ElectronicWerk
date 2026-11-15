# ADR-0015: Interoperability — KiCad export, Eagle import, Specctra, Gerber reference, DXF

- **Status:** accepted
- **Date:** 2026-11-15
- **Milestone:** M14

## Context

M14 asks for further open formats, so that a design never stays locked in
and boards can travel to other tools: a **KiCad export** (round trip),
**Specctra DSN/SES** for external autorouters such as Freerouting,
**Gerber/Excellon as a reference layer**, a **DXF export** of the outline
and mechanical layers (moved from M8) and an **Eagle XML import**
(`.sch`/`.brd`/`.lbr`) with an import report and the libraries routed into
the duplicate check.

## Decision

### KiCad export

- **KiCad 8 format** (board `20240108`, schematic and symbol libraries
  `20231120`): KiCad 8, 9 and 10 open it; newer KiCad versions upgrade it
  when they save. Writing the newest format would lock out KiCad 8 users.
- **The inverse of the import.** Every conversion (page frame with y down,
  bottom footprints flipped back, pad and text angles, 3D model offsets,
  field placement, mirror and rotation of symbols) is the inverse of the
  KiCad import, through a small S-expression writer (`ew-kicad::write`).
  KiCad requires the angle in schematic positions, so a given angle is
  always written, also when it is 0.
- **Linked like KiCad does.** Placed symbols carry their instance paths
  (`/root/sheet…`) and footprint fields with the library name, board
  footprints the path of their symbol, so KiCad's schematic parity and
  _Update PCB_ find nothing.
- **Native items get KiCad counterparts:** power ports become `power:<net>`
  symbols; the project's own symbols and footprints a `.kicad_sym`, a
  `.pretty` folder and library tables. What KiCad cannot hold (mechanical
  layers 2–32, user layers above 13, further boards, unplaced sheets) is
  listed in a report.
- **Verified both ways:** import → export → import keeps the netlist, the
  parts and every board item; locally, `kicad-cli` reads the export and
  produces the same netlist and the same design rules results as for the
  original.

### Eagle import

- **XML only** (Eagle 6 to 9, Fusion exports), read with `roxmltree`.
  Binary Eagle files (before version 6) are refused with a hint.
- **Same report, same flow** as the KiCad import: the engine's
  `import.kicadProbe`/`import.kicad`/`import.library` recognize Eagle files
  and return the same report type, so the app reuses its dialogs.
- **Libraries:** one symbol per device (pins numbered with the device's
  pads), packages as footprints, both named `library:name`; `.lbr` files
  go through the library import and its duplicate check.
- **Orientation:** Eagle turns first and mirrors after (`MR90`); in
  ElectronicWerk's order (mirror first) that is the negative angle. This
  was checked against KiCad's own Eagle reader (`pcbnew`); mirrored
  elements sit on the bottom.
- **Nets by name, connectivity by geometry:** Eagle connects what a net
  lists; ElectronicWerk what touches. The import snaps labels onto their
  wires, splits wires at pins they meet in the middle, gives every segment
  of a multi-segment net a global label (Eagle's names are global) and
  builds a top sheet with a sheet symbol per Eagle sheet. After the import
  the board is linked to the schematic with the board update's own part
  keys and net names, so the update finds nothing.

### Specctra, Gerber reference, DXF

- **Specctra DSN** describes every part as placed (an image per part at
  rotation 0 on the front, pads in their final shape and layers), so the
  router's rotation and mirror conventions do not matter. Net classes
  carry width, clearance and via; locked routing is _protect_. A **session
  (SES)** replaces the unlocked routing of the nets it routes, in one undo
  step. Coordinates: µm, y up, resolution 0.1 µm. Unlocked routing carries
  no wire type, because Freerouting reads every type except `normal` as
  fixed; protected wires of a session that are the board's locked tracks are
  not added twice. A recorded Freerouting 2.5.0 run is replayed as a test.
- **Gerber/Excellon reference:** the files' images are united into areas
  (clear shapes cut out, holes joined by cut-ins) and drawn as filled
  polygons on user or mechanical layers — drawings, not copper.
- **DXF** is an output kind writing ASCII R12 (`AC1009`): centre lines of
  the outline, mechanical and user layers, part drawings and texts,
  relative to the drill and placement origin; with a panel, the panel.

### The y axis

ElectronicWerk's y axis points up on sheets and boards (like Gerber, DXF and
Specctra). The DXF import (M7) mirrored y as if the board's pointed down;
asymmetric outlines came in upside down. It was corrected together with
the new formats, which all keep coordinates as they are.

## Consequences

- Projects can leave ElectronicWerk for KiCad at any time and come back
  through the KiCad import; Eagle users can bring their designs in.
- No autorouter is bundled; Freerouting and other Specctra routers stay
  external tools the user installs.
- Eagle features without a counterpart (frames, buses as nets, design
  blocks, modules, multiple pads per pin) are reported, not converted.
- The local KiCad checks are ignored tests (`EW_KICAD_CLI`), because CI
  runners have no KiCad.
