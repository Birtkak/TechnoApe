class_name PlaytestTools
extends Node3D

## Playtest gadgets (/kit gadgets): debug items for testing the simulation, rigid pieces and
## performance, not progression. An item's "playtest" entry in items.json names its kind:
##   laser    hold left click: the beam cuts laser_depth_m through anything breakable, past the first
##            solid voxel; the brush size (Ctrl + wheel) is its width; what the cuts free falls (P12)
##   crater   right click: a crater_radius_m blast where you look (sneak: twice that); what it frees falls
##   bomb     right click: a ball of "material" ("radius_m") dropped where you look (/blast with a material)
##   tree     right click on the ground: a straight test tree (bark over wood, a leaf crown) to cut
##   measure  right click two voxels: the chat shows the distance between them
## Every cut is native (SimBubble.blast = SimWorld.fill_sphere, fill_box): GDScript only lays the
## spheres along the beam and the seed cells for the rigid piece check.

const BEAM_RADIUS_M := 0.03
const SEED_RING := 8           # seed cells around each sphere of a cut, for the rigid piece check

var main: Node
var stats := {"laser_pulses": 0, "laser_spheres": 0, "laser_ms_max": 0.0, "laser_checks": 0, "craters": 0, "bombs": 0, "trees": 0}
var _beam: MeshInstance3D
var _beam_mat: StandardMaterial3D
var _pulse_t := 0.0
var _check_t := 0.0
var _seeds := {}               # Vector3i -> true: cut cells waiting for the next rigid piece check
var _measure_a := Vector3i(1 << 30, 0, 0)
var _script_t := 0.0           # /laser: seconds the beam still fires on its own
var _script_turn := 0.0        # and radians per second it turns the view meanwhile


func setup(p_main: Node) -> void:
	main = p_main
	_beam_mat = StandardMaterial3D.new()
	_beam_mat.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	_beam_mat.albedo_color = Color(1.0, 0.25, 0.15)
	_beam_mat.emission_enabled = true
	_beam_mat.emission = Color(1.0, 0.2, 0.1)
	_beam_mat.emission_energy_multiplier = 4.0
	var mesh := CylinderMesh.new()
	mesh.top_radius = 1.0
	mesh.bottom_radius = 1.0
	mesh.height = 1.0
	mesh.radial_segments = 6
	mesh.rings = 1
	mesh.material = _beam_mat
	_beam = MeshInstance3D.new()
	_beam.name = "LaserBeam"
	_beam.mesh = mesh
	_beam.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	_beam.visible = false
	add_child(_beam)


## MaterialWarmup draws these under the loading screen (a first draw mid-game compiles the pipeline).
func warm_materials() -> Array:
	return [_beam_mat]


static func kind_of(def: Dictionary) -> String:
	return str(def.get("playtest", {}).get("kind", ""))


# ---------------- laser ----------------

## /laser: fire for `seconds` without the mouse, turning across `sweep_deg` (it starts half of it to
## the left), so scripted checks can cut through a trunk.
func fire_for(seconds: float, sweep_deg: float) -> void:
	_script_t = seconds
	_script_turn = -deg_to_rad(sweep_deg) / seconds
	var p: Player = main.player
	p.set_look(p._yaw + deg_to_rad(sweep_deg) * 0.5, p._pitch)


func _process(delta: float) -> void:
	if _script_t <= 0.0:
		return
	_script_t -= delta
	var p: Player = main.player
	p.set_look(p._yaw + _script_turn * delta, p._pitch)
	var cam: Camera3D = p.camera
	_fire(_script_t > 0.0, cam.global_position, -cam.global_transform.basis.z, delta, p.brush_size)


## Every frame while a laser is held. `firing`: the break button is down.
func laser(firing: bool, from: Vector3, dir: Vector3, delta: float, brush_size: int) -> void:
	if _script_t > 0.0:
		return   # /laser has the beam
	_fire(firing, from, dir, delta, brush_size)


