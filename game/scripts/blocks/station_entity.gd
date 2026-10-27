class_name StationEntity
extends RefCounted

## A crafting table (ADR 0007, 0034). It holds no state; being a block entity
## gives it a footprint (BLOCK^3 voxels from its anchor) and a view, like the
## chests and furnaces that do hold inventories. The table item drops through
## mining.json, so contents() is empty.

signal changed

var kind := "station"
var pos: Vector3i


func _init(p_pos: Vector3i) -> void:
	pos = p_pos


func tick() -> bool:
	return false


func is_lit() -> bool:
	return false


func contents() -> Array:
	return []


func to_dict() -> Dictionary:
	return {"kind": kind, "pos": [pos.x, pos.y, pos.z]}


func load_from(_d: Dictionary) -> void:
	pass


func summary() -> Dictionary:
	return {"kind": kind, "pos": pos}


# ---------------- models (ADR 0053, 0076) ----------------
## The crafting table is a 1 m workbench (bug report 2026-10-25 02-44-58: "too small and ugly"): a
## footprint of SIZE voxels, drawn in 6.25 cm cells like the animals (MobModel.cell 2) with 8-shade
## patterned oak: four square legs, an apron, a thick plank top with a 3 x 3 inlaid grid, a saw and
## a hammer on it and a shelf below. The chest and the furnace are the same size and kind of model
## (ADR 0076: every placed station reads at one scale), keyed by material name.

const SIZE := Vector3i(8, 6, 8)   # voxels: 1 m x 0.75 m x 1 m
## footprint per entity kind, in voxels (kinds not listed keep BlockEntities.cube)
const SIZES := {"station": SIZE, "chest": Vector3i(8, 6, 8), "furnace": Vector3i(8, 8, 8)}
const CELLS := 2                  # cells per voxel
const CHEST_LID_Y := 4.5          # voxels: where the chest's lid sits on its body (its 12 cells less 3)
const SHADES := MobModel.SHADES
# paints (palette index + 1)
const OAK_LIGHT := 1
const OAK := 2
const OAK_DARK := 3
const GRID := 4
const IRON := 5
const HANDLE := 6
const STONE := 7
const STONE_DARK := 8
const SOOT := 9
const EMBER := 10
const STONE_LIGHT := 11
const STONE_WARM := 12
const IRON_DARK := 13
const BRASS := 14
const EMBER_DARK := 15
const ASH := 16
const CHEST_WOOD := 17
const CHEST_LIGHT := 18
const PALETTE := [[196, 150, 96], [160, 116, 68], [104, 70, 40], [62, 40, 22], [150, 154, 160], [120, 78, 40],
	[106, 106, 102], [52, 50, 48], [30, 26, 24], [255, 150, 40], [146, 144, 136], [122, 106, 88], [84, 88, 96],
	[216, 172, 72], [184, 62, 16], [104, 100, 96], [150, 98, 52], [190, 136, 78]]
const FLAT := [EMBER - 1, EMBER_DARK - 1]   # paint indices that keep one tone

static var _material: StandardMaterial3D
static var _tiles: PackedInt32Array
static var _meshes := {}
static var _ghost_material: StandardMaterial3D


static func material() -> StandardMaterial3D:
	if _material:
		return _material
	var cols := 8
	var rows := ceili(float(PALETTE.size() * SHADES) / cols)
	var img := Image.create(cols * 4, rows * 4, false, Image.FORMAT_RGBA8)
	for t in PALETTE.size():
		for k in SHADES:
			var i := t * SHADES + k
			img.fill_rect(Rect2i((i % cols) * 4, (i / cols) * 4, 4, 4), WorkPalette.tone_color(PALETTE[t], k))
	_material = StandardMaterial3D.new()
	_material.albedo_texture = ImageTexture.create_from_image(img)
	_material.texture_filter = BaseMaterial3D.TEXTURE_FILTER_NEAREST
	_material.roughness = 0.9
	_tiles = PackedInt32Array()
	_tiles.resize(PALETTE.size() * SHADES + 1)
	_tiles[0] = -1
	for i in PALETTE.size() * SHADES:
		_tiles[i + 1] = i
	return _material


