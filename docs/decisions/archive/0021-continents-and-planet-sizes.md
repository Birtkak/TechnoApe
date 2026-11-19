# 0021 – A continental planet; 4 km standard, 8 km huge

- **Status:** accepted
- **Date:** 2026-11-10

## Context
The first playable's planet (ADR 0003, 1 km radius) had hills of ±30 m and a sea 4 m under the radius. That
made a pond world: lakes in hollows, no coasts, nothing to sail or fly across. The developer wants a
continental world before lighting. They also want a bigger standard planet, now that the engine holds its
budget on the 4 km headroom planet.

Taste calls by the developer:
- **4 km is the standard planet and 8 km the huge one** (N = 12288, R = 4244 m; N = 24576, R = 8488 m);
- about **55 % ocean**;
- **3 to 5 continents plus islands**;
- **hills and some mountains:** rolling hills to about 20 m, ranges to about 50 m, oceans about 25 m deep
  with shelves.

## Options considered
1. **Threshold one noise field.**
   - Pros: cheap.
   - Cons: the share of sea changes from seed to seed, and coasts are blobs.
2. **A warped low-frequency continent field, cut at a measured quantile, with metre-scale detail on top.**
   - Pros: the sea's share is exact on every seed. The continents scale with the planet, while hills,
     mountains and islands stay in metres.
   - Cons: four more noise lookups per column, and a lattice of 4096 samples when the world is made.
3. **Plates and erosion.**
   - Pros: better shapes.
   - Cons: far more code and time. Later, if the biomes ask for it.

## Decision
**Option 2, in `world.rs` (`Land`). The moon keeps its old ±30 m hills.**
- **Continents.**
  - The continent value comes from 3-octave noise at 0.75 cycles per radius.
  - Its input is domain-warped by 2-octave noise at 1.6 cycles per radius, 0.22 radii deep, so coasts
    wander.
  - The cut is the value at the 55 % quantile over a Fibonacci lattice of 4096 directions. So OCEAN of the
    surface lies under it, on any seed and size.
- **Heights** are metres over the sea, computed from `e` = continent − cut.
  - Land:
    - plains rise 14 m inland;
    - hills (1/140 m, 4 octaves) grow from ±1 m at the coast to ±10 m inland;
    - ridged mountains (1/650 m) add up to 45 m well inside a continent.
  - Sea: a shelf first, then the deep, 23 m down. Islands (1/260 m) rise from the shelf and the near deep.
  - The ground stays within AMP = 80 m of the radius.
  - Planets under 1 km radius get flattened relief, so tests on small planets keep their footing.
- **The sea level** is 4 m under the radius (ADR 0020), snapped to 1/8 block under a layer top. A block at
  the waterline then stands 6 cm out of the water instead of flickering through it.
- **The spawn** is the lattice point nearest the +y pole that is land at least 2 m over the sea.
  - `spawn_point()` stands a player above the highest ground within 1 m of it.
  - Clients, bots, the soak and the views start there.
  - Server tests find land through `on_land()`.
- **The moon** sits 2.83 planet radii out, as it did from the 1 km planet, but never nearer than 3 km (for
  small test planets). Orbit runs climb to 3.3 radii.
- **The far sea.**
  - A coarse sea cell that is solid draws its water one layer over the cell's top. Without this the far
    planet shows no sea.
  - At level 0, a block that holds the sea level stays dry.
  - Merged sea quads stay within about 90 m. A flat quad 2 km wide sags 120 m under the sphere, below the
    terrain.
- **Sizes.**
  - The default is `planet=12288` in the client, the server and the benchmarks' `small` series.
  - `huge` is 24576.
  - `base` (1536) stays as the comparable small planet.

## Consequences
- Coasts, oceans, islands and mountains show from orbit, and the start is on land near the north pole.
- 55 % of the planet is sea that costs nothing until someone touches it (ADR 0020).
- The 8 km planet holds ADR 0004 in quick stand and orbit runs (165 MB RAM, 495 MB VRAM). A full `huge`
  series is still to run.
- Walking around the standard planet takes 103 min, so travel (boats, flight, rails) is needed earlier.
- Known limits:
  - the continent field is one noise, with no plates;
  - a far chunk's coarse cells point-sample edits, so builds thinner than a coarse cell vanish from far
    away;
  - a body's grid ends at its build headroom, so nothing can be built in high orbit.
  - The last two are on the roadmap.
