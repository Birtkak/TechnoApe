# NNNN – Pressure as a field, and rock and minerals that form by pressure and heat

- **Status:** proposed (the content is a draft for the developer)
- **Date:** 2026-11-18

## Context
`deep-world.md` makes pressure part of the model (the developer, 2026-11-18); one of its five effects is which
rocks and ores form at which depth. Before this step a mantle world (ADR 0154) had one rock, peridotite, under a
ruler-straight line at exactly 192 m, and nothing in it. Heat already existed (`Body::heat`, ADR 0063); pressure did
not.

## Options considered
1. **Pressure from the real column under each cell for everything, rocks included.** Honest without isostasy, but a
   boundary then follows the hills above it at about 80 % of their height: under a 350 m peak every mantle boundary
   would rise about 290 m and the upper mantle would almost vanish, and the crust's floor would rise into the crust's
   ore and caves. The variation would be the topography's, not the heat's.
2. **Pressure as a field (the real column) for the field's readers, and the rocks formed by the column at sea
   level (isostasy).** Real mountains float on roots, so deep down the pressure at a radius is about the same under
   hill and sea. The rocks' boundaries then move with heat (province and climate), by tens of metres, and with a slow
   wobble, as asked. Chosen.
3. **A per-cell iterative solve of the boundary depth.** Unneeded: the rule is asked per cell as "has this rock
   formed here", which needs no solve; a column crosses each boundary once because the data's slopes are bounded.

## Decision
**The pressure law.** `Body::pressure(face, i, j, z) -> Option<f32>` (beside `Body::heat`, None without a map), MPa:
one atmosphere (0.101325) at the top, plus fresh water (1000 kg/m3, at the surface g) from the water's top over the
column (`water_at`: the sea, a lake, a river) down to the ground, plus the rock from the ground down to the cell:
the crust's density above R − 192 m, the mantle's under it, with the gravity inside the body of ADR 0152 integrated
in closed form (`world::frames::pull_integral`: an antiderivative of each piece of `pull_inside`, joined). In the
game's own metres and its doubled gravity (19.62 m/s2): about 0.053 MPa a metre in the crust, 0.065 to 0.073 in the
mantle. Under the 4 km planet's spawn (seed 1): 10.35 MPa at the crust's floor, 81.7 MPa at the mantle's floor
(the brief guessed about 40; the real number is twice that because g is twice Earth's). Generated only: digging does
not change it. The planet's surface g is used on every body (a shortcut until Luna's deep is generated).