## Material names drawn by these models. The placed model, the held and dropped mesh (item_mesh) and the
## inventory icon (icon) all come from one cell grid (cells), so they cannot drift apart (ADR 0084).
const MODELS := ["crafting_table", "chest", "furnace", "furnace_lit"]
static var _cells := {}
static var _icons := {}


## A station's shaded cells and their grid size, by material name: [PackedInt32Array, Vector3i];
## chest_body and chest_lid are the chest's two parts for its lid animation (the lid is the top 3 cells).
static func cells(name: String) -> Array:
	if _cells.has(name):
		return _cells[name]
	var size: Vector3i = SIZES.get({"crafting_table": "station", "furnace_lit": "furnace", "chest_body": "chest", "chest_lid": "chest"}.get(name, name), SIZE) * CELLS
	var c := PackedInt32Array()
	c.resize(size.x * size.y * size.z)
	var box := func(from: Vector3i, to: Vector3i, paint: int) -> void:
		for z in range(from.z, to.z):
			for y in range(from.y, to.y):
				for x in range(from.x, to.x):
					c[x + size.x * (y + size.y * z)] = paint
	match name:
		"chest", "chest_body", "chest_lid":
			_chest(box, size)
			if name == "chest_body":
				box.call(Vector3i(0, size.y - 3, 0), size, 0)
			elif name == "chest_lid":
				box.call(Vector3i.ZERO, Vector3i(size.x, size.y - 3, size.z), 0)
		"furnace", "furnace_lit":
			_furnace(box, size, name == "furnace_lit")
		_:
			_table(box, c, size)
	_cells[name] = [WorkPalette.shade_cells(c, size, FLAT), size]
	return _cells[name]


## A station's mesh in voxel units, its min corner at the origin (the entity's anchor), by material
## name: crafting_table, chest, furnace or furnace_lit.
static func model_mesh(name: String = "crafting_table") -> ArrayMesh:
	if not _meshes.has(name):
		_meshes[name] = _build(name, CELLS, Vector3.ZERO)
	return _meshes[name]


## The same model 1 unit across its widest side, centred on the origin like an item sprite: the
## crafting table, chest and furnace in the hand and dropped on the ground (BlockMeshes.cube).
static func item_mesh(name: String) -> ArrayMesh:
	var key := name + "@item"
	if not _meshes.has(key):
		var size: Vector3i = cells(name)[1]
		var g := maxi(size.x, maxi(size.y, size.z))
		_meshes[key] = _build(name, g, -Vector3(size) * 0.5 / g)
	return _meshes[key]


static func _build(name: String, per_unit: int, origin: Vector3) -> ArrayMesh:
	material()
	var cs := cells(name)
	var mesh: ArrayMesh = MobModel.outward(MicroMesher.build(cs[0], cs[1], per_unit, _tiles, Vector2i(8, ceili(float(PALETTE.size() * SHADES) / 8)), origin))
	mesh.surface_set_material(0, _material)
	return mesh


const ICON_PX := 64
const ICON_FACE := [0.6, 1.0, 0.8]   # x, y, z faces: shaded like ItemIcon's block cubes (top lit, left 0.8, right 0.6)


