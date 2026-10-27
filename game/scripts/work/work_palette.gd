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
	[244, 128, 52],    # GLOW: hot metal on the anvil (ADR 0042)
	[98, 98, 104],     # ANVIL: the worked stone face
	[214, 190, 132],   # SAND: an earth oven's cover, grit on a grinding slab (ADR 0059)
	[38, 26, 20],      # TAR: birch tar, burnt-out wood
	[226, 216, 190],   # BONE
	[122, 92, 62],     # FUR
	[196, 186, 160],   # LINEN
	[146, 144, 136],   # STONE_LIGHT (fieldstone tones, like the furnace)
	[52, 50, 48],      # STONE_DARK: mortar
	[122, 106, 88],    # STONE_WARM
	[30, 26, 24],      # SOOT
	[84, 88, 96],      # IRON_DARK
	[190, 98, 58],     # BRICK: fired clay brick
	[208, 120, 74],    # BRICK_LIGHT
	[96, 52, 36],      # BRICK_DARK: brick mortar
	[176, 132, 70],    # BRONZE: ingots in a pile (ADR 0095)
	[178, 182, 186],   # TIN
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
const GLOW := 23
const ANVIL := 24
const SAND := 25
const TAR := 26
const BONE := 27
const FUR := 28
const LINEN := 29
const STONE_LIGHT := 30
const STONE_DARK := 31
const STONE_WARM := 32
const SOOT := 33
const IRON_DARK := 34
const BRICK := 35
const BRICK_LIGHT := 36
const BRICK_DARK := 37
const BRONZE := 38
const TIN := 39
const GUIDE_GRID := 32   # guide plates are meshed 4x finer: 3x3 coloured, a one-cell gap as grid lines
## Every paint comes in MobModel.SHADES tones (tile = paint * SHADES + tone), so a piece is not one flat
## colour: shade_cells hashes a tone per cell and darkens the cells on a convex edge, which draws the
## shape's outline the way the item sprites' dark edge pixels do.
const SHADES := MobModel.SHADES
const FLAT := [MOLTEN, EMBER, GLOW, PLACE, AVOID]   # signal colours: one tone, no grain, no edge
const ATLAS := Vector2i(SHADES, 40)

static var _material: StandardMaterial3D
static var _ghost: StandardMaterial3D
static var _tiles: PackedInt32Array


static func material() -> StandardMaterial3D:
	if _material:
		return _material
	var img := Image.create(ATLAS.x * 4, ATLAS.y * 4, false, Image.FORMAT_RGBA8)
	for t in PALETTE.size():
		for k in SHADES:
			img.fill_rect(Rect2i(k * 4, t * 4, 4, 4), tone_color(PALETTE[t], k))
	var tex := ImageTexture.create_from_image(img)
	_material = StandardMaterial3D.new()
	_material.albedo_texture = tex
	_material.texture_filter = BaseMaterial3D.TEXTURE_FILTER_NEAREST
	_material.roughness = 0.9
	_ghost = StandardMaterial3D.new()
	_ghost.albedo_texture = tex
	_ghost.albedo_color = Color(1, 1, 1, 0.35)
	_ghost.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
	_ghost.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	_ghost.texture_filter = BaseMaterial3D.TEXTURE_FILTER_NEAREST
	_tiles = PackedInt32Array()
	_tiles.resize(PALETTE.size() * SHADES + 1)
	_tiles[0] = -1
	for i in PALETTE.size() * SHADES:
		_tiles[i + 1] = i
	return _material


## Tone k of a paint: 0 is the outline (half brightness), 1..SHADES-1 run dark to light around the
## base colour (tone 5), warmer towards the light like MobModel.shade_color.
static func tone_color(base: Array, k: int) -> Color:
	if k == 0:
		return Color8(base[0] / 2, base[1] / 2, base[2] / 2)
	var t := float(k - 1) / (SHADES - 2)
	var f := lerpf(0.72, 1.18, t)
	var warm := (t - 0.5) * 14.0
	return Color8(clampi(int(base[0] * f + warm), 0, 255), clampi(int(base[1] * f), 0, 255), clampi(int(base[2] * f - warm), 0, 255))


## Tones per cell for a grid of paint + 1 cells: paint * SHADES + tone + 1, pixel-art style: a
## little grain around the base tone, the bottom rim and the vertical edges in the outline tone,
## the top rim lit. `flat` paints keep their colour (the base tone).
static func shade_cells(c: PackedInt32Array, size: Vector3i, flat: Array = FLAT) -> PackedInt32Array:
	var out := PackedInt32Array()
	out.resize(c.size())
	var empty := func(x: int, y: int, z: int) -> bool:
		return x < 0 or y < 0 or z < 0 or x >= size.x or y >= size.y or z >= size.z or c[idx(size, x, y, z)] <= 0
	for z in size.z:
		for y in size.y:
			for x in size.x:
				var i := idx(size, x, y, z)
				if c[i] <= 0:
					continue
				var paint := c[i] - 1
				var k := 5
				if not (paint in flat):
					var grain := float(hash(Vector3i(x / 3, y, z / 3)) % 1000) / 999.0
					var n := float(hash(Vector3i(x, y, z) * 7 + Vector3i(1, 2, 3)) % 1000) / 999.0
					var sx: bool = empty.call(x - 1, y, z) or empty.call(x + 1, y, z)
					var sz: bool = empty.call(x, y, z - 1) or empty.call(x, y, z + 1)
					k = int(round(4.3 + grain * 1.2 + n * 0.8))
					if empty.call(x, y - 1, z) and (sx or sz):
						k = 0           # bottom rim: outline
					elif sx and sz:
						k = 1           # vertical edge: dark
					elif empty.call(x, y + 1, z) and (sx or sz):
						k = 7           # top rim: lit
				out[i] = paint * SHADES + clampi(k, 0, SHADES - 1) + 1
	return out


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
	var a := MicroMesher.build_arrays(shade_cells(cells, size), size, grid, _tiles, ATLAS, Vector3.ZERO)
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


