class_name MobModel
extends Node3D

## Bodies of the passive animals (ADR 0022), micro-voxel cells meshed by the
## native MicroMesher like the player's ApeModel: 16 cells per voxel (3.1 cm),
## so one Minecraft model pixel (1/16 m) is 2 cells and Minecraft's sheep and
## pig proportions carry over. Colours come from a small palette atlas made
## here. Head and legs are separate meshes pivoted at the neck and hips;
## meshes are built once per species (and per sheared look) and shared.
## The animal faces +Z, feet at y = 0, in voxel units.

const GRID := 16
## The cells were laid out for the old 0.5 m voxel (16 cells = 50 cm, 3.1 cm each); since ADR 0033 a
## voxel is 12.5 cm, so the model node is scaled by a block's worth of voxels (Mob._apply_size).
## Unscaled, every animal was a quarter of its size (bug report 2026-10-24).
const UNIT := float(Settings.BLOCK)
const ATLAS := Vector2i(8, 6)   # 48 tiles: the farm animals and the wild ones (ADR 0058)

# palette tiles (paint = tile + 1)
const WOOL := 0
const WOOL_SHADE := 1
const SKIN := 2          # sheep face and legs
const SKIN_DARK := 3     # hooves, nose
const PINK := 4          # pig
const PINK_DARK := 5     # snout, hooves
const EYE_WHITE := 6
const EYE := 7
const NOSTRIL := 8
const PINK_LIGHT := 9
const SHORN := 10        # a sheared sheep's short fleece
const RED := 11          # chicken comb and wattle (ADR 0027)
const YELLOW := 12       # chicken beak and legs
const FUR := 13          # bear
const FUR_DARK := 14     # bear legs, ears, paws
const MUZZLE := 15       # bear snout
const PALETTE: Array = [
	[236, 236, 230], [214, 212, 204], [218, 190, 160], [150, 120, 96], [240, 162, 160], [212, 116, 120],
	[250, 250, 248], [26, 22, 22], [120, 52, 60], [250, 186, 182], [226, 212, 186],
	[200, 40, 36], [236, 186, 60], [104, 70, 44], [70, 46, 30], [160, 124, 90],
] + WildModels.PALETTE   # the wild animals' colours (ADR 0058), from index WildModels.BASE

static var _material: StandardMaterial3D
static var _tiles: PackedInt32Array
static var _meshes := {}   # "species/part/variant" -> ArrayMesh

var species := "sheep"
var head: MeshInstance3D
var body: MeshInstance3D
var legs: Array[MeshInstance3D] = []   # front left, front right, back left, back right (chickens: left, right)
var wings: Array[MeshInstance3D] = []  # chickens
var tails: Array[MeshInstance3D] = []  # foxes, wolves, fish (WildModels.animate)
var flapping := false                  # set by Mob while in the air
var _phase := 0.0
var _time := 0.0
var _sheared := false
var _head_rest := Vector3.ZERO
var _overlay: StandardMaterial3D


static func material() -> StandardMaterial3D:
	if _material:
		return _material
	var img := Image.create(ATLAS.x * 16, ATLAS.y * 16, false, Image.FORMAT_RGBA8)
	var rng := RandomNumberGenerator.new()
	rng.seed = 7171
	for t in PALETTE.size():
		var base: Array = PALETTE[t]
		var spread := 7 if t in [WOOL, WOOL_SHADE, SHORN, FUR, FUR_DARK] else 4
		for y in 16:
			for x in 16:
				var k := rng.randi_range(-spread, spread)
				img.set_pixel((t % ATLAS.x) * 16 + x, (t / ATLAS.x) * 16 + y, Color8(clampi(base[0] + k, 0, 255), clampi(base[1] + k, 0, 255), clampi(base[2] + k, 0, 255)))
	_material = StandardMaterial3D.new()
	_material.albedo_texture = ImageTexture.create_from_image(img)
	_material.texture_filter = BaseMaterial3D.TEXTURE_FILTER_NEAREST_WITH_MIPMAPS
	_material.roughness = 0.95
	_tiles = PackedInt32Array()
	_tiles.resize(PALETTE.size() + 1)
	_tiles[0] = -1
	for i in PALETTE.size():
		_tiles[i + 1] = i
	return _material


