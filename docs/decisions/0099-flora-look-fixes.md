# 0099 – Flora look fixes: crowns' fluff as cards over the faces, bigger clutter, taller grass

- **Status:** proposed (tool tier for the paintings; the shader waits for the developer, CLAUDE.md principle 13)
- **Date:** 2026-11-13
- **Reworks:** ADR 0090 decision 4 and 8 (the fluff's shape); data from ADR 0082 and 0091

## Context
Two F9 reports of 2026-11-13:

- `14-25-54`: "the way extra detail comes from leaves and shrubs is very ugly and needs a nice rework". ADR 0090's
  fluff drew every open leaf face again as a plane 1.7 faces wide, stood up to 0.55 m out and tilted 25-35
  degrees (hinged on the wood's side, 28 degrees out, up to 1.4 faces long). Re-shot at the report's view
  (`out/flora-look/before-bug2.png`, `before-shrub.png`): a shrub reads as a cube with big flat slanted sheets
  of streaky leaves standing out of it, and a tree crown as a box with sheets poking off its top. A sheet seen
  at a slant stretches its texels into streaks, and a plane that size is a plane from every angle.
- `14-24-52`: "Twigs, stones too small, this grass is kinda small but a few here and there as young ones is
  ok". Twigs were three sticks 0.44 m by 3 cm, pebbles 6-12 cm, the loose stone 25 cm, the stick 0.56 m by 3 cm;
  short grass was 0.4 m high in all four outlines.

## Options considered (fluff; stills in `out/flora-look/`)
1. **A: a card over the face** (chosen). Each open face's one fluff quad becomes a card 1.4-1.9 faces wide lying a
   hand over the face (0.03-0.17 m out), tilted only 4-12 degrees. Its mask keeps the face (corners rounded) and,
   past the edges, lobes of leaves from a smooth field about 0.3 m across, up to the card's overhang (0.2 to
   0.48 m at the broadleaf's `size` 1.7, by the clump field), in whole texels with a ragged rim. The painting is
   the block's own at 32 px, shifted half a face (the same for every card) so the card does not repeat the face
   under it. A shrub reads
   as a lumpy bush, a crown as a rounded mass with a bumpy outline (`a3-shrub`, `a3-crown`, after `after-*`).
2. **B: clump cards.** One smaller card a face (0.55 m plus the overhang), centred off the face's middle, tilted
   15-30 degrees, a round lobed mask. Same quads. Most clumps stayed inside the face, so the cube's edges showed
   again; the crown was boxier than A (`b1-shrub`, `b1-crown`). Not kept.
3. Several small cards a face: more triangles (the brief: same order as today's fluff), not built.

Variants of A tried: overhang 0.12 x `size` (still a box at 4 m, `a1-shrub`); the clump field leaving faces
without a card (a whole small shrub could fall in a dent and stay a cube, `a2-shrub`): every open face now gets
its card (`dense` still thins them), the clump field sets only how far it spills. A card under a crown stood off
and spilling read from the side as a slab floating under the crown with the sky between: the bottom face's card
stays at 0.3 of the push and overhang, and a top face's at 0.5 (it read as a lid on a shrub from the side).

## Decision
- **Fluff = A** (`terrain_vertex` mode 2 and `fluff_in`, shader.wgsl). The hinged planes are gone: bare twigs no
  longer fan out of a crown as planes; a winter crown's cards show its twig painting the same way (turned by the
  root bits as the face is). `VOut.plane` is a `vec3f`: the card's point from the face's middle and its overhang.
  `Fluff.long` is no longer read (kept in the schema for frond species' later shapes); `size`, `push`, `droop`,
  `dense` and `light` are, with new meanings for `size` (overhang 0.2 x size metres) and `push` (a fraction of
  it, 0.1-0.4 by clump). No block row changed.
- **Lit as the face.** A card takes its face's normal and the existing shade (toward the wood, the clump's
  dents, the bottom 0.6, a top face's 0.72), so it reads as the same leaves as the cube under it.
- **Clutter** (`tools/paint_clutter.py`, data/models): twigs a fallen branch 0.88 m and 6 cm thick, a 0.63 m
  stick and a 0.38 m twig; pebbles stones of 0.16-0.34 m; the loose stone 0.38 x 0.19 x 0.31 m; the stick 0.81 m
  and 6 cm thick.
- **Short grass** (`tools/paint_textures.py`): three outlines 0.75 m high with a third more blades; the fourth
  (`.3`, picked by hash, one in four) stays the 0.4 m young tuft.
- Test: `clutter::tests::clutter_reads_from_standing_height` (seen failing on main's data).

## Consequences
- Fluff draws the same quads as before; its cards are about the size of the old planes (1.4-1.9 faces against
  1.1-2.4), so the pixels shaded are of the same order. Cost below and in `docs/perf-debt.md`.
- Crowns and shrubs change in every view with trees or shrubs (the seasons views, the spawn's forest, lod and
  high); the clutter and grass in every grassland view (`tools/views.sh accept` after the gate).
- Willow, palm and pine (unpainted drafts) lost their long drooping hinged fronds; they are a crown like the
  broadleaf's until their own shapes come.

## Review
A fresh reviewer read the shader and test diff. Fixed: `droop` sank a top card into its face (willow, droop 1.0,
always), now sides only; the tilt could put a card's corner up to 0.15 m inside its block (jungle), now never
less than 0.03 m out; the lobe field mixed a coarse chunk's cells with level-0 base cells (a seam at far chunks'
edges), now in the chunk's own cells as `terrain_texel`; the painting's per-card shift moved the lobe field per
card, so neighbouring cards' lobes did not meet at a shared edge: one shift for all cards now. Checked and fine:
the mask's texels against the painting's, `VOut.plane` (only shader.wgsl, no other reader), the depth and colour
passes' invariance, the face never cut by its own mask, the test's face layout.

## Calls for the developer to confirm
- The look of A (stills listed in the hand-over); the overhang and push are one place in the shader to tune.
- Bare winter crowns: twigs no longer fan out as planes from the wood (ADR 0090 decision 4); the cards carry the
  twig painting at the crown's edge.
- Shrubs stay yellower than tree leaves: that is the paintings' colour (ADR 0090 calls), not the fluff; no tint
  added here.
- One young short-grass tuft in four.

## Numbers
One measurement each (`tools/slot.sh measure`, play build, `run=fly quick=1`), main's shader against this one,
both with the new clutter and grass: GPU p50 4.89 to 5.05 ms (+0.16), G-buffer 2.99 to 3.14 ms (every open face
keeps its card now, and one `vnoise` per fluff pixel), quads the same (830k), frame thread flat, jobs 464 to 454
us, p99 9.48 to 9.09 ms (noise; both over the 8.3 budget as before, ADR 0096). Row in `docs/perf-debt.md`.
