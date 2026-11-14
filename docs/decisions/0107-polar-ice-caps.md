# 0107 – Polar ice caps: a colder count for ice toward the poles, sea and lake ice as a sheet

- **Status:** proposed (the rule and its numbers are Claude's, open to taste)
- **Date:** 2026-11-14

## Context
Bug 2026-11-14_02-03-05: "How come there are no polar ice caps?" The bake (ADR 0042, 0055, 0060) has an `ice`
biome for land whose year's mean (jittered by the 40 m blend noise) is under -13 C, and nothing for the sea.
What the bake of the developer's world (seed 933105400, planet 6144) makes, by latitude (`LIFEDBG=1`, the
year's mean air at the ground, p10 / p50 / p90):

| latitude | land | sea | ice before |
|---|---|---|---|
| 50-60 | -3.6 / 1.4 / 5.6 | 1.1 / 4.5 / 8.7 | 1 % of land (high ground) |
| 60-70 | -7.5 / -5.3 / -3.1 | -0.5 / -0.1 / 0.4 | 1.5 % of land |
| 70-80 | -6.9 / -5.4 / -4.1 | 0.1 / 0.5 / 0.8 | none |
| 80-90 | -7.1 / -5.7 / -5.0 | 0.6 / 0.8 / 0.8 | none |

The air model's poles are a few degrees under 0 and flat from 60 degrees on: its sea keeps them mild and it has
no ice whose brightness would cool them (real poles: -18 C in the Arctic, -50 C on Antarctica). So no land
reached -13 C, and the sea is never cold. A small planet (768) is warmer still: its air columns are few and
its poles near 10 C.

## Options considered
1. **Count the year's mean colder toward the pole, for ice only** (chosen) – keep the air model; for the
   ice test only, the mean counts up to `polar_k` = 20 K colder at the pole, nothing equatorward of 55
   degrees, linear between. Stands for what the model misses (the long polar night, the ice's own albedo).
   The edge still follows the climate: land colder than sea, high ground colder (the lapse), the climate grid's
   wobble and the 40 m blend noise. Small change, one place.
2. Retune the air model so its poles are cold – the real fix, but it moves every climate, rain and biome on
   the planet (ADR 0060's tuning, the weather) and is a much larger change.
3. A latitude line with noise – simplest, but ignores the climate, as the developer asked not to.

## Decision
**Bake** (`bake/life.rs`, stage 8): `cold = tt - polar_k * max(0, (lat - polar_from) / (90 - polar_from))`;
land is `ice` where `cold < t_ice` (-13 C, as before), the sea is `ice` (the same biome row, reused) where
`cold < t_sea_ice` (-8 C: the sea air is some 6 K milder than the land's at the same latitude, so the sea's cap
ends near the land's). On the developer's world: ice on 25 % of the land and 12 % of the sea at 60-70
degrees (the ragged edge), 98-100 % poleward of 70; about 6.5 % of the planet. `shares` keeps sea ice out of
the land's shares. The sea floor under sea ice takes the ice biome's roughness (1.1 for 1.0; left so, as
`rough` runs on old maps too and must not move their ground).

**Ground** (`data/biomes.toml`): the ice biome's top is now snow, its band ice, with 20 % patches of bare ice
(an ice sheet under its snow, blue ice where the wind scours it); before, bare ice on snow.

**Sea and lake ice** (`world.rs`, `Body::ice_sheet`): where a level-0 column's biome (jittered as the ground's) is
ice and water that is not a river stands over its ground, one block fills the layer the water's surface lies
in (the sea's surface is 7/8 into its layer, so the sheet stands an eighth of a block proud, with open water
under it). The block is the ice biome's top, as on land: snow (sea ice under its snow), patches of bare ice.
A first try with bare ice everywhere read as pale blue sea from orbit, hardly a cap. It is laid through `plants_with`, the one function the mesher, physics and the water
read blocks over the ground through, so all three see it. A cheap test first (no ice cell within the jitter's
reach) keeps it off every other column. Far tiles (levels 1+): a coarse column over a frozen sea or lake
stands at the sheet's top, in ice, as the far canopy does (ADR 0100).

**Globe map** (`client/src/globe.rs`): ice cells are drawn in the ice colour over land, lake and sea alike.

**Version** (`bake::VERSION` 6 to 7): the same layout and ground as 6, only biomes differ. `Map::bake_v6`
bakes a 6 again (the rules before, `LifeParams::capless`), `for_hash` tries it, `find` and `from_bytes` read 6,
and the peaks' `AMP` holds for 6 and 7; `bake_v5` and the older layouts derive from the capless rules too.

## Consequences
- **A new world** on any seed bakes version 7 once (about 40 s for the 4 km planet) and has caps.
- **An existing save keeps its map**: a save names its map by hash, so `world-1.sav` (seed 933105400) carries on
  on its version 6 map from `saves/bakes/` (or baked back by `bake_v6` if the file is gone; checked: a copy of
  `world-1.sav` with the map file moved aside bakes 7, then 6, byte for byte the old file, and loads): **no caps
  there**.
  One small change does reach it: the few `ice` cells a version 6 map has (high ground at 50-70 degrees) now wear
  snow over ice (`data/biomes.toml`). The sheet is only laid on maps of version 7 (`ice_sheet`'s gate), so an
  older map's icy lakes stay open. Moving `world-1` onto version 7 would need a one-off
  that rewrites the save's map hash (its edits are far from the poles); not done, the developer's call.
- Sea ice is solid: a boat or a swimmer stops at its edge; a player walks on it and digs through it to water.
- From orbit the caps are white, land and sea; the clouds over the poles hide some of it. A sheet of snow
  floating on water is a stand-in for snow on sea ice (one block); digging it gives snow, under it is water.
- Revisit: when the air model gets cold poles (ice albedo, polar night), `polar_k` goes to 0.
- Not verified: the tide under a sheet (ADR 0067). The water reads the sheet as a solid block through
  `plants_with`; at low tide a thin gap of air opens under it, at high tide the sea stands against its
  underside. No test runs the tide there.
- Far levels: the far sheet stands up to half a coarse cell over the sea (a few metres at the levels seen from
  orbit, more at the coarsest), a step at the cap's edge from far away, as the far canopy has.
- The sea's biome under the caps is `ice`: anything keyed on the biome (herds, the F3 line, rain per biome)
  sees ice over the sea there; a dry tidal band in it wears snow.

## Review (principle 12)
A fresh reviewer read the diff with the task. Fixed: the sheet had no version gate, so a version 5 or 6 map's
icy lakes (land `ice` existed before) would have grown solid blocks under an old save (now only on version 7;
tested with the same map relabelled 6); a `rough` arm for sea ice also changed old maps' lake beds in tundra
and ice (reverted); a lost space in a test. Answered, left: the tide under the sheet (above, not verified), the
far plateau (above), `plant_top_with` does not see the sheet (the far tiles lay it themselves), `bake_v6` costs
a save on version 5 without its map file one more bake, once.

Tests: `both_poles_carry_an_ice_cap_on_land_and_sea` (bake/mod.rs: on the standard planet of two seeds, ice on 4
to 14 % of the planet, over 90 % of it poleward of 55 degrees, over 80 % of each pole's last 10 degrees, on land
and sea; seen failing before the rule: 0.0 % ice in its first form on small planets, which then showed that
a small planet's air has no cold poles, so it moved to the standard planet, where version 6 has under 1 %) and `the_polar_sea_carries_a_sheet_of_ice_over_open_water`
(world.rs: on the developer's world's pole face, every sheet is water under the sheet (snow or ice) under air with the water's
surface inside it, no ice over air anywhere else, a sheet on over 5 % of the face's columns, and a far tile over
the frozen sea shows the sheet; written with `ice_sheet`, so not seen red). `every_version_a_save_names_is_baked_back_without_its_file`
covers version 6; it caught `from_bytes` refusing a version 6 map, which would have stopped `world-1.sav` loading.
Far tiles: a coarse cell is solid only when the ground reaches its middle, so a far sheet one block thick sank
under the sea's surface (the first orbit stills showed open sea); the far sheet now reaches at least the middle
of the coarse cell it lies in (up to half a coarse cell over the sea, as the far canopy stands proud too).

## Cost
`run=fly quick=1`, play build, one run (`bash tools/slot.sh measure`, the PC shared with other agents):

| | p50 | p99 |
|---|---|---|
| frame | 2.67 ms | 6.59 ms (max 13.2) |
| CPU (frame thread) | 2.50 ms (select 0.08, acquire 1.38) | 6.29 ms |
| GPU total | 2.48 ms (gbuffer 0.90, screen 1.09, trace 0.17) | 3.43 ms |
| mesh job | 588 us average, 408 us after load | |

The run is ADR 0106's, the same day, and far quieter (ADR 0106's 5.7 ms p50 was taken while other agents
used the GPU); the two are not a before and after. What the caps add: the bake's biome pass (a few float ops a
cell, inside the 40-50 s bake, lost in its noise); in tile builds, five map lookups a column (the cheap test),
and only in a cap's jitter reach the jittered biome, the water and the column's height (level 0, and once a far
column). The fly flies the spawn's latitudes, so it pays only the cheap test; a far tile over the caps pays the
rest. Nothing on the frame thread, nothing on the GPU (the sheet is ordinary blocks).
