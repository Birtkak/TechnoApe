# ADR 0160: Moving contraptions: lifts, drawbridges, cranes as driven rigid pieces

Status: accepted (2026-11-07)

## Context
Fun pass row M (2026-11-07): player-built parts that move on a track (a lift, a drawbridge, a crane), driven by a
hand crank, a lever or power, stopping at their ends, carrying what stands on them, saved with the world. The world
already moves voxels as one body: rigid pieces (P12, ADR 0037) cut loose wood, fly it, draw it and paste it back.
A second physics path for contraptions was ruled out.

## Decision
1. **A contraption part is a *driven* rigid piece.** `RigidPieces` gains `lift(seed, lo, hi, max_cells, wood_only)`
   (cut out the seed's material, 6-connected, inside a box; refuses with `edge` / `big` / `outside` / `nothing`
   instead of taking part of a floor), `drive(id, local_to_world)` (the pose for the next step; driven pieces skip the
   physics) and `release(id, settle)` (paste into voxels at that pose now, or become an ordinary piece that falls and
   settles). Meshing, drawing, interpolation and pasting are the existing piece code; FallingPieces draws them.
   Called between ticks (FallingPieces.tick runs `Contraptions.tick` between `collect()` and `queue_step()`).
2. **At rest a part is plain voxels.** The drive (`ContraptionEntity`: `lift_winch`, `drawbridge_hinge`, `crane`, an
   LV machine block) saves only facing, lever, pose and whether it has arrived. The part is whatever the player
   built against it, in one material: the lift's platform in front of it (3 m out, 3 m wide, the block's height),
   the drawbridge's span in front (6 m out), the crane's mast and jib on top (6 m round, 4 m up). A blueprint that
   copies the voxels and the entity copies the contraption (ADR 0161). A part of the same material as the floor round
   it is refused with a message (it would run on past its box).
3. **Poses end on whole voxels.** Lift: a translation up until the slab ahead meets terrain (stopped flush, on the
   whole voxel touching it) or `lift_max_m`. Drawbridge: 90 degrees about the top edge of the hinge's front face, the far end up.
   Crane: 90 degrees about the drive's middle column (a voxel corner). Quarter turns about those lines map voxels onto
   voxels, so a part pasted at an end is exactly the part.
4. **Driving is power on the LV network (ADR 0123).** A right click flips the drive's lever (the end it heads for);
   it is a 40 W `PowerLoad` that wants power only while it has somewhere to go, and moves while the solver gives it.
   So a hand crank beside it (cranking = moving), a battery, a generator, through a knife switch, all drive it.
   Speeds: `contraption_lift_mps` 1 m/s, `contraption_swing_dps` 30 deg/s.
5. **The crane's hook:** when the jib swings out, the wood (rigid class wood: logs, planks) under the jib's tip,
   within `crane_hook_m`, is cut out as a second driven piece, raised under the jib, swung with it, lowered until
   it meets the ground and released as an ordinary piece, which settles by itself. Rope and hook: the climbing
   rope's warmed material and the grappling hook's mesh (no new pipeline).
6. **Bodies:** each physics frame (after the player, mobs and items moved) whatever stands on a moving part's top
   (its bounding box; up to 45 degrees of tilt) follows the drawn transform onto it. Per tick, a part that would push
   a rider into terrain, or run into the player or a mob standing in its way, waits for that tick (the mob is hurt
   2 every half second); it never shoves anyone into voxels.
7. **Saving:** on quit (`save_terrain(wait_s > 0)`) every moving part is snapped to its end (a lift to a whole
   voxel) and pasted, the crane's cargo where it hangs, then the blocks are saved again; the drive goes on when the
   world loads.
8. `/contraptions [toggle [kind]|ride [bridge]|jump] [cam]` builds one of each on cranks; `/scenario contraptions`, `/kit contraptions`.

## Consequences
- Parts move only inside the main sim window (rigid pieces live there); a drive far from the player waits.
- Mobs and items on a part past 45 degrees are not slid off (only the player); they stay where they are.
- A quit mid-move pastes the part at its current pose (checked: a lift quit at 7.2 voxels reloads at 7 and goes on).
- No new material ids, no generator change.
