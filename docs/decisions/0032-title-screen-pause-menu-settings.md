# 0032 – Title screen, pause menu, player settings and graphics presets

- **Status:** accepted
- **Date:** 2026-10-23

## Context
The game started straight in a world chosen by the command line, Esc only
freed the mouse, and every option lived in `settings.gd`, args or the F4
playtest menu. ADR 0031 found the GPU savings that cost picture quality and
asked whether to offer them. The developer answered: "make a main menu and
an esc menu when in game (pause game in singleplayer) with settings etc".

## Options considered
1. **Menus inside the game scene** (a world loads behind the title). The
   world would generate before the player chose one.
2. **A title scene that hands over to the game scene.** Chosen: the world
   loads only once it is picked, and the game scene stays what every
   benchmark and debug run already starts.
3. Graphics as one quality slider, or **presets plus the values they set**.
   Presets plus values: the four measured knobs stay visible and tunable.

## Decision
- **The title screen is the main scene** (`scenes/title.tscn`,
  `TitleScreen`): Singleplayer, Multiplayer (disabled until the dedicated
  server, phase 4), Settings, Quit, over a drawn sky of blocky hills.
  Runs with `autopilot=1` or `play=1` skip it and enter the game scene with
  the command line's world, so ADR 0014's benchmarks are unchanged.
- **Worlds** (`WorldList`, `user://worlds/index.json`): name, seed (text
  allowed; empty = random unused), game mode, created, last played. Save
  files stay keyed by seed and generator version, so two worlds may not
  share a seed. Saves from before the title screen are listed as
  "World <seed>". Delete asks first, then removes every file of the seed.
- **The player is saved with the world** (`player_<seed>_g<gen>.json`):
  position, look, respawn point, game mode, flying, health, hotbar choice,
  the 36 slots and the worn bag. Before this, quitting lost the inventory.
- **Esc menu** (`PauseMenu`): Back to game, Settings, Save and quit to title,
  Save and quit game. In singleplayer it pauses the scene tree, so the
  simulation, animals, calendar and player stop. It also opens when the
  window loses focus. Esc closes the inventory, chat or menus first, as before.
- **Settings screen** (`SettingsPanel`, the same one in both menus), saved
  to `user://settings.cfg`, never read by autopilot runs:
  - Video: graphics preset, shadows, full detail distance, view distance,
    ambient occlusion, bloom, field of view, VSync, frame cap, fullscreen.
    Changing a single value makes the preset read "Custom".
  - Controls: mouse sensitivity, invert Y, and the key list.
  - Everything applies live, including the terrain's render distance.
- **Presets** (ADR 0031's knobs):

  | preset | shadows | full detail | SSAO / bloom |
  |---|---|---|---|
  | low | hard, 2 cascades, 48 m, filter 0 | 48 m | off |
  | medium | soft, 2 cascades, 64 m, filter 1 | 64 m | on |
  | high (default, the look the budget was measured with) | soft, 4 cascades, 96 m, filter 2 | 64 m | on |
  | ultra | as high | 96 m (64 m rings) | on |

  `graphics=low|medium|high|ultra` sets one for a run.

## Measured
See the table below (dev box, 1920x1080, vsync off). `menu_test=1` drives
the whole flow (title, worlds, create, settings, play, Esc pause with the
clock stopped, a preset applied live, save and quit, the world listed and
reloaded with its flint and position, delete); every field passes.

Each preset, 20 s after load (12 s standing), `other_godot_processes` 0:

| preset | standing 8 m up: p50 / p99 / max, GPU | fast flight 40 m/s: p99 / max | forest fire: p99 / max | triangles (standing) | VRAM | budget |
|---|---|---|---|---|---|---|
| low | 2.61 / 3.33 / 8.05, 2.19 | 3.12 / 8.01 | 3.51 / 4.41 | 3.5 M | 0.6-0.9 GB | ok |
| medium | 3.58 / 4.33 / 6.54, 3.15 | 4.44 / 9.25 | 4.51 / 11.59 | 5.3 M | 0.9-1.2 GB | ok |
| high | 4.61 / 5.52 / 8.44, 4.18 | 4.89 / 7.83 | 5.06 / 6.10 | 10.3 M | 0.9-1.2 GB | ok |
| ultra | 5.94 / 6.90 / 9.45, 5.51 | 7.26 / **27.99** (1 frame > 16.7) | 5.78 / 6.19 | 16.9 M | 1.4-1.9 GB | fails fast flight |

Ultra's 96 m of full detail does not stream at 40 m/s (ADR 0020 measured the
same: it holds at 20 m/s, the normal flying speed). The preset stays, with a
warning on the Settings screen.

## Consequences
- The F4 menu's "Terrain detail" start option overlaps the presets. It is
  kept as a playtest override and applied after the player's settings.
- Multiplayer must not pause the tree; the pause menu says so in code.
- Audio settings come when the game has sound.
