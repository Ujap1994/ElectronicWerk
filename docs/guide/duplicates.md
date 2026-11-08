# Duplicates

Large libraries contain the same part more than once: a footprint copied into
another library, two symbols that differ only in their name, a 3D model
saved twice. The base library finds these duplicates automatically, shows
each part once, and lets you decide the cases that need a human eye.

![Reviewing near duplicates of footprints](/screenshots/library-duplicates.png)

## How duplicates are found

Every symbol, footprint and 3D model gets a **fingerprint**, a hash of a
canonical form of the object. The canonical form ignores everything that
does not change the object:

- **Footprints:** the pads (number, type, shape, size, drill, layers) and the
  drawn graphics (silkscreen, fabrication, courtyard), with coordinates
  rounded to 1 µm, moved so that the pads are centred on the origin, and
  turned by the quarter turn that gives the smallest canonical form. Names,
  texts and the order of the items do not matter, nor do the units the
  library was drawn in.
- **Symbols:** the pins of every unit (number, name, electrical type,
  position, direction, length) and the body graphics and texts.
- **3D models:** the triangles of the tessellated mesh, with corners rounded
  to 1 µm, independent of the order of the triangles and of the file name,
  plus the colours.

The fingerprints do not depend on the source, so the same rules will merge
parts from other libraries (online part search, other formats) later on.

## The three classes

| Class                | Means                                                                                                                                              | Default            |
| -------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------ |
| **Exact duplicates** | The same object twice: footprints with identical pads and graphics, symbols with the same name and drawing, models with the same mesh and colours. | Merged             |
| **Near duplicates**  | Equal within a tolerance or different only in details (see below).                                                                                 | Waits for a review |
| **Aliases**          | Different parts that share one drawing, for example KiCad's derived symbols (`LM358` and `TL072` with the same op-amp symbol).                     | Kept separate      |

Near duplicates are:

| Kind       | Near duplicate when                                                                                                                                  |
| ---------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- |
| Footprints | The pads are identical but the graphics differ (for example `R_0805` and `C_0805`), or the pads match within **0.05 mm** (position, size and drill). |
| Symbols    | Two symbols have the same name and the same pin numbers but another drawing (for example a symbol drawn in two styles in two libraries).             |
| 3D models  | The mesh is identical but the colours differ, or the bounding box matches within 0.05 mm and the triangle count is the same.                         |

Symbols with the same pins but another body are _not_ near duplicates: in
practice they are different part types (a diode, a Zener diode and a
Schottky diode share their pins).

## Merged entries

When a group is merged, one entry stays visible: the **shown** entry. The
others are hidden in the search results, but:

- they are still found by their own names: searching for a merged entry's
  name finds the shown entry, which lists it under **Merged duplicates**;
- **Show merged duplicates** in the filters lists them again;
- they keep all their data, so nothing is lost.

By default the shown entry is the one from the first source, then in the
alphabetically first library.

## Reviewing duplicates

Open the **Duplicates** tab. The badge on the tab counts the near-duplicate
groups that wait for a review.

1. Choose what to review with the filters on the left: the kind (symbols,
   footprints, 3D models), the class and the decision (**To review**,
   **Merged**, **Separate**). The default is near duplicates to review.
2. Select a group. Its members are drawn side by side (2D for symbols and
   footprints, 3D for models); the shown entry is framed.
3. Decide:
   - **Merge** shows one entry and hides the others.
   - **Keep separate** keeps all entries visible.
   - **Show this one** under a member makes it the shown entry.

![Near duplicates of 3D models](/screenshots/library-duplicates-3d.png)

Decisions take effect at once and are saved in `decisions.json` in the
library folder. They are applied again after every rebuild and after
installing a newer library, as long as the group has the same members. If a
new library version changes a group, the group is new and asks for a review
again.

## The deduplication report

Every build writes a report into the library folder:

- `dedup-report.md`: a summary per kind (entries, groups per class, merged
  entries, groups to review) and every exact and near group with its reason;
- `dedup-report.json`: everything, including the aliases, for scripts.

The report is **reproducible**: two builds from the same libraries give the
same files, byte for byte, whatever order the files were read in and however
many threads did the work. **About this library → Show the deduplication
report** opens it; `ewerk library duplicates` prints it.

## Numbers for the KiCad libraries

For the KiCad libraries (10.0, about 45,600 entries) the engine finds:

| Kind       | Exact groups | Near groups | Alias groups |
| ---------- | -----------: | ----------: | -----------: |
| Symbols    |            1 |          92 |        3,017 |
| Footprints |          675 |       1,647 |            0 |
| 3D models  |          120 |         453 |            0 |

Exact duplicates among footprints are mostly connector and package variants
with different part numbers but identical geometry; near duplicates are
typically the same land pattern with different silkscreen, or pads that
differ by a few hundredths of a millimetre.
