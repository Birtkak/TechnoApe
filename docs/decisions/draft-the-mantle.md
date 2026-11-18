# NNNN – The mantle: a new world's grid goes down to R/√2

- **Status:** proposed (step 1 of ADR 0152)
- **Date:** 2026-11-18

## Context
ADR 0152 carries today's grid on down through the mantle to R/√2, about 1245 m on the 4 km planet, where it stopped
192 m down (ADR 0063). Saves made before must keep their floor and every layer index they hold. Below the new
floor nothing is dug until step 2 (the liquid core).

## Options considered
1. **Negative layers under today's layer 0.** Old saves keep their indices for free, but every reader that treats
   layer 0 as the floor (the edit shape, the water's columns, piles, the far summary, the streamer's bands)
   would need a second bound. Many more places to get wrong.
2. **Layer 0 moves down, every layer above gains the mantle's count in index** (as ADR 0063 did for the 192 m
   crust), per world: a save from before builds its planet without it. Every reader already asks
   `0..layers`; none has to change.

## Decision
Option 2.

- **Per world, recorded.** `Body::with_mantle` (before the map) moves `r0` down by `mantle_layers(n)` layers and
  adds them to `layers`; `Body::mantle` holds the count. `World::with_grid(.., mantle)` gives it to the planet
  only: Luna keeps her depth (her zones are a later step), and a planet for which `deep_layers` is 0 (the 500 m
  class) gets `mantle_layers` 0. A new world (`World::baked`, `Server::baked_warp`) has it; `mantle=0` on the game,
  the server and `report=1` makes one without (an escape hatch, and main's grid for comparisons).
- **On disk and the wire.** The save is version 20 with a last field `mantle: bool`; a 19 or older reads as
  `false` (`Save::from_bytes`), and `Server::from_save` builds the world as the save says, so an old world
  written again stays on its 192 m floor. `ToClient::Welcome` carries `mantle`, so a client or bot builds the
  same grid. The conformal grid's trial (ADR 0131) never reached the save; this is the first grid choice kept
  in it. A recording from the seed replays on the deep grid (`Ev::Start` does not say; shortcut noted in
  `replay.rs`).
- **Whole steps of 256 layers.** The mantle is the multiple of 256 layers nearest R/√2 (`MANTLE_STEP`): the
  coarse levels' chunks and cells (2 to 256 layers) then stand on the same layers above the crust's floor as on
  an old world, and the surface is meshed and drawn as before (with the layer nearest R/√2, 1274 on the 4 km
  planet, every view differed: far chunks sampled other layers). The 4 km planet: 1280 mantle layers under the
  595 it had (1875 in all); layer 0 at 2997.8 m, 4.2 m under R/√2, 1248 m under the radius; the lowest block
  0.706 m. The 8 km planet: 2816 under 601 (3417), 2534 m deep, 47 m under R/√2. The 1536 test planet: 256,
  378 m deep (67 m under R/√2, the lowest block 0.64 m). Both large planets still fit one root band of the
  streamer (2048 and 4096 layers).
- **The GPU counts from the old floor.** The client gives the GPU a body's `r0` at layer `mantle` and every chunk's
  base `mantle` layers lower (`frame.rs`), so every shader's look hash (a texture's variant and tone, a grass
  fringe, a crown's fluff and clumps, a reflection's texel) and the light's random seeds (`tx_seed`) read the
  numbers an old world gives them, and the f32 positions are computed from the same values; no shader changed.
  The two CPU hashes of a layer that pick a roll take the mantle off too: a cell's held item (`edit::held`,
  flint in gravel) and a good hit's drop count (`crack::drop`), as the ore halo does.
- **Mantle rock.** `data/rocks.toml` names `mantle = "peridotite"`. `Body::rock` (the one rule the mesher's fill
  and `generated_under` ask) gives it under R − 192 m in every province on a body with a mantle, and today's
  layers above. Peridotite (block 58, rubble 191) is as hard as quartzite (10, grade 1), the hardest rock of
  the crust: a steam-age pick digs it. Its texture is a draft (`tools/paint_textures.py`, `data/drafts.txt`).
