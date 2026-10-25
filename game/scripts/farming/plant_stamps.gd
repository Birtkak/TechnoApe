class_name PlantStamps
extends RefCounted

## Multi-voxel shapes of placed things (ADR 0053): flowers, mushrooms and seashells the player puts
## down (the pieces FineGen scatters, ADR 0047: FineGen::clutter_voxel, ported), crops at every age,
## and torches on floors and walls. A stamp is {Vector3i offset: material id} from the voxel the
## thing was placed in; its variety comes from a hash of that voxel, so a stamp can be rebuilt from
## the position alone (crops after a reload, a torch's cells). Callers write a stamp's cells only into
## free cells (is_free_cell()).

const FLOWER_HEADS := ["flower", "flower_red", "cornflower", "oxeye_daisy", "allium"]
const MUSHROOMS := ["mushroom_brown", "mushroom_red"]


static func seed_of(p: Vector3i) -> int:
	return hash(Vector3i(p.x * 7 + 3, p.y, p.z * 13 + 1)) & 0x7fffffff


## A cell a stamp may take: air or a replaceable plant (tall grass, ferns).
static func is_free_cell(m: int) -> bool:
	if m == GameMaterials.AIR:
		return true
	return m > 0 and m <= GameMaterials.defs().size() and GameMaterials.defs()[m - 1].get("replaceable", false)


## The piece a placed clutter material grows into, or {} when it is not one (kelp and seagrass stay a voxel).
static func for_placed(material: int, p: Vector3i) -> Dictionary:
	var name := GameMaterials.name_of(material)
	var s := seed_of(p)
	if name in FLOWER_HEADS:
		return _flower(material, s)
	if name in MUSHROOMS:
		return _mushroom(material, s)
	if name == "seashell":
		return _shell(material, s)
	return {}


# ---------------- clutter (FineGen::clutter_voxel) ----------------

static func _flower(head: int, s: int) -> Dictionary:
	var out := {}
	var stem := GameMaterials.id_of("plant_stem")
	var ball := GameMaterials.name_of(head) == "allium"
	var h := 6 if ball else 4 + ((s >> 3) & 1)
	for y in h:
		out[Vector3i(0, y, 0)] = stem
	var lx := 1 if s & 1 else 0
	var lz := 1 - lx
	out[Vector3i(lx, 1, lz)] = stem
	out[Vector3i(-lx, 2, -lz)] = stem
	for dz in range(-2, 3):
		for dx in range(-2, 3):
			var ax := absi(dx)
			var az := absi(dz)
			if ball:
				for dy in range(-1, 2):
					if ax <= 1 and az <= 1 and ax + az + absi(dy) < 3:
						out[Vector3i(dx, h + 1 + dy, dz)] = head
				continue
			if (ax <= 1 and az <= 1) or (ax + az == 2 and (ax == 0 or az == 0)):
				out[Vector3i(dx, h, dz)] = head
			if ax <= 1 and az <= 1:
				out[Vector3i(dx, h + 1, dz)] = head
	return out


static func _mushroom(cap: int, s: int) -> Dictionary:
	var out := {}
	var stem := GameMaterials.id_of("mushroom_stem")
	if s % 3 == 1 and (s >> 6) & 1:
		# tall and thin: a 4-5 high stem under a small domed cap
		var t := 4 + ((s >> 7) & 1)
		for y in t:
			out[Vector3i(0, y, 0)] = stem
		for d in [Vector3i(0, 0, 0), Vector3i(1, 0, 0), Vector3i(-1, 0, 0), Vector3i(0, 0, 1), Vector3i(0, 0, -1)]:
			out[d + Vector3i(0, t, 0)] = cap
		if (s >> 8) & 1:
			for d in [Vector3i(1, 0, 1), Vector3i(-1, 0, 1), Vector3i(1, 0, -1), Vector3i(-1, 0, -1)]:
				out[d + Vector3i(0, t, 0)] = cap
		out[Vector3i(0, t + 1, 0)] = cap
		return out
	var big := s % 3 == 0
	if big:
		var h := 3 + ((s >> 2) & 1)
		for z in 2:
			for x in 2:
				for y in h:
					out[Vector3i(x, y, z)] = stem
				out[Vector3i(x, h + 1, z)] = cap
		for z in range(-1, 3):
			for x in range(-1, 3):
				if not (absi(x * 2 - 1) == 3 and absi(z * 2 - 1) == 3):
					out[Vector3i(x, h, z)] = cap
		return out
	out[Vector3i(0, 0, 0)] = stem
	out[Vector3i(0, 1, 0)] = stem
	for z in range(-1, 2):
		for x in range(-1, 2):
			out[Vector3i(x, 2, z)] = cap
			if absi(x) + absi(z) <= 1:
				out[Vector3i(x, 3, z)] = cap
	return out


