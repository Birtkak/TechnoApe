# 0056 – River lines and beds in the bake

- **Status:** accepted (routine technical calls; the look is open for the developer)
- **Date:** 2026-11-12

## Context
Worldgen plan W2 left two points open: "river lines and beds" and "the walk test" (a river followed from
spring to sea never climbs). The engine's bake (ADR 0043) already drained the land: `land::bake` finds
every cell that drains at least `river_area_m2` (1.5e5 m²) and its receiver, and the prototype's
`check_rivers` showed the filled surface falls along them. But the map kept no lines, and the ground had
no beds. `docs/design/rivers-and-tides.md` puts the water of a reach at rest, its live hand-over and its
ledger in W5.

## Options considered
1. **Lower the map's 8 m cells along the rivers.** No new lookup, but the spline would round a 2 m stream
   into an 8 m dip. Its overshoot and the 1 m detail noise climb, so the walk test cannot hold.
2. **Lines kept in the map, the bed cut in the one height function.** Each river cell is joined to the next
   by a line. A profile around each line caps the ground. A spatial index finds the lines near a column.
   Exact, and every reader (server, height cache, far chunks) gets it through `height_at`.
3. **Water in the beds now** (a still surface per reach). The sea is one level per body in three places:
   the generator, `sea_quads` with its shader, and the liquid's `generated`. A level per reach changes all
   three, and the live hand-over belongs to W5.

## Decision
Option 2. Beds stay dry: option 3 is live-water and renderer work and stays in W5.

- **The map** (`bake::Map`, `VERSION` 2) keeps `rivers: Vec<Reach>` (cell, the cell it drains into, area),
  downstream first, only land cells. `bake/rivers.rs` builds `Lines` on load: 0.05 s.
- **Banks:** the lowest map height at or upstream of each river cell. One pass, upstream first, so a bank
  never rises downstream.
- **Width and depth by flow:** half width `(area / 1.5e5)^0.4`, clamped to 1-6 m (a 2 m stream to a 12 m
  river). Depth is `0.6 + 0.4 * half width` (1-3 m) under the bank. Both are interpolated along a line.
- **The profile across a line:**
  - a flat bottom out to 0.8 of the half width, wider than a column's half diagonal;
  - a rounded side up to the bank at the half width;
  - a bank that cuts at a slope of 1 for 6 m more (`BANK`).
- **In `World::height_at`,** after the detail noise: `h = min(h, cut).max(floor)`. Here `cut` is the lowest
  profile of the lines near the column. `floor` is the nearest line's bed while the column is inside that
  channel. On a line the ground is therefore the bed exactly, whatever the noise or the spline does.
- **Lines run through column centres.** A cell's centre is a corner of the columns under it (8 columns a
  cell). Each node is moved half a column, to the centre of the column just past it. A reach along a row
  or a diagonal then crosses column centres in order. Without this the walk test found 6 one-block steps up
  in 17.6 km, all where a line ran exactly along a column boundary.
- **Far chunks:** a line is cut only for columns no wider than the river (`2 * half width >= column`).
  Level-0 and level-1 chunks show every river, level 2 rivers from 4 m wide, level 3 from 8 m. Mips are
  unchanged.
- **The index:** a bit per map cell says whether any cut reaches it. Behind it is a sorted list of
  (cell, segment) pairs, from the cells around three points of each segment, padded for the narrowest
  cell (0.7 of the nominal edge). A test checks it against every segment (the slow path).
- **Old bakes and old saves:**
  - v2 maps live in `...-v2.map`. A map is kept in the file of its own version, in the bytes of its own
    layout, so its hash holds.
  - A new game (`get`, `find` without a hash) only takes a v2 map.
  - A save names its map by hash. `for_hash` reads a v1 map for it as a map with no rivers, so a world
    played before this carries on, on the ground it was played on.
  - If a v1 file is gone, the fresh bake is tried in the v1 layout. Rivers change neither the heights, the
    biomes nor the spawn, so its hash matches.
  - `Server::on_map` uses `for_hash`.
