# 0052 – Structures and grown flora

- **Status:** accepted
- **Date:** 2026-11-12

## Context
docs/design/studio-tools.md "Structures and flora" asks for boxes of whole world blocks made by hand, and trees
and bushes grown from a few numbers and a seed, then frozen for hand work, each judged in the game's light.

## Options considered
1. Structures as models (1/8 m voxels) – one format, but a tree must be world blocks the player can dig.
2. A file of whole blocks with a letter key and layers, like a model's voxels – readable, diffable, the same
   reader in the Studio, the client and later the generator. Chosen.
3. Grown trees stored as their blocks only – simple, but loses the numbers the wall of seeds varies.

## Decision
- `data/structures/<name>.toml`, read by `engine::structures` (`Structure::parse(file, text, &blocks)`, every
  refusal naming the file and the field or the layer, row and column; `write` is byte-exact on its own output):
  `size = [x, y, z]` (1..=64 blocks a side, `MAX_SIDE`), `anchor = [x, y, z]` (the cell set into the place it
  stands on; for a tree the trunk's foot, set into the first air over the ground), `layers` (bottom up, a blank
  line between, a row per z, a letter per x; `.` leaves the world's block, `_` clears it to air) and `[key]`
  (letter to a `blocks.toml` name).
- A grown one is `[grow]` alone: `seed`, `wood`, `leaves` (block names), `trunk_height` 1..32 blocks,
  `trunk_girth` 1..3, `branches` 0..12, `branch_angle` 0..90 degrees from up, `branch_length` 0..12 blocks,
  `crown` round | cone | flat, `crown_size` 0..12 blocks (radius), `leaf_density` 0..1. `grow(&Recipe)` is
  deterministic (splitmix64; a leaf's chance hashes its cell, so order does not matter): the trunk, branches up
  the trunk's upper half spread by the golden angle each with a tuft, the crown about the trunk's top; cut to
  64 a side about the foot. A bush is the same recipe with a short trunk (no separate kind). "Freeze" drops
  the recipe and writes the layers.
- Seeing it: `run=stand structure=name[:seed]` sets its blocks at the stand point (x along j, z along i, the
  camera frame of `models::cell_axes`). The Studio needs no new view command: it sends the blocks through the
  existing `place` of `run=view`, plus a clearing of air 4 blocks round it so the spawn's trees do not hide it.
- The Studio's Structures page (one page, both kinds): the files of `data/structures/`, the game's view with
  orbit, a layer slice with Add, Remove (back to the world), Air, Box, Fill, Mirror, Copy layer up, size and
  anchor, undo; for a grown one the recipe's sliders with units, the seed with "Another", the variations wall
  of six seeds (its spread also moves the numbers a little), and Freeze. Saves go through a `.tmp` and a rename
  after the game's reader takes the text; `check=1` reads every structure.

## Consequences
- Left out: placing structures on the planet (world generation and the bake are untouched: later work, when
  the generator grows plants; a row will then say its biome and how many per hectare), lean and root flare,
  placing and digging in the 3D view itself (the slice does the editing), the `kind` field of the design's
  plant row.
- Added 2026-11-12:
  - **Copy and paste.** The Select tool (S) drags a rectangle on the layer; Ctrl+C copies it by block name,
    Ctrl+V pastes it with its corner under the pointer on any layer or structure (what falls outside the box is
    dropped; a copied "world's block" or air cell replaces what was there). One layer at a time: a region of
    several layers is copied layer by layer.
  - **Small plants as models.** `voxel::plant(kind, seed)` grows a tuft, a flower or a mushroom as an ordinary
    one-part model in a block's box, from palette colours; the Models page's "Plant…" makes one ("tuft 3" is
    seed 3) to finish by hand. No recipe is kept in the model file: asking again grows another.
- The stand run stands at the default point (at sea since the baked planet) unless `at=` names land, and its
  camera stands in the structure's anchor cell; a look at one needs `at=`/`yaw=` or the Studio's view.
- Review: not yet reviewed by a fresh reviewer (rule 12) at the time of writing.
- Driven 2026-11-12 (`structures_box_drag_fills_the_cells_dragged_over_and_reloads_equal`,
  `structures_freeze_writes_the_grown_blocks_as_layers`): a quick drag on the slice lost its first cell (egui
  starts a drag only once the pointer has moved, and the box and the brush took the cell under it then); both
  now start at the press's cell, and a box let go outside the slice is held to its edge. Freeze was right.

## Addendum (2026-11-12): bad stand arguments
`place=`, `fxedit=` and `structure=` are read by `client::stand_args`, which also checks `creature=`, `model=` and
`fxplay=`'s count: bad input prints one line naming the argument on stderr and exits 1, never a panic. Test
`bad_stand_arguments_are_named_not_panicked`, seen red on the first panic.
