# NNNN – One bake of named stages; Luna's rock field (generation 4)

- **Status:** proposed
- **Date:** 2026-11-19

## Context
`docs/design/planet-model.md`, build order step 4, first half. Terra's bake (`bake/land.rs`, `life.rs`: plates,
erosion, rivers, climate, life) and Luna's (`bake/luna.rs`: craters, maria, tubes) were two pipelines picked by a
row's `bake = "terra" | "luna"` (0163-n-bodies) that shared nothing; a Mars-like planet needs half of each. Luna had
no rock field: `Body::rock_at` answered `Rock::NONE` there, her rock was her kinds' biome rows' (`data/biomes.toml`:
anorthosite under the highland dust, mare basalt under the maria's all the way down), so `rocks.toml`,
`minerals.toml` and `caves.toml` meant nothing on her. Constraints: old worlds bit for bit (the locked `made_hash`es,
the version-21 fixtures), Terra's baked map byte-identical (so the cached `saves/bakes/` files stay valid), no rewrite
of the stages' insides, no physics deciding stages (the regime is a later step).

## Options considered
1. **Keep `bake` as a preset name** – nothing to restructure, but a new body cannot pick stages. Rejected.
2. **A `stages` list in the row, today's code split into stage functions (chosen).** Each stage runs on what the
   ones before made; the split is made where today's code already passes whole values from one part to the next, so
   the bytes cannot change.
3. **One global stage order (the planet-model table's)** – Luna's code lays the polar shadow before the tubes and
   Terra's ice is a rule inside the biomes, so one order for both is not possible without rewriting a stage. The row's
   order is the run order; `check` holds what each stage needs before it.
4. For Luna's rock: **bake a `rock` layer into her map** (new bytes, a new map version and hash, every Luna save
   re-pointed) against **read it from the kinds her bake already laid (chosen)**: no new bytes, her map file and hash
   stay, and the province border is exactly the kinds' border (same jitter).

## Decision
**Stages** (`bake::Stage`, `bake::check`, `bake::run`). A row of `data/bodies.toml` lists `stages`, in the order
they run, in place of `bake`:
- Terra: `["plates", "erosion", "climate", "ice", "life"]`; Luna: `["crust", "impacts", "ice", "volcanism"]`.
- `plates` (`land::crust`: grids, plates, boundaries) and `crust` (`luna::crust`: the old crust without plates) are
  the two crusts; one of them is first.
- On plates: `erosion` (`land::erosion`: coarse and fine erosion, the provinces as the grid is refined, the sea,
  rivers and lakes), `climate` (`life::climate`: the air model's year on the cells), `life` (`life::life_of`: water on
  land, soil, biomes), and `ice`, the polar caps (ADR 0107), which the biome rule lays: it is listed between
  `climate` and `life`, and without it the bake is the capless one (map versions 5 and 6 are Terra's list without
  `ice`, `Stage::TERRA_ICELESS`). A bake on plates ends with `life` (the map's ground is its biomes).
- On a crust: `impacts` (`luna::impacts`: basins and craters oldest first; the maria flood among them at
  `T_MARE` when `volcanism` is listed, as the real ones did), `ice` (`luna::ice`: the polar shadow), `volcanism`
  (`luna::volcanism`: the thorium province and the lava tubes). Luna's order puts `ice` before `volcanism`; in
  her old code the thorium came first: the two commute (the thorium takes only highland cells, the shadow every cell
  but metal, and the shadow draws no random number), and the lock below proves it.
- `check` refuses with a message: an unknown name (the reader's "unknown variant"), a list not starting with a
  crust, a crust past the first place, a stage twice, a stage without what it needs before it, and the crossings not
  built yet ("not built yet on plates": `impacts`, `volcanism`; "not built yet on a crust without plates": `erosion`,
  `climate`, `life`). The Mars step lifts the crossings.
- `bodies::Bake` is the list and its body's sun at its highest (`luna::sun_high` of the row's size and orbit),
  which a crust's `ice` reads: a stage gets the body it bakes, never looks one up. Terra's and Luna's lists keep
  their maps' files, versions and old layouts (`bake::get`, `for_hash`, `luna::get`, ...); `luna::bake` is the
  home's moon's (`bodies::moon`). Shortcut: another list is baked on every call and kept under a file name nobody
  writes (no body has one today); the Mars-like planet's step gives such a body its cache name.
- `Map::bake`, `bake_with`, `bake_v5`, `bake_v6` go through `run_with` (plates), `luna::bake` and `bake_stats`
  through `luna::run` (a crust), and `run` picks between them by the first stage. A reordered crust list
  (`volcanism` before `ice`, or no `volcanism`) bakes: it is another body, the rng order is the same (the ice draws
  no number), only Luna's own list must make today's Luna.

**Generation 4** (`world::GENERATION` 3 -> 4; `known_generation` takes 4 and refuses 5). Every body now carries its
world's generation (`Body::gen`; before, only the home's was set, Luna's stayed 0: no rule of hers read it, checked
by the lock). From 4, `Body::rock_at` answers on Luna; before 4 it answers `Rock::NONE` there as it did. Terra is the
same at 3 and 4 (no rule of hers asks 4).

**Luna's rock field.** `Map::rock` on a Luna map is `luna::province(kind)` of the cell's kind: the maria (and fresh
mare) the `mare` province, the thorium kind `kreep terrane`, every other kind (highland, fresh highland, metal
crater, polar shadow) `highland crust`. Rows in `data/rocks.toml` after Terra's (ids 6, 7, 8; `bake::rock::NAMES`):
- `highland crust`: anorthosite to the bottom. Real: the lunar highlands are the old feldspar crust that floated on
  the magma ocean, pale and heavily cratered.
- `mare`: `mare basalt` 60 m from the datum (60 m), so about 40 m of basalt under a mare's ground (about 40 m over the
  sea datum), over anorthosite. Real: the maria are basalt flows that filled the near-side basins a few hundred
  metres to kilometres deep over the highland crust; a deep fresh crater in a mare reaches the anorthosite under it,
  as Copernicus-class craters do.
