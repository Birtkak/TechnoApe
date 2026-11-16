# Seasonal looks: one block, a look per season

- **Status:** design proposal, 2026-11-12, for an Opus coding session; **built (ADR 0061)**. The developer's call (2026-11-12):
  living blocks such as grass and leaves stay **one block with one name**, and their colour and look depend on
  the season; the Studio paints the seasons of a block. Everything else here is a routine technical default
  until spiked.
- **Why now:** W3 gave the air its seasons (ADR 0060: the spawn's noon runs from -2 C to 28 C), but the four
  season views differ only in light. The ground must show the year.
- **The rule it obeys** (ADR 0020's, applied to looks): a look that follows from the clock and the place is
  stored nowhere. No block changes, nothing is saved, nothing goes over the wire, no chunk is meshed again.

## What the player sees
- Grass is fresh green in spring, full green in summer, yellowing in autumn, dull and brown in winter.
- Broadleaf leaves go green, then yellow, orange and brown in autumn; trees turn **one by one**, not all on
  the same day. Spruce stays green.
- The south has its winter when the north has its summer. Near the equator nothing changes.
- A leaf block the player placed turns too: it is the same block.

## The season at a place
One pure function, as the sky's are, shared by the engine (tests, the Studio) and the shader:

`season(direction, t) -> (phase, strength)`

- `phase`: the year's angle from `sky.rs` (0 the northern spring equinox, a quarter turn per season), plus
  half a turn in the southern hemisphere. Spring, summer, autumn and winter sit at 0, 1, 2, 3 quarter turns.
- `strength`: 0 at the equator, 1 from the mid latitudes on: `smoothstep(10 deg, 35 deg, |latitude|)`.
- A block's look is the two neighbouring seasons' looks mixed by `phase`, and that mixed with the summer look
  by `strength`. So the tropics stay summer and the change over a season is smooth.
- **Stagger:** each block column shifts its own phase by a hash of its place, up to about 3 days either way
  (a number per block row, default 0; leaves take it, grass takes little). This is what turns trees one by one.
- `shortcut:` latitude stands in for the climate. A warm coast and a cold mountain at one latitude turn
  together. Upgrade when it looks wrong: take the strength from the baked climate's summer-to-winter swing,
  one number per chunk.
- `shortcut:` no lag: the browning follows the sun, not the temperature that trails it. Upgrade: a fixed
  lag in days, one constant.

## Data
A block row gains optional season looks. A season it does not name is its base look, and the base look is
summer's. A block with no seasons is untouched, pixel for pixel.

```toml
[[block]]
name = "broadleaf leaves"
colour = [0.22, 0.45, 0.15]
pattern = "speckle"
stagger = 3.0                      # days, either way
[block.spring]
colour = [0.35, 0.58, 0.20]
[block.autumn]
colour = [0.72, 0.42, 0.12]
colour2 = [0.60, 0.20, 0.08]
[block.winter]
colour = [0.38, 0.28, 0.18]
```

- A season may set what a look already has: `colour`, `colour2`, `colour3`, `amount`, `contrast`. Not the
  pattern, not the shape, nothing the server reads.
- **Painted blocks** (ADR 0045): `data/textures/<block>[_top|_side|_bottom][@spring|@autumn|@winter].png`.
  The file without a season is summer's and the base. A refused file is named on stderr as today.
- First rows to get seasons: grass, broadleaf leaves, shrub, reed. Spruce, jungle and acacia leaves and the
  cactus get none.
- Schema checks in `blocks.rs`: a season names only look fields; `stagger` is 0 to 10 days.

## In the renderer
- The look table holds a block's four season looks; the fragment shader picks and mixes them from the
  fragment's direction on the planet and the clock. Cost per fragment: one dot product with the axis and
  two mixes. **No CPU cost per frame, no remesh**, which is the point (the game will be CPU-bound).
- Painted faces: the season's layers sit beside the base layers in the same buffer (ADR 0045).
- The far terrain, the orbit view and a ray's seen colour (`seen_colour`) take the same season, so a forest
  seen from orbit is brown in its winter. Luna has no seasons: its strength is 0.
- A block in the hand, the hotbar and the inventory shows the look of the season where the player stands.
- **Spike first** (CLAUDE.md, principle 1): the cost in the shader on `stand` and `orbit` against the
  baseline, and whether the look table fits as four rows per block or needs a second table for the few
  blocks that have seasons.

## In the Studio
- **Textures and the block's look:** a row of four season tabs (Spring, Summer, Autumn, Winter) on a block.
  A season with no look of its own shows the base, greyed; "Add season" copies the base into it, "Remove"
  drops it. Painting and the palette work as on any texture.
- **The viewport** (the game's renderer, ADR 0049) gets a year slider beside its clock and a latitude
  choice (north, equator, south), so the mix and the stagger are seen moving, not only the four ends.
- **The palette** gets ramps for the turning leaves and the winter grass (`autumn`, `dry grass`).
- `check=1` covers the season files; `shot=` takes `season=`.

## Not in this design
- **Snow on the ground.** It is weather, not a season: it follows the live air's temperature and what fell,
  so it needs the air grid's state at the place. Its own design, after this.
- **Bare trees.** Leaves that fall and leave branches change what a block is to light and sound. Later, with
  living plants (W7).
- Crops and plants that grow by season: W7.

## Taste calls
1. **The colours** of each season for grass, broadleaf leaves, shrub and reed: the developer picks from a
   sheet of the spawn at four seasons, with two or three candidates per block.
2. **How long autumn's colour lasts** before the brown (default: the whole autumn quarter, brown by winter).

## Checks
- A test of `season`: the equator's strength is 0, the hemispheres are half a turn apart, the four season
  times of `sky::next_season` give the four pure looks.
- The fixed views without a season block in them do not change by a pixel; the four `season-*` views now
  differ and are accepted after the developer's look.
- A filmstrip of the spawn over one year (12 frames) and one from orbit.
- `stand`, `fly` and `orbit` against `tests/perf/baseline.json`.
