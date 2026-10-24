# To test (playtest list)

The things to try in the current build (`build/windows/TechnoApe.exe`), newest first. Tick
what you tried, note what felt wrong. Press **F9** (or `/bug <what>`) at the moment something
looks off: it saves a screenshot, the state and the log for Claude to read. Old worlds do not
load after a generator change: start a **new world** from the title screen.

Handy: `T` opens chat, `/gamemode creative` to fly (double-tap space), `/goto <place>` jumps to
the nearest one within 4 km (`forest`, `old_growth_forest`, `taiga`, `jungle`, `plains`,
`mountain`, `lake`, `river`, `cave`, `ruin` ...), `/time set noon`, F3 shows position, biome and
frame times.

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
      Eating works whenever you are not full. An empty bar hurts down to one heart (not to death): right call?
- [ ] The thermometer left of the hotbar: at night in a cold biome it drops; standing by a lit firepit warms you; a
      closed hut (walls, roof, floor, no gaps) pulls the air halfway to 18 C (`/survival` shows the air and "in a room").
- [ ] Food spoils: raw meat rots in about 2 game days at 16 C (slower when cold, in a storage pot or a cellar 3 m
      down); a bar under the icon and the tooltip show it; rotten food turns into Rot.
