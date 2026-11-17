# 0144 – Things on the ground are aimed at themselves

- **Status:** accepted (the click rules are the developer's call)
- **Date:** 2026-11-17

## Context
ADR 0082 took a pick-up (stick, loose stone, flint, mushroom, berries, an ore sample) by a left click on the ground
block under it, and outlined that block. The developer (2026-11-14, roadmap "Queued by the developer") asked for
the thing itself to be aimed at: a hit test on its model's boxes, nearer than the block behind it; the outline on
it; a left click mines it; a held right click picks it up.

## Options considered
1. The server re-does the hit test from the player's eye and look: the look it holds (`Move`'s `fwd` and `pitch`)
   lags the click by the wire, scripted bots never set a look (`pick dx dy dz`), and a dig already names its
   target for the server to check by reach.
2. **The client aims, the server checks what it is named**, as for a dig. Chosen: no wire change.

## Decision
- **The hit test** (`clutter::aim_thing`, in the engine beside `edit::aim`): the ray walks the cells as `edit::aim`
  does and stops where that would; in each air cell where a pick-up lies (`pickup_at`) and is not taken, it is
  tested against the thing's own faces as drawn (`clutter::thing_faces`: its alternate's faces in `Still`, turned
  and set in the cell as shader_models.wgsl `still_local` does, the cell flat between its corners). A ray through
  the cell that misses the faces goes on to the block behind.
- **Aimed at only when a click would take it:** a thing with the bucket in hand, one whose items do not fit, or
  one whose take is on its way is not aimed at; the clicks and the outline then go to the block behind it.
- **Outline:** with no button held, the aimed thing's face edges take the place of the block's outline
  (`hud::thing_ghost`, the same white and the same rainbow setting).
- **Clicks:** left click on a thing takes it at once, whatever is in hand (pick-ups have no dig rule; when one has,
  it goes by that rule). Right click held `PICK_HOLD` = 0.3 s on the same thing picks it up **only with the hand
  empty** (no item in the hotbar slot, no hammer, no bucket); looking off it, letting go or taking something in
  hand cancels. With anything in hand the right click does what it did before: place, strike, pour, open a
  window. Both send `ToServer::Take { seq, at }` as before; one click sends one take (the cell waits for the
  server's refusal or its `Taken`).
- **Frame thread:** the aim asks only height tiles already built (`Body::tile_if_built`); a miss is no thing there
  this frame. `Body::tile` builds a whole tile on a miss and is for workers and the server.
- **Server:** unchanged. `Take` is checked as ADR 0082 says: reach, that one lies in the named cell, not taken,
  fits. A take named past the thing names a cell where nothing lies and is refused.

## Consequences
- `thing_faces` repeats shader_models.wgsl `still_local`'s placing of a model by hand (a comment at each names
  the other; the test holds it to the packed face table and the cell, the shader itself cannot run in a test).
- Both clicks give the same today; they part when a thing gets a dig rule or a "mined" result of its own.
- `shortcut:` a face reaching over its cell's side is hit only from within its cell (the models stay inside but
  for a turned corner).
- The server trusts the client's aim as it does for a dig; a cheat could take a thing in reach it does not look at.
- Cost: `aim_thing` about 2.2 us a call (release build, the engine test's timing), once a frame. No perf-debt row.

## Calls for the developer
- The right click picks up only with the hand empty, the left click takes whatever is held.
- 0.3 s for the held right click; whether right click should also gather by sweeping while held.
- The outline on a thing as small as a mushroom is drawn nine times a pixel apart, like the block's, and nearly
  covers it; thinner for things?
