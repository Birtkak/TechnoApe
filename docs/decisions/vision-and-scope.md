# Vision and scope

What the game is, the engine under it, the rules of its planets, the ages, the air, and how the story and docs are
kept. What holds today. Numbers in brackets are the ADRs (`archive/NNNN-*.md`) a decision came from; INDEX.md lists
every number. Ground truth: `docs/design/vision.md`, `fundamentals.md`, `progression.md`, `lore.md`.

## The developer's calls

- **Planets first.** The planet and space are the identity of the game; small spherical worlds you walk around, dig
  into and fly between. Chosen because small voxels, a long view without LOD and fast travel cannot all be had on a
  flat world [0001].
- **Seamless travel** from the ground to orbit to other bodies with no loading cut: a far representation of each
  planet, a floating origin or double precision, radial gravity [0001].
- **Far terrain is drawn coarser (LOD), visibly but tidily:** bigger blocks with clean seams, no cross-fade. The look
  is part of the style [0001].
- **The planet's size stays a parameter.** The first playable's 1 km is not an upper bound; the engine keeps the radius
  a parameter and benchmarks the 4 km planet as headroom; it grows if a playtest finds it limiting [0001, 0008]. (Sizes
  in force: see grid-and-planets.md.)
- **Three planet rules are pillars:** sun and time zones, a finite planet, real orbits. Line of sight over the horizon
  is not one [0008].
- **Survival is harsh:** food, water, temperature and air, a main activity next to automation. Every need starts as a
  chore and ends as a factory [0008].
- **Megaprojects pull the ages:** each tech age ends in a planet-scale build that changes the world; they are
  free-form, designed by the player and judged by the planet by a rule, not by a shape [0008, 0009].
- **The player is an ape** with bare hands that climbs to orbit. The start has no tools, no suit, no map [0009].
- **Visit early, live late:** orbit and the moon are first reached about halfway; surviving there and hauling in bulk
  is the second half. Space content cannot be left to the end of development [0009].
- **Electricity from LV on is one base system in GregTech-style tiers** (LV, MV, HV, ...), quite realistic; rotation
  powers everything from steam until LV [0009].
- **A whole planet can be mined out and drained** late in the game; it must be physically possible [0009].
- **The game has a water cycle, weather, pollution and radiation**, planned work [0010].
- **There is no stone age.** The Steam Age is the first age; the alloy stays bronze; iron stays inside the Steam Age
  (the dynamo needs its magnets); three furnaces [0137].
- **The 8 cube corners are ordinary ground**: nothing is placed there and no sites are kept clear [0136, which
  supersedes the corners call of 0027 and the corner-landmark line of 0003].