**Rock that forms by pressure and heat.** `data/rocks.toml`: `density` (the crust's, 2700), and a `[mantle]` table
with `density` (3300), `wobble` (metres) and `[[mantle.rock]]` rows top down, each `block`, `pressure` (MPa),
`heat` (C) and `slope` (MPa a degree). A row takes over, going down, where the column's pressure reaches
`pressure + slope · (T − heat)`. The first row ends the crust. Checks: pressures rising row by row, 0..1000 MPa,
slope within ±0.04 MPa a degree (a province warms at most 1 C a metre and the pressure grows at least 0.053 MPa a
metre, so a column crosses each boundary once), heat −50..2000 C, densities 1000..20000, wobble 0..20 m.
`Body::rock` (still the one rule the mesher's fill, `generated_under`, physics and the ledger ask, ADR 0062) asks it
under R − 192 m + 64 m (`MANTLE_LIFT`) on a body with a mantle. The pressure there is the sea-level column's
(`rock_weight(R − 4 m, r)`), the heat the province's gradient under the column's year air (`Rock::air`, filled with
`Rock::wobble` by `rock_at` on a mantle body only), and each boundary moves by the wobble: the fold noise at a 600 m
width, ±4 m. So a boundary steps where a province does, as `Body::heat` already does.

**Minerals with a window.** `data/minerals.toml`: optional `pressure = [min, max]` (MPa) and `heat = [min, max]`
(C). A body is placed only where `Body::pressure` and `Body::heat` at its middle's column and layer lie inside both;
a host rock is then checked by `Body::rock` (the mantle's rocks), not the crust's layers alone. Such a kind may lie
1400 m deep, on a planet with a mantle only, is not cut at the crust's old floor (`OreBody::span`), and has no halo,
gossan or gravel. The kinds with a window come last in the file, and the gossan's kind is placed before them, so the
crust's kinds keep their indices and every body they place is the one placed before, in the same order; the deep
kinds' bodies come after. The ledger (`ore::count`, `open_ledger`) counts them as any body.

**Content (drafts for the developer):**

| Rock | Block, rubble | Where (data) | Look | Hardness, grade |
|---|---|---|---|---|
| peridotite (upper mantle) | 58, 191 (exists) | 10.07 MPa at 42 C, slope −0.04: the crust's floor, thinner under hot rifts | dark olive | 10, 1 |
| ringwoodite (transition zone) | 59, 192 | 33.0 MPa at 95 C, slope +0.03 (a real olivine-to-spinel slope is positive) | deep blue | 12, 2 |
| bridgmanite (lower mantle) | 60, 193 | 56.7 MPa at 148 C, slope −0.03 (the real post-spinel slope is negative) | dark brown | 15, 3 (`later`: no iron pick exists yet, so the lower mantle cannot be dug today; a taste call) |
| diamond (ore) | 61, 194 | 8 lumps of 60 blocks, 560–900 m down, 36–56 MPa and 0–110 C, host ringwoodite | ringwoodite with white crystals | 13, 2 |

Diamond really forms at high pressure where it is comparatively cool, and ringwoodite was first found in nature as
an inclusion in a super-deep diamond. Its element is "diamond"; ingots in the report are gems by the ore grade
rule. No item, recipe or use beyond its rubble (ores drop their rubble today). Textures: `tools/paint_textures.py`,
listed in `data/drafts.txt`; the Studio's `check=1` passes.

**What it gives on seed 1, the 4 km planet** (`technoape-server report=1`, new field `deep`): under the spawn
(basin) the crust's floor 192.9 m down (43.0 C, 10.35 MPa), ringwoodite at 544.9 m (95.8 C, 33.35 MPa),
bridgmanite at 898.0 m (148.7 C, 57.06 MPa), the floor at 1246.2 m (201 C, 81.73 MPa): roughly thirds. Under the
nearest rift: 166.9 / 599.1 / 826.0 m; under the nearest shield: 176.1 / 516.7 / 888.2 m (that column lies under
15.6 m of water, so its depths are from a lower ground). Diamond: 8 bodies, 481 blocks, 595.6–695.0 m down, the
nearest 985 m from the spawn.

**Old worlds.** A world without a mantle (`mantle=0`, a save of version 19 or older, small planets, Luna) generates
as before: `Rock` gets its new fields only on a mantle body and `Body::rock` asks the mantle only there; `made_hash`
for `mantle=0` is `0926f028ddcd85a7` before and after. On a mantle world the crust's bodies are unchanged: the new
`made_hash_crust` (caves and the crust's kinds' bodies) is `9b209f3e64fcf265`, main's whole `made_hash`; the whole
hash is now `f283474fb9f15012` with the diamonds. The crust's rock changes only within the band where its floor now
waves (about 166 to 200 m down; never above 128 m, `MANTLE_LIFT`) and the mantle under it.
**Saves of version 20 made with the mantle (the last day) change under their owners' feet** where they have not
been dug: their mantle's rocks, the crust's floor and the diamond bodies appended to the ledger. Accepted (nobody
has one worth keeping); the save format did not change, so no new version.

## Checks

| Check | Result |
|---|---|
| `rock_weight` against the world's gravity summed every centimetre, 100/192/700/1248 m | 5.187 / 10.066 / 43.414 / 81.60 MPa, within 0.01 % (red: uniform g, 42.95 against 43.41 at 700 m) |
| pressure 1 atm at a land column's ground, rising every layer to the floor, a sea floor's = 1 atm + its water | pass (red: without the atmosphere, 0 at the ground) |
| a boundary's shift between rift and shield is its slope times their heat over the pressure's growth | peridotite 169.5/195.3 m, ringwoodite 602.6/536.0, bridgmanite 828.7/907.4 (red: heat ignored) |
| rock and mineral rows with a bad window or order refused | pass (red: the checks removed) |
| diamond only in its window, no rift or arc, none in the crust, cells held, ledger counts each body | pass (red: no window check, a diamond at 134 C; the old span cut, no cells) |
| the readers agree to the mantle's floor; the surface the same with and without the mantle | pass |
| `made_hash` `mantle=0`; `made_hash_crust` on a mantle world | unchanged |
| Studio `check=1` | ok |

## Consequences
- The field exists for the next steps (air, liquids, collapse, suits); none reads it yet.
- A boundary steps at a province border, as heat does; a smoothed heat field would soften it.
- The crust's floor is no longer one radius: anything that assumed exactly 192 m for rock now asks `Body::rock`.
- Not done: pressure changed by digging, Luna's own g, caves, kimberlite pipes, magma, any use of diamond.
- Cost (`docs/perf-debt.md`, one timing in the test profile): `Body::rock` 18 ns a mantle cell against 2 ns for
  the crust's layers, about +0.6 ms for a deep chunk generated cell by cell; untouched deep chunks are still
  `Gen::Solid`. Cheaper: the column's boundary radii once per tile.
- Seen: a room dug 3 m under each boundary under the spawn (`run=stand depth=196|548|901`, lamps, a sample block
  of each rock on the floor) shows the upper rock over the lower in a level line across a 9 m wall.
