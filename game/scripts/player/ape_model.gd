class_name ApeModel
extends Node3D

## The player's body: a monkey in goggles and a lab coat, built from
## micro-voxel cells (GRID cells per voxel = 3.1 cm, 8x Minecraft's pixel
## density) with the native MicroMesher. Each limb is its own mesh pivoted
## at its joint so the body can walk, swing and look. Colours come from a
## small palette atlas generated here (16 flat tiles with a little grain),
## not from the terrain atlas. Front of the character is -Z, feet at y = 0.
##
## Parts (cells, from the feet): legs 0..24 (boots 0..6), torso 24..42 with
## the coat skirt down to 18, shoulders at 40, arms 22 long (hands at 18),
## neck at 42, head to 57 = 1.78 m. See `build_arm_mesh` for the first-person arm.

const GRID := 16       # cells per voxel for the body (3.1 cm)
const HEAD_GRID := 28  # the head is built on a finer grid (1.8 cm cells): round skull, real eyes; 32 cells = 57 cm, a cartoon head

# palette tile indices (paint = tile + 1 in the cell arrays)
const FUR_DARK := 0
const FUR := 1
const FACE := 2
const COAT := 3
const COAT_SHADE := 4
const COLLAR := 5
const ACCENT := 6
const RIM := 7
const LENS := 8
const TROUSERS := 9
const BOOTS := 10
const NOSE := 11
const BELT := 12
const BUCKLE := 13
const STRAP := 14
const EYE := 15
const EYE_WHITE := 16
const PALETTE: Array = [
	[95, 62, 38], [132, 90, 54], [214, 178, 138], [236, 236, 232], [206, 206, 200], [52, 54, 62],
	[72, 205, 225], [28, 28, 32], [150, 236, 255], [84, 90, 102], [46, 40, 38], [190, 110, 110],
	[70, 45, 30], [214, 184, 82], [92, 92, 98], [20, 20, 24], [246, 246, 244],
]
const ATLAS := Vector2i(8, 3)

static var _material: StandardMaterial3D
static var _tiles: PackedInt32Array

var head: MeshInstance3D
var torso: MeshInstance3D
var arm_r: MeshInstance3D
var arm_l: MeshInstance3D
var leg_r: MeshInstance3D
var leg_l: MeshInstance3D
var tail: MeshInstance3D
var _phase := 0.0
var _time := 0.0


static func material() -> StandardMaterial3D:
	if _material:
		return _material
	var img := Image.create(ATLAS.x * 16, ATLAS.y * 16, false, Image.FORMAT_RGBA8)
	var rng := RandomNumberGenerator.new()
	rng.seed = 4242
	for t in PALETTE.size():
		var base: Array = PALETTE[t]
		var spread := 6 if t != LENS else 14
		for y in 16:
			for x in 16:
				var k := rng.randi_range(-spread, spread)
				var c := Color8(clampi(base[0] + k, 0, 255), clampi(base[1] + k, 0, 255), clampi(base[2] + k, 0, 255))
				img.set_pixel((t % ATLAS.x) * 16 + x, (t / ATLAS.x) * 16 + y, c)
	_material = StandardMaterial3D.new()
	_material.albedo_texture = ImageTexture.create_from_image(img)
	_material.texture_filter = BaseMaterial3D.TEXTURE_FILTER_NEAREST_WITH_MIPMAPS
	_material.roughness = 0.9
	_tiles = PackedInt32Array()
	_tiles.resize(PALETTE.size() + 1)
	_tiles[0] = -1
	for i in PALETTE.size():
		_tiles[i + 1] = i
	return _material


