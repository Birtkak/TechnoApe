# 0063 – Rock, caves by rock, ore bodies and the ore ledger

- **Status:** accepted (W4 steps 2 and 3 of `docs/plans/worldgen-plan.md`; the developer's four calls of
  2026-11-12; the rest routine technical calls). The rocks' and ores' looks are drafts, back to the developer.
- **Date:** 2026-11-12

## Context
`worldgen.md` stages 9 (rock) and 10 (ore) and `ore-budget.md`. Step 1 (ADR 0062) made one generated block
for every reader. The developer's calls: the ground goes 192 m deep, hot and undiggable below for now; real
rock and mineral names on the "looking at" label, no map, ore found by eye from gravels, stains and plants;
one palette draft per rock, shown on a look sheet before any is accepted; `ore-budget.md`'s numbers kept per
block.

## Options considered
1. **Rock and ore as data in the map** (the bake writes a rock and ore id per column and layer). Exact, but
   tens of MB per planet and a bake per change of a rule.
2. **Rock as a rule over the map's province, ore bodies as counted objects** placed from the map and the seed
   when the planet loads, the same on the server and every client. The map grows by one byte a cell.
3. Ore as noise thresholds per rock. Cheap, but the amount is whatever the noise gives: no budget, no ledger.

## Decision
Option 2.

