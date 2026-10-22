extends Node3D

## Wires world, player, simulation bubble, items and UI together. Owns the
## world-side of interaction (ADR 0005): voxel writes requested by the player
## go through the sim bubble when inside it, drops become ItemEntity nodes,
## item entities are picked up, merged and despawned here.

var world: World
var atmosphere: Atmosphere
var stains: StainMap
var player: Player
var bubble: SimBubble
var hud: DebugHud
var game_hud: GameHud
var inv_screen: InventoryScreen
var items: ItemRegistry
var recipes: RecipeRegistry
var mining: MiningTable
var player_inv: PlayerInventory
var blocks: BlockEntities
var commands: CommandRegistry      # slash commands (ADR 0018)
var game_commands: GameCommands
var chat: ChatBox
var debug_menu: DebugMenu
var _tick_accum := 0.0
var _save_accum := 0.0
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

	# sky, sun, fog, bloom, SSAO, underwater / underground looks (scripts/visuals/atmosphere.gd)
	atmosphere = Atmosphere.new()
	atmosphere.name = "Atmosphere"
	add_child(atmosphere)

	items = ItemRegistry.load_default()
	recipes = RecipeRegistry.load_default(items)
	mining = MiningTable.load_default()
	for err in items.errors + recipes.errors + mining.errors:
		push_error("data: " + err)
	player_inv = PlayerInventory.new(items, recipes, Settings.max_health)
	blocks = BlockEntities.new(items, recipes)
	if Settings.persist_world and blocks.load_file(_blocks_path()):
		print("block entities loaded: %d" % blocks.size())

	player = Player.new()
	player.name = "Player"
	player.world = world
	var spawn := _find_spawn()
	var h := world.surface_height(spawn.x, spawn.y)
	player.position = Vector3(spawn.x + 0.5, h + 3, spawn.y + 0.5)
	_spawn_pos = player.position
	add_child(player)
	player.setup_inventory(player_inv, mining, world.prop_material)
	atmosphere.setup(world, player.camera)
	player.voxel_broken.connect(_on_broken)
	player.voxel_placed.connect(_on_placed)
	player.item_dropped.connect(_on_item_dropped)
	player.inventory_toggled.connect(_on_inventory_toggled)
	player.died.connect(_on_died)
	player.block_used.connect(_on_block_used)

	_items_root = Node3D.new()
	_items_root.name = "Items"
	add_child(_items_root)

	bubble = SimBubble.new()
	bubble.name = "SimBubble"
	add_child(bubble)

	game_hud = GameHud.new()
	game_hud.name = "Hud"
	add_child(game_hud)
	# covers the first seconds while the terrain streams in (ADR 0020)
	var loading := LoadingScreen.new()
	loading.name = "LoadingScreen"
	loading.bubble = bubble
	add_child(loading)
	game_hud.setup(player_inv)
	player.hud = game_hud
	inv_screen = InventoryScreen.new()
	inv_screen.name = "InventoryScreen"
	add_child(inv_screen)
	inv_screen.setup(player_inv)
	inv_screen.closed.connect(_on_screen_closed)

	# chat, slash commands, game modes and the playtest debug menu (ADR 0018)
	commands = CommandRegistry.new()
	game_commands = GameCommands.new(commands, player, player_inv, self)
	chat = ChatBox.new()
	chat.name = "Chat"
	chat.commands = commands
	chat.can_open = _can_open_overlay
	add_child(chat)
	chat.opened.connect(_on_overlay_opened)
	chat.closed.connect(_on_overlay_closed)
	debug_menu = DebugMenu.new()
	debug_menu.name = "DebugMenu"
	debug_menu.main = self
	debug_menu.can_open = _can_open_overlay
	add_child(debug_menu)
	debug_menu.opened.connect(_on_overlay_opened)
	debug_menu.closed.connect(_on_overlay_closed)
	player.game_mode_changed.connect(_on_game_mode_changed)
	var mode := GameMode.parse(Settings.game_mode)
	player.set_game_mode(mode if mode != "" else GameMode.SURVIVAL)

	hud = DebugHud.new()
	hud.main = self
	add_child(hud)
	hud.toggle()                 # F3 overlay off by default

	DebugCapture.attach(self)
	for stack in DebugCapture.give:
		player_inv.pickup(stack["id"], stack["count"])
	if DebugCapture.autopilot:
		player.autopilot = true
		player.flying = true
		if DebugCapture.third_person > 0:
			player.set_perspective(DebugCapture.third_person)
		player.autopilot_speed_m = DebugCapture.speed_m
		if DebugCapture.target != "":
			var t: Vector3i = world.generator.gen.find_nearest(DebugCapture.target, spawn.x, spawn.y, 4096)
			if t.y >= 0 and DebugCapture.inside:
				# stand in the target itself (a cave room): eye height above its floor voxel, a torch on the player
				player.position = Vector3(t.x + 0.5, t.y + 0.1, t.z + 0.5)
				player.set_look(0.0, DebugCapture.pitch)
				var torch := OmniLight3D.new()
				torch.omni_range = 40.0
				torch.light_energy = 2.5
				torch.light_color = Color(1.0, 0.9, 0.75)
				torch.position = Vector3(0, 1.5, 0)
				player.add_child(torch)
				print("autopilot inside %s at %s" % [DebugCapture.target, t])
			elif t.y >= 0:
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
			_debug_browser.call_deferred()
		if DebugCapture.stations:
			_place_debug_stations.call_deferred()
		if DebugCapture.scatter_items:
			# one entity per given item, in a row two metres ahead, so a screenshot shows them
			var fwd := -player.camera.global_transform.basis.z
			var i := 0
			for stack in DebugCapture.give:
				var pos := player.position + Vector3(0, Settings.m2v(1.5), 0) + fwd * Settings.m2v(2.0) + player.camera.global_transform.basis.x * (i - DebugCapture.give.size() / 2.0) * 1.2
				spawn_item({"id": stack["id"], "count": stack["count"]}, pos, Vector3.ZERO)
				i += 1
	else:
		for c in Settings.start_commands:   # the debug menu's saved start options
			chat.send(c)
		if Settings.start_minimized and EngineDebugger.is_active() and not _embedded_in_editor():
			# playtests from the editor start minimized; after restoring the window a click captures the mouse
			DisplayServer.window_set_mode(DisplayServer.WINDOW_MODE_MINIMIZED)
		else:
			player.capture_mouse(true)
	for c in DebugCapture.chat_commands:
		chat.send(c)
	if DebugCapture.chat_text != "":
		chat.open(DebugCapture.chat_text)
	if DebugCapture.debug_menu:
		debug_menu.open(true)
	if DebugCapture.f3:
		hud.toggle()
	bubble.setup(world, player.position)
	# blood, oil and soot stains on the voxels they touch (scripts/visuals/stain_map.gd)
	stains = StainMap.new()
	stains.name = "Stains"
	add_child(stains)
	var stained: Array[ShaderMaterial] = [world.atlas_material]
	stains.setup(world, bubble, stained)
	_last_health = player_inv.health
	player_inv.health_changed.connect(_on_health_changed)


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
	stains.follow(player.position)


