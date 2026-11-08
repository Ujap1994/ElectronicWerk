# ADR-0005: Base library and deduplication

- **Status:** accepted
- **Date:** 2026-11-08
- **Milestone:** M4

## Context

Requirement F2 asks for the base libraries to be checked for duplicates and
merged into one library. The first source is KiCad (about 23,000 symbols,
15,000 footprints and 7,000 3D models, more than 3 GB of STEP files); online
parts (M10) and further formats (M14) follow and must plug into the same
engine. The roadmap sets the acceptance criteria: a search over the whole
base library answers in less than 50 ms, and the deduplication report is
reproducible. The 3D models must be shown without making the installer
3 GB larger.

Since KiCad 10, the official 3D library contains STEP files only (no WRL),
so showing a model means tessellating STEP. The official libraries live on
GitLab; the GitHub mirrors stopped in 2020.

## Decision

### An index in SQLite with FTS5

The crate `ew-library` keeps the library in one SQLite database
(`library.sqlite`, through `rusqlite` with the bundled SQLite):

- `items`: one row per symbol, footprint or model with its search fields
  (name, library, description, keywords, value, package, part number,
  manufacturer, parameters), counts, size, model links and the id of the
  entry it is merged into;
- `blobs`: the full definitions in the native data model, deflated and
  stored without their ids (which are derived from the name when read);
- `groups` and `members`: the duplicate groups with class, reason, decision
  and shown entry;
- `search`: a contentless FTS5 table (`unicode61`, diacritics removed,
  prefix indexes for 2 and 3 characters).

User input becomes FTS5 prefix phrases (`SOT-23` → `"sot 23"*`), ranked by
BM25 with weights (name highest, then part number, value, package). The
index is built in one transaction from sorted entries, so ids and order are
stable. Searches over the KiCad libraries take 0.1 to 15 ms.

### Source-independent fingerprints

Sources are read into `Entry` values (`ingest`, KiCad first, through
`ew-kicad`). Every entry gets a fingerprint of a canonical form:

- footprints: pads and graphics rounded to 1 µm, centred on the pads and
  turned by the quarter turn with the smallest canonical text, pad rotations
  normalized for symmetric shapes;
- symbols: pins per unit (number, name, type, position, direction, length)
  and the body;
- 3D models: the triangles of the GLB mesh rounded to 1 µm, sorted and with a
  normalized winding start, with and without colours.

### Three classes, decided once

`dedup` groups entries into **exact duplicates** (same canonical form;
symbols also need the same name), **near duplicates** (footprints with the
same pads or pads within 0.05 mm; symbols with the same name and pin numbers;
models with the same mesh or the same bounding box and triangle count) and
**aliases** (different symbol names with one drawing). Near groups are
connected components of these relations. Exact groups are merged by default,
near groups wait for a review, aliases stay separate.

A group's key is a hash of its kind, class and sorted member names.
Decisions are stored by key in `decisions.json` next to the index, so they
survive rebuilds and library updates; a group with other members is a new
group. Merged entries stay in the index: they are hidden in searches and
found through the shown entry's alias column.

Symbols with the same pins but another body are deliberately not near
duplicates: in the KiCad libraries this rule put 498 diode symbols into one
group.

### Reproducible results

Reading is parallel (rayon), but entries are sorted before anything depends
on their order, groups are sorted by kind, class and their best member, and
the reports contain no time stamps. Two builds from the same libraries give
byte-identical `dedup-report.json` files; a test checks this for the
synthetic library and, locally, for the KiCad libraries.

### 3D models: STEP → GLB with OpenCASCADE (WebAssembly)

As planned in ADR-0001, STEP models are tessellated with OpenCASCADE through
its WebAssembly build `occt-import-js` (LGPL-2.1, loaded at run time). A
shared TypeScript module (`apps/desktop/src/shared/mesh.ts`) converts the
result to GLB (metres, y up, one primitive per colour, quantized normals).
It runs in two places:

- `apps/desktop/scripts/tessellate.ts` converts a whole 3D library in worker
  threads for the library build (7,257 KiCad models in 7.4 minutes on eight
  cores, 29 million triangles, 656 MB of GLB);
- a worker thread of the engine host converts single STEP files on demand
  (`mesh.tessellate`) for libraries built from local folders, caching the
  result.

The Rust index only reads GLB files (fingerprints, bounding boxes); it does
not depend on OpenCASCADE. The 3D preview uses three.js, as ADR-0001 chose
for the 3D view, loaded on demand.

### Distribution as packs

The library is not bundled with the app. The **Library** workflow builds it
from the KiCad libraries on GitLab and publishes a release
`library-kicad-<version>-r<run>`, never marked as latest: an index pack
(~42 MB), one mesh pack per 3D library (105 packs, ~220 MB in all) and a
`manifest.json` with sizes and SHA-256 checksums. The app downloads the index
when the library is set up and a mesh pack the first time a model of that 3D
library is shown. Users can also build the library from their own KiCad
installation.

### License

The KiCad libraries are CC-BY-SA 4.0 with a design exception. The library
release is published separately from the app with a `LICENSE.md` that names
the source and the changes; the app stays MIT. `occt-import-js` is loaded as
a separate WebAssembly file and keeps its LGPL-2.1 license.

## Consequences

- The library manager can search the whole base library interactively and
  show every part in 2D and 3D.
- New sources only need an ingest step that produces entries; fingerprints,
  groups, decisions, search and packs work unchanged.
- The tessellation settings are part of the mesh fingerprints: changing them
  changes the 3D groups (once, with the next library release).
- The index format has a version (`meta.format`); a newer app refuses an
  index it does not understand instead of misreading it, and the user
  downloads or builds a new one.
