extends Node3D

## Wires world, player, simulation bubble, items and UI together. Owns the
## world-side of interaction (ADR 0005): voxel writes requested by the player
## go through the sim bubble when inside it, drops become ItemEntity nodes,
## item entities are picked up, merged and despawned here.

var world: World
var player: Player
var bubble: SimBubble
var hud: DebugHud
var game_hud: GameHud
var inv_screen: InventoryScreen
var items: ItemRegistry
var recipes: RecipeRegistry
var mining: MiningTable
var player_inv: PlayerInventory
var _items_root: Node3D
var _item_entities: Array[ItemEntity] = []
var _merge_accum := 0.0
var _spawned := false
var _spawn_xz := Vector2(0, 0)
var _spawn_pos := Vector3.ZERO
var picked_up_total := {}    # item id -> count, for debug JSON
var item_stats := {"spawned": 0, "picked": 0, "merged": 0, "expired": 0, "invalid": 0}


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

	items = ItemRegistry.load_default()
	recipes = RecipeRegistry.load_default(items)
	mining = MiningTable.load_default()
	for err in items.errors + recipes.errors + mining.errors:
		push_error("data: " + err)
	player_inv = PlayerInventory.new(items, recipes, Settings.max_health)

	player = Player.new()
	player.name = "Player"
	player.world = world
	var spawn := _find_spawn()
	var h := world.surface_height(spawn.x, spawn.y)
	player.position = Vector3(spawn.x + 0.5, h + 3, spawn.y + 0.5)
	_spawn_pos = player.position
	add_child(player)
	player.setup_inventory(player_inv, mining, world.atlas_material)
	player.voxel_broken.connect(_on_broken)
	player.voxel_placed.connect(_on_placed)
	player.item_dropped.connect(_on_item_dropped)
	player.inventory_toggled.connect(_on_inventory_toggled)
	player.died.connect(_on_died)

	_items_root = Node3D.new()
	_items_root.name = "Items"
	add_child(_items_root)

	bubble = SimBubble.new()
	bubble.name = "SimBubble"
	add_child(bubble)

	game_hud = GameHud.new()
	game_hud.name = "Hud"
	add_child(game_hud)
	game_hud.setup(player_inv)
	player.hud = game_hud
	inv_screen = InventoryScreen.new()
	inv_screen.name = "InventoryScreen"
	add_child(inv_screen)
	inv_screen.setup(player_inv)
	inv_screen.closed.connect(_on_screen_closed)

	hud = DebugHud.new()
	hud.main = self
	add_child(hud)
	hud.toggle()                 # F3 overlay off by default; the game HUD draws the crosshair
	hud.crosshair.visible = false

	DebugCapture.attach(self)
	for stack in DebugCapture.give:
		player_inv.pickup(stack["id"], stack["count"])
	if DebugCapture.autopilot:
		player.autopilot = true
		player.flying = true
		player.autopilot_speed_m = DebugCapture.speed_m
		if DebugCapture.target != "":
			var t: Vector3i = world.generator.gen.find_nearest(DebugCapture.target, spawn.x, spawn.y, 4096)
			if t.y >= 0:
				# start offset_m before the target along +z, fly -z over it
				player.position = Vector3(t.x + 0.5, t.y + Settings.m2v(DebugCapture.altitude_m), t.z + Settings.m2v(DebugCapture.offset_m) + 0.5)
				player.set_look(0.0, DebugCapture.pitch)
				print("autopilot target %s at %s" % [DebugCapture.target, t])
			else:
				print("autopilot target %s not found" % DebugCapture.target)
		else:
			player.position.y = h + Settings.m2v(DebugCapture.altitude_m)
		if DebugCapture.walk or DebugCapture.mine or DebugCapture.place:
			player.flying = false
			# stand on the ground at the player's actual x/z (targets move it away from spawn)
			var gh := world.surface_height(int(floor(player.position.x)), int(floor(player.position.z)))
			player.position.y = maxi(gh, Settings.SEA_LEVEL_VOXEL) + 3
		if DebugCapture.place:
			player.autopilot_action = "place"
			player.set_look(0.0, -0.8)   # the ground a few voxels ahead; blocks pile up there
		if DebugCapture.mine:
			player.autopilot_action = "mine"
			# stand on the corner of four voxels, look almost straight down: the ray
			# hits one voxel of the 2x2 under the feet; Player turns after each break
			player.set_look(PI / 4.0, -1.45)
			player.position.x = floor(player.position.x)
			player.position.z = floor(player.position.z)
		if DebugCapture.open_inventory:
			inv_screen.open()
		if DebugCapture.scatter_items:
			# one entity per given item, in a row two metres ahead, so a screenshot shows them
			var fwd := -player.camera.global_transform.basis.z
			var i := 0
			for stack in DebugCapture.give:
				var pos := player.position + Vector3(0, Settings.m2v(1.5), 0) + fwd * Settings.m2v(2.0) + player.camera.global_transform.basis.x * (i - DebugCapture.give.size() / 2.0) * 1.2
				spawn_item({"id": stack["id"], "count": stack["count"]}, pos, Vector3.ZERO)
				i += 1
	else:
		player.capture_mouse(true)
	bubble.setup(world, player.position)


