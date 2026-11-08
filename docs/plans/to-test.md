# To test (playtest list)

The things to try in the current build (`build/windows/TechnoApe.exe`), newest first. Tick
what you tried, note what felt wrong. Press **F9** (or `/bug <what>`) at the moment something
looks off: it saves a screenshot, the state and the log for Claude to read. Old worlds do not
load after a generator change: start a **new world** from the title screen.

Handy: `T` opens chat, `/gamemode creative` to fly (double-tap space), `/goto <place>` jumps to
the nearest one within 4 km (`forest`, `old_growth_forest`, `taiga`, `jungle`, `plains`,
`mountain`, `lake`, `river`, `cave`, `ruin` ...), `/time set noon`, F3 shows position, biome and
frame times.

## Machine size and machine guides (2026-11-08)

- [ ] `/kit contraptions` and `/kit lv`: the lift winch and vent fan are furnace sized (1 m), the crane and drawbridge
  hinge 1.5 m, the hand crank stays a small 0.5 m piece; `/contraptions` still lifts, raises and swings.
- [ ] Hover an LV machine in the inventory and press **P** (or `/ponder alloy_smelter`): three steps (what it is, the
  power cable, IN and OUT with a hopper) and a strip of every recipe with item icons.

## Underground hazards: firedamp, the vent fan, flooding springs (ADR 0148)

- [ ] `/scenario firedamp`, then dig into the stone room ahead with the safety lamp in hand: a low hiss as the gas
      wakes, the lamp's light turns blue and brighter near the ceiling, a toast names it. Does the flare read as a warning?
- [ ] Swap to a torch inside the room: a flash through the gas and a TNT-sized blast that cracks the room (creative
      keeps you alive). Too soon, too big, too small? Does one pocket make one blast?
- [ ] Place a vent fan at the room's door (wrench turns it, `/kit electric` has a battery box and cables), power it:
      the gas in front of it streams out of the room in a few seconds. Is 4 m of reach enough for a gallery?
- [ ] Mine a real coal vein (`/goto` a mine, or prospect one) with a torch: do pockets turn up often enough to
      matter and rarely enough to be a shock?
- [ ] Dig into an aquifer (ADR 0110: follow the trickle to spring stone): the chamber keeps pouring after it drains,
      the mine floods up to the spring and stops there; a pump below the spring drains it. Does the rate feel right?

## Moving contraptions: lift, drawbridge, crane (ADR 0160)

`/scenario contraptions` (look at open flat ground first) builds one of each, each on a turning hand crank; right click a
drive to flip its lever (or `/contraptions toggle`), `/contraptions ride` stands you on the lift.
- [ ] Lift: stand on the plank platform and flip the lever: it rises (1 m/s) through the stone ring and stops under it;
      you ride up without jitter; flip again and it takes you down. Stand under it while it comes down: it waits.
- [ ] Drawbridge: the 4 m span swings up to stand beside the hinge and back down over the moat. Stand on it while it
      rises: you ride it a bit, then slide off. Drop an item on the lift or bridge: it rides along.
- [ ] Crane: it hooks the oak log under its jib, lifts it, swings a quarter turn, sets it down; the log settles. Rope
      and hook look right?
- [ ] Build your own: `/kit contraptions`, place a lift winch, lay a 1.5 m oak-plank platform in front of it at its
      foot, cable a hand crank to it (or put it beside), flip the lever, crank. A platform of the floor's own material is
      refused with a message. Quit mid-move and reload: the part is in the voxels where it stopped and goes on once cranked.
- [ ] Riding round 2: on an L-shaped platform you do not ride over its empty corner; jumping off the rising lift
      works (`/contraptions jump`); the lift stops flush against the ring; on the rising bridge (`/contraptions ride
      bridge`, then `/contraptions toggle drawbridge_hinge`) you slide off past 45 degrees, never through it.
## Seasons that matter (ADR 0156)

- [ ] `/season winter` then `/weather snow` at night by a river: snow builds up patchy on the ground over ~20 s, the river freezes and you can walk on it. `/season winter now` does it at once.
- [ ] Meadows in winter: tall grass and flowers look snowed in, and they are all still there in spring.
- [ ] An old world (from before seasons) keeps its date on load.
- [ ] `/season spring now`: snow gone, the river is water again (no flood, no moving water).
- [ ] A season is now 9 days (3 h). Does the year turn at a good pace? Is the cold winter (-2 C mean on temperate ground) too harsh?
- [ ] Summer: rare showers, most of them thunderstorms; a lit grass fire spreads faster when it has not rained. Unwatered farmland: crops say "Parched".
- [ ] Plant beans in autumn (`/season autumn`): the look line says they will not grow. In late autumn a chat line warns of the first frost a few days ahead.
- [ ] Winter night: beans die back to dead bushes, carrots survive.
## Water wheels and dams (ADR 0154)

- [ ] `/scenario dam`: the race fills from the pool, the thin dirt dam weeps, creaks louder for ~35 s, then gives way; the flood turns the wheel and lights the lamp on it, then stops when the pit is full. Warning long enough? Break dramatic enough?
- [ ] Set a water wheel in a real river (`/goto river`): it turns; in a lake or a dug pond of still water it stays still. Craft a large water wheel (two wheels, planks, iron rod, copper coil): about 2 kW on a good flow.
- [ ] Build your own dam: a dirt wall 0.5 m thick holding over 1.5 m of water over dry ground should weep then break; three blocks thick, stone, or a spillway that keeps the level down should hold.

## Fuse cord and flung debris (ADR 0150)

- [ ] `/scenario fuse` (look at open ground first): a 4 m cord burns toward the charge with sparks and smoke, about 8 s, then the blast. Speed right?
- [ ] Craft fuse cord (cordage, sulfur, charcoal), lay it with the brush at size 1 from a TNT block through tall grass, light the far end with flint and steel: the grass beside it must not catch.
- [ ] Blasts throw chunks that fly and land as gravel/dirt piles around the crater. Violent enough? Too many or too few?
- [ ] Lay a cord up a staircase of blocks and through a puddle: it keeps burning to the charge. Stand near a blast in survival: flying chunks hurt and shove you.
## Pollution pushes back (ADR 0155)

- [ ] `/pollute` (best with a `farm_speed=40` launch): fish in the fouled third rise and hang at the surface ("Gasping"
      in the look line), then float belly-up; the wheat beside the brown waste ground says "Sickly", then wilts to dead
      bushes while the clean plot stays. `/pollute smoke` (survival): cough, grey edge, a toast, then hurt; wear a
      Cloth Mask (plant fiber, wool, charcoal) and it stops. Fair timing (cough 2 s, hurt from 8 s)?
## Movement toys: glider, climbing rope, grappling hook (ADR 0151)

`/kit toys` gives all three.
- [ ] Glider: hold it, jump off a cliff, press jump (or right click) while falling: the wing opens over your head. Looking
      level you sink slowly (about 8 m forward per metre down); look down to dive and gain speed, look up to trade it for
      height. Turning banks the view a little, the wind gets louder with speed. Too floaty, too fast, too much roll?
- [ ] Glider: jump again in the air folds it (you keep your speed for a bit); landing, water or switching item folds it too.
      A glide into the ground at dive speed hurts, a level landing does not.
- [ ] Rope: right click the top of a cliff near its edge: a rope hangs down to the ground. Walk into it from below: W
      climbs, S goes down, let go of both and you hang. At the top you step onto the ledge. Jump lets go. Right click
      the rope (its knot at the top, or the rope itself) gives it back. Does it come back after save and reload?
- [ ] Grappling hook (iron age, crafting table): right click a rock face or tree up to 36 m away, keep holding to reel
      in, let go to swing under it (WASD pumps), jump to let go and fly on. Sand, gravel or leaves: it slips. Fun? Too
      strong a pull, too slow a reel?
- [ ] On a real cliff with a lip or a ledge: the rope hangs past the ledge and you get over the lip at the top.
      Dig away the block you hung it on: the rope drops as an item. The hook holds in a tree crown: swing from it.
