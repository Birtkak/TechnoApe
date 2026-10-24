# ADR 0038: Shape building, box mode and prefabs

Status: accepted (2026-10-24)

## Context
ADR 0037 decision 2 takes Lay of the Land's building into the game from the start: shapes from a
radial menu, a preview with drag handles, rotate and mirror, click away to commit, voxels taken from
the inventory; box mode for digging and building; prefabs. The brush (ADR 0034) stays as "normal"
mode. How LotL does it: `docs/research/2026-10-24-lay-of-the-land-building-tools-fluids.md`.
Today a brush placement writes at most 512 cells, one GDScript call each, from the held stack only.
A shape is easily 10-100k cells (a 4 m wall is 32 x 32 x 4 voxels), so the per-voxel work has to be
native (principle 8), and the inventory has to pay for it across all stacks.

## Decision
1. **Native shapes** (`ShapeTool` in technosim): one call rasterizes a shape into cells, one call
   builds its preview mesh (surface faces only), one call writes cells into the `SimWorld` (only
   where the cell is empty or replaceable) and returns how many it wrote. Shapes: **box, wall,
   cylinder, cone, sphere, stairs, slope**, sized by a voxel box (width x height x depth), with a
   **hollow** wall thickness (0 = solid). Orientation is one of the 24 cube rotations plus a mirror
   (an integer 3x3 matrix): shapes stay exact on the grid, no free-angle rotation.
2. **Build mode** (`ShapeBuilder`, GDScript, orchestration only): hold **V** for the radial menu
   (normal brush, box mode, the shapes, prefabs; the mouse picks a slice, releasing V takes it).
   With a voxel material in hand: the preview follows the aim; a click sets it down; then handles,
   aimed with the crosshair and dragged by holding the left button and turning the view (the handle
   follows the aim ray along its axis): **height** (top arrow), **sides** (one arrow per side, each
   grows that side), **hollow** (an inner knob), **move** (a centre knob, in the ground plane; the
   height arrow with Shift moves it up and down). **R** turns it 90 degrees about up, **Shift+R**
   tips it 90 degrees toward the camera, **X** mirrors it left-right. A **left click away** from the
   shape commits, **right click / Esc** cancels.
3. **Paying for it:** a commit needs the material for every cell it fills (empty or replaceable
   cells only), counted over hotbar, main inventory and bag, and takes it from them; the preview is
   red and the HUD shows "needs N, have M" when it is short. Creative pays nothing.
4. **Box mode**, both ways: with a material, click a start voxel, aim, click the end: the box gets
   the same handles before the commit. With a tool, the same two clicks mark a box to dig; holding
   break digs it over the time the largest brush would take for those cells, with the drops the
   brush gives (`MiningTable.voxel_drops`); cells the tool cannot dig stay.
5. **Prefabs:** every commit is kept as **Last build**. **Select** (in the menu) marks a box like box
   mode and saves what is in it, named, in a category: `user://prefabs/<category>/<name>.json`
   (`{"version": 1, "size": [x, y, z], "palette": [material names], "runs": [[palette index, count],
   ...]}`, x fastest, then z, then y; index 0 is "nothing"; names, not ids, so a prefab outlives id
   changes). Placing one uses the preview with move, rotate and mirror handles, and pays per
   material.
6. **Server-ready:** the builder only plans; a commit is one request to `Main` (cells or shape
   params, material, payer), the same entry a server will take (ADR 0037 decision 10).

## Consequences
- The brush keeps its keys (Ctrl + wheel, B); V, R, Shift+R and X are new and free today.
- The preview is a mesh rebuilt when the shape changes (throttled), never terrain edits: a shape costs
  nothing until committed. Writes go through the bubble (the building area is inside it); a shape
  reaching outside the window is refused for now.
- Falling pieces (P12) check every commit's removed cells only for box digs; a built shape that
  floats is fine (built wood is `rigid` and holds what it touches, like any wood).
- First version (2026-10-24): a click on a selection opens the chat with `/prefab save builds ` typed,
  to name it (and change the category); a proper dialog comes when the menus get their next pass. The Prefabs menu shows the first 12.
- Taste calls left open: the radial menu's look, handle sizes and colours, whether stairs come in
  step heights other than 0.5 m.