func _physics_process(delta: float) -> void:
	_update_items(delta)
	_tick_accum += delta
	var n := 0
	while _tick_accum >= BlockEntities.TICK_S and n < 10:   # 20 ticks/s, catch up at most 0.5 s
		_tick_accum -= BlockEntities.TICK_S
		n += 1
		for p in blocks.tick():
			_on_lit_flipped(p)
	_save_accum += delta
	if _save_accum >= 30.0:
		_save_accum = 0.0
		_save_blocks()


func _exit_tree() -> void:
	_save_blocks()


func _blocks_path() -> String:
	return "user://worlds/blocks_%d_g%d.json" % [Settings.world_seed, TerrainGenerator.GEN_VERSION]


func _save_blocks() -> void:
	if Settings.persist_world and blocks != null and blocks.dirty:
		blocks.save_file(_blocks_path())


func _unhandled_input(event: InputEvent) -> void:
	if event.is_action_pressed("toggle_debug"):
		hud.toggle()


# ---------------- chat, debug menu, game modes (ADR 0018) ----------------

## Chat and the debug menu open only over the plain game view.
func _can_open_overlay() -> bool:
	return not player.ui_open and not player.autopilot and not inv_screen.is_open


func _on_overlay_opened() -> void:
	player.set_ui_open(true, false)


func _on_overlay_closed() -> void:
	if not chat.is_open and not debug_menu.is_open:
		player.set_ui_open(false)


func _on_game_mode_changed(mode: String) -> void:
	game_hud.set_hearts_visible(GameMode.rule(mode, "takes_damage"))
	inv_screen.browser_view.cheat_give = player_inv.creative_take if GameMode.rule(mode, "item_picker") else Callable()


## Godot 4.4+ can run the game inside the editor's Game tab; minimizing that would hide it.
func _embedded_in_editor() -> bool:
	return Engine.has_method("is_embedded_in_editor") and bool(Engine.call("is_embedded_in_editor"))


## /stations and the debug menu: a crafting table, chest and furnace ahead.
func place_stations() -> void:
	_place_debug_stations()


