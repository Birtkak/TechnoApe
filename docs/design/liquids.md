# Liquids and gases as one system

- **Status:** design proposal, 2026-11-23. The developer's calls are in ADR 0177
  (`docs/decisions/sky-weather-and-water.md`); everything else here is a routine default until its spike has
  measured it. It builds on the water of ADR 0020 (`crates/engine/src/liquid.rs`), the tides of ADR 0067, the air
  grid of ADR 0060 (`air.md`) and the pipes of `networks.md`.
- **Why now:** an F9 report (`bugs/2026-11-21_18-21-24`): a block placed in the tidal band makes the sea glitch.
  The tide is a formula on the untouched sea and a vertex offset in the shader; stored water never feels Luna, it
  is only woken once a second where it borders the open sea. The glitch lives in that seam. The developer:
  "I don't want to fake things".

## The rule

**One physics, two ways to hold it.** A fluid at rest and untouched is held by a formula (generated from the
seed and the sky, like terrain: nothing stored, nothing sent). A fluid that was touched is held as counted
units and simulated. The formula is **the state the simulation settles to**, so a place behaves the same the
moment it becomes stored. A test holds every formula to that: store a stretch of untouched fluid, step it, and it
does not move.

What is fake today, by that rule:
1. **Gravity.** Done from world generation 8 (real gravity): `Ground::g` gives each body's surface pull to the
   link speed-up, `fall_cap`, `drag_for` and `steady_speed`. Worlds before it keep 9.81 (`liquid::EARTH_G`)
   on every body. The pull does not yet fall off with depth.
2. **The tide.** Stored water has no tidal term; `retide` nudges it at the stored edge. Under real gravity
   Terra's tide is half a millimetre, so this waits for a body with a sea and a large tide.
3. ~~**One liquid.** A gap holds one number, so nothing can lie on, sink through or react with water.~~ Done in
   step 4: a gap holds layers by density.

## Forces

The acceleration on a link between two columns is the gradient of one **head**: the surface height times the
body's pull at that depth, plus the tidal potential of every other body.

- **Gravity** comes from the body (`Interior::weight_g`, falling off inward as the deep world already computes
  it for pressure). `CG` and `fall_cap` take it per body; waves run at sqrt(g depth) for that g.
- **Tides** from the general form of today's `Sky::tide`: for each other body k of mass M_k at distance d_k,
  the equilibrium rise is `h_k (3 (up . dir_k)^2 - 1) / 2` with `h_k` from M_k, d_k and the body's own radius
  and pull. Luna and the sun on Terra are two terms of the same sum; Jupiter on Callisto is a third (and large:
  a locked moon carries a fixed bulge, which the bake's sea level should already include). Amplitudes are
  **derived** from the bodies' masses (their rows' densities and radii), distances and the body's own pull, not
  `TIDE_LUNA = 0.5` (the developer, 2026-11-23: derived, "as close to real astrophysics as possible"). No
  scale factor: if a derived tide is unplayable, the bodies' rows change (a mass, a distance), not the physics.
  The spike prints every body's derived tides first, so the sizes are known before anything is built.
- The untouched sea's level stays `mean + tide(up, t)`, the equilibrium of that head. Stored water gets the
  same term as a head offset on every link, so a harbour, a tide pool and a tide mill work with no special rule,
  and `retide` and the dead band go.
- **Open:** equilibrium tides have no lag and no resonance (a real bay amplifies). A stored bay will show its
  own slosh; the open sea will not. That is honest by the rule above only at rest; say so, and measure it.

## Fluids are rows

