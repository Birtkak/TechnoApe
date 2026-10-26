class_name OceanLife
extends Node3D

## Fish around the player (ADR 0027). Pure presentation: nothing here is a
## voxel, saved or simulated. Kelp and seagrass are voxels now, laid by FineGen
## (ADR 0047). A native map (WorldGen.water_floor_map: sea floor and water level
## per column, frozen water excluded) is rebuilt on a worker when the player has
## moved REBUILD_V voxels; from it:
##   fish      a few schools of 8-14 fish that swim between random spots in the
##             water around the player; a school only steers to points that are
##             water voxels, so it never swims into the ground. One MultiMesh,
##             transforms set per frame (~100 fish), the tail beats in the shader.

const MAP_N := 64             # columns per side of the water map
const MAP_STEP := int(1.0 / Settings.VOXEL_M)   # voxels per column (1 m): 64 m across
const REBUILD_V := 12.0 / Settings.VOXEL_M   # rebuild the map when the camera moved this far (12 m, in voxels)
const SCHOOLS := 6
const FISH_PER_SCHOOL := 12
const SCHOOL_SPEED := 1.2 / Settings.VOXEL_M   # voxels/s (1.2 m/s)
const SCHOOL_COLORS := [Color(0.72, 0.76, 0.80), Color(0.95, 0.55, 0.15), Color(0.35, 0.55, 0.85), Color(0.85, 0.80, 0.35), Color(0.55, 0.70, 0.55)]

var world: World
var camera: Camera3D
var stats := {"schools": 0, "maps": 0}

var _fish: MultiMeshInstance3D
var _task := -1
var _map := PackedInt32Array()
var _map_x0 := 0
var _map_z0 := 0
var _pending_x0 := 0
var _pending_z0 := 0
var _center := Vector3(INF, INF, INF)
var _spots: Array[Vector3] = []    # mid-water points fish can swim to
var _schools: Array = []           # {pos, vel, target, color, offsets, phase}
var _rng := RandomNumberGenerator.new()
var _check_t := 0.0
var _row := -1                     # swim spots in progress: next map row (ROWS_PER_FRAME per frame)
const ROWS_PER_FRAME := 8


func setup(p_world: World, p_camera: Camera3D) -> void:
	world = p_world
	camera = p_camera


func _ready() -> void:
	_rng.randomize()
	_fish = MultiMeshInstance3D.new()
	_fish.name = "Fish"
	var fm := MultiMesh.new()
	fm.transform_format = MultiMesh.TRANSFORM_3D
	fm.use_colors = true
	fm.mesh = _fish_mesh()
	fm.instance_count = SCHOOLS * FISH_PER_SCHOOL
	fm.visible_instance_count = 0
	_fish.multimesh = fm
	_fish.material_override = _material(_FISH_SHADER)
	_fish.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	add_child(_fish)


func _exit_tree() -> void:
	if _task >= 0:
		WorkerThreadPool.wait_for_task_completion(_task)
		_task = -1


func _process(delta: float) -> void:
	var _pt0 := Time.get_ticks_usec()
	_process_timed(delta)
	DebugCapture.note_proc("ocean", _pt0)


func _process_timed(delta: float) -> void:
	if world == null or camera == null or world.generator == null:
		return
	_update_map()
	_update_fish(delta)


# ---------------- the water map ----------------

func _update_map() -> void:
	if _task >= 0:
		if not WorkerThreadPool.is_task_completed(_task):
			return
		WorkerThreadPool.wait_for_task_completion(_task)
		_task = -1
		_map_x0 = _pending_x0
		_map_z0 = _pending_z0
		stats["maps"] += 1
		_row = 0
		_spots.clear()
		return
	if _row >= 0:
		_place_spot_rows()
		return
	var p := camera.global_position
	if Vector2(p.x - _center.x, p.z - _center.z).length() < REBUILD_V:
		return
	_center = p
	_pending_x0 = int(floor(p.x)) - MAP_N * MAP_STEP / 2
	_pending_z0 = int(floor(p.z)) - MAP_N * MAP_STEP / 2
	var gen: FineGen = world.generator.gen
	var x0 := _pending_x0
	var z0 := _pending_z0
	_task = WorkerThreadPool.add_task(func() -> void:
		_map = gen.water_floor_map(x0, z0, MAP_N, MAP_STEP))


## A few map rows per frame (a whole map is 4096 columns: a frame spike at once): the mid-water
## points fish swim to.
func _place_spot_rows() -> void:
	var last := mini(MAP_N, _row + ROWS_PER_FRAME)
	for j in range(_row, last):
		for i in MAP_N:
			var v: int = _map[j * MAP_N + i] if _map.size() == MAP_N * MAP_N else -1
			if v < 0:
				continue
			var floor_y := v & 0xFFFF
			var depth := (v >> 16) - floor_y
			var wx := _map_x0 + i * MAP_STEP
			var wz := _map_z0 + j * MAP_STEP
			if depth >= 3 and hash(Vector2i(wx, wz)) % 7 == 0:
				_spots.append(Vector3(wx + 0.5, floor_y + depth * 0.45, wz + 0.5))
	_row = -1 if last >= MAP_N else last


# ---------------- fish ----------------

func _is_water(p: Vector3) -> bool:
	var m := GameMaterials.material_of_voxel(world.get_voxel(Vector3i(p.floor())))
	return m == GameMaterials.WATER or m == GameMaterials.WATER_STATIC


