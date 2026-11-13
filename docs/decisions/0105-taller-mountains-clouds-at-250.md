# 0105 – Taller mountains, clouds at 250 m

- **Status:** accepted
- **Date:** 2026-11-13
- **Supersedes:** ADR 0098's cloud base of 100 m (its crisper, more open clouds and near-camera march stay)

## Context
The developer (2026-11-13): "the highest mountain peak [should] be able to reach the clouds since we have those
in real life too". ADR 0098 answered that by lowering the cloud slab to 100 m over the sea, because the 4 km
planet's highest land is 120 m (`bake::land` `mountain_m`); that put the clouds low over every hill. The
developer then decided: make the mountains taller instead and put the clouds back up.

The guiding principle for the bake from this round on (the developer, 2026-11-13): "i want features to be
realistic ish, but we need to exaggerate some things since we are working on such small planets". Landforms
follow real-world logic (how and where they form); their relief and contrast are exaggerated so they read on a
4 km-radius world where real proportions would be invisible.

The baked land is strongly skewed: on seed 1337 half the land is under 14 m, 99 % under 50 m, 99.9 % under
77 m, and one summit reaches 120 m (seed 480895600 is broader: p99 64 m).

## Options considered
1. Raise `mountain_m` (one scale for all land): every hill, plain and coast rises in proportion; rivers'
   gradients and the lowland biomes all change.
2. New mountain noise on top: peaks where the plates put none; rivers would run uphill.
3. A monotone remap of the baked height above a threshold, where the bake turns its model heights into metres
   (`land::bake`): under it the land is the old bake's to the bit; above it the relief is stretched by a factor
   that eases in, so the order of heights (every downhill, so every river and lake) is kept.

