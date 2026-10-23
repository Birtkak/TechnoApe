class_name WorkPalette
extends RefCounted

## Colours and meshes of the Vintage Story work pieces (ADR 0023): knapping
## slabs, clay forms, ground storage, pit kilns, firepits and sieves. Every
## piece is a grid of 6.25 cm cells (the ADR 0006 chisel grid) meshed by the
## native MicroMesher with a small palette atlas of its own, like furniture
## (ADR 0022). A view node sits on the voxel's corner; ghost cells use a
## see-through copy of the material; guide plates (the clay form's next layer:
## green = fill, red = leave empty, with grid lines between the cells) are
## thin opaque tiles meshed on a finer grid.

const PALETTE: Array = [
	[92, 90, 98],      # FLINT
	[150, 44, 38],     # FLINT_MARK: cells still to chip off (red: take away)
	[150, 156, 168],   # CLAY (blue-grey raw clay)
	[190, 104, 62],    # FIRED (terracotta)
	[206, 118, 64],    # COPPER
	[255, 168, 52],    # MOLTEN
	[214, 192, 112],   # STRAW (dry grass)
	[140, 100, 56],    # WOOD
	[96, 68, 38],      # WOOD_DARK
	[128, 128, 124],   # STONE
	[255, 96, 24],     # EMBER
	[70, 66, 62],      # ASH
	[200, 180, 140],   # MESH (sieve weave)
	[150, 140, 130],   # GRAVEL
	[112, 78, 50],     # HIDE: raw, hair side down (ADR 0030)
	[214, 196, 164],   # SCRAPED: the flesh side scraped clean
	[150, 96, 52],     # LEATHER
	[96, 70, 40],      # FIREWOOD: split log ends
	[150, 112, 70],    # FIREWOOD_LIGHT: split faces
	[30, 26, 24],      # CHARCOAL
	[120, 86, 60],     # SOIL (pit cover in the view, unused by the world)
	[86, 196, 82],     # PLACE: the clay form's guide, cells to fill (green)
	[200, 62, 52],     # AVOID: the clay form's guide, cells that stay empty (red)
]
const FLINT := 0
const FLINT_MARK := 1
const CLAY := 2
const FIRED := 3
const COPPER := 4
const MOLTEN := 5
const STRAW := 6
const WOOD := 7
const WOOD_DARK := 8
const STONE := 9
const EMBER := 10
const ASH := 11
const MESH := 12
const GRAVEL := 13
const HIDE := 14
const SCRAPED := 15
const LEATHER := 16
const FIREWOOD := 17
const FIREWOOD_LIGHT := 18
const CHARCOAL := 19
const SOIL := 20
const PLACE := 21
const AVOID := 22
const GUIDE_GRID := 32   # guide plates are meshed 4x finer: 3x3 coloured, a one-cell gap as grid lines
const ATLAS := Vector2i(23, 1)

static var _material: StandardMaterial3D
static var _ghost: StandardMaterial3D
static var _tiles: PackedInt32Array


static func material() -> StandardMaterial3D:
	if _material:
		return _material
	var img := Image.create(ATLAS.x * 16, ATLAS.y * 16, false, Image.FORMAT_RGBA8)
	var rng := RandomNumberGenerator.new()
	rng.seed = 2323
	for t in PALETTE.size():
		var base: Array = PALETTE[t]
		var spread := 3 if t == MOLTEN or t == EMBER else 8
		for y in 16:
			for x in 16:
				var k := rng.randi_range(-spread, spread)
				img.set_pixel(t * 16 + x, y, Color8(clampi(base[0] + k, 0, 255), clampi(base[1] + k, 0, 255), clampi(base[2] + k, 0, 255)))
	var tex := ImageTexture.create_from_image(img)
	_material = StandardMaterial3D.new()
	_material.albedo_texture = tex
	_material.texture_filter = BaseMaterial3D.TEXTURE_FILTER_NEAREST_WITH_MIPMAPS
	_material.roughness = 0.9
	_ghost = StandardMaterial3D.new()
	_ghost.albedo_texture = tex
	_ghost.albedo_color = Color(1, 1, 1, 0.35)
	_ghost.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
	_ghost.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	_ghost.texture_filter = BaseMaterial3D.TEXTURE_FILTER_NEAREST
	_tiles = PackedInt32Array()
	_tiles.resize(PALETTE.size() + 1)
	_tiles[0] = -1
	for i in PALETTE.size():
		_tiles[i + 1] = i
	return _material


