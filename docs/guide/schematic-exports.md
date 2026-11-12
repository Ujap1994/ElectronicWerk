# Exports: netlists, bill of materials, SVG and PDF

The export button (the arrow into a tray) in the view toolbar of a sheet
writes the schematic data to files for other tools, for ordering parts and
for printing. Each export asks where to save; the project folder is
suggested.

![The export menu](/screenshots/schematic-export.png){width=300}

| Export                      | File            | Contents                                                     |
| --------------------------- | --------------- | ------------------------------------------------------------ |
| **Netlist (KiCad)**         | `<project>.net` | Components and nets in KiCad's netlist format (version E)    |
| **Netlist (CSV)**           | `*-netlist.csv` | One line per pin: net, designator, pin                       |
| **Bill of materials (CSV)** | `*-bom.csv`     | One line per group of equal parts                            |
| **This sheet as SVG**       | `<sheet>.svg`   | The open sheet as a vector image                             |
| **All sheets as PDF**       | `<project>.pdf` | Every sheet of the project, one page each, in its paper size |

Every export covers the whole hierarchy. A sheet that is placed more than once
appears once per placement, with the designators of that placement.

## Netlists

A netlist lists every component and which pins are connected to each other.

**KiCad format.** The file uses KiCad's netlist format, which KiCad's board
editor (**File → Import → Netlist**, with footprints linked by **reference
designator**) and many other tools read. Each component has its designator,
value, footprint and symbol; each net its name and its pins with their
electrical type:

```text
(export (version "E")
  (design (source "Blinky") (tool "ElectronicWerk 0.7.0"))
  (components
    (comp (ref "C1") (value "10u") (libsource (lib "") (part "Device:C")))
    …)
  (nets
    (net (code "1") (name "/CV")
      (node (ref "C2") (pin "1") (pintype "passive"))
      (node (ref "U1") (pin "5") (pintype "input")))
    …))
```

Net names follow KiCad's rules, so a netlist of an imported KiCad project
matches the one KiCad writes:

- a power port, power symbol or global label gives its name (`GND`, `VCC`);
- otherwise a net label gives `/<sheet path>/<name>` (`/DIS`, `/Power/VIN`);
- otherwise the net is named after its first pin: `Net-(R1-Pad2)`, or
  `Net-(D1-A)` when the pin has a unique name.

Power symbols and power flags are not components and do not appear.

**CSV.** One line per pin, sorted by net, designator and pin:

```text
Net,Designator,Pin
/CV,C2,1
/CV,U1,5
GND,C1,2
…
```

ElectronicWerk itself can compare a netlist with a board; see
[Netlists](./netlists).

## Bill of materials

The bill of materials (BOM) groups equal parts: same value, footprint,
symbol, manufacturer and part number. Columns:

| Column         | Contents                                                            |
| -------------- | ------------------------------------------------------------------- |
| `Item`         | Line number                                                         |
| `Quantity`     | Number of parts                                                     |
| `Designators`  | All designators of the group, in natural order (`R2 R10 R11`)       |
| `Value`        | Value                                                               |
| `Footprint`    | Footprint name                                                      |
| `Symbol`       | Symbol name                                                         |
| `Description`  | Description of the symbol                                           |
| `Manufacturer` | The parameter `Manufacturer`                                        |
| `MPN`          | The parameter `MPN` (manufacturer part number)                      |
| `DNP`          | `DNP` for parts marked **Do not place** (listed in their own lines) |

Parts marked **Leave out of the BOM** are not listed. Add parameters in the
[Properties](./schematic-editor#editing-properties) panel. This first BOM is a
plain CSV file that opens in every spreadsheet program; a configurable BOM
with ordering data follows with the online part search.

## SVG

**This sheet as SVG** writes the open sheet in millimetres: the page with its
frame and title block, every symbol, wire, label and text, in the colours of
the light theme on white paper. Texts are real text in a monospace font, so
they can be searched and copied. The SVG opens in browsers and in vector
programs such as Inkscape.

## PDF

**All sheets as PDF** prints every schematic sheet of the project to one PDF,
one page per sheet in the sheet's own paper size (A4, A3, Letter, …). Use it to
print the schematic or to send it to someone without ElectronicWerk.

## See also

- [Drawing schematics](./schematic-editor)
- [Electrical rules check](./erc)
- [Engine API: `schematic.netlist`, `schematic.bom`](../reference/engine-api#schematic-netlist)