`data/fluids.toml`, a row a fluid: `density`, `viscosity` (the link's friction), `freeze`/`boil` points and
what it turns into, `heat` capacity, look, and `reacts = [{ with, gives, heat }]`.

First rows (the developer's list): **water, lava, liquid metals** (one row a metal, from the material's row:
copper, tin, bronze, iron; the metal sea of ADR 0156 is one), **oil, toxic sludge**.

## Mixing in the world

A gap holds a **stack of layers** sorted by density (oil on water on sludge on metal), each a count of units
of one fluid. Each layer flows by its own surface's head, pressed by the layers over it. Cost stays by area: a
column with one fluid is today's one number.

- **Displacement:** a denser fluid arriving sinks under a lighter one, a step at a time, with a rate.
- **Reactions** where two layers meet or a fluid meets a block, from the rows: lava and water give rock
  (basalt, obsidian by how fast) and steam; hot metal and water give steam violently; sludge poisons water
  (a pollutant count riding in the water, the same units the air grid carries); oil burns.
- **Heat** rides with the fluid (a temperature a layer), as `networks.md` already plans for pipes. Lava
  cools to rock from the edge; metal freezes in a mould; water freezes to ice and boils to steam.
- **The ledger** counts every fluid apart, and a reaction books what it consumed and made, so `counted()` stays
  exact per fluid. A pipe's open end, a tank and a bucket use the same units.
- **Open:** miscible pairs (sludge in water) as a concentration instead of a layer; how many layers a gap may
  hold before the thinnest merges (a cap of 4 is the first guess).

## Gases

The air grid (`air.md`) is already counted and conserving, with 128 m cells. The same head moves it:

- **Gravity** sets the air's scale height per body (thin on a small body, none where `air = false`).
- **Atmospheric tides:** the same tidal potential on the air's columns gives a pressure wave twice a day; the
  sun's heating gives a larger daily one. On Terra with Luna it is a few percent of a cell's pressure.
- **Does it change play?** By itself, barely: a small, regular wind and pressure swing. It matters where the
  tide is huge (a moon close to a gas giant: winds that turn with the orbit) and once machines read pressure
  (wind power, balloons, boiling points). Proposal: put the term in, at its derived size, cost one multiply a
  column a step; make nothing depend on it until a feature asks.
- **Heavy gases** (smoke, CO2, sludge fumes, steam) as the grid's counted species already planned for
  pollution: dense ones pool in hollows and caves near the ground, which needs a finer local store than 128 m
  cells. **Open:** gas in the gaps of columns (the liquid's own structure, a compressible fluid) near players.

## Phases: one substance, three states

The developer (2026-11-23, ADR 0183): solids melt and vaporize and back; the game should feel like "a bare bones
model of the universe". So a row is a **substance**, not a fluid: its solid is a block (`data/blocks.toml`), its
liquid the counted units of this document, its gas a counted species in the air grid, and the row names all three
with the `melt` and `boil` points between them and the heat each change takes or gives (latent heat). One ledger
spans the three: a block is U units, and ice + water + vapour of a world is one number that only a reaction changes.

- **Water first** (the developer: steam, water and ice). All three homes exist: the `ice` block, stored water, and
  the air grid's vapour and temperature per cell (`air.md`). Missing is only the rule that moves units between
  them.
- **What decides:** the temperature at the place. The air's (the grid's cell, `sky::temperature` at the ground),
  a heat source beside it (fire, furnace, lava, the deep rock's heat), and pressure for the boiling point (thin air
  on Mars or a peak boils water colder; no air boils it at once, so ice there goes straight to vapour).
- **Rates, not switches:** a change moves units a step by how far the temperature is past the point and how much
  heat can flow, so a lake freezes from the top over nights, an ice block drips, a pot takes time to boil.
- **Two paces** (ADR 0187): heat exchanged with the air and sky runs faster by the year's compression (a real
  year over the game's, about 52), so ponds freeze in winter and thaw in spring; heat from a source (fire, lava,
  the rock) runs at real rates.
- **Untouched water follows the formula** (the rule at the top): the bake's sea ice is where the climate holds the
  sea under freezing; a stored patch in the same place must freeze to the same answer.
- **Open:** where the heat is held (a temperature a liquid layer and a block near a source, not every block of the
  world); steam near the ground before it joins a 128 m air cell; snow as fallen vapour in thin layers (ADR 0189); every rock's melting
  (lava is rock's liquid, so stone beside lava long enough melts) and metals' (a mould is a freeze).

## Steps, each with a test seen failing and a measured cost

1. **Gravity per body** in `liquid.rs` (`CG`, `fall_cap` from the body). Small. Test: a dam break on Callisto
   runs slower than on Terra by sqrt of the pulls' ratio.
2. **Tide as a force** (spike in `prototypes/` first): the head term on links, the general tide sum, `retide`
   removed. Test: a stored bay beside the open sea stays level with it through a tide; the F9 report's block in
   the band leaves the sea whole. Measure: awake nodes and tick time on a coast with 100,000 stored columns.
3. **Fluid rows and one more fluid** (lava; done, ADR 0186) with no mixing: a kind per node, the ledger per fluid, the wire and
   the save (a format change: its own decision).
4. ~~**Layers and reactions:** oil on water, lava into water. The mesh and the look per fluid.~~ (built, draft
   "Fluids lie in layers by density; lava and water react": a layer a node, up to 4 a gap; lava's skin to basalt
   and water to counted vapour by the heat both rows carry; a layer seen from the side under another still open.)
5. **Phases of water** (spike in `prototypes/` first): ice, water and vapour by temperature and pressure, with
   rates and the one ledger. *The first rock phase change is in (draft 2026-11-23, "Lava lights, carries a body,
   and freezes to basalt"): lava's heat in a sparse store at 1 Hz, freezing to basalt at real rates; water's
   phases are still to come.* Test: an ice block in warm air becomes exactly its units of water; water in freezing
   air becomes ice; a pot over a fire loses to the air cell's vapour what it boils off; a stored patch of polar
   sea freezes as the bake's sea ice did. Then heat in every fluid, rock to lava, liquid metals; sludge as a
   pollutant.
6. **Gases:** the tidal and gravity terms in the air grid; heavy gases near the ground.

Costs to watch (principle 8, and CPU before GPU): the tide wakes every stored column on a coast twice a day;
layers multiply the links' work in mixed columns only; reactions are edits through the ordinary queue.