- [ ] With a second player (ADR 0145): they do not see your rope, line or wing (known, ADR 0151).
## Blueprints and the builder (2026-11-07, ADR 0161)

- [ ] `/kit builder` gives blueprints, a builder, a hand crank and a chest. Build a little hut with a furnace in it, hold a
      blueprint, right click, mark the box around the hut, click: a Written Blueprint; its tooltip lists size and bill.
- [ ] Hold the written one, right click: the ghost (R turns, X mirrors), click: the site stays marked in blue.
- [ ] Set the builder and the chest beside each other near the site, crank the crank, fill the chest: it builds it bottom up.
      Leave something out: its look-at lines list what is missing. Put a stone block in the ghost: it builds around it.
- [ ] No builder: stand by a site, the hint at the top says what goes next and a yellow marker shows where. Fill it by hand.
- [ ] Save, quit, load: the written blueprint and the marked site are still there.
## Meteorites (ADR 0157)

- [ ] At night `/meteor near`: a streak crosses the sky toward a spot ahead, a boom follows, an orange glow column
      stands there and a chat line + "Meteorite" waypoint point to it. Does it feel like an event worth chasing?
- [ ] Walk there: a scorched bowl with a molten core that smokes and burns if you stand on it. Dig the meteoric iron
      with a copper pick; a furnace smelts a block of it to an iron ingot. Too rare, too generous?
- [ ] `/meteor here` lands 6 m ahead (instant crater). Come back a game day later: the lava has turned to basalt.
- [ ] Nights without commands: about one in eight has a fall (`Settings.meteor_chance_per_night`). Right rate?
## Running machines look and sound running (ADR 0153)

- [ ] `/stations factory`, wait ~40 s: the boiler puffs white steam and chimney smoke and its red needle climbs with the
      pressure; the turbine breathes steam; the macerator's rollers turn and it trembles; LV machines show a green lamp while
      they work. Readable at a glance which machines run?
- [ ] Break the cable to a loaded machine: its lamp blinks red. Too subtle, too loud?
- [ ] `/kit steam`, a steam hammer on a boiler with hot metal: sparks and a puff of steam on every blow, a hiss while it runs.
- [ ] Walk away from a running factory: the hums and hisses fade; past ~32 m the puffs and parts stop.
- [ ] A loaded belt or a running lathe no longer glows like a fire at night.
- [ ] `/stations fx`: 30 machines all running at once. Frame time OK with it all in view?
- [ ] `/stations fx lathe,wiremill,cutter 1.5` (and crane, lift_winch, assembler): the moving parts are the machine's own
      pieces (lathe chuck, copper spool, saw disc, crane ring), not boxes stuck on. Do they fit the look?
- [ ] A lift or drawbridge on a hand crank: its drum turns while it moves; flip the lever with no power: a red blinking lamp.
## Surface hazards: lightning and eruptions (2026-11-07, ADR 0149)

- [ ] `/weather storm` out in the open: the sky flickers and far thunder rolls first (the warning); then strikes
      nearby: a flash, thunder a moment later, fire where it hit. Does the delay between flash and thunder read as distance?
- [ ] Build a plank tower taller than the trees around and wait in the storm (or `/lightning`): the strike goes for the
      tower and it burns; the fire rain comes later. Too often, too rare?
- [ ] Craft a lightning rod (2 iron rods over a copper ingot), place it on the tower's top: strikes within 24 m hit the
      rod instead (flash and thunder, nothing burns). Does the 2 m pole read as a rod? Does the foot break cleanly?
- [ ] Stand next to a strike (`/lightning here`): 8 health gone, "lightning" on the death screen if it kills you.
- [ ] Dig any part of a placed lightning rod: the whole pole goes and one rod drops. Iron ore or rails on a hilltop
      draw strikes a little more than stone.
- [ ] `/goto volcano` (lands on the rim now), `/erupt`: 20 s of rumble and ash over the lake, then lava over the rim on your
      side running down the cone, bombs landing on the flank. Does it feel like a threat? Does the frame rate hold?
- [ ] Wait near a volcano (`/weather` shows nothing, it rolls every 90 s): an eruption starts on its own now and then.

## Building after Lay of the Land, lava, sounds, armour (ADR 0144)

- [ ] G or V opens the ring (tap, then click; or hold and release), names under the icons. Brush tile: shapes, wheel sizes
      it. Does it replace the old G panel, or do you miss something from it?
- [ ] Hold planks, pick Cylinder: the ghost looks like planks and you can still see through it a little. Right amount?
- [ ] Handles: white arrow, pixel ring, cross at the foot, cyan under the cursor; the list at the bottom right says what a drag does.
- [ ] In a build mode no brush ghost or outline shows beside the shape, also while holding the right button to look.
- [ ] A big shape stays smooth while you drag it (coarse for a moment, then full).
- [ ] Pour lava next to logs: they catch fire. Fire crackles; water on lava hisses.
- [ ] Fire looks like flames (orange wisps, not pale yellow cubes); lava in a pool under a tree: soft steam, no columns of
      teal blocks raining down. Still ugly, unclear or sluggish anywhere? Press F9 there.
- [ ] Shift + middle click a material, dig: only that goes. Shift + middle click at the sky clears it.
- [ ] Wear copper armour: no clipping, hotbar icons read as a helmet, chestplate and greaves.

## Building with the mouse (2026-11-04, ADR 0143)

- [ ] Hold planks, V, Box, click the ground: the cursor appears. Drag the green top arrow (height), the red ring at
      the foot (grab a side, pull it out), the yellow ring on top (pull inward: hollow), the white knob (move).
      The wheel over a handle steps it one snap. Hold the right button to look around; a plain right click
      puts the shape back on your aim. Click away to build. Does dragging with the cursor beat turning the view?
- [ ] V, Select, two clicks: the selection frame gets the top arrow, the ring and the move knob too; adjust it,
      then click away to name and save the prefab.
- [ ] Looks: the ghost shows the planks pattern washed blue with a bright rim; handles are a green arrow, a white
      ball and two thin rings that show faintly through the ground where hidden. Too pale, too thick, too loud?

## Start here: what's new (2026-11-03, ADR 0130)

- [ ] `/whatsnew` lists what landed lately and opens F4 with a button per addition on top (hover one: what to
      look for); a click takes you there or hands you the kit. `/tour` walks them in order (`/tour back`,
      `/tour 5`, `/tour bridge`). 25 stops: home first (machines, power, the Workshop book, the Hunting tab),
      then the world biome by biome (bridges, igloos, tombs, temples, cairns, hoodoos, the new plants and animals).
      Lanes add stops to `game/data/whats_new.json`.
- [ ] `/book lv_workshop` opens the book at a chapter, `/browse antler` the recipe browser on an item,
      `/goto ruin_igloo 16` searches 16 km instead of 4.

## Pollution is real matter (2026-11-04, ADR 0141)

- [ ] `/stations pollution` (a slab 4 m up, ahead): two electrolyzers pour sludge. One stream turns olive-brown
      (aim at it: "fouled water (fouled: unsafe to drink)", and the journal gets an entry); the other machine's
      gravel pond turns the sludge into waste ground and its stream stays clear. Dig the waste ground.
- [ ] `/stations lv`: the electrolyzer leaves sludge behind it; wall its back in: it stops and says "Waste port
      blocked". A pipe from its back to a scrubber (`/give scrubber`) fills the scrubber's tank: sulfur out.
- [ ] A boiler or smelter puffs smoke that leans with the wind; a chimney stack on top lets it out higher.
- [ ] `/stations smoke`: a fire in a room dug 4 m down; its smoke pools under the roof (no wind) and leaks up the
      vent shaft in one corner. Is the slick on a fouled stream obvious from the bank?
- [ ] A bucket of fouled water drunk (aim at the sky, right click) makes you sick; crops watered by it wilt; cows
      will not walk into it.

## Real units, LV machines, animals, calmer water (2026-11-03, ADR 0119, 0123, 0131)

