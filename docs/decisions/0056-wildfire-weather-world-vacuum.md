# ADR 0056: Forest fires, flint and steel, weather, and the world vacuum

Status: accepted (2026-10-25)

## Context
Bug report 2026-10-25 02-15-01: "add fire and flint and steel, make fire impactful, scary and
destructive ... add weather: when a forest fire begins, start a random timer to bring rain and douse
it, so not the entire map burns down but chunks." And 02-59-22: a creative "world vacuum" that sucks up
only chosen voxels where you aim.

Direction calls (developer, 2026-10-25): fire **moderate** (a tree burns in about 20 s, no jumping
gaps, placed wood burns too, rain 60-180 s after a fire starts); **random weather as well** as the fire
rain.

Fire already lives in the native sim kernel (`SimWorld::update_burning`): per-material `fire_hp`,
`autoignition_temperature`, `temperature_of_fire`, an ignite chance of 10 / fire_hp, and water next
to a burning cell puts it out. Only cells in the SimBubble window burn.

## Decision
1. **Fire data, not kernel changes.** Leaf litter burns (fire_hp 45) and leaves dirt, so a fire walks
   the forest floor from tree to tree; trunks (bark), leaves, grass, ferns and twigs keep their values,
   which already give a tree in 15-20 s (`tools/fire_bench.gd`). Planks, wattle and thatch burn as
   before. Measured in a generated forest: one flame at the foot of a tree grew to about 160 sampled
   fire cells within 70 s.
2. **Flint and steel** (`flint_and_steel`, Iron Age: flint + iron ingot; tags tool and firestarter,
   `"ignites": true`, 64 uses): a right click puts a flame on the clicked face (`Weather.ignite`,
   `SimBubble.set_voxel(..., fire)`), when that cell is air or something burnable; on a firepit, pit kiln
   or charcoal pile it lights them like the firestarter. Any item with `"ignites"` works the same
   (part F adds a Flint Age pyrite striker). `/ignite` does it where you look.
3. **Weather** (`scripts/world/weather.gd`, `Main.weather`, `weather=0` turns it off):
   - random showers: every ~`weather_roll_s` a roll with a chance from the column's humidity starts
     ~`weather_rain_s` of rain; snow below 0 C (`Survival.env_c`);
   - fire watch: every 0.5 s, 4000 random cells within 24 m are sampled for flames, smoke and burning
     cells (`SimBubble.sample_cells`, as the sound loops do); the first fire seen (or `fire_started`, from
     flint and steel and burst charcoal pits) starts a 60-180 s timer; then it rains until no fire was
     seen for 20 s;
   - dousing: while it rains, one water cell is dropped into the air just above each fire cell the scans
     found (up to 160 a second, `SimBubble.set_voxel`); water puts burning cells out in the kernel and
     runs off into puddles (wet ground). Not water spheres (`blast`): a sphere edit runs on the worker
     pool, which a burning forest keeps busy with rigid piece checks of burnt trunks; it blocked the main
     thread 200-300 ms a call, and the spheres left water lying on the leaves;
   - look and sound: `Atmosphere.overcast` closes the clouds, dims the sun and thickens the fog; rain or
     snow particles around the camera stop on roofs and canopies (a heightfield collision that follows
     the camera); a rain loop (BigSoundBank 1289, CC0) quieter indoors; out in the rain the coat gets wet.
   `/weather [clear|rain] [s]` shows or sets it; the autopilot JSON has `weather` with a 5 s history of
   fire cells found and the rain intensity.
4. **World vacuum** (`world_vacuum`, a playtest gadget): right click a voxel to add its material (and its
   static/loose twins) to the filter the stack carries (`"filter"`, saved per stack); sneak + right
   click clears it; held left click removes only those materials from a brush-sized sphere where you
   aim, `SimBubble.blast(..., only)` (one native call per pulse).

## Measured (2026-10-25, generated forest, one flame at a tree's foot, `fire_rain_delay_s=40`)
Fire cells found per scan (4000 samples): 0 -> 19 by 55 s, when the rain set in; it kept growing in the
canopies to ~55 at 65-110 s, then fell to 3 by 160 s (`out/d_fire_win2`, filmstrip). Weather cost:
scan 0.7 ms max, drops 0.26 ms max. The frame budget could not be judged: other sessions' Godot
processes ran on the machine (a fire run without weather had process p50 22 ms as well).

## Consequences
- A fire set in a forest burns a patch of it and the rain ends it: the map is not lost.
- The fire's reach is the sim window (64 x 32 x 64 m); a bigger live zone (part A) lets fires grow.
- Cost: the fire scan is one native call every 0.5 s, the drops a few dozen cell edits every 0.25 s while it rains.
- The burning count in the sim stats stays a full scan (debug only); the weather never calls it.
