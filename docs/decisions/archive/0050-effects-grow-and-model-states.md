# 0050 – The effects grow, and a model's states

- **Status:** accepted
- **Date:** 2026-11-12

## Context
docs/design/studio-tools.md, "Effects" and "A model's life: states" (wave 3): the effect rows of ADR 0031 gain
colour, size and brightness over life, motion from noise, glow, lasting effects, more starts and looks, and
groups; a model file gains `[[state]]` tables and timed clip events, so a furnace is judged whole. Constraints:
every existing row draws exactly as before, the frame budget (ADR 0004), CPU over GPU, data refused by name and
reason, the Studio (another crate, being written at the same time) builds on the same types and functions.

## Options considered
- **Glow:** (1) emission only; (2) every glowing particle a lamp (thousands of lamps: the lamp grid and the
  traced lamp rays cost per lamp); (3) emission per particle plus **one lamp per lasting effect that glows**,
  through the existing lamp list, so it is lit and shadowed by the same traced rays as a lamp block. Taken: 3.
- **Looks:** a painted texture for particles as (a) a new buffer of `data/effects/` images, or (b) the
  `data/textures/` rule: the top face of a named block, painted or its pattern, from the buffer the terrain
  already reads. Taken: b (no new buffer, the Studio's Textures page paints it). A model look is drawn by the
  models' own draw (one more instance list), not by a second model path in the effects' shader.
- **States in the model file:** fields on `Model` and `Clip` would break the Studio's exhaustive struct
  literals (`Model { .. }` in studio/voxel.rs, `Clip { .. }` in studio/ui/models.rs), which this change may not
  edit. Taken: the life is read from the same file into a separate `Life` (`parse_life`), `Model::parse` takes
  the `state` and `event` keys out before its strict read, `write_life` puts them back.

## Decision
**Effects** (`crates/client/src/fx.rs`, `data/effects.toml`, `vs_fx`/`fs_fx`/`fx_colour`/`fx_out` in shader.wgsl):
- New row fields, each optional: `rate`, `start` ring (`radius`) / cone (`angle`, `dir`) / box (`box`),
  `gradient` (palette names, `engine::palette` through `fx::PALETTE`), `size_curve`, `bright_curve`,
  `noise = { strength, scale, speed }` (smooth value noise, one hash a lattice corner for three channels),
  `rise`, `spread` (a steady push each particle keeps), `glow`, look `model` (`model`) and `texture`
  (`texture`), `face` camera / flight / tumble; `count` now optional. `[[group]] name, rows`.
- The GPU particle keeps its 32 bytes: `info` bits 16-17 the face, 18 the texture, 24-31 glow x 32; a flight
  square's `a` is its direction (3 x 10 bits) and `angle` its streak (a 30th of a second's flight); a tumbling
  square is the debris cube a tenth as thick. A camera-facing square turns by its `angle` (its spin).
