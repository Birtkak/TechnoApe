# 0053 – Creatures and the walk rig

- **Status:** accepted
- **Date:** 2026-11-12

## Context
docs/design/studio-tools.md "Fauna" asks for creatures made from a voxel model (ADR 0047) plus a row of
`data/creatures.toml`, rigged "by choice": pick a body plan and the walk comes from it, never keyed by hand. The
game has no creatures yet (no spawning, no AI, nothing on the server), so this is the data, the rig and the
Studio page the game will take later.

## Options considered
1. Keyed walk clips in each model file, the page writing them. Pros: the game already plays clips. Cons: a
   change of speed or step length means re-keying; the numbers that made the walk are lost.
2. The row keeps the plan and the gait numbers; `engine::creatures::walk` makes an ordinary clip from them when
   asked. Pros: one source of truth, any speed, tested once. Cons: a hand-tuned walk needs a later "freeze" to
   keys (not built).

## Decision
Option 2.

- **The row** (`[[creature]]`, `engine::creatures`): name, model, `plan` (`biped`, `quadruped`, `bird`, `fish`),
  `scale`, `walk_speed`, `run_speed` (m/s), `step` (m a foot moves a step), the part roles by part name (`legs`
  in a fixed order: biped left, right; quadruped front left, front right, back left, back right; `arms` and
  `wings` none or two; `head`, `tail` or ""), `step_sound` and `voice` (rows of `data/sounds.toml` or ""), and
  the gait: `swing` (deg), `bob` (voxels), `sway` (deg, head), `wag` (deg, tail). `parse` refuses by row and
  reason (unknown model, a part the model lacks, the wrong number of legs for the plan, a number outside
  `RANGES`, run slower than walk, an unknown sound); `write` is byte-exact on its own output; the Studio rewrites
  one row through `rows::save`, every other byte kept.
- **The walk:** a looping clip of 16 linear keys a cycle (plus the first again, so it loops without a jump). One
  cycle is two steps: length = 2 x step / speed, so the feet keep pace with the ground. Legs turn about x at
  their pivots, sin-shaped: a biped's (and bird's) opposite; a quadruped's in diagonal pairs (front left with
  back right). Arms swing against the legs of their side at 0.8 of the swing; wings beat about z twice a cycle.
  The swing grows with the square root of speed / walk speed (0.3 to 1.6 times the row's). The first root part
  dips by `bob` where the legs reach furthest, twice a cycle, so the feet stay near the ground. Head sways and
  tail wags about y, a quarter cycle apart. Speed 0 gives `idle` (3 s: the body breathes, head and tail move a
  little). A missing optional part is skipped. Deterministic: same row, same clip.
- **In the game:** `creature=name[:speed]` on any run gives that creature's model the clip `walk` (from
  `data/creatures.toml`, read when the models folder is read); `run=stand creature=boar model=boar:walk` stands
  it 3 m ahead walking in place, at the spawn (on land) unless `at=` is given.
