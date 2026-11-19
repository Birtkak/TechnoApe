# 0065 – The Studio's model view: where am I, where does the click land

- **Status:** accepted
- **Date:** 2026-11-12

## Context
The developer's first hour in the Studio: "moving the camera around, placing voxels on the right place and
clearness what you are doing and where is difficult inside the models screen". `docs/design/studio.md`,
"Usability round (2026-11-12)" step 4, lists the fixes, and "The bar" asks that a new model begins from a
starter shape, never an empty box. The view is the game's own renderer (ADR 0049) with the Studio's 2D overlay
on top; `ui/viewport.rs` is shared with the other pages, whose cameras must stay as they are.

## Options considered
1. **Flat view as a true orthographic camera in the renderer**: exact, but a new projection through the
   client's renderer and its culling, for one Studio view.
2. **Flat view as a long lens**: the eye 8 times as far and the field of view 8 times narrower (a `zoom k`
   command on `run=view`, the client's existing zoom). One line in the client; parallel enough that rows line
   up (the filmstrip shows it). Chosen.
3. **Edges drawn by the renderer** against the overlay's lines: the overlay has no depth, so it finds the faces
   the eye sees by casting a ray to each exposed face's middle (`voxel::pick`). Chosen, cached by the eye, the
   model's edit count, the layer and the pose; not found again while a button is held.
4. **Dimming the rows above the layer** by a dimmed copy of the model for the renderer: its colours must be
   palette names. Instead the renderer shows the model cut at the layer (`voxel::sliced`, as the old slice did)
   and the overlay draws the cut rows' faces faint. Chosen.

## Decision
Built in `crates/studio/src/ui/models.rs`, `voxel.rs` and `ui/viewport.rs` (plus `zoom` in `client/src/view.rs`):

- **Camera:** a view cube in the top right (x red, y green and up, z blue; a click on a face looks from there),
  with Flat and Frame buttons under it; numpad or row 1/3/7 Front/Right/Top, Shift for Back/Left/Bottom
  (Ctrl+number is the page keys), 5 flat; the six views are also buttons in the inspector. F frames the
  selection, else the part. Turning goes about the selection, else the model's middle (`Orbit::turn_about`, the
  point keeps its place on screen); the wheel zooms toward the pointer (`Orbit::zoom_about`); Alt + left drag
  turns. The floor names its edges (front is +z). All of it sits behind `Viewport::model`, which the Models
  page sets each frame and the view takes; every other page keeps the plain orbit.
- **Placing:** a half see-through ghost in the first colour with a light outline where a left click adds; the
  voxel a right click removes outlined red; the hovered face lit; mirror's twins shown too and its planes drawn.
  The part's floor gets 12.5 cm lines, 1 m heavier, 4 cells round its box; a click there with no voxel under the
  pointer places on the floor (`voxel::pick_layer` on row 0). **Layer mode** (Layer chip, Page Up/Down): clicks,
  boxes and lines land on one row's plane, the rows above are cut from the view and drawn faint. The selection's
  up and down moved to Shift + Page Up/Down. **Edges**, on at the start.
- **Clearness:** a bar in the view's top left, always: the tool and what each button does, how the view turns,
  and what is on (layer, mirror, flat). The status line reads "x 1 y 1 z 2 of 4 x 3 x 6, part body: dirt".
- **Starters:** Ctrl+N and New model show block, slab, post, tool (a head on a handle), plant cross and
  creature (body, head, four legs, pivots at neck and hips, facing +z); the page with no model shows the same
  list and says what the first click does. The old empty new model is gone.

Checks: `cargo test -p studio` (new: the six views, turning and zooming about a point, framing; picking on a
layer plane; exposed faces; every starter reads back, has every part filled with its pivot in its box and
stands in its block). Drive scripts (headless `shot=` with `input=`) set the six views by key, one by a click
on the cube, and frame two parts; a filmstrip in layer mode shows hover, add, remove, Page Up and a box drag;
the flat view was shot with a debug build of the renderer. Every picture was read.

**Review** (a fresh subagent, the diff against the design section): fixed: the floor fallback reached 32 cells
out, so a stray click far off grew the part (now only the drawn grid); layer row -1 walked down a row per add
as the box grew (the layer now stops at row 0); a box or line begun on a plane ended on another (half a row up)
so it landed a cell off its ghost (now ends on the same plane); the edges and the dimmed rows were found every
frame (now cached together, not found while a button is held); Back, Left and Bottom needed a turn or a key
(now inspector buttons). Answered, not changed: the layer is cut at rest while the faces are drawn posed, so a
clip scrubbed away from rest moves them apart (a `shortcut:` comment; leave layer mode to scrub); the flat
view's far eye can stand in a hill or a cave wall (choose the None or Checker scene); making a new model is not
an undo step (it changes nothing that was there; the new file is unsaved until saved); F, 5 and Page Up still
act on the open model while the starter list shows (harmless); a starter that failed to read would open nothing
without a word (every starter is read by a test).

**Fix after the play-test (2026-11-12): the box and the edges off the voxels.** The developer saw the selected
part's box and the Edges lines a voxel off the drawn model, more so down the snake's chain and on a grown leaf.
Cause: the renderer stood its camera and the shown model in the stand cell's own axes (`cell_axes`), which on the
cube-sphere are skewed and about 1.11 m at a face's middle, while the overlay and the click's ray count square
metres. The two agree only along the eye's line to the look-at point and drift apart away from it (about a tenth
of the distance). Fixed once in the shared frame: `models::view_axes` is the cell's axes made square (1 m, at right
angles, up the cell's up, the floor's middle kept), and the view's camera (`view::cam_pose`), the shown model and
its states' effect points use it. Placed block models keep their cell's axes and still fill their cell.
Tests: `the_view_shows_a_model_point_where_the_studio_projects_it` (seen red on the old frame: 0.227 against
0.244 across), `the_view_frame_is_square_metres`, and `a_click_lands_on_the_posed_voxel_of_a_child_part` (the
snake's tail, a child of a child with `at` and pivot, bent by a clip; it passed before the fix too, a regression
check of picking). Shots of the snake with the tail selected, before and after, and the leaf at rest and grown into
-x were read: the lines follow the voxels.
Review (a fresh subagent): the frame, its handedness and origin are right, and the camera and the shown model
both use it. Answered, not changed: while a **state plays** the renderer poses its clip on its own clock and the
overlay the page's clip at its time, so the box, the edges and the clicks follow the page's clip (stop the state to
edit); the state's effect points ignore the Fauna page's walk placement and scale (both from before this fix, a
follow-up); the 1 m stone block beside the model, and a block model as placed in the game, fill a skewed cell
about a tenth larger than the model drawn here.

## Consequences
- The Models page is usable without the manual's keys: every new control has a word, a tip and a button.
- A renderer exe older than the `zoom` command ignores it (a line on its stderr) and the flat view shows the
  model small and framed too near; the Studio's handed-over renderer is built with it.
- A face half hidden draws its edges whole (seen by its middle): clip per edge if it shows.
- Later: a true orthographic camera if the long lens's slight perspective matters for a big model.
