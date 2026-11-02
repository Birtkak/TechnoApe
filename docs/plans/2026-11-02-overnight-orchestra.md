# Overnight orchestra (2026-11-02)

The developer asked for a big leap in content across the board, with progression capped at the
electrical age (first electric tier, LV) and wands shelved. The overseer session briefs, judges,
merges into local `main` and bumps `GEN_VERSION`. Nothing is pushed.

## Lanes

| Lane | Session | Owns | ADR | Material ids | Atlas tiles |
|---|---|---|---|---|---|
| Electrical age | phase3 (strong model) | `scripts/work/*`, networks, machine items/recipes | 0117, 0122, 0123 | 240..259 | 208..215 |
| Liquid overhaul, then perf hunt | caves / "Fable Agent" | liquid sim, its mesh/shader, `sim_world.cpp` | 0119, 0124 | 280..289 | ask |
| All world generation | surface-gen | `fine_gen.cpp`, `world_gen.cpp`, ruins, biomes | 0118, 0125 | 260..279 | 192..207 |
| Life and ambience | smoke-sound-critters | mobs, audio, survival effects, particles | 0120, 0126 | none | none |
| Things to make | content-pass | food, furniture, decor, tool/armour tiers, farming | 0121, 0127 | 216..239 | 180..191 |
| Progression | recipe-browser | progression book, quests, recipe browser, old failing tests | 0115 | none | none |
| Open Oct 28 reports | playtest-tools | player, survival, equipment, stations | 0116, 0128 | ask | none |

ADR 0114 is unused (job 6, wands, was dropped).

## Main

- Wave 1 merged (caves ADR 0109, surface pass, smoke/reverb/dart frogs, planks and saplings ADR 0112),
  `GEN_VERSION` 27, machines ADR 0113 merged.
- Full suite after the machines merge: 608 cases, 6 failures. Five were already failing before the night
  (brush dig cap, crafting reachability of `miners_helmet`, fire clay mold, quest graph); one is the cave
  pool sheets (sent back to the cave session).

## Merge log

(appended as branches land)

## Ideas parked for the developer

- "Veins of the world" (cave session): a hidden world-scale network of aquifers, gas pockets and magma
  conduits you can hear. The static-generation part went to the world generation lane; anything simulated
  waits for the liquid ADR.
