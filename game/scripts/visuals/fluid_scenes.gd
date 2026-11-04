class_name FluidScenes
extends RefCounted

## The water scenes of ADR 0138 as data over a box callable, shared by the in-game film (scripts/visuals/fluid_film.gd)
## and the bare-kernel runner (tools/fluid_feel.gd). No autoloads here: the runner is a --script.

const W := 64    # patch voxels along x
const D := 48    # along z
const H := 28    # air above the ground
const DEEP := 16  # dirt under it
const ALL := ["pit_lake", "pour_pit", "cave", "dam", "slope", "channel", "trench", "fall"]
const SLICE_Z := 24
const SLICE_Y := Vector2i(-16, 16)


# --- the scenes: `box.call(a, b, material)` fills [a, b) relative to the patch corner; ground is y < 0 ---------------

static func build(name: String, box: Callable) -> void:
	match name:
		"pit_lake":
			box.call(Vector3i(4, -10, 8), Vector3i(28, 0, 40), GameMaterials.WATER_STATIC)   # a lake, surface at ground level
			box.call(Vector3i(32, -10, 16), Vector3i(46, 1, 32), GameMaterials.AIR)          # a pit dug beside it
		"pour_pit":
			box.call(Vector3i(24, -12, 16), Vector3i(40, 1, 32), GameMaterials.AIR)
			box.call(Vector3i(28, 6, 20), Vector3i(36, 14, 28), GameMaterials.WATER)   # 512 cells of water dropped in
		"cave":
			box.call(Vector3i(6, -16, 18), Vector3i(W, 0, 30), GameMaterials.STONE)       # rock: a dirt roof falls in
			box.call(Vector3i(8, -15, 20), Vector3i(W - 1, -7, 28), GameMaterials.AIR)   # a tunnel
			box.call(Vector3i(8, -7, 22), Vector3i(12, 1, 26), GameMaterials.AIR)        # a shaft down to it
			box.call(Vector3i(30, -16, 20), Vector3i(40, -15, 28), GameMaterials.AIR)    # a dip in its floor
		"dam":
			_rim(box, 8)
			box.call(Vector3i(2, 0, 2), Vector3i(20, 12, D - 2), GameMaterials.STONE)
			box.call(Vector3i(3, 0, 3), Vector3i(18, 12, D - 3), GameMaterials.WATER)   # a tank of moving water behind a wall
		"slope":
			for x in range(0, W):
				var top := 16 - x / 3   # a voxel down every three
				if x >= 48:
					top = -4            # a basin at the foot
				if top > 0:
					box.call(Vector3i(x, 0, 0), Vector3i(x + 1, top, D), GameMaterials.DIRT)
					box.call(Vector3i(x, top, 0), Vector3i(x + 1, top + 4, 2), GameMaterials.STONE)   # side walls
					box.call(Vector3i(x, top, D - 2), Vector3i(x + 1, top + 4, D), GameMaterials.STONE)
				else:
					box.call(Vector3i(x, top, 2), Vector3i(x + 1, 0, D - 2), GameMaterials.AIR)
			box.call(Vector3i(0, 16, 0), Vector3i(2, 26, D), GameMaterials.STONE)   # a back wall at the top
			box.call(Vector3i(2, 17, 16), Vector3i(10, 25, 32), GameMaterials.WATER)
		"fall":   # a lake on a cliff top; its lip is cut and it falls 3 m into a pool
			box.call(Vector3i(4, 0, 14), Vector3i(26, 22, 34), GameMaterials.STONE)
			box.call(Vector3i(6, 17, 16), Vector3i(22, 22, 32), GameMaterials.WATER_STATIC)   # surface row 21
			box.call(Vector3i(26, -6, 14), Vector3i(50, 0, 34), GameMaterials.AIR)
		"channel", "trench":
			box.call(Vector3i(4, -10, 8), Vector3i(28, 0, 40), GameMaterials.WATER_STATIC)
			if name == "trench":   # a shelving bank: only the surface row touches the cut (water_test's river)
				box.call(Vector3i(22, -10, 8), Vector3i(28, -1, 40), GameMaterials.DIRT)


