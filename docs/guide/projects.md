# Projects

Everything you design in ElectronicWerk belongs to a **project**, as in Altium
Designer and KiCad. A project is an ordinary folder on your disk that holds the
schematics, the board, the project library, the design rules and the output
jobs, plus any other files you keep with it (notes, datasheets, pictures).

![A project open in ElectronicWerk](/screenshots/workspace-light.png)

## What a project folder contains

```text
LED Blinker/
├── LED Blinker.ewproj        # Project file: name, revision, variables, document list
├── schematics/
│   ├── root.ewsch            # Top sheet of the schematic hierarchy
│   └── power-supply.ewsch    # Further sheets
├── pcb/
│   └── led-blinker.ewpcb     # The board: stack-up, outline, parts, tracks, zones
├── library/
│   └── project.ewlib         # Project library: symbols and footprints used by the design
├── rules/
│   └── design.ewrules        # Manufacturing limits and net classes
├── outputs/
│   ├── fabrication.ewout     # Output job: which manufacturing files to generate
│   └── generated/            # Generated files (not committed to git)
├── .gitignore                # Keeps .ewerk/ and outputs/generated/ out of git
└── .ewerk/                   # Local state: autosave journal, backups (never committed)
```

- The **project file** (`.ewproj`) lists every document. Double-click it (or
  open it with **File → Open Project…**) to open the project.
- Every document is a separate, readable JSON file. The files are written in a
  fixed order, so a project works very well with git: a change to one part
  changes one line. The format is described in the
  [file format reference](../reference/file-format).
- `.ewerk/` holds the [autosave journal](./undo-and-recovery) and backups.
  It is local to your computer and must not be committed.

## Creating a project

Click **New project** on the start screen, choose **File → New Project…**
(<kbd>⌘</kbd>/<kbd>Ctrl</kbd>+<kbd>N</kbd>) or run **New project** from the
[command palette](./command-palette). A wizard guides you through three steps.
You can click **Create project** on any step once the first step is complete;
the remaining steps keep their defaults.

### Step 1: Project

![Step 1: name and location](/screenshots/wizard-project.png)

