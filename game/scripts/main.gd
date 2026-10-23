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
var _loading: LoadingScreen
var near_field: NearField            # ray-cast near field (ADR 0012 step 3), null unless Settings.near_field
var farming: Farming                 # tilled farmland and crops (ADR 0022)
var mobs: MobManager                 # sheep and pigs (ADR 0022)
var _furniture_root: Node3D
var _furniture_views := {}           # anchor -> MeshInstance3D
var work: WorkSystem                 # knapping, clay forming, pit kilns, firepits, sieves (ADR 0023)
var _stone_age_test: StoneAgeTest
var _farm_accum := 0.0
var phys_cost := {}   # ms per physics callback this frame (player, items, item_bodies, blocks); DebugCapture reads it on a spike
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
	if Settings.persist_world:
		atmosphere.calendar.load_dict(_load_json(_save_path("calendar")))   # before _ready: date= still overrides it (ADR 0025)
	add_child(atmosphere)

	items = ItemRegistry.load_default()
	recipes = RecipeRegistry.load_default(items)
	mining = MiningTable.load_default()
	# first-use costs paid behind the loading screen, not on the first item picked up:
	# parsing the sprite data (7 ms) and the held-item material's shader (15 ms, ADR 0023 run)
	ItemSprites.get_default()
	BlockMeshes.sprite("stick")
	for err in items.errors + recipes.errors + mining.errors:
		push_error("data: " + err)
	player_inv = PlayerInventory.new(items, recipes, Settings.max_health)
	blocks = BlockEntities.new(items, recipes)
	if Settings.persist_world and blocks.load_file(_blocks_path()):
		print("block entities loaded: %d" % blocks.size())
	farming = Farming.load_default()
	for err in farming.errors:
		push_error("data: " + err)
	farming.get_voxel = get_world_voxel
	farming.set_voxel = set_world_voxel
	if Settings.persist_world:
		farming.load_dict(_load_json(_save_path("farms")))

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
	player.landed.connect(trample)
	player.farm = farming
	player.place_furniture = _place_furniture

	_items_root = Node3D.new()
	_items_root.name = "Items"
	add_child(_items_root)
	_furniture_root = Node3D.new()
	_furniture_root.name = "Furniture"
	add_child(_furniture_root)
	for e in blocks.all_furniture():
		_add_furniture_view(e)
	mobs = MobManager.new()
	mobs.name = "Mobs"
	add_child(mobs)
	mobs.setup(world, self, Settings.world_seed)
	for err in mobs.errors:
		push_error("data: " + err)
	if Settings.persist_world:
		mobs.load_file(_save_path("mobs"))
	mobs.spawning = DebugCapture.mob_spawning
	player.entity_pick = mobs.pick
	player.entity_attack = func(m: Mob, dmg: int) -> void: mobs.attack(m, dmg, player.position)
	player.entity_use = func(m: Mob, held: Dictionary) -> bool:
		var r := mobs.interact(m, held)
		return r == "love" or r == "grew"

	bubble = SimBubble.new()
	bubble.name = "SimBubble"
	add_child(bubble)

	game_hud = GameHud.new()
	game_hud.name = "Hud"
	add_child(game_hud)
	# covers the first seconds while the terrain streams in (ADR 0020)
	_loading = LoadingScreen.new()
	_loading.name = "LoadingScreen"
	_loading.bubble = bubble
	_loading.finished.connect(func() -> void:
		bubble.hold_pushes = false
		player.edits_locked = false)
	player.edits_locked = true
	add_child(_loading)
	game_hud.setup(player_inv)
	player.hud = game_hud
	inv_screen = InventoryScreen.new()
	inv_screen.name = "InventoryScreen"
	add_child(inv_screen)
	inv_screen.setup(player_inv)
	inv_screen.closed.connect(_on_screen_closed)
	work = WorkSystem.new()
	work.name = "Work"
	add_child(work)
	work.setup(self)
	player.work_use = work.use
	player.work_hit = work.hit

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
				player.set_look(DebugCapture.yaw, DebugCapture.pitch)
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
				player.set_look(DebugCapture.yaw, DebugCapture.pitch)
				print("autopilot target %s at %s" % [DebugCapture.target, t])
			else:
				print("autopilot target %s not found" % DebugCapture.target)
		else:
			player.position.y = h + Settings.m2v(DebugCapture.altitude_m)
			player.set_look(DebugCapture.yaw, DebugCapture.pitch)   # same as the player's default unless pitch= is given
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
			_after_load(_place_debug_stations)   # an edit before the LOD rings exist cascades into nothing
		if DebugCapture.animals > 0:
			_after_load(_debug_animals)
		if DebugCapture.farm_test:
			_after_load(_debug_farm)
		if DebugCapture.hands_test:
			_after_load(_debug_hands)
		if DebugCapture.stone_age_test:
			_after_load(func() -> void:
				_stone_age_test = StoneAgeTest.new()   # held here: a RefCounted coroutine dies with its last reference
				_stone_age_test.run(self))
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
	if Settings.near_field:
		# the brick world around the player, ray-cast at 6.25 cm (scripts/visuals/near_field.gd)
		near_field = NearField.new()
		near_field.name = "NearField"
		add_child(near_field)
		near_field.setup(world, bubble, player.camera)
	# blood, oil and soot stains on the voxels they touch (scripts/visuals/stain_map.gd)
	stains = StainMap.new()
	stains.name = "Stains"
	add_child(stains)
	var stained: Array[ShaderMaterial] = [world.atlas_material]
	stains.setup(world, bubble, stained)
	player.stain_drip.connect(func(at: Vector3, colour: Color) -> void: stains.drip(at, colour.srgb_to_linear()))
	game_hud.player_stains = player.stains
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
	var t0 := Time.get_ticks_usec()
	_update_items(delta)
	phys_cost["items"] = maxf(float(phys_cost.get("items", 0.0)), (Time.get_ticks_usec() - t0) / 1000.0)
	t0 = Time.get_ticks_usec()
	_tick_accum += delta
	var n := 0
	while _tick_accum >= BlockEntities.TICK_S and n < 10:   # 20 ticks/s, catch up at most 0.5 s
		_tick_accum -= BlockEntities.TICK_S
		n += 1
		for p in blocks.tick():
			_on_lit_flipped(p)
	phys_cost["blocks"] = maxf(float(phys_cost.get("blocks", 0.0)), (Time.get_ticks_usec() - t0) / 1000.0)
	# Minecraft random ticks for farmland and crops, batched per second (ADR 0022)
	_farm_accum += delta * DebugCapture.farm_speed
	if _farm_accum >= 1.0:
		var ticks := int(_farm_accum / BlockEntities.TICK_S)
		_farm_accum -= ticks * BlockEntities.TICK_S
		t0 = Time.get_ticks_usec()
		farming.season_growth = atmosphere.calendar.crop_growth()   # crops rest in winter (ADR 0025)
		_spawn_drop_events(farming.tick(ticks))
		phys_cost["farm"] = maxf(float(phys_cost.get("farm", 0.0)), (Time.get_ticks_usec() - t0) / 1000.0)
	_save_accum += delta
	if _save_accum >= 30.0:
		_save_accum = 0.0
		_save_blocks()


