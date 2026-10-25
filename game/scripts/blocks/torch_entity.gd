class_name TorchEntity
extends RefCounted

## A placed torch (ADR 0030, ADR 0053). Being a block entity puts every torch in the saved
## block-entity list, so TorchLights finds them after a reload without scanning the world. Since
## ADR 0053 a torch is a piece of voxels (PlantStamps.torch: stick, ember wrap, flame), standing on a
## floor or leaning out from a wall (`wall` = the direction to it); its footprint is those voxels, so
## breaking any of them breaks the torch. Torches from older saves are their one voxel (multi false).
## The torch item drops through mining.json, so contents() is empty.

signal changed

var kind := "torch"
var pos: Vector3i
var wall := Vector3i.ZERO
var multi := false
var cells: Array[Vector3i] = []


func _init(p_pos: Vector3i) -> void:
	pos = p_pos
	cells = [pos]


## Makes this a multi-voxel torch hanging on `p_wall` (ZERO: standing).
func set_mount(p_wall: Vector3i) -> void:
	wall = p_wall
	multi = true
	cells = []
	for o in PlantStamps.torch(wall):
		cells.append(pos + o)


## Where the flame burns, in voxels (the light's place).
func flame() -> Vector3:
	if not multi:
		return Vector3(pos) + Vector3(0.5, 0.85, 0.5)
	return Vector3(pos + PlantStamps.torch_flame(wall)) + Vector3(0.5, 0.5, 0.5)


func tick() -> bool:
	return false


func is_lit() -> bool:
	return false   # never ticks, never makes the save dirty


func contents() -> Array:
	return []


func to_dict() -> Dictionary:
	var d := {"kind": kind, "pos": [pos.x, pos.y, pos.z]}
	if multi:
		d["wall"] = [wall.x, wall.y, wall.z]
	return d


func load_from(d: Dictionary) -> void:
	var w: Array = d.get("wall", [])
	if w.size() == 3:
		set_mount(Vector3i(int(w[0]), int(w[1]), int(w[2])))


func summary() -> Dictionary:
	return {"kind": kind, "pos": pos, "wall": wall}


# ---------------- the torch in the hand and on the ground (ADR 0053) ----------------
## The same voxels as a placed torch (PlantStamps.torch), as a mesh 1 unit tall centred on the origin
## like an item sprite (BlockMeshes.sprite hands it to the viewmodel and to dropped stacks): a stick
## and ember wrap in 8-shade palettes, and the flame as a second surface that glows; TorchLights
## flickers its glow with the lights.

const STICK := [[58, 38, 20], [92, 60, 32], [118, 80, 44]]   # dark wood: three bases, 8 shades each
const WRAP := [[110, 50, 20], [176, 86, 30], [232, 140, 50]]  # charred cloth glowing at the seams
const FLAME := [[255, 120, 30], [255, 176, 60], [255, 228, 130]]
const SHADES := 8

static var _mesh: ArrayMesh
static var flame_material: StandardMaterial3D


static func item_mesh() -> ArrayMesh:
	if _mesh:
		return _mesh
	var st := PlantStamps.torch(Vector3i.ZERO)
	var lo := Vector3i(1 << 20, 1 << 20, 1 << 20)
	var hi := -lo
	for o in st:
		lo = Vector3i(mini(lo.x, o.x), mini(lo.y, o.y), mini(lo.z, o.z))
		hi = Vector3i(maxi(hi.x, o.x), maxi(hi.y, o.y), maxi(hi.z, o.z))
	var size := hi - lo + Vector3i.ONE
	var body := PackedInt32Array()
	body.resize(size.x * size.y * size.z)
	var fire := body.duplicate()
	var flame_id := GameMaterials.id_of("torch")
	var wrap_id := GameMaterials.id_of("torch_wrap")
	for o in st:
		var q: Vector3i = o - lo
		var i := q.x + size.x * (q.y + size.y * q.z)
		var h := hash(q) % 3
		if st[o] == flame_id:
			fire[i] = 1 + h * SHADES + clampi(3 + q.y - size.y + 4 + int(hash(q * 3) % 3), 0, SHADES - 1)
		else:
			var base := 3 + h if st[o] == wrap_id else h
			body[i] = 1 + base * SHADES + int(hash(q * 5) % SHADES)
	var g := size.y   # cells per unit: the torch is one unit tall
	var origin := Vector3(-size.x * 0.5, -size.y * 0.5, -size.z * 0.5) / g
	var tiles := PackedInt32Array()
	tiles.resize(6 * SHADES + 1)
	tiles[0] = -1
	for k in 6 * SHADES:
		tiles[k + 1] = k
	var atlas := Vector2i(SHADES, 6)
	var tex := _palette_texture(STICK + WRAP, atlas)
	var body_mat := StandardMaterial3D.new()
	body_mat.albedo_texture = tex
	body_mat.texture_filter = BaseMaterial3D.TEXTURE_FILTER_NEAREST
	body_mat.roughness = 0.9
	flame_material = StandardMaterial3D.new()
	flame_material.albedo_texture = _palette_texture(FLAME + FLAME, atlas)
	flame_material.texture_filter = BaseMaterial3D.TEXTURE_FILTER_NEAREST
	flame_material.emission_enabled = true
	flame_material.emission_texture = flame_material.albedo_texture
	flame_material.emission_energy_multiplier = 2.0
	var a := MobModel.outward(MicroMesher.build(body, size, g, tiles, atlas, origin))
	var b := MobModel.outward(MicroMesher.build(fire, size, g, tiles, atlas, origin))
	_mesh = ArrayMesh.new()
	_mesh.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, a.surface_get_arrays(0))
	_mesh.surface_set_material(0, body_mat)
	_mesh.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, b.surface_get_arrays(0))
	_mesh.surface_set_material(1, flame_material)
	return _mesh


## One flat tile per (base, shade): row = base, column = shade, dark to light.
static func _palette_texture(bases: Array, atlas: Vector2i) -> ImageTexture:
	var img := Image.create(atlas.x * 4, atlas.y * 4, false, Image.FORMAT_RGBA8)
	for b in bases.size():
		for k in SHADES:
			img.fill_rect(Rect2i(k * 4, b * 4, 4, 4), MobModel.shade_color(bases[b], k))
	return ImageTexture.create_from_image(img)
