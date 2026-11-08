# Introduction

ElectronicWerk is an open-source ECAD suite for beginners and advanced
hobbyists. It takes inspiration from the workflow of Altium Designer, KiCad,
DipTrace and Eagle, but it is written from scratch and released under the
[MIT License](https://github.com/Ujap1994/ElectronicWerk/blob/main/LICENSE).

![A project open in ElectronicWerk](/screenshots/workspace-light.png)

## What you can do today

Version 0.2 adds **projects**: create, open, organise, save, archive and check
projects with schematics, a board, a library, design rules and output jobs,
with unlimited undo, an autosave journal and crash recovery. The editors
follow in the next releases.

| Feature                                   | Status    | Page                                                   |
| ----------------------------------------- | --------- | ------------------------------------------------------ |
| Installation on Windows, macOS and Linux  | Available | [Installation](./installation)                         |
| Start screen and status bar               | Available | [First start](./getting-started)                       |
| Language (German/English) and appearance  | Available | [Settings](./settings)                                 |
| Command palette                           | Available | [Command palette](./command-palette)                   |
| Update notice with release notes          | Available | [Updates](./updates)                                   |
| Command-line tool `ewerk`                 | Available | [Command-line tool](../reference/cli)                  |
| Projects, project tree, properties        | Available | [Projects](./projects), [Project tree](./project-tree) |
| Undo, autosave and crash recovery         | Available | [Undo and recovery](./undo-and-recovery)               |
| Native, git-friendly file format          | Available | [File format](../reference/file-format)                |
| 2D viewer for schematics and boards       | Available | [Viewing sheets and boards](./viewer)                  |
| KiCad import                              | Planned   | v0.4                                                   |
| Schematic, symbol, footprint, PCB editors | Planned   | v0.6 to v0.8                                           |
| Live 3D view, manufacturing outputs       | Planned   | v0.9 and v0.10                                         |

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
