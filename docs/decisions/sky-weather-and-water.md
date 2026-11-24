# Sky, weather and water

How water is simulated, drawn and bedded; the seasons' looks; the weather the player sees and hears (clouds, rain,
snow, wind, storms); the light snapped to texels; the globe map. What holds today. Numbers in brackets are the ADRs
(`archive/NNNN-*.md`) a decision came from; INDEX.md lists every number.

## The developer's calls

- Water has momentum and waves; a generated sea now, basin lakes with the biomes; splashes as client particles now,
  12.5 cm cells later; a bucket and simple swimming; creative pours freely but counted; no endless sources [0020].
- Grass and leaves stay one block each and their look follows the season; nothing stored, nothing meshed again per
  season [0061].
- Water "still looks ugly": the look is traced and clear, still on the pixel grid [0071].
- The water's clock steps 4 times a second so ripples, foam and falls jump like pixel art instead of gliding [0071].
- Blocks set into water must not make water behave strangely [0071].
- River, sea and lake bottoms are clay, sand or stone, whatever is geologically right; dirt too, never grass [0086].
- Rain must not fall above the clouds ("its snowing while i am above the clouds") [0108].
- The weather's cloud shadow is soft and cloud-shaped, not the cells' hard squares [0076].
- Lightning is rarer than the first version: every 20 to 40 s under a storm [0076].
- Clouds feel bunched and too dense: scattered cumulus with blue between in fair weather (a look call left to
  Claude, [0108]).
- The light is snapped to the block texture's texels, so shadows read as pixel art, not grain [0108].
- Creative sees the whole globe map, survival only what it has seen; survival has waypoints on the globe too [0077].
- **Liquids and gases are one system, and nothing is faked** [0177, 2026-11-23]: an untouched fluid may follow a
  formula as long as it is the state the simulation settles to; every fluid answers to its body's gravity and to
  the tides of its moons and sun; the first liquids are water, lava, liquid metals, oil and toxic sludge, and they
  mix and react in the open world, not only in machines; gases follow the same logic, with atmospheric tides where
  they change play or the simulation. Tide sizes are derived from the bodies' masses and distances, with no fixed
  amplitude and no scale: the simulation is to be as close to real astrophysics as possible. The design and its steps: `docs/design/liquids.md` (a proposal; each step is
  spiked and measured before it is built). Replaces, when step 2 is built: 0067's tide as a level only (`retide`,
  the shader's offset on a sea meshed once). Open: equilibrium tides have no lag; the layer cap; gases near the ground.
- Open taste calls (colours and amounts are first guesses): every seasonal colour and how long autumn lasts [0061];
  the water's colours (`WATER_FOG`, the absorption) and how fast the underwater fog closes [0071]; foam as a thin
  broken line, not surf over the whole shelf [0071]; the Low preset does not set Water [0071]; cloud height, cell
  size, how much sky the cover fills, rain density and sound levels, snow white from 10 mm of water [0076]; the bed
  numbers (35 degrees, 1 %, 8 m, 1.5 m, 0.5 m), deep sea as mud over clay, alpine lakes as bare rock [0086].

## The water's simulation (`crates/engine/src/liquid.rs`)

The server simulates; clients show it [0020].

- **Model.** Shallow water on the columns' air runs ("gaps"): one node per gap holding water, a velocity per link
  (virtual pipes). Water is in integer units, 4096 to a block (`U`), so counts are exact [0020].
- Speed carries on from link to link; friction, a thin-sheet drag and a stop speed slow it. A fall into a gap
  passes at most Torricelli's flow. A full gap under a ceiling holds 1/16 block extra per block of head, so closed
  tunnels pass pressure on [0020].
- A step is 1/60 s; the server runs as many steps as the physics does [0020].
- **Closed gaps.** A link into or out of a closed gap passes at most what would level the two heads (half the
  difference, or the room plus the rest over K + 1 once the gap is full, K = 16), shared over the gap's links; the
  gap settles in a step, full, with the pressure share that matches the sea [0071].
- **Activity.** Only awake nodes are stepped; a node sleeps after 30 quiet steps. Every 15 steps a calm body is laid
  exactly flat (binary search for its level, every unit kept) and put to sleep [0020].
- **Budget.** At most 60 000 node-steps a tick (about 7 ms). Past that, water slows down instead of the server
  lagging. The count of skipped steps goes on the stats line [0020].
- **Blocks set into water.** Water a block displaced whose gap above stands over the column's generated open water
  joins that water's account (`book(reach)`), only the part under that level and not in a river, so no puddle stays
  on the block and the ledger stays exact. After a dig the column closes and is forgotten (generation's sea again).
  Test: `a_block_placed_in_the_sea_and_dug_leaves_the_sea_as_it_was` [0071].
- **Play.** B takes the bucket: left click scoops up to a block, right click pours it. Creative pours and scoops a
  full block every time, counted. Swimming is in `phys::step`: buoyancy holds a body 80 % under water, drag, jump
  held swims up at 2 m/s, walking is slower by how deep you are [0020]. (also player-and-feel: the sprint's `sub`
  share and the dive key are in 0118.)

### Heat runs on two constants: a pace of 6 and blocks that store a fiftieth [0200, 2026-11-24]
The developer: "pace 6, bulk 300". **No one speed can serve:** a process held back by a block's stored heat (a
pond's ice, a pot's boil, a pit of lava's freeze) takes days to months at real rates, because a block is a tonne,
while small and surface ones (a flame across a plank, a hand by a fire) take seconds to hours; one factor fast enough
for the first makes the second instant. So heat runs on **two world constants** (`interior::HEAT_PACE`, `LIGHT`,
beside `G` and, like the stronger G of ADR 0180, a taste call): **every heat flow is `HEAT_PACE` = 6 times the real
one**, without exception (every conductivity, film coefficient, radiative term and contact term: `liquid::surface`
and `loss`, the rock's effusivity, `phases::Phases` k, k_ice, h and the sea's flux, the held rock's `ROCK_K`, a hot
fluid beside an ice block), and **every block and fluid stores `LIGHT` = 50 times less heat** than it really would
(specific and latent heats of water, ice and lava; the rock's volumetric heat, so its effusivity goes as
sqrt(6 / 50)). Densities, units and temperatures are untouched; the vapour curves keep the real latent heat.
What follows: what a block's heat holds back runs **300 times real**, a body that is not a block (a creature, later)
feels the pace alone, every steady temperature is the real one (T*, the rest depths, 100.000 C, 1.946 C at 700 Pa, the
sea's rest ice), and both sides of every exchange run on one rate, so ratios hold (a skin of lava boils the same water).
The closed forms say so: Stefan's growth rho L / dT (d^2 / 2k + d / h), the cube's melt 3 rho L / (5 h dT), the contact
term's 1 / sqrt(pi alpha t) (alpha 300 times, e sqrt(6/50) times) each come to real time / 300 (asserted in the tests).
Sublimation's vapour carries the real latent heat at the pace out of a block that stores a fiftieth: its mass flux is
300 times real, its surface the real one. **The one row exception:** a fluid row's `heat.pace` (`data/fluids.toml`,
over 0, default 1) slows its own loss to the air and rock; lava's is 0.5 (the developer, 2026-11-24, "make heat loss
a bit slower", so a flow runs further), so lava freezes at 150 times real. `react` has no rate: a skin a second, its
heat (real, in ratio, `LIGHT` cancels) exactly the water's boil; lava beside an ice block gives it the plain pace.
Matter flows keep the world's clock (an eruption's supply and `effuse`, ADR 0196; ground water, 0190). The calendar's
real-year factor is no longer used for heat. **Cost accepted:** a winter grows more ice than a real one: a pond under
-10 C air over a game winter (a quarter of 601,200 s, the real equivalent 4.5e7 s) by Stefan grows about 2.3 m,
against 0.9 m in a real three-month winter (2.6 times; it was a real winter's at 0187's 52.5).

| Tests (game seconds are seconds of play) | real | at 0187 | now (hand) |
|---|---|---|---|
| a block of water down an open shaft on 300 C rock, air 300 C (by hand) | 2.1e5 s | 2.1e5 s (58 h) | 700 s (12 min) |
| 4/3 block on 200.3 C rock under 199.6 C air (1536 world) | 816,748 s | 813,000 s | 2,760 s (2,722) |
| 4/3 block sealed in on 300 C rock | 3.91e6 s | 3,910,800 s | 13,080 s (13,036) |
| 1/8 block on 1000 C rock, one atmosphere / 700 Pa | 185,710 / 94,370 s | the same | 620 s / 320 s |
| 4/3 block in a shaft on 400 C rock under 20 C air, to one block | 3.18e6 s | 3,184,200 s | 10,560 s (10,621); rests at 0.843 m |
| 3 x 3 pit of lava two deep freezing through (lava at 150x) | about 5.2e5 s | about 5 days | 3,191 s at dt 1, 3,250 at dt 10 (3,467) |
| a sheet of lava half a block deep (by hand) | about 7e4 s | the same | about 470 s; liquid its first 130 s |
| a sheet an eighth of a block deep (by hand) | about 5e3 s | the same | about 33 s |
| an ice block's melt in +10 C air | 1.34e6 s | 24,540 s | 4,140 s (4,470) |
| the pond's two Stefan blocks under -10 C air | 9.9e6, 3.5e7 s | 188,820, 664,320 s | 33,000, 116,220 s (33,046, 116,242) |
| an ice block sublimating in no air | 3.2e6 s | 60,180 s | 10,380 s |

