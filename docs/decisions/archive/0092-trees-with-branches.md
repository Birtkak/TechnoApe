# 0092 – Trees with branches, tiers, sizes and more species

- **Status:** accepted
- **Date:** 2026-11-13

## Context
Part C of the flora direction (ADR 0089, `docs/design/flora.md`). Trees were a trunk under a cube crown in four
species. The developer wants:
- branches showing inside and under the crowns;
- layered conifers;
- three sizes per species;
- six more species, with Vintage Story as the reference.

A later call: in winter a crown must read as twigs "coming from a log". So every leaf needs wood near it, and
the wood must be one skeleton.

The overseer's reviews of the shots asked for:
- broad domes, not balls on poles: an oak's crown radius 4 to 6, wider than tall, limbs leaving from a third of
  the trunk;
- willow strands one block wide;
- forests whose crowns meet;
- low, wide bushes;
- branches joined face to face.

Trees stay computed from the seed (ADR 0043), never stored. The crowns' look (part A), small plants (part B) and
paintings (part D) belong to other sessions.

## Options considered
1. **Branches as their own block kind** (a thin limb model).
   - Pros: slimmer limbs.
   - Cons: a new kind for the mesher, physics and rays, and part A draws the twigs anyway.
2. **Branches as the species' log blocks inside and under the crown, all from `Plant::block`.**
   - Pros: every reader (mesher, physics, water, rays) already takes plants through that one function (ADR 0043).
   - Cons: limbs are a block thick.
3. **Keep the four shapes and only add species.**
   - Cons: no branches, which the developer asked for.

## Decision
Option 2.

- **One more input:** `Plant::block(h, v, dx, dz, z)` takes the tree's own hash `v` (`world::Foot::v`, from the
  spot's hash), so trees of one species and height differ. It stays a pure function of seed and place, inside the
  plant's radius and height.
- **`REACH` goes from 3 to 6.** An oak's crown is 13 blocks across. `PLANT_TOP` rises with it, and the face margin
  and `clutter::ground`'s seam band widen to 6 columns.
- **Sizes:** where a tree's height falls in its row's range makes it small, medium or large (`Plant::size`). Its
  crown is 2, 1 or 0 blocks narrower than the row's radius, and at least 1.
