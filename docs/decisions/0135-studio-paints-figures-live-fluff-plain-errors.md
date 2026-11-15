# 0135 – The Studio paints figures on the model, shows fluff live, and says plainly what went wrong

- **Status:** accepted for the Studio (merged, tool tier); proposed for the view's `fluff` command (`crates/client`,
  branch `claude/view-live-fluff`, a taste call for the developer, CLAUDE.md principle 13). Until it is merged the
  Studio's renderer logs "unknown command" for `fluff` while a slider is dragged; the saved row shows within a second
  either way (ADR 0134). Numbered 0135: ADR 0134 (the live definitions) landed the same day under that number.
- **Date:** 2026-11-15

## Context
The developer, after ADR 0133: the error messages are opaque; the Models page still places voxels on models, though
no model will use that system again, while the tools that really edit a model (painting, states) are small menus
hidden down the inspector; and sliders like a reed's fluff do not show until the game is rebuilt.

Every model in `data/models/` is painted boxes (ADR 0116), but the page opened on the voxel Add tool, nine voxel
tools, mirror, layer mode, Shade by light and Carve. The painting was an inspector section under a palette of 1043
colours, with its own undo (Ctrl+Z did not undo a stroke). The Fluff sliders wrote `data/blocks.toml`, which the
game builds in (`include_str!`), so neither the game nor the Studio's view (the game's own renderer, ADR 0049)
showed a change until a rebuild.

## Options considered
1. Live fluff by reading `data/blocks.toml` from disk in the game: every block row would be live, but the registry
   is a static that the protocol hashes (client and server must agree) and that the mesher and the world share.
   Too wide for a slider.
2. Live fluff in the view only: the renderer takes the numbers over a command and writes the block's look rows on
   the GPU. Fluff is a look (`looks_of` words 16-17), not world data, so nothing else depends on it. Chosen.
3. For painting: a bigger Painting section in the inspector. The inspector is narrow and shares its space with the
   palette; painting on the model where it is seen is how figures are judged. Chose painting on the model, with the
   painting drawn large beside the view.

## Decision
- **Models page:** tools are Paint (default), Picker, Pivot, Point. Paint works on the model in the view
  (`voxel::pick_paint`: the ray against `Model::box_faces`, the nearest face with a painted pixel there, else the
  nearest face) and on the painting, now half the work area beside the view, the hovered pixel lit on both. Left
  the first colour, a drag a stroke; right the second; E the eraser. Point adds on the face clicked. One undo holds
  model, life and painting (`MDoc.hist`), so Ctrl+Z takes a stroke back. The inspector: Paint (tools, eraser, the
  palette in a box of its own), States, Part, Point, Ready-made motion, Model. New models from the starter shapes
  are made painted boxes at once (`voxels_to_boxes`). Removed: the voxel tools, mirror, layer mode, edges, the
  selection, Shade by light, Carve from a texture, Voxels to painted boxes, the colour count, and their `voxel.rs`
  functions and tests; the voxel test files `furnace_voxels.toml`, `snake_voxels.toml`. The game still reads voxels.
- **Fluff live:** letting a slider go writes the row as before, and since ADR 0134 a running game and the view take
  the saved row within a second. While a slider is dragged (proposed, the branch) the view takes
  `fluff <id> size push droop long dense light` (`view.rs`) and `Gpu::set_fluff` writes the block's fluff and light
  words in its four season rows (`fluff_words`). Nothing keeps them: the next `looks_of` (the let-go save's reload)
  writes the row's own back. (A first version kept them in a static that `looks_of` read; with rows reloading live
  that would have hidden a later edit of the file, so it was cut.) A `long` slider is added.
- **Messages:** about 45 refusals reworded to say what did not happen, why, and what to do (text only).

## Consequences
- Evidence: `a_ray_on_a_painted_box_finds_the_pixel_drawn_there` (red: not built), on the branch
  `a_dragged_fluff_lands_on_the_blocks_look_in_every_season` (red: the words one off); the drive tests paint the shipped furnace (`models_paint_then_undo_leaves_the_model_as_opened`,
  the edit and comments tests). `cargo test -p studio -p client`: client 176, studio 111 unit and 29 drive pass.
  Shots read: `out/st2/models3.png` (the ape, Paint, the hovered pixel `leg_r x 46 y 34` lit on model and
  painting); reed's Block view at fluff size 0 and 3 differs in 5382 of 55500 pixels, two runs at the same size in 32.
- A renderer without the branch prints "unknown command" for `fluff` and draws the saved numbers.
- Shortcuts: the painting beside the view is drawn at a fractional scale under 4 px a pixel (a wide painting);
  a hover pixel on a face whose painting rectangle is empty still paints (a hole can be filled). Painting on the
  model paints one pixel at a time (no brush size).

## Review
A fresh reviewer (Opus) read the whole diff: nothing serious; the look-row offsets and the static (never set in the
game, only by the view's command) are right. Fixed: a stroke begun on a pixel already its colour merged into the
step before it (`edit_paint` now says whether it changed, a stroke merges only after its first change); the painting
beside the view painted with the Pivot and Point tools; `pick_paint` took back faces, so a plane seen from behind
painted the mirrored pixel (now only faces turned to the ray, test extended, red seen); a refused fluff write left the view on unsaved numbers
(sent back). The cut-down branch had its own fresh review: see below.

The cut-down branch (`claude/view-live-fluff`, 2a74fe93) had a fresh reviewer (Opus): offsets, the id range (no
block over 255) and the test are right. Fixed: the command took missing or non-finite numbers as they came (now
six finite numbers, clamped to the sliders' ranges), and said "no cutout block" when it had no GPU. Answered: a
let-go that saves nothing new (the same text, or a refused file) moves no reload, but the Studio sends the row's
numbers to the view as soon as the slider stops, so no dragged value stays drawn.
