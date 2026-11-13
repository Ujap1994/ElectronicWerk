# Online part libraries

A part that is not in the [base library](./library) can usually be found on
**SnapEDA** or **Ultra Librarian**, the two largest online part libraries:
symbol, footprint and 3D model, ready for KiCad. ElectronicWerk opens them
for you and imports what you download into the project library, with the 3D
model and a check against the base library.

![The online part libraries](/screenshots/parts-online.png)

## How it works

1. Search the part on the provider's website, in a **provider window** of
   ElectronicWerk or in your own browser.
2. Download it **in KiCad format** (KiCad 6 or newer). Both providers need a
   free account for downloads.
3. ElectronicWerk shows what the download contains and imports it into the
   project library.

ElectronicWerk does not search, scrape or download anything on the providers'
websites by itself: you browse and download, as on any website. This is what
both providers' terms of use allow (SnapEDA, for example, forbids automated
access). The downloaded files are licensed for use in your own designs:
SnapEDA's files under CC BY-SA 4.0 with a design exception (no attribution
needed on your boards), Ultra Librarian's for use in ECAD and MCAD tools. Read
the terms of each provider before you share libraries made from downloads:
SnapEDA does not allow passing on more than ten of its files to one place.

## Searching

Open the online part libraries with:

- **Find parts online (SnapEDA, Ultra Librarian)** in the
  [command palette](./command-palette),
- **Online parts** in the header of the [library manager](./library),
- **Search online (SnapEDA, Ultra Librarian)…** in the symbol chooser of the
  [schematic editor](./schematic-editor) (it takes over what you typed).

Type a part number or a keyword and choose **Search on SnapEDA** or **Search
on Ultra Librarian**. The provider's search page opens with your query in
the provider window. **In your browser** opens the same page in your own
browser instead.

### The provider window

The provider window is a separate, isolated browser:

- Each provider has its **own browser profile**, kept between sessions: sign
  in once and stay signed in. The sign-in stays inside that profile;
  ElectronicWerk never sees your password.
- The window runs **sandboxed**, without any access to ElectronicWerk, your
  files or your projects, and only opens https pages. Sign-in pop-ups (for
  example "Sign in with Google") open in the same profile.
- Every file you **download** there is saved into ElectronicWerk's download
  folder and offered for import right away. Other files (a datasheet PDF)
  are only saved; the notice names the folder.

## Importing a download

When a download arrives — or after **Import a download…** in the dialog for a
file downloaded with your own browser — the import dialog shows its content:

![Importing a SnapEDA download](/screenshots/parts-import.png)

- **Library name**: the prefix of the imported names, by default the
  provider (`SnapEDA:NE555-DEMO`, `UltraLibrarian:DBV0005A`). Change it to
  sort downloads into your own groups, for example `Timers`.
- **Into**: the project library the parts go to (if the project has more
  than one).
- **Symbols** and **Footprints** with their pins and pads. Parts that are in
  the library already are skipped.
- **3D models**: the STEP files of the download. They are copied into the
  project folder (`3d/<library name>/`), and the footprints point at them
  with `${KIPRJMOD}`, so the [3D view](./view-3d) and the
  [3D exports](./export-3d) find them and the project stays complete when you
  move or share it.
- **Problems**, for example a download that only has KiCad 5 libraries
  (`.lib`): download it again in the KiCad 6 (or newer) format.

**Import** adds everything in one step; **Undo** takes it back (the copied
3D files stay in the project folder).

### The base library's duplicates

Every symbol and footprint is compared with the [base library](./library)
like the [duplicate check](./duplicates) does:

| Note                                         | Meaning                                                           |
| -------------------------------------------- | ----------------------------------------------------------------- |
| identical to … in the base library           | The same pads and graphics: the same footprint under another name |
| same pads as … in the base library           | The same pads, other silkscreen or courtyard                      |
| pads within 0.05 mm of … in the base library | Almost the same land pattern                                      |
| same pins as … in the base library (symbols) | The same pins with another body                                   |

For footprints the dialog offers **Use … from the base library instead**
(already chosen for identical footprints): the downloaded footprint is left
out, the base library's footprint is added, and the downloaded symbol uses
it. A project then uses one footprint for one land pattern, wherever the
parts came from.

![An Ultra Librarian download with its own footprint and 3D model](/screenshots/parts-import-ul.png)

After the import, place the symbol from the symbol chooser like any other
part of the project library; its footprint and 3D model come along.

### What a download may contain

ElectronicWerk reads ZIP files and single files:

| File                                     | Imported as                                  |
| ---------------------------------------- | -------------------------------------------- |
| `.kicad_sym` (KiCad 6 or newer)          | Symbols                                      |
| `.kicad_mod`, also inside `.pretty`      | Footprints                                   |
| `.step`, `.stp` (and `.wrl`, `.wrz`)     | 3D models, matched to the footprints by name |
| `.lib`, `.dcm`, `.mod` (KiCad 5)         | Not imported (a problem is shown)            |
| Anything else (instructions, other CADs) | Ignored                                      |

The files can be anywhere in the ZIP: SnapEDA puts them in the root, Ultra
Librarian in a folder per CAD format (`KiCADv6`, `STEP`, …). A footprint
whose model is not named finds the only model of a download with a single
footprint. For safety, a ZIP may not unpack outside its folder, have more
than 2,000 entries or more than 512 MB.

## Accounts

The **Online part libraries** section of the [settings](./settings) lists both
providers:

![The online part libraries in the settings](/screenshots/parts-settings.png)

- **Sign in** opens the provider's sign-in page in its provider window.
- **Sign out** deletes the provider window's sign-in and everything the
  website stored (cookies, cache).
- **Terms of use** opens the provider's terms in your browser.

The status shows whether the provider window keeps a session.

## Command line

`ewerk import archive` imports a download into a project; `--dry-run` only
shows its content, `--use-library` takes the base library's footprint for
duplicates. See the [command-line reference](../reference/cli#ewerk-import-archive).
