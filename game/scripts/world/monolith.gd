class_name Monolith
extends RefCounted

## The Gelvin monolith at the world spawn (ADR 0098, docs/design/lore.md), after the developer's
## reference: two ashlar columns (48 m, as the developer asked for taller and one height so the top reads
## straight) with glowing rune bands and a magenta seam between them, pointed tops, a stepped base, four rune pylons and a broken flagstone plaza with rubble. All of it is
## MONOLITH* material: never mined (not in mining.json), never blasted (hp past Tnt.NEVER_HP).
## boxes() lists [lo, hi, material] world boxes for SimBubble.fill_box. Overlapping boxes only overlap
## inside the stone, so they can be filled in any order: Main retries the ones not loaded yet (the top of
## the columns waits for the player to come up within the sim window's reach).
## Courses and runes are patterned tiles (16 voxels = 2 m), so heights snap to that grid.

const V := 8          # voxels per metre (ADR 0033)
const P := 16         # pattern period: one rune band, one 2 m ashlar course
const COLUMN_W := 3 * V
const SEAM_W := 4
const BASE_HALF := 7 * V
const PLAZA_R := 18 * V
const BLOCKS := 24    # 2 m blocks per column, ~54 m with base and tips. The sim window reaches ~38 m over a standing
                      # player, so the top blocks fill once the player climbs or flies near (each block is its own box)
const VERSION := 2    # saved with the world: an older monolith is raised again (2: taller, both columns one height)

const M := GameMaterials.MONOLITH
const MR := GameMaterials.MONOLITH_RUNE
const MS := GameMaterials.MONOLITH_SEAM


## Centre of the base: 12 m north (-z, the way a new player looks) of spawn, on the pattern grid.
static func centre(spawn: Vector2i) -> Vector2i:
	return Vector2i(snappedi(spawn.x, P), snappedi(spawn.y - 12 * V, P))


static func boxes(world: World, spawn: Vector2i) -> Array:
	var out: Array = []
	var rng := RandomNumberGenerator.new()
	rng.seed = hash(spawn)
	var c := centre(spawn)
	var low := 1 << 30
	for o in [Vector2i(-1, -1), Vector2i(1, -1), Vector2i(-1, 1), Vector2i(1, 1), Vector2i.ZERO]:
		low = mini(low, world.surface_height(c.x + o.x * BASE_HALF, c.y + o.y * BASE_HALF))
	var y0 := snappedi(low, P)
	# foundation down past the lowest corner, then four 1 m steps
	out.append([Vector3i(c.x - BASE_HALF, mini(low, y0) - 2 * V, c.y - BASE_HALF), Vector3i(c.x + BASE_HALF, y0, c.y + BASE_HALF), M])
	for i in 4:
		var h := BASE_HALF - i * V
		out.append([Vector3i(c.x - h, y0 + i * V, c.y - h), Vector3i(c.x + h, y0 + (i + 1) * V, c.y + h), M])
	var top := y0 + 4 * V
	# two columns of 2 m blocks, each with a recessed joint row; every 4th block a rune band
	var z0 := c.y - COLUMN_W / 2
	var z1 := c.y + COLUMN_W / 2
	var tallest := 0
	for side in [-1, 1]:
		var x0: int = c.x - SEAM_W / 2 - COLUMN_W if side < 0 else c.x + SEAM_W / 2
		for b in BLOCKS:
			var by := top + b * P
			out.append([Vector3i(x0, by, z0), Vector3i(x0 + COLUMN_W, by + P - 1, z1), MR if b % 4 == 2 else M])
			out.append([Vector3i(x0 + 1, by + P - 1, z0 + 1), Vector3i(x0 + COLUMN_W - 1, by + P, z1 - 1), M])
		var ty := top + BLOCKS * P
		for k in 4:   # the pointed top
			var ins := 2 + k * 3
			out.append([Vector3i(x0 + ins, ty + k * 3, z0 + ins), Vector3i(x0 + COLUMN_W - ins, ty + (k + 1) * 3, z1 - ins), M])
		tallest = maxi(tallest, ty + 12)
	# the seam: set back 2 voxels from the faces, up to the tips; one box per block, so the lower ones fill
	# while the top still waits for its chunks
	for b in BLOCKS + 1:
		out.append([Vector3i(c.x - SEAM_W / 2, top + b * P, z0 + 2), Vector3i(c.x + SEAM_W / 2, mini(top + (b + 1) * P, tallest), z1 - 2), MS])
	# four rune pylons at the corners: a foundation, one rune band, a cap with a seam tip
	for o in [Vector2i(-1, -1), Vector2i(1, -1), Vector2i(-1, 1), Vector2i(1, 1)]:
		var px: int = c.x + o.x * 10 * V
		var pz: int = c.y + o.y * 10 * V
		var g := world.surface_height(px, pz)
		var py := snappedi(g, P)
		out.append([Vector3i(px - V / 2, g - V, pz - V / 2), Vector3i(px + V / 2, py, pz + V / 2), M])
		out.append([Vector3i(px - V / 2, py, pz - V / 2), Vector3i(px + V / 2, py + P, pz + V / 2), MR])
		out.append([Vector3i(px - V / 2 + 1, py + P, pz - V / 2 + 1), Vector3i(px + V / 2 - 1, py + P + 4, pz + V / 2 - 1), M])
		out.append([Vector3i(px - 1, py + P + 4, pz - 1), Vector3i(px + 1, py + P + 7, pz + 1), MS])
	_plaza(out, world, c, rng)
	return out


