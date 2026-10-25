# 0059 – The four Stone Age stages: a deeper Flint Age

- **Status:** accepted
- **Date:** 2026-10-25

## Context
The Flint Age (ADR 0023, 0028, 0030, 0042) was one book chapter of 35 quests from loose flint to a
cast copper tool. The developer asked for more steps, grounded in the real Stone Age, and approved
the whole list of part F of `docs/plans/2026-10-25-bug-report-split.md`: the Old, Middle, New and
Copper Stone Age, each its own chapter, every step a mechanic in the world in the style of ADR
0030/0042 (work on the ground, held clicks, no new screens where the world can do it).

The developer's calls (2026-10-25), asked first:
- **Book layout:** today's 35 quests split over the four chapters as shown (ids kept); eight new
  steps join the main road (birch tar and tar hafting; microliths and the bow; pottery and the
  polished axe; cold-hammered copper and the malachite discovery), the rest are side roads.
- **Timing:** game hours of the calendar (sleeping counts): short jobs 1-3 game hours, curing and
  retting about a game day (the 20-minute day).
- **Megalith reward:** a travel network: a finished stone circle is home, and with two or more
  circles you travel between them.
- **Canoe:** a boat with momentum: W/S paddle, A/D turn, it glides and slows, a river pushes it.

## Options considered
- **Where the new mechanics live.** In part D's work files (`work_system.gd`, the entities) or in
  their own. Chosen: their own folder `scripts/work/stone_age/` (`StoneAge` node plus entities), one
  player hook (`item_use`, checked first on a right click) and small hunks elsewhere, so the parts
  working in parallel merge cleanly. Items and recipes went into their own files
  (`data/stone_age_items.json`, `data/stone_age_recipes.json`) for the same reason.
- **New recipe types vs one entity per step.** One shape per kind of work, data-driven:
  `bury` (earth oven), `rack` (drying and smoking), `stroke` (grinding, polishing, sewing, weaving:
  right clicks with a tool on a surface), `soak` and `trap` (set in water). The Vintage Story
  knapping grid takes bone, antler, tempered flint and native copper too (copper is cold-hammered
  "knapping" with a stone in hand, as it historically was).