## The inventory icon: the model's cells seen from the corner ItemIcon draws block cubes from (top,
## +z face on the left, +x face on the right), ray-cast pixel by pixel; null for other materials.
static func icon(name: String) -> ImageTexture:
	if not name in MODELS:
		return null
	if _icons.has(name):
		return _icons[name]
	var cs := cells(name)
	var c: PackedInt32Array = cs[0]
	var size: Vector3i = cs[1]
	var f := -Vector3(1.0, sqrt(2.0 / 3.0) * 1.0, 1.0).normalized()   # looking down about 30 degrees
	var right := f.cross(Vector3.UP).normalized()
	var up := right.cross(f).normalized()
	var mid := Vector3(size) * 0.5
	var ext := 0.0   # the box's widest projected half extent
	for k in 8:
		var corner := Vector3(size.x * (k & 1), size.y * (k >> 1 & 1), size.z * (k >> 2 & 1)) - mid
		ext = maxf(ext, maxf(absf(corner.dot(right)), absf(corner.dot(up))))
	var px := ext * 2.0 / (ICON_PX - 2)
	var img := Image.create(ICON_PX, ICON_PX, false, Image.FORMAT_RGBA8)
	var far := Vector3(size).length()
	for v in ICON_PX:
		for u in ICON_PX:
			var o := mid + right * (u + 0.5 - ICON_PX * 0.5) * px - up * (v + 0.5 - ICON_PX * 0.5) * px - f * far
			var hit := _cast(c, size, o, f)
			if hit[0] > 0:
				var t: int = hit[0] - 1
				img.set_pixel(u, v, WorkPalette.tone_color(PALETTE[t / SHADES], t % SHADES) * ICON_FACE[hit[1]])
	_icons[name] = ImageTexture.create_from_image(img)
	return _icons[name]


## First filled cell along the ray o + d * t through the grid: [cell value, axis of the face it entered by].
static func _cast(c: PackedInt32Array, size: Vector3i, o: Vector3, d: Vector3) -> Array:
	var t0 := 0.0
	var t1 := INF
	var axis := 0   # the face the ray came in by: the box wall it crossed last
	for a in 3:   # clip the ray to the grid's box (d has no zero component)
		var lo := -o[a] / d[a]
		var hi := (size[a] - o[a]) / d[a]
		if minf(lo, hi) > t0:
			t0 = minf(lo, hi)
			axis = a
		t1 = minf(t1, maxf(lo, hi))
	if t0 > t1:
		return [0, 0]
	var p := o + d * (t0 + 1e-4)
	var cell := Vector3i(clampi(int(p.x), 0, size.x - 1), clampi(int(p.y), 0, size.y - 1), clampi(int(p.z), 0, size.z - 1))
	var step := Vector3i(int(signf(d.x)), int(signf(d.y)), int(signf(d.z)))
	var next := Vector3.ZERO   # t at which the ray crosses the next cell wall, per axis
	var delta := Vector3.ZERO
	for a in 3:
		delta[a] = absf(1.0 / d[a])
		next[a] = (cell[a] + (1 if step[a] > 0 else 0) - o[a]) / d[a]
	while true:
		var v := c[cell.x + size.x * (cell.y + size.y * cell.z)]
		if v > 0:
			return [v, axis]
		axis = 0 if next.x < next.y and next.x < next.z else (1 if next.y < next.z else 2)
		cell[axis] += step[axis]
		if cell[axis] < 0 or cell[axis] >= size[axis]:
			return [0, 0]
		next[axis] += delta[axis]
	return [0, 0]


## The see-through copy of a station's model the placement ghost draws (bug report 2026-10-27_15-31-39);
## its material is shared, so tinting it (free or blocked) changes no pipeline.
static func ghost_mesh(name: String) -> ArrayMesh:
	var key := "ghost " + name
	if not _meshes.has(key):
		if _ghost_material == null:
			_ghost_material = material().duplicate()
			_ghost_material.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
			_ghost_material.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
			_ghost_material.albedo_color = Color(0.25, 0.6, 2.4, 0.7)
		var m: ArrayMesh = model_mesh(name).duplicate()
		m.surface_set_material(0, _ghost_material)
		_meshes[key] = m
	return _meshes[key]


static func ghost_tint(blocked: bool) -> void:
	if _ghost_material:
		_ghost_material.albedo_color = Color(1.0, 0.35, 0.3, 0.5) if blocked else Color(0.25, 0.6, 2.4, 0.7)


