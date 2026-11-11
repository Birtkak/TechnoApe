# Effects spike (ADR 0031)

## Questions
1. Can particles be one instanced draw into the G-buffer, so the traced light lights them like the world?
2. Per-fragment light (the full `light()`: sun, moon, lamps, soft edge) or one sun visibility per particle?
3. Do they stay out of the traced scene (no shadows from them)?
4. What do 4,096 and 16,384 live particles cost on the GPU and on the frame thread? The design's proposal:
   4,096 at "normal", under 0.15 ms GPU and 0.05 ms frame thread, no allocation per frame.

## Answers (release build, dev PC, `run=fx quick=1`, three runs each, p50)

| live | lighting | frame thread fx ms | GPU fx ms |
|---|---|---|---|
| 4,096 | per fragment | 0.053-0.066 | 0.043-0.047 |
| 4,096 | one visibility | about the same | 0.035-0.038 |
| 16,384 | per fragment | 0.19-0.23 | 0.126-0.135 |
| 16,384 | one visibility | about the same | 0.091-0.100 |
| `stand`, effects off | - | 0.001 | 0.001 |

1. Yes: one instanced draw of 36 vertices per particle in its own pass after the G-buffer's, writing the same
   three targets and the depth, so GI and the composite treat them as world.
2. **Per fragment.** One visibility per particle is 20 % cheaper on the GPU (0.01 ms at 4,096) but loses the
   lamps, the moon and the soft shadow edge; a 0.01 ms saving does not pay for that.
3. Yes by construction: the particles are never put in the TLAS, so they cast no shadows and other rays never
   hit them.
4. GPU holds the budget with room (0.06 ms in the final build at 4,096, which also draws the grow and looks).
   **The frame thread misses it:** the final build's `step` with ground collision takes 0.060-0.064 ms at
   4,096 (0.030 before the per-column floor). Levers, not taken: step on a worker with a double buffer, or
   move the motion to the GPU. No allocation per frame: the pool and its output have fixed capacities.

## Reproduce
`git apply prototypes/effects/one-visibility.patch` (adds `fxlit=one`), build release, then hold `out/.lock`
and run `bash prototypes/effects/measure.sh`. `git apply -R` the patch after.

`close.png` and `far.png` are filmstrip frames of a break and a place, close and from 20 m (the commands are
in ADR 0031).

`dig.png` is a survival dig held (ADR 0034), four frames enlarged: the grass top cracking, then the next blocks
down breaking under the hold. It is from `technoape.exe run=play save=0 settings=out/dig-settings.txt
seconds=4.5 acts="0.2:say:/playmode survival,0.5:m:0:700,1.0:L" capture_ms=250 capture_w=480 sheet=out/dig.png`.