func _exit_tree() -> void:
	_save_blocks()


func _blocks_path() -> String:
	return "user://worlds/blocks_%d_g%d.json" % [Settings.world_seed, TerrainGenerator.GEN_VERSION]


func _save_blocks() -> void:
	if not Settings.persist_world:
		return
	if blocks != null and blocks.dirty:
		blocks.save_file(_blocks_path())
	if farming != null and farming.dirty:
		_save_json(_save_path("farms"), farming.to_dict())
		farming.dirty = false
	if mobs != null:
		mobs.save_file(_save_path("mobs"))   # animals move all the time: always written
	if atmosphere != null:
		_save_json(_save_path("calendar"), atmosphere.calendar.to_dict())   # the date moves all the time too


## user://worlds/<what>_<seed>_g<gen>.json, next to the blocks file.
func _save_path(what: String) -> String:
	return "user://worlds/%s_%d_g%d.json" % [what, Settings.world_seed, TerrainGenerator.GEN_VERSION]


func _load_json(path: String) -> Dictionary:
	if not FileAccess.file_exists(path):
		return {}
	var d = JSON.parse_string(FileAccess.get_file_as_string(path))
	return d if d is Dictionary else {}


## Atomic write (temp file + rename), like BlockEntities.save_file.
func _save_json(path: String, data: Dictionary) -> void:
	DirAccess.make_dir_recursive_absolute(path.get_base_dir())
	var f := FileAccess.open(path + ".tmp", FileAccess.WRITE)
	if f == null:
		push_error("cannot write %s" % path)
		return
	f.store_string(JSON.stringify(data))
	f.close()
	DirAccess.rename_absolute(path + ".tmp", path)


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


