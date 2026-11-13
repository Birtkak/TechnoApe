# 0103 – Optimisation batch: the G-buffer pass

- **Status:** proposed (engine, client and shader: waits for the developer, CLAUDE.md principle 13)
- **Date:** 2026-11-13
- **Works down:** `docs/perf-debt.md` rows of ADR 0090, 0091, 0099 (ADR 0096's optimisation batch, G-buffer half)

## Context
The developer plays at 2560x1440 (Leaves fancy, Clutter many, Vegetation normal) and F3 showed 10-13 ms of GPU in
meadow and forest views, against a budget of 8.3 ms for the whole frame (ADR 0004). The G-buffer pass (terrain,
leaves, fluff cards, plants, still models; `shader.wgsl` `terrain_vertex`, `vs_still` and their fragments) was most
of it. The trace, post and cloud stages are another session's.

### Where the time went (1440p, `run=stand quick=1`, the three F9 views of 2026-11-13; one run each)
| | meadow 14-24-14 | shrubs 14-25-54 | forest 14-48-48 | fly (default size) |
|---|---|---|---|---|
| G-buffer, main | 9.68 | 7.86 | 9.39 | 3.31 |
| of which the direct light in the G-buffer (`light()`, one shadow ray a shaded fragment; `dbg_nolight`) | 4.5 | 2.7 | 4.0 | |
| of which plants and still models (`dbg_nostill`) | 3.5 | 2.0 | 1.2 | |
| of which fluff (`fluff=0`) | 0.8 | 1.3 | 1.4 | |
| fancy against fast leaves | 0.8 | 1.6 | 1.3 | |
| plant shadows (`plant_shadows=0`, G-buffer part) | 0.8 | 0.3 | 0.1 | |
| the same view at 640x360 (vertex and primitive work; pixels 1/16) | 3.4 | | 3.0 | |

So: plants were shaded with discard after the terrain, every blade over another and the ground under the grass
running the full light; and at 1440p half the pass was vertex work: some 2 M quads a view, every quad's vertex
shader run six times (no index buffer, no reuse), the crowns' fluff run over every cutout quad (fancy leaves keep a
crown's inner faces, four in five), plants placed past their reach and placed three more times for a normal.
(`dbg_*` were a throwaway build's switches, not kept.)

## Options considered
1. **A depth pass for the plants** (taken, win 1): as the cutout leaves since ADR 0090.
2. **A depth pre-pass for the opaque terrain**: tried; forest -0.5 ms, meadow and shrubs nothing, fly +0.2 (the
   terrain is drawn near to far already). Not kept.
3. **Fluff only within some 40-64 m** (perf-debt's idea), cards shrinking into their face by a hashed reach: at 64 m
   fly -0.5, the 1440p views -1.1 to -1.4 ms (at 40 m without the chunk cull, -0.6 to -0.8). Not kept: shot at 40 m the forest past it turned into a field of cubes, and
   at 1440p a lobe (0.2-0.5 m) is still 3-5 px at 64 m and 2 px at 150 m (`out/shots/cmp-f40-edge.png`). The
   cards are where the developer's crowns stop being boxes; a far cut is a taste call (below).
4. **Fluff only on faces that open on air** (taken, win 2): the same cards, without the inner faces' empty runs.
5. **No local arrays in the vertex shaders** (`frame_at`'s and the corners' `var array`s as arithmetic): measured,
   no change. Not kept.
6. **Plants: stop at their reach, a cheaper normal** (taken, win 3).
7. **Indexed quads** (taken, win 4): four vertex shader runs a quad instead of six.

## Decision
1. **Plants' depth first** (`fs_still_depth`, `still_depth_pipeline`): the still quads' depth after the cutout
   blocks' and before the terrain, then `fs_still` where that depth is (`Equal`, no write). Each plant pixel is lit
   once and the ground under a plant not at all. The model's back face still goes in both passes.
2. **Fluff on open faces only**: the mesher puts the cutout faces that open on air (bit 25) first and counts them
   (`Mesh::fluff` -> `State::Ready` -> `DrawChunk::fluff`); the fluff's passes draw that many from their own
   indirect list. Test: `mesh::tests::faces_are_culled_only_against_opaque_neighbours` (seen failing: 0 of 81164).
3. **`vs_still`**: a plant past its reach returns at once (it had shrunk into the ground there: nothing seen); its
   normal is the cross of its two turned edges (`plant_turn`, `turned`), the same numbers as the three extra corners.
4. **Indexed drawing**: one static index buffer, six indices a quad over its four corners (`quad_index`, 65536
   quads, 1.5 MB); vertex = quad * 4 + corner, `corner_k` maps corner 3 to `quad_corner`'s 5, so both triangles
   and the winding (mirrored faces too) are as before. The terrain's, cutout, fluff, glass and still draws are
   `multi_draw_indexed_indirect` over regions `IND_*` of `indirect` (20-byte args; the sea keeps its pieces
   unindexed). `quad_draws` splits a run longer than the buffer; test
   `frame::tests::quad_draws_cover_every_quad_once_within_the_index_buffer` (seen failing on the unsplit draw).

## Results (1440p, `run=stand quick=1`, median of three, `slot.sh measure`, play build; fly at the default size)
| | meadow | shrubs | forest | fly |
|---|---|---|---|---|
| GPU, before -> after | 13.62 -> 8.65 | 11.63 -> 8.04 | 13.66 -> 10.62 | 5.12 -> 4.55 |
| G-buffer | 10.02 -> 4.92 | 8.35 -> 4.68 | 9.50 -> 6.60 | 3.34 -> 2.81 |
| trace (not this batch) | 1.62 -> 1.86 | 1.45 -> 1.40 | 1.82 -> 1.82 | 0.41 -> 0.39 |
| frame p99 | 19.84 -> 9.88 | 16.63 -> 9.24 | 20.30 -> 11.80 | 9.15 -> 9.05 |
| frame thread's own work p50 | 0.79 -> 1.20 | 0.78 -> 1.13 | 0.76 -> 1.20 | 0.82 -> 0.88 |
| VRAM MB | 1659 -> 1638 | 1659 -> 1638 | 1403 -> 1382 | 1349 -> 1351 |

The G-buffer step by step in the meadow (single runs): 9.68, plants' depth 6.98, open-face fluff 6.15, plants'
early-out 5.88, indexed 4.93. The frame thread's p50 in the 1440p views was taken while other jobs shared the PC
(single runs before it read about 1.0 for both builds); its added work is one list and sort and `quad_draws`, a few
microseconds. Measured again alternating the two builds (two runs each): meadow base 0.81 / 1.10 against 1.03 /
0.96 ms, fly 0.85 / 0.80 against 0.79 / 0.95; the same within noise.

One change can be seen, and it is a fix: `tests/views` `lod` (mean difference 1.08 of 255) lost the dark speckle on
the far savanna, plants pressed flat past their reach fighting the ground's depth; win 3 no longer draws them.

Stills (`out/shots/`, base against each win, the last frame of a 2.5 s filmstrip at 1280 px, the three views, a
crown close-up and the forest edge from 20 m up): mean difference 0.5-2.1 of 255, the same as two runs of one
build; the only differences seen are the sky's clouds between shots. No distance cut-off was added, so no pop.

## Consequences
- Not changed: the look (but the `lod` speckle, above). VRAM +2.8 MB (index buffer, larger indirect regions), under the reported MB's grain.
- Frame thread: one more draw list and sort (fluff), `quad_draws` per chunk; within noise.
- What is left in the G-buffer at 1440p is about half the direct light (`light()`'s shadow ray per fragment, now
  near one a pixel) and half raster and vertex. For the trace session: moving the direct light out of the G-buffer
  into a pass over the finished G-buffer (deferred; 1 ray a pixel whatever the overdraw), or tracing it at half
  resolution with the bounce, is the next big win (some 2-3.5 ms in these views); plant shadows past 30 m and fast
  leaves opaque to rays outside winter are its perf-debt rows.
- A taste call for the developer: fluff past ~64 m (option 3) saves another 1.1-1.4 ms at 1440p (measured before win 4) and turns far
  crowns back toward cubes.
- With ADR 0101's sloped tops (merged after): a slope split along the other diagonal lost a triangle (the four
  indexed corners missed (1, 1)) and the shared corners blended the two triangles' normals. Now the index order is
  0 1 2, 3 0 2, a swapped slope's corners are `quad_corner`'s 3 4 5 0, and the slope's normal goes flat
  (`VOut.slope`) from each triangle's first corner; test
  `frame::tests::indexed_quads_draw_both_triangles_of_every_split_with_its_own_provoking_corner` (seen failing),
  stills `out/shots/cmp-slope-{top,side,side2}.png` (before over after). The traced geometry keeps its six corners.
- `vs_fluff`'s bit-25 early-out is now redundant but kept (cheap, guards a mesh from before).
- Review (fresh subagent on the diff): no serious finding; glass's MAX_CHUNKS cut dropped the nearest panes, not
  the farthest (fixed); a pre-existing overlap of the crack bits (`ck << 20`) with the twig and open-on-air bits in
  `mesh.rs` noted for the mesh session (not from this batch).
