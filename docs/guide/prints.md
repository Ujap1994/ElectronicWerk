# Prints and drawings

The **PDF** and **SVG** outputs of an [output job](./outputs) print the
schematic, board layers or assembly drawings. PDF puts every page into one
file with a bookmark per page; SVG writes one file per page. Both are vector
drawings at their real size, so they can be zoomed and printed sharply.

![The settings of an assembly drawing](/screenshots/outputs-print.png)

## Content

| Content               | Pages                                                                   |
| --------------------- | ----------------------------------------------------------------------- |
| **Schematic**         | Every sheet, in reading order: the top sheet, then its sub-sheets       |
| **Board layers**      | The chosen layers of the board, on one page or one page per layer       |
| **Assembly drawings** | One page per side with parts: outline, part outlines, pads, designators |

### Schematic

The schematic print draws each sheet exactly as the viewer shows it, in the
colours of the light theme or in black and white, with the frame and title
block. With **Paper** set to _Size of the drawing_ every sheet keeps its own
paper size (A4, A3, …); A4, A3 or Letter fit every sheet onto that paper.

![A schematic sheet as printed](/screenshots/output-schematic.png)

### Board layers

Tick the layers to draw; they are drawn in the order of the list, later ones
on top, and the board outline is always included. **One page per layer**
gives each layer its own page (for checking a layer at a time), **Seen from
the bottom** mirrors the drawing for bottom layers. In colour, every layer
has a print colour (copper red and blue, silkscreen black, …); black and
white draws everything in black.

![Top copper and silkscreen as printed](/screenshots/output-board-print.png)

### Assembly drawings

An assembly drawing shows where each part goes: the board outline, the pads
of that side in grey, the part outlines of the fabrication layer and the
designator in the middle of every part, sized to fit the part and turned
along long parts. **Values under the designators** adds the value. The bottom
side is drawn as seen from the bottom, so it matches the board turned over.

![The assembly drawing of the top side](/screenshots/output-assembly-top.png)

![The assembly drawing of the bottom side, seen from the bottom](/screenshots/output-assembly-bottom.png)

## Paper

| Paper                         | Use                                                                     |
| ----------------------------- | ----------------------------------------------------------------------- |
| **Size of the drawing (1:1)** | The page is as large as the drawing plus a margin; boards print at 1:1  |
| **A4**, **A3**, **Letter**    | The drawing is fitted onto the paper (landscape or portrait as it fits) |

## Text in prints

Texts are drawn with ElectronicWerk's stroke font, which has the same
metrics as the font of the viewer: a text covers the same area on paper as
on screen. PDF files also contain the texts as invisible, selectable text, so
designators, values and net names can be searched (`Ctrl+F`) and copied in
any PDF viewer.

## Other ways to print

- The [schematic export menu](./schematic-exports) of the sheet view writes
  the open sheet as SVG or all sheets as PDF, without an output job.
- The [Gerber viewer](./gerber-viewer) shows the manufacturing files
  themselves.
