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
   1.7 times larger, stood about 0.35 m out of the crown and tilted 25-35 degrees off its face about a line
   turned by a hash of the place (per face, so the plane stays flat), so no plane is seen edge-on along an axis
   as a thin rod and neighbours differ; inside a ragged round of texels (the varying `plane`). Its painting is read
   in the plane's own coordinates (32 texels a metre, tiling past the face's edges), never stretched. A rooted one
   is hinged on its wood's side, one face long: that edge stays on the face, the far edge stands 0.55 m out
   (28 degrees), twisted a little about the hinge, its round mask centred on the hinge so the tips are ragged,
   so bare twigs fan out of the crown from the wood. Fluff is not traced, and far levels stay one colour.
   `fluff=0` on a run leaves it out.
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

7. **A bare crown is lit like bark.** A crown block where winter outweighs the season mix takes its winter
   painting alone (no summer leaf tint mixed in at a mid latitude; `bare_of`, `is_bare`). To rays, fast leaves were
   whole blocks in the BLAS's opaque geometry, so a bare crown shadowed itself and its twigs were near black. Now
   level 0's fast leaves are the BLAS's second geometry too (`frame.rs` `shell`, chunk info bit 18): a ray takes
   them whole unless they are bare where they stand (the season at the ray's start with the hit column's stagger,
   `hit_season`), then it passes between the twigs. Fancy leaves take the season's texel the same way. Light falls
   through a winter crown onto the ground, and each twig has a lit and a shaded side like the trunk.

8. **Every plant's fluff is data, in clumps, with its own shade** (the developer: the target picture
   `out/flora-refs/6-leaf-block-target.png`, a crown of bulging lobed clumps, bright on top, dark under and
   inside; "everything biological more or less, single flowers and mushrooms not"). A cutout block's row takes
   `fluff = { size, push, droop, long, dense }` (`blocks::Fluff`; unnamed numbers are the broadleaf's 1.7, 0.35,
   0, 1, 1; size 0 none). It rides in the look row's new word 16 (`Fluff::packed`, six bits a number; the row grew
   from 16 to 20 words). `vs_fluff` reads it: `long` stretches the plane along the way from the wood (fronds,
   blades), `droop` lowers its outer parts, `dense` keeps that share of faces. A smooth field of the place about
   2.5 m across (`vnoise`) makes clumps: where it is high a plane is up to 1.4 times larger and pushed up to 2.2 times out, where
   low it stays on the face or keeps no plane, so a crown bulges in lobes with dents. Each cutout quad and plane carries a
   shade in `ao`: darker toward its wood (the steps to it), under (a face down 0.6) and in a dent. The rows:
   spruce short, drooping and long; jungle large and drooping; acacia wide and sparser; shrub small; reed narrow
   blades 1.8 long, near upright (a hinged plane leans by the push: 28 degrees at 0.35). Species to come (palm, bamboo, willow) take their rows the same way.

9. **Leaves shade, not block, the sun** (the overseer: a closed canopy was near black inside). Fast leaves are
   already the BLAS's second geometry (decision 7), so a shadow ray meets each leaf face as a candidate: it
   multiplies `ray_tint` (glass's path, ADR 0081) by the face's `fluff.light` (the block row: broadleaf, jungle and
   shrub 0.5, spruce 0.35, acacia 0.65, reed 0.7), a little green, and goes on; below 2 % left it stops. A ray
   through a crown passes two faces or more, so a broadleaf's shade lets about a quarter through, two crowns a
   sixteenth. Bare crowns take their twigs' texels as before (nearly no shade). Looking rays (bounce, reflections)
   still take a leaf block whole. The overseer's plan (opaque BLAS, a fraction at the committed hit) would not see
   the second crown and needs the geometry flag per season anyway; the candidate path was already paid for.
   Also: a top face's fluff stands 45-60 degrees and a third as far out, leaves rising out of a shrub, not a slab.

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
(sim), more draws to record; jobs +8 % in fly (the walk to wood, more quads). The look fixes (decision 7: fast leaves
as ray candidates; the planes) then cost fly +0.4 ms GPU p50 (3.64 to 4.04) and +0.23 ms on the frame thread
(the BLAS's second geometry), p99 7.28 to 8.13, inside the budget by 0.2 ms; sim flat. Measured once on a quiet
PC before decision 8 (data fluff, clumps), which adds a `vnoise` per fluff vertex; not measured again (ADR 0096). The row and its ideas are in
`docs/perf-debt.md` (ADR 0096: no further cost work here).

## Review of the data fluff (decision 8)
A fresh reviewer read the diff: the look row's layout (80 bytes, stride and buffer), the bits and the branches.
Fixed: a hinged plane's round mask followed its size across the hinge (reed blades square, jungle ones cut
thin), now in a 1.7 plane's units like the rest; `droop` lowered a whole plane by one amount, now each corner
by how far it stands out, so planes bend down away from the crown; push and droop step by 0.025 (the default
0.35 is exact); `fluff` on a block that is not cutout is refused by name; a stale "64 bytes". Noted: coarse
levels carry no root bits and coarse cells, so their shade and clumps differ from level 0's and a crown's fluff
changes where a chunk changes level (some 200 m out at the default Detail).

## Review of the look fixes (decision 4's planes, 7)
A fresh reviewer read the diff. Fixed: the ray's season had no column stagger, so a tree drawn bare during the
turn of the season could still shadow as a whole block (`hit_season`); the planes folded along their diagonal
(a per-corner hash, a bilinear twist; both per face and linear now); hinged planes ended square (the mask is
centred on the hinge now). Noted: the round mask touches the plane's edge midpoints, as before. Cost: below.

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
- From straight below, a bare crown's own vertical leaf faces (not fluff) are seen edge-on and read as short
  dark bars; branch logs show as cubes inside a bare crown (part C's shapes).
- Leaves' shade: a quarter of the sun under one broadleaf crown (decision 9); ADR 0088's call on solid shadows
  under bare shrubs is answered (light through the twigs).
- Palm, bamboo and willow are not on this branch (part C's species): their rows take `fluff` the same way; no
  stills of them here.
- The topmost twigs in full sun read lighter and warmer than the calmed bark: part D's twig painting is a tan
  family; one palette with `log_side` would make one tree one colour.
- How far the fluffy look reaches: all of level 0, which a level 1 chunk (64 m) hands down within Detail x its
  edge (4 x 64 m at the default), so about 200 m; past it a cutout block is one colour. Part D has far paintings
  (`data/textures/hd/far/`, 8 px a face) ready as the next step for the first coarse level; not read yet.
- Twigs shorten to nothing three blocks from the wood; big crowns may want a longer reach once part C's
  branches are in.