## Decision
Option 3, `land::stretch`. Under `STRETCH_FROM` (45 m) a height is unchanged; above, the height over 45 m is
multiplied by a factor that eases from 1 to its full value over 20 m (smoothstep, so the slope is continuous),
the full factor such that the highest land (120 m) stands `peak_extra_m` higher. The lakes are found and the
puddles filled before the stretch (so the lake cells are the unstretched bake's), then the ground, the filled
surface and the lake levels are stretched; the climate, soil and biomes are made from the stretched heights.
The cells' temperature now takes the air model's own lapse (`air::Params::cool`: 0.1 K/m to 150 m, 0.015
above) instead of 0.1 K/m all the way, in `life` and `Map::temp` alike (the same to the bit under 150 m, so
every older map's temperatures hold): a 350 m summit is 18 K colder than the sea, not 35.

Taste calls made by default (the coordinator's targets, recorded as such; all open):
- `peak_extra_m` 230: the highest land 350 m over the sea on the 4 km planet (target 320-380); mountain ground
  that was 60-80 m is now about 110-175 m; the cloud base back at 250 m (`CL_BASE`, the lightning's height
  follows), so only real mountains reach into and through the slab, the highest by 100 m (the slab is 64 m
  thick: its top at 314 m, the highest summits above the cloud sea).
- Planet sizes: the extra scales with the planet up to the 4 km one (`n / 6144`, at most 1): the 768 planet's
  highest land is 149 m, 1536's 178 m, the 8 km planet (12288) gets the same 350 m as the standard one (the
  clouds are at the same height on every planet). Luna's bake (`luna.rs`) is its own and keeps its relief.
- `world::AMP` (how far generated ground may lie from the radius) 140 -> 400, so the streamed layers and the
  "whole chunk is air" test cover the new peaks. The clamp of baked heights follows it (378 m) on maps of this
  version only; older maps (and Luna's) keep the clamp of 140, so their ground is the same to the bit.

Bake version 6 (`bake::VERSION`). A save names its map by hash: a version 5 map is found in `saves/bakes/` as
before, and when its file is gone `for_hash` bakes it back with `Map::bake_v5` (the bake without the stretch
and with the old lapse being the same under 150 m), from which the older layouts (4 to 1) are made as before.
That costs a second bake (about 36 s more on the 4 km planet) only for a save whose map file is gone. New
worlds get version 6.

## Evidence
Seed 1337, 4 km planet (`out/tm/` of the worktree; `b-` before, `a-` after, `play` build):
- Highest column: cell (face 2, i 673, j 660), direction (0.4660, 0.7342, 0.4938), 120.0 -> 350.0 m; seed
  641653600's highest stays at its cell too (120 -> 350 m).
- Land heights: p99 49.8 -> 51.9 m, p99.9 77.1 -> 175.4, p99.99 100.0 -> 268.5; land over 100 m 0.01 % ->
  0.33 %. All 3,506,215 cells under 45 m identical to the bit; the rivers and the lakes' cells identical
  (`bake::tests::the_peaks_rise_and_the_lowland_stays_to_the_bit`, 768 and 1536 planets).
- Biomes: all land over 150 m is alpine (its top snow over 100 m, `data/biomes.toml`; with the old lapse 7 %
  of it was the ice biome, 66 % over 250 m); trees stop at the alpine line (58 m). The taller ranges move the
  climate a little: 2.0 % of the lowland's land cells changed biome (rain shadows), the spawn did not move.
- `report=1`: provinces and caves unchanged (one cave ball fewer); every mineral within 0.1 % of its blocks
  (copper 339,071 -> 339,033, iron 808,332 -> 808,324, gold 4,578 -> 4,575), bodies, depths and nearest
  distances the same; `made_hash` 8ecb5dadddbbf9b5 -> 3b563610bd1d358f.
- Stills: from the sea 0.8 km off the range (`a-far.png`: a snow range over the forest; before, nothing over
  the trees), the range from 400 m (`a-range.png`), from orbit (`a-orbit.png`: a branching chain where a grey
  smear was), from the summit (`a-summit.png`, `a-down.png`: a cloud sea under the summit, neighbouring peaks
  through it), seed 641653600's old highest spot (`b-old.png` inside cloud fog at 117 m, `a-old.png` above the
  cloud sea), the coast view unchanged to the pixel and the river view the same but for the water's glints.

Cost: `docs/perf-debt.md` (fly GPU +0.5 ms p50, frame p99 +1 ms; 48 % more chunks generated, the air up to
AMP).

Review (a fresh reviewer read the diff): (1) the height clamp moved with AMP and raised old worlds' highest
columns: the clamp now depends on the map's version; (2) the crack counter's "surely solid" sphere followed AMP
400 m down: it keeps the old 141 m; (3) the lakes were found on stretched depths, so high puddles became lakes
the old bake filled: the lakes are found before the stretch (the test now compares the lakes' cells); (4) the
bake's and the air's lapses disagreed above 150 m: both take `cool`; (5) the lake test's dry point: one of 273
lake points on the 1536 test planet reads no water although the column is open and generated water stands
there; a live water node answers for it and was not found to be wrong: the test allows 1 % (open);
(6) lowland biome changes: measured and written here; (7) `for_hash`'s second bake: written here; (8) the cost:
the perf-debt row; nits fixed (a test's doc comment, comments now false, an assert on the stretch's range).

## Survey of the baked landforms (same round)
Seeds 1337, 641653600 and 7 on the 4 km planet, version 6: orbit stills `out/tm/s-o<seed>-{a,b,c}.png` (the `b`
views are on the night side), the range from 400 m, the summit, the far view, and the heights of the maps.
The bake takes 39 s for the 4 km planet (seed 7, the stretch adds nothing measurable).
Below: what is weak, the change proposed, how much it would be exaggerated, and its risk. None of it is
implemented. Every change below either moves the lowland, the rivers or the biomes (so no longer
"lowland to the bit") or adds a new landform. That makes each one a taste or scope call, not a low-risk win.
- **Mountain chains:** good now. Arcs along plate edges, branching, rising out of the forest (`a-orbit`,
  `a-range`). Weak: snow begins at 100 m everywhere (`snow_over`), just over the tree line (58 m), so a range
  is forest, then snow, with no bare rock band and no change by latitude. Proposal: a snow line from
  temperature (the air's `cool`), and rock between trees and snow. Exaggeration: none, it is real logic.
  Risk: the biome data and every view that sees a range. *Taste call.*
- **River valleys and canyons:** rivers run in the old lowland; the stretch lifts the ridges but cuts no valley
  into a range, so there are no gorges. Proposal: incise valleys by discharge where the ground was stretched.
  Exaggeration: 2-3x real depth/width ratio. Risk: changes rivers' beds and lakes; needs its own river tests.
  *Scope call.*
- **Deltas and waterfalls:** not seen in the stills; the 45-60 m band is now steeper, so falls may appear
  where a river leaves a range (not checked).
- **Coasts:** almost all coasts are forest down to a thin beach. 19 % of seed 1337's land is under 5 m. There are
  no cliffs, even where a range meets the sea (`a-range`). Proposal: sea cliffs where upland or a range meets
  the coast, and fjords on cold, high coasts. Exaggeration: cliffs 10-30 m. *Taste call (a new landform).*
- **Plains vs hills:** half the land is under 14 m (seed 1337; 20 m on 641653600). Mostly plains with low
  hills; it reads well against the new ranges. No change proposed.
- **Plateaus and mesas, volcano cones:** none exist. Proposal: mesas in dry provinces, cones on the island arcs.
  *Taste calls (new landforms).*
- **Lake basins:** present and found before the stretch (unchanged). No change proposed.
- **Shelves and the deep sea:** the sea floor is 30-40 m almost everywhere (the 25th to 90th percentile of
  depth is 29.5-38 m), so the shelf and the abyss barely differ, and the ocean is now ten times shallower than the
  highest peak is tall. Proposal: deepen the floor beyond the shelf to 80-150 m. Risk: `world::FLOOR` and
  `SEA_DEPTH`, the water's volume, ores under the sea, views. *Scope call.*
- **Island arcs:** seeds 7 and 641653600 have long, thin, almost straight island chains across an ocean
  (`s-o7-c`, `s-o641653600-c`). They read as drawn lines. Proposal: break them into
  separate islands and volcanoes. *Taste call.*
- **Rain shadows:** the deserts lie beside seed 1337's range (`a-orbit`), and 2 % of the lowland's biomes moved
  with the taller ranges. Plausible; no change proposed.
- **Snow and tree lines:** see the chains above. The ice biome no longer shows on summits (the air's lapse).
- **Luna's craters:** not surveyed. The still (`s-luna.png`) landed on the night side. Luna's bake is
  unchanged.

## Consequences
- Old worlds keep their terrain; a new world on the same seed has taller mountains on the same lowland.
- Mountain slopes are up to about 4x steeper over the old 65 m line: more bare rock and gravel by the slope
  rules; the alpine biome over 58 m takes a little more ground (the stretch begins under it).
- AMP 400 adds about 260 layers of air to every body and widens ground-following ores' z range, so more chunks
  are generated; small bodies (radius under 400 m) never take `generate`'s quick "solid" exit.
- The fixed views that see the range (`orbit`, `high`, `lod`) change; the low ones change only where a biome
  moved with the climate.
