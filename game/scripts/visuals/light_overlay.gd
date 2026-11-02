class_name LightOverlay
extends MultiMeshInstance3D

## F7 (ADR 0130, like Minecraft's Light Overlay mod): marks the block tops within RADIUS_M that light does
## not reach. Red: no torch, fire or lamp light and no open sky (dark by day and night); yellow: only the
## sky lights it (dark at night). A source gives 15 at its flame, falling linearly to 0 at its light's
## range (TorchLights.sources); a spot at LIT or more counts as lit.
## shortcut: no shadows (a wall does not stop the level), upgrade when darkness drives gameplay (spawning).
## Columns are found with native raycasts within BUDGET_US a frame; the marks are rebuilt after each pass.

const RADIUS_M := 8.0
const LIT := 8
const BUDGET_US := 500           # per frame: 128 columns a frame took the cave p99 from 2.9 to 11.9 ms
const SKY_M := 32.0              # open sky: nothing solid this far up
const RED := Color(0.95, 0.15, 0.1, 0.6)
const YELLOW := Color(1.0, 0.85, 0.1, 0.5)

var main: Node                    # Main: world, player, torch_lights
var _cols: Array[Vector2i] = []   # the block columns of the pass under way
var _next := 0
var _marks := {}                  # column -> [top of the block (voxels), colour]
var _seen := {}                   # columns visited this pass
var _sources: Array = []
var stats := {"marks": 0, "ms_max": 0.0, "passes": 0}


func _ready() -> void:
	multimesh = MultiMesh.new()
	multimesh.transform_format = MultiMesh.TRANSFORM_3D
	multimesh.use_colors = true
	var q := QuadMesh.new()
	q.size = Vector2.ONE * Settings.m2v(0.36)
	q.orientation = PlaneMesh.FACE_Y
	var m := StandardMaterial3D.new()
	m.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	m.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
	m.vertex_color_use_as_albedo = true
	m.cull_mode = BaseMaterial3D.CULL_DISABLED
	q.material = m
	multimesh.mesh = q
	cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	visible = false


## On or off; returns the new state.
func toggle() -> bool:
	visible = not visible
	_cols.clear()
	_marks.clear()
	multimesh.instance_count = 0
	return visible


## The light level 0..15 at `at` (voxels) from sources [d2, position, def] (TorchLights.sources).
static func light_level(at: Vector3, sources: Array) -> int:
	var best := 0
	for s in sources:
		var range_v := Settings.m2v(float((s[2] as Dictionary).get("range_m", Settings.torch_light_range_m)))
		best = maxi(best, roundi(15.0 * (1.0 - at.distance_to(s[1]) / range_v)))
	return best


func _process(_delta: float) -> void:
	if not visible or main == null or main.world == null:
		return
	var t0 := Time.get_ticks_usec()
	if _next >= _cols.size():
		_start_pass()
	var B := Settings.BLOCK
	var from_y: float = main.player.position.y + Settings.m2v(2.0)   # head height: in a cave this is under the ceiling
	while _next < _cols.size() and Time.get_ticks_usec() - t0 < BUDGET_US:
		var c := _cols[_next]
		_next += 1
		_seen[c] = true
		_marks.erase(c)
		var x := c.x * B + B * 0.5
		var z := c.y * B + B * 0.5
		var hit: VoxelRaycastResult = main.world.raycast(Vector3(x, from_y, z), Vector3.DOWN, Settings.m2v(8.0), VoxelBody.MASK_SOLID)
		if hit == null or hit.position.y + 1 >= from_y:
			continue
		var top := Vector3(x, hit.position.y + 1, z)
		if light_level(top + Vector3(0, B * 0.5, 0), _sources) >= LIT:
			continue
		var sky: bool = main.world.raycast(top + Vector3(0, 0.5, 0), Vector3.UP, Settings.m2v(SKY_M), VoxelBody.MASK_SOLID) == null
		_marks[c] = [top, YELLOW if sky else RED]
	if _next >= _cols.size():
		_rebuild()
	stats["ms_max"] = maxf(float(stats["ms_max"]), (Time.get_ticks_usec() - t0) / 1000.0)


func _start_pass() -> void:
	var p: Vector3 = main.player.position
	var B := Settings.BLOCK
	var r := ceili(Settings.m2v(RADIUS_M) / B)
	var pc := Vector2i(floori(p.x / B), floori(p.z / B))
	_cols.clear()
	for dz in range(-r, r + 1):
		for dx in range(-r, r + 1):
			if dx * dx + dz * dz <= r * r:
				_cols.append(pc + Vector2i(dx, dz))
	_next = 0
	_seen.clear()
	var reach := Settings.m2v(RADIUS_M + maxf(Settings.fire_light_range_m, Settings.torch_light_range_m))
	_sources = main.torch_lights.sources(p, reach * reach) if main.torch_lights else []


func _rebuild() -> void:
	for c in _marks.keys():
		if not _seen.has(c):
			_marks.erase(c)   # walked away from it
	multimesh.instance_count = _marks.size()
	var buf := PackedFloat32Array()   # one write: 12 floats of transform (3x4 rows) and 4 of colour per mark
	buf.resize(_marks.size() * 16)
	var i := 0
	for c in _marks:
		var at: Vector3 = _marks[c][0]
		var col: Color = _marks[c][1]
		buf[i] = 1.0
		buf[i + 3] = at.x
		buf[i + 5] = 1.0
		buf[i + 7] = at.y + 0.06
		buf[i + 10] = 1.0
		buf[i + 11] = at.z
		buf[i + 12] = col.r
		buf[i + 13] = col.g
		buf[i + 14] = col.b
		buf[i + 15] = col.a
		i += 16
	multimesh.buffer = buf
	stats["marks"] = _marks.size()
	stats["passes"] = int(stats["passes"]) + 1


## Under the loading screen: one tiny mark in front of the camera, so the first F7 compiles nothing.
func warm(camera: Camera3D) -> void:
	var mmi := MultiMeshInstance3D.new()
	mmi.multimesh = MultiMesh.new()
	mmi.multimesh.transform_format = MultiMesh.TRANSFORM_3D
	mmi.multimesh.use_colors = true
	mmi.multimesh.mesh = multimesh.mesh
	mmi.multimesh.instance_count = 1
	mmi.multimesh.set_instance_color(0, RED)
	mmi.extra_cull_margin = 16384.0
	camera.add_child(mmi)
	mmi.transform = Transform3D(Basis.from_scale(Vector3.ONE * 0.0005), Vector3(0.5, 0.008, -1.0))
	get_tree().create_timer(5.0).timeout.connect(mmi.queue_free)
