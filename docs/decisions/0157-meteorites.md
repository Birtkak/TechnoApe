# ADR 0157: Meteorites

Status: accepted (2026-11-07). Runtime only (`scripts/world/meteors.gd`); no generator change, GEN_VERSION stays.
Materials 147 `meteoric_iron`, 148 `scorched_earth`; atlas tiles 186, 187.

## Context
Fun pass (docs/plans/fun-pass.md): rare night events that send the player somewhere, and an early, rare road to
iron before the bloomery (ADR 0070). The world is drawn only inside the render box (ADR 0073), so a fall a few
hundred metres off must be told by the sky and sound, and its crater made when the spot comes near.

## Decision
- **The roll** is per night, from the world seed: `Meteors.fall_time(seed, day, chance)` gives a time between
  20:24 and 03:36 or none (`Settings.meteor_chance_per_night` 0.12, about one a week). A fall fires when the
  calendar crosses that time in the dark; a sleep or time jump skips it.
- **The spot** comes from the seed and the fall's time (`spot_for`), `meteor_min_m`..`meteor_max_m` (250-600 m)
  from the player, and is saved (`meteors_<seed>_g<gen>.json`: time, spot, per-fall seed, applied, cooled, core).
- **The sky:** a streak (an additive box drawn 40 m from the camera, inside the far plane) runs 3 s from high
  in the sky down to the horizon at the spot's bearing; a flat boom follows after distance / 343 m/s; an orange
  glow column (`MonolithBeacon` with height, radius and colour) stands over the spot for `meteor_glow_h` game
  hours, fading (a white flash as it lands), until the crater is in the drawn box. The streak tapers from a
  flickering blue-white head to an ember tail. A chat line gives the distance and compass direction; a
  unique `meteorite` waypoint goes on the map.
- **The crater** is a runtime edit made once, when the spot's ground column, the bowl depth and the fire reach
  on four sides are loaded in the sim window: a 3.5 m bowl (`SimBubble.blast`), a 1 m shell of scorched earth
  over soil and snow, the plants around cleared, a 0.6 m ball of meteoric iron at the bottom and five chunks in
  the walls. While `meteor_hot_h` (24 game hours) lasts: lava over the core, smoke, `meteor_heat_damage` per half
  second near the core, smoke and sparks, a flickering orange light (20 m) over the core while in the box, and half the falls set burnable solids within 5 m past the bowl alight. Later the lava
  turns to basalt (once its spot is loaded); an arrival after the hot hours finds it cold. The edit goes through
  the bubble, so the terrain save keeps it and other players get it (ADR 0145).
- **Meteoric iron** digs with a copper pick (tier 1) and smelts in the plain furnace straight to an iron ingot
  (one block's worth per ingot): no bloomery, no bloom. The progression walk prices it as an event source,
  only where nothing else gives it (`EVENT_ONLY`, `EVENT_S` 4800 s per crater of ~6 blocks).
- `SimBubble.blast` with an `only` list now keeps air unless `only` names it (its doc said "everything else
  stays"; air was always filled). Every caller before blasted to air or listed air.
- `/meteor [here|near|stats]`: 6 m ahead, 120 m ahead, or the usual distance; JSON `meteors`.

## Consequences
- The crater shows only once its spot is inside the 64 m sim window, close to the edge of the drawn box.
- A far diagonal chunk can stay unloaded at short view distances; the blast skips chunks still loading, so the
  crater's farthest fire corner may miss there.
- Lava pooled in the bowl lights little at night; the core reads by its smoke more than by its glow.
- No falling pieces of the meteor itself, no damage at the impact (it lands far away or where you aimed it).
