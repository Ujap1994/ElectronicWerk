# ADR-0003: 2D rendering engine

- **Status:** accepted
- **Date:** 2026-11-08
- **Milestone:** M2

## Context

Every editor (schematic, symbol, footprint, layout) needs a 2D view that stays
smooth on large designs: the roadmap budget is 60 fps for panning and zooming
an L board (5,000 parts, 200,000 track segments, 8 layers) on a mid-range
integrated GPU, and visual feedback within 16 ms. The view must show layers
with colours and opacity, text, a grid, and support hover, click, box and
lasso selection. The renderer process is sandboxed and has no Node.js access;
the design lives in the Rust engine in another process.

## Decision

### Engine prepares, GPU draws

The engine converts a document into a **scene** once (`view.scene`, crate
`ew-scene`); the renderer only draws it. All placement (footprint and symbol
transforms, bottom-side mirroring, pad expansion, arcs from three points,
Bézier tessellation, polygon triangulation with earcut) happens in Rust.

- **Five instanced primitive kinds** cover every board and sheet object: line
  segments with round caps, arcs, discs/rings, rotated rounded rectangles and
  indexed triangle meshes. Each kind has a fixed instance layout of f32 values
  plus a u32 object index.
- **Exact shapes in the fragment shader.** Each instance is a quad; the
  fragment shader computes the signed distance to the real shape. Edges stay
  sharp at every zoom, and a minimum on-screen width keeps hairlines visible.
- **One binary buffer** holds the object table and every primitive, at 4-byte
  aligned offsets listed in the JSON metadata. The Node addon returns it as a
  `Buffer` (`Engine::dispatch_binary`), the engine host posts it to the
  renderer, and the renderer uploads it to one WebGL buffer without parsing.
  JSON clients (`ewerk rpc`) get the same bytes as base64.
- **Precision:** coordinates are stored relative to the scene origin (the
  board centre or page centre), so f32 keeps nanometre-level precision near
  the design. The camera works in f64 in JavaScript.

### Compositing layers

Layers are drawn back to front (bottom side first, the active layer above the
other side layers, outline and drill holes on top). Within a layer, shapes
overlap a lot (tracks into pads), so plain alpha blending would darken
overlaps. Instead:

- The canvas uses **MSAA**; the fragment shader writes the edge coverage as
  alpha and **alpha-to-coverage** turns it into a sample mask
  (anti-aliasing).
- Layer opacity comes from the **constant blend colour**
  (`CONSTANT_ALPHA`), not from the fragment alpha.
- Each layer gets its own **depth value** with `LESS`, so every sample is
  blended at most once per layer: the union of a layer's shapes is blended
  with the layer opacity in a single pass, without offscreen buffers.

### Culling and level of detail

The engine sorts each layer's instances along a **Z-order (Morton) curve** and
stores a bounding box per chunk of 1,024 instances. Per frame, the renderer
finds the chunks that touch the view, merges neighbours into runs and issues
one `drawArraysInstanced` per run (WebGL 2 has no base instance, so the
attribute offsets move instead). Texts below 3.5 device pixels are dropped in
the vertex shader. Zooming out is limited to a tenth of the fitted size,
because a design only a few pixels large costs fill rate without showing
anything.

### Text

Texts use one bundled monospace font (JetBrains Mono, SIL OFL). Its fixed
advance lets the engine compute exact text boxes for hit-testing and label
outlines without font metrics, and the renderer lays out glyphs with the same
formula. Glyphs are rasterized on demand into a single-channel **signed
distance field** atlas (tiny-sdf) and drawn as instanced quads; bold uses a
lower SDF threshold, italic a shear. Manufacturing text (stroke fonts for
Gerber) is a separate topic for the outputs milestone.

### Interaction

- **Hit-testing** runs in the renderer for latency: a packed Hilbert R-tree
  (Flatbush) over the object boxes, then exact distance tests on the drawn
  primitives of each candidate (an object → primitive table is built once per
  scene). Priorities pick the most specific object (pads before tracks before
  footprints before zones); footprints, symbols and sheets are hit anywhere in
  their box.
- **Selection and hover** are flags in an `R8UI` texture indexed by object;
  the vertex shaders read them, so selection changes never re-upload
  geometry. Children (pads, pins, fields) inherit the flags of a selected
  parent.
- The canvas renders **on demand** (one `requestAnimationFrame` per change),
  never in a loop.

### Software fallback

The main process sets `enable-unsafe-swiftshader`, so Chromium falls back to
its software WebGL implementation when no GPU is usable (virtual machines,
remote desktops, CI runners). The app only loads its own content, so the JIT
of the software renderer is not exposed to web content.

## Alternatives considered

- **Canvas 2D or SVG:** far too slow for 500,000+ shapes per frame.
- **Tessellating everything into triangles (as many CAD tools do):** more
  vertices, view-dependent quality for arcs and round caps, and anti-aliasing
  only by multisampling. SDF quads give exact shapes with four vertices.
- **A framebuffer per layer for opacity:** correct, but one full-screen pass
  per layer per frame. The depth trick gives the same result in one pass.
- **Hit-testing in Rust over IPC:** keeps one implementation, but adds a
  round trip to every mouse move and blocks while the engine is busy. The
  R-tree in the renderer answers within a millisecond.
- **Building scenes in TypeScript from `document.get`:** the L board as JSON
  is about 55 MB; parsing alone would exceed the open budget.
- **A stroke font (like KiCad) for all text:** needs a font data set and
  looks dated for schematics; SDF text is crisp at every size.

## Consequences

- Opening the L board in the app takes about 0.2 s (scene 40 ms in Rust),
  and panning and zooming run at the display's 60 fps with about 0.2 ms CPU
  per frame (Apple M3). The XL board (2.3 million primitives) also stays at
  60 fps.
- Edits currently rebuild the scene of the open document. The editors
  (M5, M7) will add incremental updates per change set, using the stable
  object keys (id, type and child index) that the selection already uses.
- Every new object type needs a mapping to the five primitive kinds in
  `ew-scene` and, if selectable, an entry in the inspector (`view.object`).
