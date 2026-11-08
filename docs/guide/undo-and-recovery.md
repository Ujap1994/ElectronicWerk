# Undo, autosave and recovery

ElectronicWerk is built so that you never lose work: every change can be
undone, every change is recorded on disk immediately, and unsaved work is
restored after a crash, a power cut or a forced quit.

## Undo and redo

| Command | macOS                                      | Windows / Linux              |
| ------- | ------------------------------------------ | ---------------------------- |
| Undo    | <kbd>⌘</kbd>+<kbd>Z</kbd>                  | <kbd>Ctrl</kbd>+<kbd>Z</kbd> |
| Redo    | <kbd>⌘</kbd>+<kbd>Shift</kbd>+<kbd>Z</kbd> | <kbd>Ctrl</kbd>+<kbd>Y</kbd> |

Undo and redo are also on the toolbar and in the command palette. Hover over
the toolbar buttons to see what will be undone or redone:

![The undo button shows the next step](/screenshots/toolbar-undo-tooltip.png)

- **Everything is undoable:** project properties, variables, document names,
  paper and title block settings, adding and removing documents (including
  their sheet symbols). Each command is one step, even if it changes several
  things at once.
- **Unlimited steps:** the history is only limited by memory. If it grows past
  256 MB, the oldest steps are dropped; the newest step always stays.
- **Undo goes past saving:** you can undo changes you have already saved. If
  you undo or redo back to the saved state, the project counts as unchanged
  again.
- While you type in a text field, <kbd>⌘</kbd>/<kbd>Ctrl</kbd>+<kbd>Z</kbd>
  undoes your typing in that field first, as usual.

## Autosave journal

Every change is written to the **autosave journal**
(`.ewerk/journal.jsonl` in the project folder) the moment you make it, long
before you save. The journal records each change, undo and redo in order.
When you save, the journal is deleted because everything is now in the
project files.

You never need to look at the journal, but it is plain text and never leaves
your computer. `.ewerk/` is excluded from git by the project's `.gitignore`.

## Crash recovery

### If the engine stops

All design work happens in a separate engine process. If it ever stops
unexpectedly, ElectronicWerk starts it again within a moment, reopens your
project and replays the journal. You keep working where you were, including
your undo history, and a notice confirms it:

![The notice after an engine restart](/screenshots/engine-restart-restored.png)

### If ElectronicWerk was closed unexpectedly

If the whole application stopped (a crash, a forced quit, a power cut), the
journal is still there the next time. The start screen marks the project
with **Unsaved changes**:

![A recent project with unsaved changes](/screenshots/start-screen-recent.png)

When you open it, ElectronicWerk offers to restore the work:

![Restore unsaved changes?](/screenshots/recovery-dialog.png)

| Button              | Effect                                                                                                                                   |
| ------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| **Restore changes** | Replays the journal. The project shows every change from the last session, with its undo history, as unsaved changes. Save to keep them. |
| **Discard**         | Opens the project as it was last saved. The journal is kept as `.ewerk/journal.discarded.jsonl` until the next time you discard.         |

![The project after restoring](/screenshots/recovery-restored.png)

Until you decide, the project cannot be edited or saved, so the journal
cannot be overwritten by accident. If you close the project without deciding,
the offer is shown again next time.

If the project files were changed after the journal was written (for example
by a `git pull` or another program), the dialog warns you. ElectronicWerk then
tries to restore the changes, and if they no longer fit, nothing is changed
and an error explains why.

## How safe is saving?

- Files are written **atomically**: each file is written to a temporary file,
  flushed to disk and then renamed over the old file. A crash during saving
  leaves the old or the new file, never a mix.
- All files of one save are prepared first and only then put in place.
- Only changed documents are written; removed documents are deleted after the
  new files are in place.
- The journal is written for every change and flushed to disk at least once a
  second.

These guarantees are covered by automated tests, including a test that kills
a running engine process in the middle of editing and checks that every
change comes back.
