# ADR-0020: Part catalog

- **Status:** accepted
- **Date:** 2026-11-22
- **Milestone:** M19

## Context

After M18 the maintainer found the base library "not really usable yet": it
listed about 23,000 symbols, 15,000 footprints and 7,000 3D models side by
side, the way they come from KiCad, without complete parts and without an
order to browse. Asked what bothered most, the maintainer named the missing
complete parts (symbol, footprint and 3D model as one) and the missing
overview (categories). Ordering data (distributor part numbers, prices)
were left out for now.

## Decision

### Categories are derived, not stored

The category of a symbol follows from the name of its KiCad library
(`MCU_ST_STM32F4` → microcontrollers › ST, `Regulator_Linear` → power,
`Sensor_Humidity` → sensors) and, for KiCad's general `Device` library,
from the symbol name (`R`, `C_Polarized`, `LED`, `Q_NPN_BCE` …). The tree
has nine main groups (passive parts, discrete semiconductors, integrated
circuits, RF, connectors, electromechanical parts, modules, mechanical
parts, other) with fixed children, plus the vendors below
microcontrollers.

- **Why not in the index:** the index is distributed as a pack of a library
  release. Adding a column would need a new pack format and a migration of
  every installed library. Deriving the categories when the index is first
  asked takes about 25 ms for 23,000 symbols, kept in a temporary table of
  the database connection, so searches filter by category in SQL like by
  library.
- **Why from names:** KiCad's libraries are named systematically, and the
  rules (a table of name prefixes) cover all 223 symbol libraries; a test
  checks that every rule points into the tree. Libraries of other sources
  fall into "other" until rules for them exist.
- Power symbols, simulation sources and graphics are not parts.

### Packages from the footprint filters

- A symbol that names its footprint (79 % of the KiCad symbols) is a
  complete part with exactly that package.
- A basic part without a footprint offers the footprints that its KiCad
  footprint filters (`R_*`, `Connector*:*_1x??_*`) allow and that have as
  many pads as the symbol has pins. Matching the pad count keeps out what
  the filters alone would allow (a 3-pin footprint for a 2-pin diode).
- **Common packages first:** a short list per kind of part (resistors:
  0603, 0805, 0402, 1206, 2512, axial; LEDs: 0603, 0805, 1206, 3 mm, 5 mm;
  pin headers by their row count …) is shown first and marked as common;
  merged duplicates are kept for these well-known names.

### A part enters the project as a component

Adding a part copies the symbol and the chosen footprint into the project
library and creates a **component** (the existing library object that ties
a symbol to a footprint, value and part number), named after the package
(`R_0603_1608Metric`). Placing the component puts the symbol on the sheet
with its footprint assigned, so the board gets the right package when it is
updated from the schematic. The value (`10k`) is set after placing; one
component per package, not per value, keeps the library small.

### Search ranks exact names first

The catalog showed that the full-text ranking put descriptions mentioning a
word before the part of that name (`LED_RGB` before `LED`). An entry whose
name equals the search text now comes first; the rest keeps the existing
ranking.

## Consequences

- New modules `ew_library::category` and `ew_library::parts`; engine methods
  `library.categories`, `library.packages`, `library.addPart`; `category` in
  `library.search`.
- The library window opens on a **Parts** tab with the category tree and a
  part view (symbol, package choice, footprint in 2D and 3D, add and place);
  the schematic's place dialog leads to it.
- `ewerk library categories|packages`, `ewerk library search --category`;
  MCP tools `list_part_categories` and `part_packages`.
- Limits: categories and common packages exist for KiCad's naming; ordering
  data and parametric search (by resistance, voltage …) are not part of the
  catalog yet.
