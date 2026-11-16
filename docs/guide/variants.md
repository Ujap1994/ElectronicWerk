# Assembly variants

One board often goes into several products: a cheaper version without the
display, a version for 120 V and one for 230 V, a test version with extra
connectors. The copper is the same; only some parts are left out, added or
have another value. An **assembly variant** describes such a version: it
lists only the parts that differ from the design.

Variants are kept in the project. Output jobs are made for a variant: the
bill of materials, the pick-and-place files, the solder paste, the
documentation and the 3D model of the job then show the parts as the
variant has them.

## Making a variant

Variants are edited in the project overview (click the project name at the
top of the project tree), under **Assembly variants**:

1. Click **Add a variant**. It is named `Variant 1`; type another **Name**
   and a **Description** if you like.
2. The table lists every part of the design with its value. For each part
   that differs, choose under **In this variant**:
   - **As designed** — as in the schematic (the default),
   - **Fitted** — fitted, even if the design marks it _do not populate_,
   - **Not fitted** — left out.
3. Under **Value in this variant**, type another value where it differs
   (empty: the designed value).

![A variant without the third channel and with another value for C1](/screenshots/variants-editor.png)

The number of changed parts is shown next to each variant; **Parts** opens
and closes its table, **×** removes the variant. Every change is one undo
step.

## Outputs for a variant

Open an [output job](./outputs) and choose the **Assembly variant** next to
the output folder (**As designed** makes the outputs as before). The choice
is saved with the job, so a project can have one job per variant, each with
its own output folder.

![An output job made for the variant Lite](/screenshots/variants-job.png)

For a job with a variant:

- The **bill of materials** marks the parts the variant leaves out as `DNP`
  (or leaves them out when the BOM does not list DNP parts), and shows the
  variant's values and fields.
- **Pick-and-place** files leave out the parts that are not fitted (unless
  the output lists do-not-populate parts), the **solder paste** layer leaves
  out their pads, and **IPC-2581** marks them as not populated.
- The **3D model** (STEP) leaves them out.
- The text variable `${VARIANT}` holds the variant's name, for example in a
  title block or a file name.

Gerber and drill files are the same for every variant: the bare board does
not change.

A job for a variant the project no longer has fails with a clear message
until you choose another one.

## Sheets used more than once

A part on a [repeated sheet](./channels) has one designator per use (R1,
R2, R3 …). A variant changes the bill of materials and the board for each
of them separately; the schematic keeps showing the design.

## In the files

Variants are stored in the project file (`.ewproj`) as `variants`, see the
[file format](../reference/file-format):

```json
{
  "variants": [
    {
      "name": "Lite",
      "description": "Without the third channel",
      "parts": {
        "R3": { "fitted": false },
        "C3": { "fitted": false },
        "C1": { "value": "220n" }
      }
    }
  ]
}
```

Scripts change them with
[`project.setProperties`](../reference/engine-api#project-setproperties)
and preview a variant's bill of materials with
[`outputs.bom`](../reference/engine-api#outputs-bom).
