# 0090 – Fluffy crowns, twigs from the wood, leaves in the wind

- **Status:** proposed (flora part A, `docs/design/flora.md`; ADR 0089)
- **Date:** 2026-11-13

## Context
ADR 0088 drew fast leaves with open holes and cost the forest fly-through about 1 ms of GPU (p99 6.3 to 7.4 ms,
budget 8.3). The developer wants fluffy crowns (planes past the block), leaves that sway, and bare winter crowns
whose twigs come out of the log instead of "a block with random sticks flying".

## Where ADR 0088's cost came from
Fly, quick, three runs each, PC quiet, under the lock (`gbuffer` is the G-buffer pass's GPU time):

| | p99 ms | GPU p50 ms | gbuffer ms | CPU `work_ms` | quads |
|---|---|---|---|---|---|
| main (before 0088) | 6.20-6.33 | 2.87-2.91 | 0.96-0.99 | 0.57-0.65 | 659k |
| 0088 as built | 7.32-7.43 | 3.75-3.89 | 2.00-2.01 | 0.58-0.60 | 712k |
| 0088, leaf quads not drawn | | | 0.86-0.91 | | |
| 0088, leaves through the opaque pipeline | | | 1.64-1.66 | | |
| 0088, leaves drawn before the terrain | | | 1.56-1.62 | | |

The CPU side did not move (sim `work_ms` 0.59-0.68 against 0.64-0.65). The GPU cost was order and late depth: the
leaves went last, so the ground behind every crown was shaded first and then overdrawn, and a discarding shader
gets no early depth.

## Decision
1. **Two passes for cutout quads** (`gpu.rs` `cut_depth_pipeline`, `cut_lit_pipeline`): their depth alone first
   (`fs_cut_depth` discards empty texels and writes no colour), then the opaque terrain, then their G-buffer with
   `Equal` and no discard. Each pixel is shaded once, with early depth everywhere. ADR 0088 stays the base.
2. **Twigs from the wood** (mesher). Block rows get `wood = true` (`log`; part C's branches). `mesh::roots` walks
   out from wood through the cutout blocks, up to three steps (a cutout block on an opaque non-wood block roots in
   it, below: a shrub's twigs grow from the ground). Every cutout quad carries its way to the wood in free bits of
   its high word: bits 20-22 the side of the face the wood is on (1..4 u-, u+, v-, v+; 5 behind the face; 0 none),
   bits 23-24 the steps less one, bit 25 the face opens on air.
3. **The painting is turned** (`texel_a`): a cutout block's painting has its twigs rooted on its **bottom edge**;
   the shader turns it so that edge lies on the wood's side, or folds it about the middle when the wood is behind
   the face. A winter painting of four variants is picked by role (`winter_texel`, part D's layout): beside the
   wood v0 or v1 by hash, a step on v2 (a limb through), two on v3 (tips); a bare block with no way to wood
   within three steps draws nothing, so a crown thins to its edge and nothing floats.
4. **Fluff** (`vs_fluff`, `fs_fluff_depth`, `fs_fluff_lit`): every cutout quad that opens on air is drawn again,
   1.7 times larger and stood 0.3-0.55 m out of the crown, turned per corner by a hash of the place, inside a ragged
   round of texels. A rooted one is hinged on its wood's side: that edge stays on the face, the far edge stands
   out, so bare twigs fan out of the crown from the wood. Fluff is not traced: to rays leaves stay whole blocks
   (ADR 0069, 0088), and far levels stay one colour. `fluff=0` on a run leaves it out.
5. **Sway** (`sway`, in `vs_cut` and `vs_fluff`): a smooth field of the place and the sky's clock (`g.water.x`),
   so faces that share an edge move together; amplitude from the wind here (`g.water.w`, frame.rs: 0.5 m/s plus
   the weather's wind, ADR 0076), more at a fluff plane's free edge. In a shot `g.water.w` is 0: the fixed views
   stand still.

6. **Flora at 32 px a metre** (the developer, 2026-11-13: "still too low quality of details and colours"). A
   painting 32 px high (32, 64, 96 or 128 wide: 1 to 4 variants of 32 x 32) is a fine painting; the file's
   height says it, no block row changes, and 16-high files stay valid, so every other block is unchanged.
   `engine::textures::decode_fine` keeps each fine variant as its four 16 x 16 quarters in turn (top left, top
   right, bottom left, bottom right), so the texture buffer, its layers and `place` stay as they were; the face
   word's bit 14 marks it (`looks_of`), and `paint_texel` reads it at 32 texels a metre (`face_xy` maps both
   grids, with the twig turn). Unpainted patterns stay 16. A fine face takes four layers a variant of the 2048
   (2 MB, unchanged); the GPU cost is one branch per painted texel. The Studio's check and textures page take
   the size (see below).

## The Studio (tool tier)
`art::Img` has a height: a fine file opens 32 x 32 a variant (the engine's quarters put back together), saves at
its own size, and `check=1` and `check_dir` take 32-high files of the allowed widths for any block. The textures
page's canvas, grid and every pointer tool work on the open face's own grid; previews and thumbnails draw a fine
face at the same 1 m size. A new painting is 16 unless any file of the block is already fine, then 32. Patterns
and "Start from a picture" still make 16 px faces (on a fine face they paint its top-left quarter). The
Studio's preview picks variants by hash only; it does not show a winter crown's pick by role. Tests:
`art::tests::a_fine_texture_round_trips_through_its_png`, `check_takes_a_fine_cutout_file_of_a_width_the_rule_allows`,
`drive::a_fine_painting_paints_at_its_size_and_a_new_face_takes_it`.

## Hooks for blossom and fruit
A season's look is a painting `data/textures/<block>@<season>.png` (spring, autumn, winter; the base is summer).
Blossom is the spring painting (`broadleaf leaves@spring.png` already is). Fruit is named **`@fruit`**:
`<block>@fruit.png`, shown in late summer. It is not drawn yet (no fifth look row); part D's `TAGS` can take the
tag now. All seasons' paintings of a cutout block follow the bottom-edge rule above.

## Painting layout for part D
Agreed with part D (2026-11-13): `<species> leaves@winter.png` is four variants, the wood on the **bottom
edge** (row 15, or 31 at 32 px) of each: v0 one stem entering near the middle and fanning out, v1 the same
entering off-centre, v2 a limb through from bottom to top at the middle with side twigs, v3 sparse short tips.
Top and bottom faces follow the sides' rule; with the wood behind the face the painting folds about the middle
(the lower half mirrored up), so its lower half must read on its own. The 32 px paintings live in
`data/textures/hd/` (read in place of their 16 px namesakes, `textures::HD`); main's engine before this branch
refuses 32-high files, so part D kept them there; they may move up once this is merged. Other seasons' leaf paintings are turned the same way; isotropic
leaves need nothing.

## Consequences
- Cutout quads are drawn four times with fluff (two passes, two planes); cost below.
- The bit layout of a cutout quad is fixed by `mesh::twig_bits`; joins (bits 16-19) stay lamps' only.
- Part B adds `Globals.plants` (wind as a vector at the camera). When it lands, `sway` can read it instead of
  `g.water.w` and lean the crowns downwind.

## Numbers
One measurement, the PC alone (`tools/slot.sh measure`), play build, quick runs, median of three, 32 px leaves:

| | GPU p50 ms | G-buffer ms | frame p99 ms | frame thread CPU ms | jobs us |
|---|---|---|---|---|---|
| main, fly | 2.90 | 0.99 | 6.82 | 0.68 | 352 |
| this, fly | 3.91 | 1.97 | 7.52 | 0.74 | 381 |
| this, fly, `fluff=0` | 3.26 | 1.48 | 6.78 | 0.60 | 378 |
| main, sim | 2.85 | 0.72 | 6.03 | 0.74 | 422 |
| this, sim | 2.93 | 0.80 | 6.34 | 0.84 | 421 |

Fly's p99 holds the budget (8.3 ms) with fluff on. GPU +1.0 ms in fly: fluff 0.65, the rest the leaf faces no
longer merging (742k quads against 659k) and the two passes. CPU: the frame thread +0.06 ms (fly) and +0.10 ms
(sim), more draws to record; jobs +8 % in fly (the walk to wood, more quads). The row and its ideas are in
`docs/perf-debt.md` (ADR 0096: no further cost work here).

## Review
A fresh reviewer read the diff against ADR 0088's branch (mesher, shader, gpu.rs, frame.rs, textures.rs).
Fixed:
1. Level 0 meshes on a lattice of up to 8, so open leaf faces merged and a merged quad carried one cell's root
   and one fluff plane (crowns at 20-30 m kept flat box tops in the stills). Cutout faces never merge now
   (test `leaf_faces_know_the_way_to_the_wood` at lattice 8, seen failing first).
2. The walk to wood covered the inner cells only: it now covers the padding, so a crown across a chunk edge
   roots through its neighbour's cells (test: through a padding leaf to a padding log). Wood more than the
   padding away is still unseen (a `shortcut:` in `roots`).
3. The watcher did not see files changing in `hd/`: it now looks one folder down.
4. `@invariant` on the position, so the depth and colour passes meet with `Equal` by rule, not by luck.
5. The steps-0 variant pick could reach variant 2 when the hash was 1.0; clamped.
6. Sway jumped when the clock wrapped at 3600 s: its frequencies are whole turns an hour now.
7. The walk went through glass: cutout blocks only now.
Answered: "sway slides the outline over a still painting" and "fluff tiles its painting" do not hold: `o.local`
(the painting's coordinates) is the quad's own unswayed corner, so the painting rides with the vertices and a
fluff plane spreads its face's painting once. Noted: cutout quads go through the vertex stage four times with
fluff; the Studio read a fine file as quarters (the Studio change below).

## Fixed views
The cutout blocks' look changes everywhere they show: every view with trees or shrubs at level 0 (the seasons
views most, the spawn and forest views), and their shading through the twig turn and 32 px paintings. Shots stand
still (no sway). Not re-accepted here: the overseer runs the views on a quiet PC after the merge.

## ADR 0088's fate
Kept as the base: its mesher rule (`Cut::Shell`) and the cutout pipeline stay; its cost came from drawing the
leaves last and is won back by decision 1 above. This branch carries it to main.

## Calls for the developer to confirm
- Fluff on by default (see the numbers); `fluff=0` is a run argument, not yet a setting.
- Bare crowns' fluff planes seen edge-on from below read as thin rods; and branch logs show as cubes inside
  a bare crown (part C's shapes).
- Whole-block shadows under crowns stay (ADR 0088's call).
- Twigs shorten to nothing three blocks from the wood; big crowns may want a longer reach once part C's
  branches are in.
