# To test (playtest list)

The things to try in the current build (`build/windows/TechnoApe.exe`), newest first. Tick
what you tried, note what felt wrong. Press **F9** (or `/bug <what>`) at the moment something
looks off: it saves a screenshot, the state and the log for Claude to read. Old worlds do not
load after a generator change: start a **new world** from the title screen.

Handy: `T` opens chat, `/gamemode creative` to fly (double-tap space), `/goto <place>` jumps to
the nearest one within 4 km (`forest`, `old_growth_forest`, `taiga`, `jungle`, `plains`,
`mountain`, `lake`, `river`, `cave`, `ruin` ...), `/time set noon`, F3 shows position, biome and
frame times.

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