- **Nothing new is placed in the mantle.** Caves, ores, water, piles and plants keep their depths under the
  ground: their placement works in radii, and the one hash that read a layer index (an ore halo's fraying)
  takes the mantle off it, so a deep world's halos are main's.
- **Heat** carries on by the province's rule (air plus `heat` C a metre): at the floor, about 1245 m down, the shield
  is about 150 C over its air, the basin 186, the folded belt 224, the ocean floor 249, the volcanic arc and the
  rift 435; under the spawn the report says 200.4 C.
- **Gravity inside a body** is g_surface · r / R (a ball of even density), the square law outside as before;
  this holds on every world, old ones too (under the radius it was up to 1.25 g before: a sea floor or a valley
  under the radius now pulls a few thousandths less).

## Consequences
- What scales with the layer count: `Solid::gaps` looks up every chunk band of a column for edits
  (`world/solid.rs:42`: 59 bands where there were 19); `Streamer::new` makes roots per 2048-layer band
  (`stream.rs:419`), still one band on the 4 km and 8 km planets; the horizon occluder is the sphere of `r0`
  (`stream.rs:379`), now R/√2 where it was R − 192 m, so less is culled behind the planet.
- The far side's culling and the mantle's cost: `docs/perf-debt.md`.
- Step 2 puts the liquid outer core under `r0`; until then a dig under layer 0 is refused as before.
- Open for the developer (plain defaults taken): peridotite's hardness and grade (quartzite's, diggable now;
  `iron-nickel`'s 20 and grade 3 would wall the mantle off until a later age); its look; the line at exactly
  R − 192 m (no fold, no mixing with the crust's last layer); gravity's new law on old worlds too.

## Checks
| Check | Result |
|---|---|
| Tests | `the_mantle_carries_the_grid_down_to_r_over_root_2` (red on a stub: floor at 4052 m); `the_readers_agree_from_the_surface_to_the_mantles_floor`, mesher fill, `generated_under`, `generated_solid_under` and the water's gaps cell by cell in a column of a 1536 planet from over the ground to layer 0, peridotite only under the crust (red with the mantle off: no cells under the crust); `gravity_falls_evenly_inside_the_planet` (red: 1.23 g at R/2); `a_player_digs_at_the_mantles_floor_and_not_under_it`, on the server a dig at layer 2, 310 m down, is sent back, one at layer −1 refused, the save keeps the mantle, a save without it loads on today's floor with its cells at the same indices (red with the mantle off); `a_19_loads_without_the_mantle_and_this_version_keeps_it` (written after the code, not seen red). The old gravity test now asserts the new law |
| Report, seed 1337, 4 km | every province, cave and mineral count equal to main's (copper 339 035 blocks, iron 808 378, coal 888 247, ...). Two lines differ: `made_hash` (caves' layer coordinates gain 1274; with `mantle=0` it is main's 2a79455bb6161781) and the heat under the spawn, 13.6 C at the ground and 200.4 C at the new floor (42.8 at the old) |
| Cost | `docs/perf-debt.md`, row "The mantle": fly's walk 0.1 to 1.0 ms, quads +5-12 %, load +0.5-1 s; under the budget |
| Seen | `out/mantle-lit.png`: standing in a room dug 1000 m under the spawn, peridotite walls lit by three lamps; `out/mantle-dark.png`: the same room without lamps is black |
| Review | not yet: a fresh reviewer reads the diff before the merge (principle 12) |
| Gate | `check.sh fast`: ok false. Tests, soak (hash ccfd8f5f5192a336), replay and stand/fly/sim (p99 4.23/4.82/6.17 ms, budget ok) pass. The views fail by pixels, cracks 0 on every one (seam 19.7 %, corner 45.3 %, lod 17.3 %, high 19.8 %, orbit 19.2 %, river 7.2 %, lake 11.6 %, seasons 11.7-16.0 %, coast 0.001 %): the shaders' per-cell texture variants hash the absolute layer index (`shader.wgsl:251,262,383,504`, `shader_vertex.wgsl:298-306`), which gains 1274, so every block shows another variant of the same texture (the likely cause, read from the code; the views were not rerun with `mantle=0`). Baselines not accepted. Feel's clicks failed (3 < 4); rerun twice on this branch 4, 3 and twice with `mantle=0` 4, 6: flaky on both, not the mantle |
| Open | The shaders' variant hash (renderer area): take the body's mantle off the layer before hashing, or accept the new views. `crack.rs:130` and `edit.rs:265` hash the layer too (a crack's shape, a drop's roll): different per cell, not visible as a change |