func _ready() -> void:
	material()
	# The part meshes come out facing +Z; the node is turned so the ape faces
	# the player's forward (-Z) and its right arm is on the player's right (+X).
	rotation.y = PI
	var s := 1.0 / GRID
	torso = _part(_torso_cells(), Vector3i(14, 24, 9), Vector3(-7, 0, -4) * s)
	torso.position = Vector3(0, 18, 0) * s
	head = _part(_head_cells(), Vector3i(32, 30, 32), Vector3(-16, 0, -16) / HEAD_GRID, HEAD_GRID)
	head.position = Vector3(0, 41, 0) * s
	arm_r = _part(_arm_cells(true), Vector3i(6, 22, 6), Vector3(-3, -22, -3) * s)
	arm_r.position = Vector3(-10, 40, 0.5) * s
	arm_l = _part(_arm_cells(false), Vector3i(6, 22, 6), Vector3(-3, -22, -3) * s)
	arm_l.position = Vector3(10, 40, 0.5) * s
	leg_r = _part(_leg_cells(), Vector3i(6, 24, 7), Vector3(-3, -24, -3) * s)
	leg_r.position = Vector3(-3.5, 24, 0) * s
	leg_l = _part(_leg_cells(), Vector3i(6, 24, 7), Vector3(-3, -24, -3) * s)
	leg_l.position = Vector3(3.5, 24, 0) * s
	tail = _part(_tail_cells(), Vector3i(4, 20, 14), Vector3(-2, 0, 0) * s)
	tail.position = Vector3(0, 20, 4) * s


func _part(cells: PackedInt32Array, size: Vector3i, origin: Vector3, grid: int = GRID) -> MeshInstance3D:
	var mi := MeshInstance3D.new()
	mi.mesh = MicroMesher.build(cells, size, grid, _tiles, ATLAS, origin)
	mi.material_override = _material
	add_child(mi)
	return mi


## Walk cycle and idle motion. speed: horizontal speed in voxels/s.
func animate(delta: float, speed: float, on_floor: bool, flying: bool, pitch: float, swing: float) -> void:
	_time += delta
	var moving := speed > 0.2 and (on_floor or flying)
	if moving:
		_phase += delta * minf(speed, 12.0) * 1.1
	var amp := clampf(speed / 6.0, 0.0, 1.0) if moving else 0.0
	var sw := sin(_phase) * amp
	if flying and not on_floor:
		leg_r.rotation = Vector3(0.25, 0, 0.08)
		leg_l.rotation = Vector3(0.15, 0, -0.08)
		arm_r.rotation = Vector3(0.3 + 0.1 * sin(_time * 2.0), 0, -0.45)
		arm_l.rotation = Vector3(0.3 - 0.1 * sin(_time * 2.0), 0, 0.45)
	else:
		leg_r.rotation = Vector3(0.9 * sw, 0, 0)
		leg_l.rotation = Vector3(-0.9 * sw, 0, 0)
		arm_r.rotation = Vector3(-0.7 * sw, 0, -0.08 + 0.03 * sin(_time * 1.7))
		arm_l.rotation = Vector3(0.7 * sw, 0, 0.08 - 0.03 * sin(_time * 1.7))
	if swing > 0.0:
		arm_r.rotation.x = -1.4 * sin(swing * PI)
	head.rotation = Vector3(clampf(pitch, -0.9, 0.9), 0, 0)
	tail.rotation = Vector3(0.1 + 0.12 * sin(_time * 2.3), 0.25 * sin(_time * 1.3), 0)
	torso.rotation = Vector3(0.05 * amp, 0, 0)


## The first-person arm: a right arm with the hand at the origin, pointing +y
## (the viewmodel rotates it so it pokes into the view from the lower right).
static func build_arm_mesh() -> MeshInstance3D:
	material()
	var mi := MeshInstance3D.new()
	mi.mesh = MicroMesher.build(_arm_cells(true), Vector3i(6, 22, 6), GRID, _tiles, ATLAS, Vector3(-3, -2.5, -3) / GRID)
	# the arm sits in the camera's shadow most of the day: lift it with a little
	# self-emission and ignore cast shadows, like Minecraft's flat-lit arm
	var m: StandardMaterial3D = _material.duplicate()
	m.disable_receive_shadows = true
	m.emission_enabled = true
	m.emission = Color(1, 1, 1)
	m.emission_energy_multiplier = 0.08
	m.emission_texture = _material.albedo_texture
	mi.material_override = m
	mi.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	return mi


# ---------------------------------------------------------------- cell builders

