# 0054 – Cave reverb from rays at the head

- **Status:** accepted
- **Date:** 2026-11-12

## Context
The roadmap's "3D audio with cave reverb and material sounds" (step 1). Two of the three were already
built (ADR 0032): sounds have a place against the head (pan, the far ear late, quieter with distance and
gone at the row's reach, duller behind; the head's frame is the player's right, up and ahead on the planet),
and digs, places and steps play `dig/place/step.<sound>` by the block's material (`data/blocks.toml`,
held by `every_block_has_its_sounds`). Missing: the cave reverb, `docs/design/sound.md`'s "second cut":
a few rays from the listener measure how closed in the place is and how big, and that sets one shared
reverb. No new dependency: ADR 0032 chose our own mixer for exactly this.

## Options considered
1. **Rays at the head on the frame thread, a timer, one streaming reverb on the card's thread** – 14 rays
   (the frame's six ways and eight corners) in 1 m steps out to 24 m, four times a second; the mixer gets
   one message when the place changes. Small, testable without a card.
2. **The rays on a worker** – no frame time at all, but a copy of the world or a lock for a cost that is
   measured in microseconds; not needed now.
3. **A reverb per voice (the `reverb` chain step at play time)** – a filter per sound, many times the CPU,
   and a sound that started outside would not ring when you walk in.

## Decision
Option 1. `crates/client/src/audio.rs`:
- `room(solid, ear) -> (wet, size)`: the rays stand on the body's up (not the look), each out to the face
  of a cube 24 m from the head (a corner ray √3 times as far); the share that meet a solid cell (the block's
  `solid`) under half (a field: only the ground's five) gives no reverb, all of them full; `size` the mean
  distance of the sideways and upward hits.
- `Reverb`: the Freeverb shape of `chain::reverb` (four damped combs, two all-passes) run sample by sample,
  each ear's 23 samples longer; rings 0.4 s (a burrow) to 3 s (a 24 m hall); at most half as loud as the
  sound. Only the World and Steps buses feed it; the interface and music are in the head. Its share eases
  in and out (no click); fully out it is not run and its buffers are cleared.
- `Msg::Room { wet, size }`, `Audio::listen` (called each frame in `frame.rs`, measures every 250 ms, sends
  on a change of 0.02 or 1 m), and `"room":{"wet","size","us_max"}` in every run's `audio` JSON. No rays
  without a card (an off-screen run measures with `sound=1`) or more than 24 m over the build headroom, the highest place a block can be (high flight,
  orbit): there the place is the open.

## Measured
Dev PC, release build, 2026-11-12, `run=stand quick=1 sound=1 at=0.102815,0.769509,0.630306` (on land;
the default stand point is on the sea, 35 m over its floor, so no rays there).

| What | Number |
|---|---|
| Tests | `rays_tell_a_cave_from_a_field`, `a_cave_rings_on_after_the_sound_and_a_field_does_not`, `a_big_hall_is_as_wet_as_a_tunnel_and_bigger`, `looking_up_or_down_does_not_change_the_room`, `no_rays_with_nothing_to_hear`, `the_reverb_dies_away_and_a_bigger_room_rings_longer` |
| Open ground | `room` wet 0.00, size 15.0 m (something to the side or above), longest measure 174 µs |
| The same in a 7 x 7 x 5 stone box (`place=`) | wet 1.00, size 4.4 m, longest measure 65 µs |
| The same without `sound=1` | no rays (`us_max` 0) |
| Frame thread | one `room` every 250 ms with a card: at worst about 0.17 ms, under 0.05 ms per frame on average |
| Mixer, on the card (44.1 kHz headset) | `mix_us_max` 32-41 µs per buffer in these runs (no world sounds playing; the reverb idle in the open) |

## What the review found (2026-11-12), and what was done
| Finding | Done |
|---|---|
| Corner rays reached only 13.9 m: a big hall came out drier than a tunnel, the 3 s hall unreachable; the floor pulled `size` down | rays go to a 24 m cube's face; `size` from sideways and upward hits only (`a_big_hall_is_as_wet_as_a_tunnel_and_bigger`: red wet 0.43) |
| The ray frame tilted with the look's pitch | the frame is built from the body's up alone (`looking_up_or_down_does_not_change_the_room`: red) |
| Leaves and plants count as rock | a block row's `soft = true` (leaves, shrub, reed) is no wall to the rays (`plants_are_soft_and_ground_is_not`: red) |
| Rays paid for with nothing to hear | none without a card or more than 24 m over the build headroom (`no_rays_with_nothing_to_hear`: red; first gated by the ground, which heard a tower's room or a hut over deep water as the open: `only_over_the_build_headroom_is_there_surely_air`, red) |
| The test's bound check ran after tanh | the reverb's own output checked, its feedback under 1 at the smallest and largest size, a bigger room rings longer (passed at once: a missing check, not a bug) |
| The turned-ray test passed even if `room` ignored `ear` | a field with x up, which must give no reverb and no size (red: size from the floor's hits) |
| A size change stepped the feedback while it rang | the feedback eased per sample (in the reverb test: red "no step") |
| Denormals as the combs ring down in silence | a 1e-20 floor on the comb input (in the reverb test: red, a denormal at sample 1601, the comb's low-pass state) |

## Consequences
- Not built: sound behind a wall duller and quieter (sound.md, one ray per voice), water, machines.
- shortcut: the rays treat water as air and cactus and coral as walls; refine
  when it is heard.
- The height gate is the build headroom (`Body::over_top`), so flight under it (to about 180 m over the sea
  level) casts the rays too: up to 0.17 ms once every 250 ms on the frame thread.
- shortcut: `room()` runs on the frame thread four times a second; move it to a worker if its `us_max`
  ever shows in the frame budget.
- The reverb is one for all sounds: a sound in a cave heard from outside rings by the listener's place,
  not its own.
