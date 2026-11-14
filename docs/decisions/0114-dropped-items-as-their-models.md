# 0114 – Dropped items drawn as their models; painted boxes on the instance path; an item's size

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
  checked as the clutter checks it), and keeps the rest bounds. A model with painted boxes whose painting is
  missing, unreadable or too small for its boxes is refused like a refused `.toml`: a reload keeps its last good
  model with one stderr line, a first load draws it not (`painting_of`). "Has a model" means it has at least one
  quad to draw (`drawn`, one place for the hand, the pile and the inventory's picture), so a model that loads
  with nothing to draw lies as its cube, and nothing is drawn in hand. `Models::frame`
  appends each box's pose, uses `item` for the hand, and draws `Models::piles`. `crates/client/src/piles.rs` (new)
  holds `lay` (the pure map from piles to model instances and leftover cubes) and `App::lay_piles`, moved out of
  `inventory.rs` (only that function and `PILE_DRAWN` left it). A stack whose item or block name is a loaded model
  lies as that model; any other keeps the cube of its look; past 64 m nothing; no count in the world.
- The painted hand axe (the overseer's recipe in `tools/paint_clutter.py`, 13 x 22 px point up) gets
  `item = 0.3`: 21 cm long in hand and in a pile (at the clutter default of 1 it lay 69 cm long). Its boxes'
  edge columns are tone 4.5, under the ridge's 5, so the ridge is the lightest line (at 6 they striped it).
- Flint gets `item = 0.6`: a 12 cm shard in hand and in a pile. `item` sizes a painted model in hand and in a
  pile alike; the clutter on the ground (`clutter::Still`) never reads it, so the flint lying in the world stays
  19 cm.
- In hand a painted-box model is held by its rest bounds' middle (the cell's middle before), so a flat thing that
  lies low in its cell is not under the screen's edge; a flat one (height under 0.4 of its width: flint and the
  stick) is tipped 60 degrees to show the eye its top (`Loaded::held`). A voxel model (a creature, the furnace)
  is held by its cell exactly as on main.
- The Studio's model view copies the open model's `.png` beside the `studio.toml` it hands the renderer, so
  painted boxes show there too (they were drawn nowhere in it before): written as `studio.png.part` and renamed,
  and handed over again when the painting's modified time changes, not only the model.
- A pile's stacks reserve their pieces per model, whole: a model whose instances would pass `MAX_PIECES` is left
  out entirely for that frame, never drawn in part.

## Evidence
- Tests, each seen failing first: `models::tests::a_boxs_painting_becomes_one_quad_a_pixel_on_its_face`,
  `an_items_size_is_held_to_its_range_and_absent_is_not_written` (engine); `a_painted_box_model_is_drawn_in_hand`,
  `a_models_item_size_is_its_size_in_hand_and_in_a_pile`, `piles::tests::a_pile_draws_its_models_and_cubes_for_the_rest`
  (client). `cargo test -p client` 142 passed; engine models 15 passed.
