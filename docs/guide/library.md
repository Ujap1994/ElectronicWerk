# The base library

The base library brings the symbols, footprints and 3D models of the official
KiCad libraries into one searchable place: about 23,000 symbols, 15,000
footprints and 7,000 3D models. The library manager lets you search them in
milliseconds, look at every part in 2D and 3D, and copy what you need into
your project. Its **part catalog** sorts the parts into categories and
gives every part its package, so a resistor, an LED or a microcontroller is
one click from the schematic. It also finds duplicates, so the same part
shows up once instead of three times (see [Duplicates](./duplicates)).

![A footprint with its 3D model in the library manager](/screenshots/library-footprint-3d.png)

## Opening the library

- On the start screen, click **Library**.
- In a project, click the library icon (**Library**) in the toolbar.
- Choose **View → Library…** in the menu bar, or press
  <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>L</kbd> (<kbd>⌘</kbd>+<kbd>⇧</kbd>+<kbd>L</kbd>
  on macOS).
- Run **Open the library** in the [command palette](./command-palette).

![The library card on the start screen](/screenshots/library-start-action.png)

The library covers the whole window. Your project stays open underneath:
press <kbd>Esc</kbd> or click **×** to go back to it. The library remembers
the search, the filters and the selected entry until you quit the app.

## The part catalog