static func _cells(size: Vector3i) -> PackedInt32Array:
	var c := PackedInt32Array()
	c.resize(size.x * size.y * size.z)
	c.fill(0)
	return c


static func _cell_set(c: PackedInt32Array, size: Vector3i, x: int, y: int, z: int, tile: int) -> void:
	if x < 0 or y < 0 or z < 0 or x >= size.x or y >= size.y or z >= size.z:
		return
	c[x + size.x * (y + size.y * z)] = tile + 1


static func _cell_get(c: PackedInt32Array, size: Vector3i, x: int, y: int, z: int) -> int:
	if x < 0 or y < 0 or z < 0 or x >= size.x or y >= size.y or z >= size.z:
		return 0
	return c[x + size.x * (y + size.y * z)]


static func _box(c: PackedInt32Array, size: Vector3i, x0: int, y0: int, z0: int, x1: int, y1: int, z1: int, tile: int) -> void:
	for z in range(z0, z1):
		for y in range(y0, y1):
			for x in range(x0, x1):
				_cell_set(c, size, x, y, z, tile)


static func _ellipsoid(c: PackedInt32Array, size: Vector3i, cx: float, cy: float, cz: float, rx: float, ry: float, rz: float, tile: int) -> void:
	for z in range(int(floor(cz - rz)), int(ceil(cz + rz)) + 1):
		for y in range(int(floor(cy - ry)), int(ceil(cy + ry)) + 1):
			for x in range(int(floor(cx - rx)), int(ceil(cx + rx)) + 1):
				var dx := (x + 0.5 - cx) / rx
				var dy := (y + 0.5 - cy) / ry
				var dz := (z + 0.5 - cz) / rz
				if dx * dx + dy * dy + dz * dz <= 1.0:
					_cell_set(c, size, x, y, z, tile)


## Repaints the cells of `from` that have an empty neighbour (the visible shell).
static func _repaint_shell(c: PackedInt32Array, size: Vector3i, from: int, to: int, y0: int, y1: int) -> void:
	for z in size.z:
		for y in range(y0, y1):
			for x in size.x:
				if _cell_get(c, size, x, y, z) != from + 1:
					continue
				if _cell_get(c, size, x + 1, y, z) == 0 or _cell_get(c, size, x - 1, y, z) == 0 or _cell_get(c, size, x, y, z + 1) == 0 or _cell_get(c, size, x, y, z - 1) == 0:
					_cell_set(c, size, x, y, z, to)


