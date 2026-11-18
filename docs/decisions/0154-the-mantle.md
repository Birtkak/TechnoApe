# 0154 – The mantle: a new world's grid goes down to R/√2

- **Status:** accepted (step 1 of ADR 0152; merged under principle 15, the developer plays it after)
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
  in it. A recording from the seed says it by an `Ev::Mantle` right after its Start; one without it (from
  before) replays on the old floor, one from a save as the save says.
- **Whole steps of 256 layers.** The mantle is the multiple of 256 layers nearest R/√2 (`MANTLE_STEP`): the
  cells of every level and the chunks of levels 0 to 3 (32 to 256 layers tall) then stand on the same layers
  above the crust's floor as on an old world, and the surface is meshed and drawn as before (with the layer
  nearest R/√2, 1274 on the 4 km planet, every view differed: far chunks sampled other layers). Chunks of
  level 4 and up (512 layers and more, seen from a kilometre up and further) begin elsewhere against the
  surface, so their skirts and their early outs (`Empty`, `Solid`) fall elsewhere: the orbit view differs from
  its baseline by 0.20 % where the old grid gives 0.03 % (high, lod and the seasons: the same as the old grid's).
  Aligning those too would take the mantle to a multiple of 2048, past R/√2 and out of one root band. The 4 km planet: 1280 mantle layers under the
  595 it had (1875 in all); layer 0 at 2997.8 m, 4.2 m under R/√2, 1248 m under the radius; the lowest block
  0.706 m. The 8 km planet: 2816 under 601 (3417), 2534 m deep, 47 m under R/√2. The 1536 test planet: 256,
  378 m deep (67 m under R/√2, the lowest block 0.64 m). Both large planets still fit one root band of the
  streamer (2048 and 4096 layers).
- **The GPU counts from the old floor.** The client gives the GPU a body's `r0` at layer `mantle` and every chunk's
  base `mantle` layers lower (`frame.rs`), so every shader's look hash (a texture's variant and tone, a grass
  fringe, a crown's fluff and clumps, a reflection's texel) and the light's random seeds (`tx_seed`) read the
  numbers an old world gives them, and the f32 positions are computed from the same values; no shader changed.
  The cell looks of effects (a dig's cracks, a gliding block, a pile's blocks, a break's cubes) count the same
  way (`fx::gpu_cell`), and a pile item's turn is seeded from it.
  The two CPU hashes of a layer that pick a roll take the mantle off too: a cell's held item (`edit::held`,
  flint in gravel) and a good hit's drop count (`crack::drop`), as the ore halo does.
- **Mantle rock.** `data/rocks.toml` names `mantle = "peridotite"`. `Body::rock` (the one rule the mesher's fill
  and `generated_under` ask) gives it under R − 192 m in every province on a body with a mantle, and today's
  layers above. Peridotite (block 58, rubble 191) is as hard as quartzite (10, grade 1), the hardest rock of
  the crust: a steam-age pick digs it. Its texture is a draft (`tools/paint_textures.py`, `data/drafts.txt`).