| Field         | Meaning                                                                                                                                                                    |
| ------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Project name  | Any name up to 100 characters. It is shown in the title bar and used in title blocks as `${PROJECT_NAME}`.                                                                 |
| Location      | The folder in which the project folder is created. **Choose…** opens a folder dialog; you can also type or paste a path. ElectronicWerk remembers the last folder.         |
| Folder (hint) | The full path of the new project folder. Characters that are not allowed in file names are replaced by `_`.                                                                |
| Storage       | **On this computer** creates a local folder. **On GitHub** also puts it into a new or an existing empty GitHub repository, see [GitHub](./github#a-new-project-on-github). |

The project folder must not exist yet, or it must be empty. ElectronicWerk
never overwrites existing files.

### Step 2: Template

![Step 2: template](/screenshots/wizard-template.png)

| Template                 | Contents                                                                                              |
| ------------------------ | ----------------------------------------------------------------------------------------------------- |
| **Empty project**        | A top schematic sheet, the project library, design rules and an output job. Add a board later.        |
| **2-layer board**        | As above plus a 100 × 100 mm board with two copper layers and a standard 1.6 mm FR4 stack-up.         |
| **4-layer board**        | As above with four copper layers (prepreg – core – prepreg) for inner power and ground planes.        |
| **Arduino-style shield** | A 68.58 × 53.34 mm (2.7 × 2.1 in) shield outline with the four mounting holes as locked footprints.   |
| **Demo project**         | A small generated example with 50 parts, nets, tracks and vias, to explore the project view and tree. |

### Step 3: Board

![Step 3: board settings](/screenshots/wizard-board.png)

| Field           | Values                                                                                     |
| --------------- | ------------------------------------------------------------------------------------------ |
| Width, Height   | 5 to 2000 mm (entered in the chosen units). Fixed for the Arduino-style shield.            |
| Copper layers   | 1, 2, 4, 6, 8, 10, 12, 14 or 16. The stack-up is built for you and can be refined later.   |
| Board thickness | 0.2 to 10 mm. The dielectric layers are sized so that the total matches exactly.           |
| Units           | Millimetres, mils or inches. Used everywhere lengths are shown; you can change them later. |

Both a comma and a point work as decimal separator. Invalid values are marked
and the wizard does not continue until they are fixed:

![A value outside the allowed range](/screenshots/wizard-board-error.png)

## Opening a project

- **Open project** on the start screen, **File → Open Project…**
  (<kbd>⌘</kbd>/<kbd>Ctrl</kbd>+<kbd>O</kbd>) or the command palette: choose the
  `.ewproj` file.
- **Recent projects** on the start screen and **File → Open Recent**: the ten
  projects you opened last.
- **From your file manager:** double-click a `.ewproj` file, or drop it on the
  ElectronicWerk icon. If ElectronicWerk is already running, the project opens
  in the running window.

Only one project is open at a time. Opening another project first closes the
current one (and asks about unsaved changes).

### The recent projects list

![Recent projects on the start screen](/screenshots/start-screen-recent.png)

Each entry shows the project name, the path of the project file and when you
opened it last. Two badges can appear:

| Badge               | Meaning                                                                                                                                  |
| ------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| **Unsaved changes** | ElectronicWerk stopped while this project had unsaved work. Opening it [offers to restore](./undo-and-recovery#crash-recovery) the work. |
| **Not found**       | The project was moved or deleted. Remove the entry with **×**.                                                                           |

**×** removes an entry from the list (the project itself is not touched).
**Clear list** removes all entries.

### Projects from newer versions

Every file records the version of its format. ElectronicWerk opens projects of
all earlier versions; they are converted to the current format when you save
(the original files are kept in `.ewerk/backup/`). A project saved by a newer
ElectronicWerk cannot be opened; update the app in that case.

## Saving

| Command          | Shortcut                                                   | What it does                                                                                                                                  |
| ---------------- | ---------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------- |
| **Save**         | <kbd>⌘</kbd>/<kbd>Ctrl</kbd>+<kbd>S</kbd>                  | Writes all changed documents. Unchanged files are not touched.                                                                                |
| **Save as…**     | <kbd>⌘</kbd>/<kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>S</kbd> | Writes the whole project into a new folder (choose the parent folder) and continues working there. The old folder stays as it was last saved. |
| **Save a copy…** | –                                                          | Writes a copy into another folder and keeps working on the current project.                                                                   |
| **Archive…**     | –                                                          | Writes a `.zip` archive of the project (see below).                                                                                           |

Saving is **atomic**: every file is first written completely to a temporary
file and only then put in place. Even a power cut during saving leaves either
the old or the new version of each file, never a damaged one.

The toolbar shows whether there are unsaved changes:

![Unsaved changes in the toolbar](/screenshots/toolbar-dirty.png)

![Everything saved](/screenshots/toolbar-saved.png)

Removed documents are deleted from the disk when you save, not before, so
**Undo** can bring them back until then.

## Archiving

**Archive…** (toolbar, **File → Archive Project…** or the command palette)
writes a ZIP file with one folder containing the project file, every document
and all other files of the project folder. The archive contains the current
state, including unsaved changes. Generated outputs and the local `.ewerk/`
folder are left out.

![The notice after archiving](/screenshots/archive-notice.png)

## Closing

**Close project** (toolbar, **File → Close Project** or the command palette)
returns to the start screen. Closing the window quits ElectronicWerk. If there
are unsaved changes, you are asked first:

![Save changes before closing?](/screenshots/close-dialog.png)

| Button         | Effect                                                                                     |
| -------------- | ------------------------------------------------------------------------------------------ |
| **Save**       | Saves, then closes.                                                                        |
| **Don't save** | Closes without saving. The changes are kept once more in `.ewerk/journal.discarded.jsonl`. |
| **Cancel**     | Keeps the project open.                                                                    |

## Checking a project

**Check project** (toolbar, **File → Check Project**) checks the structure of
the project: missing or unreadable files, duplicate ids, sheets that include
themselves, references to nets, layers or library parts that do not exist,
invalid net classes and more.

![No problems found](/screenshots/validation-ok.png)

![A project with problems](/screenshots/validation-issues.png)

Each finding shows its severity, the file and a stable code (for example
`builtin-variable-shadowed`). The same check is available on the command line
as [`ewerk project validate`](../reference/cli#ewerk-project-validate).

## Working from the command line

Every project action is also available in the [`ewerk`](../reference/cli)
command-line tool, for scripts, continuous integration and AI agents:

```sh
ewerk project new "LED Blinker" --template board-2l --width 80 --height 50
ewerk project info "LED Blinker"
ewerk project validate "LED Blinker"
ewerk project archive "LED Blinker"
```
