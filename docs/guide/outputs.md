# Output jobs

An **output job** collects every file a board maker or an assembly house
needs — Gerber and drill files, the bill of materials, the placement file,
PDF prints, test netlists — and generates all of them with one click. Every
project can have several jobs, for example one for the prototype service and
one for the assembly house, and each job remembers its own settings. Jobs
live in the project (`outputs/*.ewout`), so they travel with it and work the
same on every computer and in the [command line](../reference/cli#outputs).

![An output job with the settings of the Gerber output](/screenshots/outputs-job.png)

## Opening a job

New projects come with a job called **Fabrication**. Open it with

- the **Outputs** button in the project toolbar,
- a double-click on the job in the [project tree](./project-tree) (group
  **Output jobs**), or **Open** in its details,
- **Outputs: generate manufacturing files** in the
  [command palette](./command-palette).

To add another job, use **Add output job** in the menu of the **Output jobs**
group of the project tree. A new job starts with the
[generic preset](./ordering#presets).

## The editor

| Area                 | What it holds                                                                                                                                                         |
| -------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Toolbar**          | **Generate all**, the [Gerber viewer](./gerber-viewer), **Show in folder** and **Apply a preset…** for [board makers](./ordering).                                    |
| **Output folder**    | Where the files go, relative to the project folder (default `outputs/generated`). Files outside the project folder are not allowed.                                   |
| **Outputs** (left)   | The outputs of the job in the order they run. The check box switches an output on or off; the badge shows the result of the last run.                                 |
| **Settings** (right) | The settings of the selected output: its name, what it contains and its subfolder. **Generate this output** runs just this one.                                       |
| **Results** (bottom) | After a run: every output with its status and files. **Open** opens a file with its application (PDF viewer, spreadsheet, …); the arrow shows it in the file manager. |

Below the list, **Add output** adds an output of any kind; the arrows move the
selected output up and down, and the bin removes it. Every change is one
step in the project history, so **Undo** takes it back.

## Kinds of outputs

| Output                | Files                                                                | Details                                                                  |
| --------------------- | -------------------------------------------------------------------- | ------------------------------------------------------------------------ |
| **Gerber X2**         | One file per layer (`.gtl`, `.gbl`, `.gts`, …) and a Gerber job file | [Gerber and drill files](./gerber-drill#gerber-files)                    |
| **Excellon drill**    | `-PTH.drl`, `-NPTH.drl`, one file per blind or buried layer pair     | [Drill files](./gerber-drill#drill-files)                                |
| **Drill map**         | A drawing of every hole with a symbol per drill size (PDF or SVG)    | [Drill map](./gerber-drill#drill-map)                                    |
| **Bill of materials** | CSV, Excel workbook or HTML page                                     | [Bill of materials](./bom-placement#bill-of-materials)                   |
| **Pick and place**    | Positions and rotations of the parts (CSV)                           | [Placement files](./bom-placement#pick-and-place)                        |
| **PDF**, **SVG**      | Schematic sheets, board layers or assembly drawings                  | [Prints and drawings](./prints)                                          |
| **IPC-D-356 netlist** | The netlist for the electrical test of the bare board                | [Test netlist](./gerber-drill#ipc-d-356-netlist-for-the-electrical-test) |
| **IPC-2581**          | The whole board in one XML file                                      | [IPC-2581](./gerber-drill#ipc-2581)                                      |
| **Netlist**           | The schematic netlist (KiCad format or CSV)                          | [Netlists](./schematic-exports#netlists)                                 |
| **ZIP package**       | A ZIP archive of the files of chosen outputs                         | [Below](#zip-packages)                                                   |
| **3D model**          | The board with its parts as STEP, GLB or VRML                        | [3D models](./export-3d)                                                 |

## Generating

**Generate all** runs every output that is switched on, in order, and writes
the files into the output folder. A typical job takes well under a second.
When it is done, the results list every file:

![The results of a run](/screenshots/outputs-results.png)

- **Generated** — the files were written (existing files of the same name are
  replaced).
- **Skipped** — the output is switched off.
- **Failed** — the output could not be made; the reason is shown next to it,
  for example _the project has no board_. The other outputs still run.

The [project tree](./project-tree) lists the files under **Generated files**.
Generated files are not part of the project itself: [project archives](./projects#archiving)
leave them out, and the `.gitignore` of new projects lists the default output
folder.

::: tip Check before you order
Open the [Gerber viewer](./gerber-viewer) after generating: it shows the
Gerber and drill files exactly as the board maker will read them.
:::

## File names

Files are named after the board (Gerber, drill, placement, board prints) or
the project (BOM, schematic PDF), with characters that file systems do not
like replaced by `_`. Each kind has a default subfolder that can be changed
in its settings:

| Subfolder        | Outputs                                               |
| ---------------- | ----------------------------------------------------- |
| `gerber/`        | Gerber files, Gerber job file, drill files, drill map |
| `assembly/`      | Bill of materials, pick and place                     |
| `documentation/` | PDF and SVG prints                                    |
| `netlists/`      | IPC-D-356 and schematic netlists                      |
| (output folder)  | ZIP packages, IPC-2581                                |

## ZIP packages

A **ZIP package** packs the files of other outputs of the same run into one
archive — the file most board makers ask you to upload. Its settings choose
which kinds go into it (Gerber and drill by default) and the file name
(`{board}-fabrication.zip`, where `{board}` becomes the board name). The
package must come after the outputs it packs; the files are stored without
their subfolders.

## Several boards

A project with more than one board shows a **Board** choice in the settings
of every board output. **First board of the project** follows the order of
the project tree.

## From the command line

The same jobs run without the app, for example in a CI pipeline:

```sh
ewerk outputs run path/to/project               # the first job
ewerk outputs run path/to/project --job JLCPCB  # a job by name
ewerk outputs run path/to/project --output Gerber --output Drill
```

The exit code is 1 when an output fails. See the
[command reference](../reference/cli#outputs) and the
[engine API](../reference/engine-api#output-jobs) for scripts.
