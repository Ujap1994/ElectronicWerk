# Introduction

ElectronicWerk is an open-source ECAD suite for beginners and advanced
hobbyists. It takes inspiration from the workflow of Altium Designer, KiCad,
DipTrace and Eagle, but it is written from scratch and released under the
[MIT License](https://github.com/Ujap1994/ElectronicWerk/blob/main/LICENSE).

![A project open in ElectronicWerk](/screenshots/workspace-light.png)

## What you can do today

Version 0.11 adds the **online part libraries**: parts downloaded from
SnapEDA and Ultra Librarian are imported into the project with their 3D
models and checked against the base library. Version 0.10 added the **live
3D view**: the board with its laminate, copper,
solder mask, silkscreen and the 3D models of its parts, following every edit
at once, in a tab or in its own window for a second monitor, with sections,
measuring and screenshots, and **3D exports** as STEP (exact solids for
mechanical CAD), GLB and VRML. It builds on the **manufacturing outputs**
with output jobs and the Gerber viewer (0.9), the **layout editor** with
routing, zones and the design rules check (0.8), the **schematic editor**
(0.6) and the **library editors** (0.7), the **base library** (0.5), the
**KiCad import** (0.4), the **2D view** (0.3) and **projects** (0.2). Together
they take a design from the first symbol to the files a board maker needs
and the model the enclosure is designed around.

| Feature                                          | Status    | Page                                                   |
| ------------------------------------------------ | --------- | ------------------------------------------------------ |
| Installation on Windows, macOS and Linux         | Available | [Installation](./installation)                         |
| Start screen and status bar                      | Available | [First start](./getting-started)                       |
| Language (German/English) and appearance         | Available | [Settings](./settings)                                 |
| Command palette                                  | Available | [Command palette](./command-palette)                   |
| Update notice with release notes                 | Available | [Updates](./updates)                                   |
| Command-line tool `ewerk`                        | Available | [Command-line tool](../reference/cli)                  |
| Projects, project tree, properties               | Available | [Projects](./projects), [Project tree](./project-tree) |
| Undo, autosave and crash recovery                | Available | [Undo and recovery](./undo-and-recovery)               |
| Native, git-friendly file format                 | Available | [File format](../reference/file-format)                |
| 2D viewer for schematics and boards              | Available | [Viewing sheets and boards](./viewer)                  |
| KiCad import (projects and libraries)            | Available | [Importing KiCad projects](./import-kicad)             |
| Netlist comparison and import                    | Available | [Netlists](./netlists)                                 |
| Base library with search and 3D previews         | Available | [The base library](./library)                          |
| Duplicate detection and review                   | Available | [Duplicates](./duplicates)                             |
| Schematic editor and ERC                         | Available | [Drawing schematics](./schematic-editor), [ERC](./erc) |
| Symbol, footprint and component editors          | Available | [The library editor](./library-editor)                 |
| Layout editor, routing, zones                    | Available | [The layout editor](./layout-editor)                   |
| Design rules and DRC                             | Available | [Design rules](./design-rules), [DRC](./drc)           |
| Schematic ↔ board updates, cross-probing         | Available | [Schematic and board](./schematic-to-board)            |
| Output jobs: Gerber, drill, BOM, prints          | Available | [Output jobs](./outputs), [Ordering](./ordering)       |
| Gerber viewer                                    | Available | [The Gerber viewer](./gerber-viewer)                   |
| Live 3D view, STEP/GLB/VRML export               | Available | [The 3D view](./view-3d), [3D models](./export-3d)     |
| Online part libraries (SnapEDA, Ultra Librarian) | Available | [Online part libraries](./online-parts)                |

The [roadmap](https://github.com/Ujap1994/ElectronicWerk/blob/main/ROADMAP.md)
lists every milestone with its scope and status. It is updated with every
feature.

## How this documentation is organized

- **Getting started:** installing ElectronicWerk and a tour of the first start.
- **Using ElectronicWerk:** one page per feature, with screenshots and
  step-by-step instructions.
- **Reference:** the command-line tool and the engine API, for scripting.
- **Development:** architecture, building from source and the release process.

## Conventions

- Keyboard shortcuts are written for macOS and Windows/Linux, for example
  <kbd>Cmd</kbd>+<kbd>K</kbd> / <kbd>Ctrl</kbd>+<kbd>K</kbd>.
- Screenshots show the English user interface in the light appearance unless
  noted otherwise. All screenshots and examples use synthetic demo data.
- Menu paths are written as **Menu → Entry**. On macOS, application-wide entries
  such as **Settings…** are in the **ElectronicWerk** menu. On Windows and
  Linux, they are in the **File** and **Help** menus.

## Getting help

- Use **Help → Report an Issue** in the app, or open an issue on
  [GitHub](https://github.com/Ujap1994/ElectronicWerk/issues).
- Please include the version shown in the status bar (bottom right) and your
  operating system.
