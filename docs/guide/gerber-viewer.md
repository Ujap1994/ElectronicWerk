# The Gerber viewer

The Gerber viewer shows Gerber and drill files the way a board maker reads
them — not the board in the editor, but the generated files. Use it to check
the files before you upload them: are all layers there, is the solder mask
open where it should be, are the drill holes in the pads?

![The Gerber viewer with the files of the Blinky example](/screenshots/gerber-viewer.png)

## Opening files

- **Gerber viewer** in the toolbar of an [output job](./outputs) opens the
  Gerber and drill files the job generated (after **Generate all**; before
  the first run in this session, the files found in the job's output folder).
- **Gerber viewer: open Gerber and drill files…** in the
  [command palette](./command-palette) opens any files, for example the
  files of another tool or from a board maker.

The files open in a tab of their own, next to the documents. The tab is
read-only; **Reload** reads the files again after a new run.

## What it reads

The viewer reads Gerber files in the RS-274X format with or without X2
attributes, and Excellon drill files:

- standard apertures (circle, rectangle, obround, polygon) and aperture
  macros with variables and expressions,
- lines and arcs (single- and multi-quadrant), regions with several
  contours and cut-ins,
- dark and clear polarity, step and repeat,
- Excellon tools, drilled holes, slots (`G85`) and routed slots, metric and
  inch, decimal and implicit coordinates.

Each file becomes a layer. The colour comes from the file's X2 file function
(`Copper,L1,Top`, `Soldermask,Bot`, …), or from its extension (`.gtl`,
`.gbs`, …) when the file has no attributes. Files the viewer cannot read, and
commands it skipped, are listed under **Problems** in the side panel.

## Working with the view

Everything works as in the [viewer](./viewer): zoom with the wheel or the
toolbar, fit with **Home**, pan with the middle mouse button or the space
bar. The **Layers** panel switches files on and off and sets their opacity;
**Only active** shows a single file. **View from the bottom** turns the board
over:

![The bottom side in the Gerber viewer](/screenshots/gerber-viewer-bottom.png)

::: tip Inner planes
An inner ground plane covers the whole board and hides the layers below it.
Switch it off in the **Layers** panel to see the outer layers, as in the
first picture.
:::