func _fire(firing: bool, from: Vector3, dir: Vector3, delta: float, brush_size: int) -> void:
	if not firing:
		_beam.visible = false
		_pulse_t = 0.0
		_flush_check(from)
		return
	var hit: VoxelRaycastResult = main.world.raycast(from, dir, Settings.m2v(Settings.laser_range_m))
	var end := (Vector3(hit.position) + Vector3(0.5, 0.5, 0.5)) if hit else from + dir * Settings.m2v(Settings.laser_range_m)
	_show_beam(from, end)
	_pulse_t -= delta
	if hit and _pulse_t <= 0.0:
		_pulse_t = Settings.laser_pulse_s
		_cut(end, dir, maxf(brush_size * 0.5, 1.0))
	_check_t -= delta
	if _check_t <= 0.0:
		_flush_check(from)


## The beam from just below and right of the eye (the hand) to `end`.
func _show_beam(eye: Vector3, end: Vector3) -> void:
	var cam: Camera3D = main.player.camera
	var b := cam.global_transform.basis
	var start := eye + b.x * Settings.m2v(0.2) - b.y * Settings.m2v(0.15) - b.z * Settings.m2v(0.3)
	var v := end - start
	if v.length() < 0.01:
		_beam.visible = false
		return
	var y := v.normalized()
	var x := y.cross(Vector3.UP if absf(y.y) < 0.99 else Vector3.RIGHT).normalized()
	var z := x.cross(y)
	var r := Settings.m2v(BEAM_RADIUS_M)
	_beam.global_transform = Transform3D(Basis(x * r, y * v.length(), z * r), start + v * 0.5)
	_beam.visible = true


## One pulse: spheres of `r` voxels every 0.75 r along the beam, laser_depth_m deep from `at`.
func _cut(at: Vector3, dir: Vector3, r: float) -> void:
	var t0 := Time.get_ticks_usec()
	var first: int = main.get_world_voxel(Vector3i(at.floor()))
	var depth := Settings.m2v(Settings.laser_depth_m)
	var step := maxf(r * 0.75, 1.0)
	var u := dir.cross(Vector3.UP if absf(dir.y) < 0.99 else Vector3.RIGHT).normalized()
	var w := dir.cross(u)
	var n := 0
	var t := 0.0
	while t <= depth:
		var c := at + dir * t
		if main.bubble.blast(c, r) > 0:
			_seed_ring(c, u, w, r)
		n += 1
		t += step
	stats["laser_pulses"] += 1
	stats["laser_spheres"] += n
	stats["laser_ms_max"] = maxf(stats["laser_ms_max"], (Time.get_ticks_usec() - t0) / 1000.0)
	if main.fx and first > 0:
		main.fx.block_break(Vector3i(at.floor()), first)


## Emptied cells just inside a sphere's rim, around the beam: their faces touch what is left on
## either side of the cut, where the rigid piece check starts its fills.
func _seed_ring(c: Vector3, u: Vector3, w: Vector3, r: float) -> void:
	_seeds[Vector3i(c.floor())] = true
	var rr := maxf(r - 0.5, 0.0)
	for k in SEED_RING:
		var a := TAU * k / SEED_RING
		_seeds[Vector3i((c + (u * cos(a) + w * sin(a)) * rr).floor())] = true


func _flush_check(from: Vector3) -> void:
	_check_t = Settings.laser_check_s
	if _seeds.is_empty():
		return
	if main.pieces:
		main.pieces.check(_seeds.keys(), from)
		stats["laser_checks"] += 1
	_seeds.clear()


# ---------------- right click gadgets ----------------

## A right click with a gadget. True when it did something (the player swings, a bomb is used up).
func use(def: Dictionary, hit: VoxelRaycastResult, from: Vector3, dir: Vector3, sneaking: bool) -> bool:
	var pt: Dictionary = def.get("playtest", {})
	match str(pt.get("kind", "")):
		"crater":
			return _crater(hit, from, Settings.crater_radius_m * (2.0 if sneaking else 1.0))
		"bomb":
			return _bomb(hit, from, dir, str(pt.get("material", "water")), float(pt.get("radius_m", 2.0)))
		"tree":
			return _tree(hit, sneaking)
		"measure":
			return _measure(hit)
	return false