- [ ] `/stations factory`: look at a cable, the battery box and the electric furnace with the multimeter or the
      look panel: volts near 48, amps, watts (no EU anywhere). The knife switch, fuse box and breaker in the line
      conduct; right click the knife switch: the lamp and the assembler beyond it go dark, again: back on.
- [ ] Overload a fuse or breaker (more machines on one line): the fuse blows, refit it with fuse wire in hand;
      the breaker trips, right click resets it.
- [ ] `/stations lv`: the extractor (resin), cutter (planks), extruder (tin cans), canning machine, compressor
      (carbon electrodes) and electrolyzer (copper ingots) each work through their load. One turbine cannot run them
      all at once: the electrolyzer waits until the others finish. Does that read as fair or as broken?
- [ ] `/goto desert`, `taiga`, `jungle`, `forest` (and other biomes): each has its own animals (scorpions and
      vultures, ptarmigans, boars, songbirds ...). Do they sound and move right?
- [ ] `/goto lake`: the game loads quickly and the lake stays still; F3 frame times stay low beside it.
- [ ] `/scenario river`: you stand on running water (no dry bed). Dig into the bank (crater rod, laser): the
      river runs into the hole and levels out.
- [ ] Lava next to water (`/kit water`, a lava bomb into a river): it boils to a white steam cloud that rises
      and drifts; toxic sludge boils to a green gas that sinks into low caves.

## Quality of life: toggles, inventory gestures, status (2026-11-02, ADR 0130)

- [ ] Settings > Controls: turn on Toggle sprint and Toggle sneak. Ctrl starts and stops sprinting, Shift starts
      and stops sneaking (a toast says which); off again, both are held keys as before.
- [ ] Alt + mouse wheel in game: the hotbar swaps with the inventory rows (wheel down brings the first row up).
      A column with a favourite slot (Alt + click in the inventory) does not move.
- [ ] In the inventory, pick up part of a stack, then double click a slot: every same item jumps onto the cursor.
      Shift + double click a stack: all of that item moves to the other side (hotbar / main, or into an open chest).
- [ ] Settings > Interface: Coordinates and compass shows "X Y Z m  NE 45°" top left (gone while F3 is open).
- [ ] Take a hit (fall a bit): the screen edge flashes red, the lost hearts blink. Stand in snow or water until
      the thermometer goes blue: frosty edges; near lava or in a desert noon: orange edges. Low health shakes the
      hearts, low food shakes the food row. "Hurt, cold and heat effects" off removes it all.
- [ ] Every 30 s "Saving world..." shows bottom left for a moment.
- [ ] Look at a turbine, cable, battery box, lathe or electric furnace: the top panel says what it is doing and
      what its network makes, wants and moves. Middle click a machine or station: its item comes to hand.
- [ ] Hover items in the inventory: food, fuel time, "Made by", "Used in N recipes"; hold Shift for id and tags.
- [ ] Recipe view (R) at a crafting table or the 2x2 grid: Craft makes one straight into the inventory, Shift as many as you can.
- [ ] `/sethome`, walk away, `/home`: back there; the map (M) shows a green Home pin, a second /sethome moves it.
      Die in creative (`/kill`): a grey "Last death" pin marks the spot (survival still makes a grave).
- [ ] Settings > Controls "Auto step up blocks" off: walking into a 0.5 m block stops you (jump onto it), rough
      ground still walks; on again: you step up as before.
- [ ] Send a few chat lines or commands, quit the game, start again: Up in the chat brings them back.
- [ ] F1 hides the HUD, minimap and hand; F1 again shows them. F2 still saves a screenshot.
- [ ] F7 (or `/lightmap`): in a cave the floor gets red marks; put torches down and the marks around them go; outside
      at night open ground shows yellow. F7 again removes them. Frames stay smooth.
- [ ] Hold Z: the view zooms in (mouse slower), let go: back. Rebind it in Settings > Controls.
- [ ] In the inventory, middle click a main slot: the grid sorts; in a chest, middle click a chest slot: the chest sorts.

## Playtest shortcuts (2026-11-02)

- [ ] `/totest` (or F4 > "Open items") lists the newest unticked items of this list; `/totest 20` shows more.
- [ ] `/scenario river` (F4 > To-test scenarios) takes you to a river in creative with buckets, a crater rod, the
      laser and lava bombs (`/kit water`); `cave_stream` does the same and names the nearest cave to dig toward;
      `lit_station` sets night and puts down a table, chest and a burning furnace; `ledge` stands you in survival
      on a 6 m stone platform 3 m up to try Ctrl sprint and Shift stopping at the edge.
- [ ] `/stations` places all three again (the chest and furnace said "No room"), on the real ground, furnace lit.
- [ ] `/blocks` lists every machine and station with what it draws and has made (`/blocks lathe` filters);
      `/count steam` (any material) says how many cells of it the simulation holds around you.

## Faster start, near ground first (2026-10-29, ADR 0108)

- [ ] Create a new world: the loading screen lifts after 2-3 s (was ~9 s at 72 m, ~20 s at 100 m) with the ground
      around you drawn; far terrain keeps appearing at the edge for a few seconds. Load a save the same way.
- [ ] Set 100 m and fly (spectator) right after the loading screen: the ground under and around you is there, no
      holes; frames stay smooth while the distance fills in.
- [ ] Teleport far (`/tp`): the short cover and the ground under you as before.
- [ ] The monolith near spawn still rises (its lower part first; the top as you climb).

## Render distance 72 m by default, merged faces (2026-10-29, ADR 0105-0107)

- [ ] Render distance is now 72 m by default (check Settings > Video): flying (spectator) and walking feel smooth, F3 frame times stay under
      8 ms most of the time.
- [ ] The ground looks as before: same colours and occlusion shadows at the foot of steps and walls, no flickering
      single pixels or hairline cracks along flat ground and cliffs (watch while turning slowly). The corner shading
      is now drawn per pixel by the shader: dig a hole and build a wall, the dark corners follow right away.
- [ ] Dig and build: the edited spot redraws right away (an edit now remeshes an 8 m block).

## World water is endless, only spills simulate (2026-10-28, ADR 0104)

- [ ] Stand at a river, scoop with a bucket, drop a crater rod or laser a trench into the bank: the hole fills from
      the river and the river stays where it was (no dent). F3: sim tick stays low and "active chunks" is a handful,
      not hundreds, even after you touch the river in several places.
- [ ] A river surface you have touched still looks like a river (flat, no field of small cubes).
- [ ] Dig a tunnel from a river bank down into a cave: water follows you in as a stream and fills the cave up to the
      river's level, then goes quiet. Breach a bank towards lower ground: it floods that ground slowly until you plug
      the breach; it never rises above the river's surface.
- [ ] Lava bomb in a river: it hisses, steams and crusts to obsidian; the river does not drain or pile up.

## Start to iron, playtest readiness (2026-10-27)

New world. Scripted checks of the whole road (knapping, pit kilns, copper, crafting in the world, bronze,
bloomery, forge, bronze anvil) pass again; two fixes you may notice:

- [ ] Anvils, the forge, the bloomery and the firepit are 1 m pieces now, like the table and the furnace
      (ADR 0076). They need a free 1 m footprint ("No room" otherwise); a piece placed against another
      lines up flush with it. Smithing on the anvil: the 8x8 grid cells are 12.5 cm now, is aiming at a
      column easier? Old saves keep their 0.5 m bodies under a 1 m view: break and replace them.
- [ ] Held tools and blocks are bigger in the hand. Too big, right, or still small?
- [ ] First load: no hitch the first time a fire or liquid shows up (every terrain material's pipeline
      is compiled under the loading screen again; it had silently stopped doing so).

## No LOD, the brush flush and centred, placed sand falls (2026-10-26, ADR 0073)

Start a new world. The view distance is now one box of full-detail voxels (Settings > Video: 24 / 32 / 40 m).

