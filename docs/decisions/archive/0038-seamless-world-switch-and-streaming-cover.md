# 0038 – Three seams: the window between worlds, the ground in fast flight, the far planet's band

- **Status:** accepted
- **Date:** 2026-11-11

Three F9 reports of 2026-11-11, each a seam that breaks immersion. One decision each.

## A. Going between a world and the title shows the desktop (report 2026-11-11_12-54-55)

### Context
"going from world to menu and back it switches windows for a split second". Nothing re-applies the window,
the fullscreen state or the cursor grab when the pause menu opens or closes: `set_menu` only frees or
confines the pointer, and `apply_display` runs only when a display setting changed (`settings::RISKY`).
The switch is the process itself: the pause menu's Title row and the title's world rows call `start_again`,
which ends this process and starts a fresh one per world (ADR 0023's choice: nothing of the old world carries
over). The old window closed before the new process even started, so for the new process's start-up the
desktop showed.

### Options considered
1. **One process, one window, the world swapped inside it** – no seam at all, but the App, the local server,
   the streamer, the GPU buffers and the audio would all need a reset path: a large redesign, and every
   leftover is a bug.
2. **Hand over the window** – the old process keeps its window (its last frame stays on screen) until the
   new process has presented its first frame; the new process says so by writing a file the old one named
   (`shown=<file>`); the old one waits for it at most 5 s, then closes.
3. **Start the new window hidden and show it after its first frame** – would also hide the new window's
   blank first moment, but presenting into a hidden window is untested on our drivers.

### Decision
Option 2: the smallest change that removes the gap, and the save is still written before the new process
starts (the old local server stops first). `sys::wait_shown` holds the wait and has a test
(`waits_for_the_next_window`); the first frame's present writes the file (`frame.rs`).

### Consequences
For up to the new process's start-up (usually under a second) two processes hold the GPU. If the new
process fails, the old window stays 5 s longer, frozen. Option 1 stays the real fix if the seam still shows;
option 3 can follow if the new window's first moment flashes.

## B. The ground vanishes in fast flight (report 2026-11-11_13-02-00)

### Context
"flying quickly now make it so everything goes invisible, it quickly loads again": creative flight at
1000 m/s, 200 m up. The screenshot shows the near ground and the far rings, and a band between them with
nothing drawn. The LOD walk (ADR 0025 F) lets a coarse chunk cover its children while they load, but it
counted a child that splits as done ("a child that splits too is no child to wait for", so that a chunk
meshed from far away does not cover forever once the camera is back). At 1000 m/s the camera nears a
child before its mesh is made; it splits, never gets a mesh of its own (only leaves do), the coarse chunk
stopped covering, and the grandchildren, still loading, were holes. The same happened behind and to the
side: a chunk that becomes a leaf again could stand in only by its children, not by finer chunks further
down. The look-ahead (0.5 s along the velocity) hides it in a straight flight; turning defeats it.

### Options considered
1. **Count a splitting child as done only when its own children are** (recursively), and let a stand-in
   draw as deep as the finer chunks go – a few lines in `select`; the recursion stops at the first chunk
   with a mesh, and split chunks keep their old meshes while seen, so it goes deep only where a fast camera
   split chunks that were never meshed.
2. **Keep a coarse shell always drawn under everything** (a whole-planet low-detail mesh) – a new
   draw path, z-fighting with the real ground, more VRAM: a redesign.
3. **A longer look-ahead** – costs work for flights that turn, and turning is the case that fails.

### Decision
Option 1 (`Streamer::complete`, `kids_below`, `emit_below`). The test
`a_fast_flight_never_leaves_a_hole_where_a_coarse_chunk_was_drawn` flies 6 s at 1000 m/s, 200 m up, with
meshing slower than the flight: 96 holes in a frame before, none after. `run=fly speed=1000 alt=200
prefetch=0 quick=1` (no look-ahead, as in a turn), five runs each: holes_max 0/148/0/0/124 before, 0 in all
five after.

### Consequences
While its children load, a coarse chunk stays drawn a little longer than before (coarse ground for a moment
instead of none). The walk's cost grows only where chunks were split unmeshed; it is per walk, not per frame.

## C. A hard band across Terra seen from far away (report 2026-11-11_13-06-37)

### Context
"the seems should be more gradient where i zoomed in": Terra from 100 m over Luna, zoomed, shows a sharp
line across the disc where the ground steps from lit to dim. It is not a LOD ring: the geometry runs on
across it unchanged. It is a line of equal sun height, parallel to the terminator, and two terms switch
there at once in `light()` (shader.wgsl): the sun's light ends where the sun sets behind the body's sphere
(`left_by`, over the sun's half-degree disc, about 37 m of ground; sides of blocks facing the sun go from
full light to none), and the moon's light was switched on only where `day < 0.5`. On the ground it is never
seen whole; from orbit or the moon it is a hard edge. Reproduced with `run=stand at=0,1,0 eye=4000 pitch=-89
time=20837`: a straight step across the dim side.

### Options considered
1. **Fade the sun near the horizon where the body has air, and fade the moon in** – a smoothstep each; it is
   what air does (the sun dims as it sets), costs nothing measurable, keeps Luna's airless edge hard.
2. **Soften the sphere's cover over a larger angle on every body** – also blurs Luna's terminator, which
   has no air to blur it.
3. **Blend the far planet's lighting in a pass of its own** – a new pass for one line.

### Decision
Option 1: `sunset()` fades the sun's direct light from 7 degrees over the horizon to 1.7 below, on bodies
with a sea (as `sky_light` decides air); the moon's light fades in as `day` goes from 0.5 to 0.3 instead of
switching on at 0.5. Taste call for the developer: the fade's width (7 degrees).

### Consequences
On Terra's ground the last minutes before sunset are a little dimmer than before (sun under 7 degrees); the
fixed viewpoints differ by at most 0.026 % of pixels, under their limit. Two smoothsteps per lit pixel.

## Added 2026-11-11: the review, and the developer's answers
A fresh reviewer read the three commits and found no defect in the streaming or the light. On the window:
the old window waited a fixed 5 s, so a world that failed to load left it frozen that long, and one that
loaded slower than 5 s brought the flash back. The developer: "the sun dimming is fine and fix the failed
to load wait time". **Changed:** the old window now watches the new process and closes at once when that
has ended without showing anything; while it lives the wait is 30 s. A file left by an earlier process of
the same number is cleared before the wait. **Left:** a frame may be presented a moment before the screen
shows it, so a flicker of one frame is still possible; the streaming test was not seen failing by the
reviewer (its author reports 96 holes before, 0 after).