## Broken flagstones, 1 m each: dense by the steps, thinning out to the plaza's edge; some sit a voxel
## proud, some have chipped corners or a crack through them; rubble and a few fallen blocks lie on top.
static func _plaza(out: Array, world: World, c: Vector2i, rng: RandomNumberGenerator) -> void:
	var n := PLAZA_R / V
	for gz in range(-n, n):
		for gx in range(-n, n):
			var x := c.x + gx * V
			var z := c.y + gz * V
			if absi(x + V / 2 - c.x) < BASE_HALF and absi(z + V / 2 - c.y) < BASE_HALF:
				continue
			var r := Vector2(x + V / 2 - c.x, z + V / 2 - c.y).length() / PLAZA_R
			if r > 1.0 or rng.randf() > 0.95 - 0.7 * r:
				continue
			var h := world.surface_height(x + V / 2, z + V / 2)
			var top := h + rng.randi_range(1, 2)
			var a := Vector3i(x + rng.randi_range(0, 1), h - 3, z + rng.randi_range(0, 1))
			var b := Vector3i(x + V - rng.randi_range(0, 1), top, z + V - rng.randi_range(0, 1))
			if rng.randf() < 0.2:   # cracked in two, a voxel's gap between the halves
				var mid := rng.randi_range(a.x + 2, b.x - 3)
				out.append([a, Vector3i(mid, top, b.z), M])
				out.append([Vector3i(mid + 1, a.y, a.z), Vector3i(b.x, top - rng.randi_range(0, 1), b.z), M])
			else:
				out.append([a, b, M])
	for i in 60:   # rubble: chips and stones on the plaza
		var ang := rng.randf() * TAU
		var d := rng.randf_range(BASE_HALF + V, PLAZA_R + 2 * V)
		var x := c.x + int(cos(ang) * d)
		var z := c.y + int(sin(ang) * d)
		var s := Vector3i(rng.randi_range(2, 5), rng.randi_range(1, 4), rng.randi_range(2, 5))
		var y := world.surface_height(x, z) + 1
		out.append([Vector3i(x, y, z), Vector3i(x, y, z) + s, M])
	for i in 5:   # fallen blocks from the columns, half sunk at the foot of the steps
		var ang := rng.randf() * TAU
		var d := BASE_HALF + rng.randi_range(2, 6) * V
		var x := c.x + int(cos(ang) * d)
		var z := c.y + int(sin(ang) * d)
		var y := world.surface_height(x, z) - V / 2
		var s := Vector3i(2 * V, V + 4, V) if i % 2 == 0 else Vector3i(V, V + 4, 2 * V)
		out.append([Vector3i(x, y, z), Vector3i(x, y, z) + s, MR if i == 0 else M])


## Where the seam's light hangs: 3 m over the top step, 3 m out from the seam's face, so it washes
## the steps and the plaza (Main's pink OmniLight).
static func light_pos(world: World, spawn: Vector2i) -> Vector3:
	var c := centre(spawn)
	return Vector3(c.x, world.surface_height(c.x, c.y) + 7 * V, c.y + COLUMN_W / 2 + 3 * V)


## The foot of the sky beam (MonolithBeacon): the seam's tip, level with the columns' points.
static func beam_foot(world: World, spawn: Vector2i) -> Vector3:
	var c := centre(spawn)
	var low := 1 << 30
	for o in [Vector2i(-1, -1), Vector2i(1, -1), Vector2i(-1, 1), Vector2i(1, 1), Vector2i.ZERO]:
		low = mini(low, world.surface_height(c.x + o.x * BASE_HALF, c.y + o.y * BASE_HALF))
	return Vector3(c.x, snappedi(low, P) + 4 * V + BLOCKS * P + 12, c.y)
