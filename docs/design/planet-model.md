# The planet model: one description a body is baked and generated from

- **Status:** direction set by the developer 2026-11-18; everything marked *proposal* is open. Each build step
  gets its own ADR.
- **Why now:** in one day the deep world added a mantle, a metal sea, pressure, rock that forms by pressure and
  heat, caves by rock and a generation number, each as its own special case on body 0. The field is next, and
  the developer wants "lots of rocky and gas planets and moons" later. Without a base model every one of those
  is another `if body == 0`.

## The developer's calls (2026-11-18)
1. **A base model to build on:** the bake and the generation are reworked so pressure, the magnetic field and
   the geology come from one model, and many rocky and gas planets and moons can follow.
2. **A body is a row you tune.** A few physical inputs per body in `data/bodies.toml`; everything else is
   derived. Sol's bodies are written by hand; the seed only changes a body's detail.
3. **A gas planet can be entered and has a solid core.** Cloud layers to fly down through, pressure and heat
   rising, and deep under them a solid core that can be reached and mined late in the game.
4. **The first new body is a Mars-like planet:** rocky, cold, thin air, a dead core and so no field, old
   craters and dry river beds, giant dead volcanoes with long lava tubes.
5. From before, and they hold for every body: real-world logic decides where things are, and each must be
   something to find, mine and craft with; nothing is placed because of where the spawn is, except copper;
   relief is exaggerated for small planets; a world made earlier keeps generating as it did.

## What is wrong today
- **Two bodies, wired by index.** `with_grid` asserts two rows; body 0 gets the map, the mantle, the generation,
  the ore, the weather, the tide, the herds; body 1 gets `bake/luna.rs`, a separate pipeline. Saves, Welcome,
  recordings and the command line carry `planet_n` and `moon_n`; the sky poses one planet and one moon.
- **Terra's numbers serve as everybody's.** One `G`, one core profile (`CORE`, `CORE_DENSE`), one crust and sea
  depth, one day, year and tilt, one air top; `Body::pressure` uses them on any body.
- **Content is Terra's.** Rock provinces, minerals, caves and biomes are global tables that only make sense
  where plates, rain and life exist; Luna has no rock field at all, so it has no ore and no caves by rock.
- **Two bakes share nothing.** Terra's runs plates, erosion, rivers, climate, life; Luna's runs craters, maria
  and tubes. A Mars needs half of each.

## The model, in three layers

### 1. Inputs: a row per body (built for the interior, step 1; *proposal* for the rest)
| Input | Meaning | Terra | Luna | Mars-like | Gas giant |
|---|---|---|---|---|---|
| `kind` | rocky, icy or gas (only rocky is built) | rocky | rocky | rocky | gas |
| `size` | columns per face edge (radius) | 6144 | /3.7 | about /1.9 | far larger, see below |
| `iron` | the metal core's radius over the body's | 1/sqrt 2 (stated: the grid's floor) | 1/sqrt 2 (Terra's for now; the Moon's is 0.2) | small | small solid core |
| `heat` | how hot the inside still is, 0 dead to 1 young: the inner core is 1 - heat of the core's radius | 0.65 | 0.05 | 0.15 | 1 |
| `density` | crust, mantle, core, kg/m3 | 2700, 3300, 7920 | Terra's for now | own | - |
| `weight_g` | the pull rock is weighed with, where not the body's own (a stopgap) | - | 19.62 (Terra's, until the next generation) | - | - |
| `water` | share of the surface under liquid, or ice | 0.55 | 0 | 0, ice caps | none on a surface |
| `air` | surface pressure and gas mix | 1 atm | 0 | 0.01 atm | bottomless |
| `sun` | distance from the sun (light, warmth) | 1 | 1 | 1.5 | 5 |
| `spin`, `tilt`, `year` | day length, seasons | today's | locked | own | fast |
| `parent`, `orbit` | what it circles and how far | sun | Terra | sun | sun; its moons name it |
| `age` | how long the surface has been collecting craters | young | old | old | - |

### 2. Derived: the interior and the fields (one module, pure functions of the row)
- **Zones:** crust, mantle, liquid outer core, solid inner core, each with a radius and a density. `iron` and
  `heat` set them: a hot body with much iron has Terra's large liquid core; a cold one has frozen solid; a small
  one has almost none. The grid's depth (ADR 0152's layered grid) follows the zones, per body.
- **Gravity** at any radius from the zones' mass (ADR 0152's profile becomes the general case; surface gravity
  stops being "radius over Terra's radius").
- **Pressure** (ADR 0158) and the **geotherm** (heat with depth) from the same zones, per body.
- **Magnetic field:** strength from the liquid core's volume and the spin; none without a liquid core. It is
  the number `deep-world.md` section 3 spends: aurora, radiation, air and water kept or lost, compasses.
  Draining the core lowers the liquid volume, and the field follows from the same formula.
- **Air kept:** from gravity, the field, `sun` and `heat`. It is why Luna has none and Mars little, and what a
  planet with a mined-out core loses.
- **Regime:** *plates* (hot, with water), *one plate* (warm: volcanoes over hot spots, no drifting plates),
  *dead* (cold: only impacts). Which rocks, caves and ores can exist follows from it.
- **Surface temperature and what flows:** water, ice, or nothing; later methane on a cold moon.

### 3. The bake: stages, switched on by what was derived
One pipeline; a body runs the stages its model calls for, in this order, each writing layers of one `Map`:

| Stage | Runs when | Terra | Luna | Mars-like |
|---|---|---|---|---|
| Crust and plates | always (plates only in the plates regime) | yes | crust only | crust only |
| Volcanism: hot spots, shields, flows, tubes | `heat` now or in the past | arcs, rifts | maria, tubes | giant shields, long tubes |
| Impacts: craters by `age`, thinned by air and by resurfacing | always | few | many | many |
| Erosion and rivers | liquid on the surface, now or once | yes | no | dry beds, old deltas |
| Ice | cold enough | caps | polar shadow | caps |
| Climate | air | yes | no | thin, dust |
| Life | a biome table for the body | yes | no | no |

- **Rock provinces everywhere.** Every rocky body gets a rock field (Luna: highland anorthosite, mare basalt;
  Mars: basalt shields, old cratered crust, sediment in old basins), so minerals and caves by rock work on all
  of them through the tables that exist (`rocks.toml`, `minerals.toml`, `caves.toml`), which gain a "which
  bodies or regimes" key instead of meaning Terra.
- **The map carries what the model needs to be read back:** the body row's hash and the stages run, so a save
  made earlier re-bakes the same way.

## Gas planets (*proposal*, the developer chose the hardest form)
- No blocks of its own until the core: the body is its derived profile (pressure, heat, wind, gas mix by depth)
  drawn as cloud decks you fly through. Hull limits by pressure and heat gate the dive, as suits gate the
  mantle. Gases are harvested by depth; platforms float where their density matches.
- Deep down, a **solid core** on the same layered grid as any rocky body, under an ocean of dense fluid: the
  metal sea's mechanics (one level per body, ADR 0156) reused for the layer above the core.
- **Its size is a real giant's** (the developer, 2026-11-18: "why couldn't we have a gas giant with ~300 Terra
  masses?"): about 300 Terra masses and 11 Terra radii (some 47 km), which gives about 2.5 times Terra's gravity
  at the cloud tops, as Jupiter has. Gas has no blocks and bodies move on rails, so the mass and size cost the
  engine nothing; the costs are the cloud renderer, the 40 km dive, and the core. Only the **core is blocks**:
  its size is a row value (*proposal*: between Luna's and Terra's; a real one would be larger than Terra, with a
  bake to match). Its moons are ordinary rows whose `parent` is the giant.
- A strong field and a radiation belt around it follow from the model (fast spin, large fluid core).

## Orbits are real, later (the developer, 2026-11-18: "space exploration will depend on it")
Today every body is a circle on rails with a hand-set period, and a ship feels only the nearest body's pull
inside a sphere of influence. The model is shaped now so real orbits need no second rework:
- **A row states an orbit, not a period:** parent, distance, eccentricity, inclination. The period follows from
  the parent's mass (Kepler's third law), and the mass follows from the zones. One constant scales it so Luna's
  month and Terra's year stay what they are today.
- **Bodies stay on rails, but Kepler's rails** (ellipses, fixed from the row): deterministic from the clock, so
  server, clients and replays agree without simulating the planets (*proposal*, as flight games do).
- **Ships and loose things fall under real gravity** from the masses: orbits, transfers, slingshots, capture by
  a moon. That is the later step; it needs a ship worth flying first.
- What it must not break: the perfect eclipses (Luna's size and distance against the sun's), tides, day length.

## Build order (*proposal*; one feature a run, each keeps earlier worlds bit for bit)
1. **Built (0162-planet-interior):** the row and the derived module, proving nothing changed. `bodies.toml`
   gained `kind`, `iron`, `heat`, `density` (and Luna's stopgap `weight_g`); `interior.rs` derives the zones,
   mass, gravity at any radius, the weight of rock and pressure, the liquid core and the field (a value in
   `report=1`), replacing the constants; Terra and Luna come out identical (hash and bit locks). Left for later
   steps: the geotherm and the regime.
2. **Built (0163-n-bodies):** N bodies instead of two. A row names its `parent` (the sun or a row above it),
   whether it is the `home` (the first row) and its `bake`; a body is posed on its parent's pose (Terra and Luna
   to the bit as before; one round the sun that is not home on a plain ring until real orbits). Saves (version
   22), Welcome and recordings carry every body's columns and map as a list; version 21 and old recordings read
   as Terra and Luna. `planet=`/`moon=` unchanged. The GPU's four bodies are a stated limit, refused at start.
   Left: eclipses, tides and moonlight are the home's first moon's only; the home must be the first row.
3. **Built (draft-magnetic-field):** the magnetic field as a value of the world, `World::field(b)` from the
   interior (no new state: the metal sea is a fixed level, ADR 0156; the ADR names the counted liquid metal, save
   and wire field core mining must add), and its first effect, the aurora: an oval at 70° latitude at field 1 that
   widens toward the equator and dims as the field falls, none under 0.02 (Luna). `field=x` scales it for a run.
4. **One bake pipeline.** Terra's and Luna's bakes become stages of one; Luna gets a rock field, and with it
   ore and caves by rock.
5. **The Mars-like planet**, the first body made only from a row.
6. Later: the gas kind and its moons; icy bodies; other stars.

## Open
- The exact fields of a row and their units; how exaggeration is expressed per body.
- Whether Terra's surface gravity should change at all when gravity comes from mass (today's feel is tuned).
- How much of a new body's bake time is acceptable (Terra's is about 36 s, once).
- Travel time and fuel between bodies, and where the Mars-like planet sits in the ages (`progression.md`).
- The cost of this session's deep-world steps (`docs/perf-debt.md`) should be won back before or with step 1:
  the derived module is where per-column caching of boundaries and cave spans belongs.
