# Finding your way: coordinates, the compass, waypoints, the map

- **Status:** parts 1 and 2 built 2026-11-11 (ADR 0030); parts 3 and 4 are proposals.
- The developer's question: block coordinates as in a flat world do not fit small spinning planets with a
  moon and the space between. What then?

## The rule
A place is named the way a planet names it: **a body, a latitude, a longitude and a height.** It stays true
while the body spins and travels, reads the same on Terra and Luna, and a player can say it aloud. The
grid's own numbers (face, column, layer) stay in F3 for finding bugs.

- **North is the axis the body spins about** (its +z, `engine::sky`). So the sun rises in the east, the
  pole star stands still, and a compass means something.
- **Longitude 0 runs through the spawn** on Terra, the first Home. On a moon it runs through the middle of
  the face it keeps turned to its planet, so 0° on Luna is "under Terra".
- **Height** is over the body's mean radius. Over the sea follows when the sea level has a clean number
  (the tide moves it, `rivers-and-tides.md`).

## 1. Where you are (built)
"Terra 12.4 N 48.1 E 37 m up", under the compass. F3's Position group has it always, as a debug number.

## 2. The compass and waypoints (built)
- **The compass** is a strip top middle: 60° either side of where you look, a tick every 15°, the four
  winds by letter, north in the accent colour.
- **A waypoint** is a name at a place on a body. N drops one where you stand; the pause menu's Waypoints
  page lists them and a right click or Delete takes one away. It shows in the world as a diamond with its
  name and distance (the nearest sixteen), and on the compass at its bearing, held at the strip's end
  when it is further round. Kept beside the save, as the player's own notes.
- **Earned, not given** (the developer, 2026-11-11): in survival the compass, the coordinates and the
  waypoints come with the first tool that gives them; creative has them always. That tool does not exist
  yet, so today they are creative's.

Still to do here: naming and recolouring a waypoint, a "Home" mark set by the first bed or base, waypoints
on another body shown as a mark on that body in the sky, sharing a waypoint in chat, and the tool itself
(crafting: a lodestone needle in the stone age is the obvious first).

## 3. The globe (proposal; after the bake, `worldgen-plan.md` W2)
A flat map lies about a small sphere, and the bake already holds the planet as a map with its coarser
copies. So the map is **a globe you turn**: drag to spin it, scroll to come closer, click to set a
waypoint, your own place and heading marked on it.
- It fills in as you see the world (learning by seeing, ADR 0027): land you have walked or looked over
  from a height is drawn, the rest is bare. Climbing a mountain or reaching orbit is how you chart.
- Layers come with their systems: height first, then biomes and rivers (W3, W5), what you built, ore you
  found, and from the EV age the survey satellite's view of everything.
- Luna gets its own globe; from orbit both show at once.
- Cost: it is the bake's coarse map as one textured sphere, no chunks, so it can open in a frame.
- Open questions for the developer: is the globe a tool to be made (proposed: yes, paper and ink in the
  stone or steam age), and does a map made by one player show to their friends?

## 4. Out in space (proposal; with the jetpack and the climb to orbit)
North means nothing between bodies. There the strip gives way to **an attitude ball**: a small sphere that
turns with you, marked with where Terra, Luna and the sun are, your waypoints, and the way you are moving
and its opposite. It is what makes a landing on Luna something a player can fly by hand.

## Checks
- `nav.rs` tests: latitude, longitude and bearings against the spin axis; the waypoints' file.
- `hud.rs` tests: north in the middle when facing it, a waypoint at its bearing, the strip across north.
- A scripted run: drop a waypoint, walk, drop another, see both, remove one; the file beside the save.
