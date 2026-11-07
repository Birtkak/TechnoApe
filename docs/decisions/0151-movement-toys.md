# ADR 0151: Movement toys: glider, climbing rope, grappling hook

Status: accepted (2026-11-07). `scripts/player/movement_toys.gd` (new, `MovementToys`), small hooks in `player.gd`
and `main.gd`, three items, four recipes, sprites. No new materials (141..142 stay free).

## Context
The fun pass of 2026-11-07: the world is tall (ravines, ADR 0133; cliffs, the monolith) and getting around it is
walking, ladders and the late steam jetpack. The developer picked movement toys as cheap fun, early in the game.

## Decision
- **One node owns all three** (`MovementToys`, a child of Main like `PlaytestTools`). An item's `move_toy` names
  its kind. `Player._physics_step` calls `toys.step(delta, dir, input, jump)` on foot, after the walk, gravity,
  jetpack and ladder and before `_move`; `Player._use` offers it the right click first. A toy only sets
  `Player.velocity` (and `_fall_peak_y`): the body still moves through `Player._move` and `VoxelBoxMover`, which
  sweeps the box against the voxels, so no toy can tunnel (the scripted check counts 0 frames inside a voxel).
- **Glider** (Stone Age: 3 linen cloth or 3 leather, 3 sticks, 2 cordage or leather strips). Jump or right click
  while falling opens it. Flight in metres: the path angle is the camera pitch less the glide angle
  (`atan(1 / glider_glide_ratio)`, ratio 8); gravity along the path speeds a dive and slows a climb, drag `k v^2`
  settles the speed at `glider_trim_speed_m` (9 m/s, a 1.1 m/s sink) when looking level; capped at 28 m/s; under
  5 m/s it cannot climb and the nose drops. It opens at trim speed or the fall's speed if faster (a dive is kept).
  The heading follows the camera at 4/s and the view rolls by up to 0.35 rad with the turn; a delta wing shows over
  the head; the wind loop rises in volume and pitch with speed. Ground, water, jump again or another item folds it.
  A landing hurts by its downward speed only (`_fall_peak_y` from v_y), not the height glided down.
- **Climbing rope** (Stone Age: 4 cordage and a stick). Not a voxel: a line `{x, z, top, bottom}` drawn as a thin
  cylinder and saved with the player (`ropes`, like the TNT charges), so it needs no material id and no mesher
  change. On a top face it goes out along the look (1.5 m at most) to the first column with no ground under it and
  hangs from there, against the edge; on a wall or a ceiling it hangs where you click; it runs down to the first
  solid voxel, `rope_max_m` (32 m) at most, at least 1 m. Against it (body radius + 0.3 m): W climbs at the ladder
  speed, S goes down, nothing held hangs, the body is held at the rope's side (round 2: real rims below); near the top W keeps walking so the
  body steps onto the ledge; jump lets go (0.5 s before it grabs again) with a push off it. A right click within
  0.25 m of the rope (or just past the face the aim hits, where the rope lies) takes it back.
- **Grappling hook** (iron: 3 iron ingots and a climbing rope at the crafting table). Right click fires the hook at
  70 m/s along the look, 36 m at most; it catches on a voxel that blocks bodies and is not a powder, sand, gravel
  or snow, else it slips (a toast; leaves hold since round 2). Held right click shortens the line at 14 m/s (the pull no faster than
  that); released, the line holds a pendulum: gravity and WASD (5 m/s^2) act on the body's own velocity, then the
  step's end is pulled back onto the line's sphere and the velocity is what reaches it (position-based, so a swing
  keeps its energy and the line never stretches; the first version removed the outward speed and corrected the
  stretch by velocity, which bled the swing away in two periods). Jump lets go with the swing's speed; another
  right click, putting it away, or the voxel being dug unhooks it.
- **Momentum after a toy** (`_carry`): a fold, a let-go or a jump off a rope leaves the body's sideways speed; in the
  air it carries on with 1.5 m/s^2 drag (the walk steers a little), on landing it becomes `Player._knock` (a slide).
  `_knock` alone (a blast's 6 m/s^2 air decay) killed a swing's let-go within half a second.
- **Not shared in multiplayer** (ADR 0145): the toys are player-local and remote players only send position and
  look, so others see the body move but no wing, rope or line, and a rope hung by one player is not in the other's
  world. Sharing it needs a toy state in the player packet and a rope op; left for when multiplayer is played.

## Consequences
- `/kit toys`; the recipe browser shows all four recipes; sprites from `make_item_sprites.py`.
- Check: `toys_test=1` (game/README.md): glide ratio 8.0, trim 9.0 m/s, dive 14 m/s, pull-up +3.3 m, a 16 s flight
  from 26 m, landed unhurt; a 27 m rope climbed in 11.5 s onto the top and taken back; the hook slips on sand,
  catches the beam at 14.7 m, reels to 7.5 m, swings an 11.4 m arc, a let-go at 9.2 m/s flies 7.9 m; 0 frames inside
  a voxel. One sheet per toy (filmstrip + burst).
- The ape model has no climbing or hanging pose.

## Round 2: real terrain (2026-11-07)
`toys_test=1 toys_world=1` finds the steepest drop within 400 m of the spawn (generator heights, then the real
voxels; seed default: a 16 m limestone cliff with a 0.6 m stone lip on its rim) and runs the toys there. What it
showed and what changed:
- **Glider:** a blocked step cost 60 % of the speed, so brushing a crown or a lip stalled the glide. Now a scrape
  costs 20 % a step and lifts the glider (at least 2 m/s up) over the lip. Off the cliff: 108 m in 10.4 s, landed
  in a forest unhurt.
- **Rope:** the rim walk refused any bump ("Something is in the way") and stopped at a ledge 2.7 m down. Now it
  climbs lips up to 1 m and walks through hollows under 1 m; out to 0.75 m past the rim it hangs where it falls
  furthest (a short piece back to its stake); its top is set over the highest ground on the way (the climber must
  get over the lip); a climber under an overhang leans out (up to 0.8 m) until the head clears; the grab reaches
  0.6 m over the top. A rope remembers the voxel it was hung on and drops as an item when it is dug away. On the
  cliff: 17.1 m, climbed in 7.2 s onto the top.
- **Hook:** leaves now hold (swinging from a crown is the fun; the plants without collision were always passed
  through); sand, gravel, snow still slip. On the cliff it caught the stone 15 m up and reeled the body to the rim.
- The test runs 60 m south of the spawn: the monolith plaza north of it is in any straight path off a tower.