- [ ] The world ends in sky about 40 m out, with no coarse rings, no dither pattern and no wall of cubes;
      first load takes a few seconds. From the air the ground below fades out at 30-40 m.
- [ ] Creative, 1 m cube brush: a left click on a floor digs a 1 m hole 1 m deep under the crosshair; a right
      click with sand puts a 1 m cube standing there, over the same footprint. On a wall the cube goes into /
      stands off the wall. The ghost (blue / orange shell and outline) shows exactly that at every size.
- [ ] Sand placed on the ground keeps its shape; placed hanging in the air it falls.
- [ ] G: the top of the panel says in words what left and right click do now; every mode has its line.

## Films dry up, smoother water (2026-10-26, ADR 0069 B3 and A2)

- [ ] Spill a water balloon on flat ground: the puddle spreads thin and is gone after about half a minute. A lava
      spill leaves dark basalt specks at its thin edge.
- [ ] Gentle river slopes and spills on a slope look softer, with no visible cell edges.

## Fluid baseline, parts A and B (2026-10-26, ADR 0069)

`/goto river`, `/gamemode creative`, `/kit gadgets`.

- [ ] River surfaces are smooth sheets where they slope: no triangles or wedges (the report of 15-53-53). Where a river
      used to have its waterfall it now descends as a staircase of water faces: that is the generator's steps, part C.
- [ ] Water balloon / `/blast 1 water` on flat ground: the puddle spreads two to three times as fast as before and ends
      flat; sand bombs feel as before; `/blast 1 lava` still creeps slowly. Water poured onto a lake levels out in a second.
- [ ] A water bomb dropped from high up still lands as one splash, not a spray of cells drifting sideways.

## Liquid looks and burning items (2026-10-26)

- [ ] Every liquid now has water's look: oil a dark glossy film with a faint rainbow, blood deep red, toxic sludge
      glowing green with bubbles, lava dark crust drifting on glowing cracks. Try a lush cave (sludge) and a magma
      cave (`/goto magma_cave`); `/gadget lava_bomb` or a bucket pours some anywhere.
- [ ] Throw a stack into lava (Q): it burns up with flames, smoke and a sizzle. Obsidian and a lava bucket survive.

## Part D (2026-10-25): controls, offhand, inventory, fire and weather (ADR 0055, 0056)

- [ ] Ctrl sprints, Shift sneaks: slower, eyes lower, you stop at a ledge instead of falling off. Flying, Shift goes down.
      Ctrl + wheel still sizes the brush and the laser. F swaps hands (double-tap space still toggles flight).
- [ ] Clay: Shift + right click with a clay block on the ground starts clay forming (the picker opens); right clicks then
      add clay from the block (a block = 4 balls). A plain right click places the block as before.
- [ ] Hold a seashell (or flower, mushroom) block and right click a seashell on the ground: it builds on it, no pickup.
      Empty hand + held right click still picks the shell up.
- [ ] Firepit: a quick click with coal says "Hold right click"; holding lays coal on (a piece a second). Break a firepit
      anywhere on it: it is gone completely, no ghost stones (`/kit fire` has one).
- [ ] Items on the ground get picked up from about 2 m now.
- [ ] Inventory: click outside the panel with a stack on the cursor throws it (left all, right one). Q over a slot drops
      one, Ctrl + Q the stack. The trash slot left of the hotbar row: drop a stack in; click it with an empty cursor to
      get the last one back.
- [ ] Offhand: the slot right of the bag slot; F swaps it in play. It shows on the HUD left of the thermometer and in your
      left hand. Try: torches in the offhand, empty main hand, right click a wall (places the torch); hammer in the
      offhand + work piece on the anvil, left click smiths; gravel in the offhand + empty hand on a sieve, hold right
      click (it keeps refilling); clay balls in the offhand + empty hand on a clay form.
- [ ] `/kit` lists: tools, builder, stations, farm, stone, flint, smith, magic, gadgets, fire, clutter. Builder gives full
      stacks of blocks now (it gave one block of each before).
- [ ] World vacuum (`/kit gadgets`): right click sand and gravel to choose them (chat lists the filter), hold left click
      on a beach: only sand and gravel go. Shift + right click clears the filter.
- [ ] Fire: `/kit fire`, flint and steel on the foot of a tree in a forest (`/goto forest`). It should take the tree in
      about 20 s and creep along the leaf litter to the next trees. After 1-3 minutes the sky closes, rain falls (and
      stops under roofs and trees), the rain sound plays, and the fire dies down. `/weather` shows the state;
      `/weather rain 60` or `/weather clear` sets it. Is it scary enough, too slow, too fast?
- [ ] Random weather: now and then a shower (more often in wet biomes), snow when it is below 0 C. Outside in rain the
      ape gets wet.
- [ ] Minimap top right (Settings > Video: corner and size, Off). Coordinates and biome under it. Fly around: the map
      fills in around you and stays filled. With F3 open it moves down. (Until part A's sim change is merged it shows
      the generated land, so your own digs appear only after that merge.)
- [ ] M opens the full map: drag to move, wheel to zoom, Space back to you, right click to add a waypoint (name it),
      right click a pin to rename, change colour or delete. In the world each waypoint has a light beam with its
      name and distance. Waypoints survive save and quit.
- [ ] Auto-refill: use up a stack of torches or break a tool with another in the inventory: the next one jumps into
      the hand. A tool near the end of its life beeps once and says how many uses are left.
- [ ] Pick things up: "+3 Flint" lines bottom right; with a full inventory "Inventory full".
- [ ] Settings > Controls: rebind keys (click, press the new key or mouse button, Esc cancels), Reset keys.
- [ ] Chests: Sort button on a chest and in your inventory, Stack button (puts what nearby chests already hold into
      them), type a name for a chest top right of its screen; aiming at it shows the name. Alt + click a slot marks it
      as a favourite (gold corner): Sort, Stack, shift-click and refill leave it alone.
- [ ] Die in survival (`/kill`): a grave with everything stands where you fell, the chat gives its coordinates, the map
      has a grave waypoint. Right click it: you get it all back (tools keep their wear).

## Part C (2026-10-25 bug reports): how things look (ADR 0053)

- [ ] Farming: till a patch, sow wheat and carrots. Plants stand four to a 0.5 m block and grow stalks and leaves
      through every stage, every column of a plant a stalk; ripe wheat turns gold with ears on top, ripe carrots show an orange shoulder. Breaking any
      leaf takes the whole plant and drops once. Does a field read as plants, not a slab? (`/kit farm`)
- [ ] Place a flower, a mushroom and a seashell: they grow into the same multi-voxel piece the world scatters
      (a stem with leaves and a head, a capped mushroom, a ribbed shell), and dig whole.
- [ ] Held and dropped: mushrooms, kelp and seashells show a sprite in the hand and on the ground, not a cube.
- [ ] Torches: a 1 m torch, a dark stick with a big flickering flame. Click a floor to stand one, a wall to hang one
      leaning out. Breaking any part takes the whole torch. Held and dropped it is the same model.
- [ ] The crafting table is a 1 m workbench (legs, grid, saw and hammer). It needs a 1 m x 1 m spot; with no room it
      drops back and says so. Load a saved world: the tables still show (they used to vanish).
- [ ] Ores in the inventory, in the hand and dropped: Minecraft 1.17-style ore blocks (stone with chunky clusters).
- [ ] Animals: half-voxel cells with 8-shade patterns; they walk at a calm pace now (their legs raced before).
- [ ] Night: hold a torch (or a lava bucket) and walk around: the ground around you lights up. Drop it: it keeps
      lighting where it lies. Light now reaches ~16 m with a soft edge; lit firepits and furnaces light up too.
      Ten or twenty torches in view should stay smooth.

## Part E: rock kinds, clutter, ruin pots, wild animals (ADR 0057, 0058)

New world needed (the generator changed). `/summon <species>` spawns any animal where you look.

