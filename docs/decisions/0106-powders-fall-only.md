# 0106 – Powders only fall: ADR 0101's slopes, slides and glides switched off

- **Status:** accepted (the developer's call, bug 2026-11-14_01-53-32)
- **Date:** 2026-11-14

## Context
Bug 2026-11-14_01-53-32: "Just have sand and gravel fall down under them but nothing else for now. maybe in
the future we find a better fix". ADR 0101 gave loose blocks three things on top of the fall of ADR 0079: a
pile's top slides off a step of two, a loose block's open top is drawn as a slope toward lower loose
neighbours, and a block the server moves glides to its cell (with grains and a slump hiss). The developer wants
only the fall for now, and the work kept for a better fix later.

## Options considered
1. **One switch, off** (chosen) – a const that turns all three off where they start; nothing deleted, the
   tests of ADR 0101 keep running with it on.
2. Delete the slopes, slides and glides – less code, but the developer asked to keep the way back open.
3. A data flag (`data/blocks.toml` or settings) – live to flip, but a server and its clients would have to
   agree on it and nobody asked to flip it at run time.

## Decision
`engine::fall::SLOPES_AND_SLIDES = false`. It gates:
- **Slides** (`fall.rs`): `Falls::step` runs `step_as(.., SLOPES_AND_SLIDES)`; with it off a supported loose
  block never moves. A block with an empty cell under it falls straight down, as before.
- **Slopes** (`mesh.rs`): `mesh_cracked` runs `mesh_as(.., SLOPES_AND_SLIDES)`; with it off the slope pass is
  skipped, so no quad carries the sloped bit and every level meshes loose blocks as plain cubes. The shader's
  slope path stays and is simply never taken (no shader change).
- **Glides** (`client/src/frame.rs`, one condition): a server move no longer starts `fx::Glide`, so no grains
  and no cell meshed as air meanwhile; the block hops a cell a tick as before ADR 0101. The slump hiss plays
  only for sideways moves, so it is silent too.

Setting the const to true brings all three back unchanged.

Tests: `a_column_over_a_dug_hole_falls_straight_and_its_neighbours_stay` (fall.rs: a 3 x 3 x 3 block of sand,
the floor under its middle dug; every move is straight down, the eight columns round it keep their three
sands) and `a_sand_step_has_no_sloped_quad` (mesh.rs) were seen failing with ADR 0101's behaviour (a sideways
move; 520 sloped quads) and pass with the switch off. ADR 0101's own tests (the angle of rest, a step of two,
only a pile's top slides, the corridor slump; the slope corners, `held`, the flat floor at a chunk's border, no
gaps between chunks) now call `step_as`/`mesh_as` with the switch on, so they still guard that code.

## Consequences
- Sand and gravel stand in steps of whole blocks again; heaps keep any shape, a tower of sand stays a tower.
- The fixed views with sand change at beaches and deserts (`tools/views.sh`); not accepted in this change,
  the overseer's call after looking.
- Revisit: when a better fix for powders is found (finer cells, a real angle of rest), start from the switch.

## Cost
`run=fly quick=1`, play build, one run (`bash tools/slot.sh measure`, the PC shared with other agents' builds):

| | p50 | p99 |
|---|---|---|
| frame | 5.67 ms | 9.59 ms (max 26.7) |
| CPU (frame thread) | 5.47 ms (select 1.33, acquire 3.21) | 9.35 ms |
| GPU total | 5.07 ms (gbuffer 3.02, screen 1.22, trace 0.43) | 6.50 ms |
| mesh job | 658 us average, 454 us after load | |

No new work: the switch only skips the slope pass in mesh jobs (cheaper where loose blocks are) and the glides;
the frame thread and the GPU do what they did before ADR 0101.
