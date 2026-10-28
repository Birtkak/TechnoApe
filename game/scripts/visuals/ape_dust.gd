class_name ApeDust
extends MultiMeshInstance3D

## The apetion falling apart and being put back together (ADR 0098). Grains sampled from the ApeModel's
## own mesh (vertex positions, palette colour from the UV) as small cubes in one MultiMesh.
## crumble: from the top down the body turns to powder and falls, piling at the feet.
## assemble: the grains swirl in out of thin air, magenta (Gelvin light) cooling to the body's colours,
## and land on the body. `finished` fires when it is done; the caller shows the model and frees this.

signal finished

const GRAINS := 700
const GRAIN := 0.42        # voxels (~5 cm)
const CRUMBLE_S := 2.2
const ASSEMBLE_S := 2.8
const GELVIN := Color(1.0, 0.25, 0.85)

var _target: PackedVector3Array   # per grain: where it sits on the body (local to this node)
var _colour: PackedColorArray
var _pos: PackedVector3Array
var _vel: PackedVector3Array
var _delay: PackedFloat32Array
var _start: PackedVector3Array    # assemble: where each grain comes from
var _assemble := false
var _t := 0.0
var _blip_t := 0.0                # assemble: when the next Gelvin blip plays


## Grains off the model's meshes, posed as they are now, relative to `body` (the player).
func _sample(model: ApeModel, body: Node3D) -> void:
	var inv := body.global_transform.affine_inverse()
	var verts: Array = []
	for mi in model.get_children():
		if not (mi is MeshInstance3D) or (mi as MeshInstance3D).mesh == null:
			continue
		var a: Array = (mi as MeshInstance3D).mesh.surface_get_arrays(0)
		var xf := inv * (mi as MeshInstance3D).global_transform
		verts.append([xf, a[Mesh.ARRAY_VERTEX], a[Mesh.ARRAY_TEX_UV]])
	var rng := RandomNumberGenerator.new()
	for i in GRAINS:
		var part: Array = verts[rng.randi() % verts.size()]
		var vs: PackedVector3Array = part[1]
		var k := rng.randi() % vs.size()
		var uv: Vector2 = (part[2] as PackedVector2Array)[k]
		var tile := clampi(int(uv.x * ApeModel.ATLAS.x), 0, ApeModel.ATLAS.x - 1) + clampi(int(uv.y * ApeModel.ATLAS.y), 0, ApeModel.ATLAS.y - 1) * ApeModel.ATLAS.x
		var p: Array = ApeModel.PALETTE[mini(tile, ApeModel.PALETTE.size() - 1)]
		_target.append((part[0] as Transform3D) * vs[k])
		_colour.append(Color8(p[0], p[1], p[2]))


func _setup(model: ApeModel, body: Node3D) -> void:
	_sample(model, body)
	multimesh = _multimesh(_target.size())
	cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	global_transform = Transform3D(body.global_transform.basis, body.global_transform.origin)


static var _box: BoxMesh


static func _multimesh(count: int) -> MultiMesh:
	if _box == null:
		_box = BoxMesh.new()
		_box.size = Vector3.ONE * GRAIN
		var mat := StandardMaterial3D.new()
		mat.vertex_color_use_as_albedo = true
		mat.roughness = 0.9
		_box.material = mat
	var mm := MultiMesh.new()
	mm.transform_format = MultiMesh.TRANSFORM_3D
	mm.use_colors = true
	mm.mesh = _box
	mm.instance_count = count
	return mm


## One tiny grain in front of `camera` for half a second under the loading screen: the grains'
## pipeline compiles then, not on the first death (a 50 ms frame).
static func warm(camera: Camera3D) -> void:
	var mmi := MultiMeshInstance3D.new()
	mmi.multimesh = _multimesh(1)
	mmi.multimesh.set_instance_transform(0, Transform3D(Basis.from_scale(Vector3.ONE * 0.002), Vector3.ZERO))
	mmi.multimesh.set_instance_color(0, Color.WHITE)
	mmi.extra_cull_margin = 16384.0
	camera.add_child(mmi)
	mmi.position = Vector3(0.5, -0.004, -1.0)
	camera.get_tree().create_timer(0.5).timeout.connect(mmi.queue_free)