- `kreep terrane`: KREEP 45 m from the datum, about 20 m under the dust, over anorthosite (the Procellarum terrane
  round Imbrium is a highland crust rich in potassium, rare earths, phosphorus and thorium).
- fold 2 m over 400 m, heat 0.02 C a metre (a dead, cold body), caves `none`.
The biomes' dust (the band: 4 m on the highlands, the top metre on the maria) still lies on top: the rock field is
what lies under it. No new blocks: anorthosite (200, rubble 182), mare basalt (202, rubble 183) and KREEP (205,
rubble 185) came with Luna's bake (ADR 0070), each mined with a pick, with a rubble twin, stacking to 16.

**The tables' body key.** A province row may say `body = "<row name>"` (`rock::Province::body`, a row index; the
home's when not given); a name that is no row is refused, and so is a key that disagrees with the province's id:
ids to the ocean floor are the home's, those after it the home's moon's (`Map::rock` hands them out by id), so the
loader keeps Terra's provinces off Luna and the reverse. Terra's six say none, Luna's three `body = "Luna"`. A Terra
map whose cells name another body's province is refused on read (`Map::from_bytes`: Terra's provinces only).
A world without a mantle (generation 0) never gets Luna's rock field: it is asked from generation 4 only.
`minerals.toml` and `caves.toml` gain theirs with the second half (they key on provinces and rocks, so the key above
is what they will filter by).

**Uses (the developer's rule: every rock is something to find, mine and craft with).** Named, not built:
- anorthosite: nearly pure plagioclase feldspar, so aluminium (as the Moon's real plan is: aluminium and oxygen
  from highland regolith) and the raw stuff of ceramics, glass and refractory bricks for Luna's furnaces;
- mare basalt: ilmenite in it (titanium and iron, and oxygen to breathe, ADR 0070), basalt fibre and cast basalt for
  building;
- KREEP: thorium and the rare earths, late-game power and magnets.

## Numbers
| | before | after |
|---|---|---|
| Terra's map bytes (seed 5, 256; v6 and v5 of it) | 16ee32bb94308b73, 065e789cbd17a892, 5bfb7a403005f7d7 | the same |
| Luna's map (seed 7, 416) | cb20ef2abc8ed3a7 | the same |
| Cached `seed-1-planet-6144-v7.map` / `seed-1-planet-1664-v102.map` | main's files | baked fresh by this build (28.9 s, 1.9 s): byte-identical, so the caches stay valid |
| Luna's generated blocks, small Luna (`luna_blocks_hash`), gen 0 and 3 | a22a376de6be21ba | the same |
| the same at gen 4 | - | ca3af00891b21215 |
| `made_hash` seed 1 planet 6144, mantle=0 / gen=1 / gen=2 / gen=3 / default (4) | 0926f028ddcd85a7, 37b2d1b1006b66f7, a2ef250b3ed4203a, bfb878ce14d32b75 | the same; default (4) bfb878ce14d32b75 |
| Luna's provinces, small Luna (seed 7, 416), every 31st column of 33,495 | - | highland 0.873, mare 0.112, KREEP 0.015 (the map's maria 0.115) |