**Parts**, the first tab of the library, shows **complete parts**: a symbol
with its footprint (and the footprint's 3D model), ready to place.

![The part catalog: a resistor with its packages](/screenshots/part-catalog.png)

### Categories

The tree on the left sorts the parts by what they are, with the number of
parts in each category:

| Category                    | Contains                                                                                                                                                          |
| --------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Passive parts**           | Resistors, capacitors, inductors and transformers, crystals and resonators, fuses and protection, filters                                                         |
| **Discrete semiconductors** | Diodes, LEDs, transistors, thyristors and triacs                                                                                                                  |
| **Integrated circuits**     | Microcontrollers (by vendor), processors, FPGAs and CPLDs, memory, power, amplifiers, ADCs and DACs, interfaces, drivers, logic, timers, sensors, audio and video |
| **RF and wireless**         | Wireless modules, RF amplifiers, mixers, antennas                                                                                                                 |
| **Connectors**              | Pin headers and generic connectors; USB, audio and other connectors                                                                                               |
| **Electromechanical**       | Switches and buttons, relays, motors, displays, buzzers and speakers, batteries                                                                                   |
| **Modules**                 | Development boards and modules (Arduino, …)                                                                                                                       |
| **Mechanical parts**        | Mounting holes, fiducials and the like                                                                                                                            |

Click a category to see its parts; its subcategories open below it.
**All parts** shows every part (power symbols and simulation sources are not
parts and are left out). The search searches inside the chosen category:
`3.3` in **Power** finds the 3.3 V regulators, `STM32F401` in
**Microcontrollers › ST** the matching microcontrollers. A part named exactly
like the search comes first (`LED` before the many LEDs whose description
says "LED").

![The categories of the catalog](/screenshots/part-categories.png)

The categories come from the names of the KiCad libraries (`MCU_ST_STM32F4`,
`Regulator_Linear`, `Sensor_Humidity` …) and, for KiCad's general `Device`
library, from the symbol names (`R`, `C_Polarized`, `LED`, `Q_NPN_BCE` …);
see [ADR-0020](https://github.com/Ujap1994/ElectronicWerk/blob/main/docs/adr/0020-part-catalog.md).

### Packages

On the right, the catalog shows the part's symbol, its **package** and the
package's footprint in 2D and 3D.

- A part whose symbol names its footprint — about four of five KiCad
  symbols, most ICs among them — **comes with that package**: an
  STM32F401RETx is an LQFP-64.
- A **basic part** — resistor, capacitor, inductor, LED, diode, crystal,
  fuse, generic pin header — lets you **choose the package**. The list shows
  the footprints of the base library that KiCad's footprint filters of the
  symbol allow and that have as many pads as the symbol has pins. The
  **common packages** come first (resistors: 0603, 0805, 0402, 1206, 2512
  and axial; electrolytic capacitors: 5 × 5.4 mm, 6.3 × 5.4 mm …; a 1 × 4
  header: 2.54 mm pin header, socket, JST XH, JST PH, 1.27 mm), then every
  other footprint that fits. Each entry says whether it is SMD or
  through-hole and whether it has a 3D model.

![A complete part: a microcontroller with its own package](/screenshots/part-catalog-mcu.png)

### Adding and placing

- **Add to project** copies the symbol, the chosen footprint and a
  **component** that ties them together into the project library, as one
  step you can undo. The component is named after its package
  (`R_0603_1608Metric`, `Conn_01x04_PinHeader_1x04_P2.54mm_Vertical`); a part
  already in the project library is not added twice.
- **Place in schematic** (with a schematic open) adds the part the same way,
  closes the library and lets you place it: it hangs at the pointer, a click
  puts it down with its footprint already assigned. Set the value (`10k`,
  `100n`) in the properties.

The schematic's place dialog (<kbd>A</kbd>) leads here as well: **Part
catalog…** opens the catalog with what you typed.

## Setting up the library

The first time you open it, the library is empty. There are two ways to fill
it:

![Setting up the library](/screenshots/library-setup.png)

### Download the KiCad library

Click **Download the KiCad library**. ElectronicWerk downloads the library
index (about 40 MB) from the
[GitHub releases](https://github.com/Ujap1994/ElectronicWerk/releases) of this
project and installs it. The card shows how many symbols, footprints and 3D
models the release contains and how much is downloaded now.

The 3D models are much larger (more than 3 GB as STEP files). They are
downloaded **when you first look at a model**, one 3D library at a time (for
example all resistor models at once, a few megabytes). After that they work
offline.

Every download is checked against the SHA-256 checksum in the release's
manifest before it is installed. A damaged download is discarded and the
previous library stays as it was.

### Use KiCad libraries from this computer

If KiCad is installed, the card lists its libraries. Click **Use these** to
build the library from them. You can also click **Choose a folder…** and
select:

- a KiCad installation's library folder (with `symbols`, `footprints` and
  `3dmodels`), or
- a folder with checkouts of the KiCad library repositories
  (`kicad-symbols`, `kicad-footprints`, `kicad-packages3D`).

Building takes a few seconds; a progress bar shows the stages (symbols,
footprints, 3D models, index and duplicates). The 3D models are converted
from STEP when you first look at them (see [3D models](#3d-models)).

| KiCad installation | Library folder                                                                                        |
| ------------------ | ----------------------------------------------------------------------------------------------------- |
| Windows            | `C:\Program Files\KiCad\<version>\share\kicad`                                                        |
| macOS              | `/Applications/KiCad/KiCad.app/Contents/SharedSupport`                                                |
| Linux              | `/usr/share/kicad` (Flatpak: `/var/lib/flatpak/app/org.kicad.KiCad/current/active/files/share/kicad`) |

### Where the library is stored

The library lives in the `library` folder of the app's settings folder
(Windows: `%APPDATA%\ElectronicWerk\library`, macOS:
`~/Library/Application Support/ElectronicWerk/library`, Linux:
`~/.config/ElectronicWerk/library`). **About this library** shows the exact
path. The folder holds the index (`library.sqlite`), your duplicate
decisions (`decisions.json`), the deduplication report and the 3D mesh cache
(`meshes/`). You can delete it at any time to start over.

## Searching

Type into the search field. The list updates as you type; the line below the
field shows the number of results and how long the search took (usually a
few milliseconds, even over the whole library).

- Every word must match the **start** of a word in the name, library,
  description, keywords, value, package, part number, manufacturer or
  parameters: `res 0805` finds `R_0805_2012Metric` with the description
  "Resistor SMD 0805".
- Words with punctuation are matched as a phrase: `SOT-23` finds `SOT-23` and
  `SOT-23-5`, but not every entry with "SOT" and "23" somewhere.
- Upper and lower case and accents do not matter.
- Names count most, then part numbers, values and packages, then keywords,
  libraries and descriptions. Shorter names come first among equals.
- With an empty search field, the list shows everything (filtered), sorted by
  library and name.

### Filters

![Filters](/screenshots/library-filters.png)

| Filter                                     | Effect                                                                                                                                    |
| ------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------- |
| **All / Symbols / Footprints / 3D models** | Shows one kind of entry.                                                                                                                  |
| **Mounting** (footprints)                  | SMD, through-hole or other.                                                                                                               |
| **Only with a 3D model** (footprints)      | Footprints that reference a 3D model.                                                                                                     |
| **Only entries with duplicates**           | Entries that belong to a duplicate group of any class.                                                                                    |
| **Show merged duplicates**                 | Also lists entries that were merged into another one (they are hidden by default).                                                        |
| **Libraries**                              | One or more libraries of the selected kind, with the number of entries in each. Type into **Filter libraries** to find a library quickly. |

**Clear filters** resets all filters except the kind.

### Keyboard

| Key                                       | Effect                            |
| ----------------------------------------- | --------------------------------- |
| <kbd>↑</kbd> / <kbd>↓</kbd>               | Select the previous or next entry |
| <kbd>Page Up</kbd> / <kbd>Page Down</kbd> | Move by one page                  |
| <kbd>Home</kbd> / <kbd>End</kbd>          | First or last entry               |
| <kbd>Esc</kbd>                            | Close the library                 |

The list is virtualized: only the visible rows are loaded, so scrolling
through tens of thousands of entries stays smooth.

## Looking at an entry

Select an entry to see its preview and details on the right.

![A symbol in the library](/screenshots/library-symbol.png)

- **Symbols** are drawn like on a sheet. Symbols with several units have a
  unit selector.
- **Footprints** have a **2D** view (pads, silkscreen, fabrication and
  courtyard layers) and, if they reference a model, a **3D** view of the
  model on a small piece of board, placed with the footprint's offset,
  rotation and scale.
- **3D models** are shown on their own.

The 2D preview uses the same view as sheets and boards: zoom with the
wheel, pan by dragging with the middle or right mouse button (see
[Viewing sheets and boards](./viewer#navigating)). In 3D, drag to turn the
model, scroll to zoom and drag with the right mouse button to move it;
the button at the top right resets the view.

The details list everything the library knows about the entry:

| Field                     | Content                                                                                         |
| ------------------------- | ----------------------------------------------------------------------------------------------- |
| Description, Keywords     | From the source library.                                                                        |
| Value, Footprint          | The symbol's default value and footprint.                                                       |
| Part number, Manufacturer | From the symbol's properties (`MPN`, `Manufacturer`, and similar names).                        |
| Pins / Pads, Units        | The number of distinct pin or pad numbers, and the symbol's units.                              |
| Size                      | Footprints: the courtyard (or the pads, if there is no courtyard). 3D models: the bounding box. |
| 3D model, File            | The model a footprint uses, and the model's file in its 3D library.                             |
| Parameters                | Every other property, including KiCad's footprint filters.                                      |
| Merged duplicates         | Entries merged into this one (see [Duplicates](./duplicates)).                                  |
| Duplicate groups          | The groups the entry belongs to, with their class and reason.                                   |
| Source                    | The source library, its version and license.                                                    |

**Copy name** copies the full name (`Library:Name`) to the clipboard.

### 3D models {#3d-models}

![A 3D model](/screenshots/library-model.png)

ElectronicWerk shows 3D models as triangle meshes in the GLB format. The
KiCad libraries contain STEP models, which are converted once with
[OpenCASCADE](https://dev.opencascade.org/) and then cached:

- In a **downloaded** library, the meshes are part of the release and are
  downloaded with the mesh pack of their 3D library.
- In a library built from **local folders**, the STEP file is converted the
  first time you look at the model. This takes from a fraction of a second up
  to a few seconds for very detailed models; the status line under the
  preview says what is happening.

Colours come from the STEP files. Light, unsaturated colours (pins, metal
cases) are drawn slightly metallic.

## Adding parts to a project

With a project open, **Add to project library** copies the selected symbol
or footprint into the project's library. For a symbol, its default footprint
is copied as well, if the base library has it. Entries the project library
already has are skipped, and a notice says what happened. The change is one
step in the history: **Undo** removes everything that was added.

![Adding a symbol to the project library](/screenshots/library-add.png)

A part the base library does not have can be downloaded from SnapEDA or
Ultra Librarian: **Online parts** in the header opens the
[online part libraries](./online-parts). Their downloads are checked
against the base library when they are imported.

## About this library

**About this library** shows the numbers of symbols, footprints and 3D
models, the source and license, the duplicate groups, the folder and the
size of the index.

![About this library](/screenshots/library-info.png)

- **Check for a newer library** looks for a newer library release on GitHub
  and offers to download and install it. Your duplicate decisions are kept.
- **Build from KiCad libraries…** opens the setup to rebuild the library from
  local folders (for example after updating KiCad).
- **Show the deduplication report** reveals `dedup-report.md` in your file
  manager.

## License

The KiCad libraries are licensed under
[CC-BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/) with the KiCad
libraries exception: designs that use them, and files generated from them
(Gerber, BOM, …), may be released under any license. The downloaded library
is published separately from the app under the same license, with a
`LICENSE.md` that names the source and the changes (conversion, meshes,
deduplication). The ElectronicWerk app itself is MIT-licensed.

## Command line

The command-line tool builds, searches and distributes the library, for
example in scripts or CI:

```sh
ewerk library build --kicad /usr/share/kicad
ewerk library search "SOT-23" --kind footprint
ewerk library duplicates --markdown > report.md
```

See [`ewerk library`](../reference/cli#library) for all options.
