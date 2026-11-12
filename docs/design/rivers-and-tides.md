# Rivers at rest, and a sea that follows Luna

- **Status:** design proposal, 2026-11-10, for the two spikes of `worldgen-plan.md` W5. The developer's
  calls: rivers and lakes hold real counted water (ADR 0020, 0026) and tides follow Luna (ADR 0026).
  The tide's height is decided (2026-11-12): **1 m between low and high at springs**, one number in the
  body's data. Rivers and lakes are built before tides (the developer, 2026-11-12); until W3's air grid
  runs live, a reach's runoff is the bake's mean rain over its catchment. The rest is routine technical
  default until spiked.
- **The rule both obey** (ADR 0020): water that is doing what generation says costs nothing and is stored
  nowhere. Only disturbed water is simulated. Every unit is counted either way.

## Rivers

### The reach graph
- The bake leaves the river network as lines. Cut at every confluence, lake and waterfall, they are
  **reaches**: a few thousand on Terra.
- A reach holds: its line, bed profile and width (from the bake), the reaches it flows into, and one
  number that changes, **its water** in the liquid's units.
- Once a second (the air's step) each reach, upstream first:
  - takes in the runoff of its catchment (`air.md`, pass 7) and what the reaches above let out;
  - lets out a flow that grows with its water (a fixed curve per reach, from its slope and width);
  - loses a little to evaporation and seepage.
- **Lakes** are reaches with a large store and a spill level. A lake with no outlet in a dry basin
  shrinks to a salt flat by itself.
- A few thousand integer updates a second: nothing. It runs for the whole planet, always.
- Droughts, spring floods from snow melt and dry river beds all follow from the rain, with no extra rule.

### A reach at rest
- The generator fills a reach's columns to the level its water gives (a second curve per reach: water to
  depth). It is the river's version of "every column under the sea level holds water up to it".
- Levels are stepped to an eighth of a block, so a slowly rising river changes its mesh rarely.
- Speed is flow over cross-section. **A water wheel, a boat or a swimmer reads the reach's speed without
  waking any water.** Wheels sit in every early base, so this matters more than anything else here.
- Drawn as the sea is: surface quads with the chunks, moving along the line's direction.

### A reach disturbed
- An edit in or beside the river, a bucket, a pump, or live water arriving wakes the columns around it,
  exactly as touching the sea does today. They become stored, simulated water.
- The live patch has two open ends in the river. The upstream end **pours in** the reach's flow; the
  downstream end **takes out** whatever arrives. Both book against the reach's water, as the open sea
  books against `sea_extra`.
- **A dam** is then nothing special: live water piles up behind it, the downstream end receives less, the
  reaches below get less, and their generated level falls. Break it and a real wave runs down the live
  patch; past its end the reaches below simply receive more for a while.
- When the patch is calm and matches what the reach would generate, its columns are forgotten again.

### Questions for the spike
1. No visible step where a patch wakes or sleeps: the generated level and the simulated one must agree
   within the dead band (1/64 block).
2. The ledger: reach water + stored columns + the rest stays constant through a dam built and broken, in
   the soak.
3. How long a flood wave should stay live before the reaches take it back (cost against truth).
4. A pump or a wheel in an undisturbed reach wakes nothing: asserted by a work counter.
5. Waterfalls: a reach boundary with a drop. Drawn by the client; live only when touched.

## Tides

### The sea level as a function
- Today the sea is one level. With tides it is a function of place and time, as pure as the sky's:

  `level(direction, t) = sea + A_luna * shape(angle to Luna) + A_sun * shape(angle to the sun) + offset(t)`

  where `shape(a) = (3 cos² a - 1) / 2` (the classic tidal bulge: high under Luna and opposite it, low in
  between) and Luna's and the sun's directions come from `sky.rs` in Terra's own turning frame.
- **The sea's volume stays exact.** The bulge moves water, it does not make any. `offset(t)` is the small
  correction that keeps the integral over the actual oceans constant. The oceans' share of each bulge
  pattern is five numbers computed once in the bake, so the offset is a closed form each tick.
- **What it does without further rules:**
  - two high tides per lunar day, 16 minutes apart (the lunar day is 1920 s since ADR 0041);
  - spring tides when the sun and Luna line up, weak ones at half moon (*proposal:* the sun at a third of
    Luna's pull);
  - the two daily tides differ by latitude and season (Luna's orbit leans 2°, ADR 0041; the axis 25°).
- **Built** (ADR 0067): as above, with the sun at a third of Luna; the measured answers are in
  `prototypes/tides/README.md`.
- **The height** (decided 2026-11-12): 1 m between low and high at springs (2 blocks in 5 minutes: you
  can watch it, it will not drown you). On a 1 : 20 beach that is a tidal flat 20 m wide.

### What changes in the engine
| Where | Today | With tides |
|---|---|---|
| Generation | a column is sea if its ground is under the level | compared with `level(direction, t)`: the tidal band goes wet and dry with no storage |
| Sea quads | meshed for columns under the level, drawn at it | meshed for columns under the **highest** tide, never remeshed for the tide; the shader lays each vertex at `level(...)`, and the terrain hides the water where the ground is higher |
| Physics | swimming against one level | `World::water_surface` asks the function |
| Live water at the coast | the open-sea edge is held at sea level | held at `level(...)`, so the tide **really flows** into a harbour, a dug channel or a mill pond, and out again |
| The waterline rule | the level sits an eighth under a layer top so blocks do not flicker | no longer true at all times; the spike must show whether a water surface passing a block top flickers, and fix it in the shader if so |
| The ledger | `sea_extra` | unchanged: the offset keeps the generated sea's volume fixed |

- **A tide mill** then needs no rule: a pond with a gate, filled by the real inflow at high tide, drained
  through a wheel at low. It is the first power that follows the sky.

### Questions for the spike
1. The cost of the function per vertex and per physics query (expected: a dozen multiplications).
2. Flicker and pops at the waterline through a whole tide, in a filmstrip.
3. A live harbour: inflow and outflow through a 2-block gap follow the tide and the ledger holds.
4. Wet and dry sand: does the tidal band need its own surface block or look?
5. Rivers meet the sea: the last reach's outflow level follows the tide (an estuary), cheaply.
