# 0043 – The bake in the engine: Terra's map, biome ground and still plants

- **Status:** accepted (the developer accepted the biome layout and looks on 2026-11-11: "Accept both");
  the views wait for the developer's look before `tools/views.sh accept`
- **Date:** 2026-11-11

## Context
ADR 0042 baked Terra's climate and fifteen biomes in `prototypes/world-bake`, but the game still stood on the
noise land of ADR 0021. Plan W2 (`docs/plans/worldgen-plan.md`) moves the bake into the engine. This ADR
takes that as far as the biomes need: the map, its heights, its biomes' ground and still plants, the spawn,
the save and the download. It also records the looks the developer accepted:

| biome | ground (top / band / rock; patches) | plants |
|---|---|---|
| ice | ice / snow / stone | none |
| tundra | grey-green grass, gravel patches / dirt | rare shrubs |
| cold forest | blue-green grass, snow patches / dirt | spruce 8-12 m, shrubs |
| temperate forest | grass / dirt | broadleaf 6-9 m, shrubs |
| grassland | yellow-green grass / dirt | lone broadleaf, shrubs |
| dry scrub | olive grass, dirt and gravel patches / dirt | shrubs 1-2 blocks |
| desert | sand / sand / stone | rare cacti 2-4 blocks |
| savanna | straw grass / dirt | flat-topped acacias far apart, shrubs |
| rain forest | deep green grass / dirt | dense jungle trees 12-16 m, undergrowth |
| wetland | mud, dark marsh grass patches / clay | reeds, shrubs |
| alpine | gravel, stone patches / stone; snow over 100 m | none |
| beach, shelf | sand / sand | none |
| deep sea | mud / clay | none |
| warm reef | sand, coral patches / sand | none |

After seeing them in game, the developer asked for four changes (2026-11-11), made in data and in `Plant::block`:

- **Sand** is a warmer yellow, plain with soft grain instead of speckle; it read grey-white.
- **Dry scrub** has fewer dirt and gravel patches (0.1 and 0.04, from 0.2 and 0.1); it read as rubble.
- **Trees** are fewer and less boxy:
  - broadleaf crowns have a radius of 2, from 3;
  - every round crown loses about a third of its outer shell to a hash;
  - fewer spots grow trees: broadleaf 0.25 (from 0.35), jungle 0.4 (0.6), spruce 0.38 (0.5).
  - Forests show trunks and ground between the crowns, not a closed roof.
- **Grasses:** meadow is less neon, scrub and savanna a little warmer.
- A fresh reviewer read the crown change and found nothing. It checked that the hash cannot overflow
  within the reach and height limits, and that about 35 % of the shell is dropped.
- The soak gives `2ccfc15b35aeef0d` on two runs. Two quick `sim` runs have `budget_ok`, with max
  frames of 5.0 and 4.5 ms and a tick p99 of 1.5 ms. A first run after the build had one 23.8 ms stall,
  and the reruns had none.

New blocks (ids 8 to 29): sand, snow, ice, gravel, clay, mud, log, four leaves (spruce, broadleaf, jungle,
acacia), cactus, shrub, reed, coral, and seven tinted grasses. The tints are separate blocks, so far chunks
show each biome's colour without any change to the renderer or shaders.

## Decision (routine technical calls)
- **`engine::bake`** (`crates/engine/src/bake/`): the prototype's grid, land (stages 1 to 4), life
  (5 to 8) and air modules, moved as they were. Their only edits are the `use` paths, plus the spawn below.
  - A `Map` keeps, per 8 m cell, the height over the sea (a lake cell at its level, flat until lakes hold
    water in W5) and the biome. It also keeps the spawn and its hash. Mips of the heights (2 x 2 means)
    are built on load.
- **Kept by hash.** `bake::get(seed, n)` takes the map from memory if a local server, its client or in-process
  bots already have it; from `saves/bakes/seed-<s>-planet-<n>-v<VERSION>.map`; or else it bakes and keeps it.
  `VERSION` changes whenever the bake makes something else.
- **Old worlds keep the noise land.** `World::new` stays the noise land, for tests and for saves from before
  the bake. New games use `World::baked` and `Server::baked`.
  - A save of version 6 names its map's hash. Versions 4 and 5 load on the noise land.
  - A save whose map comes out different stops the game, as an unreadable save already does: its edits
    belong to other ground.
- **Heights:** a Catmull-Rom spline through the 4 x 4 cells around a column, folded over seams, plus 1 m
  of detail noise.
  - Chunks of level L read mip L - 3, so a mip cell is about as wide as their columns.
  - The sea floor is clamped to 56 m (the bake's 60 m would reach `r0`; CRUST stays 64, so saves keep
    their layers).
  - `AMP` goes from 80 to 140 m for the bake's 120 m peaks and the tallest tree. The noise land keeps its
    old 80 m bound, so old worlds keep their heights to the bit.
- **Ground per column** from `data/biomes.toml` (schema and checks in `crates/engine/src/biomes.rs`):
  - top, band and rock blocks;
  - patches (a share of the surface, in clumps of about 3 columns);
  - snow above a height.
  - The biome lookup and the patches are moved by noise of about 6 columns, so borders interfinger.
  - The height cache's tiles hold each column's ground; `generate`, `generated_under` and the water read it.
- **Still plants** as plain blocks, through one function:
  - The plants come from one candidate spot every 4 x 4 columns, hashed from the seed. The biome's rows
    give the share that grows each plant. The shapes are data: cone, round, flat, column and bush.
  - A plant stands at least 3 columns inside its face, so none crosses a seam.
  - `Body::plants_with` is the only source. The mesher reads it through the tile, physics through
    `col_plants`, and the water through `Solid::gaps`. A test checks that all three agree on tall crowns
    over slopes, with tiles cached and uncached. It was seen failing with the water's plants removed, and
    with the physics cutoff below.
  - Plants stand only where the ground leaves air. An edit wins over them. Wood wins over leaves where
    two crowns meet.
  - Plants are on level 0 only: far chunks show the biome's grass colour.
