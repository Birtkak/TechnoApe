# 0030 – Latitude and longitude, a compass, waypoints; one look for the HUD

- **Status:** accepted
- **Date:** 2026-11-11 (the developer's calls in a design session; 0029 is left for the world generation)

## Context
The developer wanted a coordinate system "like Minecraft's" but saw that block coordinates do not fit
spinning planets, a moon and space, and asked for a recommendation; then chose parts 1 and 2 of it to be
built and part 3 to be planned (`docs/design/navigation.md`). In the same session they asked for the menus,
text and backgrounds to look better and took all five proposals.

## Options considered
- **Naming a place:** the grid's face, column and layer (exact, unreadable, six seams); x, y, z from the
  body's centre (turns with the body but means nothing to a walker); latitude, longitude and height.
- **Finding the way:** coordinates alone; a compass; waypoints; a flat map; a globe; an attitude ball in
  space.
- **The look:** each a separate taste: real small letters or the small capitals; one panel style or each
  part its own; block icons or colour squares; a blurred or a dimmed world under menus; motion or none.

## Decision
- **A place is a body, a latitude, a longitude and a height.** North is the body's spin axis; Terra's
  longitude 0 runs through the spawn, a moon's through the middle of the face turned to its planet
  (`crates/client/src/nav.rs`).
- **A compass strip and waypoints**, creative's until the tool that earns them in survival exists
  (ADR's sibling call, 2026-11-11: earned in survival, always there in creative). N drops a waypoint; the
  pause menu lists and removes them; they are the player's notes, kept beside the save as
  `<save>.waypoints`, and go with a removed world.
- **The globe map waits for the bake** (`worldgen-plan.md`, after W2); the attitude ball for the jetpack.
- **The look:**
  - the font draws its small letters (`data/font.txt`; 8 rows for a letter with a tail);
  - every panel is one dark glass with the accent down its side (`hud::PANEL`): F3, the chat, the hotbar,
    the labels, the menu's rows;
  - the hotbar shows each block as a little cube, its top and sides in the block's colours;
  - the world is dimmed under a menu. Softening it would need a blur pass over the picture; not done;
  - a menu page's rows come in from the left over 0.14 s and the lit row moves out 8 pixels in 0.1 s. The
    places that take a click do not move, and a scripted off-screen run shows no motion (`motion=1` does).

## Checks
Tests in `nav.rs` and `hud.rs` (22 in the client now); scripted runs with filmstrips of the compass, a
waypoint in the world, the Waypoints page and the hotbar; `tools/check.sh fast`. A reviewer who did not
write it found six defects, all fixed: a removed world left its waypoints to the next world of that name,
the markers trailed the camera by a frame, survival could drop waypoints and read coordinates off the
menu, nothing capped the markers drawn, the motion made scripted pictures vary, and three letters were
redrawn (l read as 1, i did not match j, r's arm was not joined).

## Consequences
- shortcut: a waypoint cannot be renamed or recoloured, and one on another body is not shown; a remote
  server's waypoints are not kept (they need a file per server).
- shortcut: the cubes are flat colours, not the blocks' textures; those need the texture function on the
  HUD's side.
- shortcut: the look has no settings yet (the Interface page of `docs/design/settings.md`).
- The capital N and M of the font are its own squared shapes; with real small letters beside them they are
  the next thing to look at, a taste call.