## Runs `cb` once the loading screen is gone (now, if it already is).
func _after_load(cb: Callable) -> void:
	if _loading == null or _loading.is_done():
		cb.call_deferred()
	else:
		_loading.finished.connect(func() -> void: cb.call_deferred())


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


func _on_broken(p: Vector3i, drops: Array) -> void:
	var was := get_world_voxel(p)
	var furn := blocks.furniture_at(p)
	if furn != null:
		_remove_furniture(furn, GameMode.rule(player.game_mode, "drops"))
		return
	set_world_voxel(p, GameMaterials.AIR)
	for s in blocks.remove(p):   # chest / furnace contents spill like vanilla; work pieces give their parts back
		spawn_item(s, Vector3(p) + Vector3(0.5, 0.5, 0.5), Vector3(randf_range(-1.5, 1.5), 2.5, randf_range(-1.5, 1.5)))
	if work:
		work.forget(p)
	var events := farming.on_broken(p)
	if farming.is_crop_material(was) and events.is_empty() and GameMode.rule(player.game_mode, "drops"):
		events.append({"pos": p, "drops": farming.drops_for_material(was)})   # an untracked crop voxel
	if not GameMode.rule(player.game_mode, "drops"):
		for e in events:
			e["drops"] = []
	_spawn_drop_events(events)
	for d in drops:
		spawn_item(d, Vector3(p) + Vector3(0.5, 0.5, 0.5), Vector3(randf_range(-1, 1), 2.0, randf_range(-1, 1)))


## Farming results ({pos, drops}) become item entities.
func _spawn_drop_events(events: Array) -> void:
	for e in events:
		for d in e["drops"]:
			spawn_item(d, Vector3(e["pos"]) + Vector3(0.5, 0.3, 0.5), Vector3(randf_range(-1, 1), 2.0, randf_range(-1, 1)))


## A body landed on `p` (player or animal): farmland may be trampled (ADR 0022).
func trample(p: Vector3i, fall_m: float) -> void:
	_spawn_drop_events(farming.trample(p, fall_m))


# ---------------- furniture (ADR 0022) ----------------

func _place_furniture(kind: String, anchor: Vector3i, facing: int) -> bool:
	var e := blocks.create_furniture(anchor, kind, facing)
	for c in e.cells:
		set_world_voxel(c, GameMaterials.FURNITURE)
		_spawn_drop_events(farming.on_placed(c, GameMaterials.FURNITURE))
	_add_furniture_view(e)
	return true


func _add_furniture_view(e: FurnitureEntity) -> void:
	var v := e.make_view()
	_furniture_root.add_child(v)
	_furniture_views[e.pos] = v


