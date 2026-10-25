# Bug reports of 2026-10-25 02:01-02:59: four sessions

31 reports came in after the last fix commits (db6d353, 02:13), plus one older report that is still
open (far LOD trees, 2026-10-24 22:15). Everything older was fixed in 55f3027, df9f59f and the ADR
0046-0048 commits. The reports are in `%APPDATA%/Godot/app_userdata/TechnoApe/bug_reports/<folder>/`
(report.md, state.json, screenshot.png, log.txt): read the screenshot of every report you take on.

The split follows the code, so no two sessions edit the same native sources:

| Part | Area | Main files | Builds technosim? |
|------|------|------------|-------------------|
| A | Liquids, simulation, falling pieces | `p5/src/sim_world.*`, `rigid_pieces.*`, `sim_bubble.gd`, `voxel_patch_fluid.py`, fluid shaders | yes |
| B | World generation | `p5/src/world_gen.*`, `fine_gen.*`, `biomes.json`, `GEN_VERSION` | yes |
| C | How things look: crops, items, models, animals, light | `tools/make_item_sprites.py`, `tools/make_atlas.py`, `scripts/farming/`, `scripts/mobs/`, `scripts/blocks/*_entity.gd`, `scripts/visuals/` | no |
| D | Player, inventory, interaction, fire and weather | `scripts/player/`, `scripts/ui/inventory_screen.gd`, `scripts/crafting/inventory.gd`, `scripts/items/`, `scripts/work/`, `scripts/commands/`, new `scripts/world/weather.gd` | no |
| E | New content: more animals, stone types, clutter items (added after A-D started; not from a report) | `data/mobs.json` (new species), new stone and clutter entries in `materials.gd`/`items.json`/`mining.json`, new FineGen content in its own source file with small hooks | yes |
| F | A deeper Flint Age: four Stone Age stages (added last; not from a report) | `data/quests.json`, `recipes.json`, new Flint Age items/materials, new mechanics in new files under `scripts/work/` and `scripts/survival/` | no |

## Rules for all four

- **Own worktree each.** A and B both rebuild the technosim DLL, and a running Godot locks it; four
  sessions in one folder would keep tripping over that. The DLLs are not in git, so after the worktree
  is made: copy `game/addons/technosim/bin/` and `game/addons/zylann.voxel/bin/` from the main checkout,
  then run `--headless --path game --import` once.
- **Headless runs only** unless the developer says they are free (they may be playing).
- **Reserved numbers**, so branches merge without clashes:
  - ADRs: A 0049-0050, B 0051-0052, C 0053-0054, D 0055-0056, E 0057-0058, F 0059-0060.
  - Material ids (definition order, append only): A 116-119, B 120-129, C 130-149, D 150-159,
    E 160-199, F 200-239. A part that appends fewer leaves the gap as reserved placeholders, so ids
    stay put whatever the merge order.
  - Atlas tiles (atlas is full at 104; add rows): B 104-111, C 112-127, D 128-135, E 136-167,
    F 168-199. Grow the atlas to the rows your tiles need; the merge of `make_atlas.py` is by hand.
  - `GEN_VERSION`: only B bumps it.
- **Ask the developer first** about the direction calls listed in your part (working principle 3);
  everything else: sensible default, recorded.
- Before handing back: gdUnit passes, the part's scripted checks pass, a budget run (`autopilot=1`,
  `load_cap_s=30`) shows `budget_ok`, screenshots or filmstrips for anything visual. Add your items to
  `docs/plans/to-test.md` under one heading per part. Commit on your branch; merge order at the end:
  D, C, E, F, B, A (the native-heavy ones last, they rebuild the DLL from the merged tree).
- Something in another part's files? Message that session instead of editing it.
- **Patterned tiles above 127:** the terrain shader's `pattern_tiles` is `int[4]` (a bit mask), so only
  atlas tiles 0-127 can be `"pattern": true` (`terrain_common.gdshaderinc`, `ti < 128`). Part E widens
  it to `int[8]` (tiles up to 255) as one small hunk and lists it under the merge notes; D and F use
  patterned tiles at 128+ only on top of E's change (until the merge, test them below 128 or plain).
