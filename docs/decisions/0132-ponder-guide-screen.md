# ADR 0132: The guide screen (ponder): animated lessons on how pieces connect

Status: accepted (2026-11-03). Builds on ADR 0113 (networks), 0117 (LV machines), 0122 (steam and water
channels), 0123 (the DC circuit) and 0100 (transport). Bug report: "add a Create-like ponder screen that shows an
animation with how things like boilers and machines need to be connected, and make it clear how you connect
pipes and wires."

## Context
Players placed a boiler, a pipe and a turbine and nothing ran, because the rules live in the solver (face-to-face
block cells on one lattice, no diagonals, an open switch is a gap) and nowhere a player can see them. Create's
ponder screens teach this with a small animated stage; ours had tooltips and the look-at panel only.

## Decision
- **Scenes are data**: `game/data/ponder/<id>.json`, one short lesson each: a title, the item ids it is the guide
  for, and steps. A step keeps what the steps before it built and adds its own: `place` (real pieces by `kind`,
  with `item`, `facing`, `axis` and any entity field under `set`), `set` (fields on a piece placed earlier: a
  switch closes, a turbine stops), `remove`, `blocks` (coloured voxel boxes by material name: bricks, water, a
  dirt step), `mark` (a pulsing green or red box: right or wrong), `flow` (dots running from one block to
  another: steam, water, power or items) and `text`. Positions are 0.5 m blocks from the stage's origin.
- **The stage is the game's own look**: `PonderScreen` (a CanvasLayer) builds a throwaway `BlockEntities` store
  from the scene and draws each piece as WorkSystem does (`WorkPalette.art_parts` + `make_view`) in a SubViewport
  with its own world, a slow orbiting camera and a flat slab for ground. No art pipeline, no scene files: a new
  machine gets a guide by naming it in a JSON file. The play-range cutoff on work-piece meshes (96 voxels) is
  cleared on the stage, whose camera stands further than that. The turbine and the small sources turn while
  their `rpm` / `drive` is set, as in play.
- **Entry points**: P over an item in the inventory or the browser opens its guide (the tooltip says
  "Guide: how it connects (P)" next to "Made by (R)"), `/ponder <scene or item id>` opens one from chat
  (`/ponder` lists them). Esc closes, Space or the right arrow steps, the left arrow steps back, R replays, and a
  step advances by itself after `seconds` (6 by default). Opened over the inventory it closes with it.
- **The rules the scenes teach were checked on the solver** (`test_ponder.gd`): two pieces join when a block
  cell of one touches a block cell of the other face to face on the same lattice; a corner joins; a diagonal does
  not; a piece a voxel higher does not; a pipe on a one-block step beside a boiler does, because the boiler fills
  every level it stands on. Open circuit devices are gaps (`CircuitEntity.net`), pipes have no reach limit and
  the weakest pipe caps a run (ADR 0122), machines pass the line through and the newest load sheds first (ADR
  0123), hoppers push into what they face or what is under them and belts hand into what they face (ADR 0100).
- **Pipes drew too small to read**: the pipe model now fills its footprint's height and three quarters of its
  width, with a full-width flange at each end (the footprint is unchanged: 4 x 2 x 4 voxels).

## Scenes (the developer asked for every complicated system or station)
steam (boiler, pipes, a steam machine, the gap, copper caps the run, boiler care), electricity (dynamo, cable,
machine, pass-through and shedding, ratings and voltage drop, meters), connecting (face to face, corners,
diagonals, levels, an open switch), water (pump, pipes, tank, boiler), protection (battery, fuse, breaker, knife
switch, daylight switch), sources (water wheel, wind turbine, hand crank), transport (hand loading, hoppers, belts,
the chute), coke_oven (the multiblock). Every work item in the data has a guide except the hand stations (firepit,
sieve, firewood pile, anvils, forge, bloomery, rack, loom) and the rail: their lessons are about hand work, not
connections, and are the next scenes to write (the format carries them: `place` a forge, `set` its heat, `mark`
the anvil beside it).

## Checks
- `test/test_ponder.gd`: every scene loads and names only real kinds, items, materials and entity fields; every
  work item leads to a guide; the connecting rules above hold on `NetworkSolver`.
- `ponder_test=1` (an autopilot arg): plays every scene, four frames a step, into `out/<run>/ponder/<id>.png`
  (a row per step: the dots must move and the marks pulse between the frames of a row) plus one full-size frame
  of the last step, and prints `debug ponder {...}`.

  ```
  Godot_..._console.exe --path game --resolution 1280x720 --position -30000,-30000 -- autopilot=1 ponder_test=1 duration=300 view_distance_m=24 volume_master=0 out=res://out/ponder1
  ```

## Open (taste)
- Whether the stage should sit over the world (as now, the inventory dimmed behind it) or on the title screen's
  background too; a "Guides" page in the progression book listing every scene.
- The captions' numbers repeat Settings defaults (40 A, 95 %, 2000 Wh); when a default changes, the text is
  edited by hand (a template would cost more than it saves at eight scenes).
- A highlight box hides a small piece inside it; an outline would read better and is a few lines of
  ImmediateMesh when wanted.
