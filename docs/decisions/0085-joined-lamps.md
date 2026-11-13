# 0085 – Lamps side by side glow as one surface

- **Status:** proposed (engine, shader and data; waits for the developer before it is merged)
- **Date:** 2026-11-13

## Context
ADR 0084 painted each lamp as a framed panel, so a wall of lamps read as a grid of frames. The developer's
call (2026-11-13): lamps next to each other glow as one surface, with the frame only on the outer edge. That
needs each face to know which of its sides meet another lamp: connected textures.

## Options considered
1. **Texture variants picked at meshing**: sixteen paintings a lamp, one for each set of joined sides. The
   painted-face format holds at most four variants (2 bits) picked by the cell's hash, so this needs a new
   format, sixteen files a block, and the Studio painting all of them.
2. **The frame drawn in the shader from a neighbour mask**: the mesher already looks at a face's neighbours
   for ambient occlusion; it writes four more bits (which sides meet the same block whose face is open too)
   into the quad's free high bits, and the shader reads a joined side's frame texels from inside. One painting,
   as now.
3. **The shader asks the world** (a ray or a cell lookup per fragment): no cell buffer on the GPU, and far
   too costly per pixel.

## Decision
Option 2.

- `data/blocks.toml`: `joins = true` marks a block whose faces join (lamp, blue lamp); `blocks::Def::joins`.
- `mesh.rs`: the quad's high word is `material8 | ao8 << 8 | join4 << 16`; for a joining block, `join_sides`
  sets bit 0..3 (u-, u+, v-, v+) where the in-plane neighbour is the same block and the cell in front of it is
  air, so its face there is drawn too. A joining block's faces never merge (each knows its own sides). Other
  blocks pay one bit test a quad; the side work is a cold function out of the loop.
- `shader.wgsl` `texel_a`: with a mask, the texel index is clamped `JOIN_FRAME` (2) texels in from each joined
  side before anything else, so the frame there reads as the glow next to it. It works in the face's u, v
  before the per-face turning, so all six faces agree with the mesher.
- `lamp` in `tools/paint_textures.py`: a two-texel frame (dark outside, shade inside) round an even glow of lit
  and light patches with no core, four variants: a joined wall shows no repeat a block.

## Proof
- Red then green: `mesh::tests::lamps_know_their_neighbours_in_the_wall` (a row of three lamps, stone beside
  the last, a stone covering one lamp's face) failed with no mask, passes now; the other mesh tests unchanged.
- Filmed (play build, noon and night): a 3 x 3 wall, an L of three, a single lamp and a 2 x 2 of blue lamps
  each glow as one panel framed on the outside only (`out/lamps-wall.png`, `out/lamps-wall2.png`).
- `technoape-studio check=1` ok; `cargo test -p engine -p studio` pass.
- **CPU (the mesher, `run=meshbench chunks=8`, play build, alternated under the lock):** the same tree with
  `main`'s `mesh.rs` 62.0 µs a surface chunk (8 runs, 61.5..62.6), this change 63.4 µs (62.3..64.6), the
  change with the join test compiled out 62.9 µs. So 1.4 µs (2 %) a chunk with no lamps, about half of it
  code layout. A chunk full of lamps does a little more (four cell reads a lamp face, no merging): not
  measured, lamps are few.
- GPU: one branch a terrain fragment on a flat bit test; not measured apart (no gate on this branch).

## Consequences
- A joining block's faces are never merged: a lamp wall costs one quad a face, as at level 0 anyway.
- An inner corner (two sides joined, the diagonal not) loses its frame corner texel; flat walls and rows are
  exact. `shortcut:` in the shader.
- Any block can join by data; the clamp suits textures with a frame of `JOIN_FRAME` texels.
- The quad's bits 20..31 stay free.

## Review
Fresh reviewer: pending at the time of writing; its findings are listed below.
