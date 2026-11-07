# First start

When you open ElectronicWerk, the **start screen** appears. This page explains
everything you see on it.

![The start screen in the light appearance](/screenshots/start-screen-light.png)

## The start screen

### Header

The header shows the ElectronicWerk icon and a short welcome text.

### Actions

The three cards are the entry points into your work:

| Card                     | What it will do                                              | Available in |
| ------------------------ | ------------------------------------------------------------ | ------------ |
| **New project**          | Create a project with schematic and PCB (local or on GitHub) | v0.2         |
| **Open project**         | Open an existing ElectronicWerk project                      | v0.2         |
| **Import KiCad project** | Convert a KiCad project into an ElectronicWerk project       | v0.4         |

Cards for features that are not finished yet are greyed out and show the
version that will bring them.

### Recent projects

Projects you open will be listed here, so you can continue where you stopped.
Until then, the area shows a short hint.

## The status bar

The bar at the bottom of the window shows the state of the application.

![The status bar](/screenshots/status-bar-light.png)

From left to right:

1. **Engine status:** ElectronicWerk does all design work in a separate engine
   process (see [Architecture](../development/architecture)). The dot shows its
   state:

   | Dot    | Text (example)           | Meaning                                                               |
   | ------ | ------------------------ | --------------------------------------------------------------------- |
   | Green  | Engine 0.1.0 · 8 threads | The engine is running and answers requests.                           |
   | Yellow | Engine starting…         | The engine is starting or restarting (usually under 1 s).             |
   | Red    | Engine stopped: …        | The engine could not start. See [Troubleshooting](./troubleshooting). |

   The number of threads tells you how many processor cores the engine may use
   for heavy work such as design rule checks.

2. **Version:** the installed ElectronicWerk version. Please mention it when
   you report a problem.
3. **<kbd>⌘K</kbd> / <kbd>Ctrl+K</kbd>:** opens the
   [command palette](./command-palette).
4. **Gear:** opens the [settings](./settings).

## Language and appearance

ElectronicWerk starts in your system language if it is German or English, and
in English otherwise. The appearance follows your system's light or dark mode.
You can change both at any time in the [settings](./settings).

![The start screen in German](/screenshots/start-screen-german.png)

![The start screen in the dark appearance](/screenshots/start-screen-dark.png)

## Menus

| Menu                            | Contents                                                                           |
| ------------------------------- | ---------------------------------------------------------------------------------- |
| **ElectronicWerk** (macOS only) | About, Check for Updates…, Settings…, Hide, Quit                                   |
| **File**                        | New Project…, Open Project… (from v0.2); on Windows/Linux also Settings… and Quit  |
| **Edit**                        | Undo, Redo, Cut, Copy, Paste, Select All                                           |
| **View**                        | Command Palette…, zoom, full screen                                                |
| **Window** (macOS only)         | Minimize, Zoom                                                                     |
| **Help**                        | Documentation, Report an Issue; on Windows/Linux also Check for Updates… and About |

## Only one window at a time

ElectronicWerk runs as a single instance. If you start it again while it is
already open, the existing window comes to the front instead of a second copy
opening. Closing the window quits ElectronicWerk completely, on every operating
system (including macOS).

## If the engine restarts

In the unlikely case that the engine stops unexpectedly, ElectronicWerk starts
it again automatically within a moment. The window stays open and shows a
notice:

![Notice after an automatic engine restart](/screenshots/engine-restarted.png)

Click **×** to dismiss the notice. If the engine fails several times in a row,
the status bar turns red; see [Troubleshooting](./troubleshooting).