## Builds every part mesh now (the loading screen covers it): the cell loops take
## ~25 ms, a frame spike when the first herd spawns otherwise.
static func prewarm() -> void:
	var m := MobModel.new()
	for key in [["sheep", ["body", "body_shorn", "head", "leg", "leg_shorn"]], ["pig", ["body", "head", "leg"]],
			["chicken", ["body", "head", "leg", "wing"]], ["bear", ["body", "head", "leg"]]]:
		m.species = key[0]
		for part in key[1]:
			m._mesh(part)
	for sp in WildModels.LAYOUT:
		m.species = sp
		for part in WildModels.parts(sp):
			m._mesh(part)
	m.free()


static func create(p_species: String, sheared: bool) -> MobModel:
	var m := MobModel.new()
	m.species = p_species
	m._sheared = sheared
	return m


func _ready() -> void:
	material()
	var s := 1.0 / GRID
	if WildModels.has(species):
		WildModels.layout(self)
	elif species == "chicken":
		body = _part("body", Vector3(0, 7, 0) * s)
		head = _part("head", Vector3(0, 15, 6) * s)
		for p in [Vector3(2.5, 8, 0), Vector3(-2.5, 8, 0)]:
			legs.append(_part("leg", p * s))
		for p in [Vector3(7, 16, 0), Vector3(-7, 16, 0)]:
			wings.append(_part("wing", p * s))
	elif species == "bear":
		body = _part("body", Vector3(0, 16, 0) * s)
		head = _part("head", Vector3(0, 30, 22) * s)
		var hip := 20.0
		for p in [Vector3(7, hip, 14), Vector3(-7, hip, 14), Vector3(7, hip, -14), Vector3(-7, hip, -14)]:
			legs.append(_part("leg", p * s))
	elif species == "pig":
		body = _part("body", Vector3(0, 12, 0) * s)
		head = _part("head", Vector3(0, 18, 14) * s)
		var hip := 12.0
		for p in [Vector3(5, hip, 10), Vector3(-5, hip, 10), Vector3(5, hip, -11), Vector3(-5, hip, -11)]:
			legs.append(_part("leg", p * s))
	else:
		body = _part("body_shorn" if _sheared else "body", Vector3(0, 22, 0) * s)
		head = _part("head", Vector3(0, 33, 15) * s)
		var hip := 24.0
		for p in [Vector3(5, hip, 11), Vector3(-5, hip, 11), Vector3(5, hip, -12), Vector3(-5, hip, -12)]:
			legs.append(_part("leg_shorn" if _sheared else "leg", p * s))
	_head_rest = head.position


## Swaps between the fleece and the shorn look (shears / grazing).
func set_sheared(on: bool) -> void:
	if species != "sheep" or on == _sheared:
		return
	_sheared = on
	body.mesh = _mesh("body_shorn" if on else "body")
	for l in legs:
		l.mesh = _mesh("leg_shorn" if on else "leg")


## A red flash while hurt (alpha 0 = off).
func set_hurt(amount: float) -> void:
	if amount <= 0.0:
		if _overlay != null:
			for c in get_children():
				(c as GeometryInstance3D).material_overlay = null
			_overlay = null
		return
	if _overlay == null:
		_overlay = StandardMaterial3D.new()
		_overlay.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
		_overlay.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
		for c in get_children():
			(c as GeometryInstance3D).material_overlay = _overlay
	_overlay.albedo_color = Color(1.0, 0.1, 0.1, 0.45 * amount)


## speed: voxels/s; graze 0..1 (head down eating); head_yaw / head_pitch in radians
## relative to the body; dying 0..1 tips the animal on its side.
func animate(delta: float, speed: float, graze: float, head_yaw: float, head_pitch: float, dying: float) -> void:
	_time += delta
	var moving := speed > 0.15
	if moving:
		_phase += delta * minf(speed, 10.0) * 2.4
	var amp := clampf(speed / 3.0, 0.0, 1.0) * 0.75 if moving else 0.0
	var sw := sin(_phase) * amp
	for i in legs.size():   # pairs, left and right opposite, each pair opposite the one before
		legs[i].rotation.x = sw if (i % 2 == 0) == ((i / 2) % 2 == 0) else -sw
	for i in wings.size():
		var flap := sin(_time * 28.0) * 0.9 if flapping else 0.0
		wings[i].rotation.z = flap * (1.0 if i == 0 else -1.0)
	body.position.y = body.position.y   # the body stays; the hips carry it
	# grazing: the head drops to the grass and chews
	var chew := sin(_time * 18.0) * 0.08 * graze
	head.rotation = Vector3(head_pitch * (1.0 - graze) + graze * 1.05 + chew, head_yaw * (1.0 - graze), 0)
	head.position = _head_rest + Vector3(0, -graze * 0.35, graze * 0.05)
	rotation.z = dying * PI * 0.5
	if not tails.is_empty() or WildModels.has(species):
		WildModels.animate(self, Settings.v2m(speed))