- **The story's ending is left open on purpose** (decide when the game shows what it has become) [0136].
- **Features first, speed in batches** (also process-gate-and-performance.md) [0096].
- **As close to real astrophysics as possible; what is not real is listed and judged** [0177, 2026-11-23; the
  liquids' part is in sky-weather-and-water.md]. **A stronger constant of gravity** [0180, 2026-11-23]: this universe's G is the one at which Terra's own mass
  (her row's densities and radius) pulls 19.62 m/s^2 at her surface, about 3,550 times the real one (2.4e-7; the
  exact value is computed from her row when built). No body's pull is set by hand after that: every pull, orbit,
  month, year and tide follows from masses, distances and that one G. After `prototypes/tide-force` (derived tides on
  Terra are millimetres; Luna orbits 1.9 times too fast for Terra's mass; the sun has no mass) the developer
  dropped "playing feels the same" and the eclipse-fitted sizes and distances (the eclipse call of 0027/0037 no
  longer binds): everything is made real under the one G, free to change anything, each choice one row value, and
  the developer picks what to keep or revert from `docs/design/real-gravity-changes.md`. Replaces: Terra's and
  Luna's `weight_g`, `TIDE_LUNA`/`TIDE_SUN`, the hand-set periods. **Kept as they are:** the water's 16 m cap on a deep wave's speed; a tide with no lag or resonance in the open sea; the air as a
  coarse grid, with gravity and tides added to it. **To make real:** every other body's pull derived the same
  way (Luna's is set by hand to Terra's today); gravity that falls off with height and bodies moved by it, before
  the climb to orbit; water by its body's gravity and tides as a force (`docs/design/liquids.md` steps 1, 2); a
  finite sea; plants placed clear of the tidal band. **Open until `prototypes/tide-force` has numbers:** the
  eclipse-fitted sizes and distances against derived tides; whether days, months and years follow from the
  masses. **Open, no lean:** the gas giant as a drawn disc with no air to enter.

## Engine and code

- **The game is built on our own engine in Rust with wgpu** (the Pillars of Simulation Engine). Chosen after two
  spikes on one benchmark: a ready-made engine with an own renderer failed the flight gate and spent 5x the CPU on edits;
  Rust + wgpu held the budget [0002].
- **Everything is new.** No code, data or content of the old flat-world game is carried over; it is in git history
  only [0001, 0002].
- **Gameplay is Rust, content is data.** No script language. Items, recipes, machines, materials and biomes are data
  files with a schema and tests [0002].
- **Multiplayer from day one.** Singleplayer runs a local server; every system is server-authoritative from its first
  version and sends deltas [0002].
- **No frame stalls by design.** Streaming, meshing, simulation and saving run on worker threads; the frame thread
  only swaps finished results in under a per-frame budget. Every system ships with its benchmark [0002].
- **No simulation bubble.** The whole loaded area keeps simulating, full rate near the player, lower rates farther
  out [0002].
- **Saves hold only what changed:** untouched terrain is rebuilt from the seed; saves keep edits, entities and
  simulation state. A generator change therefore alters untouched ground in old worlds [0002]. (Save formats:
  multiplayer-saves-and-data.md.)
- **Libraries are picked per system when that system is built, each in its own ADR** (UI, audio, physics,
  networking) [0002].
- **Bots are clients.** A bot is a windowless client on the same protocol as a player; tests drive bots by script or
  live and get JSON state and filmstrips back; many bots load-test the server. AI players that play on their own
  (goals, pathfinding, utility AI on top of the same client) come after the core loop [0002].
- **Checks:** `cargo test`, plus a headless run printing JSON stats and a filmstrip in under a minute. Long benchmark
  series only for performance decisions [0002]. (Gate: process-gate-and-performance.md.)
- **No editor and no hot reload** was the first stance [0002]; data files for effects, blocks, items, creatures and
  herds now reload live (see studio-and-art.md, multiplayer-saves-and-data.md).
- **The first playable** was one planet and its moon, radial gravity, walk, dig and place, water as fill levels,
  sound, saving, test bots, no machines [0002]. Built.

## Look, audio and day

- **One pixel grid for every block, item and model:** one texel size everywhere, settled by the block look spike
  (`prototypes/block-look/`); textures are pixel art, not flat colours; every model is built on that grid [0002].
  The grid is 16 texels to a 1 m block since the block became 1 m (see grid-and-planets.md) [0002, amended by 0033];
  flora and leaves are painted finer (see studio-and-art.md).
- **Lighting:** block light from torches and lamps, the sun from its real direction around the planet, corner
  occlusion, dark caves, real-time shadows for the sun and moving lights [0002]. (Detail: rendering-and-lighting.md.)
- **The sun really circles the planet**, so the night side is dark [0002]. The day length is set in sky code
  (`sky::DAY`, 1800 s) (see sky-weather-and-water.md).
- **Movement:** snappy first-person walking and jumping in radial gravity; a jetpack or ship unlocks the climb to
  orbit [0002].
- **UI:** pixel-art panels and slot grids on the same pixel grid as the world, keyboard and mouse, drag and drop,
  shift-click, search [0002].
- **Content is designed fresh for planets.** The first planet is Earth-like with varied biomes (oceans, forests,
  deserts, mountains, caves, ores); the moon is barren with other ores [0002].
- **Rich audio early:** 3D sound with cave reverb, material-based footsteps and digging, wind and water from the first
  playable [0002].
- **Liquids are fill levels plus fine splashes** [0002, shaped by 0020; see sky-weather-and-water.md].

## Planet rules

- Gravity falls off with distance and the moon's rail is a real orbit [0008].
- Water and rock are counted, not fixed: nothing appears or vanishes by itself, but the player can remove all of it. A
  save must be able to store "this volume is gone", and nothing may assume the core is unreachable [0009, 0010].
  (Deep reach: deep-world.md.)
- A finite planet means deposits are counted, far LOD must show edits, and the late game needs recycling [0008].
- An edit on the wire is a shape, not a block; the server validates the whole shape [0008].
- Air and heat need enclosed rooms as a simulated concept [0008].

## Ages and progression

- **Power ladder:** the Steam Age starts the game; rotation (shafts, cogs) powers everything until LV, reached after
  7-10 hours; electricity then in tiers. Metals: copper in the first half hour, bronze through the Steam Age, iron at
  its end, steel as the gate to MV. About 110-150 hours to the moon base [0009, 0137].
- **The Steam Age:** flint tools, fire, copper (about half an hour), bronze, crank, boiler and engine, cogs, shaft
  machines, farm, iron. It **ends with the first steam generator** (a steam engine turning a dynamo). The Steam Age's
  megaproject is **POWER!** (the Works, renamed). Home is no megaproject [0137].
- **Steam and rain:** released steam returns as rain, so ponds refill naturally [0009].
- **Realistic electricity:** voltage, current and cable loss are simulated, so tiers have a physical reason
  (distance); generators are shafts turning dynamos [0009].
- **After the moon base an exotic resource** (warp fuel, a quantum liquid) feeds exotic space technology; power stays
  a main concern to the end [0009]. Its form waits on `lore.md` (a fuel ladder of chemical, fusion, antimatter) [0136].
- **Three furnaces**, each one placed block with a plain window (in, fuel, out, heat): the crucible to 1100 C, the
  bloomery to 1400 C, the blast furnace to 1600 C. A multiblock blast furnace later reaches 2200 C (hot blast), for
  speed [0137]. The block with a window and an inventory is not built yet [0137].
- **No world-first rule:** "every process starts in the world and ends in a machine" is dropped; copper is melted in a
  furnace block from the start. World routes beside machines in `crafting.md` are optional [0137].
- The pit kiln, digging stick and hammer stone are no steps of their own; charcoal comes with bronze [0137].
- A megaproject's rule is a measurement the simulation already makes (power, temperature, an orbit, tonnage) with a
  threshold and a duration, as data; bots can test it [0009].
- Three network rule sets on one engine: pipes (water, steam, later fuels and the exotic liquid), shafts, wires; each
  gets its own ADR and benchmark when built [0009].

## The planet's air

- **One coarse air grid** around the whole planet carries heat, water, pollution and wind; weather is what the cells
  show; radiation uses it for fallout and for the shielding the air gives. Always running for the whole planet inside
  the deterministic tick. Not four grids, not scripted effects [0010]. (Built: sky-weather-and-water.md.)
- The temperature field that survival needs is part of this grid [0010].
- Water is counted from its first version; each substance gets a conservation law in the soak test (none appears or
  vanishes by itself; the player may take water out) [0010].
- Order of building follows the tiers: heat and water with the survival needs, weather with LV, pollution with MV,
  radiation with EV [0010].

## Story, lore and docs

- The story is a living document in `docs/design/lore.md` (the Indigenti, a swarm that binds the first animal to touch
  its monument; a fuel ladder; three systems). `progression.md` is not rewritten until the story is settled [0136].
- Where the swarm's monument and other work stand is open (listed in `lore.md` and `open-questions.md`) [0136].
- Historical ADRs that speak of 0.5 m blocks or the old game record what was decided then; ADR 0033 and the fresh start
  supersede them [0136].
- Anything built from here may not assume structures at the corners. Corners stay a place where the grid folds and a
  test target for the soak and the fixed views [0136].
- The hostile creatures and the exotic resource wait on `lore.md` [0136].

## Open

- How hard pollution and radiation punish, and whether damage can be permanent (proposal: no) [0010].
- The planet's size beyond the standard sizes, if a playtest finds it limiting [0008].
- The block with a window and an inventory for furnaces, and a pass over `crafting.md` [0137].

## No longer holds

- 0001's 0.5 m world block, 16 px per 0.5 m, and its 0.3-1 km planets as the target: replaced by 0033 (1 m blocks)
  and 0021 (planet sizes).
- 0001's "fine 12.5 cm cells (4 per block edge)": eight per block edge since 0033.
- 0002's 0.5 m block fill levels and "a day is about 20 minutes": the day is 1800 s since 0041.
- 0002's "no hot reload": data files reload live (0134).
- 0008's 1 m brush (2 x 2 x 2) as the default dig: replaced by 0017 (bare hands dig and place single blocks; the
  brush stays in the engine for later tools; see mining-and-digging.md).
- 0008's "planet size open at 1 km": see 0021.
- 0009's stone age grouped with the steam age as a starter of about an hour, and Home as a megaproject: replaced by
  0137.
- 0009's power-decision history (a new kind of power every age): replaced in the same ADR by tiered electricity.
- 0010's "design in progression.md is a proposal" status: the systems stand; the design doc follows the story.
- 0136's roadmap and status-line edits: history only.
