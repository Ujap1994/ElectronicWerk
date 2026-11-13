# 3D models: STEP, GLB and VRML

ElectronicWerk writes the board with all its parts as a 3D model for other
programs: STEP for mechanical CAD (an enclosure, a front panel, a mounting
bracket), GLB for viewers, renderers and the web, VRML for older tools. The
same exports run from three places:

- **Export** in the toolbar of the [3D view](./view-3d#screenshots-and-export),
- a **3D model** output in an [output job](./outputs), together with the
  Gerber files,
- `ewerk export-3d` on the [command line](../reference/cli#ewerk-export-3d).

## STEP

STEP (ISO 10303, application protocol AP214) is what every mechanical CAD
program reads: FreeCAD, Fusion, SolidWorks, Inventor, Onshape, CATIA and
others. The file is an **assembly**:

- **The board** is an exact solid: the outline with its straight lines and
  arcs, cut-outs, and every drilled hole as a true cylinder, extruded through
  the laminate (the dielectrics and inner copper layers of the stack-up).
  CAD tools can snap to hole centres and edges as on a part drawn in CAD. If
  holes overlap each other or cut the board edge, the board is written from
  polygons instead (straight segments only), which is always a valid solid.
- **Each part** with a STEP model is placed as an instance of that model.
  The model's STEP file is taken **as it is** — the exact solids, colours and
  names of the original — and written once, however many parts use it.
- **Parts whose model only exists as a mesh** (the 3D models of the
  downloaded [base library](./library) are meshes, to keep the download
  small) are written as faceted surfaces with their colours. They look the
  same, but CAD tools treat them as surfaces made of triangles.

The parts stand on the outer copper of their side, turned and mirrored like
on the board. In the CAD tool the assembly shows the board as one part and
every part under its designator.

::: tip Best results in mechanical CAD
Exact part models come from STEP files: install KiCad (its 3D library is
used automatically), build the base library from local KiCad folders, or put
the vendors' STEP files into the project folder and point the footprints at
them (`${KIPRJMOD}/3d/part.step`).
:::

The export is checked in the test suite: the STEP file of the Blinky example
is read back with OpenCASCADE, the geometry kernel FreeCAD and many other
tools build on, and the board, its holes and every part must be where they
belong. With the models of a KiCad installation, the parts of the exported
file match KiCad's own STEP export of the same board triangle for triangle.

## GLB

GLB (binary glTF 2.0) is the standard format for 3D on the web and in
renderers and game engines: Blender, three.js, Babylon.js, the Windows 3D
Viewer, Apple's Preview and Quick Look. The file holds the meshes of the
3D view with their materials — the laminate, copper with the surface finish
(metallic), the transparent solder mask and the silkscreen — and one shared
mesh per model, placed by a node per part named after its designator. Units
are metres, y points up, as glTF requires.

## VRML

VRML 2.0 (`.wrl`) is the older format of KiCad's 3D library, still read by
many simulation and CAD tools. The file has the same content as the GLB file,
with every model defined once (`DEF`) and reused (`USE`). The **Units**
option writes millimetres (default), metres or inches; y points up.

## Models for GLB and VRML

GLB and VRML carry meshes, not exact solids. Models of the base library have
a mesh already. A STEP model gets its mesh the first time the board is shown
in the [3D view](./view-3d): the mesh is cached, and from then on the
exports use it. A STEP model without a mesh yet is listed in the result
(_this STEP model has no mesh yet_); open the 3D view once and export again.
The command line cannot convert STEP files into meshes, so run the export from
the app first when you need GLB or VRML with such models.

## Options

| Option                                | Meaning                                                                                                                                                                 |
| ------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Format**                            | STEP, GLB or VRML.                                                                                                                                                      |
| **Origin**                            | **Drill and placement origin** (default, the same origin as the [placement files](./bom-placement)), **Centre of the board**, or the **board coordinates** as they are. |
| **Units** (VRML)                      | Millimetres, metres or inches.                                                                                                                                          |
| **Include parts that are not placed** | Also exports parts marked “do not populate” (left out by default, like on the assembled board).                                                                         |
| **Subfolder** (output jobs)           | Where in the output folder the file goes (`3d` by default).                                                                                                             |

The 3D view's **Export** uses the drill and placement origin and leaves DNP
parts out.

## In an output job

Add a **3D model (STEP, GLB, VRML)** output to a job with **Add output…**:

![A 3D model output in an output job](/screenshots/outputs-3d.png)

The file is named after the board (`3d/Blinky.step`). Models that were not
found are reported next to the output in the results, so a missing model
does not go unnoticed. Add the output to the job's **ZIP package** if the
assembly house wants a 3D model with the production files.

## Not found models

A part whose model is not found is left out of the export (the board and the
other parts are written). The report names the missing model paths; the
3D view lists them under **… not found** too. See
[Parts and their 3D models](./view-3d#parts-and-their-3d-models) for where
models are looked for.