## Where the player respawns (/spawn).
func spawn_position() -> Vector3:
	return _spawn_pos


# ---------------- world writes ----------------

func set_world_voxel(p: Vector3i, material: int) -> void:
	if not bubble.set_voxel(p, material):
		world.set_voxel(p, material)


## Reads where set_world_voxel writes: the bubble first (it is ahead of the
## terrain until the next push), else the terrain.
func get_world_voxel(p: Vector3i) -> int:
	var m := bubble.get_voxel(p)
	return m if m >= 0 else world.get_voxel(p)


func _on_broken(p: Vector3i, drop: Dictionary) -> void:
	set_world_voxel(p, GameMaterials.AIR)
	for s in blocks.remove(p):   # chest / furnace contents spill like vanilla
		spawn_item(s, Vector3(p) + Vector3(0.5, 0.5, 0.5), Vector3(randf_range(-1.5, 1.5), 2.5, randf_range(-1.5, 1.5)))
	if not drop.is_empty():
		spawn_item(drop, Vector3(p) + Vector3(0.5, 0.5, 0.5), Vector3(randf_range(-1, 1), 2.0, randf_range(-1, 1)))


func _on_placed(p: Vector3i, material: int) -> void:
	set_world_voxel(p, material)
	if material > 0:
		stains.clear_cell(p)   # a filled cell's old stain must not come back when it is mined
		var d: Dictionary = GameMaterials.defs()[material - 1]
		if d.has("entity"):
			blocks.create(p, d["entity"])


var _last_health := -1


## Losing health bleeds: a splatter at the feet, liquid blood for heavy hits.
func _on_health_changed(h: int) -> void:
	if _last_health > h:
		stains.bleed(player.position, _last_health - h)
	_last_health = h


## Right click on a station block (ADR 0007).
func _on_block_used(p: Vector3i, material: int) -> void:
	var d: Dictionary = GameMaterials.defs()[material - 1]
	var kind: String = d.get("interact", "")
	match kind:
		"crafting_table":
			player.set_ui_open(true, false)
			inv_screen.open(3, "crafting_table", "Crafting")
		"chest", "furnace":
			var e := blocks.get_or_create(p, d.get("entity", kind))
			if material == GameMaterials.FURNACE_LIT and not e.is_lit():
				set_world_voxel(p, GameMaterials.FURNACE)   # lit block without fire (older save): put it out
			player.set_ui_open(true, false)
			inv_screen.open_container(e)


## A furnace started or stopped burning: swap furnace <-> furnace_lit, or
## drop the entity when its block is gone (burnt, replaced by the sim).
func _on_lit_flipped(p: Vector3i) -> void:
	var m := get_world_voxel(p)
	if m != GameMaterials.FURNACE and m != GameMaterials.FURNACE_LIT:
		for s in blocks.remove(p):
			spawn_item(s, Vector3(p) + Vector3(0.5, 0.5, 0.5))
		return
	var e: FurnaceEntity = blocks.at(p)
	set_world_voxel(p, GameMaterials.FURNACE_LIT if e.is_lit() else GameMaterials.FURNACE)


## Debug (with open_inventory=1): search=text filters the item list,
## browse=item_id[:uses] opens its recipes (or uses) in the recipe view.
func _debug_browser() -> void:
	var args := {}
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=", true, 1)
		if kv.size() == 2:
			args[kv[0]] = kv[1]
	if args.has("search"):
		inv_screen.browser_view._search.text = args["search"]
		inv_screen.browser_view._search.text_changed.emit(args["search"])
	if args.has("browse"):
		var parts: PackedStringArray = args["browse"].split(":")
		var ok := inv_screen.browser_view.show_item(parts[0], parts[1] if parts.size() > 1 else "recipes")
		print("debug browse %s -> %s" % [args["browse"], ok])
	if args.get("hover_test", "0") == "1":
		# real input path: mouse over the 8th list cell (oak planks), R (recipes), then RMB on the 2nd cell (uses)
		var bv := inv_screen.browser_view
		await get_tree().process_frame
		var cell: Control = bv._cells[7]
		get_viewport().warp_mouse(cell.get_global_rect().get_center())
		var mm := InputEventMouseMotion.new()
		mm.position = cell.get_global_rect().get_center()
		Input.parse_input_event(mm)
		for _i in 3:
			await get_tree().process_frame
		var hovered := inv_screen.hovered_item()
		var key := InputEventKey.new()
		key.keycode = KEY_R
		key.pressed = true
		Input.parse_input_event(key)
		for _i in 2:
			await get_tree().process_frame
		print("debug hover %s tooltip=%s R -> %s" % [hovered, inv_screen._tooltip.text.replace("\n", " / "), inv_screen.browser.summary().get("view", {})])
		var c2: Control = bv._cells[1]
		var click := InputEventMouseButton.new()
		click.button_index = MOUSE_BUTTON_RIGHT
		click.pressed = true
		click.position = c2.get_global_rect().get_center()
		Input.parse_input_event(click)
		for _i in 2:
			await get_tree().process_frame
		print("debug rmb %s -> %s history=%d" % [c2.item_id, inv_screen.browser.summary().get("view", {}), inv_screen.browser.history.size()])


