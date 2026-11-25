# Plants and creatures

Structure files and grown flora, trees and their crowns, small plants, the flora direction and look, creature rows
and the walk rig, and the peaceful animals of the world. What holds today. Numbers in brackets are the ADRs
(`archive/NNNN-*.md`) a decision came from; INDEX.md lists every number.

## The developer's calls

- **Flora direction** (2026-11-13, Vintage Story as the reference) [0089]: fluffy crowns; small plants as crossed flat
  planes; branches and more species of tree (no giant trees for now); and all four extras: wind sway, colour by
  climate, dense meadows, flowering and fruit.
- **Flora is painted at 32 px a metre**, other blocks stay at 16 ("still too low quality of details and colours").
  Reason: crowns at 16 over a meadow at 32 would not match [0089, 0090, 0091].
- **Rich colour for flora**: six to eight tones with hue shifts (warm highlights, cool shadows), saturated, two or
  three variants per plant. This replaces "calm" for flora only [0089].
- **Crowns are clumps, lobed**, after the target picture `out/flora-refs/6-leaf-block-target.png`: bright on top, dark
  under and inside; fluff on everything biological more or less, single flowers and mushrooms not [0090, 0092].
- **Plants take and cast light** ("should also interact with light so they cast shadows and feel like they got
  depth") [0091].
- **Reeds**: "flat and too uniform", so four outlines and an inside-the-bed painting [0091].
- **Fluff look** (F9 "the way extra detail comes from leaves and shrubs is very ugly"): cards over the faces, not
  sheets standing off [0099]. **Clutter and grass sizes** (F9 "twigs, stones too small, this grass is kinda small but
  a few here and there as young ones is ok") [0099].
- **Forests are denser, with clearings**, so crowns meet and sun reaches the floor; trees keep rivers open [0092].
- **Herds drift while nobody is near** (2026-11-13) instead of living on unseen [0083].
- **Hostile creatures are still the developer's open call**; 0083 covers peaceful animals only [0083].
- **Rigged by choice**: a creature's walk comes from its body plan and a few numbers, never keyed by hand [0053].
- The developer's taste calls that stood open when these were written, and are still open unless a later decision
  settled them, are listed under "Open".

## Structure files and grown flora

(also studio-and-art.md for the Studio's Structures page)

- **`data/structures/<name>.toml`**, read by `engine::structures` (`Structure::parse(file, text, &blocks)`; every
  refusal names the file and the field, or the layer, row and column; `write` is byte-exact on its own output) [0052]:
  - `size = [x, y, z]`, 1..=64 blocks a side (`MAX_SIDE` 64).
  - `anchor = [x, y, z]`: the cell set into the place it stands on; for a tree the trunk's foot, set into the first
    air over the ground.
  - `layers`: bottom up, a blank line between layers, a row per z, a letter per x; `.` leaves the world's block, `_`
    clears it to air.
  - `[key]`: letter to a `blocks.toml` name.
- **A grown structure** is `[grow]` alone: `seed`, `wood`, `leaves` (block names), `trunk_height` 1..32 blocks,
  `trunk_girth` 1..3, `branches` 0..12, `branch_angle` 0..90 degrees from up, `branch_length` 0..12 blocks, `crown`
  round | cone | flat, `crown_size` 0..12 blocks (radius), `leaf_density` 0..1 [0052].
  - `grow(&Recipe)` is deterministic (splitmix64; a leaf's chance hashes its cell, so order does not matter): the
    trunk, branches up its upper half spread by the golden angle each with a tuft, the crown about the trunk's top;
    cut to 64 a side about the foot. A bush is the same recipe with a short trunk. "Freeze" drops the recipe and
    writes the layers.
  - This grower is the Studio's tool for hand work; the trees of the planet are the separate `Plant::block` below.
- **Seeing one**: `run=stand structure=name[:seed]` sets its blocks at the stand point (x along j, z along i, the
  camera frame of `models::cell_axes`). The Studio sends the blocks through the existing `place` of `run=view`, with
  a clearing of air 4 blocks round it [0052].
- **`/structure <name> [seed]`** on the server stands a file of `data/structures/` with the anchor in the first air
  over the ground five metres ahead of the player; axes as `structure=`; cells that would leave the face or hold a
  player are skipped; the blocks go out as any edit. The name is letters, digits, `_` and `-` only; Windows device
  names are refused; a structure over 4096 blocks is refused; an anchor not found within 24 blocks refuses. Anyone
  may run it until the server has roles. The file is read on the tick thread; a replay of a session that used it
  needs the same file [0052].
- **Bad stand arguments**: `place=`, `fxedit=`, `structure=` (and `creature=`, `model=`, `fxplay=`'s count) are read
  by `client::stand_args`; bad input prints one line naming the argument on stderr and exits 1, never a panic [0052].
- **`run=play lines=stdin`**: each line written to the game's stdin runs as if typed into the chat and sent (a thread
  reads; the frame thread takes four a frame; control characters dropped; a line is 200 characters at most; read in
  pieces of 4096 bytes). It is for the program that started the game: the Studio's Try in game. A line the player is
  typing by hand is kept [0052].
- **Tab completion** for `/structure` and `/spawn` and their names is on the client (`play.rs` `complete_with`), not
  in `engine::net::complete`; asked only for a line starting with `/`; the names are this machine's files, read at
  most every two seconds; names compare without regard to capitals [0052].
- The stand run stands at the spawn (open sea on the baked planet) unless `at=` names land; its camera stands in the
  structure's anchor cell [0052].
- **Not done**: placing structures on the planet by the generator; lean and root flare; placing and digging in the
  3D view [0052].

## Trees of the planet

Trees stay computed from the seed (ADR 0043) and are never stored. Every reader (mesher, physics, water, rays) takes
plants through `Plant::block` [0092].

- **`Plant::block(h, v, dx, dz, z)`** takes the tree's own hash `v` (`world::Foot::v`, from the spot's hash), so trees of
  one species and height differ. It stays a pure function of seed and place, inside the plant's radius and height.
- **`REACH` = 6** (`biomes.rs`): an oak's crown is 13 blocks across. `PLANT_TOP` rises with it; the margin and
  `clutter::ground`'s seam band are 6 columns [0092]. Trees stand with their feet anywhere in a face; a column within
  `REACH` of an edge also reads the next faces' spots (`plants_with`, `unfold`); round a cube corner a crown is cut
  at the corner's diagonal [0100, amended 0092's margin].
- **Sizes**: where a tree's height falls in its row's range makes it small, medium or large (`Plant::size`); its
  crown is 2, 1 or 0 blocks narrower than the row's radius, at least 1. A height range must span at least three
  values [0092].
- **Branches are the species' log blocks** inside and under the crown. No separate branch block kind [0092].
- **Limbs are segments.** A cell is wood where a segment passes through its cube (a supercover), so a limb steps out
  and up face to face and a bare tree is one skeleton. Directions come from a table of sixteen, not `sin` and `cos`,
  so a Linux server and a Windows client make the same blocks; everything else is `+ - * /`, `round`, `powi(2)`
  [0092].
- **Shapes** (`data/biomes.toml` documents the keys) [0092]:
  - `round`: a crown of clumps (one at the top, one at each branch's end), each 2 or 2.5 blocks across each way and
    0.85 of that up and down, each its own length out and a block or two under the top by the hash. The trunk leans
    a block in two trees of three, up to the top clump; a large `thick` tree stands straight, on 2 x 2 below its
    fork. At `fork` of its height, `limbs` limbs spread to half the crown's radius; on a crown of 3 or more each limb
    parts into two branches. On a crown of 4 or more a clump's outer leaves fall by the hash, only where the cell a
    step towards its middle is core, so no leaf hangs alone. A clump's lowest leaf is two over the ground; facing
    branches of neighbouring limbs have their own clump sizes.
  - `flat` (acacia): a lens of leaves `depth` thick each way, at most 0.7 of its radius, over limbs spreading to two
    thirds of its radius, each parting into three branches.
  - `droop` (willow): an umbrella whose rim sags. A wooden withe hangs from each limb's middle branch (it carries
    the winter twigs). Strands of leaves one block wide hang on every other column each way of the rim beside a
    withe, most to one or two blocks over the ground and one in four half way.
  - `cone`: bare under `bare` of its height, then tiers from the top down, wider as they go, with a gap layer under
    each; a tier's lower layer carries limbs along three or four axes; leaves clump within a block of a limb or the
    trunk.
  - `palm`: four fronds along the axes that bow down at their tips, and short ones between in most quarters.
  - `column` with leaves and a radius (bamboo): a clump of stalks of different heights, leaves at their tops.
  - `bush`: one layer, low, wide and lopsided: a clump of the middle, most of its four sides, a corner only beside a
    side (one corner always open), and on one side by the hash a second smaller clump two out; two to four blocks
    across.
- **Species** (ids 206 to 217, after the rubble and halo rows; each has its own log and leaves; leaves are
  `see = "cutout"` with the `leaves` pattern; logs are plain bark, since rings on a log's sides read as a 2 x 2 post)
  [0092]:

  | Plant | Shape | Height | Radius | Keys |
  |---|---|---|---|---|
  | spruce | cone | 7-14 | 3 | bare 0.15 |
  | pine | cone | 8-16 | 2 | bare 0.55 |
  | broadleaf | round | 5-9 | 4 | fork 0.4, limbs 3 |
  | oak | round | 5-10 | 6 | fork 0.35, limbs 4, thick |
  | birch | round | 7-13 | 4 | fork 0.45, limbs 3 |
  | jungle tree | round | 10-16 | 5 | fork 0.65, limbs 4, thick |
  | acacia | flat | 5-8 | 5 | fork 0.55, limbs 2, depth 0.8 |
  | willow | droop | 6-10 | 5 | fork 0.55, limbs 4, depth 1.5, thick |
  | palm | palm | 5-10 | 3 | warm 15 C |
  | bamboo | column | 6-12 | 2 | |
  | shrub | bush | 1-2 | 2 | |

  Spruce, broadleaf, jungle tree and acacia keep the shared `log`.
- **Plants per biome** (share of candidate spots, one each 4 x 4 columns) [0092]:

  | Biome | Plants |
  |---|---|
  | cold forest | spruce 0.32, pine 0.14, birch 0.06, shrub 0.05 |
  | temperate forest | broadleaf 0.18, oak 0.12, birch 0.05, willow 0.01, shrub 0.10 |
  | grassland | broadleaf 0.008, oak 0.007, shrub 0.04 |
  | savanna | acacia 0.04, shrub 0.03 |
  | rain forest | jungle tree 0.34, bamboo 0.07, palm 0.03, shrub 0.20 |
  | wetland | willow 0.05, oak 0.01, reed 0.30, shrub 0.05 |
  | beach | palm 0.05 |

- **Forests**: about one tree every 7 m in temperate forest, 5.5 m in cold forest; one patch of four by four spots
  (16 m) in eight grows no tree (clearings, by the hash). Grassland and savanna stay sparse [0092].
- **Rivers stay open**: a tree whose crown would reach a river column (its rim and half way in, sixteen columns
  looked at) is not grown. Lakes and the sea may have crowns over their shores [0092].
- **Warmth**: a row's `warm` keeps it where the baked year's mean air is at least that (`World::year_air`); palms
  from 15 C, the bake's warm band (`t_warm`) [0092].
- **Checks in `parse`**: fork 0.1..0.9, limbs 1..6, bare 0..0.8, depth 0.5..4, warm -30..40 C. Fork, limbs and thick
  only on shapes that fork; depth only on flat and droop; bare only on a cone. A forked tree is at least 3 tall; a
  thick one has a radius of at least 2 [0092].
- **Seam and far forest** (also grid-and-planets.md, streaming-and-meshing.md): feet anywhere inside a face, a crown
  over a seam is the same blocks on both sides; a coarse tile's canopy is the ground: the column takes the top of the
  highest real tree at its middle (level 1) or four quarter points (level 2 and up), and that tree's top block as its
  three materials; a column whose middle is under water keeps its ground [0100, amends 0092's "far levels draw no
  plant blocks"].
- **Animals clear of trees**: see Herds below [0092, 0100].
- **Saves and hashes**: a save holds edits only and loads as before, but every unedited tree grows differently from
  before 0092; `BLOCKS.hash` changed with the new rows; no save or map version moved; the bake's `made_hash` did not
  [0092].
- **Cost** (ADR 0096: features first): a forest chunk's first generation about 2.6 times main's at 0092 (later
  brought down by building a tree's limbs and clumps once a column, `Plant::shape_of`, `column` [0102]) and a forest
  has 62 % more quads; the row is in `docs/perf-debt.md` [0092].

## Crowns: fluff, twigs, sway, light

Cutout leaf blocks (`see = "cutout"`) are drawn as cutout quads in two passes with a fluff card over each open face
[0090, 0099].

- **Two passes for cutout quads** (`gpu.rs` `cut_depth_pipeline`, `cut_lit_pipeline`): their depth alone first
  (`fs_cut_depth` discards empty texels), then the opaque terrain, then their G-buffer with `Equal` and no discard.
  Each pixel is shaded once with early depth [0090]. Plants have the same depth-first pass [0103].
- **Twigs from the wood** (mesher): block rows get `wood = true` (`log`, branches). `mesh::roots` walks out from wood
  through the cutout blocks up to three steps (a cutout block on an opaque non-wood block roots in it, below). Every
  cutout quad carries its way to the wood in free bits of its high word: bits 20-22 the side of the face the wood is
  on (1..4 u-, u+, v-, v+; 5 behind the face; 0 none), bits 23-24 the steps less one, bit 25 the face opens on air
  (`mesh::twig_bits`; joins, bits 24-27 since "Block ids past 255", stay lamps' only). Wood more than the padding away is unseen (a
  `shortcut:` in `roots`). Cutout faces never merge [0090].
- **The painting is turned** (`texel_a`): a cutout block's painting has its twigs rooted on its bottom edge; the
  shader turns it so that edge lies on the wood's side, or folds it about the middle when the wood is behind the
  face. A winter painting of four variants is picked by role (`winter_texel`): beside the wood v0 or v1 by hash, a
  step on v2 (a limb through), two on v3 (tips); a bare block with no way to wood within three steps draws nothing
  [0090].
- **Winter painting layout**: `<species> leaves@winter.png` is four variants with the wood on the bottom edge (row 15,
  or 31 at 32 px) of each: v0 one stem entering near the middle and fanning out, v1 the same off-centre, v2 a limb
  through from bottom to top at the middle with side twigs, v3 sparse short tips. Top and bottom faces follow the
  sides' rule; with the wood behind the face the painting folds about the middle, so its lower half must read on its
  own. Other seasons' leaf paintings are turned the same way; isotropic leaves need nothing [0090].
- **Fluff cards** (`terrain_vertex` mode 2 and `fluff_in`, `shader.wgsl`) [0099, reworks 0090 decisions 4 and 8]:
  - Each open face (bit 25) gets one card, 1.4-1.9 faces wide, lying a hand over the face (0.03-0.17 m out, never
    less than 0.03 m out at any corner), tilted 4-12 degrees. Its mask keeps the face (corners rounded) and, past the
    edges, lobes of leaves from a smooth field about 0.3 m across, up to the card's overhang (0.2 x `size` metres,
    0.2 to 0.48 m at the broadleaf's 1.7, by the clump field), in whole texels with a ragged rim.
  - The painting is the block's own at 32 px, shifted half a face (one shift for all cards, so lobes meet at shared
    edges). The lobe field is taken in the chunk's own cells as `terrain_texel`.
  - A bottom face's card stays at 0.3 of the push and overhang, a top face's at 0.5; `droop` bends sides only.
  - Every open face gets its card (`dense` still thins them); the clump field sets only how far it spills.
  - A card takes its face's normal and the face's shade (toward the wood, the clump's dents, a face down 0.6, a top
    face's plane 0.72). Winter crowns show the twig painting on the cards, turned by the root bits.
  - The hinged planes of 0090 are gone: bare twigs do not fan out of a crown as planes. `VOut.plane` is a `vec3f`
    (the card's point from the face's middle and its overhang).
  - Fluff is not traced; far levels stay one colour but draw the same lobed cards in the block's colour with the shade.
    `fluff=0` on a run leaves it out. Fluff is drawn only on faces that open on air [0103].
- **Fluff is data**: a cutout block's row takes `fluff = { size, push, droop, long, dense }` (`blocks::Fluff`;
  unnamed numbers are the broadleaf's 1.7, 0.35, 0, 1, 1; `size` 0 none) and `light`. It rides in the look row's
  word 16 (`Fluff::packed`, six bits a number; the row is 20 words, 80 bytes). `size`, `push` (a fraction of the
  overhang, 0.1-0.4 by clump), `droop`, `dense`, `light` are read; `long` is kept in the schema but no longer read
  (for later frond species) [0090, 0099]. Push and droop step by 0.025. `fluff` on a block that is not cutout is
  refused by name [0090].
  - The rows: spruce short, drooping and long; jungle large and drooping; acacia wide and sparser; shrub small; reed
    `size = 0` (a painted block of the same paintings, the reed bed being the plants' [0090, 0091]). Species to come
    (palm, bamboo, willow) take their rows the same way.
  - A smooth field of the place about 2.5 m across (`vnoise`) makes the crown bulge in lobes with dents; each cutout
    quad carries a shade in `ao`: darker toward its wood, under, and in a dent.
- **Sway** (`sway`, in `vs_cut` and `vs_fluff`): a smooth field of the place and the sky's clock (`g.water.x`);
  amplitude from the wind (`g.water.w`: 0.5 m/s plus the weather's wind, ADR 0076); frequencies are whole turns an
  hour so the clock's wrap at 3600 s does not jump. Zero in a shot, so the fixed views stand still [0090].
- **Leaves shade, not block, the sun**: fast leaves are the BLAS's second geometry; a shadow ray multiplies
  `ray_tint` by the face's `fluff.light` (broadleaf, jungle and shrub 0.5, spruce 0.35, acacia 0.65, reed 0.7) and
  goes on; below 2 % left it stops. About a quarter of the sun passes one broadleaf crown, a sixteenth two. Bare
  crowns take their twigs' texels (nearly no shade). Looking rays (bounce, reflections) take a leaf block whole.
  Non-fluff cutouts (`through` 0) stop a ray at once. A leaf's shade follows the Leaves setting and the level (fancy
  leaves keep their texel shadows, coarse levels stay whole blocks) [0090].
- **A bare crown is lit like bark**: a crown block where winter outweighs the season mix takes its winter painting
  alone (`bare_of`, `is_bare`); level 0's fast leaves are the BLAS's second geometry (`frame.rs` `shell`, chunk info
  bit 18); a ray takes them whole unless they are bare where they stand (`hit_season`, with the column's stagger)
  [0090].
- **A plant is lit as one mass**: a cutout block's direct light takes its normal bent half way toward up
  (`terrain_lit`, `normal + up * 0.5`; the G-buffer keeps the true normal), its shade has a floor of 0.7, and the
  top face's plane is 0.72 of it [0090]. The shade darkens every cutout quad's direct light.
- **32 px painting** [0090]: a painting 32 px high (32, 64, 96 or 128 wide: 1 to 4 variants of 32 x 32) is a fine
  painting; the file's height says it; 16-high files stay valid, so other blocks are unchanged.
  `engine::textures::decode_fine` keeps each fine variant as its four 16 x 16 quarters (top left, top right, bottom
  left, bottom right), so the texture buffer, its layers and `place` stay as they were; the face word's bit 14 marks
  it (`looks_of`); `paint_texel` reads it at 32 texels a metre (`face_xy` maps both grids). A fine face takes four
  layers a variant of the 2048. The 32 px paintings live in `data/textures/hd/` and are read in place of their 16 px
  namesakes (`textures::HD`); the watcher looks one folder down. Fine paintings are lit 2 x 2 painting texels a light
  texel [0108]; the inventory shows a fine face's first variant halved to 16 x 16 [0113].
- **Seasonal looks**: a season's painting is `data/textures/<block>@<season>.png` (spring, autumn, winter; the base is
  summer). Blossom is the spring painting. Fruit is named `@fruit` (`<block>@fruit.png`, late summer); it is not drawn
  yet [0090].
- **Studio** (also studio-and-art.md): a fine file opens 32 x 32 a variant and saves at its own size; `check=1` and
  `check_dir` take 32-high files of the allowed widths for any block; a new painting is 16 unless any file of the block
  is already fine; patterns and "Start from a picture" still make 16 px faces. The Studio's preview picks variants by
  hash only [0090]. Fluff has its own inspector section [0133].
- **Reach of the look**: painted, with its leaves, all of level 0, which a level 1 chunk (64 m) hands down within
  Detail x its edge (4 x 64 m at the default), so about 200 m [0090].
- **Cost** (ADR 0096): the crowns' fluff, with the denser forests, took fly p99 to about 9.0 ms, over the 8.3 budget;
  merged with its row in `docs/perf-debt.md`. Open faces only, indexed quads and the plants' depth pass won part of
  it back [0090, 0099, 0103].

## Small plants

(also rendering-and-lighting.md)

A plant is a clutter row of kind `plant` (`data/clutter.toml`), a few crossed painted planes [0091].

- **Placement**: `planes = 2..4`, `looks`; placed by the per-column roll of ADR 0082 with no storage, from a biome's
  `undergrowth` list in `data/biomes.toml` (voxel models stay in `clutter`). Plants roll on their own (`PLANT_ROLL`);
  three in four take their patch's roll (cells about 12 columns across, ragged), so a kind grows in drifts; four
  plants in five follow their patch's flower colour. Open grassland is at least 85 % covered at normal (tested). Dry
  scrub is about two thirds covered, tall grass and wild wheat in drifts, which the climate tint turns straw [0091].
- **Shape**: each plane leans `PLANT_LEAN` (0.26 rad, 15 degrees) to its own side by turns; tall grass has three
  planes. Each plant turns to any angle, stands up to a third of a block off its cell's middle, and has its own width
  and height, all from the column's hash in the shader (`still_local`, shared with the BLAS so its shadow matches)
  [0091].
- **Painting sizes** at 32 px a metre, a column's painting 32 px wide so neighbours merge: short grass 32x13, tall
  grass 32x32, wild wheat 32x40, fern 48x22, reeds 32x64, flowers 32x16, berry bush 40x40. A painting is at most
  64 x 80 px (2 m wide, 2.5 m high). Variants `.1`, `.2` and so on by the place's hash; the bush's `.bloom` (late
  spring) and `.fruit` (late summer). Files are in `data/plants/` (placeholders from `tools/paint_plants.py` until
  repainted) [0091, 0133].
- **Short grass**: three outlines 0.75 m high with a third more blades; the fourth (`.3`, one in four by hash) stays
  the 0.4 m young tuft [0099].
- **Shrubs** (four planes, the berry bush; `SHRUB` flag): three leaning crosses of three planes at fixed offsets and
  sizes (`plant_faces`), a lobed clump wider than tall, lopsided by the hash; seen twice as far as the grass (up to
  120 m); about one in forty plants in grassland; spring to autumn only [0091]. (The leaf-block shrub of the trees is
  a separate thing and takes the crowns' fluff.)
- **Vegetation setting** (Graphics: few / normal / many = 0.4 / 1.0 / 1.6 of the density; `vegetation=`), far
  distance 48 / 80 / 112 m. Each plant shrinks into the ground over the last fifth of its own reach (55 to 100 % of
  the far one, by its hash). A plant past its reach is not drawn [0091, 0103].
- **Far meadow ground**: the `meadow ground` painting (96 x 32, three 1 m tiles at 32 px) is face 0 (`MEADOW` flag).
  Beyond half the far distance the grass block's top fades to it (`ground_tint`, `meadow_texel`), a tile per column by
  hash, climate-tinted; the block's sides take it in a fringe over their top 0.15 to 0.45 m, reaching down the whole
  side between one and three times the far distance (all of a coarse chunk's). Past 150 m it fades to its mean
  colour (face 0's word 14), fully by 300 m [0091]. The `.far` plant paintings are not read.
- **Reeds**: four outlines (`reeds` to `.3`; green and dry tan stalks) and `reeds.back`, two tones darker, drawn where
  the four neighbours are reeds too (a neighbour counts if it has ground within `WADE`). A plant row may say
  `wades = true` (reeds): it stands in fresh water up to `WADE` (0.8 m) deep (`ground_under`); a wet column takes
  nothing else, and the server's pick-ups keep `ground`. Short and tall grass have a fourth outline too [0091].
- **Sway** in `vs_still`: `plant_sway(wind, height, column, hash)` from the weather's wind at the camera (ADR 0076) on
  the sky's clock; zero in a `shot=` [0091]. Reading `Globals.plants` (wind xyz, far w) for leaves is prepared, not
  done [0090].
- **Tint**: `climate_tint(heat, wet, part, season)` (WGSL) = `clutter::tint` (Rust, tested): heat from the bake's air
  at the column's height (-5 to 35 C), wetness from its rain. Deep green where wet, straw where dry and hot (the dry
  end squared, so a middling climate stays green); for plants also fresh in spring, yellowing from late summer, dull
  in winter. Only leafy pixels (g > r and g > b) take it, as a gentle multiply. A chunk's climate at its four
  footprint corners (`Chunk.clim`, `clutter::chunk_climate`) is mixed bilinearly in the shader. The grass block's top
  takes the same tint without the season. Each plant is shifted a little warmer or cooler, lighter or darker by its
  column's hash. A leaf texel multiplied by `climate_tint(cl.x, cl.y, year_part(sn), sn.y)` would match the ground
  [0091].
- **Depth**: darker at the root, lit at the tips; the planes of a cross a shade apart; each plane lit by how squarely
  it faces the sun (`|n . sun|`, 0.8 to 1.08) [0091].
- **Shadows**: the still quads (plants and voxel clutter models) join the BLAS's second, non-opaque geometry;
  `expand` places them with `still_local` (unswayed, unfaded). In `trace` a still candidate is confirmed only for
  shadow and lamp rays (first hit), more than 4 cm off the ray's start, within `still_far()` of the camera, and where
  its painting is opaque; bounce and reflection rays pass through plants. A plant's shadow stops where its own shrink
  begins (`still_reach`); a plant's own pixels skip still hits within 35 cm (`still_skip`). Sun, moon and lamp rays
  take them. `plant_shadows=0` leaves them out. Shadows do not sway [0091]. The BLAS's second geometry is
  `[cut as ray_cut says][glass][still]`: `cut: ray_cut + glass + cast` [0091].
- **Cost**: at the default setting fly GPU p50 about 3.2 ms and the meadow stand 4.5 ms; the frame thread about 0.15
  ms more; mesh workers about 47 us more a level-0 chunk. Row in `docs/perf-debt.md` [0091].
- **Clutter sizes** (`tools/paint_clutter.py`, `data/models`): twigs a fallen branch 0.88 m and 6 cm thick, a 0.63 m
  stick and a 0.38 m twig; pebbles stones of 0.16-0.34 m; the loose stone 0.38 x 0.19 x 0.31 m; the stick 0.81 m and
  6 cm thick. Test `clutter::tests::clutter_reads_from_standing_height` [0099].
- **Not done**: far plane cards from the `.far` paintings; a reed bed in the open at a lake's edge and reeds on a
  grassland river's bank (`near = "water"`); a meadow's edge fraying into tufts on a biome border; a covered grass
  block still shows the fringe far away [0091].

## Creature rows and the walk rig

(also studio-and-art.md for the Fauna page)

A creature is a row of `data/creatures.toml` plus a model. The walk is made from the row, never keyed in the model
file [0053].

- **The row** (`[[creature]]`, `engine::creatures`): `name`, `model`, `plan` (`biped`, `quadruped`, `bird`, `fish`,
  `hexapod`, `snake`), `scale`, `walk_speed`, `run_speed` (m/s), `step` (m a foot moves a step), part roles by part
  name (`legs` in a fixed order: biped left, right; quadruped front left, front right, back left, back right;
  hexapod front, middle, back, left then right; `arms` and `wings` none or two; `head`, `tail` or ""),
  `step_sound` and `voice` (rows of `data/sounds.toml` or ""), and the gait: `swing` (deg), `bob` (voxels), `sway`
  (deg, head), `wag` (deg, tail). `parse` refuses by row and reason (unknown model, a part the model lacks, wrong
  number of legs for the plan, a number outside `RANGES`, run slower than walk, an unknown sound). `write` is
  byte-exact on its own output; the Studio rewrites one row through `rows::save`, every other byte kept [0053].
- **The walk**: a looping clip of 16 linear keys a cycle (plus the first again). One cycle is two steps: length = 2 x
  step / speed. Legs turn about x at their pivots, sin-shaped: a biped's (and bird's) opposite; a quadruped's in
  diagonal pairs (front left with back right); a hexapod's as two tripods in opposite phase. Arms swing against the
  legs of their side at 0.8 of the swing; wings beat about z twice a cycle, the right wing a quarter cycle on (half a
  beat) so they flap mirrored. The swing grows with the square root of speed / walk speed (0.3 to 1.6 times the
  row's). The first root part dips by `bob` where the legs reach furthest, twice a cycle. Head sways and tail wags
  about y, a quarter cycle apart. Speed 0 gives `idle` (3 s: the body breathes, head and tail move a little). A
  missing optional part is skipped. Deterministic: same row, same clip [0053].
- **Snake**: no legs and no new key; a wave runs down the model's longest chain of parts (each the child of the one
  before), each joint a third of a cycle behind the last and never past `swing`; the chain's first part stays still;
  the row's head and tail do not sway on their own when they are in the chain [0053].
- **Other clips from the row**, `creatures::act` by name (`ACTS`): `turn` (the walk's legs at 0.4 of the swing, no
  dip), `eat` (the head down and nodding over 1.6 s, the body for one without a head), `hurt` (a 0.35 s flinch that
  ends at rest and does not loop) [0053].
- **Safety**: legs not matching the plan do not swing (the Studio walks the row being edited); a speed under 1 cm/s,
  NaN, or a step not over 0 gives the idle; a speed over 25 m/s walks at 25 [0053].
- **Footfalls**: `creatures::steps` gives the walk's footfalls as clip events playing `step_sound`, a quarter and
  three quarters into each cycle (a quadruped's diagonal pair lands as one). The voice is played only for live
  animals (below) [0053].
- **In the game**: `creature=name[:speed]` on any run gives that creature's model the clip `walk` and draws it at the
  row's `scale` about the cell's floor centre (its points and events too; a creature's scale is its cells' only, never
  the model in hand or a particle). `run=stand creature=boar model=boar:walk` stands it 3 m ahead walking in place
  (film it with `pitch=-25`). `World::air_over_ground` (walked up to air, then down onto the ground) is the one place
  `model=`, `creature=`, `fxplay=`, `structure=`, `run=fx` and the Studio's view take their cell from [0053].
- **`/spawn <creature>[:speed]`** in a running game shows that row's model walking in place three metres ahead and
  `/spawn` alone takes it away: the client's own preview, nothing sent to the server; one stands at a time. The command
  needs a space or the line's end after `spawn`. A saved row or model gives the shown creature its new walk within a
  second [0053].
- **Data**: drafts `data/models/boar.toml` (quadruped), `ape.toml` (biped), `snake.toml` with its row; no six-legged
  draft (the hexapod is tested on a stand-in model) [0053]. The figures are painted boxes (see studio-and-art.md,
  ADR 0116); every part keeps its name, parent, pivot and `at`, so the walk by part name is unchanged [0116, held by
  0053's rig].
- A change to `data/creatures.toml` alone reaches a running game within a second (see Live reload below) [0053, 0134].
- **Fauna page** (Studio): the rows in the files column; the viewport shows the model with the generated clip
  (written as `out/studio-view/models/creature.toml`) walking a circle of 2.5 m radius at the slider's real speed on a
  flat grass pad (`C` or the Circle chip: in place); a speed slider from 0 through walk to run (keys 0 1 2); the
  inspector has pick lists for model, plan and every part role, the numbers as sliders with units (Body, Gait), the
  sounds, and a variations wall of six gaits; chips show turn, eat and hurt; Save rewrites the row (checked by the
  game's reader, through `.tmp` and a rename); undo and redo; rows are drafts as `data/creatures.toml#<name>`;
  `check=1` reads the file through the game's reader. The view's `model` line takes a last word `@x,z,yaw` (metres
  from the stand cell, radians; `Models::show_place`) and `model name clip t x<scale>` [0053].

## Peaceful animals in the world

Server-authoritative; herds are a pure function of the seed per site until a player comes near, then simulated and
kept as a delta, and not ticked when nobody is near. Design: `docs/design/creatures.md` [0083].

- **`engine::herds`**: a site is 64 x 64 columns of a face. The biome at a hashed spot in the site picks a row of
  `data/herds.toml` (creature, biomes, share, size; the noise land counts as grassland). It makes 1 to 12 animals
  around the spot, never on water, and (generation and drift) only where the animal's whole box is clear
  (`stand_clear`, `phys::blocked`); a spot that is not clear tries the next. `stand` itself is unchanged: `move_on`'s
  climb-out and `/spawn <creature> live` still use it [0083, 0100].
- **Herds by biome** [0083]: grassland deer and hare; temperate forest deer and boar; cold forest deer and hare;
  savanna deer and boar; dry scrub hare and boar; tundra and alpine hare; rain forest and wetland boar. Shares 0.3,
  0.3, 0.25 a site. Deer (`data/models/deer.toml`) and hare (`hare.toml`) are quadruped drafts with voices
  `deer.call` and `hare.squeak` in `data/sounds.toml`; deer run at 7 m/s, hare at 6.
- **Waking**: a site wakes when a player is within about 96 m (25 sample points) and sleeps `SLEEP_TICKS` (200) ticks
  after the last one left. Every met site is kept with its animals' `phys::Player`, mind, heading and call count,
  plus the next id and the herds' own clock [0083].
- **Drift while away** (the developer's call): a sleeping site is not ticked. When it wakes, each animal is found where
  the time gone took it (`drift`): it has walked at an eighth of its walk speed for the time unsimulated (the clock's
  ticks gone, less the `SLEEP_TICKS` it ran on) towards a spot within `LEASH` (24 m) of home, hashed from the seed, its
  id and the clock. The spot must be land (`stand`, not water) of the home spot's biome; after four refused tries it
  stays. It wakes standing, its mind Stand. No cost while away, the same on replay; a site woken within `SLEEP_TICKS`
  does not drift [0083].
- **Body**: the player's box, gravity and collision (`phys::step`). An animal at rest is stepped once a second. One
  trapped by a placed block climbs out on top; `herds::move_on` climbs a block at a time (at most 16, each step taking
  its own up direction) until the box fits; one still inside after 16 stays and tries each tick [0083, 0092].
- **Mind**: every choice comes from a hash of the seed, the id and the herds' clock [0083].
  - Flees 4 to 6 s at `run_speed` from a player within 10 m who comes at it faster than 2.5 m/s.
  - Sleeps while the sun is under its horizon (no sleep pose: a sleeper plays `a.idle`).
  - Otherwise stands (3 to 8 s), grazes (4 to 10 s) or wanders at `walk_speed`, turning for home when more than 24 m
    from its herd's spot.
  - Every 4 ticks it looks ahead 1.5 m or more; a drop over 2.5 m, a rise over a block, or water over 0.7 m deep
    turns it 90 degrees (left, right or back, else it stands); a one-block step is jumped.
  - Calls about every 30 s.
- **The herds' clock**: minds and the look-ahead count the herds' own ticks, never the world's age (which
  `/timewarp` runs faster). It is saved with the herds [0083].
- **Cost cap**: at most `MAX_AWAKE` (600) animals are stepped a tick; over the cap each tick starts at another awake
  site. About 5 us an awake animal a tick on the server [0083].
- **Wire** [0083]: `ToClient::Kinds` (names) on join. `ToClient::Animals { seen, gone }` carries per client only the
  changes within 80 m: new to it, moved over 2 cm, or a change of act or call count (the call count carries the
  voice, ADR 0149). The client draws each animal lerped over one tick with the clips `a.idle`, `a.eat`, `a.walk`,
  `a.run` (made from its row for every creature model), one model instance per animal within 128 m, and plays the
  row's `voice` on a new call. Bots keep the same map; `report` gains `animals` (how many it sees and the nearest
  five with kind, distance and act). `ToClient::Defs` carries live definitions [0134].
- **Saves, hash, replay** [0083]:
  - The herds are written beside the save as `<save>.herds` (postcard of the next id, the clock, the kind names and
    the sites); the save's own layout (version 12 at the time) is untouched. A kind is matched back by name, and a
    file naming a kind no longer in `data/creatures.toml` is refused.
  - `Server::read_herds` reads the file where a save is loaded (the client and the dedicated server). Removing a
    world in the menu moves its `.herds` along.
  - The state hash includes the herds; a recording logs `Ev::Herds` after `Ev::Loaded`.
- **`/spawn <creature> live`** asks the server for a real animal three metres ahead, in the site it stands in; anyone
  may run it; it works on Terra only (`animals live on Terra`) [0083].
- **Live reload**: the server reads `data/creatures.toml` and `data/herds.toml` once a second; `Herds::reload` takes
  the speeds and shares and keeps the animals alive; a creature row may not be removed, renamed or moved [0134,
  amends 0083's "compiled in"].
- **Home world only**: herds belong to the body with `home = true` [0163].
- **Hot ground** [0205, 2026-11-25; bug reports 2026-11-24_18-45-16 and 18-46-31; the review's answers in]: a
  live volcano's hot ground is its crater and its bare upper cone: within 2 crater radii (`volcano::BARE`) of its
  middle, in metres on the ground by direction (`Body::hot_ground`), so the same on the conformal grid and over a
  face's seam.
  - **From generation 14** (`world::HOT_GROUND`, the overseer's call): the rule changes which animals the seed
    makes and where they walk, which is hashed state, so a world of an earlier generation keeps its herds as they
    were, to the bit, and its recordings replay. The developer's world of 2026-11-24 is such a world: its hares
    stay on the cone. (`GENERATION` reaches 14 with the branch that brings it; until then no new world has the rule.)
  - No animal is made there, drifts there or is put there by `/spawn <creature> live` (`herds::hot` refuses it,
    and lava anywhere; the command says "no ground for an animal there (lava, or a live volcano's hot ground)").
    The look-ahead turns an animal from it and from lava as from deep water.
  - **An animal on hot ground is no survivor** (the overseer's call): it flees out from the crater at its run
    speed, before any other thought, heading straight out again every 2 s; the look-ahead turns it round walls
    in between and it takes any drop down the cone. It has the time its run to the edge takes thrice over, and
    2 s (a scared one that ends on it: what is left of its scare). Still on the hot ground then, as in a crater's
    bowl, where out is up a wall, it dies: removed from its herd, in the hashed state. Its mind is Flee all the
    while (boxed in, it does not stand). Off the hot ground it runs on 2 s. One whose body is in lava is removed
    (looked at once a second).
  - Rain and snow (the client's `weather::Fall::drops`, `World::boils_rain`; drawn only, every generation): no
    drop is made over a live crater's lake, however high, nor where it would land in lava poured anywhere: it
    boils off in the hot air over it. The lake by the lava that is there now at its surface's level in the drop's
    column, so a part built over or drained has rain again.
  Alternatives rejected: the rock's own heat at the surface as the rule (over 100 C five crater radii out on the
  small planet, where trees stand and the air is cool: the heat field does not take the surface as a boundary yet).
  Open: both follow the heat field once it does, as `BARE` itself. Plants' bare cone (`Body::bare_cone`) is still
  measured in columns of the crater's face, on that face only: left, since changing it moves plants in worlds made.

## Open

- Kinds per biome, shares, herd sizes, deer and hare looks, flee distance 10 m and speed 2.5 m/s, the drift's pace
  (an eighth of walk speed) and that it runs across nights: still the developer's to confirm [0083].
- Hostile creatures; combat and hurting an animal, drops, breeding, taming; birds, fish and animals on Luna;
  footfall sounds on live animals; a collision box per kind; a herd's minds living on while nobody sees it; culling
  met sites the seed makes again empty (`shortcut:` in `Herds::to_bytes`); pens (a path or "enclosed" rule for the
  drift) [0083].
- Fluff past about 64 m (cheaper by 1.1 to 1.4 ms at 1440p, turns far crowns back toward cubes) is a taste call
  [0103, 0099].
- Small and medium round crowns are nearly one ball; palms from 15 C instead of 18 to 20 C; the willow's withes; the
  species per biome and the denser forests: calls to confirm [0092].
- Own logs for the first four species, leaning trunks, palms only on the rain forest's edge, giant trees,
  conifers as ragged tiers, palms with arching fronds, cactus arms, painted pine/palm/willow leaves [0092, 0090].
- Fruit (`@fruit`) painting is not drawn; no fifth look row [0090].
- The shrub of the plants (berry bush) and the leaf-block shrubs of the trees read as one kind of thing: waits to be
  judged [0091].
- Shadows of plants do not sway [0091].

## No longer holds

- 0052's "small plants as models" (`voxel::plant`, the Models page's "Plant..."): removed with the voxel plant
  generator; plants are crossed painted planes (0091, removal in 0133).
- 0052's face margin and seam band of 6 columns for plants: replaced by 0100 (feet anywhere in a face, neighbours'
  spots read across seams).
- 0053's "no spawning, no AI, nothing on the server": replaced by 0083.
- 0053's "voice is not played": played for live animals by 0083.
- 0053's `creature=` rows read only on a run with `creature=`: rows also reload live (0134).
- 0083's "the server takes `data/creatures.toml` and `data/herds.toml` compiled in": replaced by 0134.
- 0090's hinged fluff planes and the 1.7-times larger tilted planes (decision 4) and the fluff plane shape in
  decision 8: replaced by 0099's cards over the faces.
- 0090's top planes standing 45-60 degrees and a third as far out: replaced by 0099 (a top face's card at 0.5 of the
  push and overhang).
- 0092's "far levels draw no plant blocks (only `Tile::trees`)": replaced by 0100's canopy as ground.
- 0091's voxel grass tuft, fern and flower models: replaced by painted planes of the same ADR.
- 0091's short grass 0.4 m in all four outlines: replaced by 0099.