static func ghost_material() -> StandardMaterial3D:
	material()
	return _ghost


## cells: paint + 1 per cell (0 = empty), index x + size.x * (y + size.y * z).
## Null when nothing is filled.
static func mesh(cells: PackedInt32Array, size: Vector3i, grid: int = GameMaterials.MICRO_GRID) -> ArrayMesh:
	material()
	var any := false
	for c in cells:
		if c != 0:
			any = true
			break
	if not any:
		return null
	# MicroMesher winds counter-clockwise, Godot's front faces are clockwise: flip the
	# triangles in the arrays before the mesh exists. MobModel.outward flips an
	# ArrayMesh instead, and reading its arrays back waits for the render thread
	# (ADR 0016): 4-6 ms per rebuild in game against 0.05 ms to mesh.
	var a := MicroMesher.build_arrays(cells, size, grid, _tiles, ATLAS, Vector3.ZERO)
	if a.is_empty():
		return null
	var ix: PackedInt32Array = a[Mesh.ARRAY_INDEX]
	for t in range(0, ix.size(), 3):
		var k := ix[t + 1]
		ix[t + 1] = ix[t + 2]
		ix[t + 2] = k
	a[Mesh.ARRAY_INDEX] = ix
	var m := ArrayMesh.new()
	m.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, a)
	return m


static func new_cells(size: Vector3i) -> PackedInt32Array:
	var c := PackedInt32Array()
	c.resize(size.x * size.y * size.z)
	c.fill(0)
	return c


static func idx(size: Vector3i, x: int, y: int, z: int) -> int:
	return x + size.x * (y + size.y * z)


static func box(cells: PackedInt32Array, size: Vector3i, a: Vector3i, b: Vector3i, paint: int) -> void:
	for z in range(maxi(a.z, 0), mini(b.z, size.z)):
		for y in range(maxi(a.y, 0), mini(b.y, size.y)):
			for x in range(maxi(a.x, 0), mini(b.x, size.x)):
				cells[idx(size, x, y, z)] = paint + 1


## A node for a piece at voxel `pos`: the solid cells and (optionally) ghost
## cells and a guide (cells on GUIDE_GRID, opaque, no shadow).
static func make_view(pos: Vector3i, solid: Dictionary, ghost: Dictionary = {}, guide: Dictionary = {}) -> Node3D:
	var root := Node3D.new()
	root.position = Vector3(pos)
	_add_mesh(root, solid, material())
	if not ghost.is_empty():
		_add_mesh(root, ghost, ghost_material())
	if not guide.is_empty():
		_add_mesh(root, guide, material(), GUIDE_GRID)
	return root


## A guide plate for one layer of an N x N work grid: each cell a 3x3 tile of
## `paint_of(x, z)` (-1 = none) one fine cell thick at the bottom of layer y,
## with a fine cell of gap around it so the cells read as a grid.
static func guide_plate(n: int, layer: int, paint_of: Callable) -> Dictionary:
	var k := GUIDE_GRID / GameMaterials.MICRO_GRID
	var size := Vector3i(n * k, (layer + 1) * k, n * k)
	var c := new_cells(size)
	for z in n:
		for x in n:
			var paint: int = paint_of.call(x, z)
			if paint >= 0:
				box(c, size, Vector3i(x * k + 1, layer * k, z * k + 1), Vector3i(x * k + k, layer * k + 1, z * k + k), paint)
	return {"cells": c, "size": size}


static func _add_mesh(root: Node3D, part: Dictionary, mat: Material, grid: int = GameMaterials.MICRO_GRID) -> void:
	if part.is_empty():
		return
	var m := mesh(part["cells"], part["size"], grid)
	if m == null:
		return
	var mi := MeshInstance3D.new()
	mi.mesh = m
	mi.material_override = mat
	mi.visibility_range_end = 96.0
	if mat == _ghost or grid != GameMaterials.MICRO_GRID:
		mi.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	root.add_child(mi)
