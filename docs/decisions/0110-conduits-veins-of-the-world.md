# ADR 0110: Conduits, the veins of the world

Status: accepted (2026-11-02), first slice. Follows ADR 0068 / 0069 C (still liquids, finite once opened),
0104 (world water static), 0109 (cave water).

## Context
The developer asked for a crazy idea to make the world richer to explore (2026-11-02) and picked this one: a
hidden, pressurised plumbing under the surface that the player can find by ear, breach, and follow to a source.
The engine already has the two halves it needs: still liquids that stay put until something opens beside
them, and a deterministic generator that places features from cell hashes.

## Decision
**Conduits** (`WorldGen::conduit_at_cell`, 40 % of 256 m cells, ~6 per km²): a polyline of 6 segments
(40-72 m each, bending up to a radian, rising 4-11 m) from a **chamber** (4.5-8 m sphere) 70-134 m under the
ground, magma conduits (one in four) 120-184 m, always above the lava sea. The tube (1-1.7 m) and the chamber
are still water or still lava in the rock; the top 4 m of ground stays rock, so a tail that rises into a valley
side seeps out of its wall, never the lawn. The chamber holds a 1.5 m core of **spring stone** (id 122) or
**magma heart** (id 123): hard, patterned (atlas tiles 105, 106), mined with a tier-2 pick. Carved at LOD 0-1 in
`FineGen` next to the geodes; the deep-rock uniform shortcut counts them.

**Finding them**: the **sounding rod** (`/kit gadgets`, playtest kind `sounding`) taps the rock and hums,
pitched 2.0 at the tube to 0.5 at 120 m (`FineGen.conduit_near` → `WorldGen::conduit_dist`); sneak + right
click says the distance. `/goto aquifer | magma_conduit` teleports to the top of the nearest chamber. Walking
into a tube or chamber notes the landmarks `aquifer`, `magma_conduit`, `spring_stone`, `magma_heart` in the
journal.

**Breaching**: nothing new. Open a voxel beside the tube and the still liquid becomes finite and pours (ADR
0068 / 0069 C); a conduit crossing a cave shows as liquid in the cave's walls, floor or ceiling, the natural
breach point.

## Not in this slice (follow-ups)
- Gas veins (no gas material in the sim), consequences that travel (a lake dropping when its aquifer is
  tapped: needs the liquid simulation Fable owns, ADR 0104), intersections (steam halls where magma meets
  water, geysers), and what the spring stone and magma heart *do* when used (Phase 3 wands / machines).
- A per-conduit sound loop in the terrain (the rod is the only ear for now).

## Consequences
- No new noise; per block 25 cell hashes plus ≤ 10 polylines, per column a 2D distance per segment in reach,
  per rock voxel a 3D distance per candidate: generator time unchanged within noise (`fine_gen_bench.gd`).
- Material ids 122, 123 (the reserved world-generation range); `GEN_VERSION` is for the merge to bump.
