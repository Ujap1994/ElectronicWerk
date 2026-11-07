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

## Updates

| Option                               | Effect                                                                           |
| ------------------------------------ | -------------------------------------------------------------------------------- |
| **Check for updates at startup**     | Looks for a newer release a few seconds after start (on by default).             |
| **Check now**                        | Checks immediately and always shows the result.                                  |
| **Skipped version** / **Show again** | Appears when you chose to skip a version. **Show again** brings its notice back. |

![The update section with a skipped version](/screenshots/settings-dialog-skipped.png)

The [Updates](./updates) page explains the update notice in detail.

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
  "updates": {
    "checkOnStartup": true,
    "skippedVersion": null
  }
}
```

ElectronicWerk validates the file every time it loads it. Unknown or invalid
values are replaced with the defaults, so a damaged file never prevents the
app from starting. To reset everything, quit ElectronicWerk and delete the file.
