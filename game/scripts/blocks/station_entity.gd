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


# ---------------- model (ADR 0053) ----------------
## The crafting table is a 1 m workbench (bug report 2026-10-25 02-44-58: "too small and ugly"): a
## footprint of SIZE voxels, drawn in 6.25 cm cells like the animals (MobModel.cell 2) with 8-shade
## patterned oak: four square legs, an apron, a thick plank top with a 3 x 3 inlaid grid, a saw and
## a hammer on it and a shelf below.

const SIZE := Vector3i(8, 6, 8)   # voxels: 1 m x 0.75 m x 1 m
const CELLS := 2                  # cells per voxel
const SHADES := 8
# paints (palette index + 1)
const OAK_LIGHT := 1
const OAK := 2
const OAK_DARK := 3
const GRID := 4
const IRON := 5
const HANDLE := 6
const PALETTE := [[196, 150, 96], [160, 116, 68], [104, 70, 40], [62, 40, 22], [150, 154, 160], [120, 78, 40]]

static var _material: StandardMaterial3D
static var _tiles: PackedInt32Array
static var _mesh: ArrayMesh


static func material() -> StandardMaterial3D:
	if _material:
		return _material
	var cols := 8
	var rows := ceili(float(PALETTE.size() * SHADES) / cols)
	var img := Image.create(cols * 4, rows * 4, false, Image.FORMAT_RGBA8)
	for t in PALETTE.size():
		for k in SHADES:
			var i := t * SHADES + k
			img.fill_rect(Rect2i((i % cols) * 4, (i / cols) * 4, 4, 4), MobModel.shade_color(PALETTE[t], k))
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


## The table's mesh in voxel units, its min corner at the origin (the entity's anchor).
static func model_mesh() -> ArrayMesh:
	if _mesh:
		return _mesh
	material()
	var size := SIZE * CELLS
	var c := PackedInt32Array()
	c.resize(size.x * size.y * size.z)
	var box := func(from: Vector3i, to: Vector3i, paint: int) -> void:
		for z in range(from.z, to.z):
			for y in range(from.y, to.y):
				for x in range(from.x, to.x):
					c[x + size.x * (y + size.y * z)] = paint
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
	_mesh = MobModel.outward(MicroMesher.build(_shade(c, size), size, CELLS, _tiles, Vector2i(8, ceili(float(PALETTE.size() * SHADES) / 8)), Vector3.ZERO))
	_mesh.surface_set_material(0, _material)
	return _mesh


## Tools lie on the top: they replace its top cell (the model keeps to its footprint).
static func _put_tool(c: PackedInt32Array, size: Vector3i, p: Vector3i, paint: int) -> void:
	c[p.x + size.x * (p.y + size.y * p.z)] = paint


## 8-shade patterns: a hash per cell, grain along x for the wood, lighter towards the top.
static func _shade(c: PackedInt32Array, size: Vector3i) -> PackedInt32Array:
	var out := PackedInt32Array()
	out.resize(c.size())
	for z in size.z:
		for y in size.y:
			for x in size.x:
				var i := x + size.x * (y + size.y * z)
				if c[i] <= 0:
					continue
				var base := c[i] - 1
				var grain := float(hash(Vector3i(x / 3, y, z)) % 1000) / 999.0
				var n := float(hash(Vector3i(x, y, z) * 7 + Vector3i(1, 2, 3)) % 1000) / 999.0
				var k := clampi(int(round(1.0 + grain * 3.6 + n * 2.0 + float(y) / size.y * 1.4)), 0, SHADES - 1)
				out[i] = base * SHADES + k + 1
	return out
