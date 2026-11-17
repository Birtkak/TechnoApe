# NNNN – Things on the ground are aimed at themselves

- **Status:** proposed (the click rules are the developer's call)
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
- **Outline:** with nothing held, the aimed thing's face edges take the place of the block's outline
  (`hud::thing_ghost`, the same white and the same rainbow setting).
- **Clicks:** left click on a thing takes it at once (pick-ups have no dig rule; when one has, it goes by that
  rule). Right click held `PICK_HOLD` = 0.3 s on the same thing picks it up; looking off it or letting go cancels.
  Both send `ToServer::Take { seq, at }` as before. The bucket keeps its clicks.
- **Server:** unchanged. `Take` is checked as ADR 0082 says: reach, that one lies in the named cell, not taken,
  fits. A take named past the thing names a cell where nothing lies and is refused.

## Consequences
- Both clicks give the same today; they part when a thing gets a dig rule or a "mined" result of its own.
- `shortcut:` a face reaching over its cell's side is hit only from within its cell (the models stay inside but
  for a turned corner).
- The server trusts the client's aim as it does for a dig; a cheat could take a thing in reach it does not look at.
- Cost: `aim_thing` about 2.2 us a call (release build, the engine test's timing), once a frame. No perf-debt row.

## Calls for the developer
- 0.3 s for the held right click; whether right click should also gather by sweeping while held.
- The outline on a thing as small as a mushroom is drawn nine times a pixel apart, like the block's, and nearly
  covers it; thinner for things?
