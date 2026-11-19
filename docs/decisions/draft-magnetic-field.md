# NNNN – The magnetic field as a value of the world, seen as the aurora

- **Status:** proposed (planet-model.md build step 3)
- **Date:** 2026-11-19

## Context
`interior.rs` (0162-planet-interior) derives a field strength per body from its row: (liquid core share over
Terra's) x (spin over Terra's), Terra 1.0, Luna 0.0119. It was only a line in `report=1`. `deep-world.md`
section 3 spends that number (aurora, compasses, sun storms, air loss) and core mining is to lower it. This step
makes it a value the world answers, and draws its first effect, the aurora, so a player who drains the core
will see the sky change.

## Options considered
1. **Field from the metal sea's amount**, if that were world state: draining would lower the field with no further
   work. It is not: `Body.core` (ADR 0156) is a fixed level (`Some(0.0)`, the bottom of layer 0) set from the row
   and `mantle`, not saved, not on the wire; nothing can take metal out yet.
2. **Field from the row** (the interior's number), behind one function on the world that every reader asks.
   No new state, save or wire format.
3. **A field per frame on the server, sent to clients:** a wire field for a value that cannot change yet.

## Decision
Option 2.

- **Where it lives:** `World::field(b)` (`world/frames.rs`) returns `bodies[b].interior.field`. Every reader asks it
  (the renderer, F3, the run's JSON line); none reads `Interior::field` directly. It is the one place core mining
  lowers it.
- **What core mining must add** (not built): a counted amount of liquid metal per body, server-owned, starting
  at `Interior::liquid` (m^3); `World::field(b)` becomes `interior.field x (liquid left / interior.liquid)`
  (`deep-world.md`: field = liquid metal left / at the start). That amount goes in the save (a field per body,
  a save version up; old saves read it as full) and in Welcome and a delta message when it changes (clients
  draw the aurora from it), and its ledger as the ore's (ADR 0063). `Body.core`'s level may follow from the same
  amount (the hidden shells of ADR 0156).
- **The oval rule** (`sky::aurora(field)`, engine, pure): a dipole's field line from colatitude θ reaches out
  L radii with sin²θ = 1/L, and the oval sits where the lines reach the edge of the field's bubble. The bubble
  is taken to grow as the field (the real one grows as its cube root: exaggerated so a drained core shows):
  **sin²θ = sin²(20°) / field**, at most the equator, where the two ovals meet. Latitude 70° at field 1, 67.5°
  at 0.8, 57° at 0.4, 51° at 0.3, 40° at 0.2, the equator under 0.117. **Brightness = sqrt((field - 0.02) /
  0.98)**, none at and under 0.02: 1 at 1, 0.63 at 0.4, 0.43 at 0.2, 0 on Luna (0.0119) and at 0. A stronger field
  (`field=2`) draws a smaller, brighter ring.
- **The magnetic poles are the spin poles** (body z), both hemispheres.
- **The look** (`post.wgsl` `aurora`, drawn in the clouds' pass, behind the clouds, in front of the sky and of a
  planet seen from orbit): curtains in a shell 450 to 1150 m over the sea (over the clouds' 250-314 m slab), in a
  band 4° either side of the oval. The ray is marched only where it is in the shell and in the band (cone and sphere
  intervals, closed form), up to 48 steps a piece, jittered per pixel as the clouds are. Two curtains, the second
  poleward and dimmer, whose line folds with three sines of the longitude drifting with the sky's clock. A curtain
  is a row of 6 m rays along the oval and 6 m steps across and up (the clouds' blocky cells); each ray has its own
  top and lower edge, and brightens and fades on its own beat (52 to 120 s), with a wave of light running along the
  curtain. Green low (bright just over a sharp lower edge), red-violet at the top. Every motion is whole turns an
  hour of the world clock, so every client sees the same and the hour's wrap has no jump. It is light only: it adds
  to the picture and lights nothing (the lighting stays sun-only).
- **Day and night:** faded out by the daylight in the air at the camera (`day x air` from 0.02 to 0.27, a
  smoothstep: gone with the sun above about 3° under the horizon, whole by about 9° under), and per point by the sun's
  height there (from orbit only the night side's ring shows).
- **`field=x`** (the game): scales the home's field for this run, as `warp=` was a trial. The scaled value is in F3
  (World, Field: the value, the oval's latitude and brightness) and in the run's JSON line (`field`). No other option.

## Consequences
- Core mining has one function to change and the state it needs is written above.
- Left for later: the four costs of a weak field in `deep-world.md` (compasses wander under 75 %, sun storms under
  50 %, air and water leak under 25 %, field-driven machines); magnetic poles tilted off the spin poles (and the oval
  skewed toward the night side, as Earth's is); solar storms that brighten and widen the oval for a while; the
  aurora of a body other than the home (only body 0 is drawn); its sound.

## Checks
| Check | Result |
|---|---|
| Tests, red then green | `the_worlds_field_is_the_interiors` (red: the stub's 0, "Terra is the unit"; green: Terra 1.0, Luna her interior's, 0.0119); `the_oval_widens_and_dims_as_the_field_falls` (red: a fixed oval; green: 70° at 1, colatitude up and brightness down at every step from 1 to 0.15, under 62° at 0.4 and 45° at 0.2, never past the equator, none at 0 and on Luna, smaller and brighter at 2); `the_field_argument_reaches_the_aurora` (client; red: a stub; green: `aurora_post` gives the oval of field x scale for 1, 0.4, 0.2, none at 0 and by day, dimmer in twilight). `cargo test --workspace`: 975 passed, 0 failed, 26 ignored |
| Old worlds | `technoape-server report=1 seed=1 planet=6144`: `made_hash` bfb878ce14d32b75, unchanged; the hash and bit-lock tests untouched and green |
| Seen | `out/aurora-a.png`: 66° latitude, midnight, looking poleward: the oval overhead, green curtains with rays and a sharp lower edge, red-violet tops, folds that shift and rays that brighten and fade over the 2.5 s. `aurora-b.png` (`field=0.4`, looking toward the equator): the oval moved to 57°, high in the southern sky, dimmer. `aurora-c1.png`: the equator, field 1: none. `aurora-c2.png` (`field=0.15`): a dim fringe of rays low on the northern horizon (at 0.2 the oval, at 40°, stays just under the equator's horizon). `aurora-d.png`: 5 km over the south pole: the ring round the pole over the night side, gone where the sun is up. `aurora-e.png` (`time=` 720, 760, 800 at the view of a): none at dusk, faint as the sky darkens, whole by about 10° under |
| Views | `tools/views.sh` against main's baselines: ok, all twelve, no cracks, largest diff 0.257 % (orbit) and 0.244 % (winter), as before: the views are by day and show no aurora |
| Cost | `docs/perf-debt.md`: GPU p50 +1.9 to 2.0 ms where the oval fills half the screen at night (2.42 -> 4.3 ms), frame p99 4.3 -> 6.1 ms; none by day or at `field=0` |