static func crumble(model: ApeModel, body: Node3D, parent: Node) -> ApeDust:
	var d := ApeDust.new()
	parent.add_child(d)
	d._setup(model, body)
	var rng := RandomNumberGenerator.new()
	var top := 0.0
	for p in d._target:
		top = maxf(top, p.y)
	for i in d._target.size():
		d._pos.append(d._target[i])
		d._vel.append(Vector3(rng.randf_range(-1, 1), rng.randf_range(0, 1.5), rng.randf_range(-1, 1)))
		d._delay.append((1.0 - d._target[i].y / maxf(top, 0.01)) * 0.9 + rng.randf() * 0.25)   # the head goes first
	model.visible = false
	d._draw(0.0)
	return d


static func assemble(model: ApeModel, body: Node3D, parent: Node) -> ApeDust:
	var d := ApeDust.new()
	parent.add_child(d)
	d._setup(model, body)
	d._assemble = true
	var rng := RandomNumberGenerator.new()
	for i in d._target.size():
		var a := rng.randf() * TAU
		var r := Settings.m2v(rng.randf_range(1.5, 3.0))
		d._start.append(Vector3(cos(a) * r, Settings.m2v(rng.randf_range(0.0, 3.0)), sin(a) * r))
		d._delay.append(d._target[i].y / Settings.m2v(1.8) * 0.8 + rng.randf() * 0.3)   # built from the feet up
		d._pos.append(d._start[i])
	model.visible = false
	d._draw(0.0)
	return d


func _process(delta: float) -> void:
	_t += delta
	if _assemble:
		_draw(0.0)
		if _t >= _blip_t and _t < ASSEMBLE_S:   # the machine at work: blips that climb as the body fills in
			Sound.play("player/gelvin_blip", Vector3.INF, {"pitch": 0.8 + 0.6 * _t / ASSEMBLE_S})
			_blip_t = _t + randf_range(0.05, 0.14)
		if _t >= ASSEMBLE_S + 0.2:
			Sound.play("player/gelvin_done")
			set_process(false)
			finished.emit()
		return
	var g := Settings.m2v(9.8)
	for i in _pos.size():
		if _t < _delay[i]:
			continue
		var v := _vel[i]
		v.y -= g * delta
		var p := _pos[i] + v * delta
		if p.y <= GRAIN * 0.5:   # the feet's floor: settle into a pile
			p.y = GRAIN * 0.5
			v = Vector3(v.x * 0.3, 0.0, v.z * 0.3)
		_pos[i] = p
		_vel[i] = v
	_draw(delta)
	if _t >= CRUMBLE_S:
		set_process(false)
		finished.emit()


func _draw(_delta: float) -> void:
	var mm := multimesh
	var span := ASSEMBLE_S - 1.1
	for i in _pos.size():
		var p := _pos[i]
		var c := _colour[i]
		var s := 1.0
		if _assemble:
			var e := clampf((_t - _delay[i]) / span, 0.0, 1.0)
			e = 1.0 - pow(1.0 - e, 3.0)
			var off := _start[i] - _target[i]
			p = _target[i] + off.rotated(Vector3.UP, (1.0 - e) * 3.0 * PI) * (1.0 - e)   # spiral in
			s = clampf(_t * 3.0 - _delay[i], 0.0, 1.0)   # out of thin air
			c = GELVIN.lerp(c, e)
		elif _t >= _delay[i]:
			c = c.darkened(clampf((_t - _delay[i]) * 0.4, 0.0, 0.35))   # powder dulls as it falls
		mm.set_instance_transform(i, Transform3D(Basis().scaled(Vector3.ONE * s), p))
		mm.set_instance_color(i, c)
