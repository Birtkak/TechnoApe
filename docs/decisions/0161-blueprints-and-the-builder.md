# ADR 0161: Blueprints and the builder

Status: accepted (2026-11-07)

## Context
The fun pass (docs/plans/fun-pass.md) asked for copying a build and having a machine rebuild it elsewhere:
a blueprint item that records a box (voxels and block entities with their facing), placing it with the
existing ghost, a builder machine that builds the marked site from a chest, and building it by hand when
there is no builder. Prefabs (ADR 0038) already record a box of voxels, and the shape builder already has
a selection with handles (ADR 0143) and a prefab ghost that turns and mirrors.

## Decision
1. **Two items.** `blueprint` (blank, stack 16, a scraped hide and charcoal make four) and
   `written_blueprint` (stack 1, so each keeps its own build). A right click with a blank one opens the
   shape builder's Select box (same corners and handles); a click copies the box onto a written one.
2. **The data rides on the stack** under `blueprint` (`Blueprint.capture`): the voxels as a prefab
   (`PrefabStore.encode`: material names and run-length runs, so it is small, JSON-safe and outlives id
   changes) with the entity cells left out, plus a list of entities: kind, the item that places it,
   its voxel material, its footprint box in the blueprint, and `facing`, `axis` and the sub item (pipe or
   cable kind) where it has them. Stations (chest, furnace, crafting table) and every work piece an item
   places (`"work"` key) are copied, and furniture (facing, a sign's words) and torches, which keep their
   anchor (`at`) and are placed through `Main._place_furniture` / `_place_torch` (a torch picks floor or wall
   itself). **Contents are not copied** (chests, machines): a builder that filled them would make items from
   nothing. At most 64 Ki voxels.
3. **Placing** a written blueprint is the prefab ghost (`ShapeBuilder.start_grid`, entity boxes drawn in
   their material) with two hooks: R turns about up and X mirrors, Shift + R does not tip, and the click
   calls `on_commit` instead of building. `Blueprint.layout` turns the voxels with ShapeTool and turns the
   entities the same way: each entity box is marked in a grid ShapeTool turns (its new min corner is the
   anchor), and facings and axes go through where local +x and +z land.
4. **A site** (`Blueprints`, per world in `blueprints_<seed>_g<gen>.json`, saved with the other world
   files) keeps the blueprint, corner, turn and mirror; the cells are laid out again on load. It shows a
   see-through blue ghost of what is still to build, rebuilt after each pass, and goes away when all of
   it stands.
5. **The builder** is an LV machine (`BuilderEntity` extends `MachineEntity`; item `builder`, 40 W, so a
   hand crank runs it) that works the nearest site within `Settings.builder_reach_m` (24 m) from a chest
   beside it (any side or on top). It draws power while it has a site and a chest. A site is walked in
   passes, `SCAN_PER_FRAME` (512) cells a frame, voxels bottom up, then the pieces; a powered builder
   places up to `Settings.builder_voxels_per_s` (48) a second, one item a voxel (voxel items count voxels),
   one item a piece. A cell already right is done; a cell holding anything but air, gas, fire or a
   replaceable plant (ShapeBuilder's free mask) is **blocked and left alone**; a cell whose item the
   chest lacks is skipped and listed. Its look-at lines and right click say what it lacks and what is in
   the way.
6. **By hand:** within 6 m of a site no builder works, a hint at the top names what goes next and how many
   the player has, and a yellow marker shows where. Ordinary placing fills the ghost; the next pass sees it.
7. **Multiplayer (ADR 0145):** the builder's voxels go in with `ShapeTool.fill` and, connected,
   `NetOps.fill` through the edit journal, like a committed shape. Pieces are placed the way a hand
   places them (`Main._on_placed`, `WorkSystem._create`) inside `Net.record`, so their voxels are journaled
   like a hand's; the entities themselves are not shared, as ADR 0145 leaves block entities local for now.
8. **`/blueprint`** `copy x1 y1 z1 x2 y2 z2` (a written blueprint into the inventory), `place [at x y z]
   [turn 0-3] [mirror]` (the held one: the ghost, or marked at once), `list`, `clear`. The builder hums like
   the assembler.

## Consequences
- A builder walks cells in GDScript, bounded per frame; a site is capped at 64 Ki voxels. A bigger one
  would want the pass moved into ShapeTool (count and fill of free cells by material already are native).
- Out of the sim bubble a site waits (its cells read as unloaded).
- Every station footprint is square in x and z today; a non-square one would land beside its turned box.
- Checks: `test/test_blueprints.gd` (layout turns, bill, ghost, data), scripted run `blueprint_test=1`
  (copies a hut with a furnace, a chest, a hopper, a chair and a torch, rebuilds it 5 m away by a cranked
  builder with the furnace missing at first, compares the two boxes; JSON "debug blueprint", sheet
  `blueprint.png`), and `blueprint_test=save` then `=load` with `persist=1` on one seed (the held written
  blueprint and the marked site come back).
