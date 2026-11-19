# 0116 – The ape, the animals, the furnace and the leaf as painted boxes

- **Status:** proposed (built; waits for the developer's look and merge)
- **Date:** 2026-11-14

## Context
The developer: "paint the character like the new way but also all the animals and other entities". The new way
is the painted box (ADR 0081, 0114): a box in a part's voxels whose faces are pixels of the model's `.png`, 4 px a
voxel, drawn a quad a pixel on the instance path. The ape (the player's figure: the inventory's picture and the
bare arm, ADR 0113, 0115), the boar, deer, hare and snake (ADR 0053, 0083), the furnace (ADR 0050) and the
falling leaf (the `leaves` effect's model) were still flat-coloured voxels, a colour a voxel.

## Options considered
1. **Paint by hand in the Studio, or by a script.** **Chosen:** a script, `tools/paint_figures.py`, beside
   `tools/paint_clutter.py` (its `Model`, ramps and palette): a recipe per figure names boxes on the parts and a
   paint function per face pixel (a coat lit on top and shaded to the foot with soft strands, an eye, a snout).
   The output is a draft for the developer to repaint (listed in `data/drafts.txt`), made again by one command.
2. **Keep the rig.** Every part keeps its name, parent, pivot and `at`; its voxels are emptied to a `.` grid of
   its own size and its boxes follow it, so `creatures.rs`'s walk (by part name), the clips, the points and the
   states are unchanged, and the Studio still reads every part.
3. **Boxes over 63 px.** A box side over 63 px is not drawn (the quad's 6-bit size): the script refuses one and
   the recipes split (the furnace's body in two).

## Decision
- `tools/paint_figures.py` paints ape (23 boxes, 5,456 quads), boar (13, 3,848), deer (11, 4,936), hare (9,
  1,604), snake (5, 1,194), furnace (7, 6,912), leaf (1 plane, 160); with the torch (ADR 0117, 2 boxes, 218)
  24,328 quads, the clutter's on top, against `MAX_MQUADS` 65,536 with 30 % kept free (a test).
- Client: the figure's inventory picture is its painting (it drew nothing for a painted figure); the bare arm
  draws `arm_r`'s boxes and those of parts hung from it (a box piece maps to its part); `painted_item()` (one
  part with boxes) decides the hold and the lying pose, so a painted creature in hand is held as a voxel model.
- The ape's skin ramp is warmer (`rich_rgb(26, 0.6, 0.12)`); its old palette rows are gone.
- Tests: every shipped creature is painted on every part it moves; a painted box follows its part through a walk;
  the inventory's figure is its painting; the bare arm draws the arm's boxes only; every shipped model fits the
  quads with room to spare. The Studio's voxel tests read the old voxel snake and furnace from `testdata/`.

## Evidence
Stills (play build, 768 planet, seed 1; `out/figures/`, not committed): `<name>.png` before | after pairs for
the ape, boar, deer, hare and snake; `deer_close_zoom.png` (after the nose fix and the darker back),
`snake_close_zoom.png` (the calmer 16 px blotches), `furnace_f1.png`, `deer_walk.png` and `ape_walk.png`,
`arm_swing.png` (the bare arm mid-swing at noon), `inventory.png` (the painted ape).

Costs (`docs/perf-debt.md`): 200 deer or apes, GPU p50 +0.5-0.6 ms against the voxel models (G-buffer +0.6-0.7);
24,328 model quads of 65,536.

## For the developer to confirm
- The look of each figure; the worst is named in the report.

## Shortcuts
- A pixel a quad (ADR 0114's): the furnace is 6,912 quads for a block-sized thing.

## Review
The fresh review of ADR 0117 read this round too (`1bbeb76f..claude/painted-figures`): it found the figures, the
bare arm's box mapping and `painted_item()` right; no findings against them.