## A chest: a dark oak box of three plank courses with a lighter plank lid that overhangs, two
## dark iron bands with rivets, and a brass lock plate at the front.
static func _chest(box: Callable, size: Vector3i) -> void:
	var lid := size.y - 3
	box.call(Vector3i(1, 0, 2), Vector3i(size.x - 1, lid, size.z - 2), CHEST_WOOD)
	for y in range(2, lid, 3):   # plank seams round the body
		box.call(Vector3i(1, y, 2), Vector3i(size.x - 1, y + 1, size.z - 2), OAK_DARK)
	box.call(Vector3i(0, lid, 1), Vector3i(size.x, size.y, size.z - 1), CHEST_LIGHT)   # the lid overhangs a cell
	box.call(Vector3i(0, lid, 1), Vector3i(size.x, lid + 1, size.z - 1), OAK_DARK)      # its underside rim
	for z in range(3, size.z - 1, 4):   # lid planks run across, a dark joint between them
		box.call(Vector3i(0, size.y - 1, z), Vector3i(size.x, size.y, z + 1), OAK_DARK)
	for x in [3, size.x - 5]:
		box.call(Vector3i(x, 0, 1), Vector3i(x + 2, size.y, size.z - 1), IRON_DARK)
		for y in [1, lid - 2, size.y - 1]:   # rivets
			box.call(Vector3i(x, y, 1), Vector3i(x + 2, y + 1, 2), IRON)
			box.call(Vector3i(x, y, size.z - 2), Vector3i(x + 2, y + 1, size.z - 1), IRON)
	var m := size.x / 2
	for z in [1, size.z - 2]:   # a lock plate on both long sides: chests have no facing
		box.call(Vector3i(m - 2, lid - 3, z), Vector3i(m + 2, lid + 2, z + 1), BRASS)
		box.call(Vector3i(m - 1, lid - 2, z), Vector3i(m + 1, lid, z + 1), SOOT)   # keyhole


## A furnace: courses of fieldstone in three tones with dark mortar, an arched mouth on every side
## (stations have no facing) with soot above it and ash on its floor, a lighter cap and a chimney
## stack; the fire glows when lit.
static func _furnace(box: Callable, size: Vector3i, lit: bool) -> void:
	var body := size.y - 2
	var tones := [STONE, STONE_LIGHT, STONE_WARM]
	box.call(Vector3i(1, 0, 1), Vector3i(size.x - 1, body, size.z - 1), STONE_DARK)   # mortar
	for course in range(0, body, 4):
		var off := (course / 4 % 2) * 3
		for j in range(-off, size.x, 5):   # blocks seen from the front and back
			var t: int = tones[hash(Vector3i(j, course, 0)) % 3]
			box.call(Vector3i(maxi(j, 1), course, 1), Vector3i(mini(j + 4, size.x - 1), course + 3, size.z - 1), t)
		for j in range(-off, size.z, 5):   # joints and tones seen from the sides
			var t: int = tones[hash(Vector3i(j, course, 1)) % 3]
			box.call(Vector3i(1, course, maxi(j, 1)), Vector3i(2, course + 3, mini(j + 4, size.z - 1)), t)
			box.call(Vector3i(size.x - 2, course, maxi(j, 1)), Vector3i(size.x - 1, course + 3, mini(j + 4, size.z - 1)), t)
	box.call(Vector3i(0, body - 1, 0), Vector3i(size.x, body + 1, size.z), STONE_LIGHT)   # the cap, a cell wider
	var m := size.x / 2
	box.call(Vector3i(m - 2, body, m - 2), Vector3i(m + 2, size.y, m + 2), STONE)         # chimney stack
	box.call(Vector3i(m - 1, body, m - 1), Vector3i(m + 1, size.y, m + 1), SOOT)
	var inner := EMBER if lit else SOOT
	var deep := EMBER_DARK if lit else STONE_DARK
	for side in 4:
		var along := Vector3i(1, 0, 0) if side < 2 else Vector3i(0, 0, 1)   # the face runs along this axis
		var into := Vector3i(0, 0, 1) if side < 2 else Vector3i(1, 0, 0)
		var far := side % 2 == 1
		var mouth := func(a: int, y0: int, y1: int, d0: int, d1: int, paint: int) -> void:
			# cells a..a+w along the face, depth d0..d1 in from the face, rows y0..y1
			var w := m + 3 - a
			var lo := along * a + Vector3i(0, y0, 0) + into * (size.x - d1 if far else d0)
			var hi := along * (a + w) + Vector3i(0, y1, 0) + into * (size.x - d0 if far else d1)
			box.call(lo, hi, paint)
		mouth.call(m - 3, 1, 6, 0, 4, 0)        # the opening
		mouth.call(m - 2, 6, 7, 0, 4, 0)        # the arch
		mouth.call(m - 3, 1, 2, 1, 4, ASH)      # ash on the floor
		mouth.call(m - 3, 2, 7, 4, 5, deep)     # the back wall
		mouth.call(m - 2, 2, 5, 4, 5, inner)    # the fire
		mouth.call(m - 2, 7, 9, 0, 1, SOOT)     # soot above the arch


