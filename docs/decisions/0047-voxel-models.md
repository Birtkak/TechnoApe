# 0047 – Voxel models in the game

- **Status:** accepted
- **Date:** 2026-11-11

## Context
The Studio's Models page (docs/design/studio-tools.md "Models", wave 2) needs the game to draw what it makes:
`data/models/<name>.toml`, parts of coloured voxels in a tree, named points, clips of keys. A block row may name
a model, the block in hand shows it, files reload live. The voxel is the fine cell of ADR 0033: 1/8 m, 8 to a
block's edge (`engine::models::VOXEL`, the one constant), so a part's box is 1..32 a side. Constraints: the frame
budget (ADR 0004), nothing read or meshed on the frame thread, the Studio calls the same check (no GPU in the
engine), CPU over GPU.

## Options considered
1. Bake models into the chunk meshes – free to draw, but no animation and a re-mesh per change.
2. One small mesh per part, drawn instanced with one transform per part (the design's plan) – animation is a
   matrix per part; the CPU poses each instance each frame.
3. Pose on the GPU from the clip's keys – no CPU per instance, but a second copy of the clip code in WGSL.

## Decision
Option 2.
- `crates/engine/src/models.rs`: `Model::parse(file, text, &Palette)` (serde, `deny_unknown_fields`; refused with
  the file's name and why: a colour not in the palette, over 62 colours, a box outside 1..32, a layer or row of
  the wrong size, a letter past the colours, a part named twice, an unknown parent or a loop, a point or key
  naming no part, t outside the clip), `write`, `decode`/`encode` of the voxel rows, `pose(clip, t)` (per part a
  `Mat4` from its voxels to the cell's metres; keys per part, ease linear/smooth/step from the key it leaves,
  held before the first and after the last; turn x then y then z about the pivot; children follow; a looping clip
  wraps), `mesh(&Part)` (faces against the part's own voxels dropped, same-colour runs merged greedily per slice).
- `crates/engine/src/blocks.rs`: optional `model` on a row (the registry hash is of the file's text, so it moves
  only when a row uses the field). `World::models` keeps the cells holding such a block (as `lights` does);
  `stream::hide_models` turns them to air for the mesher, so the cube is not drawn and not traced; collision is
  untouched (`world.block` still says the block).
- Client (`crates/client/src/models.rs`): the textures' watcher, now a generic `gpu::watch`, reads and meshes the
  folder on its own thread (palette from `data/palette.toml`); the frame thread copies the packed quads once per
  change (1 MB buffer, 16 B a quad). Each frame it poses every instance and writes one 3 x 4 transform per part,
  straight into the camera's frame (f64 on the CPU, so no precision loss far from the body's centre), 64 B a
  piece; one draw per part with one instance per piece (`vs_model`/`fs_model`), in the terrain's G-buffer pass,
  lit by `light()` where it stands (sun, shadows, lamps, bounce), as the effects are. A cell's frame maps the
  model's x, y (up), z to the grid's j, layer, i: a turn of the axes, so nothing mirrors. Model cells are drawn
  within 128 m. The block in hand is drawn as its model (lower right, camera-fixed) when a file of its name
  exists; the HUD cube then stays away. Clip `idle` loops when present.
- Run keys: `models=dir` another folder; `model=name[:clip]` stands one 3 m ahead of the stand point on the
  ground; `run=models n=` the benchmark (n models in a widening fan ahead, each at its own phase; clip from
  `model=`, else `idle`, else the first).
- The main bind group now has 10 storage buffers; the device asks the adapter's own limit (default is 8).
- Draft: `data/models/furnace.toml` (8 x 6 x 8 body, a door that swings 100 degrees in `work`, point `flame`),
  listed in `data/drafts.txt`; no block row uses it.

Benchmark, release, dev PC, `quick=1`, `at=-1,1,-0.85 eye=14 pitch=-30` (n = 0 is run=stand at the same spot),
the furnace's `work` clip (2 parts, 42 quads), median of three runs:

| n    | models' frame-thread work ms (p50) | frame thread ms p50 / p99 | GPU ms p50 | G-buffer ms p50 | frame p99 ms |
|------|------------------------------------|---------------------------|------------|-----------------|--------------|
| 0    | 0.08 (the table stage without models) | 2.47 / 3.03            | 2.10       | 0.95            | 3.25         |
| 100  | 0.11                               | 2.40 / 2.68               | 2.03       | 0.88            | 3.02         |
| 1000 | 0.39                               | 2.55 / 3.12               | 2.18       | 1.27            | 3.31         |
| 5000 | 1.61                               | 2.67 / 3.25               | 2.28       | 1.37            | 3.44         |

The models' work is timed inside the `table` CPU stage (no new stage, so the perf baseline's keys stay). Every
run is inside ADR 0004's budget (`budget_ok`, worst frame 4.7 ms). **Recommended cap: 1000 animated models in a
frame** (0.3 ms of frame-thread time, about 0.3 ms GPU); 5000 still fits but spends 1.5 ms of the frame thread,
which the game will want for simulation. Not enforced yet beyond the buffer (65536 pieces).

Also in this change (the mirrored sides of ADR 0045): `texel` turns x on the -x and +y side faces, so a texture
or pattern reads left to right from outside on all four sides, y down from the top. Proven with an "F" painted on
every face of dirt, seen from four sides (`out/mirror/sheet.png`, not shipped). The fixed views `seam` and
`corner` changed by 2.2 % and 1.1 % of pixels, only on patterned side faces (grass fringes, leaves); they were
looked at and accepted (`tools/views.sh accept`).

Proof: engine tests seen failing first (parse and round trip, each refusal by file and reason, the clip's eases,
pivot and parents, the mesher's culling and merging, the draft loads, a block row naming a model, the mesher's
input hiding model cells); `cargo test` green; the furnace's filmstrip with `work` playing
(`out/models/furnace.png`, `out/models/furnace_door.png`); 5000 furnaces (`out/models/bench-5000.png`); a run
whose `models=` folder gained a good and a refused file saw both within a second.

## Consequences
- The Studio can call `Model::parse`, `write`, `pose`, `mesh`, `decode`/`encode` and draw with the same code.
- Models cast no shadow and are not in the traced scene; a model cell lets the sun through. Revisit with the
  traced scene's next change.
- Posing is per instance on the frame thread (`// shortcut` in models.rs at MAX_PIECES); move it to rayon or the
  GPU (option 3) when a factory holds thousands of running machines.
- Not verified in a played session: a block row with `model =` (none exists; the path is unit-tested and drawn
  by the same code as `model=`) and the model in hand. The palette's own changes reload only with the next change
  in the models folder.
- The voxel is the fine cell's size; when fine cells arrive, a model's voxels line up with them.

## Review
Not reviewed yet: no commit was made in this session (the house rule asks a fresh reviewer before the commit).

A fresh reviewer (2026-11-12) of `models.rs` `load_keeping`, `take`, `upload` and the state restart: fixed, a
`data/palette.toml` that failed to read or parse made the watcher send no models at all at the next change in the
folder, so every model vanished; it now keeps the last good model of each file still there, as a refused model
file keeps its own (test `a_bad_palette_keeps_the_models_before`, seen failing with no models). The rest read sound.

## Addendum (2026-11-12): a save keeps the developer's text
`models::write_keeping(old, model, life)` writes over the file as it was: its leading comments on top, every other
comment line above the section it stood above (found again by header and `name`, else by its place among its kind;
a comment inside a section goes above the next one), CRLF when the old file had it. Test
`a_save_keeps_every_comment_above_its_section_and_crlf`, seen red. Left: key order and blank lines are the writer's.