## What the player does while it is filmed, `t` s in; `done` remembers the one-off steps.
static func act(name: String, t: float, done: Dictionary, box: Callable) -> void:
	match name:
		"pit_lake":
			if not done.has("dug") and t > 0.5:   # dig through the bank between them
				done["dug"] = true
				box.call(Vector3i(28, -10, 20), Vector3i(32, 1, 28), GameMaterials.AIR)
		"cave":
			if t > 0.5 and t < 3.5:   # a stream poured down the shaft
				box.call(Vector3i(9, -1, 23), Vector3i(11, 0, 25), GameMaterials.WATER)
		"fall":
			if not done.has("cut") and t > 0.5:
				done["cut"] = true
				box.call(Vector3i(22, 19, 22), Vector3i(26, 23, 26), GameMaterials.AIR)
		"dam":
			if not done.has("broke") and t > 0.5:
				done["broke"] = true
				box.call(Vector3i(18, 0, 3), Vector3i(20, 12, D - 3), GameMaterials.AIR)
		"trench":   # water_test's trench: dug at once from the bank, its floor three cells under the surface
			if not done.has("dug") and t > 0.5:
				done["dug"] = true
				box.call(Vector3i(28, -4, 21), Vector3i(40, 8, 28), GameMaterials.AIR)
		"channel":
			var step := int((t - 0.5) / 0.5)   # dug from the lake outward, a metre every half second
			if t > 0.5 and step < 4 and not done.has(step):
				done[step] = true
				box.call(Vector3i(28 + step * 8, -4, 22), Vector3i(36 + step * 8, 1, 26), GameMaterials.AIR)


## Stone walls round the patch, `h` voxels over the ground.
static func _rim(box: Callable, h: int) -> void:
	box.call(Vector3i(0, 0, 0), Vector3i(W, h, 2), GameMaterials.STONE)
	box.call(Vector3i(0, 0, D - 2), Vector3i(W, h, D), GameMaterials.STONE)
	box.call(Vector3i(0, 0, 0), Vector3i(2, h, D), GameMaterials.STONE)
	box.call(Vector3i(W - 2, 0, 0), Vector3i(W, h, D), GameMaterials.STONE)


## The camera [at, look] for a scene, relative to the patch corner.
static func camera(name: String) -> Array:
	match name:
		"pit_lake":
			return [Vector3(40, 8, 4), Vector3(33, -5, 24)]
		"channel":
			return [Vector3(46, 9, 8), Vector3(42, -3, 24)]
		"pour_pit":
			return [Vector3(32, 11, 5), Vector3(32, -7, 24)]
		"cave":   # inside the tunnel's far end, looking back at the shaft
			return [Vector3(34, -9, 24), Vector3(10, -11, 24)]
		"fall":   # side on, the whole drop in shot
			return [Vector3(40, 11, 4), Vector3(27, 10, 24)]
		"dam":
			return [Vector3(36, 16, -10), Vector3(36, 0, 24)]
	return [Vector3(W * 0.5, 26, -14), Vector3(W * 0.5, 6, 24)]


## One side slice through z = SLICE_Z as text rows, top first; `cell` (relative Vector3i) -> cell_info.
static func slice(cell: Callable) -> PackedStringArray:
	var out := PackedStringArray()
	for y in range(SLICE_Y.y, SLICE_Y.x - 1, -1):
		var line := "%4d " % y
		for x in W:
			var c: Dictionary = cell.call(Vector3i(x, y, SLICE_Z))
			var m: int = c.get("material", -1)
			if m == GameMaterials.WATER:
				line += str(int(c["eighths"])) if int(c["eighths"]) < 8 else "W"
			elif m == GameMaterials.WATER_STATIC:
				line += "S"
			elif m == 0:
				line += "."
			elif m < 0:
				line += "?"
			else:
				line += "#"
		out.append(line)
	return out


## A top view of the moving water's surface, one row per z: its height in eighths over the lowest column, as a digit
## (a-z past 9), "." where no moving water stands.
static func heights(cell: Callable) -> PackedStringArray:
	var h := {}
	var lo := 1 << 30
	for z in D:
		for x in W:
			for y in range(SLICE_Y.y, SLICE_Y.x - 1, -1):
				var c: Dictionary = cell.call(Vector3i(x, y, z))
				if c.get("material", -1) == GameMaterials.WATER:
					h[Vector2i(x, z)] = y * 8 + int(c["eighths"])
					lo = mini(lo, h[Vector2i(x, z)])
					break
	var out := PackedStringArray()
	for z in D:
		var line := "%4d " % z
		for x in W:
			var k := Vector2i(x, z)
			if not h.has(k):
				line += "."
			else:
				var d: int = h[k] - lo
				line += str(d) if d < 10 else (char(87 + d) if d < 36 else "+")
		out.append(line)
	return out