- **Nothing new is placed in the mantle.** Caves, ores, water, piles and plants keep their depths under the
  ground: their placement works in radii, and the one hash that read a layer index (an ore halo's fraying)
  takes the mantle off it, so a deep world's halos are main's. An ore body and its halo are cut at the old floor
  (`OreBody::span`, the halo's count), as they were cut at layer 0 before, so the ore budget holds.
- **Heat** carries on by the province's rule (air plus `heat` C a metre): at the floor, about 1245 m down, the shield
  is about 150 C over its air, the basin 186, the folded belt 224, the ocean floor 249, the volcanic arc and the
  rift 435; under the spawn the report says 201.0 C.
- **Gravity inside a body** is a planet's with a dense core (the developer: it grows going down toward the
  mass), the same for every body: the core's radius a = R/√2, its density k = 2.4 times the mantle's,
  M = k·a³ + 1 − a³, x = r/R; g = g_surface · (k·a³ + x³ − a³) / (M·x²) for a ≤ x < 1 and g_surface · k·x / M
  under a; the square law outside as before. It grows to about 1.135 g at the mantle's floor and falls evenly
  to nothing at the centre; 192 m down on the 4 km planet it is about 1.002 g, so old worlds are in effect
  unchanged (under the radius it was up to 1.25 g before this ADR).

## Consequences
- What scales with the layer count: `Solid::gaps` looks up every chunk band of a column for edits
  (`world/solid.rs:42`: 59 bands where there were 19); `Streamer::new` makes roots per 2048-layer band
  (`stream.rs:419`), still one band on the 4 km and 8 km planets; the horizon occluder is the sphere of `r0`
  (`stream.rs:379`), now R/√2 where it was R − 192 m, so less is culled behind the planet.
- The far side's culling and the mantle's cost: `docs/perf-debt.md`.
- A jump at the mantle's floor is about 12 % lower than at the surface (1.135 g); left as it is, a taste call.
- Step 2 puts the liquid outer core under `r0`; until then a dig under layer 0 is refused as before.
- Open for the developer (plain defaults taken): peridotite's hardness and grade (quartzite's, diggable now;
  `iron-nickel`'s 20 and grade 3 would wall the mantle off until a later age); its look; the line at exactly
  R − 192 m (no fold, no mixing with the crust's last layer); the lower jump at depth.

## Checks
| Check | Result |
|---|---|
| Tests | `the_mantle_carries_the_grid_down_to_r_over_root_2` (red on a stub: floor at 4052 m); `the_readers_agree_from_the_surface_to_the_mantles_floor`, mesher fill, `generated_under`, `generated_solid_under` and the water's gaps cell by cell in a column of a 1536 planet from over the ground to layer 0, peridotite only under the crust (red with the mantle off: no cells under the crust); `gravity_falls_evenly_inside_the_planet` (red: 1.23 g at R/2); `a_player_digs_at_the_mantles_floor_and_not_under_it`, on the server a dig at layer 2, 310 m down, is sent back, one at layer −1 refused, the save keeps the mantle, a save without it loads on today's floor with its cells at the same indices (red with the mantle off); `a_19_loads_without_the_mantle_and_this_version_keeps_it` (seen red with the v19 reader set to `mantle: true`); `the_surface_is_the_same_with_and_without_the_mantle` (every generated block 40 layers round the ground near the spawn). The old gravity test now asserts the new law |
| Report, seed 1337, 4 km | every province, cave and mineral count equal to main's (copper 339 035 blocks, iron 808 378, coal 888 247, ...). Two lines differ: `made_hash` (caves' layer coordinates gain 1274; with `mantle=0` it is main's 2a79455bb6161781) and the heat under the spawn, 13.6 C at the ground and 201.0 C at the new floor (42.8 at the old) |
| Cost | `docs/perf-debt.md`, row "The mantle": fly's walk 0.1 to 1.0 ms, quads +5-12 %, load +0.5-1 s; under the budget |
| Seen | `out/mantle-lit.png`: standing in a room dug 1000 m under the spawn, peridotite walls lit by three lamps; `out/mantle-dark.png`: the same room without lamps is black |
| Review | A fresh reviewer read the diff and found: (1) effects drawn as a cell (cracks, gliding blocks, piles, break cubes) packed the absolute layer while the GPU counts from the old floor, so they were drawn 1280 layers up: fixed by `fx::gpu_cell`, test `a_cell_look_on_a_deep_world_is_drawn_at_its_cell` (red: 396 m off on the 1536 planet); films of digs could not show it (a stand run's digs make no piles nor cracks, its break uses debris), `out/fxfilm-red.png`, `out/pile-red-compare.png`; (2) a pile item's turn seeded from the absolute layer: fixed; the client's other layer uses were checked (fx noise, hud, models, audio, weather: metres or not layers); (3) chunks of level 4 and up are not aligned: answered above, orbit 0.20 % against 0.03 %; (4) ore bodies could reach under the old floor: cut there, test `an_ore_body_stops_at_the_old_floor_over_the_mantle` (red: 1263 against 1280); seed 1337's every mineral and kind has the same bodies and blocks as main's (copper 339 035, tin 16 391, iron 808 378, coal 888 247, aluminium 190 359, lead 34 957, gold 4 572, uranium 997); (5) a recording did not say its grid: `Ev::Mantle`, test `a_recording_replays_with_the_mantle_or_without` (red: the one without replayed on the deep grid); (6) the jump at depth: noted. With them the developer's gravity of a dense core, test `gravity_grows_down_to_the_core_and_falls_to_nothing_at_the_centre` (red on the even law: 0.707 g at the floor, not 1.135) |
| Gate | `check.sh fast` after the review and main merged in: ok false on one line, perf fly's work 1.58 ms against 0.95 (the streamer's walk, row "The mantle" in `docs/perf-debt.md`, not accepted here). Tests, soak (hash 24f48a05974e46fb), replay, stand/fly/sim (p99 4.82/5.49/6.28 ms, budget ok), all twelve views against main's baselines (largest 0.24 %) and feel (4 clicks, 13.4 ms) pass. Before the surface fix every view differed by 7 to 45 % |
| Bot | a `technoape-bot` over TCP on a new 4 km world: `{"block":[0,0,0],"face":2,"pos":[5259,3435,337],"material":58}`, dug, then `material 0` at the same cell, 1000 m down (layer 337), `refused 0` |
