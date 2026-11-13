# 0095 – The changed rock around ore bodies

- **Status:** accepted (built)
- **Date:** 2026-11-13

## Context
Mining part (d), `docs/design/mining.md` part 1 and ADR 0087: a player follows an ore body by reading the rock
around it, no map and no numbers. Ore bodies are ellipsoids from the map and the seed (ADR 0063); their cells
are asked through one rule (`Body::ore_at`) by physics, the mesher and the ledger.

## Options considered
- **A look on the host block** (a tint or overlay of granite, schist, ... in the shader). Keeps each host's
  own look and hardness under the stain; needs a second id per cell or a renderer rule over (block, place), a
  client and shader change, and every host times every stain to paint.
- **New block rows** for the altered rock, one per kind of alteration, whatever the host. Plain data: one rule
  in the generator, nothing in the renderer, six paintings. The host's identity is lost inside the halo (a
  stained granite and a stained schist look alike), and the rows have one hardness each.
- Chosen: **new block rows.** Geologically an altered rock is a different rock (greisen is no longer granite).

## Decision
- **Six rows** (ids 100 to 105, clear of the rows other work adds): iron-stained rock, copper-stained rock,
  quartz-veined rock, greisen, bleached rock, dolomite. Placeholder looks; the textures session paints them
  (`out/mining-halo-blocks.txt` in the main checkout says what each should look like).
- **Per mineral** (`halo` and `halo_m` in `data/minerals.toml`, grades outermost first):

  | Kind | Host | Outer grade | Inner grade (next to the ore) | Reach | Why |
  |---|---|---|---|---|---|
  | deep copper (chalcopyrite lump) | arc, belt | iron-stained rock | copper-stained rock | 8 m | rusty pyrite halo, green malachite near the copper |
  | tin vein (cassiterite) | granite | quartz-veined rock | greisen | 4 m | quartz stockwork, then greisen, the tin vein's own |
  | banded iron (hematite layer) | shield | bleached rock | iron-stained rock | 6 m | leached host, iron staining at the bed |
  | lead pocket (galena) | limestone | dolomite | iron-stained rock | 4 m | dolomitised limestone, pyrite rust at the ore |
  | gold vein (gold quartz) | belt | bleached rock | quartz-veined rock | 3 m | sericite bleaching, quartz veinlets at the vein |
  | uranium (pitchblende) | granite | bleached rock | iron-stained rock | 3 m | clay alteration, red hematite at the ore |

  Coal, and the bodies under the ground (gravel, cap, bog, blanket, the gossan), have none: a coal seam has
  no alteration, and the ground's bodies show themselves at the surface.
- **The rule** (`OreBody::halo_grade`, `Body::halo_at`): a cell is in a body's halo when it lies inside the
  body's ellipsoid with each half axis grown by `halo_m` and not inside the body; grade g reaches `halo_m (1 -
  g / grades)` out, so the richest grade is next to the ore. Each cell's reach is frayed by a hash of the
  cell (`FRAY`, 15 % of the reach either way) so a grade's edge is ragged, not a smooth shell: where two
  grades meet they mix over 30 % of the reach, so the richer grade gets commoner toward the body rather than
  starting at a line. Growing every axis by the same metres makes a vein's halo a sheet and a lump's a shell;
  it is the full reach along the axes and a little thinner between them (off a vein's tip, about 3 m of 4). Where two halos meet, the higher
  grade wins, the first body on a tie.
- It is asked only of rock under the soil band (4 m) that is no ore and no cave: `World::generated_under`,
  the mesher's fill (`generate`, at level 0, before the ore), and `ore::count_halo`. Ore, the ledger and its
  count are untouched: halo rock is rock, holds no metal and is never `ore_cell`.
- `World::set_ores` registers a body as far wider as its halo reaches (`HALO_MAX` 12 m caps `halo_m`), so
  chunks near a body take the cell path.
- `technoape-server report=1`: each kind with a halo lists `halo_blocks` per grade; every line also gives
  `nearest_at`, the nearest body's direction (for `run=stand at=`).

## Consequences
- **Saves:** generated ground near a body changes look in existing worlds, unedited cells only; edits are
  stored blocks and stay. A block dug from the halo before this keeps what it was. No save version moves.
- **Versions and hashes:** the placement of bodies is unchanged, so `made_hash` and the bake versions do not
  move. The rule is part of what a client derives from the map: a client and a server of different builds
  disagree about halo cells (the same as a rock or ore rule change, ADR 0063). There is no protocol version
  to bump; the halo is one more reason the server and the client must be one build.
- A halo reaching a face's seam is cut there (bodies keep 6 columns from a seam, halos reach up to 9 m): the
  mesher's padding and the report ask a face's own columns only, as physics does.
- Halo rock has its row's hardness, not the host's (granite 9, these 6 to 9).
- Cost: see Measured.

## Review
A fresh reviewer read the engine diff (principle 12). Findings and answers:
1. *Medium:* `count_halo` counted columns past a face's seam that physics never makes (the copper and banded
   iron halos reach past the 6 columns bodies keep). Fixed: it, and the mesher's padding, ask the face's own
   columns only.
2. *Medium:* no cost number. Measured below (`crates/engine/tests/gen_cost.rs`).
3. *Medium:* the rule took a cosine and a sine per grade tested, up to four pairs a cell. Fixed: a cell goes to
   the body's frame once (`OreBody::local`), each grade is then three divides.
4. The shape note said a grown ellipsoid is thicker between its axes; it is thinner. Corrected.
5. Two grades interleave where they meet, and the line test checks the order in which grades first appear.
   Accepted: that is the fray, and no grade lies outside its band (the box test).
6. `cos`/`sin` come from each platform's maths library, so a cell right on a grade's edge could differ in
   its last bit between a Linux server and a Windows client. The same holds for ore (`across`) since ADR 0063;
   a halo swaps one solid block for another, so collision cannot differ. Accepted with ore's.
7. Test gaps (overlapping halos, the soil band, a cave in a halo): the extended ore test has a cave through a
   copper lump's halo; overlaps and the band are covered by the rule's order, not a test. Accepted.

## Calls for the developer to confirm
- New rows over a look on the host (the host is not seen inside a halo).
- Two grades per mineral, and the reaches above (tuned by playing).
- Coal without a halo.
- Halo rock drops itself when dug, like any rock (rubble later, ADR 0087).
