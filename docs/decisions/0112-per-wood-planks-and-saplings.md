# ADR 0112: Per-wood planks and saplings

Status: accepted (2026-11-02). Content pass from `docs/plans/fable-backlog.md` (cheaper-model list).

## Context
Every log made the same oak planks, and nothing regrew a cut forest: leaves dropped only sticks.

## Decision
- **Planks per wood.** Spruce, acacia, jungle and birch planks are materials 207..210 with their own atlas tiles
  176..179 (a new atlas row; 180..183 free), the oak planks' rules otherwise (`GameMaterials._wood_defs`). Oak keeps
  `planks` (36) and the `planks_from_log` / `planks_with_saw` recipes, now on `oak_log`; each other log has
  `<kind>_planks_from_log` and `<kind>_planks_with_saw`. All planks carry the `planks` tag, so every `#planks`
  recipe takes any of them (mixed too). The deconstruction hammer gives each back (`built` in mining.json).
- **Saplings.** Materials 211..215 (`<kind>_sapling`, order `GameMaterials.WOOD_KINDS`): one micro voxel, a bark
  stem with a leaf tuft, painted from the kind's leaf and bark tiles (no new tiles). Each kind's leaves drop their
  sapling at 5 % per 0.5 m block (Minecraft's leaf rate). A sapling places only on soil (`GameMaterials.is_soil`:
  grass, dirt, red earth, leaf litter). `Saplings` (`scripts/farming/saplings.gd`, data `data/saplings.json`) tracks
  placed ones, saved as `saplings_<seed>_g<gen>.json`, ticked with the farm (scaled by `farm_speed`); after
  10-20 game minutes it grows the tree with the instant-tree gadget's stamp: the crown filled into the air only
  (`SimBubble.blast` with `only = [AIR]`; spruce as a cone of stacked balls), then a bark trunk with the log inside.
  It waits while its spot is outside the simulated area, retries a minute later while the trunk's column holds
  anything but air, plants or clutter, and is forgotten once the sapling is gone. `/sapling <kind> [dx] | grow`.
- No native hook: the generator's tree builder (`FineGen::build_tree`) would give grown trees the wild look, but
  the generator is owned elsewhere; the stamp is GDScript over existing SimBubble calls.

## Consequences
- Grown trees are simple (straight trunk, ball or cone crown), not the generator's aged trees. A later native
  `FineGen` entry point that stamps one `build_tree` result at a position would replace `Saplings.grow`'s stamp.
- Sapling placement does not consume a different kind of ground (no farmland); saplings are one 12.5 cm voxel.
