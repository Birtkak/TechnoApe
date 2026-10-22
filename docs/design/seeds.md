# World seeds

- **Date:** 2026-10-22
- Every world is fully determined by one 32-bit seed: terrain noises, hydrology
  tiles, trees, ruins, caves, ores all derive from it (`WorldGen.configure`).
  Same seed = same world, on every machine, which is also what the dedicated
  server and clients will rely on (ADR 0002: only deltas cross the wire).

## Using a seed
- Launch: `--path game -- seed=1337` or `seed="epic mountains"` (any text;
  non-numbers are hashed with FNV-1a 32-bit, the same way Minecraft hashes
  text seeds, in `Settings.seed_from_string`).
- The HUD shows the numeric seed and the text it came from.
- Saves are keyed by seed and generator version:
  `user://worlds/world_<seed>_g<GEN_VERSION>.sqlite`. Changing the generator
  bumps `TerrainGenerator.GEN_VERSION` so old saves are never mixed into a
  new world.
- Probe a seed without the game:
  `--headless --path game --script res://tools/gen_probe.gd -- seed=42 radius=2048`
  prints biome mix, heights, nearest spring/river/lake and writes
  `game/out/map_<seed>.png` (2 km across at radius 2048).

## Interesting seeds (4 km × 4 km sample around the origin)
| Seed | Character |
|---|---|
| `1337` (default) | Coast with a big bay, forests and highlands inland, a snowy range to the north-east, many lakes. Spawn on plains by the sea. |
| `epic mountains` (4065395920) | Despite the name: gentle, forest-heavy lowlands, little alpine terrain near spawn. Good for a calm start. |
| `42` | Dry: 10 % desert, more tundra and highlands, only 24 % ocean, long rivers. Spawn lands next to a river. |

Add seeds here when you find good ones; the probe makes it cheap to check.
