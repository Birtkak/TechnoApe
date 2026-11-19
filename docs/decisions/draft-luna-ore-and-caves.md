# NNNN – Luna's ore and caves by her rock

- **Status:** proposed
- **Date:** 2026-11-19

## Context
`docs/design/planet-model.md`, build order step 4, second half. Since 0165-bake-stages a generation-4 Luna has a
rock field (highland crust of anorthosite, mare basalt over anorthosite, the KREEP terrane) and `Body::rock_at`
answers there, but `ore::place`, `caves::make`, the ore ledger and the report still assumed the home body: 0165
lists every such place. The developer's rules: real-world logic decides where a thing is and each must be
something to find, mine and craft with; nothing in generation is placed for where the spawn is except copper (and
Luna has no spawn of her own). Constraints: generation stays 4 (no generation-4 world has been handed over, so
this redefines generation 4's Luna); Terra and every older world generate bit for bit; the same tables and code
for both bodies, not a Luna special case; nothing beyond two bodies' needs.

## Options considered
1. **A Luna branch in `ore::place` and `caves::make`** (her own kinds in code). Rejected: it is the special case
   the step exists to remove.
2. **A `body` key on mineral and cave rows.** Not needed: a row's provinces already name one body (`rocks.toml`
   `body`, 0165), so the body is read from them (chosen: no new key).
3. For her lava tubes: **(A) keep her baked tubes** (ADR 0070: chains of balls along a random walk in each mare,
   two pits each) and let `make` add nothing in the mare, or **(B) generation-4 tubes from `caves.toml`'s round
   tube rule on her mare basalt**, the baked ones kept for generations up to 3. Measured on the standard Luna
   (seed 1, 1664 columns) below; **B chosen**.

## Decision
**What was lifted** (each place 0165 listed):
- `ore::place`, `caves::make` and `caves::systems_of` ask `Body::rock_field()` (a map with a rock layer, or Luna's
  map from generation 4) instead of returning on an empty `map.rock`. `Body::rock_body()` names the row whose
  provinces the field holds (the home's moon's on a Luna map, else the home's).
- **A kind's body** (`ore::Kind::body`, `caves::Kind::body`) is its provinces' (`rock::Province::body`); a row
  whose provinces are two bodies' is refused. A kind is made only on its own body (`kind.body != rock_body()`
  places none, so Terra never runs Luna's rows and the reverse).
- **The spawn:** the keep-out (`ore::SPAWN`, `caves::SPAWN_KINDS`) and the rings (`near`, `near_before_2`) apply
  to the home's kinds only; another body's row with a ring is refused. Nothing on Luna depends on her landing
  point (test: her ores and caves are the same with her spawn moved to the other side).
- **No sea, no shore, no water table:** `with_map` lays no sea on a Luna map before her caves and ores are made
  (before, `with_luna` took it away after; her ocean moments keep the old sea, so nothing else moves). `ore::place`
  and the cave walks keep their `DRY`/2 m over the sea datum only on a body with a sea (`caves::shore`); with
  none the depths are the ground's alone. `caves::water_table` is minus infinity on a body without a sea (dry
  to the bottom), and a karst kind is not made there at all.
- **Gravel** kinds need rivers: on a body without any they find no stream and are not placed (no code change).
- **Deep kinds** need a mantle (`b.mantle`, as before): Luna has none, so none.
- **Counts** are on the kind's body's standard size: `count * (n / bodies::columns(body, 6144))^2`, so Terra's
  rows read as before (6144) and Luna's on the standard Luna (1664).
- **Order:** a row's place seeds it, so another body's mineral rows come after the home's (after the deep kinds,
  whose index and the gossan's are unchanged) and the cave row after the karst. The parser refuses a home row
  after another body's.
- `caves::make`: ADR 0063's worms (the provinces' `caves` key) are the home's only; the kinds of `caves.toml` run
  on the home with a mantle as before, and on another body without one.

**Luna's minerals** (`data/minerals.toml`, the last three rows; uses named, no recipes):

| Kind | Block | Where | Depth | Standard Luna (seed 1) | Use |
|---|---|---|---|---|---|
| ilmenite bed (layer) | ilmenite (220, rubble 195; new) | mare, host mare basalt | 8 to 30 m | 10 bodies, 57,876 blocks, 119,970 ingots | titanium and iron; oxygen to breathe when reduced with hydrogen |
| merrillite pocket (lump) | merrillite (221, rubble 196; new) | kreep terrane, host kreep | 6 to 16 m | 6 bodies, 2,998 blocks, 6,320 ingots | thorium for power, the rare earths for magnets and electronics |
| meteoritic iron (lump) | iron-nickel (204, ADR 0070's) | highland crust, under the `metal crater` kind (an iron impactor's floor), below its 3 m lens | 6 to 9 m | 3 bodies of 12 asked (the metal craters are few), 907 blocks, 1,932 ingots | nickel, which Terra's ground does not give, and iron |

Real-world: ilmenite is the titanium oxide of the high-titanium mare basalts (the Apollo 11 and 17 sites); KREEP's
thorium and rare earths sit in phosphates, merrillite first; iron-nickel metal comes with iron meteorites. The
KREEP block stays the terrane's rock (thorium-rich, but rock, not counted); its ore is the merrillite in it. The
two new blocks are rows at the end of `data/blocks.toml` (ids 220, 221; rubble 195, 196), picks of grade 1, a
speckle pattern, no texture yet (the Studio paints them later). Terra has none of these: they name Luna's
provinces only.

