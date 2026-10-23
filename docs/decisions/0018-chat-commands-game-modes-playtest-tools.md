# 0018 – Chat, slash commands, game modes and playtest tools

- **Status:** accepted
- **Date:** 2026-10-22

## Context
The developer asked for Minecraft's chat box (T), slash commands, a creative
mode with `/gamemode`, a Minecraft-style F3 screen, a debug button for easy
playtests, a bug report feature, and playtests from the editor that start
minimized. Until now every test switch was a command-line argument for the
autopilot; a person playing had F (fly) and F3 (a few debug lines) only.

## Options considered
1. **Commands as the one entry point.** Chat, the debug menu buttons and the
   autopilot (`commands=`) all send the same `/command` lines. One code path
   to test headless; every button is also typeable and shows its result in chat.
2. Separate debug actions per UI. Faster to write, but every action would
   need its own tests and the menu and chat would drift apart.

## Decision
Option 1.

- `CommandRegistry` (pure, no nodes): tokenize, dispatch by name or alias,
  `ok()` / `fail()` results, Tab completion from per-argument candidate lists,
  built-in `/help`. `GameCommands` registers the game's commands: `gamemode`
  (`gm`; 0/1/s/c), `give`, `clear`, `tp` (voxel coordinates, `~` relative),
  `time`, `daycycle`, `kill`, `heal`, `seed`, `kit`, `spawn`, `locate`,
  `goto`, `blast`, `stations`, `bug`; since 2026-10-23 `setblock` and
  `fill` (Minecraft syntax, `~` relative; `/fill` goes through the native
  kernel's `fill_box`, at most 64^3 voxels and only inside the loaded sim
  bubble; block-entity materials are refused). World edits typed before the
  bubble has loaded (start commands) wait for it and report in chat.
- `ChatBox`: T opens it, `/` opens it with the slash typed (any keyboard
  layout). Enter sends, Esc closes, Up/Down history, Tab completes (Shift+Tab
  back). Messages fade after 10 s; a hint shows the usage and Tab candidates.
- `GameMode`: survival and creative as rule sets (`can_fly`,
  `instant_break`, `drops`, `consumes_items`, `takes_damage`,
  `item_picker`). Creative, as in vanilla: flight (F or double-tap Space),
  instant breaking of anything, no drops, blocks and buckets never run out,
  no damage, hearts hidden, middle-click pick block, and the recipe
  browser's item list hands out stacks (shift: into the inventory; clicking
  with a stack on the cursor deletes it). **Survival no longer flies**: F did
  so for everyone before. The default stays survival.
- F3 is rebuilt after vanilla's debug screen: game and world facts left
  (fps, sim, position, block, chunk, facing, biome, climate column, time,
  seed, mode, health, held item), system and the targeted block right (GPU,
  VRAM, draw calls; material properties, mining data, block entity), an axis
  crosshair, and a frame-time graph coloured by the ADR 0009 budget.
- F4 opens a playtest menu of command buttons (modes, kits, time, teleports
  to caves, ruins, lakes and biomes, crater and liquid-ball tests, F3, camera,
  screenshot, bug report). A "Debug (F4)" button shows top right whenever the
  mouse is free. Its start options (creative, minimized, noon, start kit,
  extra commands) are saved to `user://playtest.cfg` and applied on every
  normal launch. Autopilot runs never read them, so runs stay reproducible.
- F9 or `/bug` writes a bug report folder: `report.md`, `state.json`
  (`Main.snapshot()` plus frame times and chat), `screenshot.png` (the frame
  when F9 was pressed) and `log.txt` (Godot log tail). Editor runs write to
  `game/out/bug_reports/` (git-ignored) so Claude can read them; exported
  builds write to `user://bug_reports/`. Autopilot runs write into their own
  `out=` folder, so the newest folder in `game/out/bug_reports/` is always one
  a person filed. F2 saves a screenshot the same way.
- Editor playtests (debugger attached, not embedded in the editor's Game
  tab, not autopilot) start minimized; a click after restoring captures the
  mouse. `minimized=0` or the menu's checkbox turns it off.

## Consequences
- 14 headless tests (`test_commands.gd`) cover the registry, modes, items,
  teleport, pick block and the creative picker; autopilot `commands=`,
  `chat_text=`, `debug_menu=1`, `f3=1` capture the UI for screenshots.
- Commands run locally. With the dedicated server (ADR 0002) they become
  requests the server authorizes (operators, per-player game modes); the
  registry stays, the handlers move behind the server.
- No spectator or adventure mode, no command permissions, no `@p`/`@a`
  selectors yet. Kits and features are lists in `GameCommands`; they belong
  in data once they grow.
