# NNNN – The game reads Studio maps for the plants' wind and the particles' turbulence

- **Status:** proposed
- **Date:** 2026-11-18

## Context
The Studio's Maps page (ADR 0153, `docs/design/studio-maps.md`) makes `data/maps/<name>.toml`: a node graph
that works out a tiling, looping picture. A map whose Out is fed a direction is a 2D push per pixel. Until now
only the Studio read maps; the plants' sway (`plant_sway`, ADR 0091) was two sines and a particle row's
turbulence (`noise`, ADR 0050) value noise. The developer wants both to be drawn in the Studio. Presentation
only: nothing on the server, the wire or a save changes.

## Options considered
1. **Where the map is worked out.** (a) A baked PNG beside the toml, read by the game: no evaluator in the game,
   but two files that drift apart and a bake step the Studio must never forget. (b) **One evaluator in the
   engine**, the Studio and the game both use it: the file is the only truth. Chosen.
2. **How a 2D map pushes a particle on a sphere.** (a) A tangent chart of the sphere (a cube-sphere face's own
   coordinates): no seam where the dominant axis changes, but more code and a choice of chart per body. (b)
   **The plane of the position's dominant axis**, the push's part along up taken out: a few lines, a seam at
   the cube's edges that a particle crosses rarely. Chosen, marked as a shortcut in the code.
3. **The sway.** (a) Replace the sines outright: one path, but no way back if the map's look is disliked. (b)
   **The map when `[sway]` names one, the sines without it**: the row is the developer's switch. Chosen.

## Decision
- **A. One evaluator, `engine::map`.** The format, the nodes, the graph and their tests moved from
  `crates/studio/src/map.rs`; the Studio's `crate::map` re-exports it and keeps what only it does (`check=1`
  over the folder, the PNG bytes, export, `map=`). `rows::num` moved with the map's writer.
- **B. `engine::map::Field`:** a direction map's whole loop (side, frames, seconds, the push per pixel per
  frame), `at(u, v, t)` bilinear in space, linear between frames, wrapping in all three. A number or colour map
  is refused: "map 'x' gives a number, not a direction: wire a Slope or Curl node before its Out". Worked out
  on a watcher's thread (`wind gusts`: about 0.2 s in the play build), never the frame thread.
- **C. Particles:** `noise = { map = "smoke swirl", strength, scale, speed }`: `scale` the metres a tile
  covers, `speed` times the map's own pace (left out: 1). The map lies on the plane of the particle's dominant
  axis (body frame), its part along up taken out. A row without `map` moves bit for bit as before (a golden
  test). A row naming a missing, refused or number map is refused with the row and the map named. The maps are
  read from `data/maps/` of the working folder, else the two shipped ones compiled in (a game without the
  folder, the tests).
- **D. The plants' wind:** `[sway] map = "wind gusts", tile = 48.0` in `data/effects.toml`: `plant_sway`
  samples a 3D texture (frames along the third axis, `Rg8Snorm`, linear, repeating) at the plant's column / tile
  and the sky's clock, played a whole number of loops an hour (`round(3600 / seconds)`) so the clock's wrap has
  no jump; the bend is `amp * (dir * (0.4 + 0.8 m.x) + cross * 0.8 m.y)`. Without `[sway]`, or with its map
  refused (said on stderr), the sines exactly as they were. A shot holds the wind at 0 as before.
- **E. Live:** the effects watcher watches `data/maps/` and the effects file (and the palette); the sway has a
  watcher of its own (the GPU's, as the block textures). A field is worked out again only when its map's text
  changed; the new wind map is uploaded and the terrain's bind group made again on the frame it arrives.
- **F. Shipped:** `[sway]` names `wind gusts`; the `smoke` row names `smoke swirl` (strength 0.8, scale 4 m).

## Consequences
- The client depends on the map format: a node changed in the Studio changes the game's wind and smoke; the
  map tests are the engine's now.
- The particles' field has a seam on the cube's edges (shortcut; a chart when it is seen).
- The sway's 3D texture is 1 MB for `wind gusts` (128 x 128 x 32, two bytes a texel); a 512 px map of 64
  frames would be 32 MB of VRAM.
- No Studio UI yet: a row's `map` and `[sway]` are written by hand; the Effects page keeps them on Save.
- Leaves (`sway` in `shader_vertex.wgsl`) keep their own motion.
- Found on the way: plants had not swayed at all on the land. `plant_sway`'s gate read `c.info >> 8` as the
  body, but `frame.rs` packs log2(lattice) << 16 and the shell << 18 into `info`, and a level-0 chunk's lattice
  is 8 (bits 16-17 = 3), so the test failed on every planet chunk. It is now `(c.info >> 8) & 255`: the body
  mask makes the plants sway again. Since this fix the sines sway too, so "without `[sway]` as before" means
  the same sines, now actually seen.
- Both watchers' first loads are waited for at loading (`Fx::watch`, `Gpu::new`), so a map's field is in from
  the first frame; the built-in set (`fx::SET`) is checked against the compiled-in maps only, so a map saved
  refused in `data/maps/` is named on stderr and cannot stop the game starting. A sway map's texels are made on
  its watcher; the frame thread only uploads them.
- The map smoke drifts sideways and stays lower than the old value noise, whose upward part lifted the
  plume. Kept at 0.8 / 4 m; a higher `rise` on the row is the developer's call.