- **The spawn by score:** grassland or temperate forest, gentle (slope under 0.08), within 160 m of water,
  the temperature nearest 14 C. The spawn is then moved to a column with no plant around it.
  - The search keeps within 26 degrees of longitude of the +y meridian, because the sky's clock puts 9 am
    at world time 0 there (`sky.rs`). Without that limit, the first scored spawn lay in the night.
- **The download:**
  - `Welcome` names the map's hash.
  - A client that has the map neither in memory nor on disk sends `ToServer::Map` and gets
    `ToClient::Map` pieces of 1 MiB (18 MB for the 4 km planet). The server makes the pieces once.
  - `net::fetch_map` keeps whatever else arrives meanwhile and hands it on in order.
- **The soak** runs on a baked planet, and its bots share the server's map. Its Luna grows from 32 to 128
  columns: at 32 (radius 22 m), a sixth of Luna's ground lay under its core (measured on the noise world,
  before this change), and a walking bot fell in.

## Measured
Reference PC, release build, `out/.lock` held, bake on 8 of 12 threads (`RAYON_NUM_THREADS=8`):

- **Bake** of the 4 km planet (seed 1337): **25.0 s**, once per seed and planet; then it loads from its
  17.7 MB file at start. The prototype's peak was 349 MB during the bake. The planet of 256 columns
  (the soak's) bakes in 0.3 s.
- **Quick runs** (`quick=1`, after the review's fixes), against `tests/perf/baseline.json`:

  | run | frame p99 ms | max ms | GPU p50 ms (baseline) | job µs (baseline) | RAM MB | tick p99 ms (baseline) |
  |---|---|---|---|---|---|---|
  | `stand` at the spawn (forest) | 2.69 | 3.03 | 1.74 (1.69) | 357 (551) | 352 | - |
  | `fly` | 4.01 | 4.92 | 1.58 (1.51) | 267 (455) | 413 | - |
  | `sim` (50 bots at the spawn) | 3.36 | 5.31 | 1.97 (1.67) | 354 (588) | 371 | 1.88 (2.70) |

  - Every run has `budget_ok`.
  - Mesh jobs are cheaper than on the noise land: a spline and one noise against six noises per column.
  - `sim` draws a forest now, hence the GPU's extra 0.3 ms.
  - The default `run=stand` (at `0,1,0`) now looks at open sea, so it is no longer the same scene as the
    baseline.
- **The server's tick** with 50 bots in the forest was 8.7 ms p99 before plant spots were cached per
  body. The server builds no height tiles, so each physics lookup worked out its spots again. It is
  1.9 ms with the cache.
- **The soak** (50 bots, 1 minute, seed 1): `111dc213e16494d8` on both runs, and its replay agrees at all
  60 checkpoints.

## Review (CLAUDE.md principle 12)
A fresh reviewer read the diff. Each finding and what became of it:

- **Physics dropped crown blocks that hang over lower ground.** The air cutoff was measured from the leaf
  column's own ground.
  - Fixed: `PLANT_TOP` is now 42 m, the tallest plant on ground up to 8 m higher a column out.
  - The agreement test now covers whole crowns of the 40 tallest spots. It fails with a 6 m cutoff.
  - shortcut: a margin, not a bound; the test checks it on its planet.
- **Raising `AMP` moved the noise land's clamp,** so old worlds' island peaks would have changed. Fixed:
  the noise land keeps 80.
- **A rebaked map that differs was played with only a warning.** Fixed: it stops the game.
- **The map was serialised again for every request.** Fixed: the server makes the pieces once.
  - shortcut: a client may still ask any number of times; a limit per client comes when servers go public.
- **Without tiles, the server worked out plants for every lookup.** Fixed with the spot cache (numbers above).
  `generated_under` also reuses the column height it is given.
- **Answered, not changed:**
  - `generate` tests air in f32 while the others use f64 (older than this change: about 1 column in
    2,000 can differ by a layer at the ground's top).
  - Old recordings no longer parse: `Ev::Start` and saves gained a field. They fail with an error.
  - `meshbench` (`crates/client/src/sys.rs`) stays on the noise land: the middle of +y is sea on the map.

## Consequences
- **Files outside world generation:**
  - `crates/client/src/main.rs`: a new game's server bakes, and the client fetches the map.
  - `crates/client/src/frame.rs`: ignores `Map` pieces after the start.
  - `crates/bot/src/lib.rs`: fetches the map.
  - `crates/server/src/{lib,save,soak,main}.rs`.
  - `crates/engine/src/net.rs`: the protocol.
  - `.gitignore`: `saves/` at any depth, since tests that bake keep maps under their crate.
  - No renderer or shader file changed.
- **The developer's existing saves keep the noise land.** A new world from the title menu bakes. Its first
  start waits about 25 s with no progress shown: the loading screen is the bug session's area, and the
  title screen's progress bar (W2) is left for later.
- **Clock and spawn:** the sky's clock should take the spawn's own longitude (`sky.rs`); until then the spawn
  keeps near the +y meridian.
- **Not built (out of the brief's scope, or later W stages):**
  - rivers cut into the ground, and lakes as water (W5);
  - caves, rock layers and ore (W4);
  - tides;
  - live weather (the air model runs only in the bake);
  - seasons and tilt (not chosen);
  - living plants (W7);
  - the globe map (W2b);
  - the bake's progress on the title screen.
