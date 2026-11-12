# 0068 – See-through leaves take light through a candidate loop; still box models are 8-byte quads in the chunk mesh

- **Status:** accepted (measured in a spike; steps 2 and 6 of `docs/design/see-through-and-boxes.md` build it)
- **Date:** 2026-11-12

## Context
The developer wants see-through leaves, glass, small models of painted boxes at 32 pixels per metre and, later,
forest floor clutter (`docs/design/see-through-and-boxes.md`). Sun, lamp and bounce light are hardware ray
queries against one opaque BLAS per chunk (ADR 0022, 0025); painted textures sit in a 2 MB texel storage buffer
(ADR 0045); models today are flat-coloured voxel parts posed per frame on the CPU, about 1000 pieces at most
(ADR 0047). The design's build order starts with a measured spike whose answers decide how cutout light (step 2)
and still models (step 6) are built. Constraints: ADR 0004's budget, and the developer's rule that the game
will be CPU-bound, so CPU costs rank first.

The spike is `prototypes/see-through/` (README: every table, run line and picture); its code sits on the branch
behind run arguments (`leaves=`, `light=`, `clutter=`, `mblas=`) and leaves the game as it was when they are off.

## Options considered
Light through cutout blocks (forest at noon, fancy leaves; GPU ms p50, stand / fly; today 2.23 / 1.78):
1. Opaque to rays – 2.92 / 2.21. Holes in the crowns, a solid block shadow under them.
2. On a hit, read the texel at the hit and go on through an empty one, five queries at most – 3.62 / 2.97.
   Dappled shadow, but a crown's many faces in a row cost repeated queries, and the step limit is a cut-off.
3. Non-opaque geometry and a candidate loop that confirms what is not empty – 3.33 / 2.79, exact, even with
   every chunk flagged non-opaque in the spike. Same dappled shadow as 2.

Fancy (faces between leaf blocks kept) against fast (dropped, opaque, mean colour = today): +34 % mesh time
(102 to 137 us a chunk on one worker thread), +40 % quads, +0.4-0.7 ms GPU. The frame thread's own work
stayed 0.8-1.1 ms in every option.

Where a box model's paintings live: (a) layers of the existing texel buffer, (b) a second buffer. The same
memory either way; (a) needs no new binding in the terrain pass.

Still models in the chunk mesh, by density (meadow, stand): 1 per 16 columns +0.08 ms GPU, 1 per 4 +0.19,
every column +1.0; mesh time 159-176 us a chunk against 102; the frame thread's work flat (0.54-0.91 ms with
13 000 model quads a chunk). In the BLAS: +0.05-0.12 ms GPU but +160 MB (1 per 4) to +640 MB (every column)
of VRAM.

## Decision
- **Light: option 3.** Each chunk's BLAS gets two geometries: its opaque quads flagged OPAQUE, its cutout (and
  later glass) quads not, so only leaves enter the candidate loop. The loop confirms a candidate unless the
  texel at its barycentric point is empty. Shadow and bounce rays both use it. Glass will add its tint there.
- **Leaves: fancy is the default**, fast the setting for weaker PCs. Culling reads one bit per block id
  (opaque or not); coarse levels draw cutout blocks opaque in their mean colour.
- **A still model's quad is 8 bytes**, as a terrain quad: the cell, a turn of 16, a place in the cell, a flag bit;
  and the face's index in a **face table** that holds each model face once (origin, two edges, its painting's
  rectangle, 48 bytes). Nothing else is stored per placed model.
- **Paintings live in the block textures' buffer**, in layers after the blocks' (option a). A second buffer only
  if they outgrow it, and raising its layer count comes first.
- **Clutter default: 1 per 4 columns of its ground, on level-0 chunks** (the range follows Detail); every column
  as the "many" setting. **Clutter is not in the BLAS**; a block row's lone model or machine may be.

## Consequences
- Seeing through leaves costs about 1 ms of GPU in a forest and nothing on the frame thread; clutter costs 0.2 ms
  at its default and nothing on the frame thread at any count. Both are worker and GPU costs, which suits the
  CPU-bound game.
- The BLAS expand pass and the bounce pass now bind the texel buffer (they read model faces and empty pixels).
- The mesher must group cutout quads after the opaque ones within a chunk's land quads, for the second geometry,
  and still models after those (they stay out of the BLAS by count).
- Clutter shades like the ground around it but casts no shadow; a model that must cast one goes in the BLAS.
- To revisit: the forest fly-through's frame-time tails with fancy leaves (8.9-12.5 ms p99 in most runs on a busy
  PC, against 5.7-15 for today's) need a quiet rerun before step 2 ships; option 3 with the two-geometry split is
  predicted cheaper than the spike's, not measured; a real painted leaf texture changes how many candidates a ray
  meets; the clutter scan should read the mesher's occupancy bits instead of every cell (57 us of its 74).
- Cutout quads draw in a pipeline of their own (the fragment shader that discards), so opaque terrain keeps early
  depth; a cutout face's holes are hashed by its plane, so the two faces between neighbouring leaf blocks agree.
- Review: a fresh reviewer read the spike's diff before the commit. Nothing broke with the switches off. It found
  the clutter scan skipping a chunk's layer 0 (fixed; counts a few percent low, not rerun), the meshbench's
  best-of-five leaking into plain meshbench runs (fixed), coplanar leaf faces making light 2 brighter than light 3
  (recorded; the decision stands on cost and exactness), the discard's early-depth cost (the pipeline split above),
  and a wrong model normal in bounce rays with models in the BLAS (spike only). `prototypes/see-through/README.md`,
  "The review".
