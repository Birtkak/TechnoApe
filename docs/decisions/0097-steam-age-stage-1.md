# ADR 0097: The steam age, stage 1: boiler, pipes, steam hammer, steam sieve

Status: accepted (2026-10-27)

## Context
ADR 0096 set three eras and a three-stage steam age. The developer asked to build it and chose the
flavour (2026-10-27): a riveted iron drum on a brick firebox, short bronze pipe runs from the start,
a warning whistle before a burst that leaves bent plates and soot (no crater), pressure as a visible
number that speeds machines up, rust that an iron plate mends, and goggles and a leather apron.

## Decision
1. **Parts.** `iron_plate` (one iron ingot beaten flat on the bronze anvil), `iron_rod` (two per ingot),
   `bronze_pipe` (a bronze ingot, any anvil). Smith recipes, so stage 1 still starts by hand.
2. **The small boiler** (`BoilerEntity`, a `FirepitEntity` subclass like the forge; 3 plates, a pipe, 3
   clay bricks): coal or charcoal under it, a water bucket fills a quarter of the tank, a firestarter
   lights it. Lit with water: `pressure` climbs to 1.0 in `boiler_build_s`, the tank boils away in
   `boiler_boil_s`. Over 1.0 the whistle screams (and smoke puffs); at `boiler_burst_at` (1.3) or after
   `boiler_dry_s` lit and dry it **bursts**: the piece goes, 1-3 iron plates fly out, a ball of steam
   rises, ash and a few fires dust the ground round it, a player within `boiler_blast_m` takes
   `boiler_blast_damage`, halved per worn piece with `clothing.steam_proof` (goggles, apron). An empty
   hand vents half the pressure (a hiss).
3. **Pipes** (`PipeEntity`, a flat cross on the ground) carry steam: once a second `BoilerEntity.feed`
   walks from every boiler over pipes on the same level, at most `steam_pipe_max` long, and hands the
   boiler to the machines at the ends (`boiler` property). Machines draw `steam_per_use` per stroke and
   run faster above working pressure, slower below.
4. **The steam hammer** (`SteamHammerEntity`, an `AnvilEntity` of tier 1): worked like the bronze anvil,
   but with steam it strikes by itself: each strike drives one extra cell toward the nearest column
   short of the shape, or cuts it when none wants it. The finished piece waits for an empty hand.
5. **The steam sieve** (`SteamSieveEntity`): holds `steam_sieve_loads` blocks of gravel or sand and sifts
   one per shake with the sieve's loot table; an empty hand takes what it found.
6. **Rust:** the boiler rusts while lit, the hammer and the sieve per stroke (`wear`, 0..1 over
   `boiler_wear_s` of running); steam falls with the boiler's wear, a machine at 1 stops; a right click
   with an iron plate mends any of them.
7. **Wearables:** brass goggles (head) and a leather apron (body), small armor, `steam_proof`.
8. `/kit steam`; sounds `steam/whistle` and `steam/hiss` reuse the steam loop asset.
9. **Sizes** (developer: "models can be all kinds of sizes"): `BlockEntities.SCALE` replaces the fixed
   `WIDE` list with blocks per kind: the 1 m stations are 2, the steam hammer 3 (1.5 m wide, 3 m tall:
   the anvil under a frame, cylinder and head), the boiler 5 (2.5 m, drawn at one cell per voxel with
   its own `grid`). Pipes stay one 0.5 m block, a cross of round tubes.
10. **Alive:** the hammer's head drops for four ticks on every blow (`struck`: the anvil clang, sparks, a
    puff of spent steam), the sieve's heap shifts per shake (`shook`: the rattle, a spray of the input),
    a lit boiler smokes from its chimney and hisses a wisp at the outlet under pressure, and screams
    with thick smoke when dry or over pressure.

## Consequences
- `test_steam_age.gd` covers the data, pressure, both bursts, the pipe walk and its limit, the
  automatic smithing, the sieve and saves. Numbers in `settings.gd` are first guesses.
- Not built: a scripted in-game check like `metal_age_test`, pipes that climb, the steam-age book
  chapter, stage 2 (steel, the blast furnace, the crusher, tanks).
- Steam is a number inside the boiler, not simulated gas; only the burst makes real steam cells.
