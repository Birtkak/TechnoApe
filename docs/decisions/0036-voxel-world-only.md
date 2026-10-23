# 0036 – The voxel world only: retire the 0.5 m era, load before play, voxel palettes

- **Status:** accepted 2026-10-23 (the user's direction; the details are routine calls recorded here)
- **Date:** 2026-10-23

## Context
After ADR 0033-0035 the game ran on one 12.5 cm grid, but the 0.5 m era was still in the code and
the docs: WorldGen's own block generator (only tests and probes used it), the chisel, near-field,
brick-store and micro-sim spikes, probes built on the old generator, the 0.5 m distances in the
Settings screen (48/64/96 m full detail) and the F4 "terrain detail" option. A playtest bug report
showed what that costs: the exe read a settings file holding 96 m full detail and 64 m rings, the
12.5 cm terrain never finished loading (6.2 GB, 26 k triangles drawn: grey sky over grey water),
and the 15 s loading-screen cap dropped the player into it. The user: "remove all deprecated old
systems so we don't choke on them anymore, the voxel life is the future and we lean into it
fully"; "generating can take a long time, but when you spawn in it should be quick so you can't be
killed before seeing what happens; same with loading a save or respawning"; "make all the
textures fit the new vibe as well".

## Decision
**Removed:** `WorldGen.generate_block` and what only it used (`canopy_radius`, `trunk_top`,
`stamp_height`, `stamp_voxel`, `vein_ore`, two noises): WorldGen is the macro layer and FineGen
the only block generator. Prototypes p1, p2, p6, p7, p9, p10 (their READMEs live in git history;
p5 holds the native source, p11 the godot_voxel patches). Tools `gen_bench`, `gen_probe`,
`detail_probe`, `chisel_bench`, `settle_probe`, `wild_crop_probe`, `vein_probe` (replaced by
`terrain_map`, `fine_gen_bench`, `fine_probe`, `face_probe`). `docs/design/brick-world.md` and
fable-backlog jobs 1-4. The ultra preset and F4 terrain detail. ADR 0004, 0006, 0012, 0021 and
0029 are marked superseded. Material id 44 stays reserved (ids are append-only).

**Ported, not lost:** the biome/content tests run on FineGen through `test/fine_macro.gd` (FineGen
seen in macro coordinates); `test_fine_gen_lod.gd` replaces the old LOD test with FineGen's rules.
Porting found two real gaps: `FineGen.find_nearest` returned the surface for caves (so `/goto`
to a cave dropped you on top of it), and cave pools (water in lush caves, toxic sludge in
sulfur caves) had never been ported to 12.5 cm. Both fixed.

**Distances:** full detail and LOD rings are 16 or 24 m (`Settings.MIN_DETAIL_M`,
`MAX_DETAIL_M`); a saved file is clamped on load. Measured in a forest: 24/24 m p99 6.8 ms;
24 m with 32 m rings 9.4 ms; 32/32 m 21 ms and 3.7 GB.

**Loading before play:** `Main.cover_until_loaded` puts the loading screen up at the start (new
world or save), after a respawn and after a teleport beyond 24 m (`Player.moved_far`). Under it
the player and the animals are frozen, godot_voxel runs on all but two cores and mesh uploads are
unthrottled; it lifts when no voxel task is pending and the bubble holds its chunks (cap 120 s,
only against a hang). Autopilot runs keep the first cover and freeze nothing (their scripts act
from the start). Startup itself (title to cover) is 0.9 s (`startup_ms` in the JSON); the world
around a new spawn takes ~13 s.

**Textures:** a terrain voxel shows one texel of its tile, so the tile is a palette.
`make_atlas.py` `palette()` builds ~30 terrain tiles from hand-picked colours and weights (natural,
a little warm, a few close shades), shuffled: ores are mostly ore colour, grass voxels are green
on every side, tall grass is lighter than the ground, water is teal. Props (`terrain_prop.gdshader`:
stations, chests, held and dropped blocks) show their tile in 8 x 8 flat cells averaged from the
atlas's first mip, the 6.25 cm grain of the micro models, instead of 16 x 16 pixel art. Item icons
in the inventory stay pixel art (UI).

## Consequences
- Less code to keep in step (world_gen.cpp 1680 -> 1205 lines, ~580 files and ~57 MB of
  prototypes gone); every generator test exercises the generator the game runs.
- A respawn or a long teleport costs a few seconds of loading screen; nothing can happen to the
  player meanwhile.
- No graphics setting can ask for more detail than the world holds; more detail needs cheaper
  forests first (ADR 0035's levers).
- The palettes are a first pass: their colours are a taste call for the user's playtest.
