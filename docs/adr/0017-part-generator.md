# ADR-0017: Part generator — parts from PDF datasheets

- **Status:** accepted
- **Date:** 2026-11-20
- **Milestone:** M16

## Context

The maintainer asked for parts to be made from their datasheet instead of
being drawn by hand: symbol, footprint and 3D model, with the user only
choosing the package. Users will mostly load **their own datasheets** of
any manufacturer, so the reading must hold up on real files, and a part
that already exists in a library must be named instead of made again —
but only when its package really matches.

The open decision of the roadmap was how the values are read: only on the
computer, or with AI help. The maintainer chose **local reading by
default, with optional help from the user's own AI agent through the MCP
server — no API key in the app**.

## Decision

### Reading PDF files: an own reader

A new crate `ew-datasheet` reads PDF files itself: cross-reference tables
and streams, object streams, broken files (a scan for `n g obj`), the
Flate, LZW, ASCIIHex, ASCII85 and RunLength filters with predictors, the
standard security handler (RC4, AES-128, AES-256 with the empty user
password — many datasheets are "protected" only against copying), simple
fonts with their encodings, `Differences` and the Symbol font, composite
fonts with CMaps and `ToUnicode`, form XObjects and rotated pages. It places
every character and every straight line of a page; nothing is rendered.

- **Why not a library**: `lopdf` would parse the file but not place text,
  which is the main work; it brings a large dependency tree (and another
  licence to allow). PDFium would need native binaries for five targets.
  The reader needs only `flate2` (already used), `aes`, `md-5` and `sha2`
  (MIT/Apache).
- **Coordinates** are points from the top left of the visible page with the
  page rotation applied — the space of a PDF viewer. The app shows the
  datasheet with **pdf.js** (Apache-2.0, loaded on first use); a box of the
  engine is the same box in pdf.js's viewport, so the place a value came
  from can be framed without conversion.
- Performance: a 653-page encrypted microcontroller datasheet reads in about
  0.3 s.

### Recognition: tables, drawings and the package's standard values

- **Tables** come from the drawn rules (a grid whose cells are joined where a
  rule is missing, for merged headers and rows) or from the alignment of
  the text under a header line (tables without rules, also over two pages).
- **Pin tables** are tables with a pin, number, ball or package column;
  columns are recognised by their headers and checked by their values. A
  column per package (`SOIC-8, MSOP-8 | DFN-8`) gives the pad numbers per
  package. Tables of electrical characteristics, registers, ports and
  ordering codes are refused by their headers and values.
- **Pinout drawings** of two-row packages are read when there is no table:
  runs of pad numbers down both sides, names outside, the package from the
  caption.
- **Packages** are recognised by name (`SOIC-8`, `SOT25`, `QFN-32 5x5`,
  `8-Pin SOIC` …) with family and pin count.
- **Dimension tables** with MIN/NOM/MAX (millimetres preferred, inches
  converted) are attached to their package. Manufacturer letters are mapped
  to JEDEC symbols by value against the package's standard dimensions
  (greedy, then improved by swaps).
- **Standard dimensions** of the common packages (JEDEC outlines, typical
  manufacturer values) fill in what no table gives; these values have no
  source and the app marks them, so the user checks them against the
  drawing. Reading dimensions off drawings was not done: it is unreliable
  and every drawing differs.
- Every value keeps its **source** (page and box). The result is a **draft**
  (JSON) — the one shape shared by the recognition, the app, the command
  line and AI agents.

### Generation: the existing tools

- **Footprints** come from the footprint wizards of M6 with IPC-7351 lands;
  M16 added wizards for SOT-23/SC-70 (odd lead counts), packages with a tab
  (SOT-223, SOT-89, DPAK, D²PAK), DIP, leads in a row (TO-92, TO-220) and an
  exposed pad for gull-wing packages. Names follow the KiCad libraries.
- **Symbols** are boxes laid out by pin type and group (supply on top,
  ground and unconnected pins at the bottom, inputs left, outputs right,
  groups such as `PA0…PA7` together), split into units above 64 pins.
- **3D models** are prisms (box bodies, gull-wing profiles, terminals, balls
  as twelve-sided columns, pins, tabs) written as exact B-reps with planar
  faces into a STEP file — solids for mechanical CAD and the enclosure check
  of M17, not meshes.
- Pins without a pad are left out; pads without a pin get an `NC` pin (or
  `EP` for an exposed pad or tab), each with a note. The library checks run
  on the result, and errors block the generation.
- `part.generate` stores symbol, footprint and component as **one undo
  step** and writes the model to `models/<footprint>.step` (a file outside
  the undo history, like downloaded models).

### Duplicates: the same part in nearly the same package

As the maintainer asked, a part counts as already there only when **both**
the name or part number match (an ordering suffix allowed) **and** the
footprint has nearly the same pads: the same numbers with centres within
0.15 mm after centring. Land sizes are not compared, since they follow the
IPC density. The same part in another package is only mentioned.

### AI help without an API key

The app never talks to an AI service. The draft is saved as a file
(`parts/<name>.part.json`); the user's agent reads the datasheet
(`datasheet_text`), corrects the draft (`part_draft_read`,
`part_draft_write`) through the MCP server of M12, and the app loads it
again. Agents can also make parts on their own (`datasheet_extract`,
`part_preview`, `part_generate`).

### Not now: text recognition (OCR)

Scanned datasheets are rare for current parts. OCR would need a large
engine and trained data for every platform, and its results on tables are
poor. Scanned pages are detected and named in the notes; the user enters
the values or lets the AI agent read the page images.

## Consequences

- New crate `ew-datasheet` (reader, recognition, draft, generation); new
  modules `ew-libedit::generate` (symbols) and `ew-board3d::package`
  (models); new wizard kinds `sot`, `tab`, `dip`, `inline`.
- New engine methods `datasheet.read`, `datasheet.page`, `datasheet.text`,
  `datasheet.extract`, `part.preview`, `part.duplicates`, `part.generate`,
  `part.draftSave`, `part.draftLoad`; the desktop engine host answers
  `datasheet.file` and `datasheet.previewModel`.
- New MCP tools and `ewerk part text|extract|generate`;
  `ewerk synth datasheets` writes two synthetic datasheets for tests and
  documentation — no real datasheet is in the repository.
- Notes and warnings carry codes, so the app translates them.
