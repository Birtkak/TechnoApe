# ADR 0077: The globe map

- **Status:** built 2026-11-12 on branch `claude/globe-map-b2e479`; **not merged** (fresh review, fast gate and merge still to do).
- **Design:** docs/design/navigation.md part 3; worldgen-plan W2b. Spike: `prototypes/globe-map/README.md`.

## Decision
- M (rebindable, "Globe map", Keys page) opens `Menu::Globe`: an orthographic globe, north up, that a drag turns
  and the wheel zooms 1-64x. A click drops a waypoint on the ground there, a click on a waypoint opens its page,
  a right click removes it. Buttons: Terra, Luna, Back. The latitude and longitude under the pointer use `nav::geo_text`.
- The picture is the bake's map at mip 0, resampled once on a worker into latitude-longitude RGBA8 (Terra 1024 x 512,
  Luna 512 x 256): biome colours, sea by depth, lakes, every river cell, hillshade from the NW. 82-109 ms, off the frame thread.
- One storage buffer (binding 20, 2.70 MB VRAM) holds a 128-byte view, both pictures and both seen-masks. The UI shader
  draws the globe in one rectangle (glyph 200): 3 x 3 pixel blocks, night side where Sol really is, the unseen bare,
  a line every 30 degrees. CPU per open frame: the view write; masks only when they change.
- Seen-mask: a bit per texel, marked out to the horizon from the eye's height over the ground (row spans by the law
  of cosines: 1 us walking, 72 us from a 300 m peak, 0.95 ms from orbit; twice a second at most, after a move).
  Kept as `<save>.seen` (80 KB), written every 60 s and on exit. Creative shows everything; survival shows what was seen.
- Markers are ordinary UI rectangles: you (ring + facing tail), other players (white, named), waypoints (icon, colour).
- The server now sends the player list once a second as well as on joins and leaves, so the others' positions are current.

## Calls for the developer to confirm
- Creative sees the whole planet, survival only what it has seen. Waypoints on the globe stay creative-only, like N and the compass.
- Other players show everywhere on the globe (the design's open question: do friends see each other's maps).
- Seen-mask is client-side beside the save, like waypoints. The client already holds the whole bake to build terrain,
  so this hides nothing it doesn't already have. None for a remote server without a save.
- Edits do not show (a texel is 24 m); the "what you built" layer is for later. The look: colours, hillshade strength, the 30-degree grid.

## Evidence
- `cargo test -p client --lib globe`: 9 tests (projection round trip, drag, seen cap vs. the texel-by-texel reference, file, layout).
- Filmstrips: `prototypes/globe-map/shots/` (sunset terminator through the player, midnight, waypoints, 5x zoom, Luna).

## Review (fresh subagent, principle 12)
- Fixed: holding M flickered the map open and shut (key repeat reached the menu's close); a drag left over from the
  globe stole a later left release (the release is the globe's only while it is open, and `set_menu` drops the drag);
  the seen file is written to `.tmp` and renamed, so a crash cannot shorten it to "nothing seen"; you no longer show
  twice (the roster is filtered by your id, not by position, which lags the prediction).
- Answered: the roster each second is about 150 KB/s at 50 bots, kept; marking from orbit is about 1 ms twice a
  second while moving, within budget, left on the frame thread; Luna is told by its map's version (Luna's count
  from 101, `bake::luna`), kept; a client joined with `connect=` has no save, so survival's seen-mask lasts a
  session there, like waypoints (shortcut until per-server files).
- Confirmed fine: the shader's offsets and head fields against `globe.rs`, the math, the sun's frame, no NaN or index risk.
