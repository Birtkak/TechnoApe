# 0010 – Simulation bubble polish: shift, load/push budgets, fire pacing

- **Status:** accepted
- **Date:** 2026-10-22

## Context
CLAUDE.md listed "spikes on shift" and "fire-spread tuning" for the simulation
bubble. ADR 0009 sets the bar on the reference PC: p99 frame ≤ 8.3 ms and no
frame over 16.7 ms after load. The bubble now reports a per-frame cost
breakdown (`sim.worst_frames`) and Godot's monitors for every frame over 6 ms
(`sim.slow_frames`) in the autopilot JSON, so each step below is measured.

## Measurements and decisions

**Window shift (7³ chunks).** `SimWorld.shift` copied the cells into freshly
allocated arrays (8.4 MB) on every one-chunk move: 2.0–2.4 ms. Now one
in-place `memmove` by the linear offset plus zeroing the vacated chunks;
`moved_` holds per-tick stamps and is not shifted. **0.34–0.47 ms.** Tested in
all six directions, diagonals and beyond the window (`test_sim_shift.gd`).

**Chunk loads.** The 49 vacated chunks were loaded in one frame (~0.7 ms).
Now a time budget, `sim_load_budget_ms` = 0.3 (at least 4 chunks per frame);
waiting chunks act as walls in the simulation, as before.

**Pushes.** Dirty chunks go into a queue in world coordinates and are pasted
oldest first within `sim_push_budget_ms` = 1.5; on a shift only chunks
leaving the window are flushed. `sim_push_interval_ms` (default 0) can cap
how often one chunk is pasted.

**Fire pacing** (`tools/fire_bench.gd`, 30 Hz). A burning voxel ignited a
random neighbour with a fixed 1/8 chance, so grass (`fire_hp` 10 = 0.33 s)
died before passing the fire on: grass fires never spread. The chance is now
per material, `clamp(10 / fire_hp, 1/8, 1)` (data may set `ignite_chance`):
thin fuels catch easily, logs slowly. With plant `fire_hp` 40 (materials.gd):

| Scene | Result |
|---|---|
| Grass meadow 48×48 m | front ~1.4 m/s, burnt in 28 s, 1 tuft left |
| Plank wall 16×8 m | burnt in 29 s |
| Tree (8 m trunk, 9 m canopy) | burnt in 16 s |
| Wood beam beside water | fire stops at the water |
| One-voxel wood line | may go out part-way (as a lone fence line would) |

Kernel cost stays below 0.7 ms per tick and ~30 dirty chunks per tick.

**Where frame spikes really come from.** A burning forest in game (527 voxels
lit) gave p99 11.7 ms with frames up to 18–40 ms, while the bubble itself used
under 1 ms per frame. Variants on the same scene:

| Variant | p95 | p99 |
|---|---|---|
| default (32³ mesh blocks) | 8.3 ms | 11.7 ms |
| 16³ mesh blocks | 3.3 ms | 5.6 ms |
| push each chunk at most every 250 ms | 2.9 ms | 8.3 ms |
| terrain collisions off (experiment) | 2.8 ms | 3.1 ms, no frame > 6.4 ms |
| collision_update_delay 200 ms (experiment) | 2.8 ms | 5.0 ms, frames up to 20 ms |

godot_voxel rebuilds a mesh collider on the main thread for every re-meshed
block; that, not the simulation, breaks the budget. Defaults stay (32³
blocks, interval 0): 16³ blocks quadruple draw calls (389 → 1504) and raise
the flight p99 (2.8 → 5.1 ms), and a 250 ms interval makes water and fire
step visibly.

## Consequences
- Next step (own change): collide the player and item entities against the
  voxel grid (godot_voxel `VoxelBoxMover`, Minecraft-style AABB moves) and turn
  terrain mesh colliders off or limit them, so churn never rebuilds colliders.
  It touches player movement, so it is coordinated with the player-body work.
- `fire_test=N` (autopilot) ignites burnables around the player;
  `tools/fire_bench.gd` reports pacing and cost; both stay as regression checks.
- Smoke and flame voxels currently render as opaque grey/orange cubes and get
  colliders; gases should get no collision when the collision change lands.
