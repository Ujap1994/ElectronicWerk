# ADR-0004: KiCad import

- **Status:** accepted
- **Date:** 2026-11-08
- **Milestone:** M3

## Context

Most hobbyists who try ElectronicWerk already have KiCad projects. The
roadmap requires importing KiCad 10 projects (and older versions where
possible) completely: schematics with their hierarchy, symbols, boards,
footprints, design rules, library tables and 3D model references, plus
netlists from KiCad and other tools. The acceptance test for the milestone is
a real 4-layer, multi-sheet KiCad 10 project that must import without errors
and match the original visually and electrically.

KiCad's files are S-expressions. Large boards are 10 to 20 MB, and the
official libraries hold more than 20,000 symbols and 15,000 footprints in
several thousand files, so parsing must be fast. KiCad's coordinate systems
differ between file kinds: library symbols use y up, sheets and boards y
down; footprint items use local coordinates, and bottom-side footprints are
stored already flipped. KiCad 10 also changed details: nets are referenced by
name instead of by code, and layer ids were renumbered.

## Decision

### A separate crate, a one-way conversion

The import lives in its own crate, `ew-kicad`. It reads KiCad files and
produces an `ew-model` project plus an **import report**. It does not depend
on the engine, so the CLI, the desktop app and tests use it the same way, and
the engine wraps it in the methods `import.kicad`, `import.kicadProbe`,
`import.library` and `import.netlist`.

The conversion is one-way: KiCad files are never written. Imported projects
are ordinary ElectronicWerk projects in the native format
([ADR-0002](./0002-data-model-and-file-format.md)); there is no hidden link
back to the KiCad files. Importing again creates a new project.

### Parsing

A hand-written, non-recursive S-expression parser makes one pass over the
text and borrows every atom and every string without escapes from the input
(`Cow`). Accessors (`get`, `value`, `number`, `flag`) hide the differences
between versions, for example `hide` as a bare word (KiCad 6/7) or as
`(hide yes)` (KiCad 8 and later). A 13 MB board parses in well under a second,
and all official libraries in a few seconds.

### Mapping

- **Coordinates** are converted exactly from millimetre decimals to
  nanometres, and from KiCad's y-down sheets and boards to ElectronicWerk's
  y-up coordinates (with the page or board frame height as reference).
- **Layers are mapped by name** (`F.Cu`, `In1.Cu`, `Edge.Cuts`, `User.1`, …),
  never by number, so the renumbering in KiCad 10 does not matter.
- **Nets** are resolved by name (KiCad 10) or by code with the top-level net
  declarations (earlier versions).
- **Footprints** are converted to ElectronicWerk's convention: stored as seen
  from the top, flipped at render time. Pad and text angles, which KiCad
  stores as absolute angles, become relative to the footprint.
- **Symbols** keep KiCad's transform semantics: KiCad's orientation matrices
  map to a mirror flag plus a rotation. Fields are placed like KiCad does:
  the text box is laid out in the unrotated symbol frame, transformed with
  the symbol, and drawn centred, with the reading direction swapped for
  symbols turned by 90° or 270°.
- **Ids** are KiCad's UUIDs where they are unique. Objects without a UUID, or
  with a UUID used more than once, get deterministic ids derived from their
  content (FNV-1a, formatted as UUID version 8), so importing the same
  project twice gives the same ids.
- **Components** link a placed symbol and its footprint: the footprint's
  `path` is the sheet path of the symbol plus the symbol's UUID, and both get
  the same component id.
- **Libraries:** the symbols and footprints the design uses come from the
  design itself (the schematic's symbol cache and the board's footprints).
  Library table entries inside the project folder (`${KIPRJMOD}`) are imported
  into the project library; external libraries are noted in the report, not
  copied.
- **Rules:** net classes, assignments by pattern and board constraints come
  from the `.kicad_pro` file. Custom rules (`.kicad_dru`) are counted in the
  report until the rule editor exists.

### The report

Everything that cannot be mapped exactly becomes a **finding** with a
severity (error, warning, note), a stable code, the file and a message.
Findings with the same code, file and message are merged with a count. The
desktop app shows the report after the import and translates the titles by
code; the engine writes it into the project as Markdown
(`kicad-import-report.md`). Unknown item kinds never fail an import; only
unreadable or missing sheets and boards are errors.

### Netlists

Netlists (KiCad `.net`, KiCad XML, CSV with header detection) are compared by
**connectivity**, not by name: two nets match if they connect the same pins.
Single-pin nets mean "not connected" and are only differences if the board
connects the pin. Applying a netlist assigns pad nets and creates missing
nets as one undoable transaction.

### Tests

The repository contains a synthetic KiCad 10 project (a 555 blinker with a
sub-sheet, a 4-layer board, a project library, rules and netlists in all
three formats), generated by `scripts/make-kicad-fixture.py`. It is used by
unit, integration and end-to-end tests and for the documentation. Real
projects and the official libraries are tested locally with ignored tests
(`EW_KICAD_SAMPLE`, `EW_KICAD_LIBRARIES`); they are never committed.

## Consequences

- KiCad users can move a project over in one step and check the result
  electrically with KiCad's own netlist.
- The import is limited to what the data model can express. Dimensions,
  text boxes, images, alternate body styles and custom rules are reported
  and follow with the editors that need them.
- KiCad 5 schematics (`.sch`, a different format) are not supported.
- DipTrace import is deferred; other open formats can reuse the report and
  netlist parts.