`made_hash` is Terra's caves and ores only: it never covered Luna, hence the block lock above. The "before" map and
block hashes were computed again on an archive of main c1ceeff0 with main's code: the same hex, all four maps and
both generations.

## Checks
`cargo test --workspace`: 980 passed, 0 failed, 26 ignored (client again after the `place=` fix: 208 passed). Red
first: `bad_rows_are_refused` (an empty stage list was taken) and the generation 4 Luna test (province 255 in
column 0). `tools/views.sh` (play build): ok, no cracks; pixel differences 0 to 0.26 % (orbit 0.257, winter 0.244,
lod 0.13), within the views' own noise, since Terra's ground is unchanged. Pictures: Luna's maria dark and
highlands pale from orbit (as before: the surface is the biomes'); dug pits on a mare and a highland.

## Costs
One run each, `slot.sh job`, play build, main's against this one (row in `docs/perf-debt.md`): Luna's bake
1.92 -> 1.94 s; world load 0.87 -> 0.86 s; `run=stand quick=1` on a mare frame p99 2.62 -> 2.68 ms, chunk jobs
397 -> 422 us (+6 %: `rock_at` folds and reads the province on her columns now).

## Consequences
- A body is a list of stages; the Mars step adds stage code (a crust with craters and rivers, shields) and lifts
  the crossings in `check`, and names its map file (the shortcut above).
- Luna's ground under the dust follows rock provinces from generation 4: anorthosite highlands, mare basalt over
  anorthosite. On the surface nothing changes (the dust and the bands are the biomes'), so she looks the same from
  Terra and from orbit.
- `place=`, `fxedit=` and `structure=` in a stand shot built on Terra whatever `body=` said; they now use the
  stand's body.
- The creative inventory puts anorthosite, mare basalt and KREEP in the rocks' tab (it lists every province's
  layers).
- **What ore and caves on Luna need (the second half)**, every place that assumes the home or a map with a baked
  rock layer:
  - `ore::place` and `caves::make` return nothing when `map.rock` is empty, which a Luna map always is: ask the body
    (`rock_at` gives a province) instead;
  - `ore::place`: the spawn keep-out and `kind.ring(gen)` read `b.spawn` (Luna's landing point: by the rule only
    copper may be near it, so the keep-out stays and the rings must not apply); the gravel kinds need `map.rivers`
    (none on Luna); `ground - sea < 2.0` reads a sea datum Luna has no sea at; counts scale by `(n / 6144)^2`; the
    mineral rows' `provinces` and `biomes` name Terra's: they need Luna's rows and a body filter; the deep kinds need
    a mantle (`b.mantle`, the home's only, `World::of`);
  - the ledger: `World::open_ledger` counts `bodies[0]` only, `ore_left` is one list; `World::ore_cell` returns None
    for `body != 0`; `tally` books against `bodies[0].ore_block`; `ore::prospect` reads the ledger only for body 0;
    `report.rs` reports `bodies[0]` only;
  - `caves::make`: the worms are gated on `b.gen < 3 || b.mantle == 0` and `systems` (the kinds of `caves.toml`) on
    `b.mantle > 0`; `Body::with_luna` replaces whatever `make` made with her baked tubes (`set_caves(tubes)`), so
    Luna's caves by rock must be added to the tubes, not put in their place; `caves::water_table` reads the sea;
  - Luna's row stopgaps: `weight_g` (Terra's pull for her rock's weight) and Terra's densities, which `pressure` and
    the deep kinds' windows read.