- **Better heads (tempered flint) and tar.** Separate head items per quality, or per-stack data.
  Chosen: per-stack `dur_x` (ADR 0042's per-stack data), a durability multiplier carried by a head
  and the tool assembled from it, averaged on merge like spoilage.
- **Megaliths on rigid pieces** (the plan's idea) or as block entities moved a block at a time.
  Chosen: block entities: a 3-block stone with rollers and ropes, levered a block per six strokes and
  raised with sneak; rigid pieces (part A's native code) only cut loose wood today.
- **Timing in real seconds** like the charcoal pit, or calendar time. Calendar, as decided:
  `StoneAge.now_days` (the calendar's days with the time of day) is what ovens, racks, traps, the
  dugout and embers measure against.

## Decision
1. **Old Stone Age (chapter `flint_age`, now titled so).** A knife right clicked on birch bark
   peels it (8 % tinder fungus with it). A shovel on soil or sand digs an **earth oven** (a pit two
   voxels deep): four bark under two blocks of dirt with four sticks burning on top for 2 game hours
   give **birch tar**; four flint under sand for 3 hours give **tempered flint**. Tar is a binding
   with `haft_x` 2: a tool glued with it lasts twice as long; heads knapped from tempered flint carry
   `dur_x` 1.5 (3 with tar). Bone knaps into a needle, fish hooks, a harpoon head; antler into a pick.
   A raw hide left unscraped dries into a **fur pelt**; pelts laid on the ground and sewn with a needle
   (stroke work) make a hood, cloak and boots (and the bearskin cap); right click puts clothing on
   (`Clothing`: head, body, feet), and Survival feels the air warmer by its `warmth_c`, never past the
   top of the comfort band. The **atlatl** throws a spear from the inventory at twice the speed and
   1.6x the damage. Tinder fungus held into a lit fire becomes a smouldering ember (a firestarter for
   12 game hours, 4 lightings); flint + pyrite make the **pyrite striker** (`firestarter`,
   `ignites`; part D's flint and steel stays Iron Age). Ochre, charcoal and malachite grind on a flat
   rock into pigment, pigment and fat make paint, and paint right clicked on a wall paints hands,
   aurochs, deer, horses, a hunter or dots as Decals (saved with the world). A stone, fat and a grass
   wick make a **fat lamp** (`"light"`: part C's held lights).
2. **Middle Stone Age.** Microliths (four from one flint) set with tar and a feather on a stick make
   arrows; a bow stave (a log and a knife) strung with cordage makes a **bow** that shoots arrows from
   the inventory. A **drying rack** (sticks, cordage) hangs four pieces of meat or fish: dried in a
   game day, smoked in 10 hours when a lit firepit stands beside or under it (spoil days 24 and 50
   against raw meat's 2). The **dugout canoe**: an adze on a lying trunk (a felled tree, or logs laid
   in a row) at least 2.5 m long and 4 x 3 voxels thick marks a blank; four dry grass burnt on it for
   3 game hours and 24 adze strokes free the canoe. `Canoes` floats it on the water surface it is over,
   paddles and turns it with momentum and drag, a keel against sideways slip, a river current where
   the generator has a river, and stops it at land. Fishing: a fishing line bites after 0.3-1.2 game
   hours, a net brings a fish now and then, a basket trap set in water catches every 4 hours at 50 %, a
   weir twice as often in a river. Taming: three raw meat tame a wolf (part E's), which keeps
   `tamed_by`; following and hunting with the player are part E's mob code (`set_companion`).
3. **New Stone Age.** Pottery (moved, now on the main road) and the **polished stone axe**: an axe
   roughout knapped from flint, laid on sandstone within a block of water, ground with a loose stone
   48 strokes with sand every 16; the head hafts into an axe with `speed` 2 (fells twice as fast) and
   220 uses. **First farming:** einkorn, emmer and flax are crops (wheat's looks until part C draws
   them); tall grass and reeds drop their seeds now and then; grain grinds on a flat rock into flour
   (a saddle quern), flour wetted at water is dough, baked on a firepit into flatbread. **Linen:** flax
   laid in water rets in 36 game hours, breaks over a stick into fiber, spins with a drop spindle (a
   fired clay whorl on a stick) into thread, and weaves on a warp-weighted loom into cloth, sewn into
   a tunic or a sack. **Salt:** a cooking pot dipped in the sea fills with brine; boiled dry on a
   firepit it gives a pot of salt, four salt and the pot back; salted meat keeps 90 days. **Megaliths:**
   six wooden wedges split a 3-block stone out of bare rock; two logs (rollers) and two cordage
   (ropes) let a lever pole push it a block per six strokes (up one-block steps), and sneak + lever
   raises it. Five or more standing stones in a ring (within 35 % of their mean radius, at least 2 m)
   and a heel stone outside the ring within 15 degrees of the summer solstice sunrise (computed from the
   calendar's own sun) make a **stone circle**: an empty hand on its stones makes it the respawn point,
   and with two circles a picker travels to another. Standing in the circle at midsummer sunrise is the
   solstice quest. **Longhouses:** two blueprints (`tools/make_blueprints.py`) are copied into the
   build menu's prefabs (`blueprints`). The flint mine quest needs part E's chalk and floorstone flint.
4. **Copper Stone Age.** Copper nuggets (native copper from the pan and sieve) are cold-hammered
   with a stone on the knapping grid into an awl (a needle that lasts) or beads. A green copper-bearing
   stone (malachite) fired in a pit kiln gives copper: the discovery that opens melting. Then today's
   road: molds, crucible, the firepit, casting, the first copper tool. **The Iceman's kit** (copper axe,
   flint dagger, bow, birch-bark box, tinder fungus, grass cape, bearskin cap) is the capstone.
5. **Book.** 78 quests in five chapters (Old, Middle, New, Copper Stone Age, Copper Age); ids kept,
   so old saves keep their progress. New events: baked, dried, smoked, worked, caught, painted, worn,
   launched, ember, paddled, quarried, megalith_moved, raised, megalith, shrine, travelled, solstice,
   tamed, tar_hafted.
6. **Numbers:** material ids 200-202 are part E's (pyrite and the ochres); part F needed no new
   materials (its pieces are block entities on work voxels), so 203-239 and atlas tiles 168-199 stay
   reserved. Tunables in `Settings` (tinder chance, fishing, canoe thrust/turn/drag/current, dugout
   length and burn, taming).

## Consequences
- `flint_age_test=1` runs `StoneAgesTest` after its own checks (`stone_ages_test=1` alone): every
  mechanic through the player's own click code, one JSON line `debug stone_ages {...}`.
  `test_stone_ages.gd` holds the rules and the quest graph (every quest reachable, no orphan item).
- Items another part defines (part E's ochre, pyrite, antler, fish, chalk, floorstone flint) are
  `"stub"` entries in `stone_age_items.json`: they load until that part's definition is there and then
  step aside without an error. Delete them once part E is merged.
- Hunks in other parts' files (merge notes in the plan): the player's `item_use` hook, the knapping
  input in `work_system.gd`, `dur_x` in `inventory.gd`/`item_icon.gd`/`inventory_screen.gd`,
  part data in `assembly_entity.gd`, `throw` multipliers in `projectiles.gd`, `speed` in
  `mining_table.gd`, the browser's categories and info, five palette colours in `work_palette.gd`,
  BlockEntities creating StoneAge kinds, Survival reading clothing, the sprite hook in
  `make_item_sprites.py`, the `flint` kit.
- Not done yet: the dog's following and hunting (part E's mob code), storage pits, a dolmen (only
  circles count), grit and water for the quern, clothing slots on the inventory screen (asked of part
  D; clothing shows in chat and F3 until then), own crop looks for einkorn, emmer and flax (part C).
- Numbers (hours, strokes, warmth, durability multipliers, canoe handling) are first guesses to tune
  in play.
