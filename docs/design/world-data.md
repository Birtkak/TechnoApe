# World data: what a block is, and who may ask

- **Status:** design proposal, 2026-11-10. Routine technical calls with their reasons; each part gets its
  ADR and benchmark when it is built (`worldgen-plan.md` W4 built most of it, ADR 0062 and 0063). No taste calls in here.
- **Why now:** Terra's rocks, caves and ore (ADR 0026) make a block depend on column and layer; machines,
  dense bases and a planet that can be mined out (ADR 0009) all land on the same structures. Decided
  piecemeal, the mesher, the physics and the water end up with three answers to "what is here".

## One question, one answer
What is at block (body, face, i, j, layer)? Asked in this order, first hit wins:

1. **An edit** there (a player or a machine changed it).
2. **A removed volume** that covers it (a planet eater took the chunk): air.
3. **The generated block**, a pure function of the world's bake and seed.

Water is not a block: it lives in the gaps (ADR 0020). Machines are a block plus an entity (below).

## The generated block: a column is a list of runs
- Per column the generator produces **runs**: (first layer, block) pairs from the bottom up, such as
  granite to 212, limestone to 240, a cave's air to 244, limestone to 251, clay to 253, grass at 254, air.
  A plain column has 3 to 6 runs.
- The runs come from the **column record**, which is what the height cache of the optimisation pass holds
  per column: surface height, soil depth and kind, the rock boundaries under it, the surface block, and
  three flags: a cave may lie under this column, an ore body may, a river crosses it.
- Only flagged columns evaluate caves or ore. The bake marks them per map cell, so most of the planet
  never does.
- **Every reader uses the runs:**
  - the mesher fills its 34³ cells by copying runs (faster than today's per-cell compare);
  - `phys::blocked` searches the runs of the few columns a player's box touches;
  - the water's `Solid::gaps` is the runs' air, as it is today with one run fewer;
  - `World::block` for a single block is a search in one column.
- **One test guards it:** for random columns, with and without caves and edits, the mesher's cells, the
  physics' answer and the water's gaps must describe the same solid. It runs in the soak.
- **Coarse levels** sample the bake's mips at the cell's middle, as today's generator samples its noise.
  Caves and thin ore are below what a coarse cell can show and are skipped from level 2 up.

## Block ids: 16 bits, 12 for the kind and 4 for state
- Real rocks and minerals (ADR 0026), soils, plants, building materials and machines pass 255 kinds.
- A cell becomes 16 bits: **12 bits of kind** (4,096) and **4 bits of state**: a rotation, a fill, a growth
  stage, or for an ore its grade (fundamentals 2.2: "block = id + small state").
- An ore block is its own kind per mineral, not per mineral and host rock: the look draws the mineral's
  flecks over the rock the column record gives. That keeps kinds in the hundreds.
- Touches: the mesher's cells (78 KB a job, held only while it runs), the quad's material field (16 of
  its 24 free bits), the looks buffer (256 to 4,096 entries), `ToServer::Edit`, `ToClient::Edits`, the
  inventory, saves (a version bump; old saves load by widening), recordings.

## Edits: sparse until dense
- Today: a hash map of cells per level 0 chunk, about 9 bytes a cell in a save.
- A built-up base fills chunks. *Proposal:* a chunk's edits switch to a **palette-packed 32³ array** once
  they pass 2,048 cells (the research already planned palettes), and back when they fall under half that.
  Readers see one interface.
- Sim code never iterates a hash map (fundamentals 2.3): where order matters, edits are visited in
  chunk-then-cell order.

## Seen from far away
- A far chunk samples one block in the middle of each coarse cell, so a tower thinner than the cell
  vanishes from orbit (roadmap, "big builds seen from afar").
- *Proposal:* each edited level 0 chunk keeps a **summary pyramid**, updated on a worker when it changes:
  a coarse cell counts as built with the commonest placed block when at least an eighth of its volume was
  filled where generation had air, and as dug when at least half of its solid was removed. Coarse levels
  read the pyramid instead of point-sampling. Mining scars and megaprojects then show from Luna.

## Machines: block entities
- A machine is a block kind plus a **sparse entity** keyed by its block: inventory, progress, links to
  its networks (`networks.md`).
- Entities are stored per kind in flat arrays, ordered by a stable id given at placement, so the tick
  visits them in the same order on every run.
- A multi-block machine is one entity that owns its blocks; digging any of them asks the entity.

## Removed volumes and the ledger
- A planet eater takes whole chunks (progression, "Eating a planet"). The world keeps a **set of removed
  chunks** per level, merged upward when eight siblings are gone; a save stores that set, not billions of
  edits.
- **The material ledger per body** (roadmap) counts **elements**, not blocks (`crafting.md` explains why):
  ore bodies enter exactly (the bake counts their cells), bulk rock enters as an integral of the bake with
  a stated error, and every dig, build, recipe, launch and removed chunk moves counted amounts. A removed
  chunk's contents are counted by generating it once on a worker.
- The soak checks the ledger every tick, as it does the water's.

## Regions: the unit of saving, sending and sleeping
- A **region** is the stack of all chunks over an 8 x 8 footprint of level 0 chunks (128 m square).
- Saves write changed regions, not the whole world (ADR 0016's "regions come when a world outgrows one
  file").
- A client subscribes to regions near it (fundamentals 2.6): edits, entities and water tiles go only to
  subscribers. Today `Edits` and `Players` go to everyone.
- A region with nothing awake is not visited by any system.

## Digging to the centre (kept open, not built)
- The grid's columns narrow with depth: 1.5 % at today's 64 m crust, 4.5 % at the 192 m proposed in
  `worldgen.md`. Fine.
- Deeper needs **shells**: below a set radius a new shell starts with half the columns per face edge, so
  blocks stay near 1 m. An address gains a shell number. Nothing above may assume shell 0 is the only
  one; that is the whole constraint for now (ADR 0009).

## Order of building
1. 16-bit ids (alone, with its ADR): everything else stores them.
2. The column record and runs, behind today's generator, with the three-readers test. No look changes:
   the soak hash and the views stay as they are.
3. Rocks, caves and ore on top (W4).
4. The palette-packed chunk, regions in saves and subscriptions: when the first base outgrows the maps,
   measured by a named worst-case run ("a fully built face").
5. The summary pyramid with the first megaproject; removed volumes with the planet eater.