func _crater(hit: VoxelRaycastResult, from: Vector3, radius_m: float) -> bool:
	if hit == null:
		return false
	var at := Vector3(hit.position) + Vector3(0.5, 0.5, 0.5)
	var first: int = main.get_world_voxel(hit.position)
	var r := Settings.m2v(radius_m)
	if main.bubble.blast(at, r) <= 0:
		return false
	stats["craters"] += 1
	if main.fx and first > 0:
		main.fx.block_break(hit.position, first)
	if main.pieces:
		# seeds on the crater's rim (a Fibonacci sphere): a trunk standing in it is cut wherever it was
		var seeds := []
		var count := clampi(int(4.0 * PI * r * r / 6.0), 64, 4096)
		for i in count:
			var y := 1.0 - 2.0 * (i + 0.5) / count
			var ring := sqrt(1.0 - y * y)
			var a := i * 2.39996323   # the golden angle
			seeds.append(Vector3i((at + Vector3(cos(a) * ring, y, sin(a) * ring) * (r - 0.5)).floor()))
		main.pieces.check(seeds, from)
	return true


func _bomb(hit: VoxelRaycastResult, from: Vector3, dir: Vector3, material: String, radius_m: float) -> bool:
	var mat := GameMaterials.id_of(material)
	if mat <= 0:
		return false
	var r := Settings.m2v(radius_m)
	var at := (Vector3(hit.position) + Vector3(0.5, 0.5, 0.5)) if hit else from + dir * Settings.m2v(8.0)
	at.y += r + 2.0   # a ball above the spot, so it falls and spreads (like /blast)
	if main.bubble.blast(at, r, mat) <= 0:
		return false
	stats["bombs"] += 1
	return true


## A straight trunk on the aimed ground, bark over wood, with a leaf ball on top: 1 m thick and
## 8 m tall, or 2 m and 16 m when sneaking. The crown goes in first, the trunk through it.
func _tree(hit: VoxelRaycastResult, giant: bool) -> bool:
	if hit == null or hit.previous_position != hit.position + Vector3i.UP:
		return false   # on a top face
	var w := roundi(Settings.m2v(2.0 if giant else 1.0))
	var h := roundi(Settings.m2v(16.0 if giant else 8.0))
	var crown := Settings.m2v(4.0 if giant else 2.5)
	var base: Vector3i = hit.previous_position - Vector3i(w / 2, 0, w / 2)
	var top := Vector3(base) + Vector3(w * 0.5, h - crown * 0.3, w * 0.5)
	var bubble: SimBubble = main.bubble
	if bubble.get_voxel(base) < 0 or bubble.get_voxel(base + Vector3i(w, h + ceili(crown), w)) < 0:
		main.chat.add_message("Instant Tree: grow it inside the simulated area around you", ChatBox.ERROR)
		return false
	bubble.blast(top, crown, GameMaterials.LEAVES)
	if bubble.fill_box(base, base + Vector3i(w, h, w), GameMaterials.OAK_BARK) < 0:
		return false
	bubble.fill_box(base + Vector3i(1, 0, 1), base + Vector3i(w - 1, h, w - 1), GameMaterials.WOOD)
	stats["trees"] += 1
	return true


func _measure(hit: VoxelRaycastResult) -> bool:
	if hit == null:
		return false
	var p: Vector3i = hit.position
	if _measure_a.x == 1 << 30:
		_measure_a = p
		main.chat.add_message("Tape measure: from (%d, %d, %d), right click the other end" % [p.x, p.y, p.z])
		return true
	var d := p - _measure_a
	var span := (d.abs() + Vector3i.ONE)   # both ends included, like /fill
	main.chat.add_message("Tape measure: %.2f m apart (dx %.2f, dy %.2f, dz %.2f m); box %d x %d x %d voxels = %.2f x %.2f x %.2f m" % [
		Settings.v2m(Vector3(d).length()), Settings.v2m(d.x), Settings.v2m(d.y), Settings.v2m(d.z),
		span.x, span.y, span.z, Settings.v2m(span.x), Settings.v2m(span.y), Settings.v2m(span.z)])
	_measure_a = Vector3i(1 << 30, 0, 0)
	return true
