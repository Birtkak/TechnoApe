# 0088 – Fast leaves with open holes

- **Status:** accepted (the developer, 2026-11-13: "do the shader change here", after seeing bare winter crowns
  as black cubes)
- **Date:** 2026-11-13

## Context
With the default fast Leaves setting (ADR 0069) a cutout block was meshed and drawn as any opaque block, and an
empty texel showed the block's colour at 60 %. A bare winter crown (ADR 0084), nearly all empty texels, was a
dark cube with twigs on it; summer leaves had dark dots for holes. Fancy leaves draw the holes open but miss
ADR 0004's p99 on the forest fly-through, and ADR 0069 found the cause in the BLAS's second, non-opaque
geometry (the rays), not in the drawing.

## Options considered
1. **Fancy as the default.** Misses the frame budget (ADR 0069).
2. **Fast fills a hole with the sky's colour.** Wrong against anything but sky.
3. **Discard in the opaque pipeline.** The whole terrain loses early depth.
4. **Fast leaves through the cutout pipeline, opaque to rays.** The drawing leaves the empty texels out; the
   rays see whole blocks, so the BLAS keeps its one opaque geometry.

## Decision
Option 4.
- **Mesher** (`mesh::Cut`): `Solid` (coarse levels: the old mesher), `Shell` (fast leaves on level 0), `Full`
  (fancy). `Shell` culls a cutout block's face against a cutout block (a crown has no inner faces) and keeps an
  opaque block's face behind one (the ground under a shrub, a trunk in its crown), and puts the cutout blocks'
  quads last as `cut`, as `Full` does.
- **Client** (`frame.rs`): level 0's `cut` quads go through `fs_cut` in both settings; they are the BLAS's
  non-opaque geometry only with fancy leaves. The shader is unchanged.
- Test (red, then green): `mesh::tests::faces_are_culled_only_against_opaque_neighbours` and
  `glass_is_culled_only_against_opaque_or_its_own_kind` hold `Shell` to the slow path's face list and to the
  quads' grouping.

## Consequences
- Bare winter crowns are branches against what is behind them; a summer crown's holes are open too.
- **Fast leaves cast a whole block's shadow**, also a bare crown: a dark square under a bare shrub. Light through
  the holes stays fancy's.
- Through a hole one sees through the whole crown (no inner faces, back faces culled).
- Cost: level-0 chunks with leaves run the mesher's second pass and keep the faces under and inside crowns;
  leaf quads draw without early depth. Measured by the gate's speed guard: see the row below.
- Far crowns (coarse levels) stay one colour, so a far winter forest is brown blocks.

## Review and numbers
A fresh reviewer read the diff (mesher bit rule, quad order and offsets, the chunk table, the BLAS cache, other
readers, the shader). One finding, fixed: the ray split was taken from the live setting, so a chunk still
holding a mesh from before a change of the Leaves setting could disagree with its BLAS until it was meshed
again; the split now travels with the mesh (`Mesh::ray_cut`, `DrawChunk::ray_cut`). Noted, not changed: a
mirror or bounce ray that hits a fast leaf still shades an empty texel as the dark fill; `meshbench` now
measures `Shell`, so its numbers do not compare with older ones.
Views: winter 4.7 % changed (bare crowns), the others under 1.6 % (open holes); every picture read.

## Calls for the developer to confirm
- Whole-block shadows under bare crowns in fast leaves: keep, or let bare crowns cast none.