## Spiral out from the origin for dry, flat-ish land (plains or forest).
func _find_spawn() -> Vector2i:
	var gen := world.generator
	for r in range(0, 2048, 32):
		for a in range(0, 360, 30):
			var x := int(r * cos(deg_to_rad(a)))
			var z := int(r * sin(deg_to_rad(a)))
			var info: Dictionary = gen.column_info(x, z)
			var b: int = info["biome"]
			if (b == WorldGen.BIOME_PLAINS or b == WorldGen.BIOME_FOREST) and int(info["height"]) > Settings.SEA_LEVEL_VOXEL + 4 and float(info["river"]) < 0.05:
				return Vector2i(x, z)
	return Vector2i.ZERO


func _process(_delta: float) -> void:
	bubble.follow(player.position)


func _physics_process(delta: float) -> void:
	_update_items(delta)


func _unhandled_input(event: InputEvent) -> void:
	if event.is_action_pressed("toggle_debug"):
		hud.toggle()


# ---------------- world writes ----------------

func set_world_voxel(p: Vector3i, material: int) -> void:
	if not bubble.set_voxel(p, material):
		world.set_voxel(p, material)


func _on_broken(p: Vector3i, drop: Dictionary) -> void:
	set_world_voxel(p, GameMaterials.AIR)
	if not drop.is_empty():
		spawn_item(drop, Vector3(p) + Vector3(0.5, 0.5, 0.5), Vector3(randf_range(-1, 1), 2.0, randf_range(-1, 1)))


func _on_placed(p: Vector3i, material: int) -> void:
	set_world_voxel(p, material)


func _on_item_dropped(stack: Dictionary, from: Vector3, velocity: Vector3) -> void:
	spawn_item(stack, from, velocity, Settings.item_pickup_delay_s * 3.0)


func _on_inventory_toggled(open: bool) -> void:
	if open:
		inv_screen.open()
	else:
		inv_screen.close()


func _on_screen_closed(dropped: Array) -> void:
	for s in dropped:
		_on_item_dropped(s, player.camera.global_position, -player.camera.global_transform.basis.z * 2.0)


func _on_died() -> void:
	print("player died at %s, respawning" % player.position)
	player.respawn_at(_spawn_pos)


# ---------------- item entities ----------------

func spawn_item(stack: Dictionary, pos: Vector3, velocity: Vector3 = Vector3.ZERO, extra_delay: float = 0.0) -> ItemEntity:
	var e := ItemEntity.create(items, stack, world.atlas_material)
	e.position = pos
	e.age = -extra_delay
	e.set_initial_velocity(velocity)
	e.world = world
	_items_root.add_child(e)
	_item_entities.append(e)
	item_stats["spawned"] += 1
	return e


func _update_items(delta: float) -> void:
	if _item_entities.is_empty():
		return
	var feet := player.position
	var centre := feet + Vector3(0, Settings.m2v(Settings.player_height_m) * 0.5, 0)
	var radius := Settings.m2v(Settings.pickup_radius_m)
	var merge_r := Settings.m2v(Settings.item_merge_radius_m)
	_merge_accum += delta
	var do_merge := _merge_accum >= 0.5
	if do_merge:
		_merge_accum = 0.0
	var alive: Array[ItemEntity] = []
	for e in _item_entities:
		if not is_instance_valid(e) or e.is_queued_for_deletion():
			item_stats["invalid"] += 1
			continue
		if e.expired():
			item_stats["expired"] += 1
			e.queue_free()
			continue
		if e.can_pickup() and e.position.distance_to(centre) <= radius:
			var left := player_inv.pickup(e.stack["id"], e.stack["count"])
			var got: int = e.stack["count"] - left
			if got > 0:
				picked_up_total[e.stack["id"]] = int(picked_up_total.get(e.stack["id"], 0)) + got
			if left <= 0:
				item_stats["picked"] += 1
				e.queue_free()
				continue
			e.set_count(left)
		alive.append(e)
	if do_merge:
		for i in alive.size():
			var a := alive[i]
			if a.is_queued_for_deletion():
				continue
			for j in range(i + 1, alive.size()):
				var b := alive[j]
				if b.is_queued_for_deletion() or b.stack["id"] != a.stack["id"]:
					continue
				if a.position.distance_to(b.position) > merge_r:
					continue
				var max_stack := items.max_stack(a.stack["id"])
				var room: int = max_stack - a.stack["count"]
				if room <= 0:
					continue
				var take: int = mini(room, b.stack["count"])
				a.set_count(a.stack["count"] + take)
				if take >= b.stack["count"]:
					item_stats["merged"] += 1
					b.queue_free()
				else:
					b.set_count(b.stack["count"] - take)
		var kept: Array[ItemEntity] = []
		for e in alive:
			if not e.is_queued_for_deletion():
				kept.append(e)
		alive = kept
	_item_entities = alive


func _items_sample() -> Array:
	var out := []
	for e in _item_entities.slice(0, 5):
		if is_instance_valid(e):
			out.append({"id": e.stack["id"], "count": e.stack["count"], "pos": e.position.snapped(Vector3.ONE * 0.1), "age": snappedf(e.age, 0.1)})
	return out


func verify_bubble() -> Dictionary:
	return bubble.verify_against_generator(world.generator.gen)


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
		"player": player_inv.summary(),
		"picked_up": picked_up_total,
		"items_in_world": _item_entities.size(),
		"item_stats": item_stats,
		"items_sample": _items_sample(),
		"inventory_open": inv_screen.is_open,
		"mem_mb": OS.get_static_memory_usage() / 1048576.0,
		"draw_calls": Performance.get_monitor(Performance.RENDER_TOTAL_DRAW_CALLS_IN_FRAME),
		"primitives": Performance.get_monitor(Performance.RENDER_TOTAL_PRIMITIVES_IN_FRAME),
	}
