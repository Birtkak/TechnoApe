# 0061 – 16-bit block ids, and one generated block for every reader

- **Status:** accepted (routine technical calls; W4 step 1 of `docs/plans/worldgen-plan.md`)
- **Date:** 2026-11-12

## Context
W4 brings rocks, minerals and caves (ADR 0026, `docs/design/world-data.md`). Two things must come first:
- Block ids were `u8`. Real rocks, minerals, soils, plants and machines pass 255 kinds.
- Three readers answer "what is generated here" each on their own: the mesher (`world::generate`, per
  cell from a tile), the physics (`phys::blocked` through `World::generated_under`) and the water
  (`Solid::gaps`, which took everything under the first air layer as solid). A cave would make them
  disagree: a player walking through rock, water falling through a ceiling the mesh draws.

## Options considered
1. **Ids: a `u16` everywhere a block id is held** (registry, edits, the mesher's cells, the protocol,
   saves, inventory, structures, effects). The compiler finds every place.
2. Ids: a newtype with kind and state bits now. More code for state nobody reads yet.
3. **Readers: the column as runs** (world-data.md) built first, every reader rewritten on runs. The
   end state, but it rewrites the mesher's hot fill before anything needs it, and runs need rocks to
   be worth having.
4. **Readers: one cave query per column, `Body::col_caves`**, that each of the three readers asks, with
   the ground's bands still from `material`. Small, and the shape runs will take later.

## Decision
Options 1 and 4.

- **Ids are `u16`.** The 12 kind / 4 state split of world-data.md stays the plan; nothing reads state
  yet, so there is no newtype or mask.
- **The registry still refuses an id over 255.** The shader unpacks the material as `q.y & 255u` and the
  looks buffer has 256 entries; the shader belongs to W3b's session now. Lifting it is one change in
  `shader.wgsl` and the quad's packing (`mesh.rs`, `m as u32 | o << 8`), done when the first block past
  255 is added (the `shortcut:` in `blocks.rs` says so).
- **Saves and the wire keep their version.** postcard writes a `u16` under 128 as the same single byte as
  a `u8`, and no build ever had an id over 29, so every save, recording and old peer reads the same
  bytes (test `a_save_with_8_bit_block_ids_loads`). A save that holds an id of 128 or more is new data
  an old build could not read anyway (its registry check refuses unknown ids). A save's edits and
  inventories must now name known blocks (an id past 255 would index past the inventory).
- **The edits' chunk hash** (part of the server's hash in recordings) hashes an id under 256 as its one
  byte, so recordings made with 8-bit ids still replay; a larger id hashes both bytes.
- **`Body::caves` and `Body::col_caves(face, i, j)`**: balls of air in a face's grid, and the layers
  they hollow in one column, at layer 0 and up. Under the ground those layers are air for
  `generated_under` (so `generated_block`, `World::block` and `phys::blocked`), for `generate` at level 0
  (a chunk a cave touches is never shortcut to `Solid` or `Empty`), and for `Solid::gaps`. Over the
  ground a cave changes nothing; plants stand where they stood.
- Caves are carved at **level 0 only**; coarser chunks keep their rock (world-data.md: too small to
  show). A cave across a face seam is cut at the seam (a `shortcut:`; step 2 keeps caves off seams).
- **The check:** `the_mesher_physics_and_water_see_the_same_cave` puts a closed cave and one open to
  the sky by the spawn and compares the three readers cell by cell (red first: 0 cave cells before the
  readers asked), and a player's box fits in the closed one.

## Consequences
- A mesh job's cells are 78 KB instead of 39 KB while it runs; the fill is the same loop, and the job's
  time did not change (row below).
- Without caves (every world today) each reader pays one empty-list check per column or cell; the soak
  hash and the fixed views do not change.
- Step 2 fills `Body::caves` from the bake (caves by rock); if caves become noise instead of balls,
  `col_caves` keeps its signature and the readers do not change.
- **For step 2** (from the review): a cave open to the sky under the sea makes `gaps` start the top gap
  below `gen_floor`, where `Solid::generated` starts the generated sea; sea caves need that rule (and the
  water's ledger) settled first. And `col_caves` allocates and scans every cave per call, per cell in
  `phys::blocked`: with real cave counts, bucket caves by face and tile and cache the spans per column.
- Not checked here: whether the streamer asks for the chunks deep under the ground around a player in a
  cave (only `generate` was tested). Step 2 checks it with a stand run inside one.

| Check | Result |
|---|---|
| Review | a fresh reviewer read the diff: no id truncated, postcard claim holds, the readers agree. Fixed: the chunk hash broke old recordings (now one byte under 256), a save with an unknown id in edits or inventory panicked the server (now refused, tested), the cave test now sits on a chunk corner. Left for step 2: sea over open caves, `col_caves` cost (above) |
| Gate | `tools/check.sh` 2026-11-12: tests, soak (hash b38c506a5105e2e6), replay, views, feel and every run's budget pass; the speed guard fails `fly` (job 338 us against 273, GPU 1.79 ms against 1.48, wgpu memory 821 MB against 725). The commit before this one fails it the same way on this PC today. |
| CPU cost | `run=fly quick=1`, the two builds interleaved, three runs each: mesh job 337 us before, 335 us after; GPU 1.80 / 1.80 ms; wgpu memory 827 / 833 MB. No measurable cost. Without caves, physics and water pay one empty-list check. |
