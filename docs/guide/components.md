# Components

A **component** is a part you can order: a symbol, the footprint it is
soldered with, its value, the manufacturer and part number, suppliers and any
extra fields. Placing a component puts all of this on the sheet in one step,
so the bill of materials is complete from the start.

A symbol like `R` stands for every resistor; a component like
`R_10k_0603_1%` stands for one resistor you can buy. You do not need
components to draw schematics: a symbol with a default footprint works too.
They pay off for parts you use again and again.

## Creating a component

Open the [library editor](./library-editor), choose **Components** and click
**New**. Name the component, for example after the part number.

![The component editor: symbol, footprint, order data and the pin-to-pad check](/screenshots/libedit-component.png)

| Field                         | Meaning                                                                                                                          |
| ----------------------------- | -------------------------------------------------------------------------------------------------------------------------------- |
| **Symbol**                    | A symbol of the library. The button next to it opens the symbol in the symbol editor.                                            |
| **Footprint**                 | The footprint; suggestions come from the library. The button opens it in the footprint editor.                                   |
| **Value**                     | The value shown on the sheet (`10k`, `LM358`). Empty means the symbol's default value.                                           |
| **Manufacturer**              | The maker of the part.                                                                                                           |
| **Manufacturer part number**  | The exact part number (MPN). The [library checks](./library-checks) note components without one.                                 |
| **Datasheet**                 | A link or file name.                                                                                                             |
| **Description**, **Keywords** | Shown and searched in the symbol chooser.                                                                                        |
| **Suppliers**                 | Distributors with their order number and an optional link. Type a supplier's name, click **Add supplier**, then fill in the row. |
| **Fields**                    | Any further fields (`Tolerance`, `Voltage`, `Temperature range`).                                                                |

Every change is one undo step, like all other edits.

## Pins and pads

The right column shows the symbol and the footprint, the findings of the
[library checks](./library-checks) for this component, and the **pins and
pads** table: every pin number of the symbol (all units, normal bodies) next
to the pad with the same number.

- A ✓ in the **Pad** column means the pin has a pad.
- _missing_ in the **Pad** column: the pin has no pad, so it will not be
  connected on the board. The checks report `pin-without-pad`.
- _missing_ in the **Pin** column: a pad without a pin, for example an
  exposed pad or a mounting tab. The checks report `pad-without-pin` (a warning
  you can ignore when the pad is meant to stay unconnected).

## Placing a component

In the schematic editor, press <kbd>A</kbd> for the symbol chooser. When
you type a search, matching components are listed above the symbols:

![A component in the symbol chooser](/screenshots/libedit-chooser-component.png)

The detail panel shows the component's symbol, its footprint, manufacturer
and part number. Place it like a symbol. The placed symbol gets:

- the component's **value** and **footprint**,
- its **fields**, plus `Manufacturer`, `MPN` and `Datasheet`,
- per supplier `Supplier 1`, `Supplier 1 PN`, `Supplier 2`, … .

These fields go into the [bill of materials](./schematic-exports) like any
other field. Later changes to the component do not change parts that are
already placed; place the component again, or edit the fields of the placed
symbol in the properties panel.

## Renaming and deleting

Renaming a component, its symbol or its footprint keeps everything
connected: the component follows a renamed symbol or footprint, and placed
symbols follow a renamed footprint. Deleting a component does not change
placed symbols; their fields stay.
