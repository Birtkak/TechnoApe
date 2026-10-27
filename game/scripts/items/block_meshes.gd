class_name BlockMeshes
extends RefCounted

## Unit cube meshes textured with a material's atlas tiles (top/side/bottom),
## for item entities and the held-item viewmodel. Cached per material id.
## Items with a sprite (ItemSprites) get it extruded one pixel thick, or a
## cube for block-face sprites; anything else a flat "card" mesh in the
## item's placeholder colour.

static var _cache := {}
static var _card_cache := {}
static var _sprite_cache := {}
## Items held and dropped as a voxel model, the same as they stand in the world (ADR 0053): id -> mesh
## builder (1 unit tall, centred, like a sprite mesh). The inventory icon stays the sprite.
static var MODELS := {"torch": TorchEntity.item_mesh}


static func cube(material_id: int, surface_material: Material) -> Mesh:
	if _cache.has(material_id):
		return _cache[material_id]
	var d: Dictionary = GameMaterials.defs()[material_id - 1]
	if d["name"] in StationEntity.MODELS:
		_cache[material_id] = StationEntity.item_mesh(d["name"])   # the placed model, not an atlas cube (ADR 0084)
		return _cache[material_id]
	var t: Array = d["tiles"]
	var mesh := _box(tile_uv(int(t[0])), tile_uv(int(t[1])), tile_uv(int(t[2])), surface_material)
	_cache[material_id] = mesh
	return mesh


## Unit cube centred on the origin with a UV rectangle for top, sides and bottom.
static func _box(top: Rect2, side: Rect2, bottom: Rect2, surface_material: Material) -> Mesh:
	var st := SurfaceTool.new()
	st.begin(Mesh.PRIMITIVE_TRIANGLES)
	# faces: normal, uv rect, four corners (clockwise seen from outside = Godot front face)
	var faces := [
		[Vector3.UP, top, [Vector3(0, 1, 1), Vector3(1, 1, 1), Vector3(1, 1, 0), Vector3(0, 1, 0)]],
		[Vector3.DOWN, bottom, [Vector3(0, 0, 0), Vector3(1, 0, 0), Vector3(1, 0, 1), Vector3(0, 0, 1)]],
		[Vector3.FORWARD, side, [Vector3(1, 0, 0), Vector3(0, 0, 0), Vector3(0, 1, 0), Vector3(1, 1, 0)]],
		[Vector3.BACK, side, [Vector3(0, 0, 1), Vector3(1, 0, 1), Vector3(1, 1, 1), Vector3(0, 1, 1)]],
		[Vector3.LEFT, side, [Vector3(0, 0, 0), Vector3(0, 0, 1), Vector3(0, 1, 1), Vector3(0, 1, 0)]],
		[Vector3.RIGHT, side, [Vector3(1, 0, 1), Vector3(1, 0, 0), Vector3(1, 1, 0), Vector3(1, 1, 1)]],
	]
	var half := Vector3.ONE * 0.5
	for f in faces:
		var uv: Rect2 = f[1]
		var c: Array = f[2]
		var uvs := [Vector2(uv.position.x, uv.end.y), Vector2(uv.end.x, uv.end.y), Vector2(uv.end.x, uv.position.y), Vector2(uv.position.x, uv.position.y)]
		_quad(st, f[0], [c[0] - half, c[1] - half, c[2] - half, c[3] - half], uvs)
	st.index()
	var mesh := st.commit()
	mesh.surface_set_material(0, surface_material)
	return mesh


static func _quad(st: SurfaceTool, n: Vector3, c: Array, uvs: Array) -> void:
	for idx in [0, 2, 1, 0, 3, 2]:
		st.set_normal(n)
		st.set_uv(uvs[idx])
		st.set_color(Color.WHITE)
		st.add_vertex(c[idx])


## An item's sprite as a mesh, 1 unit across, centred on the origin. Block-face
## sprites become a cube; the rest are extruded Minecraft-style: the sprite on
## the front and back, SPRITE_DEPTH_PX pixels thick, and every opaque pixel's exposed
## edges walled in that pixel's colour.
static func sprite(item_id: String) -> Mesh:
	if _sprite_cache.has(item_id):
		return _sprite_cache[item_id]
	if MODELS.has(item_id):
		_sprite_cache[item_id] = MODELS[item_id].call()   # a voxel model instead of the flat sprite (ADR 0053)
		return _sprite_cache[item_id]
	var sprites := ItemSprites.get_default()
	var m := StandardMaterial3D.new()
	m.albedo_texture = sprites.texture(item_id)
	m.texture_filter = BaseMaterial3D.TEXTURE_FILTER_NEAREST
	m.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA_SCISSOR
	m.alpha_scissor_threshold = 0.5
	m.roughness = 0.9
	var mesh: Mesh
	if sprites.is_cube(item_id):
		m.cull_mode = BaseMaterial3D.CULL_DISABLED   # see-through faces (glass) show the far side
		var full := Rect2(0, 0, 1, 1)
		mesh = _box(full, full, full, m)
	else:
		mesh = _extrude(sprites.image(item_id), m)
	_sprite_cache[item_id] = mesh
	return mesh


