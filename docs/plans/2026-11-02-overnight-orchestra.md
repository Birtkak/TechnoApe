# Overnight orchestra (2026-11-02)

The developer asked for a big leap in content across the board, with progression capped at the
electrical age (first electric tier, LV) and wands shelved. The overseer session briefs, judges,
merges into local `main` and bumps `GEN_VERSION`. Nothing is pushed.

## Lanes

| Lane | Session | Owns | ADR | Material ids | Atlas tiles |
|---|---|---|---|---|---|
| Electrical age | phase3 (strong model) | `scripts/work/*`, networks, machine items/recipes | 0117, 0122, 0123 | 244..253 | 208..215 |
| Liquids and gases (colours, state changes by pressure and temperature, readout under the minimap), then perf hunt | caves / "Fable Agent" | fluid sim, its mesh/shader, `sim_world.cpp` | 0119, 0124 | 254..255 | ask |
| All world generation | surface-gen | `fine_gen.cpp`, `world_gen.cpp`, ruins, biomes | 0118, 0125 | 228..243 | 192..207 |
| Life and ambience | smoke-sound-critters | mobs, audio, survival effects, particles | 0120, 0126 | none | none |
| Things to make | content-pass | food, furniture, decor, tool/armour tiers, farming | 0121, 0127 | 216..227 | 180..191 |
| Progression | recipe-browser | progression book, quests, recipe browser, old failing tests | 0115 | none | none |
| Open Oct 28 reports | playtest-tools | player, survival, equipment, stations | 0116, 0128 | ask | none |
| Art pass | art-pass | restyle atlas, models, sprites, UI skin (no new ids) | 0129, 0131 | none | none |
| Quality of life | qol | HUD, menus, controls, chat, tooltips, inventory interaction | 0130, 0132 | none | none |

ADR 0114 is unused (job 6, wands, was dropped). Material ids stop at 255: `MAX_MATERIALS` is 256 and
liquid levels start at 256 (`FLUID_LEVEL_BASE`), so only 216..255 were free; the liquid lane may move that
encoding up in its ADR.

## Main

- Wave 1 merged (caves ADR 0109, surface pass, smoke/reverb/dart frogs, planks and saplings ADR 0112),
  `GEN_VERSION` 27, machines ADR 0113 merged.
- Full suite after the machines merge: 608 cases, 6 failures. Five were already failing before the night
  (brush dig cap, crafting reachability of `miners_helmet`, fire clay mold, quest graph); one is the cave
  pool sheets (sent back to the cave session).

## Merge log

- `f6ef52c` recipe browser: machine recipes as data, saved bookmarks (ADR 0115); the four old failures fixed
  (crystal counted as raw, fire clay tagged clay, quest count only grows, dig caps per the 2026-10-27 call).

## Ideas parked for the developer

- "Veins of the world" (cave session): a hidden world-scale network of aquifers, gas pockets and magma
  conduits you can hear. The static-generation part went to the world generation lane; anything simulated
  waits for the liquid ADR.
