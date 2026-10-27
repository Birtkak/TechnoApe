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


## A cell a placed station or a built firepit may take (ADR 0083): a free cell, or passable ground
## clutter (pebbles, flowers, grass) that the piece flattens, so a 1 m chest fits in a meadow.
static func is_clearable_cell(v: int) -> bool:
	if is_free_cell(v):
		return true
	var m := GameMaterials.material_of_voxel(v)
	if m <= 0 or m > GameMaterials.defs().size():
		return false
	var d: Dictionary = GameMaterials.defs()[m - 1]
	return (d.has("clutter") or d.has("micro")) and not d.has("interact") and not GameMaterials.blocks_bodies(m)


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
	# all four columns of the 2 x 2 carry a stalk, of a few heights, so a ripe field reads as a crop
	# (2026-10-25: "denser fields"); the anchor's column is the tallest
	var cols := [Vector3i(0, 0, 0), Vector3i(1, 0, 0), Vector3i(0, 0, 1), Vector3i(1, 0, 1)]
	var drop := [0, 1 + ((s >> 1) & 1), 1 + ((s >> 2) & 1), (s >> 3) & 1]
	var out := {}
	if kind == "wheat":
		var m := look
		var ear := GameMaterials.id_of("wheat_ear")
		var h: int = [1, 1, 2, 3, 4, 5, 6, 6][age]
		for i in 4:
			if age < 2 and i > 1:
				continue   # sprouts: two blades
			var ch: int = maxi(h - int(drop[i]), 1)
			var ears := 0 if age < 6 else (2 if age == 7 else 1)
			for y in ch:
				out[cols[i] + Vector3i(0, y, 0)] = ear if y >= ch - ears and y > 0 else m
		out[Vector3i.ZERO] = m   # the anchor keeps the age material
		return out
	# carrots: a tuft of feathery leaves over the 2 x 2; a ripe carrot shows its orange shoulders
	var leaf := GameMaterials.id_of("carrots_0")
	var stage: int = [0, 0, 1, 1, 2, 2, 2, 3][age]
	var h: int = [2, 3, 4, 4][stage]
	out[Vector3i.ZERO] = look
	for i in 4:
		var ch: int = maxi(h - int(drop[i]), 1)
		if stage == 0 and i > 1:
			continue
		for y in range(0 if i > 0 else 1, ch):
			out[cols[i] + Vector3i(0, y, 0)] = leaf
	if stage == 3:
		out[cols[3]] = look   # a second shoulder on the other diagonal
	return out


# ---------------- torches ----------------

## A torch: a stick of four, a cross of ember cloth on its top and a big flame on that (the "torch"
## material: tile 120, which glows and flickers). `wall` is the direction to the wall it hangs on (ZERO = standing on a floor):
## then the stick leans out from the wall, its foot against it. Offsets are from the placed voxel.
static func torch(wall: Vector3i) -> Dictionary:
	var stick := GameMaterials.id_of("torch_stick")
	var wrap := GameMaterials.id_of("torch_wrap")
	var flame := GameMaterials.id_of("torch")
	var out := {}
	var lean := -wall
	var top := Vector3i(0, 4, 0)
	if wall == Vector3i.ZERO:
		for y in 4:
			out[Vector3i(0, y, 0)] = stick
	else:
		out[Vector3i.ZERO] = stick
		out[Vector3i(0, 1, 0)] = stick
		out[lean + Vector3i(0, 2, 0)] = stick
		out[lean + Vector3i(0, 3, 0)] = stick
		top = lean + Vector3i(0, 4, 0)
	# a big burning head (2026-10-25: "bigger"): a cross of ember cloth, a 3 x 3 of flame on it, a cross
	# above that and a two-voxel tip
	var cross := [Vector3i.ZERO, Vector3i(1, 0, 0), Vector3i(-1, 0, 0), Vector3i(0, 0, 1), Vector3i(0, 0, -1)]
	for d in cross:
		out[top + d] = wrap
	for dz in range(-1, 2):
		for dx in range(-1, 2):
			out[top + Vector3i(dx, 1, dz)] = flame
	for d in cross:
		out[top + d + Vector3i(0, 2, 0)] = flame
	out[top + Vector3i(0, 3, 0)] = flame
	out[top + Vector3i(0, 4, 0)] = flame
	return out


## Offset of the flame (the light's place) in a torch stamp.
static func torch_flame(wall: Vector3i) -> Vector3i:
	return Vector3i(0, 6, 0) if wall == Vector3i.ZERO else -wall + Vector3i(0, 6, 0)
