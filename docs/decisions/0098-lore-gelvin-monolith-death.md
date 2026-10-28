# 0098 – Lore: the Gelvin monolith, and death as reconstruction

- **Status:** accepted (built 2026-10-27)
- **Date:** 2026-10-27

## Context
Requested 2026-10-27: dying and respawning at the bed or the spawn, plus "a fun little thing" on death
that fits the lore, and the lore had to be made up first. Dying, graves (ADR 0055) and the bed respawn
point already worked; death just teleported the player at once. The developer picked "The Monolith" out
of four directions (a lineage of apes, a monolith, both, a shamanic dream) and gave the frame: the
**Gelvins**, an alien race, planted the seed (the player, an **apetion**) with an intellect gene and an
indestructible monolith at spawn that rebuilds the player on death; hidden from the player, the Gelvins
want a singularity built for their own gain. The rest stays open. Story text: `docs/design/lore.md`.
Asked for in the same session: the monolith after a reference picture (two stacked stone columns, a
glowing magenta crack, a stepped base), "huge and pretty and detailed with runes", a broken stone floor,
all of it indestructible; a sky beacon to find spawn; the body crumbling to powder in third person and
swirling back together on respawn.

## Decision
- **Death sequence** (`Main._on_died`, `_dying` guards against more `died` signals): the player node is
  disabled (frozen where it fell) and the view goes third person; `ApeDust.crumble` turns the body to
  ~700 grains sampled from the ApeModel mesh (positions and palette colours), head first, which fall and
  pile at the feet (2.2 s). Then `DeathScreen.play`: a screen-texture shader breaks the view into green
  voxels, a Gelvin terminal types `APETION nnnn SIGNAL LOST`, the cause and a reconstruction bar; at the
  black point the player respawns (bed, else spawn) and `APETION nnnn+1 ONLINE. CONTINUE.` types out;
  the view resolves out of the voxels. Then `ApeDust.assemble`: grains appear out of thin air around the
  spawn point and spiral in from the feet up, magenta cooling to the body's colours (2.8 s); then the
  model shows, the view returns to what it was and control comes back. About 11 s. `tp_test` skips it.
  Both the grains' and the screen's pipelines compile under the loading screen (`ApeDust.warm`,
  `DeathScreen.warm`): without that the first death drew a 53 ms frame.
- **Death cause:** `PlayerInventory.damage(amount, cause)` keeps `last_cause`; every damage source passes
  one (fall height, drowning, burning or toxic coat, starvation, hypothermia, heatstroke, explosion,
  `<species> attack`, hot metal, /kill).
- **Apetion number:** `Main.deaths`, in the player save (`"deaths"`).
- **The monolith** (`scripts/world/monolith.gd`), 12 m north of spawn (the way a new player looks):
  two 3 m ashlar columns (26 and 28 m) of 2 m blocks with recessed joints, a rune band on every 4th
  block, pointed tops, a glowing magenta seam between them, four 1 m steps, four rune pylons and a
  broken flagstone plaza (18 m radius, thinning out, some slabs cracked or proud) with rubble and fallen
  blocks. Three new materials, ids 204..206 `monolith`, `monolith_rune`, `monolith_seam`, atlas tiles
  173..175 (patterned, 16-voxel period, so heights snap to 2 m); the terrain shader glows only the
  magenta texels of the rune and seam tiles, with a slow breath.
- **Indestructible:** not in mining.json (survival cannot mine it), hp 1e6 (`SimBubble.blast` keeps it:
  TNT, laser cutter, crater rod), and `GameMaterials.is_monolith` keeps it out of the creative dig,
  brush and paint masks (`test_monolith.gd`).
- **Raising it:** `Monolith.boxes` lists ~700 world boxes (overlaps stay inside the stone, so any order
  works); Main fills up to 48 per frame through `SimBubble.fill_box`, retrying boxes whose chunks are
  not in yet; done once per world (`"monolith"` in the player save), worlds made before this get it on
  their next load. A "Monolith" waypoint is added then.
- **Light:** a magenta OmniLight in front of the seam washes the steps and plaza; `MonolithBeacon` is a
  2 km beam of additive light from the seam's tip, seen from anywhere: past 40 m it is scaled toward the
  camera around the camera (same picture, inside the 60 m far plane), its width held to a visible line.

## Consequences
- Dying costs about 11 s of control, all of it animated.
- The tallest parts are 30+ m up: from far off they blend into the sky with the rest of the terrain (the
  40 m world box, ADR 0073); the beacon is what shows from far away.
- Raising it is ~700 small native fills over a few frames after the spawn loads; the bubble check in the
  debug JSON reports them as edits (`limestone->monolith`).
