class_name BlockMeshes
extends RefCounted

## Unit cube meshes textured with a material's atlas tiles (top/side/bottom),
## for item entities and the held-item viewmodel. Cached per material id.
## Non-block items get a flat "card" mesh in the item's placeholder colour.

static var _cache := {}
static var _card_cache := {}


static func cube(material_id: int, surface_material: Material) -> Mesh:
	if _cache.has(material_id):
		return _cache[material_id]
	var d: Dictionary = GameMaterials.defs()[material_id - 1]
	var t: Array = d["tiles"]
	var st := SurfaceTool.new()
	st.begin(Mesh.PRIMITIVE_TRIANGLES)
	# faces: normal, tile, four corners (clockwise seen from outside = Godot front face)
	var faces := [
		[Vector3.UP, t[0], [Vector3(0, 1, 1), Vector3(1, 1, 1), Vector3(1, 1, 0), Vector3(0, 1, 0)]],
		[Vector3.DOWN, t[2], [Vector3(0, 0, 0), Vector3(1, 0, 0), Vector3(1, 0, 1), Vector3(0, 0, 1)]],
		[Vector3.FORWARD, t[1], [Vector3(1, 0, 0), Vector3(0, 0, 0), Vector3(0, 1, 0), Vector3(1, 1, 0)]],
		[Vector3.BACK, t[1], [Vector3(0, 0, 1), Vector3(1, 0, 1), Vector3(1, 1, 1), Vector3(0, 1, 1)]],
		[Vector3.LEFT, t[1], [Vector3(0, 0, 0), Vector3(0, 0, 1), Vector3(0, 1, 1), Vector3(0, 1, 0)]],
		[Vector3.RIGHT, t[1], [Vector3(1, 0, 1), Vector3(1, 0, 0), Vector3(1, 1, 0), Vector3(1, 1, 1)]],
	]
	for f in faces:
		var n: Vector3 = f[0]
		var uv := tile_uv(int(f[1]))
		var c: Array = f[2]
		var uvs := [Vector2(uv.position.x, uv.end.y), Vector2(uv.end.x, uv.end.y), Vector2(uv.end.x, uv.position.y), Vector2(uv.position.x, uv.position.y)]
		for idx in [0, 2, 1, 0, 3, 2]:
			st.set_normal(n)
			st.set_uv(uvs[idx])
			st.set_color(Color.WHITE)
			st.add_vertex(c[idx] - Vector3(0.5, 0.5, 0.5))
	st.index()
	var mesh := st.commit()
	mesh.surface_set_material(0, surface_material)
	_cache[material_id] = mesh
	return mesh


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