## One cell of a courses-of-blocks wall (the furnace's look): rows of blocks two cells high with a
## mortar row between, blocks four cells long (three plus a joint) shifted every other course, seen
## the same from every side because the position along the wall is x + z. `y` counts from the wall's base.
static func wall_paint(x: int, y: int, z: int, tones: Array, mortar: int) -> int:
	if y % 3 == 2:
		return mortar
	var course := y / 3
	var u := x + z + (course % 2) * 2
	if u % 4 == 3:
		return mortar
	return tones[hash(Vector3i(u / 4, course, 0)) % tones.size()]


const WALL_STONE := [STONE, STONE_LIGHT, STONE_WARM]
const WALL_BRICK := [BRICK, BRICK_LIGHT, FIRED]


## A box of that wall (a..b, b exclusive).
static func wall_box(cells: PackedInt32Array, size: Vector3i, a: Vector3i, b: Vector3i, tones: Array = WALL_STONE, mortar: int = STONE_DARK) -> void:
	for z in range(maxi(a.z, 0), mini(b.z, size.z)):
		for y in range(maxi(a.y, 0), mini(b.y, size.y)):
			for x in range(maxi(a.x, 0), mini(b.x, size.x)):
				cells[idx(size, x, y, z)] = wall_paint(x, y - a.y, z, tones, mortar) + 1


static func box(cells: PackedInt32Array, size: Vector3i, a: Vector3i, b: Vector3i, paint: int) -> void:
	for z in range(maxi(a.z, 0), mini(b.z, size.z)):
		for y in range(maxi(a.y, 0), mini(b.y, size.y)):
			for x in range(maxi(a.x, 0), mini(b.x, size.x)):
				cells[idx(size, x, y, z)] = paint + 1


## A node for a piece at voxel `pos`: the solid cells and (optionally) ghost
## cells and a guide (cells on GUIDE_GRID, opaque, no shadow).
static func make_view(pos: Vector3i, solid: Dictionary, ghost: Dictionary = {}, guide: Dictionary = {}, laid: Array = [], blocks: float = 1.0) -> Node3D:
	var root := Node3D.new()
	root.position = Vector3(pos)
	root.scale = Vector3.ONE * Settings.BLOCK * blocks   # a piece is one 0.5 m block of BLOCK^3 voxels (ADR 0034), or two (ADR 0076)
	_add_mesh(root, solid, material(), solid.get("grid", GameMaterials.MICRO_GRID))   # a finer piece names its grid (ingot piles, ADR 0095)
	if not ghost.is_empty():
		_add_mesh(root, ghost, ghost_material())
	if not guide.is_empty():
		_add_mesh(root, guide, material(), GUIDE_GRID)
	for it in laid:
		_add_item(root, it)
	return root


## An item drawn as its own sprite (ADR 0042: tool parts on the ground, food over a fire): `at` in
## block fractions, `yaw` in degrees, `scale` in blocks; lying flat unless `upright`; `heat` (0..1) makes it glow.
static func _add_item(root: Node3D, it: Dictionary) -> void:
	var id: String = it.get("id", "")
	if not ItemSprites.get_default().has(id):
		return
	var mi := MeshInstance3D.new()
	mi.mesh = BlockMeshes.sprite(id)
	var b := Basis(Vector3.UP, deg_to_rad(float(it.get("yaw", 0.0))))
	if not it.get("upright", false):
		b = b * Basis(Vector3.RIGHT, -PI / 2.0)
	else:
		# a tool leaning on a wall (ADR 0095): tipped back by `tilt`, turned in its own plane by `roll`
		b = b * Basis(Vector3.RIGHT, deg_to_rad(float(it.get("tilt", 0.0)))) * Basis(Vector3.BACK, deg_to_rad(float(it.get("roll", 0.0))))
	mi.transform = Transform3D(b.scaled(Vector3.ONE * float(it.get("scale", 0.5))), it.get("at", Vector3(0.5, 0.02, 0.5)))
	mi.material_override = BlockMeshes.heat_material(id, float(it.get("heat", 0.0)))   # hot metal glows (ADR 0087)
	mi.visibility_range_end = 96.0
	root.add_child(mi)


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
	if mat == _ghost or grid == GUIDE_GRID:
		mi.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	root.add_child(mi)
