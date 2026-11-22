# Settings

The settings dialog holds the options that apply to the whole application.
Every change takes effect immediately and is saved automatically; there is no
**Save** button.

![The settings dialog](/screenshots/settings-dialog.png)

## Opening the settings

Any of these opens the dialog:

- the **gear** icon at the bottom right of the status bar,
- **ElectronicWerk → Settings…** (macOS) or **File → Settings…** (Windows,
  Linux),
- the shortcut <kbd>⌘</kbd>+<kbd>,</kbd> / <kbd>Ctrl</kbd>+<kbd>,</kbd>,
- the [command palette](./command-palette) entry **Open settings**.

Close the dialog with **×**, the <kbd>Esc</kbd> key or by pressing
<kbd>Enter</kbd> on the close button.

## Language

| Option             | Effect                                                                               |
| ------------------ | ------------------------------------------------------------------------------------ |
| **System default** | Uses your operating system's language if it is German or English, otherwise English. |
| **Deutsch**        | German user interface.                                                               |
| **English**        | English user interface.                                                              |

The language changes instantly, including the menus. The documentation,
changelog and release notes are always in English.

## Appearance

| Option             | Effect                                                   |
| ------------------ | -------------------------------------------------------- |
| **System default** | Follows the light or dark mode of your operating system. |
| **Light**          | Always light.                                            |
| **Dark**           | Always dark.                                             |

The appearance also applies to native elements such as menus and dialogs.

## Working mode

How much ElectronicWerk decides for you where a task needs engineering
values:

| Option                 | Effect                                                                                                                               |
| ---------------------- | ------------------------------------------------------------------------------------------------------------------------------------ |
| **Beginner** (default) | You make a few choices (for example the interface a net class carries) and ElectronicWerk computes and applies the values.           |
| **Advanced**           | You see and set every value yourself (for example target impedance, tolerance, widths and gaps per layer); the app shows the result. |

The working mode changes what the [controlled impedance](./impedance#two-working-modes)
tab of the design rules shows. Both modes edit the same design rules, so you
can switch at any time.

## Mouse wheel zoom

How far one notch of the mouse wheel zooms schematics, boards, the library
previews and the other 2D views:

| Option               | Zoom per notch |
| -------------------- | -------------- |
| **Fine**             | 8 %            |
| **Normal** (default) | 15 %           |
| **Coarse**           | 30 %           |

A notch zooms one step however the wheel reports it: high-resolution wheels
that send a notch in several small parts zoom one step in total, and a fast
turn never runs far ahead of the picture. Trackpads are not affected: two
fingers pan, a pinch zooms (see [Navigating](./viewer#navigating)).

## Updates

| Option                               | Effect                                                                           |
| ------------------------------------ | -------------------------------------------------------------------------------- |
| **Check for updates at startup**     | Looks for a newer release a few seconds after start (on by default).             |
| **Check now**                        | Checks immediately and always shows the result.                                  |
| **Skipped version** / **Show again** | Appears when you chose to skip a version. **Show again** brings its notice back. |

![The update section with a skipped version](/screenshots/settings-dialog-skipped.png)

The [Updates](./updates) page explains the update notice in detail.

## GitHub

This section appears only when the [GitHub integration](./github) is
switched on (it is switched off in this version).

Your GitHub account for [GitHub and source control](./github): **Sign in to
GitHub…** opens the sign-in (browser or personal access token), **Sign out**
deletes the token from this computer. The lines below tell how you are
signed in, with **Choose repositories…** for the ElectronicWerk app, and how
the token is protected (system keychain, or a warning when no keyring is
available).

![The GitHub section of the settings](/screenshots/github-settings.png)

## Online part libraries

The accounts of the [online part libraries](./online-parts) SnapEDA and Ultra
Librarian: **Sign in** opens the provider's sign-in page in its provider
window, **Sign out** deletes the provider window's sign-in and stored data,
**Terms of use** opens the provider's terms. The sign-in itself is kept by
the provider window, never by ElectronicWerk.

![The online part libraries in the settings](/screenshots/parts-settings.png)

## AI agents and scripts

How AI agents and scripts use ElectronicWerk through the command-line tool
`ewerk`: the MCP configuration to copy into your agent (**Copy**), the
download of `ewerk` and the [guide](./automation).

![AI agents and scripts in the settings](/screenshots/automation-settings.png)

## Where the settings are stored

The settings are saved as `settings.json` in your user folder:

| System  | Folder                                         |
| ------- | ---------------------------------------------- |
| Windows | `%APPDATA%\ElectronicWerk`                     |
| macOS   | `~/Library/Application Support/ElectronicWerk` |
| Linux   | `~/.config/ElectronicWerk`                     |

Example content:

```json
{
  "schemaVersion": 1,
  "language": "system",
  "theme": "dark",
  "workMode": "beginner",
  "wheelZoom": "normal",
  "updates": {
    "checkOnStartup": true,
    "skippedVersion": null
  }
}
```

ElectronicWerk validates the file every time it loads it. Unknown or invalid
values are replaced with the defaults, so a damaged file never prevents the
app from starting. To reset everything, quit ElectronicWerk and delete the file.