- **The deep crust.** `world::deep_layers(n)` adds whole layers under the old 64 m crust until it is at least
  192 m (146 layers on the 1.5 km test planet, about 140 on the 4 km one; none under 768 m of radius, so the
  small planets keep theirs; Luna of the 4 km planet, 1150 m in radius, gets some, and its saves move too). The surface keeps its radius; every layer index moves up by that many.
  Below layer 0 nothing is diggable (`Shape::fits`, as before). Saves move to version 10 (W5's 9 lies on disk the same): an older save's edits
  and water gaps move up by `deep_layers` (test `a_save_from_before_the_deep_ground_moves_up`).
- **Provinces** (`bake::rock`, stage 9): shield, basin, folded belt, volcanic arc, rift, ocean floor, from
  the plates (distance to collisions, arcs, rifts; old sutures as folded belts too). The map is **version 5** (W5's lakes and flows are 4)
  with a `rock` byte per cell; older maps load with none (no rock, no caves, no ore).
- **The rock rule** (`data/rocks.toml`, `rock::ROCKS.at`): each province's layers top down from a datum 60 m
  over the sea, folded by noise of the province's amplitude and wavelength. The mesher's fill (`generate`, from
  the tile's `rock` per column) and `generated_under` ask the same rule under the 4 m soil band.
- **Caves by rock** (`caves::make`): limestone systems that hold to the water table (six tenths of the
  height over the sea smoothed over 40 m, at least 3 m under the ground) and end where the limestone does;
  lava tubes 3 to 6 m under basalt ground, downhill, with skylights. Kept off seams (12 columns), the spawn
  (40) and the sea (3 m of land). Balls bucketed by chunk footprint (`cave_tiles`), which the review of 0062
  asked for.
- **Heat** (`Body::heat`): the bake's year mean at the ground plus the province's `heat` a metre down
  (13.6 C at the spawn's ground, 42.8 C at the crust's floor, seed 1337).
- **Ore bodies** (`ore.rs`, `data/minerals.toml`, stage 10): kinds with a shape (lump, vein, seam, layer,
  blanket, cap, bog, gravel), the budget's count and blocks per body (scaled by area on other planets, at
  least one), depth, provinces, host rock, biomes, a ring from the spawn for the first body, a gravel kind in
  the stream bed below and a gossan blanket over deep ones. A body is an ellipsoid solved from its blocks;
  grade (ingots per block) is 3, 2, 1 from its middle out. **One rule** `Body::ore_at` (ground, not a cave,
  first body wins) is what `generated_under`, `generate` at level 0 and the count ask.
- **Ore blocks are their own blocks** (malachite, chalcopyrite, ...), not flecks over the host rock; the sign
  blocks (copper, tin and gold gravel, gossan) are loose.
- **The ledger** (`World::ore_left`): the server opens it with the exact count before a save's edits, and
  `set_block` / `put_edit` move it when a body's own ore leaves or comes back to one of its cells. The soak
  checks every tick that it equals a recount from the edits (`Server::ore_ledger`). Clients keep none.
- **The report:** `technoape-server report=1 seed= planet=` prints provinces, caves, heat and every element's
  and kind's bodies, blocks, ingots, budget, nearest distance and depth.

## Shortcuts and consequences
- Clients derive rock, caves and ore themselves from the map; nothing new goes over the wire. A rule change
  is a protocol change (the map hash covers the map, not the rules).
- W5 reached main first with map 4 and save 9; this takes map 5 and save 10.
- Recordings from before the deep crust do not replay (every layer moved).
- Indicator plants over bodies are not done: a follow-up (stage 10's third sign).
- `bake::rough` does not read the rock yet (hard rock in steps): a follow-up.
- The ledger counts the planet only; Luna has no minerals until its own budget.
- The budget lives in `report.rs` (`BUDGET`) until `data/budget.toml` exists with the recipes.
- **Not proven across platforms:** caves and bodies are made with the platform's `sin`, `cos`, `exp`, `ln`;
  one ulp of difference between the Linux server's libm and a Windows client's can add or drop a whole cave
  or body on one side. `report=1` prints `made_hash` (seed 1337, 4 km: 853164ce64047dd8 on Windows); run it
  on the server before a remote game. If they differ: a pure-Rust `libm` for these paths, or the server sends
  the lists.
- Coal seams lose some blocks where valleys cut them (902k of 1.1M blocks on seed 1337): the seams are
  190 m long at a fixed layer.

| Check | Result |
|---|---|
| Tests | readers agree on ore and caves cell by cell (red with the mesher's ore off); the ledger reaches 0 when a body is dug (red with the tally off); a player digs a body out on the server and the soak's recount agrees (red: 60 against 0) |
| Report, seed 1337, 4 km | copper 339k blocks of 330k, tin 16.6k of 18k, iron 808k of 800k, coal 902k of 1.1M, aluminium 190k, lead 35k, gold 4.9k of 4k with its gravels, uranium 1k. Nearest: copper gravel 143 m, bog iron 344 m, tin gravel 760 m and vein 940 m, coal 563 m, deep copper 1.27 km, lead 789 m, banded iron 3.3 km |
| CPU cost | `quick=1`, the branch's last commit against this, interleaved, three runs each. `fly`: frame CPU 2.11 to 2.22 ms (p50), mesh job 343 to 308 us, GPU 1.80 to 1.87 ms. `stand`: frame CPU 1.58 to 1.67 ms, draws 362 to 557, quads 212k to 249k (rock bands break the mesher's merges; caves add faces), GPU 1.30 to 1.36 ms. Selection and the server's tick unchanged. RAM +20 to +50 MB. Opening the ledger (exact count) and the report: 0.3 s for the 4 km planet; caves and bodies are made in under a second at load |
| Review | a fresh reviewer read the diff: the readers, the ledger and the save shift hold. Fixed: physics asked rock and ore per deep cell to learn only "solid" (now `generated_solid_under`, debug-checked against the full answer); `generate` lost its shortcuts on every chunk of an ore footprint (now only those within the bodies' layers) and asked `ore_at` per cell; a `host` on a ground-following kind was read at the wrong depth (now refused); this ADR was wrong about Luna. Left: the cross-platform risk above |
| Gate | `tools/check.sh` 2026-11-12 on d5fff26: tests, soak (50 bots, 3600 ticks, the ore ledger checked each tick; hash e2bde982593acd7f), replay and feel pass. The fixed views fail as they must (rock and ores now show; not accepted until the developer has seen them). The benchmarks failed while another session compiled (mesh jobs 3305 us in `stand`); alone under `out/.lock` right after: `stand`, `fly`, `orbit` within budget, jobs 282, 306, 193 us against the baseline's 294, 273, 205. `fly`'s stale baseline (GPU 1.48 ms, wgpu memory 725 MB) is not accepted; against the last commit it is +0.06 ms GPU, the same memory |
