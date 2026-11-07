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

| Command                    | What it does                                      |
| -------------------------- | ------------------------------------------------- |
| Open settings              | Opens the [settings](./settings)                  |
| Check for updates          | Checks for a newer release ([Updates](./updates)) |
| Language: German           | Switches the interface to German                  |
| Language: English          | Switches the interface to English                 |
| Language: system default   | Follows the system language                       |
| Appearance: light          | Light appearance                                  |
| Appearance: dark           | Dark appearance                                   |
| Appearance: system default | Follows the system's light or dark mode           |
| Open documentation         | Opens this documentation in your browser          |
| Report an issue            | Opens the issue tracker on GitHub                 |

## Example: switch to the dark appearance

1. Press <kbd>⌘</kbd>+<kbd>K</kbd> / <kbd>Ctrl</kbd>+<kbd>K</kbd>.
2. Type `dark`.
3. Press <kbd>Enter</kbd>.
