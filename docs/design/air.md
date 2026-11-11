# The planet's air: one coarse grid for heat, water, wind and dirt

- **Status:** design proposal, 2026-11-10, for the spike ADR 0010 asks for. The developer's calls are in
  ADR 0010 (the four systems exist, planet-wide) and ADR 0026 (seasons; the bake's climate uses this
  model). Everything below is a routine technical default until the spike has measured it. The constants
  are tuning and belong in `data/air.toml`.
- **Two jobs:** the weather during play, and stage 5 of the bake (`worldgen.md`), which runs this same
  model to a steady year so biomes stand where the live weather brings rain.

## Not a weather model
The rules are picked for play (progression). Three properties are not negotiable:
1. **It conserves.** Water and each pollutant are integers moved between cells, never floats that drift.
   Water uses the liquid's unit (4,096 to a block, ADR 0020), so one ledger covers sea, rivers, soil, air
   and tanks.
2. **It is deterministic:** integer state, a fixed sweep order, random events seeded by (seed, tick, cell).
3. **It runs for the whole planet, always,** inside the tick, whether chunks are awake or not.

## The grid
- **Columns** on the cube-sphere, 256 block columns wide (one level 3 chunk): 48 x 48 a face on Terra,
  13,824 in all, each about 128 m across. The huge planet gets the same count by doubling the width.
- **Four layers** above the ground: the surface layer (to 30 m), low (to 150 m), middle (to 500 m), high
  (to 1,500 m, where the client already lets the air end). A mountain pokes into the low layer.
- **55,000 cells.** Per cell: temperature, vapour, cloud water, wind (two numbers; up and down follows
  from the others), and a small array of pollutants (smoke, acid, radioactive dust).
- **Per column, under the air:** ground temperature (a thin skin and a slow store; the sea's store is
  large), soil water, snow, and the share of the column that is sea, lake, plants or bare.
- Luna has no grid. A body without air has only the ground part: skin temperature from the sun and its
  own rock.

## One step (once a second of world time, spread over the 20 ticks)
In this order, each a sweep in fixed order:

| # | Pass | Rule |
|---|---|---|
| 1 | Sun | each column absorbs sunlight by the sun's height there (the sky's own maths, eclipses and seasons included), less what its clouds and its smoke block, less what its surface reflects (snow, sand, sea, forest) |
| 2 | Ground | skin and store exchange heat; the ground warms or cools the surface layer; everything radiates to space, less under cloud |
| 3 | Wind | the wind relaxes toward a target: bands by latitude from the spin (east winds near the equator, west winds in the middle latitudes), plus flow from cold columns to warm ones (the day and night sides, sea and land breezes), turned by the spin, slowed and bent by mountains |
| 4 | Carry | heat, vapour, cloud and dirt move with the wind between neighbouring cells, in whole units, upwind |
| 5 | Lift | air pushed up a slope or over a hot column rises a layer and cools; sinking air warms and dries (the dry side of a range) |
| 6 | Water | open water, wet soil and plants give vapour by warmth and wind; vapour over what the cell can hold at its temperature becomes cloud; cloud over a threshold falls as rain, or snow below freezing, and takes dirt down with it |
| 7 | Land | rain goes to soil water; what the soil cannot hold runs off to the column's river reaches (`rivers-and-tides.md`); snow lies and melts later; soil water feeds plants and seeps to the reaches |
| 8 | Events | storms where wind and wet are both high; lightning on the tallest block of a seeded column; fog in a cold wet surface layer |

**The 30 minute day drives it.** Sunset crosses the ground at 15 m/s, so a column's whole heating and
cooling cycle is 30 minutes: rain at dusk, a breeze off the sea by day, frost before dawn. The seasons
(54 days) and Luna's month only shift those.

## How the rest of the game touches it
- **The local temperature** an ape feels: the column's surface layer, corrected for height and depth
  (rock warms downward), plus nearby fires, lava and machines, or the room's own state if inside one.
- **Rooms** (fundamentals 1): a sealed room is its own small cell that leaks heat and air to the column
  through its walls; a door or a hole joins it.
- **Fires, boilers, chimneys, vents** add heat, vapour and smoke to the layer their top reaches. That is
  why a tall chimney helps.
- **Rain on awake chunks** arrives as water in the gaps there; elsewhere it goes straight to soil and
  reach totals.
- **Solar panels, windmills, crops, fire** read the cell: cloud shade, wind, soil water, rain.
- **Radiation** is two things. Fallout is a pollutant here. Direct radiation is not air at all: a
  line from the source through the blocks between, asked a few times a second per ape.
- **Clients** get the grid as slow deltas (the faces near them every few seconds, the whole planet
  coarser for the view from orbit) and draw clouds, rain, fog and haze from it.

## Used twice: the bake
- Stage 5 runs the model on the new planet for a few years until one year repeats the last, then stores
  per column the mean temperature, its yearly and daily swing, rain per season, snow days and wind.
- One second per step would make a year 65,000 steps. *Proposal:* the bake takes 10 s steps (120 a day),
  about 6,500 a year. A longer step can shift the climate a little, so W3's check compares the bake's
  means with a year of live weather per biome, and the tolerance it finds decides whether the bake must
  use the live step after all.

## Cost (estimates for the spike to replace)
- 55,000 cells and 14,000 columns, eight passes, a few dozen integer operations each: about 1 ms a step,
  so 0.05 ms a tick when spread. ADR 0010's target is 0.5 ms.
- Memory under 4 MB. A save holds the whole state. A client's deltas: under 1 kB/s.
- Faces can run in parallel in a checkerboard (fundamentals 3) if it is ever needed.

## Questions for the spike (`prototypes/air/`)
1. The tick cost on Terra and the huge planet, and the step the bake can take.
2. Does it look like weather? Image sheets over a day and a year: temperature, cloud, rain, wind. Wet
   west coasts, dry lee sides, a dry belt, ice at the poles, a sea breeze, dusk rain.
3. Is the upwind carry in whole units too smeary at 128 m? If so, a better scheme at the same cost.
4. Water's ledger over a simulated decade: exactly constant.
5. Does a year repeat? The climate must not drift between year 3 and year 30.
6. How strong must the day's swing be for night to be dangerous but not absurd next to the seasons?
   (Feeds the taste calls on survival and season strength.)
