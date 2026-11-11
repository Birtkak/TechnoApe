# 0033 – Blocks of 1 m

- **Status:** accepted
- **Date:** 2026-11-11

## Context
ADR 0001 set the block at 0.5 m. The player is 0.6 m wide and 1.8 m tall, so on that grid a doorway was
2 x 4 cells and a tunnel 8 blocks a metre; a belt one cell wide was a knee-high strip and every machine a
thing of many cells. The developer, 2026-11-11: "if we would go ahead and make a door, it would have to sit
on 4 cube grids to be able to fit a player and mineshafts too... make the blocks 1 m", then: "1m blocks,
retrofit everything to this new vision, after we will add the 12.5cm fine cells for gasses and finer
simulation, in world crafting, chisseling and building intricate things".

## Options considered
1. **1 m blocks**, the planets the same size in metres – the player is one block wide and two tall; 8x
   fewer cells in any volume; walking, jumping and reach keep their numbers. Coarser hills.
2. **A player of 0.5 x 1 m on 0.5 m blocks** – the same picture, but the cell count stays, the planet
   feels twice as big and every number of movement is rescaled.
3. **0.5 m blocks and a player under 0.5 m wide** – tunnels of 1 x 4 cells, nothing else changes; no gain
   in speed, belts and machines stay awkward.

## Decision
- **Option 1: `BLOCK` is 1 m.** The planets keep their size in metres: the columns per face edge halve
  (Terra 6144, the huge planet 12288, the benchmark's small one 768, Luna 256).
- **The fine cells stay 12.5 cm**, now eight to a block's edge (512 to a block, four before). They come
  after this, for gases and finer simulation, crafting in the world, chiselling and intricate building
  (`vision.md`). ADR 0003's grid nests them the same way, at 8N.
- **The look stays 16 texels to a block**, so a texel is 6.25 cm and a fine cell is two texels wide.
  *A default, not the developer's word:* 32 texels would keep the old texel size.
- **Old saves are refused** (save version 4): their cells mean other places now. The code that carried
  versions 1 and 2 over is deleted.
- The water's constants take the block's size from `BLOCK` (they had 0.5 written in).

## Measured (dev PC, release build, 2026-11-11, other sessions compiling; a comparison, not a baseline)
| `run=stand quick=1` | 0.5 m (main) | 1 m |
|---|---|---|
| planet radius | 4245 m | 4245 m |
| quads drawn | 401,205 | 321,282 |
| frame p50 | 1.92 ms | 1.92 ms |
| VRAM | 811 MB | 813 MB |

- 116 tests pass; a 1 minute soak of 50 bots is clean; a scripted play run digs, places and walks with no
  correction; the six views show no cracks (every picture changed, as it must).
- The engine was already written in blocks: besides the constant, two tests' numbers, the water's two
  constants and the lamp's shadow margin had the half metre written in.

## Consequences
- **The view reaches twice as far for the same cost**, because the streamer counts in chunks of 32 blocks.
  The gain in speed is there to take by lowering "Detail distance"; its default is a tuning call after
  this, with the benchmark.
- Everything simulated by volume (water, later air, belts, machines) has 8x fewer cells per cubic metre.
- A door is 1 x 2 blocks, a tunnel 2 blocks a metre, a belt or a machine one block wide.
- Coarser ground: hills step in metres. Half blocks (slabs, stairs) wait for the fine cells.
- **A ledge one block high needs a jump** (the developer, 2026-11-11: "require jump"). The step-up, which
  climbed one block and so half a metre before, is removed; a jump reaches 1.2 m. Half-block stairs come
  with the fine cells, and a step-up for those with them. **"Auto jump"** in the settings (Controls and
  Accessibility, off by default; the developer: "nice for you to use in tests, or players who want")
  jumps a one-block ledge in the way of a walk when there is room on top: `auto_jump=on` in a
  `settings=` file for scripted runs.
- The reference views are accepted anew with this change; the speed baseline (`tests/perf/baseline.json`)
  and `docs/benchmarks.md` are measured anew on a quiet machine after it.
- The baked land (worldgen W1, W2) is sampled per column: its maps for Terra are made for 6144 columns.
- A server and a client must be of the same build, as always: the home server is rebuilt, and its old
  `saves/server.sav` moved aside (it stops with the reason rather than overwrite it).
- **The review found, and this change fixed:** the top block was drawn as grass but counted as dirt in
  about half the columns (the band's depth had 0.5 written in); a click on an old world in the list
  closed the game (it now says "an older world, cannot be played" and starts nothing); a shape could be
  dragged to 32 m, past the 16 m reach, and vanish when the server refused it (the longest side is 16
  blocks now, the default height 4); a recording that starts from an older save replayed as nonsense;
  the dam-break test still counted half metres; a bot test dug the same block twice by half the seeds; a
  start point's clearance was sized for half-metre steps.
- **Tuning left for after, each a look or a measurement:** the sea's top sits up to a block from its
  nominal depth (0.4 m over on Terra, so the lowest shore is wet); the water's small thresholds are in
  block units and so twice as large in metres (3 cm, 12 cm/s) and a slow fill now takes up to three
  minutes to lie flat; a lamp's 8 m is 8 blocks of light, where it was 16; the soak's bots still dig
  2 x 2 x 2 groups, now 2 m.
