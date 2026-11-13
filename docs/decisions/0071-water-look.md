# 0071 – The water's look: clear, traced, on the pixel grid; and blocks set into water

- **Status:** accepted (built)
- **Date:** 2026-11-13

## Context
The developer: "water still looks ugly". The sea, rivers and lakes drew as a flat, dark blue, nearly opaque
sheet (`out/views/coast.png`, bug 2026-11-12_21-00-30). The look chosen: **traced and clear, still on the pixel
grid**: the bed seen through shallow water and fading to blue with depth, the sky, Sol, Luna and the near terrain
in it with a Fresnel term, small ripples that read as 16 px a metre, a river's ripples running with its flow,
foam at the shore and down falls, one calm blue with the sun's glint from orbit and no seam between levels, a
blue fog under water, and a setting so weaker PCs keep the old sheet. The water's mechanics (ADR 0020, 0059,
0067) stay; this is the client's drawing. Added during the work, also from the developer: "water also behaved
weird when you placed blocks inside it".

## Options considered
1. **Screen-space**: read the depth buffer behind the water for its thickness, reflect what is on screen. Cheap,
   but it needs the water pass to sample the depth it tests against (a copy), and reflections of what is off
   screen are missing: the classic smeared edges.
2. **Traced** (chosen): the water pass already traces a sun shadow ray (`light`); one more ray down the view gives
   the way through the water to the bed exactly, at any level; one along the reflection sees the terrain
   (`seen`, the metal's reflection, ADR 0037). Costlier, and it needs the ray tracing the game already requires.
3. **Shore foam from four short rays across the surface** to the nearest bank: built first and dropped. A beach
   shelves in whole steps under 0.4 m of water, and within a metre of the visible edge the rays met no bank
   (filmed with the hits as colours). The depth at the view ray's hit is free and finds the same shallows.

## Decision
- **`fs_water`** (shader.wgsl, "the water's look"):
  - **Ripples:** three octaves of value noise over the face's grid coordinates (columns, which are metres),
    passed from `vs_sea` and `vs_water` (`WOut.grid`, `face`), evaluated at the middle of each 1/16 m texel, so
    a ripple's facets are the blocks' own pixels. They run on the **sky's clock** (`Globals.water.x`, world time
    mod 3600 s: a `run=view` shot is the same every time), and a river's along its flow (`WOut.gflow`, columns a
    second, from the reach's fall). The clock **steps** 4 times a second (`WATER_STEPS`), so ripples, foam and falls
    jump like pixel art instead of gliding (the developer's call, 2026-11-13). They **calm** where a pixel covers more than 4 cm of water and are gone at
    40 cm, so far water is flat: from orbit one blue, no shimmer and no level seam (both levels shade from the
    same grid coordinates and the same rule).
  - **Clear:** a ray down the view (up to 16 m; nothing met counts as its reach) gives the way through the water; it takes `exp(-k len)` of the
    light, k = (0.45, 0.12, 0.075) a metre (red goes first). Blended over the bed already drawn: alpha is what of
    the bed does not show, the colour the water's own light (`WATER_FOG` lit by sun and ambient) plus the
    reflection, so shallows show sand and stone, depth turns them blue.
  - **Reflection:** Fresnel with water's 2 % head-on. Within 250 m, and only where the Fresnel term is over 8 %
    (looking down into water the reflection is too faint to pay for), the terrain is traced up to 150 m and lit
    without shadow rays (`seen_lit`: sun by the face's facing, ambient), fading into the sky over the last 50 m
    and between 8 and 16 %; past it the sky alone (`sky_colour`, which has the sun's disc). Farther, the far
    chunks' coarse cells stand a layer over the sea at the coast and a grazing ray met them. Luna is added where
    the reflected ray meets its sphere, lit by its phase. The settings' Reflections off leaves the sky only.
  - **Glint:** the sun's specular, sharp (exponent 900) on near ripples and wide (60) where they have calmed, so
    the far sea and the sea from orbit carry a sheen around the sun's reflection.
  - **Foam:** a broken line where the depth at the view ray's hit (way through times the cosine) matches a level
    that laps in and out between 0.05 and 0.35 m, in texels, near water only; on every front and fall (`side`
    quads) in streaks running down; on rapids by speed.
  - **The sea behind the horizon** is not drawn when the camera is out of the water: water writes no depth, and
    the far side of the sphere's sea showed through the near sea as a dark band along the horizon. A fall seen
    from behind is drawn as its face; the discard comes before any ray.
  - **From under the surface:** the sky through a window overhead (Snell's), the deep around it, fogged.
- **Under water** (post.wgsl composite): with the camera's eye in water (`World::water_surface`), everything dims
  to (0.55, 0.8, 0.9) and fades into `WATER_FOG` times the day's light at 0.09 a metre (most of it by 12 m); the
  old flat blue veil over the screen (play.rs) is gone.
- **Setting:** Graphics, **Water**: off (the old sheet, `water_old`, kept as it was), simple (ripples, Fresnel,
  the sky in it, a guessed 3 m depth; no rays of its own), **traced** (the default). `g.opt.x` bits 4-5.
- **Blocks set into water** (engine `liquid.rs`): the bug was in the mechanics, not the drawing. A test
  (`a_block_placed_in_the_sea_and_dug_leaves_the_sea_as_it_was`: a block at the sea's surface layer and one on
  its bed, each settled 30 s, then dug and settled again) failed for two reasons, both fixed:
  1. *A puddle on the block:* the water a block displaced rose into the gap above it, which stood over the sea's
     level, and stayed there as a thin sheet (401 units, a tenth of a layer) drawn on top of the block for
     good. Now displaced water whose gap above stands over the column's generated open water joins that
     water's account (`book(reach)`: the sea's, as a forgotten splash does), only the part under that level
     and not in a river, and the ledger stays exact.
  2. *The sea under the block swung and drained:* a closed gap's head rises K = 16 times as fast as the water it
     takes in. With the explicit step and wide pipes, the full gap under the block took in a wave, shot it out at
     16 times the height, and swung for good: water pumped out over the block and the gap was left a third
     empty, 7 layers under the sea next to it. Now a link into or out of a closed gap passes at most what would
     level the two heads (half the difference, or the room plus the rest over K + 1 once the gap is full),
     shared over the gap's links. The gap settles in a step, full, with the pressure share
     that matches the sea (head 208.77 against the sea's 208.78).
  After a dig the column closes and is forgotten (generation's sea again), the count exact throughout.

## Measured
Play build, dev PC, `run=stand` filmstrips (2.5 s), GPU p50 of the screen pass (where the water is drawn):

| View | old sheet | simple | traced |
|---|---|---|---|
| Lake, seed 1337, `at=0.078043,0.849093,0.522447 eye=6 pitch=-30`, noon | 0.82 ms | 1.29 ms | 2.65 ms (first try 3.39) |
| Beach, seed 480895600, `at=-0.457181,0.679724,0.573551 yaw=60 pitch=-20 eye=6`, noon | 0.43 ms | | 0.76 ms |
| The same beach at sunset | | | 1.53 ms |
| Orbit over it, `eye=2500 pitch=-89` | | | 1.48 ms |
| `run=fly quick=1` | | | 0.27 ms |

The worst view (the lake filling the screen, at sunset) took 3.9 ms of a 6.3 ms GPU frame, inside the 8.3 ms
budget. The water pass's CPU cost is none (one uniform). Filmstrips read: the beach at noon, sunset and night,
the river by the spawn at noon and sunset, the lake at noon, sunset and night, orbit. Shallows show the bed and
deepen to blue; trees are mirrored in the lake; the river's foam laps along its banks; from orbit the sea is one
calm blue with the sun's sheen and no seam.

Gate (fast, after merging main ad0da8f9): `stand`'s GPU p50 2.90 ms with water traced against 2.70 with it off
(the same binary back to back): the water's cost there is 0.2 ms. The rest over the baseline (2.28) is main's clouds.
The `sim` tick's p99 was noise from other sessions compiling: main's build 3.7 to 12 ms, this branch 2.5 to 2.6.

Reviews: a fresh reviewer read each round. The first found falls dark from behind, the discard after the rays, the
clock's wrap, foam twinkling on still water, booking poured water, and heads not levelled: all fixed but the wrap
(below). The second found reflections sunlit under a roof and bottoms lit as tops, near shallows turned deep at
grazing views (the far floor went by footprint, now by distance), and a contour where the view ray's reach ends:
all fixed. It also noted the surf moves a little with the view's tilt: left, a look call.

## Consequences
- The water pass is the frame's dearest after the G-buffer near water: two rays a pixel more than before (view
  and reflection). The setting's simple and off rows give it back.
- Ripples and foam use the face's grid: a cube face's edge is a line where the pattern does not match. The water
  is calm there at any distance and it is hard to see. Shortcut: per-face coordinates; a seamless 3D noise when
  it shows.
- Far water is not hazed into the air as far ground is (the water is drawn after the composite); it was not
  before either.
- The ripple clock is world time mod 3600 s, so the pattern jumps once every two game days. Shortcut: a float
  clock loses texel precision over hours; a wrap matched to the pattern's period when the jump is seen.
- The clamp on closed gaps changes how any water under a ceiling moves (caves flooded under pressure settle
  sooner, never swing); every liquid, world and server test still passes.

## Calls for the developer to confirm
- Foam is a thin broken line that laps between 5 and 35 cm of depth, not surf over the whole shelf.
- The Low preset does not set Water; Medium and below could take simple.
- The water's colours (`WATER_FOG`, the absorption) and how fast the fog under water closes (12 m).