func _update_fish(delta: float) -> void:
	_check_t -= delta
	var cam := camera.global_position
	if _check_t <= 0.0:
		_check_t = 0.5
		# drop schools far from the camera, start new ones where there is water
		for s in _schools.duplicate():
			if Vector3(s["pos"]).distance_to(cam) > MAP_N * MAP_STEP * 0.6:
				_schools.erase(s)
		if _schools.size() < SCHOOLS and not _spots.is_empty():
			var start: Vector3 = _spots[_rng.randi_range(0, _spots.size() - 1)]
			if _is_water(start):
				var offs: Array[Vector3] = []
				for k in _rng.randi_range(8, FISH_PER_SCHOOL):
					offs.append(Vector3(_rng.randf_range(-1.6, 1.6), _rng.randf_range(-0.6, 0.6), _rng.randf_range(-1.6, 1.6)))
				_schools.append({"pos": start, "vel": Vector3.ZERO, "target": start, "color": SCHOOL_COLORS[_rng.randi_range(0, SCHOOL_COLORS.size() - 1)],
					"offsets": offs, "phase": _rng.randf() * TAU})
		for s in _schools:
			var to: Vector3 = Vector3(s["target"]) - Vector3(s["pos"])
			if to.length() < 1.0 or not _is_water(s["pos"]):
				_retarget(s)
	stats["schools"] = _schools.size()
	var mm := _fish.multimesh
	var n := 0
	var t := Time.get_ticks_msec() / 1000.0
	for s in _schools:
		var pos: Vector3 = s["pos"]
		var to: Vector3 = Vector3(s["target"]) - pos
		var want := to.normalized() * SCHOOL_SPEED if to.length() > 0.01 else Vector3.ZERO
		var vel: Vector3 = Vector3(s["vel"]).move_toward(want, 3.0 * delta)
		var next := pos + vel * delta
		if _is_water(next):
			pos = next
		else:
			vel = Vector3.ZERO
			_retarget(s)
		s["pos"] = pos
		s["vel"] = vel
		var heading := vel if vel.length() > 0.05 else Vector3.FORWARD
		var b := Basis.looking_at(heading.normalized(), Vector3.UP if absf(heading.normalized().y) < 0.95 else Vector3.RIGHT)
		var ph: float = s["phase"]
		for o in s["offsets"]:
			var off: Vector3 = o
			var wob := Vector3(sin(t * 1.3 + off.x * 3.0 + ph), sin(t * 0.9 + off.z * 2.0) * 0.4, cos(t * 1.1 + off.y * 4.0 + ph)) * 0.25
			mm.set_instance_transform(n, Transform3D(b, pos + b * off + wob))
			mm.set_instance_color(n, s["color"])
			n += 1
	mm.visible_instance_count = n


## A new spot to swim to: one of the map's mid-water points near the school that
## is still water (the world may have changed), else stay.
func _retarget(s: Dictionary) -> void:
	for _i in 6:
		if _spots.is_empty():
			return
		var c: Vector3 = _spots[_rng.randi_range(0, _spots.size() - 1)]
		if c.distance_to(s["pos"]) < 24.0 and _is_water(c):
			s["target"] = c
			return


# ---------------- shaders ----------------

const _FISH_SHADER := """
shader_type spatial;
// fish: the back half beats side to side, the back is darker than the belly
varying float v_up;
void vertex() {
	v_up = VERTEX.y;
	float back = clamp(VERTEX.z * 2.6, 0.0, 1.0);
	vec3 o = MODEL_MATRIX[3].xyz;
	VERTEX.x += sin(TIME * 14.0 + o.x * 5.0 + o.z * 3.0) * 0.05 * back;
}
void fragment() {
	ALBEDO = COLOR.rgb * mix(1.15, 0.6, smoothstep(-0.04, 0.06, v_up));
	ROUGHNESS = 0.35;
	SPECULAR = 0.6;
}
"""


## A small fish: a diamond-shaped body (tapered at the snout and the tail stem)
## and a forked tail fin, 0.4 voxels (20 cm) long, facing -Z (Basis.looking_at).
static func _fish_mesh() -> ArrayMesh:
	var st := SurfaceTool.new()
	st.begin(Mesh.PRIMITIVE_TRIANGLES)
	var w := 0.035
	# body ring at the middle, points at snout (-z) and tail stem (+z)
	var snout := Vector3(0, 0, -0.18)
	var stem := Vector3(0, 0, 0.12)
	var ring := [Vector3(0, 0.07, -0.02), Vector3(w, 0, -0.02), Vector3(0, -0.06, -0.02), Vector3(-w, 0, -0.02)]
	for k in 4:
		var a: Vector3 = ring[k]
		var b: Vector3 = ring[(k + 1) % 4]
		for tri in [[snout, a, b], [stem, b, a]]:
			var nrm: Vector3 = (tri[1] - tri[0]).cross(tri[2] - tri[0]).normalized()
			for v in tri:
				st.set_normal(nrm)
				st.add_vertex(v)
	# tail fin: two thin triangles from the stem, both faces
	for tip in [Vector3(0, 0.07, 0.22), Vector3(0, -0.07, 0.22)]:
		var tri := [stem, tip, Vector3(0, 0, 0.17)]
		for side in [1.0, -1.0]:
			for i in 3:
				st.set_normal(Vector3(side, 0, 0))
				st.add_vertex(tri[i] if side > 0.0 else tri[2 - i])
	return st.commit()


static func _material(code: String) -> ShaderMaterial:
	var sh := Shader.new()
	sh.code = code
	var m := ShaderMaterial.new()
	m.shader = sh
	return m


func summary() -> Dictionary:
	return stats.duplicate()