func _remove_furniture(e: FurnitureEntity, drop: bool) -> void:
	var cells := e.cells.duplicate()
	var anchor := e.pos
	var stacks := blocks.remove(anchor)
	for c in cells:
		if get_world_voxel(c) == GameMaterials.FURNITURE:
			set_world_voxel(c, GameMaterials.AIR)
	if _furniture_views.has(anchor):
		_furniture_views[anchor].queue_free()
		_furniture_views.erase(anchor)
	if drop:
		for s in stacks:
			spawn_item(s, Vector3(anchor) + Vector3(0.5, 0.5, 0.5), Vector3(randf_range(-1, 1), 2.0, randf_range(-1, 1)))
	if player.seat != Vector3.INF and e.furniture == "chair":
		player.stand_up()


## Right click on a bed: sleep through the night (Minecraft: 12542..23459 of its
## 24000 ticks = after sunset to before sunrise) and set the respawn point
## beside it; on a chair: sit down.
func _use_furniture(p: Vector3i) -> void:
	var e := blocks.furniture_at(p)
	if e == null:
		set_world_voxel(p, GameMaterials.AIR)   # a stray furniture voxel without its entity
		return
	if e.furniture == "chair":
		player.sit(e.seat(), e.yaw())
		return
	_spawn_pos = e.seat() + Vector3(0, 0.6, 0)
	# night = the sun is down (sunset and sunrise move with the seasons, ADR 0025); wake at sunrise
	var cal := atmosphere.calendar
	var t := atmosphere.time_of_day
	if t > cal.sunset() + 0.02 or t < cal.sunrise() - 0.02:
		var wake := cal.sunrise() + 0.01
		atmosphere.set_time_of_day(wake)
		chat.add_message("You slept through the night. Respawn point set. %s" % cal.date_text())
	else:
		chat.add_message("You can only sleep at night. Respawn point set.")


func _on_placed(p: Vector3i, material: int) -> void:
	set_world_voxel(p, material)
	_spawn_drop_events(farming.on_placed(p, material))
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
		"furniture":
			_use_furniture(p)
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
		"near_field": near_field.stats() if near_field else {},
		"look": look,
		"player": player_inv.summary(),
		"player_stains": player.stains.summary(),
		"picked_up": picked_up_total,
		"items_in_world": _item_entities.size(),
		"item_stats": item_stats,
		"items_sample": _items_sample(),
		"inventory_open": inv_screen.is_open,
		"browser": inv_screen.browser.summary(),
		"blocks": blocks.summary(),
		"work": work.summary() if work else {},
		"mobs": mobs.summary() if mobs else {},
		"farm": farming.summary() if farming else {},
		"mem_mb": OS.get_static_memory_usage() / 1048576.0,
		"draw_calls": Performance.get_monitor(Performance.RENDER_TOTAL_DRAW_CALLS_IN_FRAME),
		"primitives": Performance.get_monitor(Performance.RENDER_TOTAL_PRIMITIVES_IN_FRAME),
	}


## Debug (animals=N): N sheep and pigs on the ground 4-10 m ahead; holding their
## food (give=wheat) they come over. JSON "mobs" shows their goals.
func _debug_animals() -> void:
	await get_tree().create_timer(0.5).timeout
	var fwd := -player.camera.global_transform.basis.z
	fwd.y = 0
	fwd = fwd.normalized()
	var right := fwd.cross(Vector3.UP)
	for i in DebugCapture.animals:
		var q := player.position + fwd * (8.0 + (i % 4) * 2.5) + right * ((i / 4) * 3.0 - 3.0)
		var x := int(floor(q.x))
		var z := int(floor(q.z))
		var y := world.surface_height(x, z) + 1
		var sp := DebugCapture.animal_species if DebugCapture.animal_species != "" else ("sheep" if i % 2 == 0 else "pig")
		var m := mobs.spawn(sp, Vector3(x + 0.5, y + 0.05, z + 0.5), DebugCapture.animal_babies and i % 3 == 2, randf() * TAU)
		if DebugCapture.animal_love and m != null and not m.is_baby():
			m.feed()
	print("debug animals: %d spawned" % DebugCapture.animals)
	if player.autopilot:
		player.autopilot_action = "look"
		player.set_look(player._yaw, DebugCapture.pitch)


