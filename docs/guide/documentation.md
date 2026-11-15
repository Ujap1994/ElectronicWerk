# Project documentation

The **Project documentation (HTML)** output writes the whole design into a
single HTML file: an overview, the schematic sheets, views of the board,
the assembly drawings, the layer stack-up, the bill of materials, the holes
and the [design statistics](./statistics). Open it in any browser, send it
to a colleague or a customer, print it, or save it as PDF from the
browser's print dialog.

![The first page of a generated documentation](/screenshots/documentation-top.png)

The file is self-contained: the drawings are embedded as vector images
(SVG), so it needs no other files and no internet connection, and you can
zoom into every drawing without losing sharpness.

## Adding the output

The documentation is an output of an [output job](./outputs):

1. Open the output job (**Outputs** in the toolbar, or the job in the
   project tree).
2. Choose **Project documentation (HTML)** under **Add output**.
3. Choose the sections (all are switched on for a new output).
4. **Generate this output**, or **Generate all** for the whole job.

![The settings of the documentation output](/screenshots/documentation-output.png)

The file is written to `documentation/<project>-documentation.html` in the
job's output folder; **Subfolder** changes `documentation`. When the
project has several boards, **Board** chooses the board shown in the board
sections (the first board by default).

## Sections

| Section                          | Content                                                                                                                                                                                      |
| -------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Overview** (always)            | Project name, revision, date, and the key figures: sheets, parts, nets, BOM lines, board size, copper layers, footprints and how much is routed. A table of contents links to every section. |
| **Schematic sheets**             | Every sheet as a drawing with its frame and title block, text variables filled in (`${SHEET_NAME}`, `${REVISION}`, …).                                                                       |
| **Board views (top and bottom)** | The board from the top (top copper and silkscreen) and from the bottom (bottom copper and silkscreen, mirrored, the way you see it when you turn the board over).                            |
| **Assembly drawings**            | The [assembly drawings](./prints) of both sides with designators and values.                                                                                                                 |
| **Layer stack-up**               | The layers of the board with their material and thickness.                                                                                                                                   |
| **Bill of materials**            | One line per part type: quantity, designators, value, footprint and part number.                                                                                                             |
| **Holes by diameter**            | Plated and non-plated holes per drill size.                                                                                                                                                  |
| **Design statistics**            | The [design statistics](./statistics) of the schematic, the bill of materials and the board.                                                                                                 |

![The board views in the documentation](/screenshots/documentation-board.png)

![The design statistics at the end of the documentation](/screenshots/documentation-statistics.png)

The headings and labels of the documentation are in English, whatever the
language of the program, so the file reads the same for everyone you
send it to.

## Printing and PDF

The documentation is laid out for the screen and for paper: when you print
it, every section starts on a new page and drawings are never split
across two pages. To get a PDF, open the file in a browser, choose
**Print** and **Save as PDF**.

For a PDF of the drawings only, use the [PDF output](./prints) instead:
it writes the sheets and board layers on pages of a chosen paper size.

## From the command line

The documentation is generated with the other outputs of the job:

```sh
ewerk outputs run "Sensor Board" --job Fabrication
```

See [`ewerk outputs run`](../reference/cli#ewerk-outputs-run). In the job
file, the output has the kind `documentation`; its options are listed in
the [file format reference](../reference/file-format#output-jobs).
