# 0119 – Body shadows: the players' bodies as shadow proxies in the traced scene

- **Status:** proposed (built on `claude/body-shadow`; waits for the review and the developer)
- **Date:** 2026-11-14

## Context
The developer, 2026-11-14: "add body shadow". Models are not in the traced scene (ADR 0114), so a player casts no
shadow: not under the sun, not from a lamp or a torch (ADR 0117). The light is ray-traced (ADR 0022, 0037, 0108);
the TLAS is sized to need and built when its instances change (ADR 0111).

## Options considered
1. **The ape's model in the traced scene**, animated: the true shape, but a BLAS a player a frame.
2. **A shadow proxy:** a few boxes from the ape's parts at rest, one small BLAS for every player, one TLAS
   instance a player, seen by shadow and lamp rays only. Limbs do not move in the shadow.
3. **A blob under the feet** (as ADR 0114's contact shadow under piles): no direction, wrong at a low sun.

## Decision
Option 2 (`crates/client/src/proxy.rs`).
- **The shape:** each part of `ape.toml` at rest as a box, scaled to the player's height (`phys::HEIGHT`), feet on
  the floor, front +z; a plain box until the ape has loaded. One BLAS, built again when the ape's file changes.
- **The instances:** this client's own player first (in `run=play`), then the roster's players within 64 m of the
  camera, 64 at most, each placed at its feet and turned to its look, after the chunks in the TLAS
  (`PROXY_INST + k`, mask `16 << body`).
- **Who sees it:** shadow rays and lamp rays (`shader.wgsl`); never the camera, the look's aim or the bounce.
- **Passing through:** a held lamp's rays pass through its holder's own body (the lamp's `col.w` carries the
  holder's place plus one), and the bare arm and the held item pass through the camera's own body, so the hand is
  not in the shade of the body it belongs to. Other players' bodies do shade a held light.
- **Animals:** none. Each kind would need its own BLAS and each animal an instance; see the cost.
- No wire, save or server change.

## Evidence
- Tests: `proxy::the_proxy_is_the_apes_parts_standing_in_the_players_box`,
  `proxy::one_body_a_player_in_range_and_the_cameras_own_first` (the holder's place for a held lamp in it). Written
  with the code by the first author; not seen red.
- Stills (read; `out/shadow/`, a granite floor laid over open sea with `place=`, eight bodies round the camera with
  `bodies=8`, the same run with `proxy=0` beside it): `floor.png` (9:00, eye 3 m: two dark patches on the floor,
  none without), `ground1650.png` (a low sun, eye 1.7 m: one long shadow across the floor, none without).
  **Weak:** the bodies of `bodies=` are not drawn, so a shadow stands alone; the player's own shadow in a played run
  was not caught on film (scripted mouse-look lands differently each run; the spawn's meadow is too busy to read
  a shadow on). The developer's own look in the build is the real check.
- **Cost** (play build, `slot.sh measure`, two rounds each way, `run=fly quick=1 bodies=50`, where the bodies move
  with the camera every frame):

  | | proxies off | proxies on |
  |---|---|---|
  | frame thread, `rt` p50 (ms) | 0.12, 0.18 | 0.37, 0.48 |
  | GPU `rt` p50 (ms) | 0.002, 0.001 | 0.185, 0.187 |
  | GPU p50 (ms) | 3.48, 3.57 | 3.49, 3.56 |
  | frame p50 / p99 (ms) | 3.86 / 5.24, 4.07 / 5.85 | 4.06 / 5.25, 4.24 / 5.90 |
  | TLAS builds in the run | 661, 671 | 4917, 4686 |

  So: **about 0.3 ms of frame-thread time and 0.19 ms of GPU a frame while any body in view moves**, because the
  TLAS is built again each such frame; the shadow rays themselves cost nothing measurable. A walking player alone
  (`run=play autowalk=1`, 10 s): 2,000 TLAS builds against 425. A still scene costs nothing more. The row is in
  `docs/perf-debt.md`.

## Calls for the developer to confirm
- The shadow is the ape standing still: arms and legs do not swing in it.
- Players only; animals cast none.
- 64 m and 64 bodies.
- The cost above, paid whenever a player in view moves, which in play is nearly always.

## Shortcuts
- The TLAS is built again on every frame a body moves (`rt.rs`, marked `shortcut:`): a second small TLAS for the
  bodies, or building only when a body has moved a few centimetres, is the optimisation batch's.
- Other players stand where the roster last put them (1 Hz, ADR 0117's shortcut), so their shadows step.

## Review

## Consequences
- A torch on the floor throws a passer-by's shadow on the wall; a held torch does not shade its holder.
- Animals' shadows and moving limbs wait for a cheaper way to keep moving things in the traced scene.

## Review (a fresh reader, 2026-11-15, after the merge into main)
- **Fixed:** the own body stood at the last physics step's raw place, a step ahead of the drawn camera; it stands
  under the drawn eye now.
- **Open, cost** (`docs/perf-debt.md`): the own body turns with the look, so a mouse move while standing still
  rebuilds the whole TLAS, every chunk instance written again: paid nearly every frame in play, not only "while a
  body moves". Idea: snap the yaw or the place before comparing.
- **Open, suspected, not seen:** water and particles inside the own torso's box send their sun and lamp rays out
  through its inside faces, so a swimmer looking down may see a dark rectangle at the waterline.
- **Open, minor:** the death screen keeps the body's shadow.
