# 0101 – Powders: a calmer slide, slopes drawn on their tops, blocks that glide

- **Status:** accepted (the developer's three calls, 2026-11-13; geometry and defaults Claude's, open to taste)
- **Date:** 2026-11-13

## Context
Bug 2026-11-13_14-37-25: "having entire blocks of powders moving in feels clunky ... i feel this could be
better". Sand, gravel and rubble (`falls` in `data/blocks.toml`, ADR 0079, 0093) fall and slide as whole 1 m
blocks: a heap reads as stairs, a slump hops cell by cell, and any block beside a drop of two slid out, even one
with more sand on it, so a pile undermined itself and chains of slumps ran on. The server keeps whole blocks
(collision, wire and saves stay as they are); what changes is the rule and the look.

## Options considered
The developer chose, from the options put to them:
1. **Rule: "steeper, calmer"** (chosen) – a powder falls straight down when unsupported and slides only when its
   pile stands two or more above the neighbouring column's top; the same for every powder. Against: finer cells
   (fine-cells.md (b)), a per-material angle of rest, or leaving the rule as it was.
2. **Shape: slopes drawn by the client only** (chosen) – the server's blocks stay whole; the client draws a loose
   block's exposed top as a slope down toward lower neighbours. Against: real sub-block heights on the wire.
3. **Motion: a glide, grains and one hiss** (chosen) – a moved block glides to its cell over about 0.2 s, sheds
   grains of its own texture, and a slump of many blocks plays one rolling hiss, not a sound a block.

## Decision
**Rule** (`engine/src/fall.rs`). A block in the air falls as before. A supported block slides only when it is
its pile's top (the cell over it open) and a side cell and the cell under that are open, i.e. it stands two over
that neighbour. A block with anything on it never slides out from under it: no hole opens in a pile, sand under
a stone holds. `Falls::wake` also wakes the cell under a change, which may be a pile's top now. A tall tower
peels from the top a block a tick, each new top sliding before the one ahead has landed, so a tower of four in a
corridor ends 1 1 1 1 (`a_tower_in_a_corridor_slumps_to_steps_of_one`), a heap on open ground at steps of one.
Reading of "steeper": the threshold stays a step of two (the developer's own numbers: a step of one stays, two
slides); what is calmer and steeper is that only the surface moves and that anything laid on sand pins it.

**Shape** (`engine/src/mesh.rs`, `client/src/shader.wgsl`). Every grid point at the top of layer z is a corner
four columns share. It **sinks** a cell when, of the four cells around it at z, one is open, every other one is a
loose block, and the four over them are open: a function of the point alone, so every face that meets there
agrees, within a chunk and across chunk edges (the 1-cell padding holds all eight cells). A loose block with open
air over it lowers each top corner that sinks by one cell, unless all four sink: then it keeps its top (a lone
block, the peak of a heap and a ridge one wide stay blocks, never a flat sheet over a full-height collision box).
Its sides' top edges come down with the top (a side under a ramp shrinks to a triangle or to nothing); a foot
never moves. Where a neighbour's top sinks and this one's does not, the side between them shows: each loose block
also draws its side toward a loose neighbour where a corner of that side may sink, at its own heights (inside a
solid, against an unsunk neighbour, it is never seen). So a terrace's edge is a ramp a cell long, an outer corner
a three-corner fold, an inner corner a dip, a two-wide ridge a roof.

The quad keeps its 8 bytes: bit 31 of the low word (free until now) marks a sloped quad, bits 20-23 of the high
word its lowered corners (bit du + 2 dv, as the occlusion's). A sloped quad never merges. Cracks own bits 20-23 on
rock; a cell with a crack edge draws no slope. `quad_corner` lowers the corners, so the drawn terrain, the traced
geometry (`expand`) and hit texels all read one shape; it splits a sloped quad along the other diagonal when its
(0,0) and (1,1) corners differ, so a lone sunk corner is one triangle's. A sloped top's normal is its triangle's own
(from the placed corners), in `vs` and for bounce hits (`seen`); its painting lies flat (texels by x, y as a flat
top), its look bits masked so the corners are no crack lines; its lattice cell is picked half a cell down
(`quad_at`), so a lowered corner interpolates inside the cell rather than past its edge. Coarse levels slope by the
same rule at their own cell size.

Collision stays the block: a player stands on the box, up to a cell above the drawn ramp at its low edge.

**Motion** (`client`). A server move (two edits in a row, ADR 0079) glides: the client meshes the new cell as air
(`Streamer::gliding`, level 0; to its neighbours' slopes it is still a loose block, `mesh_cracked`'s `held`) and draws the block as a whole-cell effect at its new cell offset toward the one it
left (`fx::Glide`, `vs_fx` bit 20: `p` is the offset in cells), eased over `GLIDE_S` = 0.2 s; a move on from a
gliding block's target carries on from where it is drawn, at most a cell behind (a fall a cell a tick never
trails). Glides go first in the effects buffer, so a full one never cuts one off (a hole where the cell is air). When it ends, the cell is meshed again and the block is
drawn on until the terrain shows it (at most a second). Each move sheds `grains` (`data/effects.toml`: six debris
of the block's own texels off its faces, falling, gone where they land). A message with sideways moves plays
`slump` (`data/sounds.toml`, from `base.whoosh` and `base.scrape`) at most every `SLUMP_S` = 0.45 s, louder with more
slides at once: one rolling hiss while a slump runs; a block only falling (down a shaft, a dig under it) is silent. Effects off: no glide (the block hops as before), the hiss plays.

## Consequences
- Building on sand is easier: sand under anything holds; only a pile's surface ever moves.
- Every fixed view with sand or gravel steps changes (`tools/views.sh`: the desert and beach views).
- A loose block that glides is a cube until it lands, then takes its slope: a small settle at the end.
- The ramp's low edge is drawn a cell under the box a player stands on; a player on the slope floats over it by
  up to a cell at the edge (worst on the ramp's lowest line).
- Cost: see `docs/perf-debt.md` (mesh jobs: a bit test a cell, the slope pass only for chunks with loose blocks;
  the vertex shader three more `place` calls for each sloped top vertex).
- Revisit: a peak as a pyramid (needs a centre vertex); a slope's low edge meeting water; the cross-face glide
  (still a hop, as the fall's own shortcut).

## Review (principle 12)
A fresh reviewer read the diff with the task. Fixed: a chained glide restarted its ease from where it was drawn
and, a cell a tick, settled some six cells behind its cell (now at most one; tested); glides were laid after the
particles and could be cut off a full buffer (now first); the hiss counted falls too (now slides only); two doc
comments split by the inserts. Answered, left: a cell both cracking rock and loose would draw flat between sloped
neighbours (none is; cracks win the bits); coarse levels slope too, at their own cell size (a far beach reads
like the near one; level borders already differ by the coarse sampling, nothing checks across them); the extra
sides take the occlusion of the cell they face; the slope pass's 48 KB per chunk with loose blocks (perf-debt
row). Also fixed after the first slump film: a gliding cell was air to its neighbours' slopes, so a tower's
blocks showed as wedges for a glide; the mesher now takes it as a loose block for the slopes (`held`, tested).

**Follow-up (2026-11-13):** the `high` view showed a grid on the sand sea floor: `generate` skirts the padding (air under air), so every chunk border read as a step down and its tops sloped all round, at every level; the slopes now read the cells before the skirts (`generate_raw`, `skirt`, `mesh_cracked`'s `raw`; tested by a flat skirted floor).