- **Shapes** (`data/biomes.toml` documents the keys):
  - `round` (the developer's target picture, `out/flora-refs/6-leaf-block-target.png`):
    - The crown is clumps of leaves, not one ball: a clump at the top and one at each branch's end, each 2 or
      2.5 blocks across each way and 0.85 of that up and down, each its own length out and a block or two under
      the top by the hash. So the crown is lobed and uneven with sky between its clumps.
    - The trunk leans a block in two trees of three, up to the top clump. A large `thick` tree stands straight,
      on 2 x 2 below its fork.
    - At `fork` of its height, `limbs` limbs spread out under the crown to half its radius. On a crown of 3 or
      more, each limb parts into two branches.
    - On a crown of 4 or more, a clump's outer leaves fall by the hash, but only where the cell a step towards
      its middle is core, so no leaf hangs alone.
  - `flat` (acacia): a lens of leaves `depth` thick each way, at most 0.7 of its radius, over limbs that
    spread to two thirds of its radius and part into three branches each.
  - `droop` (willow): an umbrella whose rim sags.
    - A wooden withe hangs from each limb's middle branch.
    - Strands of leaves, one block wide, hang on every other column each way of the rim beside a withe (a
      block's gap between strands from any side), most of them to one or two blocks over the ground and one in
      four half way.
  - `cone`: a layered conifer.
    - Bare under `bare` of its height, then tiers from the top down, wider as they go, with a gap layer under
      each.
    - A tier's lower layer carries limbs along three or four axes, and leaves clump within a block of a limb or
      the trunk.
  - `palm`: four fronds along the axes that bow down at their tips, and short ones between them in most quarters.
  - `column` with leaves and a radius (bamboo): a clump of stalks of different heights, with leaves at their tops.
  - `bush`: one layer, low, wide and lopsided: a clump of the middle, most of its four sides and a corner only
    beside a side (one corner always open, so never a square), and on one side by the hash a second, smaller
    clump two out, so a bush is two to four blocks across.
- **Limbs** are segments. A cell is wood where a segment passes through its cube (a supercover), so a limb steps
  out and then up, face to face, and a bare tree is one skeleton. Their directions come from a table of sixteen,
  not `sin` and `cos`: a Linux server and a Windows client must make the same blocks, and the platforms' maths
  libraries may round the last bit apart. Everything else is `+ - * /`, `round` and `powi(2)`.
- **Rivers stay open:** a tree whose crown would reach a river column (its rim and half way in, sixteen
  columns looked at) is not grown, so a river can be waded and swum along. Lakes and the sea may have crowns
  over their shores.
- **Warmth:** a row's `warm` keeps it where the baked year's mean air is at least that (`World::year_air`). So
  palms grow on warm beaches and in the rain forest, not on cold shores.
- **Species:** each new one has its own log and leaves (ids 206 to 217, after main's rubble and halo rows). The leaves are `see = "cutout"` with the
  `leaves` pattern; the logs are plain bark, because rings on a log's sides read as a 2 x 2 post.
  `out/flora-tree-blocks.txt` names them for painting.

  | Plant | Shape | Height | Radius | Keys |
  |---|---|---|---|---|
  | spruce | cone | 7–14 | 3 | bare 0.15 |
  | pine | cone | 8–16 | 2 | bare 0.55 |
  | broadleaf | round | 5–9 | 4 | fork 0.4, limbs 3 |
  | oak | round | 5–10 | 6 | fork 0.35, limbs 4, thick |
  | birch | round | 7–13 | 4 | fork 0.45, limbs 3 |
  | jungle tree | round | 10–16 | 5 | fork 0.65, limbs 4, thick |
  | acacia | flat | 5–8 | 5 | fork 0.55, limbs 2, depth 0.8 |
  | willow | droop | 6–10 | 5 | fork 0.55, limbs 4, depth 1.5, thick |
  | palm | palm | 5–10 | 3 | warm 15 C |
  | bamboo | column | 6–12 | 2 | |
  | shrub | bush | 1–2 | 2 | |

  Forests are denser, so crowns meet (about one tree every 7 m in temperate forest, 5.5 m in cold forest),
  with clearings: one patch of four by four spots (16 m) in eight grows no tree, by the hash, so sun reaches
  the floor. Grassland and savanna stay sparse.

  | Biome | Plants (share of candidate spots, one each 4 x 4 columns) |
  |---|---|
  | cold forest | spruce 0.32, pine 0.14, birch 0.06, shrub 0.05 |
  | temperate forest | broadleaf 0.18, oak 0.12, birch 0.05, willow 0.01, shrub 0.10 |
  | grassland | broadleaf 0.008, oak 0.007, shrub 0.04 |
  | savanna | acacia 0.04, shrub 0.03 |
  | rain forest | jungle tree 0.34, bamboo 0.07, palm 0.03, shrub 0.20 |
  | wetland | willow 0.05, oak 0.01, reed 0.30, shrub 0.05 |
  | beach | palm 0.05 |

  Spruce, broadleaf, jungle tree and acacia keep the shared `log`.
- **Checks in `parse`:**
  - Ranges: fork 0.1..0.9, limbs 1..6, bare 0..0.8, depth 0.5..4, warm -30..40 C.
  - Fork, limbs and thick only on shapes that fork, depth only on flat and droop (round crowns size their
    clumps themselves), and bare only on a cone.
  - A forked tree is at least 3 tall, and a thick one has a radius of at least 2.
  - A tree's height range spans at least three values (one per size).

## Evidence
**Tests,** each seen red first (`biomes.rs`, `world.rs`):
- `plants_keep_to_their_reach_and_stand_on_a_trunk`: every species, every height and 256 hashes give the same
  tree each time, inside its footprint, of its own log and leaves, on its foot; trees of one height differ by
  the hash. Red: every tree of a height was the same.
- `every_leaf_touches_a_leaf_or_a_branch`. Red: the old ragged jungle crown left a lone leaf.
- `every_leaf_of_a_bare_crown_is_near_its_wood`: in broadleaf, oak, birch and willow, every leaf is within two
  blocks of wood on each axis. Red: the willow's strands hung far from wood until the withes; the dome's top until
  the leader reached two under it.
- `the_wood_is_one_skeleton`: every block of wood of every species is joined face to face to wood on the ground.
  Red: limbs drawn within half a block of their line met only at corners.
- `crowns_are_wide_for_their_trunks`:
  - a trunk is 1 block, or 2 x 2 only on a large tree;
  - the crown is at least three times the trunk's width, and 7 or more on a large tree;
  - two thirds of its leaves have crown over or under them;
  - a round crown is no taller than it is wide.

  Red: the first thin crowns; then a small broadleaf 6 tall and 5 wide at full depth.
- `trees_branch_in_tiers_and_sizes`:
  - most large trees branch off the trunk, and small crowns are narrower than large ones;
  - conifers grow in tiers with at least two gaps, bare below;
  - a willow hangs below half its height;
  - palm fronds grow only at the top, bamboo in a clump, and a bush is one layer, never the full 3 x 3, and
    in at least an eighth of hashes four across.

  Red: no branches; then the pillar shrub; then a square bush (a 3 x 3 of nine).
- `the_species_grow_in_their_biomes`: the table above, own blocks, palms only where warm.
- `every_species_grows_in_its_biomes` on a baked planet: each species of a biome common enough is found there,
  and no palm grows under its `warm`. Red: a palm at 15.8 C, under the 18 C first tried.
- `forests_have_clearings`: one patch of four by four spots in 4 to 12 of temperate forest has no tree. Red:
  none in 731 patches.
- The existing `a_walker_follows_a_river_from_spring_to_sea_in_water` went red with the clumped oaks (a
  bank's oak hung its leaves into the channel at point 566 of 893) and is green again with rivers kept open.
- `the_limb_table_is_a_circle`.
- `bad_rows_are_refused` with the new keys and checks. Red: a round tree 1 tall, a thick one of radius 1.

**Cost** (`tree_chunk_cost`, an ignored test):
- Setup: play profile, one thread, the test planet's 621 chunks over every candidate spot of a forest biome. One
  measurement in a measure slot (ADR 0096) on commit d05e7700, the median of three runs (1326, 1408 and 1850 µs
  cold).
- "Cold" builds a chunk with its tiles' plant lists uncached; "warm" with them cached.

| Per forest chunk | main | this branch | change |
|---|---|---|---|
| generation, cold | 509–585 µs | 1408 µs | about 2.6 times |
| generation, warm | 131–172 µs | 141 µs | none |
| mesh (fast leaves) | 89–137 µs | 130 µs | about the same |
| quads, fast leaves | 1920 | 3112 | +62 % |
| quads, fancy leaves | 2248 | 6435 | 2.9 times |

**Shots:** `out/flora/<species>-<size>.png`, plus `forest-temperate`, `forest-cold`, `forest-rain` and
`forest-above`.
- Setup: seed 1337 on the 4 km planet, at noon. Each still is taken from the sun's side, 10 to 14 m out at eye
  height. `tree_spots` picks the tree nearest the spawn with a clear line to it, and each forest well inside its
  biome (30 m round all that biome).
- The magenta sky is the shot mode's clear colour, kept for its crack count.
- Read against the developer's references 1, 2, 5 and 6 (`out/flora/beside-ref6.png` puts oak-large,
  broadleaf-medium and forest-temperate beside reference 6):
  - **Oak and broadleaf:** lobed crowns of clumps on limbs spreading from low on the trunk, wider than tall at
    every size. They are still denser and blockier than reference 6, with less sky between clumps (the small
    and medium sizes most; see the review). The soft leaf block is the crowns session's.
  - **Willow:** separate one-block strands of uneven length with the trunk showing.
  - **Temperate forest at noon:** lit ground with sun patches under a canopy with gaps. Reference 1's walk under
    trees, after the clearings; it was black before them. From 40 m up, the canopy is broken by clearings.
  - **Cold and rain forest:** closed and dark-floored but readable. Their stills stand close among trunks.
  - **Conifers, palms and bamboo:** unchanged in shape this round (see Later). Several of their stills stand
    in a crowd of other trees, so the scout's clear line is only to the tree's middle.

## Review (a fresh reviewer of the engine and data diff)
Found and fixed:
1. The limb ends used `f32` `sin` and `cos`. This was fixed before the review with the table above, and the
   reviewer checked the table.
2. The tests saw 24 hashes, under 2 % of an oak's limb layouts. Every rule now uses 256.
3. A height range under three values could never make a large tree. It is refused now.
4. `warm` was unchecked (`nan` grew palms everywhere). It is held to -30..40 C now.
5. Keys that do nothing on a shape (`fork` on a cone) were taken silently. They are refused now.
6. Second review: a forked row 1 or 2 tall would panic in `clamp(1, h - 1)` on a worker thread. It is refused
   now.
7. Second review: `thick` on a radius of 1 clipped the 2 x 2 trunk to three blocks. It is refused now.
8. Third review (the clumps): a short tree's clumps reached the ground beside its foot. A clump's lowest
   leaf is now two over the ground.
9. Third review: facing branches of neighbouring limbs shared a clump size. Now each has its own.

Answered:
- **`crowned` rebuilds a tree's limb segments for every cell,** and REACH 6 means each column looks at the trees
  of a 13 x 13 window. That is the cold generation cost above. Under ADR 0096 (features first) it is a row in
  `docs/perf-debt.md`, not optimised here. The idea: build each tree's segments once per tile, and reject cells
  below the fork that are not trunk.
- **The tests sample 256 hashes rather than every limb layout** (16 turns x 3^limbs jitters). The skin leaves
  are safe by construction. The branch tips and the willow's strands are sampled, not proven.
- **A palm at exactly 15.0 C is decided by the last bit of `year_air`.** That is the same risk the biome choice
  already carries, so nothing new.
- **Small and medium round crowns (radius 2 and 3) are close to one ball:** their clumps overlap heavily and
  keep every outer leaf. Only crowns of 4 or more get gaps and ragged lobes. A taste call for the developer.
- **15 C is low for palms** (18 to 20 C is usual for coconuts). 15 C is the bake's own warm band (`t_warm`,
  where savanna and rain forest start), so it is a call for the developer below.

Soak (after the merge with main): the server's soak failed with "animal inside the ground". An animal found
inside a block was set on the ground under it, but under an oak's low limb its body still overlapped wood.
`herds::move_on` now climbs it a block at a time (at most 16) until it fits. Red: the soak at seed 1; green
with the climb. A rule that a shrub grows only on level ground was tried and dropped: it left grassland
with no shrubs at all (`every_species_grows_in_its_biomes`), and the climb covers the animal case.
A fresh reviewer of the climb: the up direction was taken before the move to the feet (now each step takes
its own). Answered: an animal still inside after 16 blocks stays put and tries again each tick (no harm, no
log); a whole-block step can lift a small animal onto a limb, where gravity takes it on the next tick.

## What changes for existing saves and checks
- **Saves:** they hold edits only, so a save loads as before. But every unedited tree grows differently: new
  shapes, heights, shares and species. An edit (a dug or placed block) stays where it was, so a cut made in an old
  tree can float beside a new one. No save or map version moves.
- **Hashes:** `BLOCKS.hash` changes with the new block rows, so a client and a server must be of this build (as
  with any new block). The bake's `made_hash` (caves and ores) does not change.
- **Fixed views:** every view with trees changes.
  - The views near the spawn's forest change: `seam`, `corner`, `river`, `lake` and the four `season-*`.
  - `lod` and `high` change a little.

  They are to be accepted on main after the merge. No views were run here.

## Consequences
- **Later:**
  - giant trees (the developer: not this round);
  - own logs for the first four species;
  - leaning trunks;
  - palms only on the rain forest's edge;
  - paintings of the new blocks (part D);
  - the developer's wider call, not done in this round: conifers as ragged tiers of uneven reach, palms with
    arching fronds, cactus arms, reed beds, each lopsided by the hash;
  - far levels draw no plant blocks (only `Tile::trees` for distant trees), so distant shrubs need the
    distance work's own stand-in.
- **CPU cost:** a forest chunk's first generation is about 2.6 times main's, and a forest has 62 % more quads
  (2.9 times with fancy leaves). The row in `docs/perf-debt.md` holds this until an optimisation batch.

## Calls for the developer to confirm
- The species per biome and their shares (the table), and the denser forests.
- Palms from 15 C mean air (the bake's warm band) rather than 18 to 20 C.
- The willow's wooden withes inside its strands (they carry the winter twigs).
- Sizes by height: a small tree's crown is two blocks narrower than a large one's.
- Small round crowns nearly one ball (above).