static var _hot_cache := {}

## Item `item_id`'s sprite material glowing at heat h (ADR 0087: dull red to yellow-white, in 8 steps), or
## null when it does not glow (cooler than Settings.glow_from_heat, or a voxel model).
static func heat_material(item_id: String, h: float) -> Material:
	if Inventory.glow_color(h).a <= 0.0 or MODELS.has(item_id) or not ItemSprites.get_default().has(item_id):
		return null
	var step := ceili(h * 8.0)
	var key := "%s|%d" % [item_id, step]
	if not _hot_cache.has(key):
		var m := sprite(item_id).surface_get_material(0).duplicate() as StandardMaterial3D
		var g := Inventory.glow_color(step / 8.0)
		m.albedo_color = Color.WHITE.lerp(Color(g.r, g.g, g.b), 0.7 * g.a)
		m.emission_enabled = true
		m.emission = Color(g.r, g.g, g.b)
		m.emission_energy_multiplier = 1.2 * g.a
		_hot_cache[key] = m
	return _hot_cache[key]


const SPRITE_DEPTH_PX := 3.0   # extrusion depth in sprite pixels: thicker than Minecraft's 1 (developer, 2026-10-27)


static func _extrude(img: Image, surface_material: Material) -> Mesh:
	var n := img.get_width()
	var p := 1.0 / n
	var hz := p * SPRITE_DEPTH_PX * 0.5
	var st := SurfaceTool.new()
	st.begin(Mesh.PRIMITIVE_TRIANGLES)
	var u := [Vector2(0, 1), Vector2(1, 1), Vector2(1, 0), Vector2(0, 0)]
	_quad(st, Vector3.BACK, [Vector3(-0.5, -0.5, hz), Vector3(0.5, -0.5, hz), Vector3(0.5, 0.5, hz), Vector3(-0.5, 0.5, hz)], u)
	_quad(st, Vector3.FORWARD, [Vector3(0.5, -0.5, -hz), Vector3(-0.5, -0.5, -hz), Vector3(-0.5, 0.5, -hz), Vector3(0.5, 0.5, -hz)], [u[1], u[0], u[3], u[2]])
	for y in n:
		for x in n:
			if not _opaque(img, x, y):
				continue
			var uv := Vector2((x + 0.5) * p, (y + 0.5) * p)
			var uvs := [uv, uv, uv, uv]
			var x0 := x * p - 0.5
			var x1 := x0 + p
			var y1 := 0.5 - y * p
			var y0 := y1 - p
			if not _opaque(img, x, y - 1):
				_quad(st, Vector3.UP, [Vector3(x0, y1, hz), Vector3(x1, y1, hz), Vector3(x1, y1, -hz), Vector3(x0, y1, -hz)], uvs)
			if not _opaque(img, x, y + 1):
				_quad(st, Vector3.DOWN, [Vector3(x0, y0, -hz), Vector3(x1, y0, -hz), Vector3(x1, y0, hz), Vector3(x0, y0, hz)], uvs)
			if not _opaque(img, x - 1, y):
				_quad(st, Vector3.LEFT, [Vector3(x0, y0, -hz), Vector3(x0, y0, hz), Vector3(x0, y1, hz), Vector3(x0, y1, -hz)], uvs)
			if not _opaque(img, x + 1, y):
				_quad(st, Vector3.RIGHT, [Vector3(x1, y0, hz), Vector3(x1, y0, -hz), Vector3(x1, y1, -hz), Vector3(x1, y1, hz)], uvs)
	st.index()
	var mesh := st.commit()
	mesh.surface_set_material(0, surface_material)
	return mesh


static func _opaque(img: Image, x: int, y: int) -> bool:
	return x >= 0 and y >= 0 and x < img.get_width() and y < img.get_height() and img.get_pixel(x, y).a >= 0.5


## UV rectangle (0..1) of an atlas tile.
static func tile_uv(tile: int) -> Rect2:
	var cols := GameMaterials.ATLAS_TILES.x
	var rows := GameMaterials.ATLAS_TILES.y
	return Rect2(float(tile % cols) / cols, float(tile / cols) / rows, 1.0 / cols, 1.0 / rows)


## Pixel rectangle of an atlas tile (for AtlasTexture / 2D drawing).
static func tile_region(tile: int, atlas: Texture2D) -> Rect2:
	var uv := tile_uv(tile)
	var s := atlas.get_size()
	return Rect2(uv.position * s, uv.size * s)


## Flat card for non-block items: a thin box tinted with the item's placeholder colour.
static func card(item_id: String) -> Mesh:
	if _card_cache.has(item_id):
		return _card_cache[item_id]
	var box := BoxMesh.new()
	box.size = Vector3(0.6, 0.6, 0.08)
	var m := StandardMaterial3D.new()
	m.albedo_color = ItemIcon.color_for(item_id)
	m.roughness = 0.9
	box.material = m
	_card_cache[item_id] = box
	return box
