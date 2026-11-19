# 0061 – Seasonal looks: four rows a block, mixed in the shader

- **Status:** accepted (routine technical calls; every colour and how long autumn lasts are taste calls open
  for the developer)
- **Date:** 2026-11-12

## Context
Worldgen plan W3b, `docs/design/seasonal-looks.md`: grass and leaves stay one block each and their look
follows the season (the developer's call, 2026-11-12). The design asks for a spike first: what the mix costs
in the shader on `stand` and `orbit`, and whether the look table fits as four rows a block or needs a second
table for the few blocks with seasons.

## Options considered
1. **Four rows a block in the one look table** (256 to 1024 rows of 64 bytes, 64 KB). No new binding; a
   season's row is `looks[season * 256 + block]`.
2. **A second table** for the blocks with seasons, indexed through the first. Saves 48 KB, costs a binding in
   three bind groups and one more dependent read.
3. **A changed block or a remesh per season.** Ruled out by the design: nothing stored, nothing meshed again.

## Decision
Option 1.

- **Data:** a row of `data/blocks.toml` may name `[block.spring]`, `[block.autumn]` and `[block.winter]`,
  each with `colour`, `colour2`, `colour3`, `amount`, `contrast` and nothing else; the row's own look is
  summer's and a season it leaves out is that too. A season's colour brings its own darker and lighter parts
  unless the season or the row names them. `stagger` is 0 to 10 days. `blocks::Def::tone(season)` gives a
  season's look.
- **The season at a place** is `sky::season_at(up, t)`: the year's phase in quarter turns (the south half a
  year on) and a strength, a smooth step on the latitude's sine from 10 to 35 degrees. The shader's `season`
  is its port. On the sine, not the angle as the design wrote it: no `asin` per fragment; the ends are the same.
- **In the shader:** each body carries its year's phase in the free float of `Body` (negative: no seasons;
  only the planet has them). `look_in` returns a block's row untouched when the block has no seasons (bit 8
  of the row's `size`) or the place has none; else the two nearest seasons' rows mixed by the phase, mixed
  with the row's own by the strength. The terrain's fragments, the effects' pieces of blocks and a ray's seen
  colour all take it.
- **Stagger:** a column's phase shifts by up to the row's `stagger`, two thirds of it by its patch of 4 x 4
  columns and a third by the column, so a tree turns nearly as one and trees turn one by one. A ray's seen
  colour takes no stagger.
- **First rows:** grass, the tundra, cold, meadow, scrub and marsh grasses, broadleaf leaves, shrub, reed.
  Spruce, jungle and acacia leaves, the savanna and jungle grasses and the cactus have none. The colours are
  first guesses for the developer to change.

## Consequences
- **Cost** (the spike; old and new binary in turn, three quick runs each, medians): the GPU stages' total
  1.331 to 1.359 ms on `stand`, 1.587 to 1.628 ms on `orbit`, about 0.03 ms (2 %) on both; nothing on the
  frame thread or any worker; 48 KB more in the look table.
- A year at the spawn and from orbit: `python -I tools/year_sheet.py out/x.png <run=stand's arguments>` makes
  the 12 frames. The north browns in its autumn while the south does not.
- The fixed views at times other than midsummer changed where they show a block with seasons (`seam`,
  `corner`, `orbit`, the three `season-*`); `coast` did not change by a pixel, `lod`, `high` and
  `season-summer` by under 0.04 % (their clocks are within half a day of a pure season).
- **Not built yet:** none of the design's parts; snow and bare trees stay outside this ADR.
- **The review** (a fresh reader of the diff) found nothing serious and three lesser points, all fixed: the
  stagger was hashed on a chunk's own cells, so a far tree could change colour as its level changed (now on
  level-0 columns); a `stagger` on a row without seasons would have sent it down the mixing path (now
  refused by `Registry::parse`); the look table had no test (`the_look_table_has_a_row_a_season`).
- With the stagger computed for every terrain fragment, old against new again: `fly` 1.79 against 1.82 ms,
  `stand` 1.42 against 1.38 ms, mesh jobs and memory the same (taken while another session compiled, so
  only the comparison counts, not the numbers).

## The rest of the design (painted seasons, the Studio, the hotbar)
- **Painted seasons:** `data/textures/<block>[_top|_side|_bottom]@spring|@autumn|@winter.png` beside the base
  files (ADR 0045's rule each); the file without one is summer's. A face a season does not paint is the
  base's, a summer or unknown season is refused by name (`textures::Set::seasons`). Each season's paint goes
  in its look row's `tex`, `tex2` and `mean` (a season without paint of its own takes the base's), and a face
  that is the base's shares its layers. A season's paint alone gives a block seasons (bit 8 of `size`); bit 9
  says a season paints its own. The shader draws the face as before (the base's paint, else `look_in`'s
  pattern, a grass side its side block's) and, only under bit 9, mixes in the two nearest seasons' rows that
  paint this face, as `look_in` mixes; the seen colour mixes the rows' means. A block no season paints costs
  what it did.
  `blocks::SEASONS` (the year's order) and `blocks::ROW` (its look row) name the seasons for files and tabs.
- **The hotbar, the "looking at" cube and the block in hand** show the season where the player stands
  (`gpu::season_here`: `sky::season_at`, none off the planet), mixed as `look_in` mixes (no stagger);
  painted blocks by their rows' means.
- **The Studio:** the Textures page has four season tabs (summer is the base files; Add season copies the
  base's files unsaved and writes a `[block.<season>]` with the base colour, Remove deletes both), a Year
  slider (day 0 to 53, each season's tab sets the day nearest its start) and North, Equator and South chips.
  The game view takes `season north|equator|south`: the seasons whole everywhere, as at that latitude
  (`Body.year` 8 and on is the phase at full strength), or none. `check=1` takes the season files, `shot=`
  takes `season=`. `data/palette.toml` gains the ramps `autumn` and `dry grass`.
- **Checks:** `out/hotbar-year.png` (a play run, the clock set every 4.5 days: the block in hand turns) and a
  year at the spawn with a scratch `meadow grass_top@autumn.png` and `broadleaf leaves@winter.png`
  (`tools/year_sheet.py ... textures=dir`): the meadow orange at autumn's start, the trees white in winter.
- **The review** of this part (a fresh reader of the diff) found: one season's side paint turned off a grass
  side's side block in every season (the first version ORed every row's paint into the face's choice); every
  block with seasons read four more words a texel; the unpainted rows were drawn one pattern each and cross-
  faded, not mixed as `look_in` mixes; season files on a block without a season look never showed; the hotbar
  lost the summer pattern when only a season was painted. All fixed as above. It also noted `place` scans the
  placed rows per face: on a reload only, left.

- **Cost:** main's binary (c5bae22) against this one in turn, three quick runs each, medians: `sim` jobs 338
  against 333 us, `orbit` 211 against 207 us, `edit` 305 against 309 us, the GPU's p50 the same or lower. The
  gate itself failed twice on worker and tick times while other sessions compiled on the PC; main fails the
  same numbers the same way then (`sim` tick p99 5.0 ms on main, 2.8 here, against a 1.4 baseline).
