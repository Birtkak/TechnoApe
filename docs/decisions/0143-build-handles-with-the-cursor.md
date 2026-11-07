# ADR 0143: Build handles dragged with the cursor, size and hollow as rings

Status: accepted (2026-11-04)

## Context
ADR 0038 set a placed shape's handles down as cubes aimed with the crosshair and dragged by turning
the view, four side arrows for the footprint. Lay of the Land, the model (research of 2026-10-24),
frees the mouse once a shape is placed and drags "physicalized" gizmos with the cursor: an outer ring
grows the shape toward where you grab it, an inner ring hollows it. The developer asked for both.

## Decision
1. **A placed shape frees the cursor** (`State.EDIT`): the aim ray goes through the mouse position
   (`ShapeBuilder._ray`), so hover, drag, the wheel and the click away all use the cursor; the player
   keeps WASD. The **right button held** captures the mouse to look around and puts the cursor back
   where it was on release; a plain right click (under 6 px of motion) still cancels. Every other
   builder state captures the mouse again. `Player` no longer recaptures on a click while `building`.
2. **Rings instead of side arrows** (`TorusMesh` handles with a `radius`): the red ring around the
   foot grows the side nearest the grab point (the same `_apply` sides as before: +x, -x, +z, -z); the
   yellow ring on the top face, with the radius of what stays open, hollows when dragged inward. The
   top arrow and the move knob stay cubes. A ring is picked where the aim ray crosses its plane.
3. `/shape drag <+x|-x|+z|-z|hollow|top|move> <voxels>` keeps working: a side names the point on the
   ring at that side, and the command captures the mouse to aim with the view.
5. **The look:** the ghost uses the brush ghost's shader (`brush_ghost.gdshader`, new `rim` uniform): a shape
   of one material shows that material's pattern washed in the state colour (blue, red when short), frames
   and prefabs the colour alone, with a brighter rim where the surface turns away. Handles are shaded with
   a little emission, the height handle an arrow, the move handle a sphere, the rings thin tori, and a
   `next_pass` draws the hidden part faintly through the ground (editor x-ray).
4. **A selection gets the same handles** (top arrow, size ring, move knob) before it is saved as a
   prefab, LotL's "select an area with adjustment gizmos"; a click away still opens the save prompt.

## Consequences
- The hint, the G panel and `docs/plans/to-test.md` say so; systems.md's building paragraph updated.
- Not done: LotL's menu on E and tool modes on F, detail mode (C): separate calls.