Replaces: 0187 (both its paces) and the single 52.5 pace on every flow drafted earlier on 2026-11-24; 0191's "gas
near a fire runs at real rates"; the real rates of 0193 and 0195 for lava and rock.
A coarse step never carries a fluid's heat past its place's temperature, and it freezes only from heat taken below
its freeze point (a thin sheet on ground over the freeze point stays liquid at any step). A fluid beside another
fluid loses nothing through that face (the exchange is `react`'s alone); through a face on an ice block it loses
exactly what `thaw` melts by (one function, `liquid::ice_face`, at the plain pace). A sheet frozen short of a block
is kept by its column (`Liquids::crust`, under a block, saved as a fifth tail) and joins the next freeze there.
Limits: `react` runs once a call, not by `dt`, so under `/timewarp` lava meeting water reacts slower than the rest.
An eruption's flow crusts in minutes at lava's 0.5: its 64 m3 on the small planet's cone (poured in 64 s) is 22 %
frozen at 80 s, half at 110 s and all by 190 s, so a flow runs a short way down the cone; the row's number is the
developer's knob. Rock frozen short of a block is held unseen (shortcut until layers in eighths, ADR 0189, can show
it). Stefan's growth in `phases` steps on the ice's thickness at the step's start, so a coarse step grows a thin
sheet too fast (shortcut: the closed form across a step when the warp makes it matter). A block of ice is still U
units, and its stored heat is its row's over `LIGHT` like every block's.

### Gas near the ground and fire that spreads [0191, 2026-11-23]
The developer's calls on the near-gas spike (`prototypes/near-gas/README.md`), not built yet (roadmap, the world
queue's step 8). Gas is sparse 1 m cells kept only where the air is off rest, conserving with the 128 m cells;
fire spreads by heat reaching the next block's ignition point, with the flame's sub-cell terms from the fuel's
row. **A forest can burn down**, simulated where a player is near and finished by a cheaper rule far away.
**Fire leaves solid char that stands**; a burnt build falls only once structural collapse exists. **Apes
breathe from their cell**: smoke and CO2 suffocate. **Fire and its gas run on the world's heat** when built
("Heat runs on two constants", 2026-11-24, replacing "gas near a fire runs at real rates"): fire's spread across a
surface follows the pace, about a minute a block of dry wood in still air (the spike's 6 minutes over
`HEAT_PACE` 6); a block's burn-out is decided when fire is built and must stay longer than the spread.