- The pool owns its `Set` (rows, groups); `fx::id` keeps naming the built-in rows and `play` maps them to the
  set's, so every caller is unchanged. `Fx::start(name, at) -> Option<Handle>`, `moved`, `stop`, `restart`,
  `play_named`; lasting rows emit `rate x scale x dt` a step (a fraction carried). One lamp per glowing handle
  (its rows' glows added, mean gradient colour x glow x 0.25, flickering by the noise when the row has noise),
  added to the frame's lamps after the world's (frame.rs).
- Live reload: `Fx::watch` puts `data/effects.toml` on a `gpu::watch` thread; the frame thread swaps the set in
  at its next `step`, living particles and lasting effects keep their rows by name; a refused file, or one
  without a row the code plays (break, place, grow, spray, splash, chip), is named on stderr and the old rows
  stay.
- `fxplay=name[:n]`: n lasting copies on the ground ahead (a fan, as `model=` places its models).
- Draft rows (data/drafts.txt): `flames`, `embers`, `smoke` and the group `fire`; `sparks`, `steam`, `dust`,
  `drip`, `glint`, `leaves` (look model `leaf`, a new draft `data/models/leaf.toml`).

**States** (`crates/engine/src/models.rs`, `crates/client/src/models.rs`):
- `Life { states, events }`, `State { name, clip, effects: [{effect, point}], sound, light: {point, colour,
  strength} }`, `Event { t, sound, effect, point }`; `parse_life(file, text, palette, effect?, sound?)` refuses by
  file and reason a state or event naming no clip, point, effect or sound, a light colour not in the palette,
  an event outside its clip or with nothing to play; `write_life`; `events_between(clip, events, t0, t1)` (each
  event once a pass of a loop: at t, t + length, ...; once in all otherwise); `Model::point_at(poses, name)`.
- Client: `models::load` reads the life against the effects and sounds files as they are; `model=name@state`
  and `Models::set_state(cell, state)` (a placed model block) put a model into a state: its clip plays from 0,
  its lasting effects start at their points and follow the parts (`Fx::moved` each frame), its looping sound is
  held (`Audio::hold`) at the nearest one's loudness by its reach, its light is a lamp at its point, and its
  clip's events play once a pass. A state no longer asked for stops its effects (their particles live out).
- The draft furnace: points `chimney` and `door`, state `working` (clip `work`, fire at `flame`, smoke at
  `chimney`, `furnace.burn`, a `lamp` light of 0.7), events at t = 2.0 (the door shut): `furnace.clank` and
  `sparks` at the door (light 0.7: at 1.5 it washed the furnace's own faces out at dusk).

## Measured
Release, dev PC, `quick=1`, three runs each; "before" is HEAD built from a clean export (the same Cargo
profile), "after" this change; the frame thread's `fx` stage and the GPU's `fx` stage, ms p50 / p99:

| run | live | frame thread fx | GPU fx | frame p50 / p99 / max | budget_ok |
|-----|------|-----------------|--------|-----------------------|-----------|
| `run=fx` before | 4087 | 0.055-0.070 / 0.090-0.105 | 0.014-0.015 / 0.017-0.029 | 1.42-1.44 / 1.99-2.15 / 2.6-3.5 | yes |
| `run=fx` after | 4087 | 0.059-0.061 / 0.086-0.094 | 0.015-0.018 / 0.018-0.019 | 1.39-1.45 / 1.92-1.98 / 2.4-3.0 | yes |
| `run=fx fx_n=0 fxplay=fire:50` | 2671 | 0.193-0.194 / 0.246-0.247 | 0.011-0.013 / 0.016 | 1.44-1.48 / 1.98-2.01 / 2.4-3.9 | yes |

The old rows cost what they did (inside the runs' spread). Fifty fires (150 lasting rows, 2,671 particles, two
thirds of them through the noise field, 50 lamps) spend 0.19 ms of the frame thread: the noise is most of it.
shortcut: the noise is sampled per particle per step on the frame thread; at hundreds of fires, sample it at
half rate or move the step to a worker (ADR 0031's own shortcut). VRAM 817-1073 MB in both (it moves between
runs as before). `tools/views.sh`: all six views pass (effects are off there).

Filmstrips (read): `out/fx50/fire-750.png` and `fire-750-4.png` (night: orange flames, dark smoke curling up, the
wall beside it lit warm by the fire's lamp), `out/fx50/fire-0.png`, `fire-0-4.png` (day: grey smoke column, pale
orange flames), `out/fx50/furnace-dusk.png` and `furnace-zoom.png` (the door swinging, smoke from the chimney and
the fire, the warm light; `furnace.clank` played 5 times in the run's JSON), `out/fx50/leaves.png` (leaf models
tumbling down and lying on the grass). The first fire looked wrong and was changed: flames blown out to white
(glow 3 from "lamp light"), black smoke squares, a light that washed the wall out; now flames run lamp -> copper ->
copper dark at glow 1.5, smoke stone dark -> stone and smaller, the lamp a quarter of the glow.

## Checks
Tests, each seen failing first (Part A's against deliberate faults put into the finished functions, Part B's
against stubs): fx.rs — the old rows draw exactly as before (their drawn bytes over two seconds hashed, the
hashes taken from the code before this change); curves and gradients; the noise field the same for the same
place and time, continuous (a millimetre or a millisecond moves it under 0.02) and varied; a rate's count over
time at two scales, stop, a group's rows as one, its one lamp, moved, restart; each start's shape (ring, box,
cone); every new refusal by row and reason; the drafts present and listed, a reload that keeps what plays by
name and refuses a set missing a code row. models.rs — a life reads, refuses by file and reason, round trips
through `write_life`; events once a pass over ten seconds in frames of 1/60, 1/7 and 0.3 s, and once for a clip
that does not loop; a point moves with its part. `cargo test -p client -p engine` green.

## Consequences
- shortcut: only lasting effects give a lamp; a burst of sparks glows itself but lights nothing round it.
- shortcut: a state's looping sound has no direction (`Audio::hold` is a bus-wide loop): the nearest one's
  loudness by its reach. Pan it when the mixer can hold a loop at a place.
- Particles are opaque in the G-buffer: smoke is puffs of grey squares, not a soft haze. A blended pass is a
  later choice of its own.
- The Studio must save models through `write_life(&model, &life)` (with `parse_life`) or a saved model loses its
  states and events: `Model::write` alone writes the model only.
- Placed blocks are put into a state by code (`Models::set_state`); no game rule does it yet (machines will).
- Not reviewed by a fresh reader yet: no commit was made in this session.

## Review
A fresh reviewer read the uncommitted game side (2026-11-12). Found: a model reloaded while it played a state kept
the state's old lasting effects (a file with that state's effects changed or removed burnt on as before), and a
reload that left no model at all (every file refused) kept every state's effects emitting and its held sound
humming, unseen. Fixed in `Models::frame`: after `take` swaps models in, each state's effects stop and start again
from the new file (the clip's time is kept); with no models, every state stops and its sound is let go. The pose
work no longer writes the GPU buffer itself (`Models::upload`), so the test drives it without a GPU
(`a_reload_or_a_refused_model_stops_the_old_states_effects`). Not fixed, reported: the game's sounds start from
the rows compiled in, so a `data/sounds.toml` edited after the build is heard only once it changes again, while
models check their events against the file on disk.

Second fresh reviewer (2026-11-12, the whole `fx.rs` diff with `vs_fx`, `fx_colour`, `fx_out`, and the parts
written after the first review). Fixed: a particle with a model look facing the camera or tumbling was turned by
`Mat3::from_cols(s, up, t)` with `(s, t) = up.any_orthonormal_pair()`, a determinant of -1: a mirror, so the
leaf (and any model particle) was drawn inside out under back-face culling. Now `(t, up, s)`; test
`a_model_particle_is_turned_never_mirrored`, seen failing at -1. Not fixed, reported: `Fx::restart` bursts again
only the rows that have a rate (only those are emitters), so restarting a lasting effect whose group has a
count-only row loses that row's burst, and a count-only effect restarts to nothing (only the tests call it now);
`models.rs` checks a state's effects and sounds against `data/effects.toml` and `data/sounds.toml` even when
`effects=` or `sound_dir=` names other files. Read and found sound: the packing of face, texture bit, glow and
the flight direction against the shader's decoding, the streak's geometry, the cone (uniform over the cap), ring
and box starts, the rate's accumulation (dt is held to 0.1 s by the caller), the curves and gradients (no
division by zero: the stop found is strictly past x), the `then` queue (bounded), the swap's remaps, the lamp cap
(`cap_lamps` keeps the nearest of all), the texture and model keep-last-good maps (a deleted file is dropped).

## Addendum (2026-11-12)
- `Fx::start` keeps an emitter for a count-only row too (rate 0: `emit` passes it by), so `restart` bursts it again.
- A state's effects and sounds are checked against the files in use (`effects=`, `sound_dir=`), not `data/`'s.
- A clip with events and no state (`model=boar:walk`) fires its events, as a state "" holding only that clip.
