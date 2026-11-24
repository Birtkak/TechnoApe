# Index of decisions

Every number 0001-0167 once. A code comment's "ADR NNNN" is resolved here: the topic document (and
heading) that carries what holds today, and the state. The numbered files are the record, in
`archive/`. New entries take the next number (see CLAUDE.md, principle 2) and get their line here.

States: `in force`; `amended by NNNN` (the topic document states the current form); `replaced by NNNN`;
`history only` (a measurement or a round, no standing decision).

| No. | Title | Where it stands | State |
|---|---|---|---|
| 0001 | Planets first: small spherical worlds of 0.5 m blocks | [vision-and-scope.md](vision-and-scope.md) - The developer's calls | amended by 0033 |
| 0002 | Our own engine in Rust + wgpu, and what it does differently | [vision-and-scope.md](vision-and-scope.md) - Engine and code | amended by 0033 |
| 0003 | The planet grid is an equi-angular cube-sphere; its seams are part of the world | [grid-and-planets.md](grid-and-planets.md) - The grid | amended by 0131 |
| 0004 | The dev PC is the reference machine; the frame budget and how it is measured | [process-gate-and-performance.md](process-gate-and-performance.md) - Reference machine and budget | in force |
| 0005 | Windows client, Linux server on the home server, automatic updates | [multiplayer-saves-and-data.md](multiplayer-saves-and-data.md) - Platforms, hosting and the home server | amended by 0130 |
| 0006 | Client and server split: one server crate, channels as the wire | [multiplayer-saves-and-data.md](multiplayer-saves-and-data.md) - Client and server split | in force |
| 0007 | Player movement: shared fixed-step physics, server-authoritative | [player-and-feel.md](player-and-feel.md) - Movement | amended by 0132 |
| 0008 | Game feel and planet rules: brush tools, harsh survival, megaprojects | [vision-and-scope.md](vision-and-scope.md) - Planet rules | amended by 0017 |
| 0009 | Progression spine: an ape, a steam starter, electricity in tiers, free-form megaprojects | [vision-and-scope.md](vision-and-scope.md) - Ages and progression | amended by 0137 |
| 0010 | The planet's air: water cycle, weather, pollution and radiation | [vision-and-scope.md](vision-and-scope.md) - The planet's air | in force |
| 0011 | Test harness I: soak, state hash, flight recorder, one gate | [process-gate-and-performance.md](process-gate-and-performance.md) - Test harness and the gate | in force |
| 0012 | Cheap speed levers: a column cache, Fx for edits, mimalloc, low-priority workers | [process-gate-and-performance.md](process-gate-and-performance.md) - Speed rules that hold | in force |
| 0013 | The block registry: one TOML data file, built into the binary | [multiplayer-saves-and-data.md](multiplayer-saves-and-data.md) - Block registry and ids | amended by 0062 |
| 0014 | Edits as boxes, whole or nothing; survival counts blocks on the server | [mining-and-digging.md](mining-and-digging.md) - Edit shapes | amended by 0074 |
| 0015 | Edit prediction: show at once, put back what the server did not confirm | [multiplayer-saves-and-data.md](multiplayer-saves-and-data.md) - Edit prediction | in force |
| 0016 | Saves: only what differs from the seed, written whole, renamed into place | [multiplayer-saves-and-data.md](multiplayer-saves-and-data.md) - Saves | in force |
| 0017 | Mining and building like Lay of the Land: shape tools, world physics, blueprints | [player-and-feel.md](player-and-feel.md) - Building like Lay of the Land | amended by 0024 |
| 0018 | Test harness II: viewpoints by number, a crack detector, the feel report | [process-gate-and-performance.md](process-gate-and-performance.md) - Test harness and the gate | in force |
| 0019 | Input steps and reconciliation | [multiplayer-saves-and-data.md](multiplayer-saves-and-data.md) - Input steps and reconciliation | in force |
| 0020 | Water: shallow water on column gaps, a counted sea, tiles to the clients | [sky-weather-and-water.md](sky-weather-and-water.md) - The water's simulation (`crates/engine/src/liquid.rs`) | amended by 0067 |
| 0021 | A continental planet; 4 km standard, 8 km huge | [grid-and-planets.md](grid-and-planets.md) - Planets and their sizes | amended by 0033 |
| 0022 | Ray-traced light, and a sky that moves: spin, orbits, eclipses | [rendering-and-lighting.md](rendering-and-lighting.md) - The ray-traced light | amended by 0041 |
| 0023 | Chat, commands, the F3 overlay and bug reports | [player-and-feel.md](player-and-feel.md) - Chat, commands and the debug UI | amended by 0073 |
| 0024 | Gravity from every body, and the player's own orientation | [grid-and-planets.md](grid-and-planets.md) - The player's own orientation | amended by 0132 |
| 0025 | The optimisation pass before world generation | [process-gate-and-performance.md](process-gate-and-performance.md) - The speed guard | amended by 0096 |
| 0026 | Terra and Luna, simulated once when the world is made | [grid-and-planets.md](grid-and-planets.md) - Bodies are simulated once (the direction) | in force |
| 0027 | Luna at 1 km, learning by doing, hostile creatures, made things at the corners | [grid-and-planets.md](grid-and-planets.md) - The developer's calls | amended by 0041 |
| 0028 | Settings as rows of data, key bindings, a frame cap | [player-and-feel.md](player-and-feel.md) - Settings pages and key bindings | in force |
| 0029 | (number never used; no file) | - | - |
| 0030 | Latitude and longitude, a compass, waypoints; one look for the HUD | [player-and-feel.md](player-and-feel.md) - Navigation and the HUD's look | in force |
| 0031 | One effects system: particles and motion | [effects-and-sound.md](effects-and-sound.md) - Particle effects (`crates/client/src/fx.rs`, `data/effects.toml`) | amended by 0050 |
| 0032 | Sound: rows of data and our own mixer | [effects-and-sound.md](effects-and-sound.md) - Sound: rows and our own mixer (`crates/client/src/audio.rs`) | in force |
| 0033 | Blocks of 1 m | [grid-and-planets.md](grid-and-planets.md) - The grid | in force |
| 0034 | Survival dig time, as in Minecraft | [mining-and-digging.md](mining-and-digging.md) - Survival dig time and the hold | amended by 0110 |
| 0035 | (number never used; no file) | - | - |
| 0036 | The keys page's groups and search, waypoint pages, the playtest menu, /place | [player-and-feel.md](player-and-feel.md) - The playtest menu (F10) | in force |
| 0037 | Gravity by size, rare blood moons, and a batch of play-test fixes | [grid-and-planets.md](grid-and-planets.md) - Gravity | amended by 0041 |
| 0038 | Three seams: the window between worlds, the ground in fast flight, the far planet's band | [streaming-and-meshing.md](streaming-and-meshing.md) - The LOD walk and the cover | amended by 0073 |
| 0039 | Block patterns with parts | [items-and-crafting.md](items-and-crafting.md) - Block patterns with parts [0039] | amended by 0045 |
| 0040 | Health, hunger, impact and death | [player-and-feel.md](player-and-feel.md) - Health, hunger and impact | amended by 0132 |
| 0041 | The evening play-test batch of 2026-11-11 | [player-and-feel.md](player-and-feel.md) - Play-test fixes of 2026-11-11 evening (mixed batch) | amended by 0132 |
| 0042 | Terra's climate from the air grid, and fifteen biomes | [bake-and-landforms.md](bake-and-landforms.md) - The air grid and the climate | amended by 0060 |
| 0043 | The bake in the engine: Terra's map, biome ground and still plants | [bake-and-landforms.md](bake-and-landforms.md) - The bake in the engine | amended by 0058 |
| 0044 | The Studio: our own art tool, and hand-made looks | [studio-and-art.md](studio-and-art.md) - The developer's calls | in force |
| 0045 | Block textures from PNG files | [studio-and-art.md](studio-and-art.md) - Textures: PNG files and the Textures page | in force |
| 0046 | The Studio's program and its interface library | [studio-and-art.md](studio-and-art.md) - Program, libraries and headless runs | amended by 0155 |
| 0047 | Nine slots and packs, death drops a pile, a night that hurts, a journal per ape / Voxel models in the game (two files) | [player-and-feel.md](player-and-feel.md) - The developer's calls; [studio-and-art.md](studio-and-art.md) - Voxel models and the model file | in force; in force |
| 0048 | Sound bases and chains | [effects-and-sound.md](effects-and-sound.md) - Sound bases and chains (`crates/client/src/audio/chain.rs`) | in force |
| 0049 | The game's renderer in the Studio | [studio-and-art.md](studio-and-art.md) - The game's renderer in the Studio | in force |
| 0050 | The effects grow, and a model's states | [effects-and-sound.md](effects-and-sound.md) - Model states (`crates/engine/src/models.rs`, `crates/client/src/models.rs`) | in force |
| 0051 | The Studio's Sounds and Effects pages | [effects-and-sound.md](effects-and-sound.md) - The Studio's Sounds and Effects pages (`crates/studio`) | in force |
| 0052 | Structures and grown flora | [plants-and-creatures.md](plants-and-creatures.md) - Structure files and grown flora | amended by 0133 |
| 0053 | Creatures and the walk rig | [plants-and-creatures.md](plants-and-creatures.md) - Creature rows and the walk rig | in force |
| 0054 | Cave reverb from rays at the head | [effects-and-sound.md](effects-and-sound.md) - Cave reverb (`crates/client/src/audio.rs`) | in force |
| 0055 | Terra's axial tilt, the seasons and the air's temperature in F3 | [bodies-and-orbits.md](bodies-and-orbits.md) - Axial tilt and seasons | amended by 0060 |
| 0056 | River lines and beds in the bake | [bake-and-landforms.md](bake-and-landforms.md) - Rivers and lakes | amended by 0059 |
| 0057 | Bake and download progress on the title screen | [bake-and-landforms.md](bake-and-landforms.md) - Bake progress on the title screen | in force |
| 0058 | Ground detail by slope, biome and height | [bake-and-landforms.md](bake-and-landforms.md) - Ground detail by slope | in force |
| 0059 | Rivers and lakes hold water | [bake-and-landforms.md](bake-and-landforms.md) - Rivers and lakes | in force |
| 0060 | The live air grid, and the bake's climate over a year with seasons | [bake-and-landforms.md](bake-and-landforms.md) - The air grid and the climate | in force |
| 0061 | Seasonal looks: four rows a block, mixed in the shader | [sky-weather-and-water.md](sky-weather-and-water.md) - Seasonal looks | in force |
| 0062 | 16-bit block ids, and one generated block for every reader | [multiplayer-saves-and-data.md](multiplayer-saves-and-data.md) - Block registry and ids | in force |
| 0063 | Rock, caves by rock, ore bodies and the ore ledger | [rock-minerals-and-ore.md](rock-minerals-and-ore.md) - Ore bodies | amended by 0166 |
| 0064 | The Studio's Textures page made plain: the block in view, patterns first, variants that explain themselves | [studio-and-art.md](studio-and-art.md) - Textures: PNG files and the Textures page | in force |
| 0065 | The Studio's model view: where am I, where does the click land | [studio-and-art.md](studio-and-art.md) - The Models page | amended by 0135 |
| 0066 | Sounds from outside files in the Studio | [effects-and-sound.md](effects-and-sound.md) - Sound import in the Studio (`crates/studio`) | in force |
| 0067 | Tides: a sea level that follows Luna | [bodies-and-orbits.md](bodies-and-orbits.md) - Tides | in force |
| 0068 | See-through leaves take light through a candidate loop; still box models are 8-byte quads in the chunk mesh | [rendering-and-lighting.md](rendering-and-lighting.md) - Cutout blocks (leaves, shrub, reed) [0068, 0069, 0088] | amended by 0069 |
| 0069 | Cutout blocks: see-through leaves, shrub and reed, a Leaves setting, light through the holes | [rendering-and-lighting.md](rendering-and-lighting.md) - Cutout blocks (leaves, shrub, reed) [0068, 0069, 0088] | amended by 0088 |
| 0070 | Luna's bake | [bake-and-landforms.md](bake-and-landforms.md) - Luna's bake | amended by 0165 |
| 0071 | The water's look: clear, traced, on the pixel grid; and blocks set into water | [sky-weather-and-water.md](sky-weather-and-water.md) - The water's look (`fs_water`, `shader_water.wgsl`, post.wgsl) | in force |
| 0072 | The light and sky bug round (2026-11-12) | [rendering-and-lighting.md](rendering-and-lighting.md) - Block light | amended by 0108 |
| 0073 | Streaming, LOD and interface bugs: 10 km/s flight, coarse cover, one window, still outline | [streaming-and-meshing.md](streaming-and-meshing.md) - One window, loading screen | in force |
| 0074 | Items, slots and piles | [items-and-crafting.md](items-and-crafting.md) - Items, slots and piles [0074] | amended by 0093 |
| 0075 | The inventory screens | [items-and-crafting.md](items-and-crafting.md) - The inventory screens [0075] | amended by 0113 |
| 0076 | The weather made visible: clouds, rain, wind, storms, snow | [sky-weather-and-water.md](sky-weather-and-water.md) - Clouds (`clouds.wgsl`, post.wgsl) | amended by 0105 |
| 0077 | The globe map | [sky-weather-and-water.md](sky-weather-and-water.md) - The globe map (`crates/client/src/globe.rs`) [0077] | in force |
| 0078 | Fine cells: where they live, and the dig's chips on their lattice | [multiplayer-saves-and-data.md](multiplayer-saves-and-data.md) - Fine cells | in force |
| 0079 | Loose blocks fall and slide; collapse designed | [mining-and-digging.md](mining-and-digging.md) - Loose blocks fall | amended by 0106 |
| 0080 | The palette grows where you paint; recovery on the Models and Sounds pages | [studio-and-art.md](studio-and-art.md) - Palette | in force |
| 0081 | Glass, and models of painted boxes | [rendering-and-lighting.md](rendering-and-lighting.md) - Glass [0081] | in force |
| 0082 | Clutter and pick-ups: computed from the seed, only what was taken is kept | [mining-and-digging.md](mining-and-digging.md) - Clutter and pick-ups | amended by 0144 |
| 0083 | Peaceful animals in the world | [plants-and-creatures.md](plants-and-creatures.md) - Peaceful animals in the world | amended by 0134 |
| 0084 | Every block painted: calm, readable textures from a script | [studio-and-art.md](studio-and-art.md) - Painted block textures (the generator) | in force |
| 0085 | Lamps side by side glow as one surface | [rendering-and-lighting.md](rendering-and-lighting.md) - Block light | in force |
| 0086 | Beds under water: sand, clay, gravel, stone and dirt by what the bake knows | [sky-weather-and-water.md](sky-weather-and-water.md) - Beds under water (`Body::bed`, engine `world.rs`) | in force |
| 0087 | Mining: read the rock, clear the rubble | [mining-and-digging.md](mining-and-digging.md) - The developer's calls | in force |
| 0088 | Fast leaves with open holes | [rendering-and-lighting.md](rendering-and-lighting.md) - Cutout blocks (leaves, shrub, reed) [0068, 0069, 0088] | in force |
| 0089 | Flora: fluffy crowns, plants of planes, more trees | [plants-and-creatures.md](plants-and-creatures.md) - The developer's calls | in force |
| 0090 | Fluffy crowns, twigs from the wood, leaves in the wind | [plants-and-creatures.md](plants-and-creatures.md) - Crowns: fluff, twigs, sway, light | amended by 0099 |
| 0091 | Plants of crossed painted planes | [plants-and-creatures.md](plants-and-creatures.md) - Small plants | amended by 0099 |
| 0092 | Trees with branches, tiers, sizes and more species | [plants-and-creatures.md](plants-and-creatures.md) - Trees of the planet | amended by 0100 |
| 0093 | Mining (a): rock digs into rubble, small stacks | [mining-and-digging.md](mining-and-digging.md) - Rubble | in force |
| 0094 | Mining (b): cracks in the rock and the drop along them | [mining-and-digging.md](mining-and-digging.md) - Cracks and the drop along them | in force |
| 0095 | The changed rock around ore bodies | [rock-minerals-and-ore.md](rock-minerals-and-ore.md) - Alteration halos | in force |
| 0096 | Features first, speed in batches; sharing the PC | [process-gate-and-performance.md](process-gate-and-performance.md) - Sharing the PC | in force |
| 0097 | Flint: on the ground by water and limestone, and inside gravel | [mining-and-digging.md](mining-and-digging.md) - Flint | in force |
| 0098 | Sky, shadow and hand fixes | [rendering-and-lighting.md](rendering-and-lighting.md) - Sky, shadow and hand fixes [0098] | amended by 0105 |
| 0099 | Flora look fixes: crowns' fluff as cards over the faces, bigger clutter, taller grass | [plants-and-creatures.md](plants-and-creatures.md) - Crowns: fluff, twigs, sway, light | in force |
| 0100 | Forest in the far chunks, trees over the face seams, animals clear of trees | [streaming-and-meshing.md](streaming-and-meshing.md) - Far chunks: forest, seams | in force |
| 0101 | Powders: a calmer slide, slopes drawn on their tops, blocks that glide | [mining-and-digging.md](mining-and-digging.md) - Dormant: slopes, slides, glides (behind `fall::SLOPES_AND_SLIDES`) | amended by 0106 |
| 0102 | Optimisation batch: CPU | [process-gate-and-performance.md](process-gate-and-performance.md) - Speed rules that hold | history only |
| 0103 | Optimisation batch: the G-buffer pass | [process-gate-and-performance.md](process-gate-and-performance.md) - Speed rules that hold | in force |
| 0104 | Rays and clouds: the bounce skips the plants, open sky skips the cloud cells | [process-gate-and-performance.md](process-gate-and-performance.md) - Speed rules that hold | in force |
| 0105 | Taller mountains, clouds at 250 m | [bake-and-landforms.md](bake-and-landforms.md) - Taller mountains and the landform principle | in force |
| 0106 | Powders only fall: ADR 0101's slopes, slides and glides switched off | [mining-and-digging.md](mining-and-digging.md) - Loose blocks fall | in force |
| 0107 | Polar ice caps: a colder count for ice toward the poles, sea and lake ice as a sheet | [bake-and-landforms.md](bake-and-landforms.md) - Polar ice caps | in force |
| 0108 | Light snapped to texels, rain only under the clouds, fairer clouds | [sky-weather-and-water.md](sky-weather-and-water.md) - Light snapped to texels [0108] | in force |
| 0109 | Builds seen from afar | [streaming-and-meshing.md](streaming-and-meshing.md) - Builds seen from afar | in force |
| 0110 | The first tools: flint knapped into a hand axe | [items-and-crafting.md](items-and-crafting.md) - Tools and knapping [0110] | in force |
| 0111 | Optimisation batch: the orbit spike, the walk at detail 8, the forest at 2560 x 1440 | [process-gate-and-performance.md](process-gate-and-performance.md) - Speed rules that hold | in force |
| 0112 | Mining (c): undercut and drop | [mining-and-digging.md](mining-and-digging.md) - Undercut and drop | in force |
| 0113 | The inventory's look: one window, the ape, pictures of things | [items-and-crafting.md](items-and-crafting.md) - The look of the inventory [0113] | amended by 0114 |
| 0114 | Dropped items drawn as their models; painted boxes on the instance path; an item's size | [items-and-crafting.md](items-and-crafting.md) - Dropped items and the model in hand [0114] | in force |
| 0115 | The bare hand: the ape's own arm in hand, and one swing for everything held | [items-and-crafting.md](items-and-crafting.md) - The bare hand and the swing [0115] | amended by 0172 |
| 0116 | The ape, the animals, the furnace and the leaf as painted boxes | [studio-and-art.md](studio-and-art.md) - Painted figures | in force |
| 0117 | The torch, and the light a held lit item gives | [rendering-and-lighting.md](rendering-and-lighting.md) - Held light and the torch [0117, 0098] | in force |
| 0118 | Sprint, and swimming | [player-and-feel.md](player-and-feel.md) - Sprint | in force |
| 0119 | Body shadows: the players' bodies as shadow proxies in the traced scene | [rendering-and-lighting.md](rendering-and-lighting.md) - Body shadows [0119] | in force |
| 0120 | Optimisation batch: the LOD walk at detail 8, dispatch, the looks in slots, the bots' clock | [process-gate-and-performance.md](process-gate-and-performance.md) - Speed rules that hold | in force |
| 0121 | (number never used; no file) | - | - |
| 0122 | (number never used; no file) | - | - |
| 0123 | (number never used; no file) | - | - |
| 0124 | (number never used; no file) | - | - |
| 0125 | (number never used; no file) | - | - |
| 0126 | (number never used; no file) | - | - |
| 0127 | (number never used; no file) | - | - |
| 0128 | (number never used; no file) | - | - |
| 0129 | (number never used; no file) | - | - |
| 0130 | The home server's container and its web page | [multiplayer-saves-and-data.md](multiplayer-saves-and-data.md) - Platforms, hosting and the home server | in force |
| 0131 | The conformal grid, as a trial behind warp= | [grid-and-planets.md](grid-and-planets.md) - The conformal warp | in force |
| 0132 | Terra pulls twice the Earth's | [grid-and-planets.md](grid-and-planets.md) - Gravity | in force |
| 0133 | The Studio catches up: plants, leaves' fluff, painted figures | [studio-and-art.md](studio-and-art.md) - Plants and Maps pages | in force |
| 0134 | Blocks, items and creatures reload in a running game | [multiplayer-saves-and-data.md](multiplayer-saves-and-data.md) - Live definitions | in force |
| 0135 | The Studio paints figures on the model, shows fluff live, and says plainly what went wrong | [studio-and-art.md](studio-and-art.md) - The Models page | in force |
| 0136 | No corner landmarks; the story is written down; the docs brought up to date | [vision-and-scope.md](vision-and-scope.md) - Story, lore and docs | in force |
| 0137 | The Steam Age is the first age; three furnaces | [vision-and-scope.md](vision-and-scope.md) - Ages and progression | in force |
| 0138 | Ore samples on the ground and a prospecting strike | [rock-minerals-and-ore.md](rock-minerals-and-ore.md) - Ore samples and prospecting | in force |
| 0139 | Hand crafting by a recipe list, and the copper tools | [items-and-crafting.md](items-and-crafting.md) - Copper tools [0139] | amended by 0140 |
| 0140 | A crafting grid, a workbench and a recipe viewer | [items-and-crafting.md](items-and-crafting.md) - Crafting grid and workbench [0140] | amended by 0143 |
| 0141 | The crucible: the first furnace | [items-and-crafting.md](items-and-crafting.md) - The crucible [0141] | amended by 0142 |
| 0142 | Tin and bronze: a furnace's second input | [items-and-crafting.md](items-and-crafting.md) - Tin and bronze [0142] | in force |
| 0143 | Block windows apart from the inventory, the recipe viewer docked right | [items-and-crafting.md](items-and-crafting.md) - The inventory screens [0075] | in force |
| 0144 | Things on the ground are aimed at themselves | [player-and-feel.md](player-and-feel.md) - Aiming at things | in force |
| 0145 | Thirst and drinking (needs step 1) | [player-and-feel.md](player-and-feel.md) - Health, hunger and impact | in force |
| 0146 | Breath (needs step 2) | [player-and-feel.md](player-and-feel.md) - Health, hunger and impact | in force |
| 0147 | Plants are cut by hand and give plant fibre | [items-and-crafting.md](items-and-crafting.md) - Plant fibre [0147] | in force |
| 0148 | What death costs (needs step 3) | [player-and-feel.md](player-and-feel.md) - Death and what it costs | in force |
| 0149 | The event list | [player-and-feel.md](player-and-feel.md) - The journal | in force |
| 0150 | The journal's data, state, message and save (journal step 2) | [player-and-feel.md](player-and-feel.md) - The journal | in force |
| 0151 | The journal's book (journal step 3) | [player-and-feel.md](player-and-feel.md) - The journal | in force |
| 0152 | The deep grid is layered: one grid to the liquid core, plain cubes inside it | [deep-world.md](deep-world.md) - The zones of a planet | in force |
| 0153 | Studio maps: the file, tiling and looping, CPU evaluation | [studio-and-art.md](studio-and-art.md) - Plants and Maps pages | in force |
| 0154 | The mantle: a new world's grid goes down to R/√2 | [deep-world.md](deep-world.md) - The mantle (step 1) | amended by 0160 |
| 0155 | The Studio looks like a modern creative tool | [studio-and-art.md](studio-and-art.md) - Pages and layout | in force |
| 0156 | The metal sea: the liquid outer core under the mantle's floor | [deep-world.md](deep-world.md) - The metal sea (step 2) | in force |
| 0157 | The game reads Studio maps for the plants' wind and the particles' turbulence | [multiplayer-saves-and-data.md](multiplayer-saves-and-data.md) - The game reads Studio maps | in force |
| 0158 | Pressure as a field, and rock and minerals that form by pressure and heat | [rock-minerals-and-ore.md](rock-minerals-and-ore.md) - Pressure | in force |
| 0159 | Cave kinds by rock and formation: lava tubes and fissures | [caves.md](caves.md) - Lava tubes (basalt; rift and volcanic arc) | amended by 0160 |
| 0160 | A world's generation number; basalt flows on the arcs; round lava tubes; only copper near the spawn | [rock-minerals-and-ore.md](rock-minerals-and-ore.md) - Generation number | in force |
| 0161 | Dry karst in the basins' limestone (generation 3) | [caves.md](caves.md) - Karst (limestone; the basin; generation 3 on) | in force |
| 0162 | A body's interior from its row | [deep-world.md](deep-world.md) - A body's interior from its row | in force |
| 0163 | Any number of bodies, each posed on its parent | [bodies-and-orbits.md](bodies-and-orbits.md) - The bodies | in force |
| 0164 | The magnetic field as a value of the world, seen as the aurora | [bodies-and-orbits.md](bodies-and-orbits.md) - The magnetic field | in force |
| 0165 | One bake of named stages; Luna's rock field (generation 4) | [bake-and-landforms.md](bake-and-landforms.md) - One bake of named stages | amended by 0167 |
| 0166 | Luna's ore and caves by her rock | [rock-minerals-and-ore.md](rock-minerals-and-ore.md) - Luna's rock and ore | in force |
| 0167 | The Mars-like planet's baked map (planet-model step 5a) | [bake-and-landforms.md](bake-and-landforms.md) - The Mars-like planet's baked map | in force |
| 0168 | A link's limits and its ping | [multiplayer-saves-and-data.md](multiplayer-saves-and-data.md) - A link's limits and its ping | in force |
| 0169 | The view runs a small planet | [studio-and-art.md](studio-and-art.md) - The view runs a small planet | in force |
| 0170 | Block ids past 255 | [multiplayer-saves-and-data.md](multiplayer-saves-and-data.md) - Block ids past 255 | in force |
| 0171 | Tuning by numbers: `tune=` | [studio-and-art.md](studio-and-art.md) - Program, libraries and headless runs | in force |
| 0172 | The swing's numbers in a data file | [items-and-crafting.md](items-and-crafting.md) - The swing's numbers in a data file | in force |
| 0173 | Mars is a body in the world | [bodies-and-orbits.md](bodies-and-orbits.md) - Mars is a body in the world | in force |
| 0174 | A gas giant, and far bodies as points of light | [bodies-and-orbits.md](bodies-and-orbits.md) - A gas giant, and far bodies as points of light | in force |
| 0175 | Sixteen bodies, loaded on approach | [bodies-and-orbits.md](bodies-and-orbits.md) - Sixteen bodies, loaded on approach | in force |
| 0176 | The giant's icy moon | [bodies-and-orbits.md](bodies-and-orbits.md) - The giant's icy moon | in force |
| 0177 | Liquids and gases are one system, and nothing is faked | [sky-weather-and-water.md](sky-weather-and-water.md) - The developer's calls | in force |
| 0178 | Mars turns | [bodies-and-orbits.md](bodies-and-orbits.md) - Mars turns | in force |
| 0179 | Closed floor: layer 0 of a body with no metal sea cannot be dug | [deep-world.md](deep-world.md) - Closed floor | in force |
| 0180 | A stronger constant of gravity; no pull set by hand | [vision-and-scope.md](vision-and-scope.md) - The developer's calls | in force |
| 0181 | Real gravity: one G, Kepler's periods, derived tides (generation 8) | [bodies-and-orbits.md](bodies-and-orbits.md) - Real gravity | in force |
| 0182 | Liquid cores, derived magnetic fields, a deep grid for every body | [vision-and-scope.md](vision-and-scope.md) - The developer's calls | in force |
| 0183 | A bare bones model of the universe: matter changes phase, water first | [vision-and-scope.md](vision-and-scope.md) - The developer's calls | in force |
| 0184 | Every body with blocks gets the deep grid (generation 9) | [deep-world.md](deep-world.md) - Every body gets the deep grid | in force |
| 0185 | A field needs a churning core; Mars's core is liquid | [bodies-and-orbits.md](bodies-and-orbits.md) - A field needs a churning core | in force |
| 0186 | Fluids are rows of data/fluids.toml; lava, one kind a gap, no mixing | [sky-weather-and-water.md](sky-weather-and-water.md) - Fluids are rows; lava, with no mixing | in force |
| 0187 | Weather-driven heat runs at the calendar's pace (about 52x); source heat at real rates | [sky-weather-and-water.md](sky-weather-and-water.md) - Weather-driven heat runs at the calendar's pace | replaced by 0200 |
| 0188 | Volcanoes and vents, fed by the planet's heat | [bake-and-landforms.md](bake-and-landforms.md) - Volcanoes and vents, fed by the planet's heat | in force |
| 0189 | Snow lies in thin layers that build and melt, counted water | [sky-weather-and-water.md](sky-weather-and-water.md) - Snow lies in thin layers that build and melt | in force |
| 0190 | Ground water: a baked table, flooded caves, springs, at the calendar's pace | [sky-weather-and-water.md](sky-weather-and-water.md) - Ground water: a table, flooded caves, springs | in force |
| 0191 | Gas near the ground as sparse 1 m cells; fire spreads by heat, a minute a block | [sky-weather-and-water.md](sky-weather-and-water.md) - Gas near the ground and fire that spreads | in force |
| 0192 | Volcanoes on Terra's arcs: cone, conduit, chamber, a lake of generated lava (generation 10) | [bake-and-landforms.md](bake-and-landforms.md) - Volcanoes on Terra's arcs | in force |
| 0193 | Lava lights, carries a body by its density, and freezes to basalt at real rates | [sky-weather-and-water.md](sky-weather-and-water.md) - Lava lights, carries a body, and freezes to basalt | in force |
| 0194 | Fluids lie in layers by density (oil on water); lava and water react to basalt and steam | [sky-weather-and-water.md](sky-weather-and-water.md) - Fluids lie in layers by density; lava and water react | in force |
| 0195 | Stored water freezes to ice from the top, ice melts back to its water, water boils by local pressure; the sea stays the formula's | [sky-weather-and-water.md](sky-weather-and-water.md) - Water freezes, melts and boils: one ledger over three phases | in force |
| 0196 | Eruptions: a live chamber fills, bursts its plug and pours counted lava at the rim until its excess is spent | [bake-and-landforms.md](bake-and-landforms.md) - Eruptions: a chamber fills, bursts its plug and pours counted lava | in force |
| 0197 | The geotherm: rock's heat at depth is a sphere's steady conduction from the province's surface gradient (generation 11) | [rock-minerals-and-ore.md](rock-minerals-and-ore.md) - The geotherm: a sphere's steady conduction from the province's gradient | in force |
| 0198 | Lava is drawn as a dark crust broken by glowing cracks, oil as a glossy slick with a film; water unchanged | [sky-weather-and-water.md](sky-weather-and-water.md) - Lava drawn as a cracked crust, oil as a glossy slick with a film | in force |
| 0199 | Felt temperature (air plus lava's radiant heat by rays), degrees on the HUD, the look-at panel names fluids with heat and hardness | [player-and-feel.md](player-and-feel.md) - Felt temperature, and the look-at panel names fluids with heat and hardness | in force |
| 0200 | Heat runs on two constants: every flow at a pace of 6, blocks and fluids store a fiftieth; lava's own cooling at half | [sky-weather-and-water.md](sky-weather-and-water.md) - Heat runs on two constants: a pace of 6 and blocks that store a fiftieth | in force |