## Debug (farm_test=1): a 9x9 field around one water voxel 3 m ahead, tilled and
## sown with wheat (carrots on the right half); farm_speed=N runs random ticks N x faster.
func _debug_farm() -> void:
	await get_tree().create_timer(0.5).timeout
	var fwd := -player.camera.global_transform.basis.z
	fwd.y = 0
	fwd = fwd.normalized()
	var c := player.position + fwd * 8.0
	var cx := int(floor(c.x))
	var cz := int(floor(c.z))
	var gy := world.surface_height(cx, cz)
	var tilled := 0
	var planted := 0
	for dx in range(-4, 5):
		for dz in range(-4, 5):
			var p := Vector3i(cx + dx, gy, cz + dz)
			for k in range(1, 4):
				set_world_voxel(p + Vector3i(0, k, 0), GameMaterials.AIR)
			if get_world_voxel(p) != GameMaterials.GRASS and get_world_voxel(p) != GameMaterials.DIRT:
				set_world_voxel(p, GameMaterials.DIRT)
			if dx == 0 and dz == 0:
				set_world_voxel(p, GameMaterials.WATER_STATIC)
				continue
			if farming.till(p):
				tilled += 1
				if farming.plant(p, "carrots" if dx > 0 else "wheat"):
					planted += 1
	print("debug farm at %s: tilled %d planted %d" % [Vector3i(cx, gy, cz), tilled, planted])
	if player.autopilot:
		player.autopilot_action = "look"
		player.set_look(player._yaw, DebugCapture.pitch)


# ---------------- debug: the player's own hands (hands_test=1) ----------------

func _hold(id: String) -> void:
	for i in PlayerInventory.HOTBAR:
		var st: Dictionary = player_inv.inv.get_slot(i)
		if not st.is_empty() and st["id"] == id:
			player_inv.select(i)
			return
	player_inv.pickup(id, 1)
	_hold(id)


## Aims the camera at a point (world voxels) and right clicks through Player._use.
## Returns the voxel the aim ray hit (what the player would see highlighted).
func _aim_use(at: Vector3) -> Vector3i:
	var dir := (at - player.camera.global_position).normalized()
	player.set_look(atan2(-dir.x, -dir.z), asin(dir.y))
	var hit := player._aim()
	player._use(true)
	player._place_cooldown = 0.0
	return hit.position if hit else Vector3i(-1, -1, -1)


func _aim_attack(at: Vector3) -> void:
	var dir := (at - player.camera.global_position).normalized()
	player.set_look(atan2(-dir.x, -dir.z), asin(dir.y))
	player._attack()