## z = 0 is the face side. Built at HEAD_GRID cells per voxel: a round
## skull with a wide tan face mask, big eyes, a smile, round ears, a fur tuft,
## and the goggles pushed up on the forehead so the face stays visible.
static func _head_cells() -> PackedInt32Array:
	var size := Vector3i(32, 30, 32)
	var c := _cells(size)
	_ellipsoid(c, size, 16, 14, 16, 13.5, 13.0, 13.5, FUR)                 # skull
	_repaint_inside(c, size, 16, 11, 7, 11.0, 10.0, 8.5, FUR, FACE)         # face mask (only existing skull cells)
	for y in range(19, 24):                                                 # heart-shaped dip of the mask at the brow
		for x in range(13, 19):
			if abs(x + 0.5 - 16) < (y - 18) * 0.9:
				_repaint_cell(c, size, x, y, FACE, FUR)
	_ellipsoid(c, size, 16, 8.5, 5.5, 6.0, 4.5, 3.5, FACE)                  # muzzle
	_ellipsoid(c, size, 1.5, 15, 16, 2.6, 5.0, 5.0, FUR)                    # ears
	_ellipsoid(c, size, 30.5, 15, 16, 2.6, 5.0, 5.0, FUR)
	_ellipsoid(c, size, 0.8, 15, 16, 1.4, 3.0, 3.0, FACE)
	_ellipsoid(c, size, 31.2, 15, 16, 1.4, 3.0, 3.0, FACE)
	for cx in [10.5, 21.5]:                                                 # eyes: white discs one cell proud of the face
		_stamp_front(c, size, cx, 15.0, 4.0, EYE_WHITE, 1)
		_stamp_front(c, size, cx + (0.5 if cx < 16 else -0.5), 14.5, 1.9, EYE, 0)
		_stamp_front(c, size, cx + (1.3 if cx < 16 else -1.3), 15.8, 0.7, EYE_WHITE, 0)
	for x in range(7, 13):                                                  # brows
		_stamp_front(c, size, x + 0.5, 20.0, 0.6, FUR_DARK, 0)
	for x in range(19, 25):
		_stamp_front(c, size, x + 0.5, 20.0, 0.6, FUR_DARK, 0)
	for x in range(15, 18):                                                 # nose
		for y in range(9, 11):
			_stamp_front(c, size, x + 0.5, y + 0.5, 0.6, NOSE, 0)
	for x in range(12, 21):                                                 # smile
		_stamp_front(c, size, x + 0.5, 6.5, 0.6, FUR_DARK, 0)
	_stamp_front(c, size, 11.5, 7.5, 0.6, FUR_DARK, 0)
	_stamp_front(c, size, 20.5, 7.5, 0.6, FUR_DARK, 0)
	_repaint_shell(c, size, FUR, STRAP, 23, 26)                             # goggle strap around the forehead
	_repaint_shell(c, size, FACE, STRAP, 23, 26)
	for cx in [10.5, 21.5]:                                                 # goggles on the forehead: rims, glass, glint
		_stamp_front(c, size, cx, 25.0, 4.6, RIM, 2)
		_stamp_front(c, size, cx, 25.0, 3.2, LENS, 0)
		_stamp_front(c, size, cx + 1.2, 26.2, 0.7, EYE_WHITE, 0)
	for x in range(14, 18):                                                 # bridge
		_stamp_front(c, size, x + 0.5, 25.0, 0.8, RIM, 1)
	for t in [[15, 27], [17, 28], [13, 27], [19, 27], [16, 29]]:            # fur tuft
		_box(c, size, t[0], t[1], 14, t[0] + 2, t[1] + 2, 18, FUR_DARK)
	return c


## Repaints cells of `from` inside an ellipsoid to `to` (keeps the shape).
static func _repaint_inside(c: PackedInt32Array, size: Vector3i, cx: float, cy: float, cz: float, rx: float, ry: float, rz: float, from: int, to: int) -> void:
	for z in size.z:
		for y in size.y:
			for x in size.x:
				if _cell_get(c, size, x, y, z) != from + 1:
					continue
				var dx := (x + 0.5 - cx) / rx
				var dy := (y + 0.5 - cy) / ry
				var dz := (z + 0.5 - cz) / rz
				if dx * dx + dy * dy + dz * dz <= 1.0:
					_cell_set(c, size, x, y, z, to)


static func _repaint_cell(c: PackedInt32Array, size: Vector3i, x: int, y: int, from: int, to: int) -> void:
	for z in size.z:
		if _cell_get(c, size, x, y, z) == from + 1:
			_cell_set(c, size, x, y, z, to)


## Front (lowest z) solid cell of a column, -1 when the column is empty.
static func _front(c: PackedInt32Array, size: Vector3i, x: int, y: int) -> int:
	for z in size.z:
		if _cell_get(c, size, x, y, z) != 0:
			return z
	return -1


## Stamps a disc onto the face: depth 0 repaints the front cell of each column,
## depth n adds n cells in front of it (features that stand proud of the skin).
static func _stamp_front(c: PackedInt32Array, size: Vector3i, cx: float, cy: float, r: float, tile: int, depth: int) -> void:
	for y in range(int(floor(cy - r)), int(ceil(cy + r)) + 1):
		for x in range(int(floor(cx - r)), int(ceil(cx + r)) + 1):
			var dx := x + 0.5 - cx
			var dy := y + 0.5 - cy
			if dx * dx + dy * dy > r * r:
				continue
			var zf := _front(c, size, x, y)
			if zf < 0:
				continue
			if depth == 0:
				_cell_set(c, size, x, y, zf, tile)
			else:
				for d in range(1, depth + 1):
					_cell_set(c, size, x, y, zf - d, tile)