## Debug: aim at a block and right click it through the player's own use path.
func _debug_use_block(at: Vector3i) -> void:
	var dir := (Vector3(at) + Vector3(0.5, 0.5, 0.5) - player.camera.global_position).normalized()
	player.set_look(atan2(-dir.x, -dir.z), asin(dir.y))
	player._use(true)
	print("debug use %s -> screen open %s (%s)" % [at, inv_screen.is_open, player_inv.summary()["container"]])


## Debug: a crafting table, chest and furnace on the ground ahead (stations=1);
## the furnace starts smelting 8 iron ore with 1 coal, the chest holds samples.
func _place_debug_stations() -> void:
	var fwd := -player.camera.global_transform.basis.z
	fwd.y = 0
	fwd = fwd.normalized()
	var right := fwd.cross(Vector3.UP)
	var base := player.position + fwd * 4.0
	var kinds := [["crafting_table", GameMaterials.CRAFTING_TABLE], ["chest", GameMaterials.CHEST], ["furnace", GameMaterials.FURNACE]]
	var spots: Array[Vector3i] = []
	for i in kinds.size():
		var q := base + right * (i - 1) * 2.0
		var x := int(floor(q.x))
		var z := int(floor(q.z))
		spots.append(Vector3i(x, world.surface_height(x, z) + 1, z))
	# the terrain must be editable there first (a player can only aim at loaded ground)
	for _f in 1200:
		var ok := true
		for sp in spots:
			if not world.is_area_loaded(Vector3i((Vector3(sp) / Settings.CHUNK).floor())):
				ok = false
		if ok:
			break
		await get_tree().process_frame
	for i in kinds.size():
		var x := spots[i].x
		var y := spots[i].y
		var z := spots[i].z
		_on_placed(Vector3i(x, y, z), kinds[i][1])
		var e: RefCounted = blocks.at(Vector3i(x, y, z))
		if e is FurnaceEntity:
			e.inv.set_slot(FurnaceEntity.INPUT, {"id": "iron_ore", "count": 8})
			e.inv.set_slot(FurnaceEntity.FUEL, {"id": "coal", "count": 1})
		elif e is ChestEntity:
			e.inv.set_slot(0, {"id": "oak_log", "count": 32})
			e.inv.set_slot(4, {"id": "cobblestone", "count": 64})
			e.inv.set_slot(13, {"id": "coal", "count": 9})
		print("debug station %s at %s" % [kinds[i][0], Vector3i(x, y, z)])
		if DebugCapture.open_screen == kinds[i][0]:
			# the strip shows the blocks first (furnace lighting up), the screen for the last 40 %
			var at := Vector3i(x, y, z)
			get_tree().create_timer(DebugCapture.duration * 0.6).timeout.connect(_debug_use_block.bind(at))
	if player.autopilot:
		player.autopilot_action = "look"   # stand still (autopilot input is zero for any action)
		player.set_look(player._yaw, -0.45)


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
	var e := ItemEntity.create(items, stack, world.prop_material)
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
		"game_mode": player.game_mode,
		"chat": chat.recent(10) if chat else [],
		"chat_open": chat.is_open if chat else false,
		"debug_menu_open": debug_menu.is_open if debug_menu else false,
		"terrain": world.terrain.get_statistics(),
		"sim": bubble.stats(),
		"stains": stains.stats() if stains else {},
		"look": look,
		"player": player_inv.summary(),
		"picked_up": picked_up_total,
		"items_in_world": _item_entities.size(),
		"item_stats": item_stats,
		"items_sample": _items_sample(),
		"inventory_open": inv_screen.is_open,
		"browser": inv_screen.browser.summary(),
		"blocks": blocks.summary(),
		"mem_mb": OS.get_static_memory_usage() / 1048576.0,
		"draw_calls": Performance.get_monitor(Performance.RENDER_TOTAL_DRAW_CALLS_IN_FRAME),
		"primitives": Performance.get_monitor(Performance.RENDER_TOTAL_PRIMITIVES_IN_FRAME),
	}
