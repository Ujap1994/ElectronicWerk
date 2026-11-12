# ADR-0007: Symbol, footprint and component editors

- **Status:** accepted
- **Date:** 2026-11-12
- **Milestone:** M6

## Context

M6 adds the editors for the parts of the project library: the symbol editor
(pins with types, styles and numbers, graphics, units, De Morgan bodies, a
pin table with bulk edits), the footprint editor (pads of every kind and
shape, silkscreen, fabrication and courtyard layers, 3D model assignment and
alignment), IPC-7351 footprint wizards, a component editor (symbol,
footprint, 3D model, parameters, supplier data) and KLC-like library checks.

The constraints of the schematic editor (ADR-0006) hold here too:

- The engine owns the data (ADR-0001). The editors must not keep a copy of a
  symbol or footprint that can drift from the engine's.
- Every change is a transaction with undo, journal and change events
  (ADR-0002).
- The 2D view (ADR-0003) draws binary scenes built by the engine.

Two further points matter for libraries:

- Library items are referenced **by name** from sheets (placed symbols),
  boards (footprints), symbols (default footprint) and components. Renaming
  must not break a design.
- KiCad's libraries are the main source of parts (ADR-0004, ADR-0005). Their
  alternate body styles must survive the import, and wizard footprints should
  look like KiCad's (names, layers, pin 1 conventions), so users can mix
  both.

## Decision

### Model: alternate bodies and components

- A symbol unit gets an optional `alternate` body (pins, graphics, texts);
  a placed symbol remembers `alternate: true`. Every reader of a placed
  symbol asks the definition for the body of its unit and style
  (`SymbolDef::body`), so the scene, the connectivity and the editing
  operations agree. The KiCad import maps body style 2 to it.
- A library gets `components`: `ComponentDef` with symbol, footprint, value,
  manufacturer, MPN, datasheet, properties and suppliers. Placing a component
  copies its data into the placed symbol's fields, so the BOM and the netlist
  need no lookup and placed parts do not change when the library does.

Both fields are optional with defaults, so the file format keeps its schema
version and older files read unchanged.

### Logic in `ew-libedit`, edits as whole-item updates

The new crate `ew-libedit` holds the editing operations, the wizards, the
checks and the reference handling. An operation takes one definition (and,
for symbols, the body: unit and style) plus an edit such as `move`,
`setPins`, `addPadArray` or `courtyard`, and returns the new definition and
the items to select. `ew-core` commits the new definition as one `update`
of the library object with a history label (`edit-symbol`,
`edit-footprint`, …).

Library items are small (a few hundred pins at most), so replacing the whole
definition per edit is cheap and keeps the operations pure and easy to test.
Items inside a definition are addressed by kind and index (`pin 3`,
`pad 7`), which is what the tables and the drawing share.

### Editor scenes with encoded ids

`view.item` builds a scene of one symbol body or footprint in which every
pin, pad, graphic and text is a top-level object. Its id encodes the item:
the kind in the high 64 bits, the index in the low 64 bits. The renderer
decodes picked ids into `{kind, index}` without a lookup table, and selects
items after an edit by encoding the indices the engine returns. The scene is
rebuilt after every change; an item has far fewer objects than a sheet, so
this stays well below a frame.

### References follow renames

`libedit.rename` collects every reference in the project (placed symbols,
board footprints, symbol default footprints, component symbols and
footprints) and updates them in the same transaction as the item. Deleting
an item that is in use needs `force`; placed parts keep the name, so undo or
a new item of the same name restores them.

### Wizards compute, the editor finishes

The wizards (chip, gull-wing, no-lead, BGA, pin header) implement the
IPC-7351 land equations without tolerances, with toe, heel and side fillets
and courtyard excess per density level from the IPC-7351B tables. They
produce an ordinary footprint definition (pads, fabrication outline with a
pin 1 bevel, silkscreen kept clear of the pads, courtyard, texts), named
like KiCad's footprints. There is no "wizard footprint" type: once created,
it is edited like any other footprint.

### Checks are data

The library checks return findings with a stable `code`, a severity, an
English message, arguments for translations and a target (pin, pad, unit).
The UI translates by code and uses the target to select the item. The same
findings feed the item list counts, the checks panel, the wizard preview,
the component editor and scripts (`libedit.check`).

### UI

A library opens in a tab like a sheet. The renderer's `ItemEditor` is the
viewer interaction (tools, snapping, move offset); tables and the side panel
call the same engine methods. Bulk edits are one `setPins` / `setPads` edit
for the selected rows, so they are one undo step. Edits are queued in order,
so quick table edits never overtake or drop each other.

The 3D alignment preview reuses the library manager's three.js view. A new
`library.model` method maps a footprint's model path to the base library's
model entry and its tessellated mesh; models outside the base library have no
preview until the 3D view of M9 tessellates project models.

## Consequences

- Undo, autosave, recovery and change events work for all library edits
  without new mechanisms, and every editor feature is scriptable.
- Placed parts and boards update at once when a symbol or footprint changes,
  because the scene builders read the library definitions.
- Component data is copied on placement. A later "update parts from library"
  command (M7/M8) will be needed to push component changes into designs.
- Addressing items by index means indices shift after a delete; the editor
  clears the selection on delete and the engine returns the indices to
  select after every edit, so clients never keep stale indices.
- The wizards cover the common package families; less common packages
  (SOT-23, SOD, D-PAK, crystals) start from a similar wizard footprint and are
  finished in the editor, or come from the base library.
