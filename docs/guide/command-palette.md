# Command palette

The command palette lets you run any command by typing a few letters, without
searching through menus. It is the fastest way to work with ElectronicWerk and
grows with every new feature.

![The command palette with all commands](/screenshots/command-palette-all.png)

## Opening the palette

- <kbd>⌘</kbd>+<kbd>K</kbd> / <kbd>Ctrl</kbd>+<kbd>K</kbd>,
- <kbd>⌘</kbd>+<kbd>Shift</kbd>+<kbd>P</kbd> /
  <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>P</kbd> (familiar from code editors),
- **View → Command Palette…**,
- the <kbd>⌘K</kbd> / <kbd>Ctrl+K</kbd> button in the status bar.

## Finding a command

Start typing. The list shrinks to the commands that contain **every word** you
typed, in any order.

![Searching for "appearance"](/screenshots/command-palette-search.png)

- Upper and lower case do not matter.
- Accents and umlauts do not matter: `offnen` finds "Einstellungen öffnen".
- English words always work, even when the interface is German. For example,
  `dark` finds "Darstellung: Dunkel".
- Commands with a word that **starts** with your input are listed first.

If nothing matches, the palette says so:

![No matching commands](/screenshots/command-palette-empty.png)

## Running a command

| Key                         | Action                                       |
| --------------------------- | -------------------------------------------- |
| <kbd>↓</kbd> / <kbd>↑</kbd> | Move the highlight down or up (wraps around) |
| <kbd>Enter</kbd>            | Run the highlighted command                  |
| <kbd>Esc</kbd>              | Close the palette without running anything   |
| Mouse click                 | Run the clicked command                      |

The palette closes as soon as a command runs.

## Available commands

Project commands appear only while a project is open.

| Command                    | What it does                                                      |
| -------------------------- | ----------------------------------------------------------------- |
| New project                | Opens the [new-project wizard](./projects#creating-a-project)     |
| Open project               | Opens a project file                                              |
| Save project               | Saves the open project                                            |
| Save project as            | Saves the project into another folder                             |
| Save a copy of the project | Writes a copy and keeps working on the original                   |
| Archive project            | Writes a `.zip` archive                                           |
| Undo / Redo                | [Undo and redo](./undo-and-recovery)                              |
| Check project              | Runs the [project check](./projects#checking-a-project)           |
| Project properties         | Shows the [project properties](./project-tree#project-properties) |
| Add schematic sheet        | Adds a sheet below the top sheet                                  |
| Show project folder        | Reveals the project in your file manager                          |
| Close project              | Closes the project (asks about unsaved changes)                   |
| Open settings              | Opens the [settings](./settings)                                  |
| Check for updates          | Checks for a newer release ([Updates](./updates))                 |
| Language: German           | Switches the interface to German                                  |
| Language: English          | Switches the interface to English                                 |
| Language: system default   | Follows the system language                                       |
| Appearance: light          | Light appearance                                                  |
| Appearance: dark           | Dark appearance                                                   |
| Appearance: system default | Follows the system's light or dark mode                           |
| Open documentation         | Opens this documentation in your browser                          |
| Report an issue            | Opens the issue tracker on GitHub                                 |

## Example: switch to the dark appearance

1. Press <kbd>⌘</kbd>+<kbd>K</kbd> / <kbd>Ctrl</kbd>+<kbd>K</kbd>.
2. Type `dark`.
3. Press <kbd>Enter</kbd>.