- Stills (`out/piles/`, off-screen `run=play planet=768 seed=1`, items thrown through the inventory's acts):
  `before.png` (the same pile with `models=` an empty folder: four cubes), `after.png` (the hand axe lying on its
  face, the flint, the stick, granite's cube), `flint_lying.png`,
  `hand_axe_in_hand.png`, `axe_in_hand_sun.png`, `flint_in_hand.png` (tipped, wholly over the hotbar's edge), `inventory_slots.png`
  (main's slot pictures of the hand axe, flint, stick and granite), `studio_flint.png`, `studio_hand_axe_crop.png`.
- Cost: `docs/perf-debt.md` row "Dropped items as models (0114)".

## For the developer to confirm
- The painted axe's `item = 0.3` (21 cm) and flint's `item = 0.6` (12 cm in hand and in a pile).
- Clutter models keep their real size in a pile; voxel models lie at 0.35 a cell unless they say.
- A standing item lies on its face; a block's model (a furnace) stands as it stands in the world.
- Flint and the stick tipped 60 degrees in hand; the other painted items upright, by their middle.

## Shortcuts
- A box quad per pixel, never merged (shortcut: merge runs of one colour per face when quads grow tight).
- Boxes over 63 px a side are not drawn on the instance path (the quad's 6-bit corner).
- Models are not traced, so a lying model casts no shadow (as every model; the cube was a particle, no shadow
  either).
- The stack in a pile is never drawn bigger for a larger count.

## Review
A fresh reviewer read the diff. Findings and what was done:
1. **A model that loads but draws nothing hid its pile** (a painted model whose `.png` was missing or refused
   loaded with no quads; `has` said yes, so the pile drew nothing and the hand nothing). Fixed: `drawn` (at least
   one quad) is the one test for the hand, the pile and the inventory's picture; such a model lies as its cube.
   Test `a_model_with_nothing_to_draw_is_its_cube`.
2. **Studio view: a repainted `.png` was not handed over, and a half-written one could be read.** Today a refused
   `.toml` on reload keeps the last good model with one stderr line; a refused painting now does the same (test
   `a_painted_model_without_a_good_painting_is_refused`: missing, half the bytes, boxes outside the painting;
   a voxel model reads without one). The view's key holds the painting's modified time, and the copy is written
   to `studio.png.part` and renamed.
3. **Hold by the middle and the tip reached every model in hand.** Now only painted-box models are moved, and
   only flat ones tipped; the 0.5 line tipped the loose stone (a lump, 0.49), so it is 0.4. In hand today
   (models with an item or block of their name; no row of `data/blocks.toml` names a `model =`):
   berries moved; flint moved, tipped; hand axe moved; loose stone moved; mushroom moved; stick moved, tipped.
   All six drew nothing in hand on main (their boxes were never meshed there), so none of them moves away from a
   picture the developer has seen. Ape, boar, deer, hare, snake, leaf, furnace (voxel) are unchanged, and fallen
   leaves, pebbles, pebbles.1, twigs (clutter) never come into a hand. The furnace has no block or item of its
   name, so it cannot be held: no before/after shot is possible; instead tests hold the boar against main's own
   formula (`a_voxel_model_in_hand_is_held_as_before`) and check every in-hand model's hold
   (`only_flat_painted_models_are_tipped_in_hand`: voxel ones identity, the tipped set flint and stick, seen
   failing with the loose stone at 0.5).
4. **Limits (answered, not optimised, principle 14).** Box quads are one a pixel against the shared
   `MAX_MQUADS` = 65,536; when the models pass it, the first model over (by file order) is dropped whole with one
   stderr line. Today all 17 models are 4,352 quads (6.6 %). `lay` makes a `String` per stack and scans the
   model list per stack per frame, and `pose()` plus the box extension allocate per instance; costs are in the
   perf-debt row. Pieces: one per non-empty part an instance; the hand axe is 6 (its painted boxes), so 200
   axe piles are 1,200 of `MAX_PIECES` = 65,536; a four-stack pile of axe, flint, stick and a block is 8. A model
   reaching `MAX_PIECES` mid-way was drawn in part; fixed by reserving per model (left out whole).
5. **Tests.** The box test had every `turn` zero: `a_turned_boxs_pixel_quads_lie_on_its_faces_as_the_clutter_draws_them`
   checks a box turned [8, 35, 0] and a plane turned [0, 90, 12] (both its faces) against `Bx::faces`, seen
   failing with the turn taken out of `Bx::frame`. `a_painted_box_model_is_drawn_in_hand` now derives its box,
   its middle and its bounds from the loaded flint, not from numbers copied from `flint.toml`.
6. **Stills re-shot** with today's flint and axe: `out/piles/after.png` (the pile from the far side: flint,
   stick, granite's cube and the hand axe all in view; the cube still hides part of the stick),
   `out/piles/axe_in_hand_sun.png` (noon, over grass: the axe reads blue-grey with lighter edges and a white
   tip, not dark, so the recipe's tones are unchanged).

## Second round (look only)
The developer, on the stills: "keep improving".
1. **The hotbar shows pictures.** `hud::layer` hands each filled slot's square (the inventory's: 40 px, 6 in)
   to its caller, which draws it through the inventory's own choice (`App::look_of`: model picture, else cube);
   counts and wear stay the hotbar's. Test `the_layer_has_a_crosshair_and_nine_slots` (seen failing with the
   call left out). Still `out/piles/hotbar.png`.
2. **A pile's block is the block.** It was a debris cube: 2 x 2 texels of a random place of its block, tilted
   about a random axis, 0.25 m an edge (its size was right; it read large beside the slim axe, nearer the
   camera and tilted). It is now the block's own whole-cell look (`Look::Block`, the world's texels and
   painting) shrunk to 0.2 m about the cell's middle and moved in cells to its corner and onto the floor (bit
   20, as a glide); no shader change. Test in `a_pile_draws_its_models_and_cubes_for_the_rest`. Stills
   `out/piles/blocks_front.png`, `out/piles/blocks_side.png` (granite, meadow grass, broadleaf leaves, the axe).
3. **Contact shadow: skipped.** Nothing cheap exists: fx, clutter and creatures cast none (models and particles
   are outside the traced scene), and the G-buffer has no blending for a soft dark patch; a dark fx square
   would be a hard tile. Real shadows for models need them in the traced scene (their quads in an acceleration
   structure beside the chunks', refit as they move) or a decal pass that darkens the lit image under each
   instance after lighting.
4. **Inventory polish.** The tooltip stands right of its slot (else left), its middle at the slot's, inside
   the panel (test `a_tooltip_stands_beside_its_slot_inside_the_panel`). A cutout block is its cube with the
   painting's empty texels left open (`hud::cube` no longer fills a hole with the tone; tests
   `a_cutout_paintings_holes_stay_open_in_the_cube`, `a_slot_shows_a_block_as_the_world_does`): every block
   with `see = "cutout"` is a cube in the world, reed too, so none stays flat and `sprite::flat` goes. A model
   picture keeps 3 px inside its square. The worn pack stands wider than the shoulders and over them. Stills
   `out/piles/inv_survival.png` (frame pack worn, the tooltip on hotbar slot 1), `out/piles/inv_creative_nature.png`.
5. **The lying axe.** It rests on the ground's top and lies 21 cm long, its `item` size (test
   `a_lying_axe_rests_on_the_floor_at_its_size`, seen failing with the model let down 5 cm): nothing wrong in
   the code. It is 4 cm thick lying, so the meadow grass's blades cover it; beside it the stick lies 80 cm
   (clutter's real size) and flint 15 cm.
6. **Slot pictures read in daylight.** They drew the painting's own sRGB tones (the same space as the cubes,
   measured: granite's top is its painting's 168,148,140 in the hotbar) shaded down to 0.62 by a light, and the
   axe's tones are dark (41,43,57 most), lifted only by the world's sun; now the face toward the eye is its
   own colour and others no less than 0.75 (test `a_picture_shows_its_front_in_its_own_colour`), and the view
   shows the painted face (`picture_view`: an upright item from its front, a flat one from 70 degrees above,
   a lump from three-quarters). Stills `out/piles/hotbar_pictures.png`, `out/piles/inv_pictures.png`.

For the developer:
- The axe's size in a pile (21 cm, under the grass blades), or a larger `item`.
- Reed is a cube in its slot as in the world (it was a flat tile).
- A cutout block's slot picture shows the summer painting (shortcut: the season's tint the flat picture had is
  not on the cube's painting).