func _part(part: String, pos: Vector3) -> MeshInstance3D:
	var mi := MeshInstance3D.new()
	mi.mesh = _mesh(part)
	mi.material_override = material()
	mi.position = pos
	mi.visibility_range_end = Settings.m2v(96.0)   # world units: 120 voxels hid animals past 15 m
	add_child(mi)
	return mi


func _mesh(part: String) -> ArrayMesh:
	var key := species + "/" + part
	if not _meshes.has(key):
		var spec: Array = _build(species, part)
		material()
		_meshes[key] = outward(MicroMesher.build(spec[0], spec[1], GRID, _tiles, ATLAS, spec[2]))
	return _meshes[key]


## MicroMesher winds its triangles counter-clockwise seen from outside, but Godot's
## front faces are clockwise: with back-face culling the far inside walls show.
## Reverses every triangle once (normals already point out). Used for the animals,
## furniture and the terrain's micro models; the ape (ApeModel) is still drawn
## inside out and was authored that way (ADR 0022).
static func outward(mesh: ArrayMesh) -> ArrayMesh:
	if mesh == null:
		return mesh
	var a := mesh.surface_get_arrays(0)
	var idx: PackedInt32Array = a[Mesh.ARRAY_INDEX]
	for t in range(0, idx.size(), 3):
		var k := idx[t + 1]
		idx[t + 1] = idx[t + 2]
		idx[t + 2] = k
	a[Mesh.ARRAY_INDEX] = idx
	var out := ArrayMesh.new()
	out.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, a)
	return out


# ---------------- cell models ----------------