static func _shell(shell: int, s: int) -> Dictionary:
	var out := {}
	var turn := s & 1
	var put := func(a: int, dy: int, b: int) -> void:
		out[Vector3i(b, dy, a) if turn else Vector3i(a, dy, b)] = shell
	match (s >> 3) % 3:
		0:   # a ribbed fan, 3 across, 2 along, ridged
			for a in range(-1, 2):
				for b in 2:
					put.call(a, 0, b)
			for b in 2:
				put.call(0, 1, b)
		1:   # a clam: a 4x3 dome, 2 high
			for a in range(-2, 2):
				for b in range(-1, 2):
					if not (absi(b) == 1 and (a == -2 or a == 1)):
						put.call(a, 0, b)
			for a in range(-1, 1):
				for b in range(-1, 2):
					if not (absi(b) == 1 and (s >> 5) & 1):
						put.call(a, 1, b)
		_:   # a snail shell: a whorl climbing to a point
			for a in range(-1, 2):
				for b in 2:
					put.call(a, 0, b)
			for a in range(-1, 1):
				for b in 2:
					put.call(a, 1, b)
			put.call(-1, 2, 1)
	return out


# ---------------- crops ----------------

## Crops stand one plant per 2 x 2 voxels (four to a 0.5 m block, like Minecraft's crop textures) and
## keep to their own 2 x 2 columns, so a field never fuses into a slab. The anchor (offset 0) holds the
## age material Farming checks; leaves and stalks are the same material (or the ear of ripe wheat).
const CROP_GRID := 2


## The anchor a plant on the farmland at `soil` has: the voxel above, snapped to the crop grid.
static func crop_anchor(soil: Vector3i) -> Vector3i:
	return Vector3i(floori(soil.x / float(CROP_GRID)) * CROP_GRID, soil.y + 1, floori(soil.z / float(CROP_GRID)) * CROP_GRID)


static func crop(kind: String, age: int, look: int, p: Vector3i) -> Dictionary:
	var s := seed_of(p)
	# one stalk at the anchor, a second on the other diagonal (or beside it), leaves in between
	var d2 := Vector3i(1, 0, 1) if s & 1 else Vector3i(1, 0, 0)
	var side_a := Vector3i(1, 0, 0) if d2 != Vector3i(1, 0, 0) else Vector3i(0, 0, 1)
	var side_b := Vector3i(0, 0, 1) if d2 != Vector3i(1, 0, 0) else Vector3i(1, 0, 1)
	var out := {}
	if kind == "wheat":
		var m := look
		var h: int = [1, 1, 2, 3, 4, 5, 6, 6][age]
		var h2: int = maxi(h - 1 - ((s >> 2) & 1), 0)
		for y in h:
			out[Vector3i(0, y, 0)] = m
		for y in h2:
			out[d2 + Vector3i(0, y, 0)] = m
		if age >= 2:
			out[side_a] = m   # blades at the foot
		if age >= 4 and age < 7:
			out[side_b + Vector3i(0, 1, 0)] = m
		if age >= 6:
			# ears: the top of each stalk (two voxels when ripe)
			var ear := GameMaterials.id_of("wheat_ear")
			var n := 2 if age == 7 else 1
			for k in n:
				out[Vector3i(0, h - 1 - k, 0)] = ear if h - 1 - k > 0 else m
				if h2 - 1 - k > 0:
					out[d2 + Vector3i(0, h2 - 1 - k, 0)] = ear
		return out
	# carrots: feathery leaves rising from the anchor; a ripe carrot shows its orange shoulder
	var leaf := GameMaterials.id_of("carrots_0")
	out[Vector3i.ZERO] = look
	var stage: int = [0, 0, 1, 1, 2, 2, 2, 3][age]
	var h: int = [2, 3, 4, 4][stage]
	for y in range(1, h):
		out[Vector3i(0, y, 0)] = leaf
	if stage >= 1:
		out[side_a + Vector3i(0, h - 2, 0)] = leaf
		out[d2 + Vector3i(0, h - 1, 0)] = leaf
	if stage >= 2:
		out[side_b + Vector3i(0, h - 1, 0)] = leaf
		out[side_a + Vector3i(0, h - 1, 0)] = leaf
		out[Vector3i(0, h, 0)] = leaf
	if stage == 3:
		out[d2] = look   # a second shoulder beside the first
	return out


# ---------------- torches ----------------

## A torch: a stick of three, a wrap of ember cloth around its top and the flame (the anchor material,
## "torch") above it. `wall` is the direction to the wall it hangs on (ZERO = standing on a floor):
## then the stick leans out from the wall, its foot against it. Offsets are from the placed voxel.
static func torch(wall: Vector3i) -> Dictionary:
	var stick := GameMaterials.id_of("torch_stick")
	var wrap := GameMaterials.id_of("torch_wrap")
	var flame := GameMaterials.id_of("torch")
	var out := {}
	var lean := -wall
	var top := Vector3i(0, 3, 0)
	if wall == Vector3i.ZERO:
		for y in 3:
			out[Vector3i(0, y, 0)] = stick
	else:
		out[Vector3i.ZERO] = stick
		out[Vector3i(0, 1, 0)] = stick
		out[lean + Vector3i(0, 2, 0)] = stick
		top = lean + Vector3i(0, 3, 0)
	out[top] = wrap
	for d in [Vector3i(1, 0, 0), Vector3i(-1, 0, 0), Vector3i(0, 0, 1), Vector3i(0, 0, -1)]:
		if d != wall:
			out[top + d] = wrap
	out[top + Vector3i(0, 1, 0)] = flame
	out[top + Vector3i(0, 2, 0)] = flame
	return out


## Offset of the flame (the light's place) in a torch stamp.
static func torch_flame(wall: Vector3i) -> Vector3i:
	return Vector3i(0, 4, 0) if wall == Vector3i.ZERO else -wall + Vector3i(0, 4, 0)