static func _table(box: Callable, c: PackedInt32Array, size: Vector3i) -> void:
	var top := size.y - 2   # the top is two cells thick
	for lx in [1, size.x - 3]:
		for lz in [1, size.z - 3]:
			box.call(Vector3i(lx, 0, lz), Vector3i(lx + 2, top, lz + 2), OAK_DARK)
	# apron under the top, a shelf low between the legs
	box.call(Vector3i(1, top - 2, 1), Vector3i(size.x - 1, top, 3), OAK)
	box.call(Vector3i(1, top - 2, size.z - 3), Vector3i(size.x - 1, top, size.z - 1), OAK)
	box.call(Vector3i(1, top - 2, 1), Vector3i(3, top, size.z - 1), OAK)
	box.call(Vector3i(size.x - 3, top - 2, 1), Vector3i(size.x - 1, top, size.z - 1), OAK)
	box.call(Vector3i(2, 2, 2), Vector3i(size.x - 2, 3, size.z - 2), OAK)
	# planks of the top, every fourth row darker at the joints
	for z in size.z:
		box.call(Vector3i(0, top, z), Vector3i(size.x, size.y, z + 1), OAK_DARK if z % 4 == 3 else OAK_LIGHT)
	# the 3 x 3 grid inlaid in the top's middle: dark lines between 3-cell squares
	var g0 := Vector3i(3, size.y - 1, 3)
	for i in 4:
		box.call(g0 + Vector3i(i * 3, 0, 0), g0 + Vector3i(i * 3 + 1, 1, 10), GRID)
		box.call(g0 + Vector3i(0, 0, i * 3), g0 + Vector3i(10, 1, i * 3 + 1), GRID)
	# a saw on the right of the grid and a hammer at the front (they rise a cell above the top)
	var tool := func(from: Vector3i, to: Vector3i, paint: int) -> void:
		for z in range(from.z, to.z):
			for x in range(from.x, to.x):
				_put_tool(c, size, Vector3i(x, size.y - 1, z), paint)
	tool.call(Vector3i(13, 0, 3), Vector3i(15, 0, 10), IRON)
	tool.call(Vector3i(13, 0, 10), Vector3i(15, 0, 13), HANDLE)
	tool.call(Vector3i(4, 0, 14), Vector3i(10, 0, 15), HANDLE)
	tool.call(Vector3i(9, 0, 13), Vector3i(11, 0, 16), IRON)


## Tools lie on the top: they replace its top cell (the model keeps to its footprint).
static func _put_tool(c: PackedInt32Array, size: Vector3i, p: Vector3i, paint: int) -> void:
	c[p.x + size.x * (p.y + size.y * p.z)] = paint