**The ledger per body:** `World::ore_left` stays one list, now every world body's ore bodies in body order (the
home's first, so its indices are as before) and `World::ore_base(body)` where a body's start. `open_ledger` counts
every body, `ore_cell` answers on any body (its index in that body's `ores`), `tally` and `ore_recount` book at
`ore_base + k` against that body's own ore block, `ore::prospect` reads the same index. The ledger is never saved or
sent (the server opens it from the generated ground at start, clients keep none), so **no format and no version
changes**. `report=1` prints Luna's part after Terra's: `"luna": {n, made_hash, provinces, baked_tubes, caves,
minerals}`; a kind's cave line also gives `longest_m`.

**Her lava tubes (B).** One row at the end of `data/caves.toml`, `lunar lava tube`: the round tube rule (ADR 0160) in
mare basalt in the maria, generations 4 on, 6 to 10 m wide, 4.5 to 20 m deep, 300 to 1500 m, a start every 192
columns at 0.7, a skylight every 150 m (with the fallen roof's heap under it), a slope of 0.001 at least (her maria
are flat flows). From generation 4 `with_luna` keeps `make`'s caves and drops the baked chains; up to 3 they are her
caves as before. Why B, on the standard Luna (seed 1):

| | A: baked (ADR 0070) | B: `caves.toml` round tubes |
|---|---|---|
| tubes | 7 | 15 (with branches) |
| length, median / longest | 216 m / 289 m | 210 m / 1,321 m; 6.7 km in all |
| openings | 12 pits | 44 skylights and trenches |
| shape | balls 7 to 11 m wide every 1.2 radii: a beaded, round-bottomed passage | one round passage, its lower third filled flat, its width waving slowly, a heap under each skylight |
| made by | a random walk on the map, not by her rock | her rock field: only in mare basalt, so they stop at the anorthosite and in a deep crater |

B gives the better tube to walk (a flat floor, a steady width, up to 1.3 km long with light falling in every 150 m)
and comes from the same rule as Terra's, read off her rock. Cost: making them is 0.53 s of a Luna load (below).

## Numbers
| | before | after |
|---|---|---|
| `made_hash` seed 1 planet 6144: mantle=0 / gen=1 / gen=2 / gen=3 / default (4) | 0926f028ddcd85a7, 37b2d1b1006b66f7, a2ef250b3ed4203a, bfb878ce14d32b75, bfb878ce14d32b75 | the same, all five |
| Luna's `made_hash` (report's, seed 1): gen 0 to 3 / gen 4 | (not printed) | c6a367341bda1227 (her baked tubes, no ore) / cdd810732c670a46 |
| Luna's blocks, small Luna (`luna_blocks_hash`), gen 0 and 3 | a22a376de6be21ba | the same |
| the same at gen 4 | ca3af00891b21215 | b0f00158257f21d2 (her tubes; ore cells left out of the lock) |

## Checks
- `bake::luna::a_generation_4_luna_has_ore_and_caves_by_her_rock` (new; red before the rows and the lifted guards:
  no kinds, no caves): at most three kinds, each with bodies and blocks in its own province, host and depth and
  none of Terra's; tubes at least 4 m under ground in mare basalt, each with an opening, nine in ten skylights open
  to the sky; `water_table` minus infinity; her ores and caves the same with her spawn moved; the ledger books a
  dig on Luna in her own segment and its recount agrees; generation 3 has no ores and only the baked tubes.
- `world::baked_tests::terra_has_none_of_lunas_minerals_or_caves` (new; red while Luna's rows had no body: they
  ran on Terra): a 1536 Terra is the same at generation 3 and 4, every kind on it the home's.
- The generation-4 Luna block lock: red with the old value, set to the new one (ore cells left out, they are
  locked by the test above).
- `ore` and `caves` row tests: a row of two bodies' provinces, a ring on another body, a home row after another
  body's are refused (added with the parse checks; not each watched red on its own).
- `cargo test --workspace`: 983 passed, 0 failed (engine 456, client 208, server 134+1+1, studio 128+38, bot 16,
  live_defs 1).

## Costs
Measured once (`docs/perf-debt.md` row): Luna's world build at a load 0.04 s at generation 3 against 0.72-0.85 s at 4
(the tube walk; ore and ledger 0.005 s), on the server and each client; on a mare `run=stand quick=1` chunk jobs
383 -> 426 us, frame p99 2.73 -> 2.78 ms, cpu p99 2.41 -> 2.47, GPU p99 2.23 -> 2.31 (one run each, noise). The
placing uses the same `sin`/`cos`/`atan2`/`powi` paths as Terra's (ADR 0063): no new platform-float risk, none removed.

## Consequences
- Luna has something to mine: titanium in her maria, thorium in her KREEP, nickel under her iron craters. Recipes
  are the copper age's and later's work.
- A body made from a row (the Mars step) gets ore and caves by naming its provinces in the rows; a karst needs a
  sea, gravel needs rivers, deep kinds a mantle, all read off the body.
- Generation-4 Luna saves would move, but none exist yet (no build handed over since 0165).
- Left: Luna's row stopgaps (`weight_g`, Terra's densities: her `pressure` is Terra's rock under her pull); a
  mantle or core for her; textures for ilmenite and merrillite; prospecting across a face's seam (as on Terra).
