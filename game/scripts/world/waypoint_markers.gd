class_name WaypointMarkers
extends Node3D

## Waypoints in the world (plan item D10, Xaero style): a thin beam of the waypoint's colour rising
## Settings.waypoint_beam_m from the spot, and its name and distance floating above it, readable at any
## range (Label3D fixed size). Rebuilt when the waypoints change; the distance text every 0.5 s.

var main: Node
var _marks := {}           # id -> {beam: MeshInstance3D, label: Label3D}
var _beam_mesh: CylinderMesh
var _t := 0.0
var _materials := {}       # Color -> StandardMaterial3D


func setup(p_main: Node) -> void:
	main = p_main
	_beam_mesh = CylinderMesh.new()
	_beam_mesh.top_radius = Settings.m2v(0.05)
	_beam_mesh.bottom_radius = Settings.m2v(0.05)
	_beam_mesh.height = Settings.m2v(Settings.waypoint_beam_m)
	_beam_mesh.radial_segments = 6
	_beam_mesh.rings = 1
	main.world_map.waypoints.changed.connect(_rebuild)
	_rebuild()
	# a Label3D's first draw compiles its pipeline: one tiny label in front of the camera under the
	# loading screen (like WorkSystem._prewarm)
	var warm := Label3D.new()
	warm.text = "."
	warm.fixed_size = true
	warm.pixel_size = 0.0012
	warm.outline_size = 8
	warm.no_depth_test = true
	warm.position = Vector3(0.4, -0.3, -1.0)
	main.player.camera.add_child(warm)
	var loading: Node = main.get("_loading")
	if loading != null and loading.has_signal("finished"):
		loading.finished.connect(func() -> void: warm.queue_free())
	else:
		get_tree().create_timer(3.0).timeout.connect(func() -> void: warm.queue_free())


## A beam material per colour: unshaded, see-through, glowing a little.
func _material(c: Color) -> StandardMaterial3D:
	if _materials.has(c):
		return _materials[c]
	var m := StandardMaterial3D.new()
	m.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	m.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
	m.blend_mode = BaseMaterial3D.BLEND_MODE_ADD
	m.albedo_color = Color(c.r, c.g, c.b, 0.45)
	m.cull_mode = BaseMaterial3D.CULL_DISABLED
	_materials[c] = m
	return m


## MaterialWarmup draws these under the loading screen.
func warm_materials() -> Array:
	var out := []
	for c in Waypoints.COLOURS + Waypoints.KIND_COLOURS.values():
		out.append(_material(c))
	return out


func _rebuild() -> void:
	for id in _marks:
		(_marks[id]["beam"] as Node).queue_free()
		(_marks[id]["label"] as Node).queue_free()
	_marks.clear()
	for w in main.world_map.waypoints.list:
		var p: Vector3i = w["pos"]
		var c: Color = w["colour"]
		var beam := MeshInstance3D.new()
		beam.mesh = _beam_mesh
		beam.material_override = _material(c)
		beam.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
		beam.position = Vector3(p) + Vector3(0.5, Settings.m2v(Settings.waypoint_beam_m) * 0.5, 0.5)
		add_child(beam)
		var label := Label3D.new()
		label.billboard = BaseMaterial3D.BILLBOARD_ENABLED
		label.fixed_size = true
		label.pixel_size = 0.0012
		label.font_size = 28
		label.outline_size = 8
		label.no_depth_test = true
		label.modulate = c.lightened(0.35)
		label.position = Vector3(p) + Vector3(0.5, Settings.m2v(2.5), 0.5)
		add_child(label)
		_marks[w["id"]] = {"beam": beam, "label": label, "name": w["name"], "pos": p}
	_t = 0.0


func _process(delta: float) -> void:
	_t -= delta
	if _t > 0.0 or main == null or main.player == null:
		return
	_t = 0.5
	var me: Vector3 = main.player.position
	for id in _marks:
		var m: Dictionary = _marks[id]
		var d := Settings.v2m(me.distance_to(Vector3(m["pos"])))
		(m["label"] as Label3D).text = "%s\n%d m" % [m["name"], roundi(d)]
		(m["beam"] as Node3D).visible = d > 4.0   # close by the beam is in the way