- **Bytes from a server are checked** before the lines are built: a river cell drains into a neighbouring
  cell (across a seam too), and its area is a positive number.

## Measured (dev PC, release, seed 1337, the 4 km planet)
- Bake: 30-31 s, about the same as before (49.7 s once, while other sessions built). The river cells were already computed; the PC was shared, and
  ADR 0043's 22-25 s was taken on a quiet PC. Lines on load: 0.054 s.
- The rivers: 11,693 river cells, 159 springs, no crossing reaches. The index has 706,256 pairs; the lines take 7.0 MB of RAM.
- A level-0 height tile takes 356-359 µs without rivers and 367-370 µs with them (+2 to 4 %, 3000 tiles
  over a face).
- Single random lookups: 279 to 348 ns anywhere, 213 to 525 ns within 6 m of a line. Random access
  overstates this: tiles read columns in order.
- `run=stand` on the river below: `budget_ok`, p99 2.8 ms.
- Walk test (1 km planet with smaller rivers, 17.6 km of river): 0 climbs on the line. 0 one-block steps
  up in the columns the line crosses. Every river ends under the sea level. Before the cut: 59,028 climbs,
  the worst 3.7 m.

To look along a river (wide, its bed 4 m over the sea):
`technoape.exe run=stand at=-0.075529,0.405132,-0.911133 yaw=35.4 pitch=-12 eye=5 capture_ms=2500 sheet=out/river-w2.png`

## Consequences
- New games' ground changes along every river. Their maps' hash differs (VERSION 2). Old saves keep their
  v1 map and have no beds.
- Lakes keep their flat beds at the lake's level (ADR 0043). A river crosses a dry lake flat as a channel.
- Left for W5: water in the beds (a still level per reach, through generator, sea quads and the liquid),
  lakes holding water, waterfalls where a reach drops.
- Left: a bot walking a river (the engine test walks the game's own height function and its column tops;
  a bot adds the physics, which would follow those heights).
- Not done: detail by rock and slope, bake progress on the title screen.
- Shortcut: a bank's cut ends at 6 m past the half width. Where the ground stands more than 6 m over the
  bank there, it shows a step down to the cut. Upgrade if the developer sees it in the river views.

## Review
A fresh subagent read the diff and the task. Its findings, and what was done with each:

1. **A fetched v1 map was kept under the v2 file name**, re-serialised in the v2 layout. A new game would
   then get a map without rivers, and every reconnect would fetch it again. Fixed: a map is kept by its
   own version and layout, a new game takes only v2, and memory is keyed by version too. Test
   `a_version_1_map_is_kept_apart_and_baked_back_for_its_save` (seen failing first).
2. **An old save whose v1 bake was deleted would panic**, since the rebake came out as v2. Fixed with
   `for_hash`: the bake in the v1 layout. Same test.
3. **A server's bytes could name a river into a far cell.** That gives a huge index, or a panic in `fold`,
   before the hash is checked. Fixed by the neighbour and area checks. Test
   `a_map_with_a_far_river_or_a_broken_tail_is_refused` (seen failing first).
4. **Two diagonal reaches that cross** would give two beds where they meet. Answered:
   - Steepest descent cannot make one, since each diagonal would need the other's end lower. Only a flat's
     flood order could.
   - Measured 0 on the test planet and 0 on the 4 km planet.
   - The lines test now asserts there are none.
5. **A cut-short v2 file could read as v1.** Fixed: the v1 fallback requires version 1. Same test as 3.
6. **Far chunks drop narrow rivers by level,** so where levels meet, a 2-4 m stream is cut on one side
   only. Answered: intended. It sits at a level boundary hundreds of metres away, where the mips already
   differ from the level-0 ground; the views will show if it reads badly.
7. **The walk test is close to true by construction on the line itself.** Answered: that construction is
   the decision. The test is what holds it against the detail noise, the spline, the clamp, the index and
   the column tops. The column-top check is what found the 6 steps up.
