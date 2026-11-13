# 0082 – Clutter and pick-ups: computed from the seed, only what was taken is kept

- **Status:** accepted (step 7 of `docs/design/see-through-and-boxes.md`, `docs/design/clutter-and-pickups.md`)
- **Date:** 2026-11-13

## Context
The first age starts with bare hands taking loose things: sticks, flint, loose stones, what grows
(`crafting.md`, `progression.md`). The design puts the ground's small things on every column as a pure function
of the seed, the column and the season, drawn as still box models in the chunk's mesh (ADR 0081), and keeps only
what was taken. Items are in main (ADR 0074, 0075), so a pick-up goes into the slots.

## Options considered
1. Stored entities, placed at generation: memory, ticks and wire bytes for ten million pebbles.
2. **Computed per column, drawn in the chunk's mesh, the taken ones remembered.** Chosen, as the design says.
3. Instanced draws per kind: fewer quads, but a second path beside the chunk's for culling, streaming and light.

## Decision
- **Data:** `data/clutter.toml` (rows: decoration or pick-up, the ground blocks it lies on, near a tree or in the
  open, seasons, `regrow = "year"`, a pick-up's item), checked by `clutter::parse`; a biome row lists its clutter
  as `[name, per 100 columns]` (`biomes.rs` holds them to the rows and to 100). The first rows: grass tuft, fern,
  fallen leaves, twigs, flowers, pebbles (decoration); stick, loose stone, flint, mushroom, berries (pick-ups,
  new items in `data/items.toml`). Their models are drafts made by `tools/paint_clutter.py`, painted boxes in
  palette colours, listed in `data/drafts.txt`.
- **The pick:** one hashed roll per column. Pick-ups take the bottom of the roll at their full weight, decoration
  the next stretch at its weight times the Clutter setting's density (off 0, few 0.25, normal 1, many 4), so a
  lower setting is a subset of a higher one and pick-ups lie the same whatever the setting. A second roll picks
  the row by weight; `near` asks the tile's trunks (within 3 columns). Never on a face's last columns, under the
  sea, in a river or lake, or on a column whose chunk holds an edit in it; only on the generated top block, with
  air over it.
- **Seasons:** the season nearest the sky's time (`sky::season_at`), the southern half two seasons on; both halves
  turn at once, so level 0 is meshed again when the year enters another season, or the setting changes.
- **Mesh:** the chunk that holds the ground cell lays the clutter's quads (`clutter::quads`, after the glass,
  before the water, ADR 0081's still models), so a chunk of air never needs meshing for it.
- **Pick-up:** a left click aimed at the ground under a pick-up sends `ToServer::Take { seq, at }`, one call. The
  server checks reach, that one lies there (`clutter::pickup_at`, the mesher's own rule), that it was not taken
  this year (or ever, for what does not regrow) and that it all fits; then it fills the slots, counts it in the
  items' ledger, marks it taken and sends `ToClient::Taken` to everyone (all marks on join). The client meshes
  the chunk again without it. Save version 14 keeps the marks; a 13 loads with none.
- **Settings:** Graphics gains Clutter (off, few, normal, many; normal by default); `clutter=` on a run.

## Calls for the developer to confirm
- The first rows, their weights per biome and the drafts' look (all data; `tools/paint_clutter.py` remakes them).
- A regrowing pick-up comes back with the new year (the design says the start of its first season).
- Pick-ups show and can be taken with Clutter off; only decoration thins.
- Taking aims at the ground under the thing (no hit test on the model's boxes).

## Consequences
- `shortcut:` every taken mark goes to every client and is looked through for every level-0 mesh job; a map by
  chunk and by region when worlds hold thousands.
- `shortcut:` the server does not hide clutter over an edit lower in the column than the ground cell's chunk;
  neither does the mesher, so the two agree.
- `shortcut:` the clutter's models are read once at the start, not live as the voxel models are.
- `shortcut:` a new season or year meshes every level-0 chunk again, though only those with seasonal rows
  change; and a client meshes its first chunks before the server's clock reaches it, then again.
- Decoration casts no shadow and has no collision; building on its column removes it (an edit).

## Measured
2026-11-13, play build, dev PC, `run=meshbench at=1125.34,3634.37,1915.81 clutter=...` (the spawn's forest,
summer; `clutter_us` the time `clutter::quads` takes a surface chunk, beside a mesh's 60 us and a level-0 job's
about 336):
| Clutter | quads a chunk | us a chunk |
|---|---|---|
| off (pick-ups only) | 57 | 25 |
| few | 248 | (first build: 116) |
| normal | 816 | 61 |
| many | 3088 | 141 |
- **CPU:** normal adds about 61 us to a level-0 surface chunk's job, some 18%; a first build took 125 us at
  normal and 113 with clutter off, nearly all in the per-column ground lookups, so a column's roll is now tested
  first against every row of its biome (`may`, one hash) and most columns stop there. Chunks with no ground top
  in them stop at once. Nothing runs on the frame thread but the draw.
- **GPU and memory:** 816 quads of 8 bytes a chunk at normal (6.5 kB); VRAM 825 MB standing in the forest at
  many and at off alike (the face table, 2 kB of faces and 10 kB of paintings, is fixed). Frame: the fast gate's
  stand p99 3.7 ms, fly 6.1 ms, sim 4.7 ms with Clutter at normal, all within ADR 0004's budget.
- Filmstrips read: the spawn's forest in summer (grass tufts among the grass), with Clutter off (none), in
  autumn; a grassland (`at=-0.4539,0.7641,-0.4585`: tufts and small flowers on the meadow grass); a rain forest
  (`at=0.4606,0.8166,-0.3479`: a dark close view where little shows; ferns want trunks near). `out/clutter-*.png`.
- Tests: the rows load and every biome's names exist; density thins decoration only and a lower setting is a
  subset; seasons choose; every row's model loads, inside its cell; `may` bounds `pick`; the remesh stamp turns
  with the season and the year; save 14 keeps the marks and a 13 loads with none.

## Review
A fresh reviewer read the branch; four bugs, all fixed:
1. The client meshed with the year of the last season change, so a regrowing pick-up taken in a year's first
   eighth stayed drawn and every click on it was refused: the remesh stamp is now (season, year) (`stamp`).
2. The season was taken per chunk on the client but per column on the server: a chunk across the equator drew
   seasonal pick-ups the server refused (and a click took an unseen one); `quads` now asks each column's.
3. With full slots a click on a pick-up was refused forever and the ground under it could not be dug: the
   client takes only what fits, and digs otherwise.
4. A body with no sea got no clutter (`is_none_or` for `is_some_and`).
Its performance notes went into the shortcuts above and the `may` early out.
