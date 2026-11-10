# 0024 – Gravity from every body, and the player's own orientation

- **Status:** accepted
- **Date:** 2026-11-10

## Context
Two bug reports (2026-11-10_19-46-22, 20-44-48) say flying out to space "throws the camera around". Up was
recomputed every step as the direction away from the nearest body (ADR 0007), and the look was laid flat on
that up. Three things followed:
- the up flipped where the nearest body or the frame changed (planet, inertial, moon);
- looking near straight down (one report was at pitch -88.8), the flat look was nearly undefined, so the view
  spun;
- the camera was drawn between the last two physics steps, and when a step changed frames those two
  positions were in different coordinates for one frame.

The developer wants the orientation to belong to the player, with gravity from the bodies keeping them
aligned. Their calls, from a short brainstorm:
- gravity rights the player gradually, as hard as it pulls;
- the bodies' summed pull decides down, not the nearest body;
- in space there is full rotation: no pitch limit, and roll on Q and E;
- creative flight near a body stays upright like walking.

## Options considered
1. Keep up = away from the nearest body and blend it over time. Small, but it keeps the flip at the
   hand-over and the spin near straight down. There is still no orientation of the player's own.
2. A body orientation of the player's own (its up and look), turned towards the summed pull at a rate set by
   the pull's strength, with the head's pitch on top while gravity holds the player, and free pitch and roll
   in space.

## Decision
Option 2.
- **Gravity** (`World::gravity`): every body pulls towards where it is now, in whichever frame the position
  is written, so the hand-over between frames changes nothing.
  - The pull is G = 9.81 m/s² over all of a body's terrain (within `AMP`, 80 m, of its radius) and falls
    with the square of the distance past that. Walking on both bodies feels as before.
  - At 3 km over the planet the pull is about half; far out it fades to nothing.
  - Up is against the sum. Near the planet's surface the moon tilts it by about 0.02°.
- **The body's own orientation** (`Player::up` beside `Player::fwd`):
  - Each step the body's up turns towards the pull's up at `RIGHT` × (pull / G)² rad/s, with RIGHT = 3.
  - While standing, or flying creative within `NEAR` (1.5 km) of a surface, it is held at `HOLD` (8 rad/s).
    That is exact while walking, and a smooth turn of a few tenths of a second when you come in tilted.
  - Nothing snaps anywhere.
- **Free rotation** (`phys::free`): out of a hold, the mouse's pitch turns the body itself (`Input::tilt`)
  and Q/E roll it (`Input::roll`, 1.5 rad/s). This applies where the pull is under `FREE_G` (2 m/s², about
  6 km over the planet), or where gravity has not yet righted the body to within 0.1 rad.
  - Otherwise the mouse's pitch is the head's, clamped to ±89° over the body's horizon, as before.
  - Going free folds the head's pitch into the body, so the view does not move.
- **Movement:** flight goes along the body's own axes (forward, right, up). Walking goes along the ground's,
  with the body's look laid level on it.
- **Drawing:** a step that changes frames carries the drawn path and any correction left into the new
  frame's coordinates.
- **Keys:** the hammer's shape moves from Q to B, and the bucket from B to V. The acts follow (`b` the shape,
  `v` the bucket).

## Consequences
- In space the player keeps facing wherever they faced. Coming back, gravity rights them as the pull grows,
  and they land upright.
- Physics depends on the moon's place, so on the clock. Steps must be taken at their own time (`Input::t`,
  as clients already send). One server test now sends a time with its steps.
- `Player` and `Input` gained fields (with serde defaults). Saves keep only the look, and a loaded player
  stands upright on the first step. Recordings (postcard, positional) from older builds no longer load.
- Gravity's strength now varies far from a body, so a fall from orbit is gentler than before.
- Revisit: how gravity shows up in the HUD (which body pulls, how hard), and whether walking should keep the
  feet planted while the body is still turning upright after a landing.