### Ground water: a table, flooded caves, springs [0190, 2026-11-23]
The developer's calls on the ground-water spike (`prototypes/ground-water/README.md`): the water table is the rest
state of Darcy flow, baked and counted per 8 m map cell where touched (a well or cave trades with its cell by a well
index). **The whole ground-water clock runs at the calendar's pace** (`Ground::calendar`, the real year over the sky's: it is
rain-driven and cannot be sped up in part; ADR 0200 took heat off that factor, ground water keeps it). **Caves below
the table are flooded.** **Springs start visible streams.** The bake's added time is accepted (`docs/perf-debt.md`).
New worlds only (generation 12, map version 10); every older world generates and stays dry as it was.
Built (2026-11-24), what holds:
- **The table** (`bake::ground`): steady unconfined Dupuit flow, div(K b grad h) + R = 0 under the ground, the sea,
  lakes and the bake's rivers as fixed heads, the ground as a ceiling. K and specific yield are rows of
  `data/rocks.toml` (`[water]`: the rain's recharge share, the `tight` K under which a rock is an aquifer's floor);
  the floor is the first tight layer under the ground, else the crust's bottom; tight rock at the ground has no
  table. Recharge is the share of rain less evaporation (at least a tenth of the rain). A link's conductance is the
  harmonic K times the mean thickness over the higher of its two floors (so each cell's balance has one root).
  Solved by Picard with conjugate gradients (one thread, a fixed order, deterministic) on **160 cells a face edge on
  every planet**, not the 8 m map cells: the 8 m solve on the 4 km planet ran 3 hours unsettled (its time goes as
  the edge's fourth power). Each solve cell is the mean of the map cells under it, held at its lowest drain; the
  table on the map cells is read back bilinearly and never over their own ground. The map keeps it per 8 m cell.
- **Springs**: a solve cell held at its ground seeps the recharge uphill of it; the flow goes to the lowest map
  cell under it where the table meets the ground. A spring of a litre a second or more (`ground::STREAM`) starts a
  stream down the drainage to a river, lake or the sea, laid as river reaches (water in the world). Springs
  under that stay in the map, unseen.
- **Flooded caves**: a cave whose floor lies under its column's table holds generation's water to it (a closed one
  full under it, at the pressure that puts its head at the table; one open to the sky a pond at the table). It is
  generation's water as the sea is: not counted until touched, then stored with the ledger unmoved. Drawn by its
  own flat surface quads in level-0 chunks.
- **Wells** (`Liquids::seep`, once a second on the server): a stored gap of water alone whose floor lies under its
  column's table trades with its map cell by Peaceman's index, Q = 2 pi K b / ln(0.198 dx / r_w), r_w 0.198 m,
  never past the cell's head. A dug hole under the table is stored dry and fills. A touched cell is counted in
  micro-units (`SUB` = 65,536 a unit) and draws from its four neighbours by Darcy, an untouched one at rest; back
  within 1/64 m of rest with nothing owed it is dropped. **The ledger**: `counted()` carries `ground_extra` (units
  the aquifer took from stored water) and `ground_ledger()` (touched cells' water off rest and owed, less what the
  rest gave them, `rim`, less `ground_extra`) is always 0. Saved and hashed with the server's water.
- **`technoape-server report=1`** prints `ground_water`: the table's depth by province, the wettest and driest
  quarters, the share of tight land, springs and their streams, the flooded caves, the depth at and round the spawn.
- Shortcuts: one rock's K a column (the top one's, layers unfolded); a well's cell over the ground does not seep
  (water poured into a well runs off at the rim); a touched cell trades with its four neighbours only; a layered
  gap (oil on water) takes no part.

### Snow lies in thin layers that build and melt [0189, 2026-11-23]
The developer: "have snow be smaller layers that build or melt over time", a slice of a block as in Minecraft,
"but our world interactions". Snow on the ground is a layer of a block's height in eighths, not a whole block. A player sinks into layers (feet on the ground under them, slowed by their depth) and walks on top of a full snow block. It is counted water: what falls leaves the air's vapour, a layer is its units, and
melt gives them back as water that runs off or soaks in (one ledger, ADR 0183). It builds where snow falls on
ground under freezing and melts by the heat at the place (air, sun, a fire, lava) on the world's heat (two
constants, 2026-11-24); enough
layers make a snow block, and deep old snow is where glaciers come from. Replaces the bake's fixed snow cover
in new worlds once built (roadmap, the world queue's step 7).

### Snow and ice in eighths: live columns near players, the formula elsewhere [0189 as built, 2026-11-24]
ADR 0189 built (`crates/engine/src/snow.rs`, `liquid::phases`). **Store:** a column's layer is its units of water
(U = 4096 a block) at a density of 125 kg/m^3: an eighth is 64 units (15.6 mm of water), a block of snow 512; it
lies on the column's top solid (`floor`) and shows `units / 64` eighths, 1 to 7. Eight eighths set the existing
`snow` block in the floor's cell where it is empty air no player stands in (a torch or a pair of feet keeps the layer at
seven eighths, taking no more; its 512 units still the layer's, `packed`) and the next layer lies on it; melting
down to a packed block takes it back. Only the server keeps it, and only where it differs from the formula: the
**live set** is every column within 16 (`LIVE`) of a player's feet on body 0 (33 x 33 = 1089 a player); elsewhere the
depth is the formula, the live air grid's lying snow (`Weather::snow`, mm of water a 128 m cell). Joining, a column
takes the formula's whole units (under a block) from that cell's lying snow; leaving, it gives back its layer (a
packed block stays a block, still lent). A column with water in its open gap (not a cave pool under it) holds
none, and is live again the second the water is gone. **Counted water:** fall takes `rain` (mm/h, where the column's air is under 0 C) from the same
cell's lying snow, then its soil; melt soaks into the cell's soil (not stored water: a layer melts to no puddle);
sublimation goes to `vapour`. A forced weather (`/weather snow`, for filming) lays and melts the live columns'
snow by its rain and temperature too; the grid and the water's heat stay the air's own. What a column takes or gives is booked `owed` to its air cell and settled at the
weather's next landing (`Weather::tick_owing`); settled from its lying snow, then its soil, then the water outside the
grid (the sea term: snow on a dry place, as a forced weather's, is the sea's vapour); what an air cell's columns take
between landings stays under what it could give at the last one (`Weather::has`), so a debt is always paid. The water ledger adds held less lent and the air's ledger lent
less owed, both 0 through fall, melt, sublimation, leaving and joining (`snow_tests.rs`). **Heat** (the two
constants, no pace of its own): melt is h(T_air) (T_air - 0) / L_f a second (L_f a unit as the water's), with no sun,
shade or ground term; under the triple point it sublimates as open ice does (`Phases::sublimation`, factored out of
`thaw`). By hand: an eighth at +5 C in still air goes in 64 x 1631 / (87.8 x 5) = 238 s of play (real heat is 300
times slower: 20 h); an hour of the grid's heavy snowfall (3 mm/h) lays 12.3 units, a fifth of an eighth, so the
first eighth shows after 5.2 h (falling snow on the live grid is a matter of hours to an eighth). **Ice in eighths:**
a water node under freezing air shows its freezing progress as ice eighths on its surface (the topmost a column) (`Liquids::ice_layers`,
floor(frac / 512), at most 7, its units still water in the node) and eight become the ice block as before; so 0195's
"less than a block never freezes" no longer holds: a third of a block freezes in eighths and melts back to exactly its
units. Lava's crust is not shown in layers (it needs its own store; not a few lines). **The player:** feet in a
layer sink to the ground under it and walk slower by `DRAG` = 0.5 of its depth (3 eighths: 0.81 of the pace); a
packed block is solid and stood on; a dig of the cell over a layer takes the layer, its water soaking in, and gives no
item; the look-at panel names "Snow" (or "Ice") and "N eighths deep, on <block>". **Wire:** `ToClient::Layers` (the
last variant) carries the changed columns once a second, every shown one on join; an older client never gets it from
an older server and a newer client joining an older server simply sees none. **Save:** a sixth tail after the crust,
written only when something is held, lent, owed or fallen short of a unit (`Snow::bare`), so a world without snow
writes the bytes it did and hashes as it did. A recording replays unchanged only for a world with no lying snow near a
player: a column joining there takes the formula's units, which moves `lent` and the hash (the recordings under
`tests/` replay as before). **Plants under snow** (the developer's call, 2026-11-24): snow builds over a plant as over
open ground, to a full block and on, and the plant is there again when it melts or the block is dug (the dig takes
the snow only), the same plant, kind and state. The plants in a floor's cell are clutter (`clutter.rs`, drawn only on
a column with no edit), so a block packed in a cell that held no edit remembers it (`packed`'s flag, in the snow tail)
and, when it goes, the cell's edit is taken away (`World::put_edit(b, None)`) rather than set to air, and the clients
are told so by `ToClient::Unedit` (the last variant) after the tick's edits: the plant is drawn again, hidden while a
block covers it and showing through a layer short of one. A cell with an edit (a cut plant's air) gets it back. **Drawing:** a slab of the
layer's height through the fx particle pipeline (Look::Block, the layer's block texture, z scaled), the 48 nearest,
rebuilt when a change arrives or the player moves 8 m; no remesh. Per second the server looks at the live columns
(a `here` per column: the weather at its air cell) and steps each; work is a few hundred nanoseconds a column.
Alternatives rejected: layers as blocks with a height (a block id per eighth: 8 rows a material, and the mesher's
cube faces); snow as stored fluid (`Liquids` is a gap fluid that flows; snow stays put); a live set as large as the
streamed view (the per-second work would grow with the view, not the players).
Shortcuts named in the code: the face edge cuts the live set (columns over a cube-face seam are not live); fall is judged by the column's air but taken from the cell's lying snow
then its soil; no sun, shade, wind or slope; the join is capped under a block (a deep formula drift shows as 7
eighths); partial ice is not walked on; slabs are not in the ray-traced scene; a block of
snow over a placed non-solid block or a player is not packed (seven eighths);
a dig takes a whole layer at once.
Open: the formula's own snow (the bake's fixed cover) stays as it is until the formula follows the live grid.

### Fluids are rows; lava, with no mixing [0186, 2026-11-23]
`data/fluids.toml` (`crates/engine/src/fluids.rs`, built in): a row a fluid, its place its kind (row 0 water,
rows only added at the end): `name`, `density` (kg/m3), `viscosity` (a whole number), `colour`, `light` (as a
block's, 0..4; a fluid with one glows), and `freeze`, `boil` ({ at = C, into }) and `reacts` ([{ with, gives,
heat }]) parsed but unused until liquids.md steps 4 and 5. First rows water (viscosity 1) and lava (2600 kg/m3,
viscosity 64, glowing orange). Not reloaded live (a kind is a row's place; a reload must reach the clients).
- **A node holds one kind** (`Node::kind`, 0 when empty). Two kinds never share a gap: a link into a gap holding
  another fluid passes nothing (a plain wall), and when two kinds flow into one empty gap in a step it takes the
  first link's and the others stay where they were. A pour into another fluid is refused. Where an edit joins two
  gaps of different fluids, the denser keeps the gap and displaced water joins its account (as 0071). Only water
  trades with the open sea, rivers and rain, is settled to the sea's level or forgotten as generation's; another
  fluid is always stored. `counted()` stays water's; `counted_of(kind)` is each fluid's, exact.
- **Viscosity** multiplies a link's friction a step: `v -= (v * visc >> 9).max(1)`, and the thin-sheet drag by the
  same factor; water's 1 is today's to the bit. Steady flow on a slope runs at 1/visc of water's; a lava dam break
  is overdamped (friction-led), so its front's time for a distance grows with the viscosity, linearly in that
  limit. Head, gravity (`Ground::g`) and every other rule are water's.
- **Wire and save** carry the kind in the top byte of a gap's units (`liquid::pack`: units | kind << 56). Water is
  kind 0, so a water-only world sends and saves the very bytes it did, every older save and recording loads and
  replays unchanged, and no save version or generation moves.
- **Look**: a glowing fluid's quads in the water mesh carry its light (7 bits a channel in `WQuad.info` bits 10..)
  and are drawn as the metal sea is (`metal_sea`, opaque, its own light); no new effect.
- A thick fluid stays awake only while the speed a head difference gives a link in a step outlasts that step's
  friction (`thick_moves`, under its body's pull); water keeps `moves`. Without it lava held still by friction
  never slept. On a slope lava still creeps for minutes before it sleeps (water runs off in seconds).
- **Play**: `/fluid <name>` sets what creative's bucket pours and scoops; survival's holds water. A stand run's
  `pour=t:di:dj:dl:fluid` pours through its server (docs/playing.md).
- **Fixed on the way:** the water mesh packed absolute layers while the GPU counts from the floor of the worlds
  before the mantle (`frame::gpu_r0`, ADR 0152), so every stored column (water too) was drawn `mantle` layers up,
  out of sight, since the mantle. Its quads are now in the GPU's layers.
Replaces: 0020's "a gap holds one number of water" (a gap holds one number of one fluid).
Open (step 4): layers by density in a gap instead of the wall, reactions (lava and water to rock and steam), the
look per fluid, lava's light into the block light and its heat on a player (it neither holds up nor hurts yet), a
server ledger for creative's other fluids.

### Lava lights, carries a body, and freezes to basalt [0193, 2026-11-23]
- **Context:** the world queue's step 2 (roadmap), `volcanoes.md` ("kept liquid by the heat"), ADR 0183 (matter
  changes phase) and 0187 (heat a source drives runs at real rates).
- **Light:** a fluid with a `light` lights the world as a lamp block does: `World::fluid_lamps` gives a lamp at the
  top cell of every stored glowing surface, one a 2 x 2 columns' patch at one cell's light, from the stored tiles
  round the camera (no scan of every node), gathered with the blocks' lamps (`gather_lamps`, nearest first, the
  Lamps setting's cap). A live crater's lake lights by its own lamps, worked out once when the map comes in
  (`Crater::lamps`): at most 16, a 4 x 4 grid of patches over it, each at its columns' mean surface and as bright as
  the 2 x 2 patches it stands for, so the lake is as bright in sum and no frame works out a lake; its columns once
  stored are left to those lamps, so none jumps. Water gives none. Taste default: that brightness and patch.
- **A body in a fluid:** `phys::step` reads the fluid at the feet (`World::fluid_surface`): it floats with
  `FLOAT x 1000 / density` of the body under (Archimedes, the body FLOAT of water's density as before), so lava at
  2600 kg/m3 carries it 0.31 under, mostly out; the vertical drag gains `3 (visc - 1) sub` a second (implicit) and
  the walk is slowed by `1 + (visc - 1) sub / 16` (a taste default). Only water is swum in (stroke, dive, under).
  Water's numbers are the old ones to the bit (its ratios are exactly 1): a hashed trace holds it. The burn is
  unchanged.
- **Heat and the freeze:** a fluid row may carry `heat = { pour, capacity, latent, conduct, emissivity }`; lava's is
  1150 C, 1200 J/(kg K), 400 kJ/kg, 1.5 W/(m K), 0.95 (basaltic lava as lava-flow models take it: Hawaiian eruption
  heat, its melt's heat capacity and latent heat of crystallisation 3.5 to 4.5e5 J/kg, conductivity 1 to 2 W/(m K),
  emissivity 0.9 to 0.95). A stored node of such a fluid holds a temperature (`liquid::Therm`, a sparse map beside
  the nodes) only while it is off its rest: poured at `pour`, mixed by units as it flows and is poured into, carried
  across an edit's regap (one heat per fluid a column). Its rest is `Ground::heat`: `Body::heat` at the place
  (`liquid::AMBIENT`, 15 C, without a map), and inside generation's lava its volcano's chamber heat, the formula's
  own reason it is liquid there, so generated lava never freezes and a stored patch of the lake rests as it is.
  `Liquids::cool`, once a second on the server (`tick`), only for those nodes: each loses heat through half its own
  depth (conduction in the lava) to the air over its open surface (radiation at its emissivity plus 10 W/(m2 K) of
  free convection, the surface's heat solved where the flows meet) and to air beside an open side, and to the rock
  under and beside it through the rock's own warming since it came (a semi-infinite solid, `sqrt(pi t) / e`, e 2200
  W s^0.5/(m2 K) for k 2.0, rho 2700, c 900). Past the freeze point the loss freezes units at the latent heat; each U
  frozen makes the node's floor cell its row's `freeze` block (basalt) through the ordinary edits (events, clients,
  save); a node frozen through hands its crust short of a block to a neighbour of the same fluid, else (a sheet) it
  froze into nothing. A cell that is taken (a player's feet, body or head, or a block set there) gets no block: the
  block's worth waits on the node and is tried the next second, so freezing buries nothing; each edit names what was
  there. Every unit frozen is booked to `Liquids::froze` by kind, so `counted_of(kind) + froze` stays
  what was carried in. Measured: a 3 x 3 pit of lava two blocks deep on cold ground freezes through in about 5 days
  (the hand calculation, 6 days, in the test); a thin sheet in under an hour; a crater lake never.
- **Save:** the heats (column, the node's floor, heat) and `froze` go after the version 22 layout's bytes, only when
  there are any; `Save::from_bytes` reads them if present, and a short or corrupt tail loads the save without them
  (logged). Each heat goes back on its column's node of that floor, else on a hot node there with none. Every
  stored node of a fluid with heat left without one, outside generation's lava, takes its row's pour heat, so no lava
  lies liquid for ever. So the version does not move: a world without heat (every
  water-only world, every older save) writes and reads the very bytes it did, and an older build reading a newer
  save with lava drops its heats (postcard ignores trailing bytes). Not on the wire: clients see the basalt edits and
  the water changes. Recordings replay (the step is in the tick and deterministic).
- **Not chosen:** a temperature on every node (water's nodes and bytes would change); a fixed timer to freeze; the
  remainder under a block rounded to a block (matter made from nothing).
- **Open:** a frozen block carries no heat, so the rock under a freezing pool is taken at the far rock's heat at once
  (a pool's last blocks freeze somewhat fast); one rock's thermal numbers for every block; lava meeting water (step 3);
  `froze` is not on the stats line.

### Fluids lie in layers by density; lava and water react [0194, 2026-11-23]
- **Context:** the world queue's step 3 (roadmap), `liquids.md` step 4 ("Mixing in the world"), ADR 0183 (matter
  changes phase), 0186 (fluids are rows, a wall between them) and 0193 (lava's heat).
- **A layer is a node.** A gap holds a stack of up to `liquid::LAYERS` (4) layers, each a node of one fluid with its
  own links and speeds, bottom-up in its column's list; a gap of one fluid is the one node it always was. Each node
  knows the units under and over it in its gap and how hard those over it press, in its own fluid's head
  (`restack`: units times their density over its own), so a layer's surface is its gap's floor plus what lies
  under it plus its own, and its head that surface plus that press. Every link rule is water's, applied a layer at
  a time through the layer's own depth in the window. Fluid passes along a link into its own fluid's layer, a dry
  gap, or, where the gap beyond holds none of it, through the link to that gap's bottom layer at the level it would
  lie there by density (`carry`); what arrives there becomes a layer of its own after the step (`layers_after`),
  sorted under the first lighter one. A gap without a fluid is dry ground to it (oil spreads over water only while
  thicker than `2 MIN`, as water over a floor).
- **Displacement:** a pour lands on the gap's top layer; a denser layer over a lighter one sinks through it at a
  Rayleigh-Taylor overturn's speed, sqrt(A g l) over the lighter one's viscosity (A the Atwood number, l a block):
  a block of water through two of oil in about 4.5 s. Emptied layers go, two of one fluid that come to touch join.
- **The cap:** a pour of a new fluid onto a full gap is refused (the bucket keeps it), a flow of one into it passes
  nothing, and an edit that joins gaps past it pushes the lightest fluid out to its account (water, as ADR 0071)
  or `spilt`: the ledgers stay exact. Water alone steps, sends and saves to the bit as before (a hashed trace).
- **Only water alone meets the sea:** a column with layers is never held at sea level, settled to it or forgotten;
  a stored calm body of one fluid is laid level by heads (`flatten`), not surfaces.
- **Lava and water react** (lava's `reacts` row, `Liquids::react`, once a second before `cool`): a lava node
  touching water (a layer on or under it, or a neighbour's beside its fluid) freezes a cell of itself, U units, at
  once into its freeze block (basalt; no obsidian block yet) at its floor, through the edits as `cool`'s are. That
  skin's heat as it cools from the node's heat to water's boil and freezes, `2600 kg (1200 J/(kg K) (T - 100) +
  400 kJ/kg)` and the reaction's `heat` (0), boils water off at `1000 kg (4186 (100 - T_w) + 2.257e6) J/kg`
  (water's new `heat` row with `vaporise`, read only by reactions so far): a block of skin at 1150 C over water at
  15 C boils 1.65 blocks, to a unit's rounding. Too little water for a skin makes the skin what it can quench; a film
  under a unit cools the lava node instead. Boiled water goes to `Liquids::vapour` (kind by kind; the air grid
  takes it in liquids.md step 6), frozen lava to `froze`: `counted_of + froze + vapour` is exact for every fluid,
  and the server's water ledger counts the steam. Each boil is an `Event::Boil { at, kind }` (the events' last
  kind) and the clients play `steam` there.
- **Wire and save:** a layered gap is one `(floor, ceil, units | kind << 56)` entry a layer, bottom-up, so the
  format does not change and a one-fluid world's bytes are what they were; an older build reading layers takes
  them as intervals in one gap (it keeps the units). `vapour` and `spilt` are a second tail after 0193's heat
  tail, written (with the heat tail before it) only when there are any; an older build ignores it, a bad one loads
  without it. No version moves.
- **Look:** a fluid other than water without a light is drawn opaque in its row's colour, lit by the sun and sky
  (`tinted` in the water shader, `WQuad.info` bit 31); the open gap's top layer decides a column's look. Oil's row:
  850 kg/m3, viscosity 4, dark.
- **Not chosen:** one node a gap with a stack of counts and one flow (layers would not flow by their own heads, oil
  could not spread over still water); instant sorting of a pour (no displacement to see).
- **Open:** a layer seen from the side under another; heat between layers that do not react; a skin by contact
  area (a lava node reacts with each node it touches once a second); obsidian by how fast it froze; steam near the
  ground (step 6); oil burning and sludge (later steps).
Replaces: 0186's "a node holds one kind", its wall and its refused pour, and "where an edit joins two gaps of
different fluids, the denser keeps the gap".

### Water freezes, melts and boils: one ledger over three phases [0195, 2026-11-24]
- **Context:** `liquids.md` step 5, ADR 0183 (matter changes phase), 0187 (two paces), 0193 (lava's sparse heat, its
  `froze` and `waiting`), 0194 (`react`, `vapour`); the rule and numbers of the spike `prototypes/water-phases`.
- **The rule** (`liquid::phases`, the spike's enthalpy method): stored water where its rest is liquid carries nothing.
  Once a second (`cool`) every stored water node alone in its gap is looked at; one open to the air (or under ice)
  where the air is under 0 C or under the triple point's pressure, or one on rock over its boiling point, is given
  lava's `Therm`. It is forgotten again when it holds no ice of its own, the air is over 0 C, the rock under its boiling
  point, or when nothing reaches it any more (a block other than ice set over it, another fluid laid on it). Heat flows
  a second (W/m^2, a column 1 m^2): to the air over an open surface, h (T - T_air); up through the ice over it,
  (0 - T_air) / (d / k_ice + 1 / h) (Stefan's law with the air's resistance, the ice holding no heat; a lid gives the
  water no heat back); from hot rock under it, through half its depth and the rock's cooling since it came (lava's;
  held rock gives steadily, through the walls too, since 2026-11-24, below).
  Past 0 C the loss is freezing progress (`Therm::frac`) while the water stays liquid and counted; once a whole U has
  frozen, its U units leave the node, are booked to `froze`, and the block of its row's `freeze` block, `ice`, is set
  through the edits as basalt is, in the node's top cell, so a pond grows ice from the top down; a step that freezes
  more sets every whole block, downward from there. A taken cell (a player) gets none: the block waits, booked, as
  0193's, and the freeze holds at that block until it is set. Gaining heat it melts its own progress first, then warms,
  never past the air's or the rock's heat, and boils whole units to `vapour` only where hot rock drives it over its
  boiling point (`frac` under 0 is the next unit's boil paid for), with an `Event::Boil`; what a melt has over is
  dropped. Under the triple point's pressure it has no liquid: (c dT + L_f) / (L_v + L_f) of it boils at once, every
  whole block of the rest freezes and what is short of a block boils too.
- **Stored sea is left to the formula in this step** (the review, 2026-11-24): a stored water node in a column of
  generated sea takes no part in freezing (no heat from cold air, no ice set); the sea's ice stays the bake's until
  the roadmap's "ice by physics" step gives the sheet a rule that returns to generation. Hot rock under it still
  boils it.
- **Ice blocks** (`Liquids::ice`, `thaw`): one placed, one the water froze, and one an edit was set beside (the bake's
  too) are followed. Each open face takes h (T_air - 0) from the air, a hot fluid beside it (lava) gives it through half
  a block of itself; the melt is held in the block, its area shrinking as a cube's (the spike's), until all of it went:
  the block is then air and its U units of water are poured there. Under the triple point it sublimates, its surface
  where the heat it takes pays the vapour leaving (Hertz-Knudsen in no air), its units to `vapour`, no water made.
- **Numbers** (water's row, `data/fluids.toml`, the solid named by `freeze.into`): L_f 334 kJ/kg, c 4186 J/(kg K),
  L_v 2.257 MJ/kg (the energy), conduct 0.6, ice's 2.22 W/(m K), molar mass 18.015 g/mol, triple point 611.657 Pa (CRC
  Handbook); the vapour curves through the triple point and 100 C at one atmosphere (the spike's fitted slope, within
  4 % of the real curve). h = 10 W/(m^2 K) free convection + 4 e sigma T^3 (about 5). Pressure: one atmosphere at the
  sea and the isothermal scale height R T / (M g) of Earth's air; a body without air 0 Pa. F = 8 / (1 / k_ice + 1 / h)
  = 15.4 W/m^2, derived from the bake's `t_sea_ice` (-8 C) so that its one-block sheet is the rest there (the real
  ocean gives 2 to 10; summer melt, which a year's mean cannot see, does the rest). **The pace** (ADR 0187): the real
  year (365.25 days) over the sky's (`Sky::year`; 52.5 for 334 days of 1800 s) on every flow with the air and sky and on
  F; rock and lava at real rates, and a pot a source heats loses to the air at the real rate. The heat step is the
  world clock's: under `/timewarp n` it runs n seconds a second.
- **The air:** the server's live air grid's temperature on the planet (`Weather::here`), else the baked climate with
  the season and the hour (`sky::temperature`), elsewhere the rock's (`Body::heat`).
- **One ledger:** `froze[water]` is units turned to ice blocks less those melted back, so the water ledger (server
  `water_ledger`) is `counted + vapour + froze + buckets - creative`: a change of phase never moves it. A block of ice
  dug or placed is not water until it melts (it then books -U from `froze`, the bake's too). Melted ice whose water
  has no place (its gap at its most layers) stays ice, booked, and tries again the next second.
- **Measured** (tests in `liquid/phase_tests.rs`, `world/volcano.rs`): a 1 m3 block in +10 C still air, 5 faces, melts
  to exactly 4096 units in 24,540 s (the cube's law by hand, 3 rho L / (5 h dT) over the pace: 25,549 s, 14 game
  days); a pond 4 1/3 blocks deep under -10 C air grows its first block in 188,820 s and its second in 664,320 s
  (Stefan by hand 188,865 and 664,352), liquid under it; 1/8 block on 1000 C rock boils dry in 185,710 s at one
  atmosphere (since 2026-11-24, below; 94,370 s at 700 Pa), never over 100.000 C, every unit in `vapour`; at 700 Pa it boils at 1.946 C; 100.00 C at 101,325 Pa,
  93.74 at 79,859 Pa (a 1 km peak at 19.62 m/s^2), -0.08 at 608 Pa (no liquid), -273.15 in no air; an ice block in no
  air sublimates in 6.0e4 s with no water made; 3 U in no air are 2 blocks of ice and 1 U of vapour in one step; a
  puddle of 2000 units at -30 C stays water; a block and a half ends as one block over half a block of water; a
  stored patch of polar sea under -20 C a game week takes no heat; a puddle half frozen melts at a 10,000 s step and
  boils nothing; water under an ice lid at -3 C a game year stays under 4 C; freezing waits while a player is in the
  cell, one block booked at most.
- **The bake's sheet against the formula** (not rebaked; stored sea does not freeze in this step, so it does not show):
  the rest sheet is k_ice ((0 - T) / F - 1 / h). It is one block (the bake's) only where the year's mean is -8 to
  -14.9 C. Colder, it is thicker (2.2 m at -16.6 C, 4.2 at -30); between -1.1 and -8 C the formula wants a sheet
  thinner than a block where the bake shows open sea. The first version let stored sea freeze by this rule and it
  crept outward over the cold sea, a column a game year (the review). The fix is the bake's sheet from the formula,
  its thickness by the year's mean at the cell (a bake format change), with "ice by physics".
- **Not built, and why:** evaporation from open water into dry air: at 20 C and half humidity it is about 6 mm a real
  day, 31 cm a game day at the pace, and with no rain on stored water and no vapour in the cell yet every stored pond
  would dry in days; it waits for the air grid's vapour and rain (step 6), and so does the sublimation of pond ice
  (the spike: a fifth faster freezing). Fires and furnaces as heat sources (their heat lives in the server's furnaces,
  not the ground); lava beside an ice block and hot rock are.
- **Save and wire:** water's heats ride 0193's heat tail, `froze` its tail, and a third tail names each heat's fluid
  (layers of one gap share a floor; a save without it restores its heats onto fluids other than water, as before); an
  ice block's melt so far is not saved (a reload starts it again, its units kept; every ice edit and its neighbours
  are followed again on load). No version moves; a world where water neither froze nor boiled writes the very bytes
  it did, and its water steps to the bit as before (a hashed trace). Clients see the ice through the edits.
- **Shortcuts named in the code:** a block of ice is U units (real ice 1.09 blocks); less than a block of water froze
  unseen until ADR 0189's "Snow and ice in eighths" (now it shows in eighths), and in no air what is short of a block boils at once; stored sea does not
  freeze (until ice by physics); water under a cave's roof or under another layer takes no part; the water under ice
  is one temperature; no heat between columns or layers; water's heat is not mixed as it flows; water beside an ice
  block gives it no heat; ice under freezing air keeps; Earth's air on every body; every stored water node is looked at
  a second (cost not measured, a feature round).
- **Limits** (the review, not fixed in this step): ice as an item is outside the water ledger: a frozen block dug and
  its item lost, or ice baked or given that melts, moves `froze` with no freeze to match; the ledger counts changes of
  phase, not the ice that exists. `exp`, `ln`, `cbrt` and `powf` in `pressure`, `p_sat`, `boils_at` and `thaw` are
  bit-stable on one binary, so a recording made on the Linux server and replayed on Windows may part where water
  changes phase.
- **Not chosen:** an ice layer in the node (a wire and save change for a thickness the block grid shows anyway); a
  temperature on every stored column (the spike: 30 times the time, 14 the memory, every save larger).
- **Open water boils on held rock (2026-11-24).** Water poured down an open shaft onto 300 C rock never boiled (64 C
  after 33 hours): the air 649 m down was the surface's 20 C, taken at the real rate, and the rock's heat came in as
  lava's contact cooling, 1 / sqrt(pi t), dying to nothing, from the floor alone. Both were wrong for rock the
  geotherm or a chamber holds at its heat. Now (`Solid::air_at`, `phases::drive`, `water_heat`):
  **Air in a hollow under the ground** is blended from the open air's to the rock's there (`Body::heat`) as
  1 - exp(-depth / 30 m), depth under the column's generated ground: 2 m down 6 % the rock's, 100 m 96 %. Air in a
  hole open upward trades with the sky by convection far deeper than the rock's own seasonal wave (2.9 m), so a pond
  in a pit or a quarry keeps the open air's weather (shortcut: one length for the shaft's convection, a rule of the
  hollow's width and depth once near-ground gas, ADR 0191, is built). At and over the ground it is exactly what it
  was; its pressure is the open air's at that height. **Held rock gives steadily**: only a cell of the planet's own
  ground (`Ground::held`: at or under its column's generated surface, not ice, not a plant; a placed block over the
  ground holds nothing), from the floor (tr - T) / (0.25 + d / 2k) and from each wall cell, by its share of the
  water's depth, (tw - T) / (0.25 + 0.5 / k): 0.25 m^2 K/W is half a block of rock at 2.0 W/(m K) (steady conduction
  from a field held far off into a sphere of the cell's half-width, k dT / r), in series with half the water's depth
  or half a block of it (k 0.6). Lava keeps its own decaying contact for the rock it warms. **It boils only where
  it can**: every flow is linear in the water's heat, so the held rock and the open air (h at the real rate) settle it
  at T* = sum G T / sum G; it boils where T* is over its boiling point by `REST_K` (0.5 K), at (sum G)(T* - T_boil),
  and where T* is under it and over 0 C it rests, liquid, with no heat entry (under freezing open air the rock is not
  counted, as before). By hand and measured: 4/3 block sealed in on 300 C rock boils dry in 3,910,787 s (the code
  3,910,800); open down a shaft on 300 C rock, the air at 300 C, a block boils in 2.1e5 s (58 hours), 4/3 in 2.7e5 s;
  on the 1536 test planet, whose rock passes 200 C (not 300) over its core, 4/3 block on 200.3 C rock under 199.6 C air
  dries in 813,000 s against 816,748 by hand. In a shaft on 400 C rock under 20 C open air 4/3 block boils to one in
  3,184,200 s (by hand 3,186,214) and rests at 0.843 m, where T* is 100.5 C; on 120 C rock under -5 C air T* is 31 C
  and it never takes a heat. Rock at 80 C under -20 C surface air keeps its water liquid and at rest, a heat entry
  dropped at the first step. A pond 2 m down a pit under -20 C air grows its first ice block 4 % later than at the
  surface (year's air -9 C). A basin of placed blocks over a live crater's flank, in air the chamber heats over 150 C,
  never boils; water in a pit dug into the flank's hot rock does. Rock passes 100 C (15 C air) about 610 m down under a
  shield and 230 m under an arc on the 4 km planet; stored water deeper boils off, generated water there is untouched
  (it follows the formula until stored).
  **Limits** (the review, not fixed): depth is measured under the column's own generated surface, so a hollow cut
  sideways into a cliff reads as deep; ice in a deep cellar melts where the year's mean is over 0 C; the air's pressure
  at depth is the open air's at that height (water boils at 100 C below the sea, not the 102 a real column of air
  gives 650 m down); the wake looks at a node's held rock only where its own column's rock at its floor or surface is
  over boiling (a hot wall of a neighbouring column alone does not wake it); under freezing open air the rock's heat is
  not counted; `h` is linearised at the air's heat (over 100 C air it overstates radiation by up to half).

### Lava drawn as a cracked crust, oil as a glossy slick with a film [0198, 2026-11-24]
The developer: "make liquids look more pretty". Lava (side bit 4 in `fs_water`, stored and the volcano's lake) is a
basalt crust (0.05, 0.043, 0.04), lit by the sun, the sky and 0.6 of the row's light, broken into 1.7 m plates (a
jittered grid's F2 - F1) whose cracks glow in the row's light; a slow swell of heat (value noise at 0.18 a metre)
widens the cracks from 13 cm to 50 cm (in F2 - F1 metres) so plates melt to open melt, adds a finer net at 0.4 of the plate size, and
warms the crust near its cracks; the crust drifts at (0.035, 0.02) columns a second. On the 1/16 m texels at
`WATER_STEPS`; a fall or a front runs molten,
streaked down. Oil (side bit 8, `tinted`) takes the water's ripple normal (`rippled`) at half the frequency, a third of
the pace and 0.05 tilt, a 6 % Fresnel to `sky_colour` along the reflected ray and Luna, a sharp glint (exponent 400
near), and a thin film's bands (a cosine palette over a drifting thickness and the angle) in drifting patches at 8 to
25 % of the sky, exaggerated over a real film's few per cent so a slick reads as oil from any angle. The water's own
look (ADR 0071: ripples, Fresnel, sky and traced reflection, glint, depth through the view ray, shore foam, falls,
under water) was already what this round set out to add, so it stays as it is.
Alternatives rejected: a crack pattern from a texture (the grid's noise is per texel already, no new binding); a
subsurface scatter term on the water's ripple flanks (tried: under 1/255 on the sea at noon, not worth its cost);
less absorption in green for clearer shallows (tried: the dim bed under the water showed more, so shallows darkened).
Open: lava's and oil's numbers are shader constants (`LAVA_PLATE`, `LAVA_DRIFT`, `LAVA_CRUST`, oil's in `tinted`), not
columns of `data/fluids.toml`, whose schema is the engine's; they move to the rows when a second glowing or tinted
fluid needs other numbers. Lava's flow direction does not move its crust (stored quads carry no flow).
A cool crack and the fine net are at least one 1/16 m texel across; past a footprint of 4 cm the cracks give way to
their mean share at that heat (0.13 + 0.3 heat^2 + 0.1 smoothstep(0.3, 0.7, heat), fitted to the pattern) and the
cores' mean lift (1.18), so a lake looks the same near and far; a fall's streaks sit on texels and fall a texel a step;
oil's far glint dims as the water's, and its film takes its share of what the Fresnel term leaves.
Limits: every pattern snaps once an hour, as the time uniform wraps at 3600 s (the water's ripples do too).
Limits: the patterns sit on face-grid columns, so they cut at cube-face seams (as the water's ripples).
Limits: any future glowing fluid gets lava's crust (side bit 4 is "glows").
Limits: oil in a cave reflects the open sky (no rays of its own).

## The sea and the ledger

- **The sea** is generated from the seed: every column whose ground is under the sea level holds water up to it.
  The sea level is 4 m under the planet's radius [0020] (also grid-and-planets: snapped to 1/8 block under a layer
  top, ADR 0021). The moon has no sea [0020].
- **Tide** (also bodies-and-orbits, ADR 0067 is another topic's number): the level in a column is the mean sea plus
  the tide there; the shader and the physics (`World::water_surface`) ask the same function, so generation, swimming
  and the underwater test agree [0020, amended 0067].
- A column is stored only once water near it moves. The edge of the stored region is open sea: a stored column next
  to unstored sea, within 1.5 cm of the sea's level, is held at that level and the difference goes to the counted
  reservoir `sea_extra`. A calm body touching the open sea is laid at the sea's level; columns that are as
  generation gives them are forgotten again [0020].
- **Conservation.** `counted() = stored - generated + sea_extra` changes only when water is carried in or out. The
  server's ledger adds the buckets and what creative made or removed; it must be 0 and the soak checks it every
  tick. The save keeps stored columns, `sea_extra` and the ledger (save version 2; version 1 still loads) [0020].
- Open (shortcut): the sea's level does not rise with what it takes in; revisit when the water cycle moves sea
  water [0020].

## Water on the wire and in the client

- **Tiles.** Water goes out in tiles of 16 x 16 columns (`TILE`), each sent whole. A tile is sent when it changed by
  more than a dead band (1/64 block), or settled, since this client last had it. Tiles go to a client within 128 m,
  nearest first, about 16 KB a tick [0020].
- A tile goes out as last listed: each column's water as of its last listed change (`tile_sent`), not its current
  water; otherwise a neighbour's change within the dead band leaks out and a later undo is never heard [0020].
- Settled tiles reach a client exactly; the soak compares them with every bot [0020].
- Shortcut: far floods are not sent, so they are not seen from orbit [0020].
- **Meshing** (`crates/client/src/water.rs`). The generated sea is the terrain chunks' trailing quads at the exact
  sea level (terrain LOD and streaming for free). Tiles holding stored water are left out of those quads (down to
  level 4) and drawn by the water mesh: a top per column with corners shared with wet neighbours (never below its
  floor), and sides over lower water or floors (fronts, falls). Heights move from the last state to the new one over
  one tick on the GPU [0020].
- Droplets fly off sides that drop over half a block and splash where they land [0020].
- The sea's quads are meshed once for the highest tide (`sky::TIDE_HIGH`); the shader's `vs_sea` lays each vertex at
  the mean plus the tide, and the client re-meshes its wet tiles' unstored sea columns every 4 s of world time, one
  tile a frame. The tide never remeshes the sea [0020, amended 0067].

## The water's look (`fs_water`, `shader_water.wgsl`, post.wgsl)

Traced and clear, still on the pixel grid [0071].

- **Ripples.** Three octaves of value noise over the face's grid coordinates (columns, which are metres), passed
  from `vs_sea` and `vs_water` (`WOut.grid`, `face`), evaluated at the middle of each 1/16 m texel. They run on the
  sky's clock (`Globals.water.x`, world time mod 3600 s, so a `run=view` shot is the same every time); a river's run
  along its flow (`WOut.gflow`, columns a second, from the reach's fall). The clock steps 4 times a second
  (`WATER_STEPS`). Ripples calm where a pixel covers more than 4 cm of water and are gone at 40 cm: far water is
  flat, from orbit one blue with no seam between levels [0071].
- **Clear.** A ray down the view (up to 16 m; nothing met counts as its reach) gives the way through the water; it
  takes `exp(-k len)` of the light, k = (0.45, 0.12, 0.075) a metre (red goes first). Blended over the bed already
  drawn: alpha is what of the bed does not show; the colour is the water's own light (`WATER_FOG` lit by sun and
  ambient) plus the reflection [0071].
- **Reflection.** Fresnel with water's 2 % head-on. Within 250 m, and only where the Fresnel term is over 8 %, the
  terrain is traced up to 150 m and lit without shadow rays (`seen_lit`), fading into the sky over the last 50 m and
  between 8 and 16 %; past it the sky alone (`sky_colour`, with the sun's disc). Luna is added where the reflected
  ray meets its sphere, lit by its phase. The Reflections setting off leaves the sky only [0071].
- **Glint.** The sun's specular: exponent 900 on near ripples, 60 where they have calmed [0071].
- **Foam.** A broken line where the depth at the view ray's hit (way through times the cosine) matches a level that
  laps in and out between 0.05 and 0.35 m, near water only; on every front and fall (`side` quads) in streaks
  running down; on rapids by speed. (The first try, four short rays across the surface to the nearest bank, failed
  on stepped beaches.) [0071]
- **The sea behind the horizon** is not drawn when the camera is out of the water (water writes no depth). A fall seen
  from behind is drawn as its face; the discard comes before any ray [0071].
- **From under the surface:** the sky through a window overhead (Snell's), the deep around it, fogged [0071].
- **Under water** (post.wgsl composite): with the eye in water (`World::water_surface`) everything dims to
  (0.55, 0.8, 0.9) and fades into `WATER_FOG` times the day's light at 0.09 a metre (most of it by 12 m). The old
  flat blue veil (play.rs) is gone [0071, replacing 0020's veil].
- **Setting.** Graphics, Water: off (the old sheet, `water_old`), simple (ripples, Fresnel, the sky in it, a guessed
  3 m depth; no rays of its own), traced (the default); `g.opt.x` bits 4-5 [0071].
- Shortcuts: ripples and foam use per-face grid coordinates, so a cube face's edge has a pattern mismatch (calm there
  at any distance); the ripple clock is world time mod 3600 s, so the pattern jumps once every two game days; far
  water is not hazed into the air as far ground is [0071].
- Known limits of the mechanics: falls inside a column land at once; waves in deep water run at most at the speed of
  8 m deep water; a cube edge has a seam in the water mesh; one thread for the simulation; the dead band means a
  client's moving water can be up to 1/64 block off until it settles [0020].

## Beds under water (`Body::bed`, engine `world.rs`)

Chosen where the ground is chosen (`Body::ground_biome_at`, the one place the mesher, `generated_under`, physics and
the water read a column's blocks from); nothing is baked, no map version changed [0086].

A column is under water when the water's level (`water_at`) stands over its ground. The bed then depends on:
- **Slope** of the map's cell (`Map::slope`, four-neighbour rise over run) over `BED_STEEP` = 0.7 (about 35 degrees):
  bare rock, the province's (`ROCKS.at`), under any water; a lake in the alpine biome is rock too [0086].
- **Rivers**, by the reach's fall (m a metre) and flow: falling 1 % or more, gravel; under 0.2 % or 2 m deep or more,
  clay; a small stream (under 0.05 m2/s a metre of width), under 0.5 m deep, over land whose band is dirt, dirt;
  else sand [0086].
- **Lakes:** under 0.5 m over land whose band is dirt, dirt; to 1.5 m sand; deeper clay [0086].
- **The sea:** to 8 m sand; deeper mud on a band of clay; a warm reef keeps its row (sand and coral) [0086].
- Grass, snow and ice never: no rule returns them. The band under a bed is the bed's block (clay under mud), the rock
  below as before. Banks above the waterline keep the biome's ground [0086]. (Polar ice caps lay their sheet over
  open water by their own rule, ADR 0107, owned elsewhere.)
- Blocks used: `sand`, `clay`, `mud`, `gravel`, `dirt`, the provinces' rocks. Kept bakes stay valid; existing saves
  keep their edits and every unedited bed generates by this rule on the next load. Test:
  `world::baked_tests::the_beds_under_water_are_sand_clay_gravel_stone_or_dirt` (1536 planet, every second column)
  [0086].
- The bed is chosen per column; a stream narrower than the map's cell takes the cell's slope. `Map::roughs` takes
  its slope from the same `cell_slope`, bit-identical to before [0086].
- Sand and gravel fall (ADR 0079), so a bed of them over a cave falls when an edit wakes it [0086].

## Seasonal looks

- **Data.** A row of `data/blocks.toml` may name `[block.spring]`, `[block.autumn]`, `[block.winter]`, each with
  `colour`, `colour2`, `colour3`, `amount`, `contrast` and nothing else; the row's own look is summer's and a season
  it leaves out is that too. A season's colour brings its own darker and lighter parts unless named. `stagger` is 0
  to 10 days, refused on a row without seasons (`Registry::parse`). `blocks::Def::tone(season)` gives a season's
  look [0061].
- **First rows with seasons:** grass, the tundra, cold, meadow, scrub and marsh grasses, broadleaf leaves, shrub,
  reed. Spruce, jungle and acacia leaves, the savanna and jungle grasses and the cactus have none [0061].
- **Look table.** Four rows a block in the one look table (`looks[season * 256 + block]`, 64 bytes a row, 64 KB); no
  second table [0061].
- **The season at a place** is `sky::season_at(up, t)`: the year's phase in quarter turns (the south half a year
  on) and a strength, a smooth step on the latitude's sine from 10 to 35 degrees. The shader's `season` is its
  port [0061].
- **In the shader.** Each body carries its year's phase in the free float of `Body` (negative: no seasons; only the
  planet has them). `look_in` returns a block's row untouched when the block has no seasons (bit 8 of the row's
  `size`) or the place has none; else the two nearest seasons' rows mixed by the phase, mixed with the row's own by
  the strength. Terrain fragments, effects' pieces of blocks and a ray's seen colour all take it [0061].
- **Stagger.** A column's phase shifts by up to the row's `stagger`, two thirds by its patch of 4 x 4 columns and a
  third by the column, hashed on level-0 columns. A ray's seen colour takes no stagger [0061].
- **Painted seasons.** `data/textures/<block>[_top|_side|_bottom]@spring|@autumn|@winter.png` beside the base files
  (ADR 0045's rule each); the file without one is summer's. A face a season does not paint is the base's; a summer
  or unknown season is refused by name (`textures::Set::seasons`). Each season's paint goes in its look row's `tex`,
  `tex2` and `mean`; a season's paint alone gives a block seasons (bit 8 of `size`); bit 9 says a season paints its
  own. The shader mixes in the two nearest seasons' rows that paint this face only under bit 9; the seen colour
  mixes the rows' means. `blocks::SEASONS` and `blocks::ROW` name the seasons for files and tabs [0061].
- **Hotbar, "looking at" cube and block in hand** show the season where the player stands (`gpu::season_here`:
  `sky::season_at`, none off the planet), mixed as `look_in` mixes (no stagger); painted blocks by their rows'
  means. The cubes show summer's paint only (ADR 0098 on the cubes) [0061].
- **The Studio** (also studio-and-art): the Textures page has four season tabs (summer is the base files; Add season
  copies the base's files unsaved and writes a `[block.<season>]` with the base colour, Remove deletes both), a Year
  slider (day 0 to 53, each tab sets the day nearest its start) and North, Equator, South chips. The game view takes
  `season north|equator|south`: the seasons whole everywhere, as at that latitude (`Body.year` 8 and on is the phase
  at full strength), or none. `check=1` takes the season files, `shot=` takes `season=`. `data/palette.toml` has the
  ramps `autumn` and `dry grass` [0061].
- Check tool: `python -I tools/year_sheet.py out/x.png <run=stand's arguments>` makes the 12 frames of a year
  (`textures=dir` for scratch paint) [0061].
- **Cost:** about 0.03 ms (2 %) of GPU on `stand` and `orbit`; 48 KB more in the look table; nothing on the frame
  thread or workers [0061].
- Ground snow and bare trees were left outside 0061; lying snow is the weather's (below) [0061, 0076].

## Weather on the wire and in the world

- **Wire** (`engine::weather`): `Cell = [u8; 6]` per column: cover, the root of rain over 40 mm/h, wind east and
  north in quarter m/s, lying snow over 10 mm of water, temperature + 128. `ToClient::Sky { t, all, changed }`: 13,824
  columns on Terra whole on join (83 kB), then on each 10 s landing only those that moved more than a byte's step.
  The lying snow comes from the grid's `Air::snow` at each landing, not from the saved `View` (saves keep their
  layout) [0076]. The client never runs its own air grid [0076].
- **`/weather clear|cloudy|rain|storm|snow|auto`** (and `weather=` on any run, which also takes a number as a forced
  cover) forces one weather over the planet in what is sent only: the grid, the rivers' runoff, the ledgers and the
  soak's hash are untouched. The fixed views run under `weather=clear` (ADR 0073) [0076, 0108].
- **Cover.** The grid's `View::cover` takes `Air::cloud_share` (the cloud over what its layers hold at `cloud_max`)
  through a smooth step from 0.2 to 1.2 (it was `per_m2 / (per_m2 + cover_half)`, at most 0.01 on Terra). F3's
  cloud reads the same. The sun pass's own shade still uses `cover_half`, so clouds barely shade the bake's or the
  live grid's ground; changing it changes every bake [0076].
- **Rain is not water in the world;** that stays the grid's runoff to the rivers (ADR 0059) [0076].
- **Lying snow.** The terrain shader whitens faces that look up by the column's snow (bilinear over the columns),
  the face's pattern showing through where thin; fully white from 10 mm of water. Nothing in the world changes
  [0076].
- **Wind.** The effects pool's drag pulls a planet particle toward the wind where the camera is (leaves, smoke,
  snow); rain starts with the wind's velocity; the wind's sound rises with its speed. Fluffy crowns and plants of
  planes sway from the same wind at the camera (ADRs 0090, 0091; in a shot `g.water.w` is 0) [0076].
- **Settings:** Graphics "Clouds" (off, low, high; low by default) and "Rain and snow" (off, light, full) [0076].

## Clouds (`clouds.wgsl`, post.wgsl)

- **Slab.** 250 to 314 m over the sea on every planet (`CL_BASE` 250, `CL_THICK` 64; also in `weather.rs`, a test
  holds the two to each other), cells of an eighth of an air column (16 m on Terra, `CL_SUB`). The highest summits
  (350 m) stand above the cloud sea [0105, replacing 0076's 300 m and 0098's 100 m].
- A cell is filled where the column's cover (bilinear, sampled at the cell's middle) beats its own clumped number
  (0.7 x clump + 0.3 x hash + `CL_OPEN`); its top rises with the margin, its edge soft where the margin is thin.
  `CL_OPEN` = 0.06: a cell under it cannot fill [0076, 0098, 0104].
- **Cover mapping.** `cl_cover` maps the air grid's share c to the cover the cells fill to, `c - 1.4 max(c - 0.3, 0)
  (1 - c)` (0.3 gives 0.3, 0.6 gives 0.43, 0.95 gives 0.90), in the clouds, their shadow field and the sky's grey
  (`weather::cl_cover`, a test holds it to the shader's line) [0108].
- **Edges and light.** A cell's edge fades over a margin of 0.15; the light falloff `CL_K` is 0.09. The shadow is
  `1 - 0.8 smoothstep(field)`: a thin edge lets most of the sun through, a thick body takes 80 % [0108].
- **Motion.** The cells slide with the column's wind in two phases half of a 240 s period apart; the client blends the
  last two landings over the 10 s step so cover moves instead of jumping. A step landing mid-blend goes on from what
  showed. A joining client sees the current step at once [0076].
- **March.** Marched after the water (premultiplied) in up to 12 (low) or 24 (high) steps a pixel, 4 m steps near the
  camera (still capped by the setting), start by interleaved gradient noise, ends at the sea's sphere (no cloud
  under the sea level, no cloud through the planet). Lit by Sol by the sun's height where the cell is (the night
  side's are dark from orbit), grey to slate with held rain, fading into the air as far ground does [0076, 0098].
- **Shadow.** One sample where the sun ray crosses the slab's middle, in `light()` (so ground, bounce hits and
  particles get it). The soft shadow `cl_soft` reads the same field as the cells but smooth; the clouds themselves
  stay blocky. Clouds shadow only the sun's light, not the moon's; they are not in the traced scene, so reflections
  do not show them [0076].
- Shortcut: the cells' pattern and the cover step at the cube's twelve edges (each face's texture is sampled alone)
  [0076].
- **Cost accepted:** GPU median frame +0.8 ms with clouds low, +1.5 ms high (about 0.35 of it the shadow in
  `light()`); the frame thread gains a 166 kB texture blend at about 6 Hz and the drops' spawning [0076].

## Rain, snow, storms and sound

- **Rain and snow** (`client::weather::Fall`): drops of the effects pool started over the camera, upwind by their
  fall, landing on the floor of the column they are over (re-done as they cross columns), so a roof or a cave's
  ceiling stops them; a small splash where they land, a ripple on water. Snow below 0 C at the camera: slow flakes
  with drag toward the wind. Spawning stops at two thirds of the pool's cap [0076]. Rain density 350 / 1,100 drops a
  second at 4 mm/h for light and full [0076].
- **Only under the clouds.** `weather::from_cloud(height over the sea)` is 1 under the slab's base falling to 0 at its
  top; a drop's spawn point draws against it (no draw under the base) and the rain's sound at the head is scaled by
  it (weighed where drops start, `RAIN_UP` + 2.5 m) [0108].
- **The sky** greys with cover and darkens with rain [0076].
- **Sound:** `rain` through the open sky, `rain.in` (low-passed) for the share of nearby drops that land above the
  head (a roof) [0076].
- **Storms:** rain over 8 mm/h. Each stormy column within 6 of the camera's strikes in a seeded share of 2 s slots
  (`strikes`: seed, column, slot), so every client sees the same strikes from the bytes alone; under a storm over all
  13 x 13 columns a strike comes every 40 s, every 20 s in rain twice as heavy [0076]. A strike within 2.5 km flashes
  (two flickers and a fade that lights every lit pixel by its albedo, weaker far off); its `thunder` plays from the
  strike, late by distance at 343 m/s, in the head (no position; gain by distance, the sound rows' reach caps at
  200 m). A later strike's flash keeps the latest; thunder's queue clears on a clock jump. The lightning's height is
  the slab's base (`CL_BASE`) [0076, 0105].

## Light snapped to texels [0108]

- **Where.** `terrain_lit` (every block face, cutout blocks and the crowns' fluff cards) takes its direct light at
  `lit_at`, the middle of its painting's texel (16 a cell edge; a coarse LOD level's as large as it draws them).
  Fine 32 px paintings (ADR 0090) are lit 2 x 2 painting texels a light texel. The functions sit between
  `// ---- the light's texels` markers in shader.wgsl (`tx_grid_of`, `tx_point`, `tx_snap`, `tx_seed`, `tx_k`).
- **On the cube-sphere grid itself:** a body-frame point to (face, column i, column j, layer), the two in-face
  coordinates to their texel's middle, back to the body frame, the face's own axis untouched. The fragment uses
  `local` (the painting's own point, so a swaying crown's light rides with its texels); `gi_trace` uses the
  G-buffer's point and normal.
- **Past a texel a pixel.** `tx_k`: where one pixel spans more than a texel (about 54 m at 1440p, 40 m at 1080p
  facing a face), the light's texels double until one spans a pixel again, on the same grid of whole texels. The step
  stops at a block (or a coarse level's painted texel); past that it returns 0, the pixel's own point.
- **Sun and lamps without random numbers.** `sun_seen` and `lamp_seen`: 2 rays, 4 in a penumbra (a penumbra is flat
  steps of a quarter); the sharp-shadow setting keeps one ray to the sun's middle; the moon is one ray to its
  middle. The sun's pairs lie on the disc's diagonals in a frame from the local up. A repeated lamp end takes the
  earlier ray's answer; the glass tint is the mean of the rays that got out. Up to 2 to 4 rays a lamp for the three
  strongest. At a bounce's hit (`seen`): one ray to the sun and one a lamp (`one_ray`).
- **Bounce.** `gi_trace` leaves from the texel's middle, its direction from the texel's seed and the frame, so every
  pixel of a texel shares one sample a frame; it snaps only where the normal is within about 8 degrees of a grid axis
  (no slope, plant, model or effect snaps), else keeps the pixel's point and seed.
- **No texel grid** (plants' stills, models, particles, water, glass panes' own pixels): their own pixel's point, but
  the same noiseless `sun_seen` / `lamp_seen`. Sloped tops keep the pixel's point. `fs_still` passes a footprint of
  -1 to `terrain_lit` ("light where you are"); a test holds it.
- **`direct_smooth`** (post.wgsl, ADR 0098's 3 x 3 smoothing of the direct light, albedo put back) is kept; it now only
  softens a texel step's edge by a pixel.
- **Cost accepted** (not optimised, ADR 0096): about +0.5 ms GPU at 1080p, +0.7 to +0.9 ms at 1440p, mostly
  `gbuffer` near the ground. Ideas: trace the second sun ray only where neighbouring texels disagree; skip penumbra
  rays past `tx_k`'s first doubling; cap the cloud march at a lower transmittance where the slab is thin.
- Known: a face seen exactly edge-on may pick the wrong axis for the bounce (smoothed anyway); `seen()` still takes a
  `px` it no longer uses past the bounce direction.

## The globe map (`crates/client/src/globe.rs`) [0077]

- **Opening.** M (rebindable, "Globe map", Keys page) opens `Menu::Globe`: an orthographic globe, north up; a drag
  turns it, the wheel zooms 1 to 64 times. A click drops a waypoint on the ground there, a click on a waypoint opens
  its page, a right click removes it. Buttons: Terra, Luna, Back. Latitude and longitude under the pointer use
  `nav::geo_text`. Holding M does not flicker it (key repeat does not reach the menu's close).
- **Picture.** The bake's map at mip 0, resampled once on a worker into latitude-longitude RGBA8 (Terra 1024 x 512,
  Luna 512 x 256): biome colours, sea by depth, lakes, every river cell, hillshade from the NW (ice cells in the ice
  colour over land, lake and sea alike, ADR 0107). About 82 to 109 ms, off the frame thread.
- **GPU.** One storage buffer (binding 20, 2.70 MB VRAM) holds a 128-byte view, both pictures and both seen-masks. The
  UI shader draws the globe in one rectangle (glyph 200): 3 x 3 pixel blocks, night side where Sol really is, the
  unseen bare, a line every 30 degrees. CPU per open frame: the view write; masks only when they change.
- **Seen-mask.** A bit per texel, marked out to the horizon from the eye's height over the ground (row spans by the
  law of cosines; twice a second at most, after a move), kept as `<save>.seen` (80 KB), written to `.tmp` and renamed
  every 60 s and on exit. Creative shows everything; survival shows what was seen. It is client-side beside the
  save, like waypoints; a client joined with `connect=` has no save, so it lasts a session there (shortcut until
  per-server files). Luna is told by its map's version (Luna's count from 101, `bake::luna`).
- **Waypoints in survival** are on the globe and in the same places as N and the compass strip, in both modes; where
  you are in numbers stays creative's (the compass strip names only the body, the chat's line for a new waypoint has
  no latitude and longitude, the globe shows none under the pointer); survival can drop a waypoint only on land it
  has charted. Defaults, open to change.
- **Markers** are ordinary UI rectangles: you (ring and facing tail, filtered by id, not position), other players
  (white, named, shown everywhere), waypoints (icon, colour).
- **Server.** The player list goes out once a second as well as on joins and leaves, so positions are current (about
  150 KB/s at 50 bots; kept) (also multiplayer-saves-and-data).
- Edits do not show (a texel is 24 m); a "what you built" layer is for later.

## No longer holds

- 0187, "Weather-driven heat runs at the calendar's pace" (2026-11-23), both its paces: heat exchanged with the air
  and sky faster by the real year over the game's (about 52.5, derived from the sky's rows), so a winter grew a real
  winter's ice; heat a source drives (a fire, a furnace, lava, the rock's own heat) at real rates, a pot's loss to the
  air taken at the real rate while a source heated it. With it 0191's "gas near a fire runs at real rates", 0193's
  "freezes to basalt at real rates", 0195's "rock and lava at real rates", and a single 52.5 pace on every flow that
  lived a few hours on 2026-11-24: replaced by the draft "Heat runs on two constants" (2026-11-24). The calendar's
  factor still sets the ground water's clock (0190), which is matter, not heat.
- 0186's "a node holds one kind": its wall between two fluids, its refused pour into another fluid, and the denser
  keeping a gap an edit joins; replaced by the draft "Fluids lie in layers by density; lava and water react".
- 0020's blue veil over the screen under water: replaced by 0071's composite dimming and fog.
- 0020's "the sea is one level": the level is the mean plus the tide, 0067.
- 0020's water mesh drawn without tide re-meshing: amended by 0067 (every 4 s of world time for wet tiles).
- 0071's first foam by four rays across the surface: dropped before the build for depth at the view ray's hit.
- 0076's cloud slab of 300 to 364 m, and its 8 m march step: replaced by 0098 (100 m, 4 m near the camera), then the
  base back at 250 m by 0105 (steps stay 4 m near the camera).
- 0098's cloud base of 100 m, `CL_K` 0.14: replaced by 0105 (250 m) and 0108 (`CL_K` 0.09).
- 0076's hard-edged cloud shadow of the 16 m cell: replaced in 0076 itself by the soft `cl_soft` shadow (0073's note
  that the shadows are hard by design is out of date).
- 0076's rain that spawns wherever the column's air says it rains, even above the slab: replaced by 0108's `from_cloud`.
- The sun's shadow by a dithered point of its disc (`in_disc`), the lamps' `lamp_target` random point and the
  bounce's per-pixel seed (0022, 0072, 0098's interleaved-gradient disc point): replaced by 0108's noiseless rays
  and texel light.
- 0077's status "not merged": it is merged (the globe is in `crates/client/src/globe.rs` and used by 0107).
