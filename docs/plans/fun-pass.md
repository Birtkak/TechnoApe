# Fun pass (2026-11-07)

Brainstorm with the developer: make the game more fun. Hazards do **real damage**, each with a warning
sign and a counter. Run as overseer batches (`.claude/skills/overseer`), as many sessions as needed (developer, 2026-11-07),
grouped by the files they touch. Numbers are handed out here so merges need no renumbering.

`GEN_VERSION`: 39 = underground hazards; 40 = geysers + wrecks. Nobody else bumps it.
Material ids come from the 133..159 reserve in `game/scripts/materials.gd` (append only).

| Id | Feature | ADR | Materials | Wave | Waits for | Status |
|---|---|---|---|---|---|---|
| A | Underground hazards: firedamp, safety lamp, vent fan, aquifer floods | 0148 | 133-136 | 1 | - | merged 8fd9ef8, fixes 990a70b |
| B | Surface hazards: lightning + rod, volcano eruptions | 0149 | 139-140 | 1 | - | merged 5f7d1ea, fixes df8dfcc |
| D | Movement: glider, climbing rope, grappling hook | 0151 | 141-142 | 1 | - | merged b990043, fixes 96f7563 |
| E | Tamed wolf companion | 0152 | - | 1 | - | merged 04ff601, fixes 2c4bcbc |
| C | Fuse cord (burns along the ground) + flung blast debris | 0150 | 137-138 | 2 | - | merged f4e67f6, fixes 47fa4ed, 8c2800e |
| F | Machines visibly working (steam, smoke, light, moving parts, sound) | 0153 | - | 2 | - | merged 2bcd8f8, fixes 311dec5 |
| N | Blueprints: copy a build, a builder places it from a chest | 0161 | - | 2 | - | merged 4d9e6a2, fixes 6fb7a98 |
| P | Signs, notes, player trading post (multiplayer-aware) | 0164 | - | 2 | - | merged 6f1af3e, fixes 6d7e532, d6be5eb |
| G | Water wheels and dams; weak dams break and flood | 0154 | 143-144 | 3 | F | merged 44f014f |
| J | Meteorites: night streak, crater, meteoric iron | 0157 | 147-148 | 3 | - | merged 02604e7, fixes c310b52 |
| O1 | Food preservation (smokehouse, salting, cellar) + home comfort buff | 0162 | - | 3 | - | merged 01df654, fixes ae96af7 |
| KL | Geysers (launch, steam source) + underwater wrecks with diving bell / air | 0158 | 149-152 | 3 | A, D | ready (needs a go) |
| H | Pollution consequences: fish die, crops wilt, smoke makes you cough | 0155 | - | 4 | E | merged 5e8f85a, fixes 5ccc12e |
| I | Seasons: frozen rivers, snow, drought and fire risk, farming rhythm | 0156 | 145-146 | 4 | B, H | merged dc4c025, fixes 1217c25 |
| O2 | Fishing rod + bite minigame, snares and pit traps | 0163 | - | 4 | D, E | merged 5b9d287, fixes 8f074f8 |
| M | Moving contraptions: lifts, drawbridges, cranes from rigid pieces | 0160 | - | 4 | C | merged 350ffc4, fixes ea25dd2 |

ADR 0159 is spare. Not chosen this round: trap-puzzle ruins, a long-term capstone goal, landslides,
steam locomotive, crafted maps, photo mode.