- [ ] Cliffs, mountain flanks, river cuts and cave walls show tilted bands of different rock; walk a
      few hundred metres and the rock changes (granite pink, diorite white-black, limestone cream,
      slate blue-grey, marble white, chalk soft white, basalt near-black). Any band too stripy or too plain?
- [ ] Mine a few kinds: each drops its own cobble; smelt a cobble back into its rock in a furnace; four
      rocks make four bricks at a crafting table. Chalk breaks without a copper pick.
- [ ] Loose stones on the ground match the rock under them; any of them works for a firepit and the
      first quest ("Sticks and Stones").
- [ ] Glinting brassy pyrite nodules in slate and chalk; red and yellow ochre patches under the topsoil
      in limestone and chalk country.
- [ ] Clutter: wild strawberries (meadows, forest edges), hazelnuts and acorns (forests), pine cones
      (taiga), bones and feathers, driftwood (beaches), antlers (rare, forests), ice shards (snow),
      violet crystals on deep cave floors (many in crystal caves). Each picks up whole; are they too
      rare, too common, readable from a few metres?
- [ ] Roast acorns and cook fish at a firepit; split driftwood into sticks.
- [ ] Ruins (`/goto ruin`): clay pots in the inside corners. Any hit shatters one with shards and a
      clatter, and it drops loot (seeds, flint, cordage, nuggets, now and then a worn old tool, rarely an
      ingot). Some pots are empty.
- [ ] Ruin kinds (ADR 0062): `/goto ruin_watchtower`, `ruin_stone_circle`, `ruin_crypt` (then
      `/goto crypt_room` for the room), `ruin_mud_house` (desert, savanna), `ruin_shrine` (forests),
      `ruin_wreck` (shores, rare; may need a few km). Does each read as what it is, sit on the ground
      without a pit or a plinth round it, hold at most 4 pots, and do the pots drop loot that fits it
      (ochre and feathers at the stones, bones and ingots in the crypt, seeds in the mud house, nuts
      and mushrooms at the shrine, shells and cordage in the wreck)? Can you walk down the crypt stairs?
- [ ] Animals: deer and rabbits bolt when you come close; foxes hunt rabbits and chickens; wolf packs
      hunt deer and sheep, some go for you; a boar you hit charges back; ducks and frogs by water,
      ducks paddle out; crabs on beaches; fish in rivers and lakes (they flop on land); bats in caves;
      cave spiders bite on sight. Do they look and sound right, move at sensible speeds?

## Part F: a deeper Flint Age, four Stone Age stages (ADR 0059)

`/kit flint` has most of it. The book (inventory, top left) now has four Stone Age chapters before the
Copper Age; old worlds keep their ticks.

- [ ] Book: Old, Middle, New and Copper Stone Age chapters read well; the main road (square quests) feels
      the right length to copper; side roads (round) are clearly optional.
- [ ] Birch tar: knife on a birch trunk's white bark peels bark; shovel on dirt digs an earth oven; 4 bark,
      2 dirt, 4 sticks, firestarter; 2 game hours later dig out tar (sleeping counts).
- [ ] Tempered flint: the same oven in sand with flint, 3 hours. Knap it: the head's tooltip says it lasts
      longer; haft it with birch tar instead of cordage: "Lasts 3.0x as long".
- [ ] Bone needle (knap a bone with a stone in hand), fur pelt (leave a raw hide unscraped), sew a cloak on
      the ground (pelts, cordage, needle strokes), right click to wear it: warmer at night (`/survival`).
- [ ] Atlatl: spear in the inventory, atlatl in hand, right click: the spear flies about twice as far.
- [ ] Bow: arrows (microlith, stick, tar, feather on the ground), right click shoots; pick the arrows up again.
- [ ] Drying rack beside a lit firepit: smoke rises, meat smokes in 10 game hours; away from fire it dries in a day.
- [ ] Polished axe: roughout on sandstone next to water, sand on it, 48 strokes with a loose stone. Chops wood
      twice as fast as the flint axe?
- [ ] Grain: einkorn/emmer seeds from tall grass (rare), farm them, grind on a flat rock, flour + water = dough,
      bake on the firepit.
- [ ] Linen: flax in water (36 h), break over a stick, spin with a drop spindle, loom + thread + stick strokes.
- [ ] Salt: cooking pot dipped in the sea, boiled dry on a firepit.
- [ ] Dugout canoe: adze on a lying trunk (or 6 logs in a row, a block wide), dry grass, light it, adze strokes.
      Launch it, get in, paddle: does it glide, turn and stop at the bank the way a boat should? River current?
- [ ] Megalith: 6 wooden wedges into bare rock (3 blocks in a row), 2 logs and 2 cordage on it, lever it along,
      sneak + lever to stand it up. A ring of 5+ with a heel stone toward the midsummer sunrise (northeast):
      a circle; an empty hand on it makes it home; two circles let you travel.