- **The Studio's Fauna page:** the rows in the files column; the Viewport shows the model with the generated
  clip (written as `out/studio-view/models/creature.toml`, as the Models page sends an unsaved model) walking in
  place; a speed slider from 0 through walk to run (keys 0 1 2); the inspector has pick lists for model, plan
  and every part role (from the model's part names), the numbers as sliders with units (Body, Gait), the sounds,
  and a variations wall of six gaits (swing, bob, sway, wag, step moved by the spread), drawn as side sketches.
  Save rewrites the row (checked by the game's reader, through `.tmp` and a rename); undo and redo; the rows are
  drafts as `data/creatures.toml#<name>`. `check=1` reads the file through the game's reader.
- Drafts: `data/models/boar.toml` (quadruped) and `data/models/ape.toml` (biped), small and from palette colours.

## Consequences
- Left out: spawning, AI, anything on the server or the protocol; turning, jumping, eating, hurt and attack
  clips; walking in a circle on the ground at real speed (the page walks in place: sliding feet are judged from
  the cycle formula, not seen); six legs and snakes; `scale` is kept but the renderer does not scale models yet;
  "freeze" of a generated walk to hand keys; foot planting (no IK: the feet swing on arcs and the body dip only
  approximates the lift); the step and voice sounds are not played by the walk.
- The Studio does not yet watch `data/creatures.toml` for outside changes (the frame's watch list is shared code);
  the page reads it on the frame's refresh.
- A change to `data/creatures.toml` alone reaches a running game at the next change of its models folder.
- Review: none yet (the author does not review; a fresh reviewer should read the diff before a commit).

## Review (2026-11-12, a fresh reviewer of `engine::creatures` and the client's `creature=`)
- `walk` indexed `legs[0..4]` by the plan without checking the row; the Studio walks the row being edited, so
  switching a two-legged row's plan to quadruped panicked (index 3 of 2). Legs not matching the plan now do not
  swing. A speed of inf, NaN or a hair over 0, or a step of 0, gave a clip of zero, infinite or NaN length (NaN
  poses): under 1 cm/s, NaN, or a step not over 0 is the idle, over 25 m/s walks at 25. Test
  `an_unchecked_row_or_a_wild_speed_never_panics_or_gives_nan`, seen red on the index panic.
- Checked and sound: a checked row cannot divide by zero (walk_speed >= 0.1, step >= 0.05, NaN fails the
  ranges); the walk and idle loop without a jump (every wave is whole cycles over the clip, the last key at the
  length; the existing test covers it); a bad `data/creatures.toml` only prints on stderr, and only a run with
  `creature=` reads it.
- Left: a row naming one part twice (two legs the same, or the head the root part) keys that part twice at the
  same times (no panic, an odd pose); one refused row refuses the whole file for `creature=`, as other data
  files do; an unchecked row with walk_speed NaN still gives NaN swings (the Studio's sliders keep it in range).

Second fresh reviewer (2026-11-12). Fixed: a bird's two wings were given phases 0 and 0.5 at two beats a cycle,
which is the same phase: both turned the same way about the body's length, a roll, not a flap. The right wing
now starts a quarter cycle on (half a beat); test `a_birds_wings_beat_mirrored`, seen failing (7.75 deg on both).
No shipped creature is a bird yet.

## Addendum (2026-11-12): scale and footfalls in the game
`creature=` draws its model at the row's `scale` about the cell's floor centre (its points and events too), and
`creatures::steps` gives the walk's footfalls as clip events playing `step_sound`, a quarter and three quarters into
each cycle (where the legs reach furthest and the body is lowest; a quadruped's diagonal pair lands as one), played
through the models' event path. Tests `the_footfalls_fall_where_the_feet_are_down` (engine) and
`a_creature_is_drawn_at_its_scale_and_steps_on_its_footfalls` (client), seen red. The voice is still not played.

## Addendum (2026-11-12): standing on the ground, and the Fauna page's view
- `model=`, `creature=`, `fxplay=` and `structure=` stood their cell with `block_at(height + 0.1)`: the terrain's
  height falls inside its top block (or over a lower neighbour on a slope), so on the baked map 120 of 300 land spots
  were a solid cell or a floating one. `World::air_over_ground` (walked up to air, then down onto the ground) is
  now the one place all of them, run=fx and the Studio's view take their cell from; test
  `the_air_over_the_ground_stands_on_it`, seen red. Without `at=` these runs stand at the spawn (the +y pole of the
  baked planet is open sea). At pitch 0 a model 3 m ahead sits at the frame's bottom edge: film it with `pitch=-25`.
- The Fauna page's view gets the walk with its footfalls as events (`the_shown_walk_steps_on_its_footfalls`) and
  draws the creature at its `scale` (`model name clip t x<scale>` on the view's protocol); a creature's scale is
  its cells' only, never the model in hand or a particle (`a_creatures_scale_never_reaches_the_model_in_hand`).
