# 0113 – Dropped items drawn as their models; painted boxes on the instance path; an item's size

- **Status:** proposed (built; waits for the developer's look and merge)
- **Date:** 2026-11-14

## Context
The developer: "make dropped items always look like their real model", then "scale in hand models" and "tools
painted like grass, finer pixels". A pile (ADR 0074, 0082) drew each stack as a 25 cm cube of its `look` block
among the particles (`App::lay_piles`), so a hand axe, a flint or a stick lay as a grey or brown cube. The model
in hand was a fixed 0.35 m a cell, so a model could not be drawn finer without growing on screen. And painted-box
models (`[[part.boxes]]` with a `.png`, ADR 0081: flint, stick, berries, the clutter) were only ever meshed into
the chunk as still quads (`vs_still`): `models.rs` had no box quads, so **ADR 0110's line that flint shows its
clutter model in hand was wrong**: nothing was drawn in hand for flint (the log said 442 quads for 17 models).

## Options considered
1. **Painted boxes on the instance path.** (a) A new pipeline drawing `SFace`s per instance: a second vertex
   shader beside `vs_still`, whose placement is tied to a chunk's cells. (b) **Chosen:** each box becomes quads of
   one pixel in the existing model quads (`Bx::pixel_quads`: per face, per painted pixel, a one-voxel face in the
   box's own pixel grid, its colour the pixel's; empty pixels none, so planes keep their cut-outs; a plane is one
   pixel thick, flattened to 2 % by its frame), and one extra "part" per box whose transform is its part's pose
   times `Bx::frame` (corner, turn about its middle, 1/4 voxel a pixel). `vs_model` and `fs_model` are unchanged:
   no shader touched. Cost: no merged runs (a pixel a quad): all 17 models now 4,352 quads of the 65,536.
2. **Where a pile's models are drawn.** (a) In the chunk mesh as clutter: a remesh per pile change. (b)
   **Chosen:** as model instances in `Models::frame`, one per stack, from a list `piles::lay` makes each frame
   from `self.piles` (so it follows the server within a frame, and a reloaded model shows at once).
3. **How a tool lies.** A per-model "lying" pose in the file (more data to paint), or **chosen:** an item whose
   rest bounds stand taller than they are wide is laid on its face (+z up, the face the eye sees in hand); every
   other model lies as it rests in its cell. Each is centred on its corner of the cell and rests on the floor by
   its bounds, turned about up by the cell's hash plus 1.9 rad a slot.
4. **An item's size.** **Chosen (the overseer's spec):** a model's optional top-level `item = <metres a cell>`
   (0.05..1, refused by name outside), read by the hand and the pile; absent, 0.35 in hand (as before) and in a
   pile 0.35 for a voxel model, 1 (real size, as the clutter lies) for a painted-box one. Not written when absent,
   so every file and the Studio's writer are unchanged.

## Decision
- Engine (`crates/engine/src/models.rs`): `Model::item: Option<f32>`, checked in `check`; `ITEM_M = 0.35`;
  `Bx::pixel_quads` and `Bx::frame`. The Studio's two `Model {..}` literals gain `item: None`.
- Client: `Loaded::new` meshes the parts and then the boxes from the model's `.png` (read in `load_keeping` and
  checked as the clutter checks it; no painting, no boxes drawn), and keeps the rest bounds. `Models::frame`
  appends each box's pose, uses `item` for the hand, and draws `Models::piles`. `crates/client/src/piles.rs` (new)
  holds `lay` (the pure map from piles to model instances and leftover cubes) and `App::lay_piles`, moved out of
  `inventory.rs` (only that function and `PILE_DRAWN` left it). A stack whose item or block name is a loaded model
  lies as that model; any other keeps the cube of its look; past 64 m nothing; no count in the world.
- The painted hand axe (the overseer's recipe in `tools/paint_clutter.py`, 13 x 22 px point up) gets
  `item = 0.3`: 21 cm long in hand and in a pile (at the clutter default of 1 it lay 69 cm long).

## Evidence
- Tests, each seen failing first: `models::tests::a_boxs_painting_becomes_one_quad_a_pixel_on_its_face`,
  `an_items_size_is_held_to_its_range_and_absent_is_not_written` (engine); `a_painted_box_model_is_drawn_in_hand`,
  `a_models_item_size_is_its_size_in_hand_and_in_a_pile`, `piles::tests::a_pile_draws_its_models_and_cubes_for_the_rest`
  (client). `cargo test -p client` 142 passed; engine models 15 passed.
- Stills (`out/piles/`, off-screen `run=play planet=768 seed=1`, items thrown through the inventory's acts):
  `before.png` (the same pile with `models=` an empty folder: four cubes), `after.png` (the hand axe lying on its
  face, the stick, granite's cube; the flint is behind the cube from this side), `flint_lying.png`,
  `hand_axe_in_hand.png`, `flint_in_hand.png`.
- Cost: `docs/perf-debt.md` row "Dropped items as models (0113)".

## For the developer to confirm
- The painted axe's `item = 0.3` (21 cm); flint in hand at the default 0.35 is small and half under the screen's
  edge (a 19 cm shard drawn at 0.35 of its size): give flint an `item` (about 1) if it should read in hand.
- Clutter models keep their real size in a pile; voxel models lie at 0.35 a cell unless they say.
- A standing item lies on its face; a block's model (a furnace) stands as it stands in the world.

## Shortcuts
- A box quad per pixel, never merged (shortcut: merge runs of one colour per face when quads grow tight).
- Boxes over 63 px a side are not drawn on the instance path (the quad's 6-bit corner).
- Models are not traced, so a lying model casts no shadow (as every model; the cube was a particle, no shadow
  either).
- The stack in a pile is never drawn bigger for a larger count.

## Review