- **Merge notes** (hunks a part made in another part's files, to check when merging):
  - D: an optional `only` parameter on `SimBubble.blast` (part A's file, for the world vacuum); two
    sprites in `tools/make_item_sprites.py` (part C's file, just above "validate + write"). Regenerate
    `data/item_sprites.json` after merging.

---

## Part A: liquids, simulation and falling pieces

The developer's main complaint: the Noita water does not behave like water. Reports, in priority order:

1. **02-19-55** "noita simulation still not working on fluids. this crater should get filled and part of
   the river should lose elevation, gravity and all that." Plus **02-54-16** "water crater": a dug
   crater in a river leaves a dent in the surface with stray triangles; the river does not refill it.
2. **02-17-32** a 3x3x3 pocket of water, dug out, gave far more water than it held: the reservoir rule
   (ADR 0045: generated water fills openings, never drains) should only apply to big bodies (sea, big
   lakes, main rivers), not to small pockets and puddles.
3. **02-22-37** water high up is frozen, not simulated: "a smaller area around the player that is always
   simulated, and then everything in vision". Today `SimBubble` is a 64 x 32 x 64 m window.
4. **02-17-49** sand should sink through liquids and push them up (density swap in the kernel).
5. **02-23-24** lava pours out of a cave wall as scattered loose cells, not a stream.
6. **02-02-16** under water in a cave, the fluid corner patch draws spikes/triangles inside the stone
   (`voxel_patch_fluid.py`: corners next to open air drop; here they drop against solid voxels).
7. **02-09-43** waterfalls do not work and have no sound (`WorldGen.waterfalls_near`, `waterfall_spray.gd`,
   the `waterfall` sim loop in `data/sounds.json` needs moving water with air below).
9. **For part D's map** (small): a native `SimWorld.surface_map()` returning, per column of the live
   window, the top non-air material and its height (PackedByteArray/PackedInt32Array), so the minimap
   shows real edits. Agree the exact signature with part D.
8. **02-51-52** "meshes should still be destructible": a felled tree in flight (a `RigidPieces` mesh) cannot
   be cut with the laser or dug. Pieces in flight should take digs/cuts like voxels.

Direction calls to ask first: what counts as a "big body" for the reservoir (size threshold, or only
sea + named rivers/lakes?); should a breached river really lose level downstream (drainable rivers
were turned down for reservoirs on 2026-10-25, this report asks for it back); how big the always-live
zone may be against the 8.3 ms budget (measure the cost of a bigger or taller window first, offer
2-3 sizes with numbers).

Checks: `water_test=1 target=river`, `tools/water_surface_probe.gd`, a laser crater beside a river,
filmstrips of the refill (never judge motion from one frame), `/fell` + laser through the falling crown.

## Part B: world generation

1. **02-03-27** caves are small and unusable, and there are no cave entrances on the surface.
2. **02-28-03** (an instruction, not a bug) "worldgen should use even more real-world features, looks and
   landmarks, and make caves epic": research real landforms worth adding (canyons, mesas, arches, sea
   stacks, karst towers, fjords, volcanic cones, river deltas, waterfalls over cliffs ...) and cave types
   (huge caverns, ravines, lava tubes, flooded caves, cave rivers, crystal geodes), pick with the developer.
3. **02-05-38** mountains look good but should be more massive: a wide foot that rises gently and gets
   steeper and steeper, with very steep peaks.
4. **02-11-15** "more generation glitches": the river surface stands above its gravel bank as a glass wall
   (water level higher than the bank it should rest against: FineGen river levels / bank fill, ADR 0045).
5. **02-20-46** sludge "generates weird": a toxic pool whose surface stands above its rim with vertical
   walls, same fault as 4 for generated pools.
6. **Older, still open (2026-10-24 22:15):** far LOD trees look chunky and ugly (LOD 1-2 beyond the 24 m
   full-detail cap; `far forests as leaf domes`).

Direction calls to ask first: the list of landforms and landmarks to add and how rare each is; cave
scale (how big is "epic", how many entrances per km^2); the mountain profile (show 2-3 variants with
`tools/terrain_map.gd` and screenshots before committing to one). New cave or landform materials use
ids 120-129, tiles 104-111. Bump `GEN_VERSION` once at the end.

Checks: `tools/terrain_map.gd`, `fine_gen_bench.gd` (generation cost per block must not grow much: loads
are already 12-17 s), `/goto cave`, `/goto mountain`, `/goto river`, screenshots from `/cam`, a budget
run in a forest and in the mountains. Keep `Biome` ids 0..12 stable.

## Part C: how things look

Everything here is a look, so show sheets or screenshots to the developer before finishing.
Their taste (see memory "voxel objects big and readable"): big, thick, irregular, varied, 8-colour
patterns, one piece, visuals over accurate sizes, treat all of a kind alike.

1. **02-16-36** farming looks weird: a carrot field is a flat orange slab with green holes. Crops should be
   multi-voxel plants (stems, leaves, the crop showing) through every growth stage, like the clutter pieces.
2. **02-35-52** a placed flower is still one weird pixel: placed flowers should be the same multi-voxel
   piece the generator scatters (ADR 0047).
3. **02-28-54** a mushroom in the hand looks like a block: held and dropped clutter (mushrooms, flowers,
   shells, kelp ...) should show its sprite or its voxel shape, not a cube.
4. **02-53-29** torches should be multi-voxel pieces (on the ground and on walls).
5. **02-44-58** the crafting table is too small and ugly: a full block-sized, readable model.
6. **02-21-05** ores in the inventory should look like modern Minecraft ore sprites (`make_item_sprites.py`).
7. **02-12-28** animals do not fit the game's look any more, and their animations play too fast.
8. **02-01-49** a held or dropped light source (torch, lava bucket ...) should light its surroundings, like
   Minecraft shader packs' dynamic lights. Mind the budget: a few `OmniLight3D`s with shadows off, capped.

New materials use ids 130-149, tiles 112-127. Every item still needs a sprite or block material
(`test_item_sprites.gd`); `tools/item_sheet.gd` renders them all. New models drawn mid-game need the
pipeline warm-up (`material_warmup.gd`).

Checks: item sheet, `tools/ape_sheet.gd`-style sheets for mobs and models, a filmstrip of a walking
animal (motion is never judged from one frame), screenshots of a farm at each stage, a budget run with
10 torches in view.

## Part D: player, inventory, interaction, fire and weather

1. **02-47-46** clay knapping places the clay instead. Controls like Minecraft: Ctrl = sprint, Shift =
   crouch/sneak, and crouch + right click on clay starts clay forming. Ctrl + wheel is brush size and
   laser width today: keep that working while Ctrl also sprints.
2. **02-18-35** cannot build onto a seashell (or other clutter): right click tries to pick it up. Picking
   up should only happen with an empty hand or a held click (ADR 0043); a click with a block in hand places.
3. **02-27-14** cannot put fuel (coal) into the firepit, and a broken firepit leaves its voxels as ghosts.
4. **02-36-31** pick up items on the ground from a bit further away.
5. **02-56-13** inventory: click outside the window to drop the held stack, Q over a slot drops it, and a
   trash slot (as in Minecraft mods).
6. **02-57-10** an offhand slot, as in Minecraft and Vintage Story, so in-world crafting (sieving, smithing:
   hammer in one hand, work piece in the other) is easier.
7. **02-59-22** creative world vacuum: pick which voxels it takes; hold left click to suck up only those
   where you aim. Belongs with the playtest gadgets (`scripts/items/playtest_tools.gd`).
8. **02-23-50** update the kits (`/kit`): bring every kit up to date with today's items and gadgets.
9. **02-15-01** fire and flint and steel: make fire a scary, destructive force (forest fires that spread
   fast), and add weather: when a forest fire starts, a random timer brings rain that puts it out, so a
   region burns, not the whole map. Fire spread is the sim's fire (data in `materials.gd` first); if the
   kernel needs changing, ask part A. Rain is new (`scripts/world/weather.gd`): sky, particles, sound,
   wet ground, and it puts fires out.

10. **Minimap and world map** (developer request, added later): a minimap on the HUD whose corner
   and size are set in Settings (`user://settings.cfg`, the ADR 0032 Settings screen), with the
   coordinates and the biome in small text under it; M opens a full map. Both show only what you have
   explored, and they update as you go, so your base, digs and builds show up. You can place, name
   and delete waypoints on the full map (shown on both maps and as a marker in the world); they are
   saved with the player, and the explored area is saved with the world.
   Suggested build (keep GDScript out of per-voxel loops, principle 8): far terrain from
   `WorldGen.map_image` (exists, deterministic, part B's: read only), shown only where the explored mask
   is set; near terrain from the real voxels, so edits show: part A adds a small native
   `SimWorld.surface_map()` (top material and height per column of the live window) that you colour
   per material (average colour of its atlas tile, computed once per material). Cache explored map
   tiles as images per region; redraw the minimap a few times a second, not every frame.

Direction calls to ask first: the map style (flat colours or shaded relief, how waypoints look); the
offhand key and what goes in it; how destructive fire may be (spread
speed, how big a burn before rain, can it burn placed wood buildings?); rain everywhere sometimes, or
only as the answer to fires. New materials or items use ids 150-159, tiles 128-135.

Checks: gdUnit for inventory moves (offhand, trash, drops keep per-stack data: `Inventory.make_stack`),
`craft_test=1`, the menu and hands tests, a scripted forest fire with `later=` and a filmstrip of it
burning and the rain putting it out, a budget run during the fire.

## Part E: more animals, stone types and clutter items

Not from a report: the developer asked for more content alongside A-D. Three kinds, each data first
(principle 6: data + schema + tests), each fitting the voxel look (memory "voxel objects big and
readable": big, thick, irregular, varied, 8-colour patterns, one piece, all of a kind alike).

1. **Animals.** New species in `data/mobs.json` with models built like the existing ones (`MobModel`,
   MicroMesher at 6.25 cm), behaviour from the existing kinds (grazer, prey, hunter like the bear), spawn
   biomes, drops, sounds (`data/sounds.json` mobs, CC0 sources as ADR 0044). Ideas to pick from with the
   developer: deer, boar, rabbit, fox, wolf, goat in the mountains, duck/frog by water, fish in rivers,
   crab on beaches, bats and cave spiders underground. Part C restyles the existing animals (report
   02-12-28): message part C for its style decision before modelling, so new and old animals match.
2. **Stone types.** Real rock kinds with their own palette and pattern (granite, diorite, andesite,
   limestone, sandstone variants, slate, marble, chalk, tuff ...), placed as strata and regions in the
   world, each with hardness, drops, a cobble and a brick form where it makes sense, and a place in the
   ages (Vintage Story-style rock types decide which ores show up). Placement lives in the generator,
   which part B is reworking: put the rock-type selection in its own function/file
   (e.g. `src/rock_types.*`) called from FineGen with a small hook, and message part B about the hook.
3. **Clutter items.** More ADR 0047 pieces that dig whole: e.g. pine cones, acorns, feathers, bones,
   antlers, snail shells, driftwood, berries, nuts, small rock kinds matching the new stone types, ice
   shards, cave crystals. Each is a voxel piece on the ground, an item with a sprite, and is placed by
   biome. Generator placement again through a separate function with a small hook in `floor_features`.

Direction calls to ask first: which animals (and which bite), which stone types and how many, which
clutter items and what each is for (a use in crafting, food, or only decoration). Show model and
material sheets before committing to a look. New materials use ids 160-199, tiles 136-167, ADR
0057-0058. Do not bump `GEN_VERSION` (part B does it once); tell part B your generator change needs it.

Checks: gdUnit (every item has a sprite, mobs.json schema, rock types deterministic from seed),
`tools/item_sheet.gd`, model sheets, `tools/clutter_probe.gd`, a filmstrip of each new animal moving,
`fine_gen_bench.gd` (no big cost growth), a budget run in a forest with the new clutter and herds.

## Part F: a deeper Flint Age

Not from a report: the developer asked for more progress steps in the Flint Age, historically
grounded, and approved the whole list below (2026-10-25). Today the Flint Age is one chapter of 35
quests (ADR 0023, 0028, 0030, 0042) running from knapping to copper casting. Part F splits it into the
four real stages of the Stone Age, each its own chapter in the progression book, and adds the steps
below as data plus in-world mechanics in the style of ADR 0030/0042 (work on the ground, held clicks,
no new screens where the world can do it). Build the starred ones first; the rest follow in order.

**1. Old Stone Age (hunter-gatherers)**
- ★ Birch tar: roll birch bark, bury it under embers (a small sealed pit like the charcoal pit), get
  tar. Tools hafted with tar wear half as fast as cordage-lashed ones (`durability`).
- ★ Heat-treated flint: flint buried in sand under a burning firepit becomes tempered flint, which
  knaps into sharper, longer-lasting heads.
- ★ Bone and antler: an eyed bone needle sews fur clothing (cloak, hood, boots) that counts in body
  temperature (`Survival`); bone fish hooks, harpoon, antler pick.
- ★ Spear-thrower (atlatl): throws the existing spear about twice as far and hard (`Projectiles`).
- Keeping fire: tinder fungus holds an ember to carry fire; the Flint Age striker is flint + pyrite
  under its own id (e.g. `pyrite_striker`). Part D keeps `flint_and_steel` as the Iron Age item (flint +
  iron ingot); giving the striker `"ignites": true` and the `firestarter` tag makes it work with D's
  fire code, no hook needed.
- Ochre and cave painting: dig red/yellow ochre, grind it on a stone, mix with fat, paint hand stencils
  and animals onto walls as a patterned overlay; a quest.
- Fat lamps: a hollowed stone with fat and a moss wick, a carried light for caves. Part C's
  `scripts/visuals/torch_lights.gd` lights any item with `"light": {}` in `data/items.json` (optional
  `{"range_m": 7.0, "energy": 1.5, "color": "#ffb86b"}`, torch values are the defaults) while held and
  while dropped; nothing else to register.

**2. Middle Stone Age (water and the bow)**
- ★ Microliths and the bow: tiny flint bladelets set in tar make arrowheads; a bow from a stave with a
  sinew or cordage string.
- ★ Dugout canoe: fell a big tree (rigid pieces), hollow the log with fire and an adze; the first boat.
- Fishing: wattle fish weirs and basket traps in rivers, bone hooks, cordage nets (fish come from part E).
- ★ Drying and smoking racks: meat hung over smoke keeps much longer (`spoil`).
- The dog: tame a wolf with meat scraps; it follows and helps hunt (wolves come from part E).

**3. New Stone Age (settling down)**
- ★ Polished stone axe: grind a roughout on a sandstone slab with sand and water, many strokes; slow to
  make, tough, fells big trees much faster.
- ★ First farming: gather wild einkorn, emmer and flax, sow them; a saddle quern grinds grain to flour,
  baked into flatbread on a hot stone.
- Flax to linen: ret in water, break, spin with a drop spindle (clay whorl), weave on a warp-weighted
  loom; cloth for clothes and bigger bags.
- Flint mines: surface nodules are poor; good flint lies deep in chalk, mined with antler picks
  (flint quality tiers). Needs chalk from part E's stone types.
- ★ Megaliths: move a huge stone on log rollers with levers and ropes (rigid pieces), raise a dolmen or
  stone circle lined up with the solstice sunrise (calendar); reward: a home shrine / respawn point
  that marks the seasons.
- Salt: boil brine in clay pots; salted food barely spoils.
- Longhouses: prefab blueprints for shape building, plus storage pits.

**4. Copper Stone Age (the bridge)**
- ★ Native copper, cold-hammered with a stone into awls and beads before smelting; green malachite
  (paint) turning into copper in a hot kiln is the discovery quest that opens the Copper Age.
- Ötzi's kit as the finale: copper axe, flint dagger, birch-bark boxes, tinder fungus, grass cape,
  bearskin cap; one capstone quest.

Direction calls to ask first: how much of today's 35 quests moves into which stage (show the new book
layout before rewriting it); the timing scale (minutes like the charcoal pit, or calendar days); what
the megalith reward is exactly; how a boat handles. Everything else: sensible default, recorded in
the ADR.

Coordination: part D owns `scripts/work/` (the firepit fuel fix) and `scripts/player/`/inventory:
put new mechanics in new files and keep hooks into existing ones small, and message part D about
them. Part E brings fish, wolves, chalk and other stones: build against their names, message part E
for them, and stub what is not there yet. Part C owns the light from held light sources and the
model looks. New materials use ids 200-239, tiles 168-199, ADR 0059-0060.

Checks: gdUnit for every new recipe type and the quest graph (every quest reachable, no orphan item),
`flint_age_test=1` and `craft_test=1` extended so each new mechanic is driven through the player's own
clicks, `/kit flint` updated, item sheet, a budget run, items in `docs/plans/to-test.md` under "Part F".

---

## Briefs to paste into each session

Each session reads `CLAUDE.md`, this file and its own part, then starts with the direction calls.

- **A:** "Take part A of docs/plans/2026-10-25-bug-report-split.md (liquids, simulation, falling pieces).
  Work in your own worktree as the file says. Read every report screenshot of your part first, ask me the
  direction calls, then fix in the priority order given."
- **B:** "Take part B of docs/plans/2026-10-25-bug-report-split.md (world generation). Own worktree. Read the
  screenshots, research landforms and cave types, ask me the direction calls with sheets, then build."
- **C:** "Take part C of docs/plans/2026-10-25-bug-report-split.md (how things look). Own worktree. Read the
  screenshots, show me sheets before you finish each look."
- **D:** "Take part D of docs/plans/2026-10-25-bug-report-split.md (player, inventory, fire, weather). Own
  worktree. Read the screenshots, ask me the direction calls, then build."
- **E:** "Take part E of docs/plans/2026-10-25-bug-report-split.md (more animals, stone types, clutter
  items). Own worktree. Ask me which ones, show me sheets, coordinate with parts B and C as it says."
- **F:** "Take part F of docs/plans/2026-10-25-bug-report-split.md (a deeper Flint Age). Own worktree.
  Ask me the direction calls, build the starred steps first, coordinate with parts C, D and E."
