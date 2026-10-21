extends Node3D

## Wires world, player, simulation bubble and debug tools together.

var world: World
var player: Player
var bubble: SimBubble
var hud: DebugHud
var _spawned := false
var _spawn_xz := Vector2(0, 0)


func _ready() -> void:
	world = World.new()
	world.name = "World"
	add_child(world)

	var env := WorldEnvironment.new()
	var e := Environment.new()
	e.background_mode = Environment.BG_SKY
	var sky := Sky.new()
	sky.sky_material = ProceduralSkyMaterial.new()
	e.sky = sky
	e.ambient_light_source = Environment.AMBIENT_SOURCE_SKY
	e.fog_enabled = true
	e.fog_light_color = Color(0.7, 0.8, 0.95)
	e.fog_density = 0.0004
	env.environment = e
	add_child(env)
	var sun := DirectionalLight3D.new()
	sun.rotation_degrees = Vector3(-50, 30, 0)
	sun.shadow_enabled = true
	sun.directional_shadow_max_distance = Settings.m2v(96.0)
	add_child(sun)

	player = Player.new()
	player.name = "Player"
	player.world = world
	var h := world.surface_height(0, 0)
	player.position = Vector3(0.5, h + 3, 0.5)
	add_child(player)
	player.break_requested.connect(_on_break)
	player.place_requested.connect(_on_place)

	bubble = SimBubble.new()
	bubble.name = "SimBubble"
	add_child(bubble)
	bubble.setup(world, player.position)

	hud = DebugHud.new()
	hud.main = self
	add_child(hud)

	DebugCapture.attach(self)
	if DebugCapture.autopilot:
		player.autopilot = true
		player.flying = true
		player.position.y = h + Settings.m2v(30.0)
	else:
		player.capture_mouse(true)


func _process(_delta: float) -> void:
	bubble.follow(player.position)


func _unhandled_input(event: InputEvent) -> void:
	if event.is_action_pressed("toggle_debug"):
		hud.toggle()


func _on_break(hit: VoxelRaycastResult) -> void:
	var p: Vector3i = hit.position
	if not bubble.set_voxel(p, GameMaterials.AIR):
		world.set_voxel(p, GameMaterials.AIR)


func _on_place(hit: VoxelRaycastResult) -> void:
	var p: Vector3i = hit.previous_position
	if not bubble.set_voxel(p, GameMaterials.SAND):
		world.set_voxel(p, GameMaterials.SAND)


## Everything the HUD shows, as data (DebugCapture prints it as JSON).
func snapshot() -> Dictionary:
	var pos := player.position
	var look := {}
	var hit := world.raycast(player.camera.global_position, -player.camera.global_transform.basis.z, Settings.m2v(Settings.reach_m))
	if hit:
		look = {"voxel": hit.position, "material": GameMaterials.name_of(world.get_voxel(hit.position)), "dist_m": snappedf(Settings.v2m(hit.distance), 0.1)}
	return {
		"fps": Engine.get_frames_per_second(),
		"frame_ms": snappedf(Performance.get_monitor(Performance.TIME_PROCESS) * 1000.0, 0.01),
		"pos_m": Vector3(pos * Settings.VOXEL_M).snapped(Vector3.ONE * 0.1),
		"pos_voxel": Vector3i(pos.floor()),
		"chunk": Vector3i((pos / Settings.CHUNK).floor()),
		"mode": "fly" if player.flying else "walk",
		"terrain": world.terrain.get_statistics(),
		"sim": bubble.stats(),
		"look": look,
		"mem_mb": OS.get_static_memory_usage() / 1048576.0,
		"draw_calls": Performance.get_monitor(Performance.RENDER_TOTAL_DRAW_CALLS_IN_FRAME),
		"primitives": Performance.get_monitor(Performance.RENDER_TOTAL_PRIMITIVES_IN_FRAME),
	}