- [ ] Paint: ochre or charcoal ground on a rock, mixed with fat; right click a cave wall. Do the pictures look good?
- [ ] Fat lamp in hand lights a cave (needs part C's held lights).
- [ ] Copper awl / beads from a copper nugget (knap it with a stone); a green stone fired in a pit kiln gives copper.
- [ ] Longhouse: V, Prefabs, blueprints: longhouse and small longhouse.

## Part G: the survival loop (ADR 0061)

Diet, seasons, the prospecting pick, the discovery journal and HUD trackers. Survival mode
(`/gamemode survival`); `/diet`, `/frost`, `/discover` and `/date set january 5` help.

- [ ] Inventory (E): a Diet panel under the inventory shows five bars (meat, grain, fruit, vegetable, dairy & eggs).
      Eat cooked meat, bread, blueberries, a fried egg: each fills its bar; stew fills two.
- [ ] Eat well from two or more groups (bars past the white mark): extra hearts appear (a second row past ten).
      Live on one food for a while: no extra hearts; let a group run empty: a heart goes (never below six).
- [ ] The bars drain over about a game day; sprinting drains them faster. Does the pace feel right?
- [ ] Winter: plant wheat and carrots (`/frost plant wheat`, `/frost plant carrots 1` at your feet), jump to
      January (`/date set january 5`, `/time set midnight`) in a cold place (taiga, tundra, mountains), wait or
      `/frost`: the wheat dies to a dry brown plant, the carrots (hardy) survive. Crops under a roof live.
- [ ] Blueberry bushes give berries in summer and autumn, nothing in winter and spring. Animals give less
      meat in winter.
- [ ] Root cellar: dig a room at least 1.5 m down, close it with a door or blocks, put raw meat in a chest
      there and some in a chest outside: the cellar meat stays fresh far longer (hover to see freshness).
- [ ] Prospecting: knap a flint prospecting head (flint), assemble it with a stick and binding; right click
      rock: "Something" or "Nothing" within 12 m. Smith a copper prospecting head, assemble the copper pick:
      right click rock lists each vein in 12 m with its ore shares and centre, and (once part D's map is in)
      a waypoint. Does the tap feel right, and is 12 m the right reach?
- [ ] Discoveries: open the book (top left of the inventory), the Discoveries entry above the chapters. Walk
      into a new biome, meet an animal, pick up an ore, find a ruin, a waterfall, a cave: a blue "Discovered!"
      toast and a chat line each, and the card fills in with its date.
- [ ] Pin a recipe (Pin in the recipe view, top right of the panel): a checklist shows at the right edge under
      the minimap and ticks off as you gather. Track a quest from the book ("Track on the HUD"): the same,
      and it leaves when the quest completes. Three at most.

## Part B (2026-10-25): world generation (ADR 0051, 0052)

New world needed (`GEN_VERSION` 16; the world is 640 m tall now). Handy: `/goto cavern`, `ravine`,
`geode`, `monolith`, `sea_stack`, `arch`, `delta`, `mountain`, `sulfur_cave`.

- [ ] Mountains: from a plain, walk toward a range: a long gentle foot that gets steeper and steeper, peaks
      ~400 m over the sea, snow on the upper part. Massive enough? Too much of the land mountain now?
- [ ] Caves: find a cave mouth on a hillside (a 2-4 m tunnel sloping down) and walk in; tunnels wide
      enough to move through; `/goto cavern` stands in a hall: does it feel big (40-120 m)? Bring torches.
- [ ] `/goto ravine`: a crack 10-20 m wide and 30-60 m deep, caves opening off its walls.
- [ ] `/goto geode`: a 3-6 m hollow lined with glowing violet crystal in a dark basalt shell; mine a crystal.
- [ ] `/goto sulfur_cave` (or `lush_cave`): sludge and water pools sit flat in hollows of the floor, never
      as a skin draped over a slope with green walls.
- [ ] `/goto sea_stack`, `/goto arch`: a cliff coast with rock pillars in the sea, some with a hole at the waterline.
- [ ] `/goto monolith`: a lone orange rock dome on a plain, near-vertical sides.
- [ ] `/goto delta` and lowland rivers: braided river mouths with sandbars, rivers that bend.
- [ ] The river of report 02-11-15 (`/tp 10118 1160 30835` in seed 1866545922): no water standing above
      its gravel bank; lakes on flat land end at a low rim instead of a glass wall.
- [ ] Far trees (24-120 m): slimmer trunks, smoother crowns, no thick brown pillars.
- [ ] Blueberry bushes and mushrooms are rarer (about a third of before): still enough to find food?

## Part A: water that flows, a taller live zone, cuttable falling trees (ADR 0049, 0050)

ADR 0068 made all water finite: the items below about rivers and lakes keeping their level, and the waterfall,
no longer apply (a crater in a river stays a dent with a lower pool; there is no waterfall sheet).

`/goto river`, `/gamemode creative`, `/kit gadgets` (crater rod, water and sand bombs, laser cutter).

- [ ] Crater rod on a river bank (sneak for the big one): water pours in and runs down into the crater, not
      teleported still water; it fills up to the river's level and the river keeps its level.
- [ ] Crater rod right in a river: the hole fills back from all sides, no dent left in the surface.
- [ ] Wall off a little pocket of river water (a few blocks), then dig around and under it: it spills what it held
      and no more. A small pond (smaller than about 8 x 8 x 1 m) dug into drains and its level drops.
- [ ] A big lake or the sea dug into: fills the hole, keeps its level.
- [ ] Sand bomb over a river or lake: the sand sinks to the bottom instead of resting on the surface.
- [ ] `/blast 1 lava` high up (or a cave lava pocket opened): it pours as one stream, not scattered bits.
- [ ] Pour water down a deep shaft or a water bomb on a slope, then fly away 30-40 m: it keeps flowing while you are
      gone (what still moves when the window leaves it keeps simulating in a "pin" until it rests), and is settled when
      you come back. The live zone reaches ~11 m further down and ~5 m further up than before (the terrain only holds
      full detail 24 m around you).
- [ ] `/goto waterfall`: foam streaks run down the sheet and you hear it roar from a distance.
- [ ] `/goto lush_cave` and swim/look around water in cave walls: no blue spikes in the stone.
- [ ] `/fell` a tree, then laser through the falling crown (or the crater rod on it in the air): the beam stops on it,
      cuts a hole, and a part cut off flies on as its own piece.

## 2026-10-24: sound (ADR 0044)

Everything is CC0 and cut by our tool, so a sound that feels wrong is a one-line swap: say which one
(`/sound list`, `/sound <name>` replays it). Volumes: Settings > Audio.

- [ ] Title screen: a soft piano piece starts after a second; buttons give one soft click; nothing else in the menus makes a sound.
- [ ] Walk over grass, dirt, stone, gravel, sand, leaf litter, snow, planks: do the steps sound like the ground?
      Sprinting steps come further apart and louder.
- [ ] Dig and place a few materials: hits while mining (4 a second), a break sound, a place sound.
- [ ] Jump off something 3-5 m high: a thud by the ground you land on.
- [ ] Wade in and swim: splash going in, strokes, head under water muffles everything and you hear the underwater bed.
- [ ] Walk into a cave: steps and digging get a little echo; out in a forest they stay dry.
- [ ] Fell a tree (`/fell`): creak, crash, leaves rustling as the crown breaks.
- [ ] `/blast 3` and the crater rod: a deep boom. Water bomb: water rushing as it spreads; lava (`/blast 1 lava`) bubbles.
- [ ] Fire (`/kit flint`, light a firepit; or set grass alight): crackle near by, louder as it spreads.
- [ ] Animals: sheep bleat, chickens cluck, pigs grunt now and then; hit one: a hurt sound; a hunting bear roars.
      (All approved on the sound check page; this is the in-game check.)
- [ ] Crafting: knapping clicks, clay squishes, the anvil rings, the sieve rattles, chests creak open and shut,
      the progression book opens with a page sound.
- [ ] Music: 1-3 minutes after loading a track plays, then 5-12 minutes of quiet. Too often, too rare, right mood?
      `/music next` skips through them (day: Holizna "Beginning" pieces and Yoiyami; night: Yoiyami).

## Build of 2026-10-24 late: your bug reports (crash, drops, animals, bushes, leaves, fiber)

New world for the bushes and the water (generator change, `GEN_VERSION` 14). Not touched yet: far trees.

**Water (ADR 0045)**: `/goto river`, `/goto lake`
- [ ] Rivers are one continuous sheet that steps down 12.5 cm at a time (soft ramps), no glass walls
      at the steps and no "holes" along the banks: the water reaches the bank everywhere.
- [ ] Dig a trench into a river bank or beside a pond: it fills up to the water's level and the river
      stays full (rivers, lakes and the sea never drain). A breach under a lake pours down; the water
      spreads at most 3 m from the lake and then rests.
- [ ] Throw a water bomb into a lake: it disappears into the lake instead of piling up.
- [ ] Still there, not mine: small dark pinwheels on the water are underwater plants poking through.

- [ ] The river crash: laser or dig next to a river until the water runs, then walk and fly
      around. The game kept crashing in godot_voxel when water was written to terrain that had
      just streamed out; that is patched (and the sim now waits until the terrain is loaded).
- [ ] Knapping: bare hands do nothing ("Knap with a stone in hand"); flint or a loose stone in
      hand chips.
- [ ] Plant fiber: aim at tall grass, ferns or reeds and dig them (they are aimed at now, the ray
      used to pass through them). Grass gives fiber 3 times in 10, reeds always.
- [ ] Q drops: the item leaves your hand at once and lands a few metres ahead (it used to hang
      in front of you for 1.6 s and then shoot off). Dropped items are bigger (40 cm).
- [ ] Animals: sheep, pigs, chickens, bears at their real size (they were a quarter of it), and
      visible further than 15 m.
- [ ] Blueberry bushes: rounded leafy domes with blue berries, named "Blueberry Bush" when aimed at
      (the "blue oak log" was one of these, aimed through).
- [ ] Laser through a crown: leaves with no wood within 3 m fall away (crumbs); a tree's own
      crown stays.

## Build of 2026-10-23 night: voxel world only, loading, textures (ADR 0036)

Fixes your bug report (grey world that never loaded): the Settings screen still offered the old
0.5 m distances (48/64/96 m), and 96 m is far more than the 12.5 cm world can load. It now offers
16 or 24 m, and old settings files are clamped. Ultra and F4 "Terrain detail" are gone.

**Loading**
- [ ] New world, and loading a save: the loading screen stays until the ground around you is
      there, then you can move. You and the animals stand still under it. How long does it take?
- [ ] Die (or `/kill`) far from your spawn point, and `/goto old_growth_forest`: a "Loading"
      screen until the place is there, never a grey world, nothing hits you meanwhile.
- [ ] `/goto sulfur_cave` (or `lush_cave`): you land in the cave now, not on the surface above it;
      pools of sludge (or water) on the cave floor.

**Textures** (a first pass: tell me what feels off)
- [ ] Ground, rock, sand, snow and leaves: soft natural colours, a few close shades each.
- [ ] Ores stand out in their own colours (coal black, iron rust, copper orange with green, tin silver).
- [ ] Grass voxels are green on every side; tall grass lighter than the ground; teal water.
- [ ] Crafting table, furnace, chest, held and dropped blocks: drawn in small flat cells (8 per
      block side) instead of pixel art. Still readable?

**Known, no need to report**
- Old growth at eye level sits right at the frame budget (p99 7.8-8.5 ms).

## Build of 2026-10-23 evening: landforms and living forests (ADR 0035)

**The land**
- [ ] Fly high (creative, a few hundred metres up): do you see large landmasses with flat
      plains, rolling hill country and real mountain ranges, instead of small islands?
- [ ] Walk across plains and hills: does the ground feel lumpy and alive (swells, small
      hummocks) without being annoying to walk on? Any steps you cannot walk up?
- [ ] Lake and river shores: smooth banks, no sudden ledges where the bumps stop?
- [ ] `/goto mountain`: big enough? (They are grey and stepped for now; the look pass comes later.)

**Forests** (`/goto forest`, `/goto old_growth_forest`, `/goto taiga`, `/goto jungle`)
- [ ] Trunks: thick enough now? Old trees up to 2 m across with roots, young ones thin.
- [ ] Crowns: do they read as many leaf clumps on forked limbs, reaching far out?
- [ ] Young trees between the big ones: crowns above your head, or still in your face?
- [ ] The forest floor: leaf litter under the trees, ferns, mushrooms at the foot of trunks,
      fallen logs (any direction, mossy, mushrooms on top), stumps, bushes, mossy rocks.
      Too much, too little, wrong mix?
- [ ] Clearings and meadows: tall grass that thickens and thins in patches, flower patches of
      one colour. Too busy or too bland?
- [ ] From far away: do distant forests keep their height, and is the pop between near trees
      and the far canopy acceptable?
- [ ] Chop a big trunk and a fallen log with an axe: bark outside, wood inside, logs drop.
- [ ] Dig leaf litter: it should drop dirt.

**Speed** (F3 shows frame times)
- [ ] Walking and flying through a dense forest stays smooth? Note where it stutters.
- [ ] Loading a new world takes longer now (12-17 s on the dev box): acceptable?

**Known, no need to report**
- Mountain slopes are plain grey stone in steps; ore, water and colour grade are the look pass.
- No paths or roads yet.
- A rare short stutter while flying fast (was there before).
- Fire and smoke are not yet tuned for the small voxels (smoke hangs as grey cubes).

## Build of 2026-10-23 afternoon: the 12.5 cm world (ADR 0033, 0034)

- [ ] Digging with the brush: Ctrl + mouse wheel changes the size (1, 2, 4, 8 voxels), B
      switches cube / ball / cylinder. Hands dig at most 2, tools 4, iron and better 8.
- [ ] Building: placing dirt, logs, planks with the same brush; stacks count voxels (a full
      stack is 4096), recipes still take blocks.
- [ ] Stations, chests, furnaces, furniture and Flint Age pieces sit on the ground at 0.5 m.
- [ ] The Flint Age and Stone Age chains still work from start to copper pickaxe.
- [ ] Sand, water and fire behave on the small voxels (fire spreads through a forest).

## Build of 2026-10-24: pickups and water

- [ ] Ground pickups stand out from walking height: sticks (1-1.5 m, pale top, dark bark, on a rim of
      soil), flint (dark chunks), loose stones (pale chunks), copper / tin / iron-bearing stones (grey
      stone with orange-green / dark brown / rust voxels). Digging one voxel picks up the whole thing.
- [ ] Water: clear turquoise over the bed in the shallows, deep teal further out, no texture.

## 2026-10-24: falling trees (P12, on by default; `rigid_pieces=0` turns them off)

- [ ] `/fell pillar` on open ground: a log pillar with a leaf crown appears ahead and falls across
      your view; the crown bursts into leaves where it hits, the log stays lying as voxels.
- [ ] Chop through a tree trunk with the brush (cut above the root flare, 2 m up is safe): the tree
      falls away from you, breaks the limbs and leaves it hits, and lies as voxels you can dig.
- [ ] `/fell` in a forest: the nearest big tree falls across your view (`/fell cam` moves a free
      camera to a spot that sees it). Should be smooth now apart from the command's own frame. It
      should smash through its neighbours' limbs (their trunks hold) and come to rest on the ground,
      propped on its own crown; it pivots on the edge of the cut as it tips.
- [ ] `/cam`: free camera (mouse looks, move keys fly, sprint 3x), `/cam` again to go back. Terrain detail
      and the simulation stay around you, so do not fly far.

## 2026-10-24: shape building (ADR 0038)

- [ ] Hold a building material (planks), hold **V**: a ring menu. Move the mouse to a shape, let go of V.
      Does the menu feel right (size, labels, choosing by moving the mouse)?
- [ ] The shape's see-through preview follows your aim; left click sets it down. Aim at a coloured handle
      (green top, red / blue sides, yellow hollow, white move), hold the left button and turn: it grows, hollows
      or moves. R turns it, Shift+R tips it, X mirrors it. Left click away from it builds; right click goes back.
- [ ] In survival a shape needs the material in your inventory (it says how much); the preview turns red when short.
- [ ] Box mode: two clicks mark a box, then the same handles. With a tool (or an empty hand), Box dig marks a box;
      hold the left button to dig it, the drops fall out.
- [ ] Select marks a box; click it, the chat opens with `/prefab save builds `: type a name, Enter. V, Prefabs places
      it again (also "last build").

## 2026-10-24: patterned building materials (ADR 0039)

- [ ] Build walls of planks, cobblestone (`/give cobblestone`), bricks (`/give bricks`) and thatch: boards, stones,
      brick courses and weave should read from a few metres; do they look right at this voxel size?
- [ ] Tree trunks show bark stripes now (oak brown, birch white with dashes). Better or worse than before?
- [ ] Bricks in survival: form clay into raw clay bricks, fire them in a pit kiln, four make a bricks block.

## 2026-10-24: hunger, body temperature, spoilage (ADR 0040)

- [ ] Survival: the drumsticks (right, above the hotbar) empty over about 13 minutes at rest, faster sprinting.
      Eating works whenever you are not full. An empty bar hurts one heart every 8 s until you die.
- [ ] The thermometer left of the hotbar: at night in a cold biome it drops; standing by a lit firepit warms you; a
      closed hut (walls, roof, floor, no gaps) pulls the air halfway to 18 C (`/survival` shows the air and "in a room").
- [ ] Food spoils: raw meat rots in about 2 game days at 16 C (slower when cold, in a storage pot or a cellar 3 m
      down); a bar under the icon and the tooltip show it; rotten food turns into Rot.

## 2026-10-24: lighting grade golden (ADR 0041)

- [ ] Golden is the default look: walk a forest at midday, watch a sunset, go into a cave with a torch. Does the warm
      haze still feel right while playing, and is the cave torch light too orange?
- [ ] `/grade natural` and `/grade clear` switch live for comparison; `/grade golden` goes back.

## 2026-10-24: crafting in the world (ADR 0042)

`/kit smith` gives an anvil, a firepit, a hammer, six copper ingots, logs, a flint axe head, a saw blade, sticks and cordage.
- [ ] Tools on the ground: right click a top face with the flint axe head, then the stick, then cordage. Does the tool
      read as it grows? An empty hand takes the last part back. No tool is made in the grid any more.
- [ ] Wear: dig and chop with a flint tool until it breaks. Is the bar under the icon clear, and do flint tools last
      long enough (knife 32 uses, axe/pickaxe/shovel 48, a use per block dug)?
- [ ] The firepit without a screen: right click logs onto it (one at a time, up to 8), meat on, light it with the
      firestarter, take the cooked meat off with an empty hand. A crucible, then copper stones, melt the same way.
- [ ] Smithing: a copper ingot on the lit firepit until it glows (about 8 s), take it off, right click the anvil with
      it and pick a shape. With the hammer, left click drives the top of a column away from you, right click splits a
      cell off; green = missing, red = too much. Is it fun or fiddly? Does walking round to push the other way work?
      Let it cool (40 s): take the piece off, reheat it, put it back.
- [ ] A saw: smith the blade, put it on a stick; a log and the saw make four planks.
- [ ] The Copper Age chapter in the book follows the Flint Age.

## 2026-10-24: held right clicks, whole sticks (ADR 0043)

- [ ] Hold right click on a stick or stones on the ground for a second (empty hand or a tool): the bar fills, it pops out
      as one item where it lay. Does a second feel right?
- [ ] Left click one end of a stick: the whole stick goes and one stick pops out. No half sticks left over.
- [ ] Firepit: hold right click to lay logs on (one a second while you hold), food on, and with an empty hand to take
      things off; they pop out beside the fire. The firestarter still lights at once.

## Bronze and iron ages (ADR 0070; `/kit bronze`, `/kit iron`)

- [ ] Lay a molten copper crucible back on a lit firepit and hold right click with 2 tin nuggets: molten bronze after ~15 s.
      Cast it in an ingot mold and an axe mold. Bronze pickaxe breaks iron ore; copper does not.
- [ ] Bloomery: place it, hold right click with 8 iron nuggets (or 4 iron-bearing stones), then 2 charcoal, light it.
      Does the clay shaft look right, lit and unlit? After 2 minutes take the glowing bloom out with an empty hand.
- [ ] An iron ingot or bloom on a firepit is refused ("needs a forge's heat"); the forge heats it. Does the forge read as one?
- [ ] The stone anvil refuses hot iron; the bronze anvil takes the bloom: beat it into a bar (hit the tall middle first).
      Then two hot iron ingots make a pickaxe head. Is iron enough grind, or too much?
- [ ] The ground is drawn where it is (fixed 2026-10-27: since the fluid patch of 2026-10-26 every terrain mesh sat
      62 cm off its data, so stations were buried and edits showed beside the cursor). Stations stand on the grass,
      a dug block vanishes under the cursor, you stand on the ground and not in it; water surfaces still look right.

## Placement ghosts, chest lid, laid firepit (ADR 0083)

- [ ] Hold a chest, crafting table or furnace: its see-through model shows where it will stand, blue when it fits,
      red when it does not; it lands exactly there, centred on the block you aim at. Easy to see?
- [ ] Open a chest: the lid swings up on the far side and shuts when you close the screen. Speed right?
- [ ] Right click the ground with a loose stone, then three more, then two sticks: a firepit appears where the ring
      lay. The Hearth quest completes. Does the ring of stones read before the sticks go in?

## The wolf companion (ADR 0152)

- [ ] Feed a wolf raw meat until it trusts you (or `/summon wolf`, `/tame`). It follows you within a few metres, swims
      after you across water and keeps up when you sprint. Teleport away (`/tp`): it is back beside you.
- [ ] Right click it with an empty hand: it sits and stays (tips back, a bark); again and it follows. Sits when you walk off?
- [ ] At night a gloam hound or an aggressive bear/wolf comes near: the dog growls first ("Your dog growls: ..."), then
      attacks it. Hit a deer: the dog joins in. Does the hound fight back at the dog? Can the dog die?
- [ ] Hurt dog heals slowly; hold raw or cooked meat and use it on the dog: hearts, 3 hearts back. Hit it yourself: no damage, no growl.
- [ ] Save and quit, load: the dog is still tamed, sitting or following, beside you.
## Food preservation and home comfort (ADR 0162)

- [ ] Craft a smokehouse (4 logs, 2 sticks, 2 cordage) and stand it beside a lit firepit: hang raw meat and fish (12 hooks).
      Smoke rises; after ~7 game hours the pieces are smoked. An empty hand takes a finished one. Does it read as a smokehouse?
- [ ] Craft a salt pan (4 clay balls), pour a pot of brine in (dip the cooking pot in the sea). In daylight under open sky it
      turns to a pot of salt in 8 game hours; at night or under a roof nothing happens. Pot of salt crafts to salt; salt + raw fish = salted fish.
- [ ] Salted fish / meat keep far longer in the inventory than raw (hover for the freshness line); a chest in a closed room 1.5 m underground slows it again.
- [ ] Build a closed room round a bed with a lantern or fire, a table and chairs. Look at the bed: "Room: Luxurious (86/100)".
      An open field bed reads Bare. Sleep through the night: "You feel rested", the buff line shows Rested. Does hunger fall slower, health come back faster? Is the buff long enough, the thresholds right?

## Signs, notes and trading posts (ADR 0164)

- [ ] `/kit social`, then place a sign on a wall and right click it: a parchment page opens with four lines of 15
      characters; type, press Done. The words are on the board in the world, readable from a few metres, centred, in a
      hard-edged font. Does it read from across a room? Is the lettering too heavy or too small for the board?
- [ ] A sign's text survives save and quit, and `/sign Hello|second line` still writes (| is a new line).
- [ ] Craft a note (paper + charcoal; paper from two birch bark). Shift + right click with it in the air: write a page of up
      to 12 lines of 28. Drop it, put it in a chest, pick it up: the words are still there. Click a wall with it: it is
      pinned (first lines on the paper); right click the pin to read all of it; break it and the note drops with its words.
- [ ] Place a trading post (it faces you). Right click: the owner screen. Top row = four offers as pairs (left = what the
      customer gets, right = what the customer pays): click a pair slot with a stack on the cursor to set it (left click
      = that count, right click = one more, empty hand = clear). Middle row = stock, bottom row = payments to take.
      Is that clear without being told? (The chat tells you once on opening.)
- [ ] Shift + right click your own post: the customer screen (pay left, get right). Click an offer: the right stacks
      move both ways; with too little payment, no room, no stock or a full takings row it says why and moves nothing.
- [ ] Tools (anything with wear) cannot be offered or stocked. Breaking a post spills stock and takings.
- [ ] Two players on a server: A's sign and post show for B (also for a late joiner); B trades, A sees the stock go down
      live; B cannot open A's stock or break A's post; while A has the stock screen open B is told the owner is busy.
      Quit B mid-trade and rejoin: no duplicate goods (and no lost payment unless the link broke before the answer).
- [ ] Look at your dog, `/name Fenrir`: the tag changes. Does the tag read well at 10 m? It survives save and load.
- [ ] A hound behind a hill or inside a cave wall does not make the dog growl; coming into the open does.
- [ ] Two players (server + client): each sees the other's dog follow, sit and carry its name.

## Fishing and traps (ADR 0163, 2026-11-07)
- [ ] `/fish pond`, look down at the water, right click: the red bobber arcs out and floats; a bite dips it and says so,
      a right click hooks; holding right click fills the blue bar, the tension bar goes green to red; letting it hit the
      end snaps the line, letting it go slack loses the fish. Does a catch take 5-15 s and feel fair?
- [ ] Fish in a river, a lake and the sea: different species (trout/perch, carp/pike, cod/mackerel); fillet one in the grid.
      More fish near the bobber bite sooner. `/fish foul`: only junk or nothing.
- [ ] Walk 18 m away or switch items: the line ends. Right click while waiting reels in.
- [ ] Craft a snare (cordage + stick), set it on the ground; a rabbit or hare that steps on it is held struggling; right
      click it to take the rabbit (meat drops at your feet), the snare is set again. Step on it yourself: hurt, held 2.5 s.
- [ ] Dig a 1 m x 1 m pit 1.5 m deep, right click the pit cover into it: a straw-and-earth patch flush with the ground.
      A deer or boar walking over it falls in and stays; walk over it yourself: stakes hurt. Aim at either trap: the panel names it.
- [ ] `/fish wolf`: the tamed wolf comes to you round the pit and snares, never over them. Also try a round or
      ragged 1.2 m pit: the cover goes in; a 2 m hole is refused with the size hint. A catch leaves one fish fewer in the water.
- [ ] A tamed wolf following you never walks onto a snare or a pit cover. Quit and reload: caught animals still held.