static func _torso_cells() -> PackedInt32Array:
	var size := Vector3i(14, 24, 9)
	var c := _cells(size)
	_box(c, size, 0, 6, 0, 14, 24, 9, COAT)                      # chest and belly
	_box(c, size, 0, 0, 1, 14, 6, 9, COAT)                       # coat skirt below the belt
	_box(c, size, 5, 0, 0, 9, 6, 9, 0)                           # open front of the skirt
	_box(c, size, 6, 0, 1, 8, 6, 9, 0)                           # split at the back
	_box(c, size, 0, 6, 0, 1, 24, 9, COAT_SHADE)                 # shaded sides
	_box(c, size, 13, 6, 0, 14, 24, 9, COAT_SHADE)
	for y in range(18, 24):                                      # V collar
		var w := 24 - y
		_box(c, size, 7 - w, y, 0, 7 + w, y + 1, 1, COLLAR)
	_box(c, size, 0, 22, 0, 14, 24, 9, COLLAR)                   # collar ring
	_box(c, size, 2, 8, 0, 6, 12, 1, COAT_SHADE)                 # pockets
	_box(c, size, 8, 8, 0, 12, 12, 1, COAT_SHADE)
	_box(c, size, 3, 11, 0, 5, 12, 1, ACCENT)                    # pen clip in the pocket
	for y in [9, 12, 15]:                                        # buttons
		_box(c, size, 6, y, 0, 8, y + 1, 1, ACCENT)
	_box(c, size, 0, 6, 0, 14, 8, 9, BELT)                       # belt
	_box(c, size, 5, 6, 0, 9, 8, 1, BUCKLE)
	_box(c, size, 1, 6, 0, 4, 8, 1, ACCENT)                      # gadget pouch
	_box(c, size, 4, 16, 8, 10, 21, 9, ACCENT)                   # emblem on the back
	_box(c, size, 5, 17, 8, 9, 20, 9, COAT)
	_box(c, size, 6, 18, 8, 8, 19, 9, ACCENT)
	return c


static func _arm_cells(right: bool) -> PackedInt32Array:
	var size := Vector3i(6, 22, 6)
	var c := _cells(size)
	_box(c, size, 0, 7, 0, 6, 22, 6, COAT)                       # sleeve
	_box(c, size, 0, 7, 0, 1, 22, 6, COAT_SHADE)
	_box(c, size, 0, 6, 0, 6, 8, 6, COLLAR)                      # cuff
	_box(c, size, 1, 2, 1, 5, 6, 5, FUR)                         # hand
	_box(c, size, 1, 3, 0, 5, 5, 1, FACE)                        # palm side lighter
	for fx in [1, 3, 4]:                                         # fingers
		_box(c, size, fx, 0, 1, fx + 1, 2, 3, FUR)
	_box(c, size, 0 if right else 5, 2, 2, 1 if right else 6, 4, 4, FUR)   # thumb
	_box(c, size, 1, 0, 1, 5, 2, 3, FUR)
	return c


static func _leg_cells() -> PackedInt32Array:
	var size := Vector3i(6, 24, 7)
	var c := _cells(size)
	_box(c, size, 0, 6, 1, 6, 24, 7, TROUSERS)                   # trouser leg
	_box(c, size, 0, 6, 1, 1, 24, 7, COLLAR)                     # dark seam
	_box(c, size, 0, 0, 0, 6, 6, 7, BOOTS)                       # boot with toe cap (z = 0 is the front)
	_box(c, size, 0, 5, 1, 6, 6, 7, FUR_DARK)                    # boot rim
	_box(c, size, 1, 2, 0, 5, 3, 1, ACCENT)                      # lace light
	return c


static func _tail_cells() -> PackedInt32Array:
	var size := Vector3i(4, 20, 14)
	var c := _cells(size)
	var n := 22
	for i in n:
		var t := float(i) / (n - 1)
		var z := 1.0 + 11.0 * sin(t * PI * 0.5)
		var y := 1.0 + 17.0 * (1.0 - cos(t * PI * 0.5))
		var tile := FUR if t < 0.8 else FUR_DARK
		_ellipsoid(c, size, 2.0, y, z, 1.4, 1.4, 1.4, tile)
	return c