## Debug (hands_test=1): till, sow, place a chair and a bed, sit, feed, shear,
## kill a sheep and eat, all through the player's input code; prints one JSON line.
func _debug_hands() -> void:
	var r := {}
	await get_tree().create_timer(0.6).timeout
	player.autopilot_action = "look"
	var base := Vector3i(player.position.floor())
	var fwd := Vector3i(0, 0, -1)
	var gy := func(x: int, z: int) -> int: return world.surface_height(x, z)
	# clear a 9 x 9 patch of plants so the aim ray hits the ground
	for dx in range(-4, 5):
		for dz in range(-8, 1):
			var g: int = gy.call(base.x + dx, base.z + dz)
			for k in range(1, 4):
				set_world_voxel(Vector3i(base.x + dx, g + k, base.z + dz), GameMaterials.AIR)
			set_world_voxel(Vector3i(base.x + dx, g, base.z + dz), GameMaterials.GRASS)
	await get_tree().create_timer(0.5).timeout   # the cleared patch reaches the terrain the aim ray reads
	base = Vector3i(player.position.floor())
	var soil := Vector3i(base.x, gy.call(base.x, base.z - 3), base.z - 3)
	_hold("flint_hoe")
	r["hoe_aim"] = _aim_use(Vector3(soil) + Vector3(0.5, 0.99, 0.5)) == soil
	r["tilled"] = GameMaterials.name_of(get_world_voxel(soil))
	_hold("wheat_seeds")
	_aim_use(Vector3(soil) + Vector3(0.5, 0.99, 0.5))
	r["sown"] = GameMaterials.name_of(get_world_voxel(soil + Vector3i.UP))
	r["crops"] = farming.crops.size()
	# chair two to the right, bed three to the left
	var cp := Vector3i(base.x + 2, gy.call(base.x + 2, base.z - 3), base.z - 3)
	_hold("chair")
	r["chair_aim"] = _aim_use(Vector3(cp) + Vector3(0.5, 0.99, 0.5)) == cp
	var ch := blocks.furniture_at(cp + Vector3i.UP)
	r["chair"] = ch.summary() if ch != null else "not placed"
	await get_tree().create_timer(0.3).timeout
	if ch != null:
		_aim_use(Vector3(cp) + Vector3(0.5, 1.5, 0.5))
		r["sitting"] = player.seat != Vector3.INF
		player.stand_up()
		r["stood_up_at"] = player.position.snapped(Vector3.ONE * 0.1)
	var bp := Vector3i(base.x - 3, gy.call(base.x - 3, base.z - 2), base.z - 2)
	_hold("bed")
	r["bed_aim"] = _aim_use(Vector3(bp) + Vector3(0.5, 0.99, 0.5)) == bp
	var bed := blocks.furniture_at(bp + Vector3i.UP)
	r["bed"] = bed.summary() if bed != null else "not placed"
	await get_tree().create_timer(0.3).timeout   # the bed's voxels reach the terrain the aim ray reads
	if bed != null:
		atmosphere.set_time_of_day(0.9)
		_aim_use(Vector3(bed.cells[0]) + Vector3(0.5, 0.9, 0.5))
		r["time_after_sleep"] = snappedf(atmosphere.time_of_day, 0.01)
		r["spawn_after_sleep"] = _spawn_pos.snapped(Vector3.ONE * 0.1)
	# a sheep ahead: feed, shear, kill
	var sp := Vector3i(base.x, gy.call(base.x, base.z - 5) + 1, base.z - 5)
	var sheep := mobs.spawn("sheep", Vector3(sp) + Vector3(0.5, 0.05, 0.5))
	await get_tree().create_timer(0.3).timeout
	_hold("wheat")
	var before := player_inv.inv.count_of("wheat")
	_aim_use(sheep.aabb().get_center())
	r["fed_love"] = sheep.love_s > 0.0
	r["wheat_used"] = before - player_inv.inv.count_of("wheat")
	_hold("shears")
	_aim_use(sheep.aabb().get_center())
	r["sheared"] = sheep.sheared
	var spawned_before: int = item_stats["spawned"]
	_hold("flint_hoe")
	var hits := 0
	for i in 12:
		if not is_instance_valid(sheep) or sheep.is_dead():
			break
		player._attack_cooldown = 0.0
		sheep.position = Vector3(sp) + Vector3(0.5, 0.05, 0.5)   # it panics away between hits: bring it back in reach
		_aim_attack(sheep.aabb().get_center())
		hits += 1
		await get_tree().create_timer(0.55).timeout
	await get_tree().create_timer(1.3).timeout
	r["hits_to_kill"] = hits
	r["sheep_dead"] = not is_instance_valid(sheep)
	r["drops_spawned"] = item_stats["spawned"] - spawned_before
	var drops := []
	for e in _item_entities:
		if is_instance_valid(e):
			drops.append("%s x%d" % [e.stack["id"], e.stack["count"]])
	r["items_on_ground"] = drops
	# eat: hurt, hold raw meat, right click at the sky
	player_inv.damage(6)
	var hp: int = player_inv.health
	_hold("raw_meat")
	player.set_look(0.0, 1.2)
	player._use(true)
	await get_tree().create_timer(2.0).timeout
	r["health_before_eating"] = hp
	r["health_after_eating"] = player_inv.health
	player.set_look(0.0, -0.5)
	print("debug hands " + JSON.stringify(r))
