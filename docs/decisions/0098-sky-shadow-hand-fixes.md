# 0098 – Sky, shadow and hand fixes

- **Status:** accepted
- **Date:** 2026-11-13

## Context
Four F9 reports from one play session (2026-11-13):

1. `14-24-14` "grain on shadows from normal blocks": a block's sun shadow had a grainy penumbra. The sun's
   shadow is one ray a pixel to a point of the sun's disc (ADR 0022); the point was picked by a white-noise
   hash of the pixel, so neighbouring pixels took unrelated points and the penumbra was salt and pepper.
2. `14-26-57` "clouds go inside the planet": at 91 m over the sea, cloud heaps were drawn below the limb. The
   clouds pass (post.wgsl `clouds`) ends a ray at the G-buffer's distance, but the sea is not in the
   G-buffer; past the ground drawn there (far sea, chunks not yet loaded when flying fast) the ray ran on
   through the planet and met the cloud slab on its far side.
3. `14-38-44` "clouds a tad less fuzzy and a bit more open; the highest peak should reach the clouds".
   The slab was 300 to 364 m over the sea; the 4 km planet's highest land is 120 m (bake::land
   `mountain_m`; seed 641653600's highest column 117.6 m).
4. `14-31-01` "the block in hand should look like the placed block; a held light source should give light".
   The hotbar, inventory, "looking at" and in-hand cubes drew a painted block as its painting's mean colour
   in noise tones, so the lamp in hand looked like sandstone; nothing lit the world from the hand.

## Options considered
1. Grain: (a) more shadow rays a pixel: smooth, but each one is a ray through the scene for every pixel;
   (b) a separate sun-visibility target filtered in the composite: clean, but a new target, a pass and the
   sun's term split out of the direct light; (c) the disc point by a low-discrepancy pattern of the pixel
   (interleaved gradient noise for the angle, the R2 sequence for the radius): neighbours take far-apart
   points, no cost; (d) (c) and the composite smoothing the direct light's light, without its albedo, over
   the 3x3 around each pixel where the neighbours lie on the same surface (post.wgsl `direct_smooth`, the
   bounce's `edge_w`), the albedo put back: the paint stays sharp, the dither becomes a gradient. (c) alone
   was measured first: the edge was still visibly dithered at 2560 x 1440. Chose (d).
2. Clouds through the planet: end every cloud ray at the sea's sphere. Exact (no cloud is under the sea
   level) and a few instructions.
3. Peaks into the clouds: move the slab down to 100 to 164 m over the sea (a terrain-relative base would
   follow every hill; the request was the highest peaks only, and the bake stays as it is). Crisper: a
   filled cell stops light faster (CL_K 0.09 -> 0.14) and its soft edge is half as wide; more open: every
   cell's number is raised by 0.06 (`CL_OPEN`), in the clouds and their shadows alike. Lightning strikes
   from the new base. Clouds this near showed their march as grain (8 m steps across 16 m cells, a white-noise
   start per pixel): the steps are now 4 m near the camera (still capped by the Clouds setting's steps) and the
   start is interleaved gradient noise.
4. Hand: the texture watcher keeps each painted block's top and side (first variant) beside the means;
   `hud::cube` draws them texel for texel when the cube is large (the block in hand, 16 x 16) and 8 x 8
   means of them when small (hotbar, inventory, "looking at", so the HUD's rectangle count is unchanged).
   A held block whose row gives light adds one lamp to the frame's lamps, 0.4 m ahead of the eye and
   0.3 m under it, in the row's light colour (through `cap_lamps`, so it counts against the Lamps setting).

## Decision
As chosen above. Taste calls made by default: the cloud base at 100 m (the highest peaks reach 18 m into
the slab), `CL_OPEN` 0.06, CL_K 0.14; the held lamp as strong as the placed one.

Evidence (out/ of the worktree, `play` build): the shadow's edge at 2560 x 1440 grainy before, a clean
gradient after (`cmp-shadow3.png`); clouds under the limb from 20 m over the sea before, none after
(`cmp-day3.png`); the highest peak (117.6 m, at=0.711685,-0.662309,-0.234201) stands in grey cloud fog
(`f-peak6-after3.png`); the lamp in hand and in the hotbar in its painting, the ground lit warm round the
player at midnight (`cmp-hand2.png`). Tests: `hud::tests::a_painted_block_s_cube_shows_its_painting`,
`gpu::lamp_tests::a_held_lamp_gives_light_from_the_hand` (both seen failing first).

Cost, `run=fly quick=1`, two runs each: GPU p50 4.72 / 4.89 ms before, 4.75 / 4.76 after; the `screen` stage
(composite and clouds) 1.03 / 1.06 -> 0.82 / 0.83 ms (the 3x3 costs less than the clouds no longer drawn
through the planet and the more open sky save); frame-thread CPU unchanged. No speed-debt row.

Review (a fresh reviewer read the diff): (1) glowing blocks write albedo 0 and grazing metal nearly 0, so
their light over albedo leaked bright rims into neighbours in the 3x3: such pixels now keep their own light and
count for nothing in a neighbour's; (2) the held lamp in the inertial frame (between the bodies) indexed past
the cameras and would panic: none there now; (3) cutout paintings' empty pixels darkened the cubes and a fine
face's variant 0 is only a quarter: the empty pixels count for nothing (an all-empty texel keeps the tone) and
fine faces keep their tones; (4) a held lamp right at a wall can sit inside the block: accepted.

## Consequences
- Other players' held lights are not shown: the protocol does not carry what a player holds. Adding it is
  a field on the move message; left until players are drawn holding things.
- The cubes show summer's paint only (a season's own paint shows in the world, not on the cubes).
- The 3x3 smooths the lamps' soft shadows the same way, and a metal's glint over three pixels. A penumbra
  wider than the filter's reach is still dithered within it; option (b), or a wider smoothing, is the way on
  if that shows.
- Clouds now hang where flyers and hilltops are: inside a cell the march is the fog.
- The fixed views (all `weather=clear`) change a little everywhere a shadow's edge or a lamp's light is: the
  direct light's 3x3.
- ADR 0076's slab height (300 m) and step (8 m) are superseded by these.