## [cells, size, origin (voxels)] of one part; the origin puts the part's pivot at its node position.
static func _build(sp: String, part: String) -> Array:
	var rng := RandomNumberGenerator.new()
	rng.seed = hash(sp + part)
	var s := 1.0 / GRID
	if WildModels.has(sp):
		return WildModels.build(sp, part)
	if sp == "chicken":
		match part:
			"body":
				# 6 x 5 x 8 px of white feathers with a tail tuft at the back
				var size := Vector3i(12, 12, 16)
				var c := _cells(size)
				_box(c, size, Vector3i(0, 0, 2), Vector3i(12, 10, 16), WOOL + 1)
				_box(c, size, Vector3i(3, 8, 0), Vector3i(9, 12, 4), WOOL + 1)
				for i in 24:
					_put(c, size, Vector3i(rng.randi_range(0, 11), rng.randi_range(0, 9), rng.randi_range(2, 15)), WOOL_SHADE + 1)
				return [c, size, Vector3(-6, 0, -8) * s]
			"head":
				# 4 x 6 x 3 px with a beak, red comb on top and wattle under it
				var size := Vector3i(8, 14, 10)
				var c := _cells(size)
				_box(c, size, Vector3i(0, 0, 0), Vector3i(8, 12, 6), WOOL + 1)
				_box(c, size, Vector3i(2, 6, 6), Vector3i(6, 8, 10), YELLOW + 1)
				_box(c, size, Vector3i(3, 3, 6), Vector3i(5, 6, 8), RED + 1)
				_box(c, size, Vector3i(3, 12, 1), Vector3i(5, 14, 5), RED + 1)
				for x in [0, 7]:
					_box(c, size, Vector3i(x, 8, 3), Vector3i(x + 1, 10, 5), EYE + 1)
				return [c, size, Vector3(-4, -2, -2) * s]
			"leg":
				var size := Vector3i(2, 8, 4)
				var c := _cells(size)
				_box(c, size, Vector3i(0, 0, 0), Vector3i(2, 8, 2), YELLOW + 1)
				_box(c, size, Vector3i(0, 0, 0), Vector3i(2, 1, 4), YELLOW + 1)   # toes
				return [c, size, Vector3(-1, -8, -1) * s]
			"wing":
				var size := Vector3i(2, 8, 10)
				var c := _cells(size)
				_box(c, size, Vector3i(0, 0, 0), size, WOOL_SHADE + 1)
				return [c, size, Vector3(-1, -8, -5) * s]
	if sp == "bear":
		match part:
			"body":
				# 12 x 11 x 22 px of shaggy brown fur, a shoulder hump at the front
				var size := Vector3i(24, 26, 44)
				var c := _cells(size)
				_box(c, size, Vector3i(0, 0, 0), Vector3i(24, 22, 44), FUR + 1)
				_box(c, size, Vector3i(3, 22, 24), Vector3i(21, 26, 38), FUR + 1)
				for i in 220:
					_put(c, size, Vector3i(rng.randi_range(0, 23), rng.randi_range(0, 25), rng.randi_range(0, 43)), FUR_DARK + 1)
				_box(c, size, Vector3i(0, 0, 0), Vector3i(24, 2, 44), FUR_DARK + 1)   # belly shadow
				return [c, size, Vector3(-12, 0, -22) * s]
			"head":
				# 8 x 7 x 8 px with a lighter muzzle, black nose, small round ears
				var size := Vector3i(16, 18, 22)
				var c := _cells(size)
				_box(c, size, Vector3i(0, 0, 0), Vector3i(16, 15, 16), FUR + 1)
				_box(c, size, Vector3i(4, 1, 16), Vector3i(12, 8, 22), MUZZLE + 1)
				_box(c, size, Vector3i(6, 6, 21), Vector3i(10, 8, 22), EYE + 1)    # nose
				for x in [0, 12]:
					_box(c, size, Vector3i(x, 15, 6), Vector3i(x + 4, 18, 9), FUR_DARK + 1)   # ears
				for x in [2, 11]:
					_box(c, size, Vector3i(x, 9, 15), Vector3i(x + 3, 11, 16), EYE + 1)
				return [c, size, Vector3(-8, -6, -2) * s]
			"leg":
				var size := Vector3i(10, 20, 10)
				var c := _cells(size)
				_box(c, size, Vector3i(0, 0, 0), size, FUR_DARK + 1)
				_box(c, size, Vector3i(0, 0, 0), Vector3i(10, 3, 10), EYE + 1)   # paws
				return [c, size, Vector3(-5, -20, -5) * s]
	if sp == "pig":
		match part:
			"body":
				# 10 x 8 x 16 px, pink with lighter flanks and a curly tail at the back
				var size := Vector3i(20, 18, 36)
				var c := _cells(size)
				_box(c, size, Vector3i(0, 0, 2), Vector3i(20, 16, 34), PINK + 1)
				for i in 30:
					_put(c, size, Vector3i(rng.randi_range(0, 19), rng.randi_range(2, 15), rng.randi_range(2, 33)), PINK_LIGHT + 1)
				_box(c, size, Vector3i(0, 0, 2), Vector3i(20, 2, 34), PINK_DARK + 1)   # belly shadow
				for q in [Vector3i(9, 12, 1), Vector3i(10, 12, 0), Vector3i(10, 13, 0), Vector3i(10, 14, 0), Vector3i(9, 14, 0), Vector3i(9, 15, 1)]:
					_put(c, size, q, PINK_DARK + 1)
				return [c, size, Vector3(-10, 0, -18) * s]
			"head":
				# 8 x 8 x 8 px with a 4 x 3 px snout, ears, eyes
				var size := Vector3i(16, 18, 19)
				var c := _cells(size)
				_box(c, size, Vector3i(0, 0, 0), Vector3i(16, 16, 16), PINK + 1)
				_box(c, size, Vector3i(4, 2, 16), Vector3i(12, 8, 19), PINK_DARK + 1)
				for x in [6, 9]:
					_box(c, size, Vector3i(x, 4, 18), Vector3i(x + 2, 6, 19), NOSTRIL + 1)
				for x in [1, 11]:
					_box(c, size, Vector3i(x, 9, 15), Vector3i(x + 4, 12, 16), EYE_WHITE + 1)
					_box(c, size, Vector3i(x + (2 if x == 1 else 0), 9, 15), Vector3i(x + (4 if x == 1 else 2), 11, 16), EYE + 1)
				# ears flop forward over the eyes' corners
				_box(c, size, Vector3i(0, 16, 6), Vector3i(5, 18, 12), PINK_DARK + 1)
				_box(c, size, Vector3i(11, 16, 6), Vector3i(16, 18, 12), PINK_DARK + 1)
				return [c, size, Vector3(-8, -6, -1) * s]
			"leg":
				var size := Vector3i(8, 12, 8)
				var c := _cells(size)
				_box(c, size, Vector3i(0, 0, 0), Vector3i(8, 12, 8), PINK + 1)
				_box(c, size, Vector3i(0, 0, 0), Vector3i(8, 2, 8), PINK_DARK + 1)
				return [c, size, Vector3(-4, -12, -4) * s]
	# sheep
	match part:
		"body":
			# fleece 10 x 8 x 18 px around the 8 x 6 x 16 px body, curly shading
			var size := Vector3i(20, 16, 36)
			var c := _cells(size)
			_box(c, size, Vector3i(0, 0, 0), size, WOOL + 1)
			for i in 160:
				_put(c, size, Vector3i(rng.randi_range(0, 19), rng.randi_range(0, 15), rng.randi_range(0, 35)), WOOL_SHADE + 1)
			# rounded edges: shave the 12 box edges
			for x in [0, 19]:
				for y in [0, 15]:
					for z in 36:
						_put(c, size, Vector3i(x, y, z), 0)
			for z in [0, 35]:
				for y in [0, 15]:
					for x in 20:
						_put(c, size, Vector3i(x, y, z), 0)
			return [c, size, Vector3(-10, 0, -18) * s]
		"body_shorn":
			var size := Vector3i(16, 12, 32)
			var c := _cells(size)
			_box(c, size, Vector3i(0, 0, 0), size, SHORN + 1)
			for i in 40:
				_put(c, size, Vector3i(rng.randi_range(0, 15), rng.randi_range(0, 11), rng.randi_range(0, 31)), SKIN + 1)
			return [c, size, Vector3(-8, 2, -16) * s]
		"head":
			# 6 x 6 x 8 px face with a fleece cap, ears out to the sides
			var size := Vector3i(18, 14, 16)
			var c := _cells(size)
			_box(c, size, Vector3i(3, 0, 0), Vector3i(15, 12, 16), SKIN + 1)
			_box(c, size, Vector3i(3, 9, 0), Vector3i(15, 14, 13), WOOL + 1)       # cap
			_box(c, size, Vector3i(2, 4, 0), Vector3i(16, 14, 8), WOOL + 1)        # fleece behind the cheeks
			_box(c, size, Vector3i(0, 8, 6), Vector3i(3, 10, 10), SKIN + 1)        # ears
			_box(c, size, Vector3i(15, 8, 6), Vector3i(18, 10, 10), SKIN + 1)
			for x in [4, 11]:
				_box(c, size, Vector3i(x, 6, 15), Vector3i(x + 3, 8, 16), EYE_WHITE + 1)
				_box(c, size, Vector3i(x + (1 if x == 4 else 0), 6, 15), Vector3i(x + (3 if x == 4 else 2), 7, 16), EYE + 1)
			_box(c, size, Vector3i(7, 1, 15), Vector3i(11, 3, 16), SKIN_DARK + 1)   # nose
			return [c, size, Vector3(-9, -4, -2) * s]
		"leg", "leg_shorn":
			# 4 x 12 x 4 px: skin and a hoof below, fleece over the top half
			var size := Vector3i(10, 24, 10)
			var c := _cells(size)
			_box(c, size, Vector3i(1, 0, 1), Vector3i(9, 24, 9), SKIN + 1)
			_box(c, size, Vector3i(1, 0, 1), Vector3i(9, 2, 9), SKIN_DARK + 1)
			if part == "leg":
				_box(c, size, Vector3i(0, 12, 0), Vector3i(10, 24, 10), WOOL + 1)
			return [c, size, Vector3(-5, -24, -5) * s]
	push_error("MobModel: no part %s/%s" % [sp, part])
	return [_cells(Vector3i.ONE), Vector3i.ONE, Vector3.ZERO]


static func _cells(size: Vector3i) -> PackedInt32Array:
	var c := PackedInt32Array()
	c.resize(size.x * size.y * size.z)
	c.fill(0)
	return c


static func _put(c: PackedInt32Array, size: Vector3i, p: Vector3i, paint: int) -> void:
	if p.x < 0 or p.y < 0 or p.z < 0 or p.x >= size.x or p.y >= size.y or p.z >= size.z:
		return
	c[p.x + size.x * (p.y + size.y * p.z)] = paint


## Fills [from, to) with paint.
static func _box(c: PackedInt32Array, size: Vector3i, from: Vector3i, to: Vector3i, paint: int) -> void:
	for z in range(from.z, to.z):
		for y in range(from.y, to.y):
			for x in range(from.x, to.x):
				_put(c, size, Vector3i(x, y, z), paint)
