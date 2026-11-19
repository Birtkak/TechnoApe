# 0081 – Glass, and models of painted boxes

- **Status:** accepted (steps 3, 4 and 6 of `docs/design/see-through-and-boxes.md`; step 5, the Studio's box
  tools, is not built)
- **Date:** 2026-11-13

## Context
The developer wants see-through glass, tinted ("see-through glass ... make it a big nice upgrade"). ADR 0068
measured the light (a candidate loop over a non-opaque BLAS geometry) and ADR 0069 built it for cutout leaves.
Glass cannot go through the G-buffer (one surface a pixel); the design draws it after the composite, beside the
water, and lets it tint the light that passes it "if the spike's numbers allow".

## Options considered
1. Glass opaque to rays (left out of the BLAS or flagged opaque): cheapest, but a glass house is dark inside and
   a coloured pane casts no colour.
2. **Glass in the BLAS's non-opaque geometry beside the cutout quads, never confirmed; each glass candidate
   multiplies the ray's tint.** Chosen: the loop already exists, and only chunks that hold glass pay it.
3. An ordered walk through the glass (closest-hit per pane): exact for rays that look, but a ray per pane.

## Decision
- **Data:** `see = "glass"` (`blocks::See::Glass`); rows glass, green glass, amber glass (ids 120-122, away from
  the rows other work adds), loose, in the creative screen's "made" tab. A glass painting may have any alpha
  (`textures::decode`); unpainted glass is its colour at alpha 0.3 with a rim one texel wide at 0.8, so a wall
  reads as panes.
- **Mesher:** a glass face is culled against an opaque block or the same glass block (a wall has no inner
  faces); a face is never culled against glass. Glass quads go last among the land quads, after the cutout ones
  (`Mesh::glass`), with either Leaves setting. A chunk with no block seen through runs the old loops plus one
  bit test per solid cell (`SEE`); one with glass or fancy cutout blocks makes a second pass for the opaque and
  glass columns. The test holds both settings to a face-by-face reference.
- **Drawing:** `fs_glass` on the terrain's `vs`, in the screen pass after the sea and the water: blended,
  depth-tested, not writing depth, back faces culled (one pane's tint per wall, not two). Lit by the forward
  light as the water is (direct light, the flat light from around, the sky at a grazing angle, the sun's glint).
  Chunks far to near; faces inside a chunk unsorted. Far levels draw glass in its one colour at alpha 0.45.
- **Light:** the glass quads join the cutout quads in each chunk's second BLAS geometry. `trace` never confirms a
  glass candidate; it multiplies `ray_tint` by `mix(1, texel colour, texel alpha)`. Sun, moon, lamp and bounce
  rays take that tint, so light through green glass falls green.
- **Studio:** the inspector names glass and leaves its kind alone (it paints whole pixels only; painting partial
  clearness, the design's "how clear" well, is not built), so the Studio never turns glass opaque.

## Part 2: painted boxes (step 4) and still models in the chunk (step 6)
- **Format:** a model part may hold `boxes` beside its voxels: each a corner and size in model pixels of 1/32 m
  (`models::PIXEL`), a turn about its middle, and `uv`, where the net of its six faces lies in the model's
  painting `data/models/<name>.png` (`Bx::rects`: top and bottom in the first row, the sides below). A box thin
  on one axis is a plane: two faces back to back on one rectangle. A part may also rest turned (`turn`). A
  painting is 8-bit RGBA, 1..128 px a side, every pixel empty or a palette colour; at most 64 boxes a model
  (`check`, `check_painting`, `painting`). A part of boxes alone has one empty voxel, so every tool that reads
  voxels still reads it.
- **Still models:** what does not move is put in the chunk's mesh: each box face one quad of dir 7 (no block
  face has it) naming a row of a face table (corner, edges, rectangle, painting) and a turn in sixteenths and
  a place in the cell. Their quads lie after the glass and before the water (`Mesh::still`), drawn by
  `vs_still`/`fs_still` into the G-buffer and lit as blocks are; they are in no BLAS, so they cast no shadow and
  hold no light. The face table and paintings are bindings 16 and 17, read once at the start.
- **Not built:** moving box models (in hand, `model=`): the client's voxel models still draw voxels only.

## Calls for the developer to confirm
- How clear unpainted glass is (0.3 middle, 0.8 rim) and the three colours.
- Tinted shadows are on (the design's default).
- Glass under the water's surface draws over the water (`shortcut:` in frame.rs; sort the two by chunk when
  anyone builds under water).

## Consequences
- `shortcut:` a ray that looks (bounce, reflection) also takes the tint of glass beyond what it hits, since
  candidates come unordered; exact for shadow rays, which stop at the first opaque hit anyway.
- ADR 0069 found non-opaque BLAS geometry behind fancy leaves' p99 tails; glass adds that geometry only to
  chunks that hold glass.
- The indirect buffer grows by MAX_CHUNKS * 16 bytes for the glass draws.

## Measured
2026-11-13, play build, dev PC, `run=meshbench at=1125.34,3634.37,1915.81` (the spawn's forest, 6 x 13 x 13
chunks, best of five each):
- **The mesher's CPU cost: none.** A first build that tested every solid cell's kind cost 64.4 against main's
  59.5 us a chunk (+8%); `mesh_cut` now looks at kinds only where glass can be (a chunk with edits: generation
  makes none) or fancy leaves draw: 60.1 and 60.2 against main's 60.6 and 62.7, 966 quads a chunk either way.
- Filmstrips read (`out/glass-day.png`, `out/glass-night.png`: a 7 x 7 house of clear panes with a green and an
  amber one, a lamp inside, `at=0.0347,0.8675,0.4975 eye=4 pitch=-25 yaw=180`): by day the panes read as framed
  panes and amber and green light lies on the floor inside; by night the lamp lights the frames and the floor.
- The fast gate after main was merged in: tests, soak and replay ok; stand p99 3.7 ms, fly 6.1, sim 4.7, all in
  budget; the views differ only by the clutter on the ground (accepted after looking).

## Review
- A fresh reviewer read the glass diff before the checkpoint: seven findings, six fixed in that commit, the
  seventh is the looking ray's unordered tint above.
- A second fresh reviewer read the whole branch (glass, boxes, still models, clutter, pick-ups): the quad
  ranges, indirect regions, BLAS ranges, `SFace` layout and bit packing checked fine; its four bugs are in ADR
  0082's review. Main's clouds took bindings 14 and 15 in the merge; the still models moved to 16 and 17.
