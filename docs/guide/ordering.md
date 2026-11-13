# Ordering boards

Every board maker accepts Gerber and drill files, but each has its
preferences: file extensions, X2 attributes or not, one drill file or two,
the columns of the bill of materials and the placement file. The **presets**
of the [output job](./outputs) set all of this for common board makers.

## Presets

Choose **Apply a preset…** in the toolbar of an output job. The outputs of the
job are replaced by the outputs of the preset (you can undo this):

![Applying a preset](/screenshots/outputs-preset.png)

| Preset           | Outputs                                                                                                                                                                      |
| ---------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Generic**      | Gerber X2 with job file, drill files, drill map, BOM (XLSX), placement, IPC-D-356, schematic PDF, assembly drawing, ZIP of Gerber and drill files                            |
| **JLCPCB**       | Gerber without X2 attributes and job file (Protel extensions), drill files, BOM and CPL in JLCPCB's columns, ZIP                                                             |
| **PCBWay**       | Gerber RS-274X, drill files, BOM workbook (designators, quantity, value, footprint, manufacturer, part number, description), placement, schematic PDF, assembly drawing, ZIP |
| **AISLER**       | Gerber X2 with job file, drill files, BOM (CSV), placement, ZIP                                                                                                              |
| **Eurocircuits** | Gerber X2 with job file, drill files, IPC-D-356, IPC-2581, BOM workbook, placement, ZIP of Gerber, drill and IPC-D-356                                                       |
| **OSH Park**     | Gerber without attributes, one drill file for all holes, ZIP                                                                                                                 |

New projects and new output jobs start with the **Generic** preset. After
applying a preset, every output can still be changed, added or removed.

## What to upload

1. Check the board with the [design rules check](./drc) — with the rules of
   your board maker, see [board presets](./board-setup#presets).
2. Fill the zones (**Fill all zones**, <kbd>Shift</kbd>+<kbd>B</kbd>) and save.
3. **Generate all** in the output job.
4. Check the files in the [Gerber viewer](./gerber-viewer).
5. Upload the ZIP package (`<board>-<maker>.zip` or
   `<board>-fabrication.zip`) to the board maker. For assembly, upload the bill
   of materials and the placement file where the order form asks for them.

::: warning Zones are written as they are filled
Gerber files contain the zone fills stored in the board. If you changed the
board after the last **Fill all zones**, fill again before generating, or the
fills may not match the tracks. The [DRC](./drc) reports unfilled zones.
:::

## Notes per board maker

- **JLCPCB** reads the BOM columns _Comment, Designator, Footprint, LCSC Part #_
  and the CPL columns _Designator, Mid X, Mid Y, Layer, Rotation_. Put the LCSC
  part number of every part into a field called `LCSC` in the schematic; the
  preset adds it as a column. JLCPCB's own rotation for some packages may
  differ: check its placement preview.
- **PCBWay** and **AISLER** read the X2 attributes; PCBWay's preset leaves them
  out for older CAM systems — tick **Gerber X2 attributes** if you prefer them.
- **Eurocircuits** checks the netlist against the copper: the IPC-D-356 file
  is in the package.
- **OSH Park** wants one drill file for plated and non-plated holes.

These presets are a starting point; the requirements of board makers change.
If a maker asks for something else, change the outputs of the job — the
settings are saved with the project.
