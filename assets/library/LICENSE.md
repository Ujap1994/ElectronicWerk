# License of the ElectronicWerk base library

The symbols, footprints and 3D models in this library pack are derived from
the official KiCad libraries (<https://www.kicad.org/libraries/>, source:
<https://gitlab.com/kicad/libraries>), © the KiCad library contributors.

They are licensed under the
[Creative Commons Attribution-ShareAlike 4.0 International License](https://creativecommons.org/licenses/by-sa/4.0/legalcode)
(CC-BY-SA 4.0) with the following exception, as published by the KiCad
project:

> To the extent that the creation of electronic designs that use 'Licensed
> Material' can be considered to be 'Adapted Material', then the copyright
> holder waives article 3 of the license with respect to these designs and
> any generated files which use data provided as part of the 'Licensed
> Material'.

In short: you may use the library in your designs and publish those designs
under any license. If you redistribute the library itself, or a modified
version of it, it must stay under CC-BY-SA 4.0 with this exception.

## Changes made by ElectronicWerk

- Symbols and footprints were converted from the KiCad file format to the
  ElectronicWerk data model and stored in a search index (`library.sqlite`).
- 3D models were converted from STEP to GLB (triangle meshes) with
  OpenCASCADE.
- Duplicates were detected and grouped; exact duplicates are shown as one
  entry. Every original entry is kept in the index.

The ElectronicWerk application itself is licensed under the MIT License; this
license only covers the library data.
