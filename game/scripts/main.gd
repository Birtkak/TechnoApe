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
var tune_panel: TunePanel
var tools_menu: ToolsMenu            # the world tools panel on G (ADR 0071)
var _tick_accum := 0.0
var _save_accum := 0.0
var _loading: LoadingScreen
var startup_ms := {}   # ms since _ready began, per startup step (printed, and in DebugCapture's JSON)
var farming: Farming                 # tilled farmland and crops (ADR 0022)
var saplings: Saplings               # planted saplings growing into trees (ADR 0112)
var pieces: FallingPieces            # rigid voxel pieces (P12), null with rigid_pieces=0
var builder: ShapeBuilder            # shape building, box mode, prefabs (ADR 0038)
var survival: Survival               # hunger, body temperature, food spoilage (ADR 0040)
var pickups_popped := 0              # sticks and stones picked up by hand (ADR 0043, debug JSON)
var _pickup_rng := RandomNumberGenerator.new()
var tools_broken := 0                # tools that wore out this session (ADR 0042, debug JSON)
var _survival_save := {}             # loaded with the player, before Survival exists
var stone_age: StoneAge               # the four Stone Age stages: earth ovens, racks, canoes, megaliths ... (ADR 0059)
var _stone_age_save := {}             # its part of the player's save, loaded before it exists
var fx: ParticleFx                   # Minecraft-style particles (ADR 0026), null with particles=0
var mobs: MobManager                 # sheep and pigs (ADR 0022)
var _furniture_root: Node3D
var _furniture_views := {}           # anchor -> MeshInstance3D
var work: WorkSystem                 # knapping, clay forming, pit kilns, firepits, sieves (ADR 0023)
var _stone_age_test: StoneAgeTest
var _flint_age_test: FlintAgeTest
var _craft_test: CraftTest
var _metal_age_test: MetalAgeTest   # the bronze and iron ages (ADR 0070)
var _info_test: RefCounted          # the look-at panel and recipe browser sheet (ADR 0093)
var _stone_ages_test: StoneAgesTest   # the four Stone Age stages (ADR 0059)
var _water_test: WaterTest
var _looks_test: LooksTest
var _liquids_test: LiquidsTest
var _tp_test: TpTest
var quests: QuestBook                # the progression book's quests and progress (ADR 0028)
var projectiles: Projectiles         # thrown spears in flight (ADR 0030)
var gadgets: PlaytestTools           # playtest gadgets: laser cutter, crater rod, bombs, instant tree, tape measure
var tnt: Tnt                         # TNT charges, fuses and blasts (ADR 0065)
var _tnt_save := []                  # its charges from the player's save, loaded before it exists
var weather: Weather                 # rain and snow, the rain that answers forest fires (ADR 0056); null with weather=0
var world_map := WorldMap.new()      # the explored map and the waypoints (plan item D10, ADR 0055)
var waypoints: Waypoints = world_map.waypoints   # add(name, pos, kind) for graves, finds ...
var map_screen: MapScreen            # M
var minimap: Minimap
var waypoint_markers: WaypointMarkers
var torch_lights: TorchLights        # real lights for the torches near the player (ADR 0030)
var quest_toast: QuestToast
var survival_loop: SurvivalLoop       # diet, seasons, prospecting, discoveries, HUD trackers (ADR 0061)
var pause_menu: PauseMenu            # Esc: pause, settings, save and quit (ADR 0032)
var quest_check_ms_max := 0.0        # slowest progression-book inventory check (debug JSON)
var _farm_accum := 0.0
var phys_cost := {}   # ms per physics callback this frame (player, items, item_bodies, blocks); DebugCapture reads it on a spike
var _items_root: Node3D
var _item_entities: Array[ItemEntity] = []
var _merge_accum := 0.0
var _spawned := false
var _spawn_xz := Vector2(0, 0)
var _spawn_pos := Vector3.ZERO
var deaths := 0   # apetions lost: the Gelvin monolith numbers each one it rebuilds (docs/design/lore.md)
var death_screen: DeathScreen
var _dying := false   # the death and rebuild sequence is running
var _monolith_spawn := Vector2i.ZERO   # the Gelvin monolith (ADR 0098): raised once around the spawn, then saved
var _monolith_boxes: Array = []        # Monolith.boxes still to fill (their chunks not loaded yet)
var _monolith_next := 0
var _monolith_up := false
var picked_up_total := {}    # item id -> count, for debug JSON
var item_stats := {"spawned": 0, "picked": 0, "merged": 0, "expired": 0, "invalid": 0, "burned": 0}


func _ready() -> void:
	var t0 := Time.get_ticks_usec()
	var mark := func(step: String) -> void:   # where the frozen first frame goes (title -> game)
		startup_ms[step] = snappedf((Time.get_ticks_usec() - t0) / 1000.0, 0.1)
	world = World.new()
	world.name = "World"
	add_child(world)
	mark.call("world")

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
	farming.is_editable = world.is_editable
	if Settings.persist_world:
		farming.load_dict(_load_json(_save_path("farms")))
	saplings = Saplings.load_default()
	for err in saplings.errors:
		push_error("data: " + err)
	saplings.get_voxel = func(p: Vector3i) -> int: return bubble.get_voxel(p) if bubble else -1
	saplings.fill_box = func(lo: Vector3i, hi: Vector3i, m: int) -> int: return bubble.fill_box(lo, hi, m)
	saplings.fill_ball = func(c: Vector3, r: float, m: int) -> void: bubble.blast(c, r, m, PackedInt32Array([GameMaterials.AIR]))
	if Settings.persist_world:
		saplings.load_dict(_load_json(_save_path("saplings")))
	mark.call("data")

	player = Player.new()
	player.name = "Player"
	player.world = world
	var spawn := _find_spawn()
	var h := world.surface_height(spawn.x, spawn.y)
	player.position = Vector3(spawn.x + 0.5, h + 3, spawn.y + 0.5)
	_spawn_pos = player.position
	_monolith_spawn = spawn
	var light := OmniLight3D.new()   # the monolith seam's magenta light on the plaza (ADR 0098)
	light.light_color = Color(1.0, 0.25, 0.85)
	light.light_energy = 4.0
	light.omni_range = Settings.m2v(24.0)
	light.omni_attenuation = Settings.torch_light_attenuation
	light.position = Monolith.light_pos(world, spawn)
	add_child(light)
	var hum := Sound.loop_voice("loop/monolith_hum")   # its deep hum, from the same spot
	if hum:
		hum.position = light.position
		hum.autoplay = true
		add_child(hum)
	add_child(player)
	add_child(MonolithBeacon.new(Monolith.beam_foot(world, spawn), player.camera))   # the sky beam over the spawn
	player.setup_inventory(player_inv, mining, world.prop_material)
	mark.call("spawn")
	atmosphere.setup(world, player.camera)
	mark.call("atmosphere")
	player.voxel_broken.connect(_on_broken)
	player.voxel_placed.connect(_on_placed)
	player.voxels_dug.connect(_on_dug)
	player.voxels_placed.connect(_on_placed_many)
	player.big_dug.connect(_on_big_dug)
	player.brush_filled.connect(_on_brush_filled)
	player.item_dropped.connect(_on_item_dropped)
	player.inventory_toggled.connect(_on_inventory_toggled)
	player.map_requested.connect(_open_map)
	player.lookup_requested.connect(_lookup)
	player.died.connect(_on_died)
	death_screen = DeathScreen.new()
	add_child(death_screen)
	player.block_used.connect(_on_block_used)
	player.landed.connect(trample)
	player.farm = farming
	player.place_furniture = _place_furniture
	player.furniture_is = func(p: Vector3i, what: String) -> bool:
		var f := blocks.furniture_at(p)
		if f == null:
			return what == "use"   # a stray furniture voxel: _use_furniture clears it
		var k: Dictionary = FurnitureEntity.KINDS[f.furniture]
		if what == "use":   # a table or shelf takes no click: a pot or lantern goes on it
			return f.furniture == "chair" or k.get("sleep", false) or f.can_open()
		return k.get(what, false)

	_items_root = Node3D.new()
	_items_root.name = "Items"
	add_child(_items_root)
	_furniture_root = Node3D.new()
	_furniture_root.name = "Furniture"
	add_child(_furniture_root)
	for e in blocks.all_furniture():
		_add_furniture_view(e)
	_station_root = Node3D.new()
	_station_root.name = "Stations"
	add_child(_station_root)
	for kind in ["station", "chest", "furnace", "storage_pot"]:
		for e in blocks.all_of(kind):
			_add_station_view(e)
	mobs = MobManager.new()
	mobs.name = "Mobs"
	add_child(mobs)
	mobs.setup(world, self, Settings.world_seed)
	for err in mobs.errors:
		push_error("data: " + err)
	if Settings.persist_world:
		mobs.load_file(_save_path("mobs"))
	mobs.spawning = DebugCapture.mob_spawning
	mark.call("mobs")
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
	# covers the start until the ground around the player is in, like a teleport (ADR 0020, 0066): the rest of the box streams in behind it
	cover_until_loaded("Generating world", true)
	_loading.finished.connect(func() -> void: bubble.hold_pushes = false)
	player.moved_far.connect(func() -> void: cover_until_loaded("Loading", true))
	mark.call("bubble_hud_loading")
	game_hud.setup(player_inv)
	player.hud = game_hud
	inv_screen = InventoryScreen.new()
	inv_screen.name = "InventoryScreen"
	add_child(inv_screen)
	inv_screen.setup(player_inv)
	inv_screen.closed.connect(_on_screen_closed)
	inv_screen.drop_requested.connect(player.throw_stack)   # a click outside the panel, Q over a slot (ADR 0055)
	inv_screen.nearby_chests = _nearby_chests
	_setup_quests()
	work = WorkSystem.new()
	work.name = "Work"
	add_child(work)
	work.setup(self)
	player.work_use = work.use
	player.work_hit = work.hit
	player.work_ghost = work.ghost_spot
	player.hold_key = work.hold_key
	player.hold_done = work.hold_done
	player.describe_block = _describe_block
	player.fire_at = func(v: Vector3i) -> bool:
		var e: RefCounted = blocks.at(v)
		return e is FirepitEntity and e.is_lit()
	projectiles = Projectiles.new()
	projectiles.name = "Projectiles"
	projectiles.main = self
	add_child(projectiles)
	player.throw_item = projectiles.throw
	torch_lights = TorchLights.new()
	torch_lights.name = "TorchLights"
	torch_lights.main = self
	add_child(torch_lights)
	mark.call("ui_work")

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
	tune_panel = TunePanel.new()
	tune_panel.name = "TunePanel"
	tune_panel.can_open = _can_open_overlay
	add_child(tune_panel)
	tune_panel.opened.connect(_on_overlay_opened)
	tune_panel.closed.connect(_on_overlay_closed)
	tools_menu = ToolsMenu.new()
	tools_menu.name = "ToolsMenu"
	tools_menu.main = self
	tools_menu.can_open = _can_open_overlay
	add_child(tools_menu)
	tools_menu.opened.connect(_on_overlay_opened)
	tools_menu.closed.connect(_on_overlay_closed)
	player.game_mode_changed.connect(_on_game_mode_changed)
	var mode := GameMode.parse(Settings.game_mode)
	player.set_game_mode(mode if mode != "" else GameMode.SURVIVAL)
	if Settings.persist_world:
		_load_player()   # inventory, position, health, game mode of this world (ADR 0032)

	pause_menu = PauseMenu.new()
	pause_menu.name = "PauseMenu"
	add_child(pause_menu)
	player.pause_requested.connect(_on_pause_requested)
	pause_menu.resumed.connect(func() -> void:
		if not player.ui_open:
			player.capture_mouse(true))
	pause_menu.quit_to_title.connect(_quit_to_title)
	pause_menu.quit_game.connect(func() -> void:
		get_tree().paused = false
		_save_blocks()
		save_terrain(5.0)
		get_tree().quit())
	Settings.user_settings_changed.connect(_on_user_settings_changed)

	hud = DebugHud.new()
	hud.main = self
	add_child(hud)
	hud.toggle()                 # F3 overlay off by default
	mark.call("chat_menus_player")
	print("startup_ms " + JSON.stringify(startup_ms))

	DebugCapture.attach(self)
	for stack in DebugCapture.give:
		player_inv.pickup(stack["id"], stack["count"], stack)
	if DebugCapture.autopilot:
		player.autopilot = true
		player.flying = true
		if DebugCapture.third_person > 0:
			player.set_perspective(DebugCapture.third_person)
		player.autopilot_speed_m = 0.0 if DebugCapture.fly_after_cover else DebugCapture.speed_m
		if DebugCapture.fly_after_cover:
			_after_load(func() -> void: player.autopilot_speed_m = DebugCapture.speed_m)
		if DebugCapture.target != "":
			var t: Vector3i = world.generator.gen.find_nearest(DebugCapture.target, spawn.x, spawn.y, int(Settings.m2v(4000.0)))
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
			_after_load(_place_debug_stations)   # an edit before the terrain is loaded goes into nothing
		if DebugCapture.animals > 0:
			_after_load(_debug_animals)
		if DebugCapture.farm_test:
			_after_load(_debug_farm)
		if DebugCapture.building_test:
			_after_load(_debug_building)
		if DebugCapture.hands_test:
			_after_load(_debug_hands)
		if DebugCapture.swim_test:
			_after_load(_debug_swim)
		if not DebugCapture.later.is_empty():
			_after_load(func() -> void:
				await get_tree().create_timer(1.0).timeout
				for c in DebugCapture.later:
					if c.begins_with("wait "):
						await get_tree().create_timer(float(c.substr(5))).timeout
					else:
						chat.send(c)
						await get_tree().process_frame)
		if DebugCapture.chop_test != "":
			_after_load(func() -> void:
				await get_tree().create_timer(1.5).timeout
				chat.send("/chop " + DebugCapture.chop_test.replace(",", " ").replace("1", "")))
		if DebugCapture.fell_test != "":
			_after_load(func() -> void:
				await get_tree().create_timer(1.5).timeout
				chat.send("/fell " + DebugCapture.fell_test.replace(",", " ")))   # fell_test=0,cam
		if DebugCapture.stone_age_test:
			_after_load(func() -> void:
				_stone_age_test = StoneAgeTest.new()   # held here: a RefCounted coroutine dies with its last reference
				_stone_age_test.run(self))
		if DebugCapture.flint_age_test:
			_after_load(func() -> void:
				_flint_age_test = FlintAgeTest.new()
				_flint_age_test.run(self))
		if DebugCapture.stone_ages_test:
			_after_load(func() -> void:
				_stone_ages_test = StoneAgesTest.new()
				_stone_ages_test.run(self))
		if DebugCapture.craft_test:
			_after_load(func() -> void:
				_craft_test = CraftTest.new()
				_craft_test.run(self))
		if DebugCapture.metal_age_test:
			_after_load(func() -> void:
				_metal_age_test = MetalAgeTest.new()
				_metal_age_test.run(self))
		if DebugCapture.info_test:
			_after_load(func() -> void:
				_info_test = InfoTest.new()
				_info_test.run(self))
		if DebugCapture.tp_test:
			_after_load(func() -> void:
				_tp_test = TpTest.new()
				_tp_test.run(self))
		if DebugCapture.water_test:
			_after_load(func() -> void:
				_water_test = WaterTest.new()
				_water_test.run(self))
		if not DebugCapture.chat_commands_after.is_empty():
			_after_load(func() -> void:
				for c in DebugCapture.chat_commands_after:
					if c.begins_with("wait "):   # commands_after=/tp 10 1030 20;wait 3;/blast 2 lava
						await get_tree().create_timer(float(c.substr(5))).timeout
					else:
						chat.send(c))
		if DebugCapture.showcase != "":
			_after_load(func() -> void:
				Showcase.new().run(self, DebugCapture.showcase.split(",", false), DebugCapture.showcase_on, true))
		if DebugCapture.looks_test:
			_after_load(func() -> void:
				_looks_test = LooksTest.new()
				_looks_test.run(self))
		if DebugCapture.liquids_test:
			_after_load(func() -> void:
				_liquids_test = LiquidsTest.new()
				_liquids_test.run(self))
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
		if Settings.start_minimized and Settings.world_name == "" and EngineDebugger.is_active() and not _embedded_in_editor():
			# playtests from the editor (play=1) start minimized; after restoring the window a click captures
			# the mouse. A world picked on the title screen just starts (the title minimized instead)
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
	player.brush_tool.bubble = bubble
	# blood, oil and soot stains on the voxels they touch (scripts/visuals/stain_map.gd)
	stains = StainMap.new()
	stains.name = "Stains"
	add_child(stains)
	var stained: Array[ShaderMaterial] = [world.atlas_material]
	stains.setup(world, bubble, stained)
	player.stain_drip.connect(func(at: Vector3, colour: Color) -> void: stains.drip(at, colour.srgb_to_linear()))
	game_hud.player_stains = player.stains
	# every terrain pipeline compiles under the loading screen, not when the first gas or flame shows up
	var warmup := MaterialWarmup.new()
	warmup.name = "MaterialWarmup"
	add_child(warmup)
	warmup.setup(world.terrain.mesher as VoxelMesherBlocky, player.camera)
	if not warmup.is_queued_for_deletion():
		warmup.add_materials(player.warm_materials(), player.camera)
		warmup.add_meshes([StationEntity.model_mesh(), StationEntity.model_mesh("chest"), StationEntity.model_mesh("furnace"), StationEntity.model_mesh("furnace_lit"), StationEntity.ghost_mesh("chest"), BlockMeshes.sprite("torch"), BlockMeshes.sprite("iron_ore")], player.camera)   # the stations; item sprites, extruded and cube (held and dropped)
	ApeDust.warm(player.camera)   # the death and rebuild (ADR 0098)
	death_screen.warm()
	# chips, dust, splashes, smoke, hearts, lava pops, drips and waterfall spray (scripts/visuals/particle_fx.gd)
	if Settings.particles:
		fx = ParticleFx.load_default()
		fx.name = "Particles"
		for err in fx.errors:
			push_error("data: " + err)
		add_child(fx)
		fx.setup(world.atlas_material.get_shader_parameter("atlas"), GameMaterials.ATLAS_TILES)
		fx.get_voxel = get_world_voxel
		fx.bubble = bubble
		fx.spray = world.spray
		fx.player = player
		fx.main_blocks = blocks
		player.landed.connect(func(p: Vector3i, fall_m: float) -> void: fx.landing(p, fall_m, player.position))
	if Settings.rigid_pieces:
		pieces = FallingPieces.new()
		pieces.name = "FallingPieces"
		add_child(pieces)
		pieces.setup(bubble, world.atlas_material, fx)
		bubble.pieces = pieces
		if not warmup.is_queued_for_deletion():
			warmup.add_materials([pieces.material], player.camera)
	builder = ShapeBuilder.new()
	builder.name = "ShapeBuilder"
	add_child(builder)
	builder.setup(self)
	survival = Survival.new()
	survival.name = "Survival"
	add_child(survival)
	survival.setup(self)
	if game_hud:
		game_hud.survival = survival
	if not _survival_save.is_empty():
		survival.load_dict(_survival_save)
	stone_age = StoneAge.new()
	stone_age.name = "StoneAge"
	add_child(stone_age)
	stone_age.setup(self)
	stone_age.load_player(_stone_age_save)
	player.item_use = stone_age.use
	survival_loop = SurvivalLoop.new()
	survival_loop.name = "SurvivalLoop"
	add_child(survival_loop)
	survival_loop.setup(self)
	if not warmup.is_queued_for_deletion():
		warmup.add_materials(builder.warm_materials(), player.camera)
	gadgets = PlaytestTools.new()
	gadgets.name = "PlaytestTools"
	add_child(gadgets)
	gadgets.setup(self)
	player.playtest_laser = gadgets.laser
	player.playtest_use = gadgets.use
	if not warmup.is_queued_for_deletion():
		warmup.add_materials(gadgets.warm_materials(), player.camera)
	tnt = Tnt.new()
	tnt.name = "Tnt"
	add_child(tnt)
	tnt.setup(self)
	tnt.load_array(_tnt_save)
	player.ignite_at = _ignite
	if not warmup.is_queued_for_deletion():
		warmup.add_meshes(tnt.warm_meshes() + [player.brush_tool.warm_mesh()], player.camera)
	player.brush_tool.warm_cache()   # the big round brushes, rasterised under the loading screen
	_setup_maps()
	if not warmup.is_queued_for_deletion():
		warmup.add_materials(waypoint_markers.warm_materials(), player.camera)
	if Settings.weather:
		weather = Weather.new()
		weather.name = "Weather"
		add_child(weather)
		weather.setup(self)
		if not warmup.is_queued_for_deletion():
			warmup.add_materials(weather.warm_materials(), player.camera)
	_last_health = player_inv.health
	player_inv.health_changed.connect(_on_health_changed)
	player_inv.tool_broke.connect(_on_tool_broke)
	player_inv.tool_low.connect(func(id: String, left: int) -> void:
		Sound.play("ui/error")
		if game_hud:
			game_hud.show_toast("%s: %d use%s left" % [items.display_name(id), left, "" if left == 1 else "s"]))
	Sound.attach(self)   # it listens to the player, particles, pieces and mobs (ADR 0044)


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


## The Gelvin monolith (ADR 0098, Monolith): up to 48 of its boxes per frame, round robin; a box whose
## chunks are not loaded yet stays for a later frame. Saved as done, so a world raises it once.
func _raise_monolith() -> void:
	if _monolith_boxes.is_empty():
		_monolith_boxes = Monolith.boxes(world, _monolith_spawn)
	var t0 := Time.get_ticks_usec()
	for _k in mini(48, _monolith_boxes.size()):
		if Time.get_ticks_usec() - t0 > 2000:   # 2 ms a frame: the start cover lifts before its ground is in (ADR 0108)
			return
		_monolith_next %= _monolith_boxes.size()
		var b: Array = _monolith_boxes[_monolith_next]
		if bubble.fill_box(b[0], b[1], b[2]) >= 0:
			_monolith_boxes.remove_at(_monolith_next)
			if _monolith_boxes.is_empty():
				_monolith_up = true
				print("monolith raised at %s" % Monolith.centre(_monolith_spawn))
				if waypoints:
					for w in waypoints.list.duplicate():   # a raised-again monolith (Monolith.VERSION) moves its tip
						if w["name"] == "Monolith":
							waypoints.remove(int(w["id"]))
					waypoints.add("Monolith", Vector3i(Monolith.beam_foot(world, _monolith_spawn)), "", Color("#ff3ad8"))
				return
		else:
			_monolith_next += 1

func _process(delta: float) -> void:
	var _pt0 := Time.get_ticks_usec()
	_process_timed(delta)
	DebugCapture.note_proc("main", _pt0)


func _process_timed(delta: float) -> void:
	Inventory.clock_s += delta   # play time: hot metal cools against it (ADR 0042)
	bubble.follow(player.position)
	stains.follow(player.position)
	if not _monolith_up:
		_raise_monolith()


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
		saplings.tick(ticks * BlockEntities.TICK_S)
		phys_cost["farm"] = maxf(float(phys_cost.get("farm", 0.0)), (Time.get_ticks_usec() - t0) / 1000.0)
	_save_accum += delta
	if _save_accum >= 30.0:
		_save_accum = 0.0
		_save_blocks()
		if bubble:
			bubble.flush_all()
		if world.edits != _saved_edits:
			save_terrain()   # a crash loses 30 s of digging at most; nothing changed, nothing walked


func _exit_tree() -> void:
	_save_blocks()


## Writes the terrain edits still loaded to the world save: the bubble's simulated chunks go back into
## the terrain, then every modified block to the SQLite stream. Blocks used to be written only when
## they unloaded, so a save-and-quit lost what was dug or built near the player (2026-10-25).
## wait_s > 0 blocks until the writes are done (quitting), at most that long.
var _saved_edits := 0


func save_terrain(wait_s := 0.0) -> void:
	if not Settings.persist_world or world == null or world.terrain == null or not world.terrain.is_inside_tree():
		return
	if bubble:
		bubble.flush_all()
	_saved_edits = world.edits
	var tracker = world.terrain.save_modified_blocks()   # ~9 ms on the main thread: it walks every loaded block
	if wait_s <= 0.0 or tracker == null:
		return
	var until := Time.get_ticks_msec() + int(wait_s * 1000.0)
	while not tracker.is_complete() and not tracker.is_aborted() and Time.get_ticks_msec() < until:
		OS.delay_msec(5)
	if not tracker.is_complete():
		push_warning("save_terrain: the terrain save did not finish in %.1f s" % wait_s)


func _blocks_path() -> String:
	return "user://worlds/blocks_%d_g%d.json" % [Settings.world_seed, TerrainGenerator.GEN_VERSION]


func _save_blocks() -> void:
	if not Settings.persist_world:
		return
	if blocks != null and blocks.dirty:
		blocks.save_file(_blocks_path())
	if stone_age != null:
		stone_age.save_world()
	if quests != null and quests.dirty:
		_save_json(_save_path("quests"), quests.to_dict())
		quests.dirty = false
	if player != null and player_inv != null:
		_save_json(_save_path("player"), _player_dict())   # the player moves all the time: always written
	if farming != null and farming.dirty:
		_save_json(_save_path("farms"), farming.to_dict())
		farming.dirty = false
	if saplings != null and saplings.dirty:
		_save_json(_save_path("saplings"), saplings.to_dict())
		saplings.dirty = false
	if mobs != null:
		mobs.save_file(_save_path("mobs"))   # animals move all the time: always written
	if atmosphere != null:
		_save_json(_save_path("calendar"), atmosphere.calendar.to_dict())   # the date moves all the time too
	world_map.save()   # explored map tiles that changed


## The progression book (ADR 0028): progress saved per world, item tasks
## counted whenever the inventory changes, a toast per completed quest.
## The look-at panel's word on a block entity: {title?, line?} (line may hold several lines split
## by "\n"). Work pieces get their own name, not "Work Spot", and stations show what is in them (ADR 0078).
func _describe_block(p: Vector3i) -> Dictionary:
	var e: RefCounted = blocks.at(p)
	if e is ChestEntity or e is FurnaceEntity:   # what is inside (ADR 0093)
		var lines := PackedStringArray()
		if e is ChestEntity and e.label != "":
			lines.append(e.label)   # the chest's name (plan item D11)
		if e is FurnaceEntity:
			lines.append(("Burning" if e.is_lit() else "Not lit") + (", smelting %d%%" % (100 * e.cook / maxi(e.cook_total, 1)) if e.cook > 0 else ""))
		lines.append(_holds_line(e.inv))
		return {"line": "\n".join(lines)}
	if not BlockEntities.is_work(e):
		return {}
	var info := {"title": _work_title(e), "mat": 0}   # the work block itself has no picture: the station's item has
	var pic: String = {"knap": "flint", "clayform": "clay_ball"}.get(e.kind, e.item if "item" in e and e.item != "" else e.kind)
	info["icon"] = pic if items.has(pic) else ""
	if e is GraveEntity:
		info["line"] = "Your things (%d stacks)%s: right click" % [e.count(), (", " + e.died_at) if e.died_at != "" else ""]
	elif e is ClayFormEntity:
		var c := e as ClayFormEntity
		if not c.has_recipe():
			info["line"] = "Clay: pick a shape"
		else:
			var out: String = items.display_name(String(c.recipe.get("output", {}).get("id", "")))
			info["line"] = "%s: layer %d of %d" % [out, mini(c.active_layer() + 1, c.height()), c.height()]
	elif e is KnapEntity:
		var k := e as KnapEntity
		if not k.has_recipe():
			info["line"] = "Flint: pick a shape"
		else:
			info["line"] = "%s: %d chips left" % [items.display_name(String(k.recipe.get("output", {}).get("id", ""))), k.remaining()]
	elif e is FirepitEntity or e is AnvilEntity:
		info["line"] = "\n".join(e.look_lines())
	return info


## "Holds 12 Coal, 3 Iron Ore and 2 more": a container's stacks in items (not voxels), or "Empty".
func _holds_line(inv: Inventory) -> String:
	var counts := {}
	for s in inv.to_array():
		if not s.is_empty():
			counts[s["id"]] = int(counts.get(s["id"], 0)) + int(s["count"])
	if counts.is_empty():
		return "Empty"
	var parts := PackedStringArray()
	for id in counts.keys().slice(0, 3):
		parts.append("%d %s" % [counts[id] / maxi(items.units(id), 1), items.display_name(id)])
	return "Holds " + ", ".join(parts) + (" and %d more" % (counts.size() - 3) if counts.size() > 3 else "")


func _work_title(e: RefCounted) -> String:
	match e.kind:
		"knap":
			return "Knapping"
		"clayform":
			return "Clay forming"
		"assembly":
			return "Tool being assembled"
		"ground":
			return items.display_name(e.item) if e.item != "" else "Ground storage"
	var id: String = e.item if e is AnvilEntity else e.kind
	return items.display_name(id) if items.has(id) else String(e.kind).capitalize()


func _setup_quests() -> void:
	quests = QuestBook.load_default(items)
	for err in quests.errors:
		push_error("data: " + err)
	if Settings.persist_world:
		quests.load_dict(_load_json(_save_path("quests")))
	quest_toast = QuestToast.new()
	quest_toast.name = "QuestToast"
	quest_toast.items = items
	add_child(quest_toast)
	quests.quest_completed.connect(func(id: String) -> void:
		var q: Dictionary = quests.quests[id]
		quest_toast.push(q["icon"], q["title"], q.get("final", false))
		if chat:
			chat.add_message("Quest complete: %s" % q["title"]))
	# once per frame at most: a shift-click craft changes slots hundreds of times in one frame
	player_inv.changed.connect(func() -> void:
		if not _quests_pending:
			_quests_pending = true
			_check_quests.call_deferred())
	inv_screen.setup_quests(quests)
	for a in OS.get_cmdline_user_args():
		if a.begins_with("quests_done="):   # debug: quests_done=all or a,b,c (for book screenshots)
			for id in (quests.quests.keys() if a.substr(12) == "all" else Array(a.substr(12).split(",", false))):
				quests.complete(id)


var _quests_pending := false

func _check_quests() -> void:
	_quests_pending = false
	var t0 := Time.get_ticks_usec()
	quests.check_inventory(player_inv.inv, player_inv.cursor, [player_inv.bag])
	quest_check_ms_max = maxf(quest_check_ms_max, (Time.get_ticks_usec() - t0) / 1000.0)


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
# ---------------- pause menu and settings (ADR 0032) ----------------

func _on_pause_requested() -> void:
	if player.autopilot or player.ui_open or inv_screen.is_open or chat.is_open or debug_menu.is_open:
		return
	pause_menu.open()
	get_viewport().set_input_as_handled()   # the same Esc must not reach the menu and close it again


## Minecraft pauses when the window loses focus (not autopilot runs, not while loading).
func _notification(what: int) -> void:
	if what == NOTIFICATION_WM_CLOSE_REQUEST:   # the window's X / Alt+F4: _exit_tree is too late, the terrain has left
		_save_blocks()
		save_terrain(5.0)
	if what == NOTIFICATION_APPLICATION_FOCUS_OUT and pause_menu != null and is_instance_valid(_loading) == false:
		_on_pause_requested()


func _quit_to_title() -> void:
	get_tree().paused = false
	_save_blocks()
	save_terrain(5.0)
	get_tree().change_scene_to_file.call_deferred("res://scenes/title.tscn")


func _on_user_settings_changed() -> void:
	atmosphere.apply_graphics()
	player.camera.fov = Settings.fov
	world.apply_distances()


# ---------------- the player's save (ADR 0032) ----------------

func _player_dict() -> Dictionary:
	var p := player.position
	var s := _spawn_pos
	return {"version": 1, "pos": [p.x, p.y, p.z], "yaw": player._yaw, "pitch": player._pitch, "spawn": [s.x, s.y, s.z], "deaths": deaths, "monolith": Monolith.VERSION if _monolith_up else 0,
		"game_mode": player.game_mode, "flying": player.flying, "health": player_inv.health, "selected": player_inv.selected,
		"slots": player_inv.inv.to_array(), "bag_item": player_inv.bag_item, "bag": player_inv.bag.to_array(),
		"offhand": player_inv.offhand(), "favourites": player_inv.favourites.keys(), "waypoints": world_map.waypoints.to_array(), "survival": survival.to_dict() if survival else {}, "clock_s": Inventory.clock_s,
		"stone_age": stone_age.player_dict() if stone_age else {}, "tnt": tnt.to_array() if tnt else _tnt_save,
		"bookmarks": inv_screen.browser.bookmarks}


func _load_player() -> void:
	var d := _load_json(_save_path("player"))
	Inventory.clock_s = float(d.get("clock_s", 0.0))   # before the stacks: hot metal cools against it (ADR 0042)
	if d.is_empty() or int(d.get("version", 0)) != 1:
		return
	var p: Array = d.get("pos", [])
	if p.size() == 3:
		player.position = Vector3(p[0], p[1], p[2])
	var s: Array = d.get("spawn", [])
	if s.size() == 3:
		_spawn_pos = Vector3(s[0], s[1], s[2])
	deaths = int(d.get("deaths", 0))
	_monolith_up = int(d.get("monolith", 0)) == Monolith.VERSION   # older saves held true (1): raise it again
	player.set_look(float(d.get("yaw", 0.0)), float(d.get("pitch", 0.0)))
	var mode := GameMode.parse(String(d.get("game_mode", player.game_mode)))
	if mode != "":
		player.set_game_mode(mode)
	player.flying = bool(d.get("flying", false)) and GameMode.rule(player.game_mode, "can_fly")
	var slots: Array = d.get("slots", [])
	for i in mini(slots.size(), player_inv.inv.size()):
		player_inv.inv.set_slot(i, _stack_of(slots[i]))
	player_inv.bag_item = _stack_of(d.get("bag_item"))
	var bag: Array = d.get("bag", [])
	for i in mini(bag.size(), player_inv.bag.size()):
		player_inv.bag.set_slot(i, _stack_of(bag[i]))
	player_inv.off.set_slot(0, _stack_of(d.get("offhand")))   # ADR 0055
	world_map.waypoints.load_array(d.get("waypoints", []))
	if d.get("bookmarks") is Array and not OS.get_cmdline_user_args().has("autopilot=1"):
		inv_screen.browser.set_bookmarks(d["bookmarks"])   # the recipe browser's bookmarks (ADR 0115)
	player_inv.favourites.clear()
	for i in d.get("favourites", []):
		player_inv.favourites[int(i)] = true
	player_inv.select(int(d.get("selected", 0)))
	player_inv.health = clampi(int(d.get("health", player_inv.max_health)), 1, player_inv.max_health)
	player_inv.health_changed.emit(player_inv.health)
	_survival_save = d.get("survival", {})
	_stone_age_save = d.get("stone_age", {})
	_tnt_save = d.get("tnt", [])


## A saved stack (JSON numbers are floats), or {} for an empty or unknown one.
func _stack_of(v) -> Dictionary:
	if not (v is Dictionary) or not v.has("id") or not items.has(String(v["id"])):
		return {}
	return Inventory.make_stack(items, String(v["id"]), int(v.get("count", 1)), v)   # freshness, wear, heat, work (ADR 0040, 0042)


func _can_open_overlay() -> bool:
	return not player.ui_open and not player.autopilot and not inv_screen.is_open


var _overlay_over_ui := false   # the bug report box opened over the inventory or another window


func _on_overlay_opened() -> void:
	_overlay_over_ui = player.ui_open
	player.set_ui_open(true, false)


func _on_overlay_closed() -> void:
	if not chat.is_open and not debug_menu.is_open and not tools_menu.is_open and not _overlay_over_ui:
		player.set_ui_open(false)


func _on_game_mode_changed(mode: String) -> void:
	game_hud.set_hearts_visible(GameMode.rule(mode, "takes_damage"))
	inv_screen.browser_view.cheat_give = player_inv.creative_take if GameMode.rule(mode, "item_picker") else Callable()


## Godot 4.4+ can run the game inside the editor's Game tab; minimizing that would hide it.
func _embedded_in_editor() -> bool:
	return Engine.has_method("is_embedded_in_editor") and bool(Engine.call("is_embedded_in_editor"))


## The loading screen until the world around the player is in: at the start (new world or save),
## after a respawn and after a long teleport. The player and the animals stand still under it, so
## nothing happens before you can see it; workers and mesh uploads run flat out, since the cover
## hides any stutter. near = a teleport or respawn cover: it lifts once the ground around the player
## is in (voxels under the feet, full detail within 24 m), not the whole 2 km horizon, which streams
## in behind it (ADR 0066). Autopilot runs only get the first cover and freeze nothing (their scripts
## act from the start), except tp_test=1, which measures the teleport covers.
func cover_until_loaded(title: String, near := false) -> void:
	if _loading != null and is_instance_valid(_loading) and not _loading.is_done():
		return
	var first := _loading == null
	if not first and DebugCapture.autopilot and not DebugCapture.tp_test:
		return
	_loading = LoadingScreen.new()
	_loading.name = "LoadingScreen"
	_loading.title = title
	_loading.bubble = bubble
	if near:   # a teleport: lift once the ground around the player is in, not the whole horizon (ADR 0066)
		_loading.world = world
		_loading.player = player
	player.edits_locked = true
	var frozen := not DebugCapture.autopilot or (DebugCapture.tp_test and not first)
	if frozen:
		player.process_mode = Node.PROCESS_MODE_DISABLED
		mobs.process_mode = Node.PROCESS_MODE_DISABLED
	var threads := VoxelEngine.get_thread_count()
	VoxelEngine.set_thread_count(maxi(threads, OS.get_processor_count() - 2))
	var has_budget := "mesh_upload_budget_mb" in world.terrain
	if has_budget:
		world.terrain.mesh_upload_budget_mb = 0.0
	# the coarse blocks over the player first: clipbox shows the near detail only under them (ADR 0066)
	var chain := near and "viewer_chain_first" in world.terrain
	if chain:
		world.terrain.viewer_chain_first = true
	_loading.finished.connect(func() -> void:
		player.edits_locked = false
		if frozen:
			player.process_mode = Node.PROCESS_MODE_INHERIT
			mobs.process_mode = Node.PROCESS_MODE_INHERIT
		VoxelEngine.set_thread_count(threads)   # parks the extra workers, no join (ADR 0108)
		if chain:
			world.terrain.viewer_chain_first = false
		if has_budget:
			world.terrain.mesh_upload_budget_mb = Settings.mesh_upload_budget_mb)
	add_child(_loading)


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
	if bubble == null:
		return world.get_voxel(p)   # station views are rebuilt in _ready before the bubble exists
	var m := bubble.get_voxel(p)
	return m if m >= 0 else world.get_voxel(p)


func _on_broken(p: Vector3i, drops: Array) -> void:
	var was := get_world_voxel(p)
	if fx:
		fx.block_break(p, was)
	var furn := blocks.furniture_at(p)
	if furn != null:
		_remove_furniture(furn, GameMode.rule(player.game_mode, "drops"))
		return
	if _break_station(p, drops):
		return
	if _break_torch(p, drops):
		return
	var piece: RefCounted = blocks.at(p)
	if piece is GraveEntity:
		for w in waypoints.list.duplicate():
			if w["kind"] == "grave" and (w["pos"] as Vector3i) == piece.pos:
				waypoints.remove(int(w["id"]))
	if work and BlockEntities.is_work(piece):
		# any voxel of a work piece takes all of it, view and footprint (bug report 2026-10-25: a broken
		# firepit's other voxels stayed as ghosts when the break was not on its anchor)
		var stacks: Array = piece.contents()
		work.remove_piece(piece)
		for s in stacks:
			spawn_item(s, Vector3(piece.pos) + Vector3(0.5, 0.5, 0.5) * BlockEntities.width_of(piece.kind), Vector3(randf_range(-1.5, 1.5), 2.5, randf_range(-1.5, 1.5)))
		for d in drops:
			spawn_item(d, Vector3(p) + Vector3(0.5, 0.5, 0.5), Vector3(randf_range(-1, 1), 2.0, randf_range(-1, 1)))
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


## A brush dig (ADR 0034): the cells become air, plants and crops go through
## farming, one burst of chips for the material that was aimed, the drops
## spawn at the brush's centre.
func _on_dug(cells: Array, drops: Array, center: Vector3) -> void:
	var first := 0
	var events := []
	RuinLoot.shatter(self, cells)   # ruin pots break into shards and loot (ADR 0057)
	for c in cells:
		var was := get_world_voxel(c)
		if was <= 0:
			continue
		if first == 0:
			first = was
		set_world_voxel(c, GameMaterials.AIR)
		if not GameMaterials.is_plain_voxel(was):
			events.append_array(farming.on_broken(c))
	if fx and first > 0:
		fx.block_break(Vector3i(center.floor()), first)
	if not GameMode.rule(player.game_mode, "drops"):
		events.clear()
	_spawn_drop_events(events)
	for d in drops:
		var s: Dictionary = d.duplicate()
		s.erase("at")   # a ground pickup's drop pops where it lay (ADR 0043)
		spawn_item(s, d.get("at", center), Vector3(randf_range(-1, 1), 2.0, randf_range(-1, 1)))
	if pieces:
		pieces.check(cells, player.position)   # a cut that frees a trunk or a limb: it falls (P12)


## A stick or stones picked up by hand (ADR 0043): the piece's voxels go and its item pops out where
## it lay, to be picked up by walking over it.
func pop_pickup(cells: Array[Vector3i]) -> void:
	if cells.is_empty():
		return
	var split := GroundPickups.complete(get_world_voxel, cells)
	if split["pieces"].is_empty():
		return
	var pc: Dictionary = split["pieces"][0]
	if not RuinLoot.shatter(self, pc["cells"]).is_empty():
		return   # a pot: shards and loot, never the pot (ADR 0057)
	for c in pc["cells"]:
		set_world_voxel(c, GameMaterials.AIR)
	if fx:
		fx.block_break(pc["cells"][0], pc["material"])
	pickups_popped += 1
	if not GameMode.rule(player.game_mode, "drops"):
		return
	for d in mining.roll_drops(GameMaterials.name_of(pc["material"]), player_inv.held_def(), _pickup_rng):
		spawn_item(d, pc["center"], Vector3(randf_range(-0.5, 0.5), 2.5, randf_range(-0.5, 0.5)))


## A brush placement of a voxel item: every cell gets the material.
func _on_placed_many(cells: Array, material: int) -> void:
	for c in cells:
		set_world_voxel(c, material)
		stains.clear_cell(c)
	if material == GameMaterials.TNT and tnt:
		tnt.register_cells(cells)


## A big creative brush dug natively (ADR 0065): one burst of chips, and the cut checked for rigid
## pieces from seeds on a sphere around the box (like a crater's rim). Crops are left to the small
## brush (the mask skips them).
func _on_big_dug(box: AABB, material: int) -> void:
	var center := box.get_center()
	if fx and material > 0:
		fx.block_break(Vector3i(center.floor()), material)
	if pieces:
		pieces.check(PlaytestTools.rim_seeds(center, box.size.length() * 0.5 + 1.0), player.position)


## A big brush placement or a paint (filled natively): TNT in it becomes charges.
func _on_brush_filled(box: AABB, material: int) -> void:
	if material != GameMaterials.TNT or tnt == null:
		return
	var b := Settings.BLOCK
	var lo := Tnt.anchor_of(Vector3i(box.position.floor()))
	var hi := Vector3i(box.end.ceil()) + Vector3i.ONE
	for y in range(lo.y, hi.y, b):
		for z in range(lo.z, hi.z, b):
			for x in range(lo.x, hi.x, b):
				tnt.charges[Vector3i(x, y, z)] = true   # light() checks what is really there


## A right click with a firestarter (flint and steel, the igniter): TNT aimed at is lit (ADR 0065);
## flint and steel sets anything else on fire (Weather, ADR 0056); the igniter only lights TNT, and
## sneaking with it lights every charge within Settings.igniter_range_m.
func _ignite(hit: VoxelRaycastResult) -> bool:
	var def := player_inv.held_def()
	if tnt and tnt.light_at(hit):
		return true
	if str(def.get("ignites", "")) == "tnt":
		if player.sneaking() and tnt:
			var n := tnt.light_near(player.position, Settings.igniter_range_m)
			chat.add_message("Igniter: %d charge%s lit" % [n, "" if n == 1 else "s"])
			return n > 0
		return false
	return weather.ignite(hit) if weather else false


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
			spawn_item(s, Vector3(anchor) + Vector3.ONE * (Settings.BLOCK * 0.5), Vector3(randf_range(-1, 1), 2.0, randf_range(-1, 1)))   # the anchor block's centre
	if player.seat != Vector3.INF and e.furniture == "chair":
		player.stand_up()


## Opens or shuts a door, trapdoor or fence gate (ADR 0094): its voxels move to the other box and its
## model swings there. Returns false when something stands in the way (a wall, the player).
func toggle_door(e: FurnitureEntity) -> bool:
	var k: Dictionary = FurnitureEntity.KINDS[e.furniture]
	var mine := {}
	for c in e.cells:
		mine[c] = true
	var to := FurnitureEntity.footprint(e.furniture, e.pos, e.facing, not e.open)
	for c in to:
		if not mine.has(c) and (not player._placeable(get_world_voxel(c)) or player._overlaps_player(c)):
			return false
	var now := {}
	for c in to:
		now[c] = true
	for c in blocks.set_open(e, not e.open):
		if not now.has(c) and get_world_voxel(c) == GameMaterials.FURNITURE:
			set_world_voxel(c, GameMaterials.AIR)
	for c in to:
		if not mine.has(c):
			set_world_voxel(c, GameMaterials.FURNITURE)
	var hinge: Node3D = _furniture_views[e.pos].get_node_or_null("Hinge") if _furniture_views.has(e.pos) else null
	if hinge != null:
		var tw := create_tween()
		tw.tween_property(hinge, "rotation", k["swing"] if e.open else Vector3.ZERO, CHEST_SWING_S).set_trans(Tween.TRANS_QUAD).set_ease(Tween.EASE_OUT)
	Sound.play("block/chest_open" if e.open else "block/chest_close", e.light_at())
	return true


## Right click on a bed or straw bed: sleep through the night (Minecraft: 12542..23459 of its
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
	if e.can_open():
		toggle_door(e)
		return
	if not FurnitureEntity.KINDS[e.furniture].get("sleep", false):
		return   # a table, shelf, window...: nothing to use
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
	if material > 0 and GameMaterials.is_station(material):
		_place_station(p, material)
		return
	if material > 0 and GameMaterials.defs()[material - 1].get("entity", "") == "torch":
		_place_torch(p)
		return
	set_world_voxel(p, material)
	_spawn_drop_events(farming.on_placed(p, material))
	saplings.on_placed(p, material)
	if material > 0:
		stains.clear_cell(p)   # a filled cell's old stain must not come back when it is mined
		var d: Dictionary = GameMaterials.defs()[material - 1]
		if d.has("entity"):
			blocks.create(p, d["entity"])
		# a placed flower, mushroom or seashell grows into the piece the generator lays (ADR 0053)
		var st := PlantStamps.for_placed(material, p)
		for o in st:
			if o != Vector3i.ZERO and PlantStamps.is_free_cell(get_world_voxel(p + o)):
				set_world_voxel(p + o, st[o])
		if not st.is_empty() and st.get(Vector3i.ZERO, material) != material:
			set_world_voxel(p, st[Vector3i.ZERO])   # the stem's foot


## A torch (ADR 0053): a piece of voxels standing on the floor, or leaning out from the wall beside
## it when there is no floor; its cells go only where they are free.
func _place_torch(p: Vector3i) -> void:
	var wall := Vector3i.ZERO
	if not MobManager.is_solid(get_world_voxel(p + Vector3i.DOWN)):
		for d in [Vector3i(1, 0, 0), Vector3i(-1, 0, 0), Vector3i(0, 0, 1), Vector3i(0, 0, -1)]:
			if MobManager.is_solid(get_world_voxel(p + d)):
				wall = d
				break
	var e := blocks.create_torch(p, wall)
	var st := PlantStamps.torch(wall)
	for o in st:
		var q: Vector3i = p + o
		if o == Vector3i.ZERO or PlantStamps.is_free_cell(get_world_voxel(q)):
			set_world_voxel(q, st[o])
			stains.clear_cell(q)


## Breaks a whole torch from any of its voxels; false when `p` is not part of one.
func _break_torch(p: Vector3i, drops: Array) -> bool:
	var e = blocks.at(p)
	if not (e is TorchEntity):
		return false
	for c in e.cells:
		var m := get_world_voxel(c)
		if m > 0 and GameMaterials.defs()[m - 1].get("entity", "") == "torch":
			set_world_voxel(c, GameMaterials.AIR)
	blocks.remove(e.pos)
	for d in drops:
		spawn_item(d, e.flame(), Vector3(randf_range(-1, 1), 2.0, randf_range(-1, 1)))
	return true


var _last_health := -1


## Losing health bleeds: liquid blood spills from the body (StainMap.bleed).
## A tool wore out (ADR 0042): a word in the chat and a puff where the hand is.
func _on_tool_broke(id: String) -> void:
	tools_broken += 1
	if chat:
		chat.add_message("Your %s broke." % items.display_name(id))
	if fx:
		var cam := player.camera.global_transform
		fx.burst("poof", cam.origin - cam.basis.z * Settings.m2v(0.6) + Vector3(0, -Settings.m2v(0.3), 0), {"material": 0, "normal": Vector3.UP})


func _on_health_changed(h: int) -> void:
	if _last_health > h:
		stains.bleed(player.position, _last_health - h)
		if fx:
			fx.burst("bleed", player.position + Vector3(0, Settings.m2v(1.0), 0), {"count_bonus": _last_health - h})
	_last_health = h


# ---------------- stations (ADR 0007, 0034) ----------------

var _station_root: Node3D
var _station_views := {}   # anchor -> MeshInstance3D


func _place_station(anchor: Vector3i, material: int) -> void:
	var kind: String = GameMaterials.defs()[material - 1]["entity"]
	# the 1 m crafting table is bigger than the block the player's check covered (ADR 0053): no room, no table
	for c in BlockEntities.footprint_of(kind, anchor):
		var m := get_world_voxel(c)
		if not (PlantStamps.is_clearable_cell(m) or GameMaterials.is_liquid(GameMaterials.material_of_voxel(m))):
			if GameMode.rule(player.game_mode, "consumes_items"):
				spawn_item({"id": items.item_for_material(GameMaterials.name_of(material)), "count": 1}, Vector3(anchor) + Vector3.ONE * (Settings.BLOCK * 0.5))
			if chat:
				chat.add_message("No room for the %s here." % GameMaterials.name_of(material).replace("_", " "))
			return
	var e: RefCounted = blocks.create(anchor, kind)
	for c in BlockEntities.footprint(e):
		set_world_voxel(c, material)
		stains.clear_cell(c)
	_add_station_view(e)


func _add_station_view(e: RefCounted) -> void:
	if _station_views.has(e.pos):
		_station_views[e.pos].queue_free()
	var m := get_world_voxel(e.pos)
	if m <= 0 or not GameMaterials.is_station(m):
		m = GameMaterials.id_of({"station": "crafting_table", "chest": "chest", "furnace": "furnace", "storage_pot": "storage_pot"}.get(e.kind, "chest"))
	var mi := MeshInstance3D.new()
	var d: Dictionary = GameMaterials.defs()[m - 1]
	if StationEntity.SIZES.has(e.kind):
		mi.mesh = StationEntity.model_mesh(d["name"])   # the 1 m workbench, chest or furnace, in voxel units (ADR 0053, 0076)
		mi.position = Vector3(e.pos)
		if d["name"] == "chest":
			_add_chest_lid(mi)
		mi.visibility_range_end = 160.0
		_station_root.add_child(mi)
		_station_views[e.pos] = mi
		return
	if d.has("micro"):
		mi.mesh = _micro_view_mesh(d)
		mi.position = Vector3(e.pos)
	else:
		mi.mesh = BlockMeshes.cube(m, world.prop_material)
		mi.position = Vector3(e.pos) + Vector3.ONE * (Settings.BLOCK * 0.5)
	mi.scale = Vector3.ONE * Settings.BLOCK
	mi.visibility_range_end = 160.0
	_station_root.add_child(mi)
	_station_views[e.pos] = mi


## A chest is drawn as body + lid so the lid can swing open (bug report 2026-10-27_15-31-56): the lid
## hangs on a "Hinge" node whose position _swing_chest_lid moves to the back edge it opens from.
func _add_chest_lid(body: MeshInstance3D) -> void:
	body.mesh = StationEntity.model_mesh("chest_body")
	var hinge := Node3D.new()
	hinge.name = "Hinge"
	var lid := MeshInstance3D.new()
	lid.name = "Lid"
	lid.mesh = StationEntity.model_mesh("chest_lid")
	hinge.add_child(lid)
	body.add_child(hinge)


const CHEST_OPEN_RAD := 1.25   # about 70 degrees
const CHEST_SWING_S := 0.25
var _open_chest := Vector3i.MAX   # the anchor of the chest whose screen is open


## Swings a chest's lid open (hinged on the side away from the player, chests have no facing) or shut.
func _swing_chest_lid(anchor: Vector3i, open: bool) -> void:
	var view: Node = _station_views.get(anchor)
	var hinge: Node3D = view.get_node_or_null("Hinge") if view else null
	if hinge == null:
		return
	var lid: Node3D = hinge.get_node("Lid")
	if open:
		var depth := float(StationEntity.SIZES["chest"].z)
		var back := 0.0 if player.position.z > Vector3(anchor).z + depth * 0.5 else depth
		hinge.position = Vector3(0, StationEntity.CHEST_LID_Y, back)
		lid.position = -hinge.position
		hinge.rotation.x = 0.0
	var tw := create_tween()
	tw.tween_property(hinge, "rotation:x", (CHEST_OPEN_RAD if hinge.position.z > 0.0 else -CHEST_OPEN_RAD) if open else 0.0, CHEST_SWING_S) 		.set_trans(Tween.TRANS_BACK if open else Tween.TRANS_QUAD).set_ease(Tween.EASE_OUT if open else Tween.EASE_IN)


static var _micro_meshes := {}

## A station drawn from its micro pattern (8 cells per block, the storage pot).
func _micro_view_mesh(d: Dictionary) -> Mesh:
	if not _micro_meshes.has(d["name"]):
		var cells := GameMaterials.micro_pattern(d["micro"], d["tiles"])
		var g := GameMaterials.MICRO_GRID
		var mesh: ArrayMesh = MobModel.outward(MicroMesher.build(cells, Vector3i(g, g, g), g, GameMaterials.micro_paint_tiles(), GameMaterials.ATLAS_TILES, Vector3.ZERO))
		mesh.surface_set_material(0, world.prop_material)
		_micro_meshes[d["name"]] = mesh
	return _micro_meshes[d["name"]]


## Breaks a whole station from any of its voxels: the footprint empties, the
## view goes, contents spill. Returns false when `p` is not part of one.
func _break_station(p: Vector3i, drops: Array) -> bool:
	var e: RefCounted = blocks.at(p)
	if e == null or not (e.kind in ["station", "chest", "furnace", "storage_pot"]):
		return false
	var anchor: Vector3i = e.pos
	var centre := Vector3(anchor) + Vector3.ONE * (Settings.BLOCK * 0.5)
	for c in BlockEntities.footprint(e):
		var m := get_world_voxel(c)
		if m > 0 and GameMaterials.is_station(m):
			set_world_voxel(c, GameMaterials.AIR)
	for s in blocks.remove(anchor):
		spawn_item(s, centre, Vector3(randf_range(-1.5, 1.5), 2.5, randf_range(-1.5, 1.5)))
	for d in drops:
		spawn_item(d, centre, Vector3(randf_range(-1, 1), 2.0, randf_range(-1, 1)))
	if _station_views.has(anchor):
		_station_views[anchor].queue_free()
		_station_views.erase(anchor)
	return true


## Right click on a station block (ADR 0007).
func _on_block_used(p: Vector3i, material: int) -> void:
	var at: RefCounted = blocks.at(p)
	if at != null and not (at is FurnitureEntity):
		p = at.pos   # any voxel of a station opens the one at its anchor
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
			if e.kind == "chest":
				_open_chest = e.pos
				_swing_chest_lid(e.pos, true)
			if material == GameMaterials.FURNACE_LIT and not e.is_lit():
				_set_station_material(e, GameMaterials.FURNACE)   # lit block without fire (older save): put it out
			player.set_ui_open(true, false)
			inv_screen.open_container(e)


## A furnace started or stopped burning: swap furnace <-> furnace_lit, or
## drop the entity when its block is gone (burnt, replaced by the sim).
func _on_lit_flipped(p: Vector3i) -> void:
	var m := get_world_voxel(p)
	if m != GameMaterials.FURNACE and m != GameMaterials.FURNACE_LIT:
		for s in blocks.remove(p):
			spawn_item(s, Vector3(p) + Vector3.ONE * (Settings.BLOCK * 0.5))   # the furnace block's centre
		return
	var e: FurnaceEntity = blocks.at(p)
	_set_station_material(e, GameMaterials.FURNACE_LIT if e.is_lit() else GameMaterials.FURNACE)


## Swaps a station's footprint to another material (furnace <-> lit furnace) and redraws it.
func _set_station_material(e: RefCounted, material: int) -> void:
	for c in BlockEntities.footprint(e):
		set_world_voxel(c, material)
	_add_station_view(e)


## Debug (with open_inventory=1): search=text filters the item list,
## browse=item_id[:uses] opens its recipes (or uses) in the recipe view,
## craft_grid=id:count,,id:count fills the hand grid slot by slot (empty = skip a slot).
func _debug_browser() -> void:
	var args := {}
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=", true, 1)
		if kv.size() == 2:
			args[kv[0]] = kv[1]
	if args.has("craft_grid"):
		var cells: PackedStringArray = args["craft_grid"].split(",")
		for i in mini(cells.size(), inv_screen.player_inv.craft.grid.size()):
			var p := cells[i].split(":")
			if p.size() == 2:
				inv_screen.player_inv.craft.grid.set_slot(i, {"id": p[0], "count": int(p[1])})
		inv_screen.player_inv.changed.emit()
		print("debug craft_grid hint=%s out=%s" % [inv_screen.player_inv.craft.hint(), inv_screen.player_inv.craft.preview_output()])
	if args.has("search"):
		inv_screen.browser_view._search.text = args["search"]
		inv_screen.browser_view._search.text_changed.emit(args["search"])
	if args.get("quest_book", "0") == "click":
		await _debug_quest_clicks()
	if args.get("quest_book", "0") == "1":
		inv_screen.open_book()
		if args.has("quest_select"):
			inv_screen.quest_view.select(args["quest_select"])
		print("debug quest_book open=%s selected=%s %s" % [inv_screen.is_book_open(), inv_screen.quest_view.selected, quests.summary()])
	if args.has("browse"):
		var parts: PackedStringArray = args["browse"].split(":")
		var ok := inv_screen.browser_view.show_item(parts[0], parts[1] if parts.size() > 1 else "recipes")
		print("debug browse %s -> %s" % [args["browse"], ok])
	if args.has("hover_slot"):   # a tooltip capture: the mouse is not reliable in an off-screen window
		for _i in 30:
			await get_tree().process_frame
		var sv: Control = inv_screen._slots[int(args["hover_slot"])]
		inv_screen.debug_hover = sv
		for _i in 3:
			await get_tree().process_frame
		print("debug hover_slot %s tooltip=%s" % [args["hover_slot"], inv_screen._tooltip.text.replace("\n", " / ")])
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


## Debug (open_inventory=1 quest_book=click): the book through real mouse
## clicks: the button opens it, a quest node selects it, its first task's
## item closes the book and opens that item's recipes.
func _debug_quest_clicks() -> void:
	var click := func(c: Control) -> void:
		var at := c.get_global_rect().get_center()
		get_viewport().warp_mouse(at)
		var mm := InputEventMouseMotion.new()
		mm.position = at
		Input.parse_input_event(mm)
		for down in [true, false]:
			var e := InputEventMouseButton.new()
			e.button_index = MOUSE_BUTTON_LEFT
			e.pressed = down
			e.position = at
			Input.parse_input_event(e)
		for _i in 3:
			await get_tree().process_frame
	for _i in 1200:   # the loading screen covers everything until the world is in
		if not is_instance_valid(_loading) or not _loading.is_inside_tree() or not _loading.visible:
			break
		await get_tree().process_frame
	await click.call(inv_screen._book_button)
	var opened := inv_screen.is_book_open()
	var qv := inv_screen.quest_view
	await click.call(qv._nodes["shape_crucible"])
	var selected := qv.selected
	var row: Control = null
	for c in qv._detail.get_children():
		if c is QuestBookView.TaskRow:
			row = c
			break
	await click.call(row)
	print("debug quest_clicks %s" % JSON.stringify({"opened": opened, "selected": selected,
		"book_closed_after_task": not inv_screen.is_book_open(), "recipes": inv_screen.browser.summary().get("view", {})}))


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
	var base := player.position + fwd * Settings.m2v(2.0)   # metres: the 1 m stations (8 voxels) need room
	var kinds := [["crafting_table", GameMaterials.CRAFTING_TABLE], ["chest", GameMaterials.CHEST], ["furnace", GameMaterials.FURNACE]]
	var spots: Array[Vector3i] = []
	for i in kinds.size():
		var q := base + right * (i - 1) * Settings.m2v(1.5)
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
		# climb out of what surface_height does not see (the spawn monolith), to the first spot with room
		var kind: String = GameMaterials.defs()[kinds[i][1] - 1]["entity"]
		for _up in 40:
			if BlockEntities.footprint_of(kind, spots[i]).all(func(c: Vector3i) -> bool: return PlantStamps.is_clearable_cell(get_world_voxel(c))):
				break
			spots[i].y += 1
		var x := spots[i].x
		var y := spots[i].y
		var z := spots[i].z
		_on_placed(Vector3i(x, y, z), kinds[i][1])
		var e: RefCounted = blocks.at(Vector3i(x, y, z))
		if e is FurnaceEntity:
			e.inv.set_slot(FurnaceEntity.INPUT, {"id": "cobblestone", "count": 64})   # iron ore wants a bloomery now
			e.inv.set_slot(FurnaceEntity.FUEL, {"id": "coal", "count": 64})   # a burn takes items.units() of each
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
	spawn_item(stack, from, velocity, Settings.item_pickup_delay_s * 2.0).throw()


func _on_inventory_toggled(open: bool) -> void:
	if open:
		inv_screen.open()
	else:
		inv_screen.close()
		if map_screen:
			map_screen.close()


## Chests and storage pots within Settings.quick_stack_m of the player (quick-stack, plan item D11).
func _nearby_chests() -> Array:
	var out := []
	var r := Settings.m2v(Settings.quick_stack_m)
	for k in ["chest", "storage_pot"]:
		for e in blocks.all_of(k):
			if (Vector3(e.pos) + Vector3.ONE * Settings.BLOCK * 0.5).distance_to(player.position) <= r:
				out.append(e)
	return out


## R / U in the world (ADR 0093): the inventory opens on the recipes or uses of the aimed block's item,
## else the held one; a toast when neither has any.
func _lookup(ids: Array, mode: String) -> void:
	if player.ui_open:
		return
	var id := inv_screen.browser.lookup(ids, mode)
	if id == "":
		hud.show_toast("No %s to show" % ("recipes" if mode == "recipes" else "uses"))
		return
	player.set_ui_open(true)
	inv_screen.browser_view.show_item(id, mode)


## M: the full map (plan item D10).
func _open_map() -> void:
	if map_screen == null or player.ui_open:
		return
	player.set_ui_open(true, false)
	map_screen.open()


## The explored map, the minimap, the map screen and the waypoint beams.
func _setup_maps() -> void:
	world_map.name = "WorldMap"
	add_child(world_map)
	world_map.setup(self)
	minimap = Minimap.new()
	minimap.name = "Minimap"
	game_hud.add_child(minimap)
	minimap.setup(self)
	map_screen = MapScreen.new()
	map_screen.name = "MapScreen"
	add_child(map_screen)
	map_screen.setup(self)
	map_screen.closed.connect(func() -> void:
		if player.ui_open:
			player.set_ui_open(false))
	waypoint_markers = WaypointMarkers.new()
	waypoint_markers.name = "WaypointMarkers"
	add_child(waypoint_markers)
	waypoint_markers.setup(self)


func _on_screen_closed(dropped: Array) -> void:
	if _open_chest != Vector3i.MAX:
		_swing_chest_lid(_open_chest, false)
		_open_chest = Vector3i.MAX
	for s in dropped:
		_on_item_dropped(s, player.camera.global_position, -player.camera.global_transform.basis.z * 2.0)


func _on_died() -> void:
	if _dying:
		return   # hunger, cold, a bite... keep landing while the apetion is rebuilt
	print("player died at %s (%s), respawning" % [player.position, player_inv.last_cause])
	if GameMode.rule(player.game_mode, "consumes_items"):
		_make_grave(Vector3i(player.position.floor()))   # survival: it all stays where you fell (plan item D12)
	deaths += 1
	if DebugCapture.tp_test:
		_respawn()   # tp_test times the bare respawn
		return
	# ADR 0098: in third person the body crumbles to powder, the Gelvin terminal rebuilds it, and at
	# the bed or spawn it swirls back together out of thin air before the view returns to the eyes
	_dying = true
	var view := player.perspective
	var mode := player.process_mode
	player.process_mode = Node.PROCESS_MODE_DISABLED   # frozen where it fell
	player.set_perspective(1)
	var dust := ApeDust.crumble(player.model, player, self)
	await dust.finished
	await get_tree().create_timer(0.4).timeout
	await death_screen.play(deaths, player_inv.last_cause, _respawn)
	dust.queue_free()
	player.set_perspective(1)   # respawn_at may move the camera; the body stays hidden until built
	dust = ApeDust.assemble(player.model, player, self)
	await dust.finished
	dust.queue_free()
	player.set_perspective(view)
	player.process_mode = mode
	_dying = false


func _respawn() -> void:
	player.respawn_at(_spawn_pos)
	if survival:
		survival.reset()


## Everything the player carries (inventory, offhand, bag and what is in it, the cursor, the crafting
## grid) goes into a grave at the first free block at or above `feet` (lava, water and plants make way;
## up to 32 blocks up). It gets a waypoint and its coordinates go to chat. Nothing to keep: no grave.
func _make_grave(feet: Vector3i) -> GraveEntity:
	if inv_screen and inv_screen.is_open:
		inv_screen.close()
	var stacks: Array = []
	for i in player_inv.inv.size():
		stacks.append(player_inv.inv.get_slot(i))
		player_inv.inv.set_slot(i, {})
	stacks.append(player_inv.offhand())
	player_inv.off.set_slot(0, {})
	for i in player_inv.bag.size():
		stacks.append(player_inv.bag.get_slot(i))
		player_inv.bag.set_slot(i, {})
	stacks.append(player_inv.bag_item)
	player_inv.bag_item = {}
	stacks.append(player_inv.cursor)
	player_inv.cursor = {}
	stacks = stacks.filter(func(s: Dictionary) -> bool: return not s.is_empty())
	if stacks.is_empty() or work == null:
		return null
	var anchor := BlockEntities.anchor_for(feet)
	for _k in 32:
		var free := true
		for c in BlockEntities.cube(anchor):
			var m := get_world_voxel(c)
			if not (m == GameMaterials.AIR or player._placeable(m)):
				free = false
				break
		if free:
			break
		anchor += Vector3i(0, Settings.BLOCK, 0)
	var g: GraveEntity = work._create(anchor, "grave", GameMaterials.WORK_BLOCK)
	g.set_stacks(stacks)
	if atmosphere:
		g.died_at = atmosphere.calendar.date_text()
	waypoints.add("Grave", anchor, "grave")
	if chat:
		chat.add_message("You died. Your things are in a grave at %d %d %d (a waypoint on the map, M)." % [anchor.x, anchor.y, anchor.z])
	player_inv.changed.emit()
	return g


# ---------------- item entities ----------------

## Hot metal in a hand without tongs in the other (ADR 0087): it falls at the player's feet and burns.
func _burn_hands() -> void:
	var dropped := player_inv.take_hot_from_hands()
	if dropped.is_empty():
		return
	for s in dropped:
		spawn_item(s, player.position + Vector3(0, Settings.BLOCK, 0) - player.global_transform.basis.z * Settings.BLOCK, Vector3(0, 1.5, 0), 1.0)
	if chat:
		chat.add_message("Too hot to hold: pick hot metal up with tongs in the other hand.")
	if GameMode.rule(player.game_mode, "takes_damage"):
		player_inv.damage(Settings.hot_hand_damage, "hot metal in bare hands")
		if player_inv.is_dead():
			player.died.emit()


func spawn_item(stack: Dictionary, pos: Vector3, velocity: Vector3 = Vector3.ZERO, extra_delay: float = 0.0) -> ItemEntity:
	var e := ItemEntity.create(items, stack, world.prop_material)
	e.position = pos
	e.pickup_delay += extra_delay   # (was a negative age, which also held a thrown item in the air that long)
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
		_burn_hands()
	var alive: Array[ItemEntity] = []
	for e in _item_entities:
		if not is_instance_valid(e) or e.is_queued_for_deletion():
			item_stats["invalid"] += 1
			continue
		if e.expired():
			item_stats["expired"] += 1
			e.queue_free()
			continue
		if e.burning():
			_burn_item(e)
			continue
		if e.can_pickup() and e.position.distance_to(centre) <= radius and GameMode.rule(player.game_mode, "interacts") 				and (Inventory.heat_of(e.stack) <= 0.0 or player_inv.has_tongs()):   # hot metal only with tongs (ADR 0087)
			var left := player_inv.pickup(e.stack["id"], e.stack["count"], e.stack)
			var got: int = e.stack["count"] - left
			if got > 0:
				picked_up_total[e.stack["id"]] = int(picked_up_total.get(e.stack["id"], 0)) + got
				if game_hud:
					var u := items.units(e.stack["id"])   # voxel items count in blocks here
					game_hud.note_pickup(items.display_name(e.stack["id"]), maxi(1, got / u) if u > 1 else got)
			if left > 0 and game_hud:
				game_hud.note_full()
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
				if b.is_queued_for_deletion() or not Inventory.same_kind(a.stack, b.stack["id"], b.stack):
					continue
				if a.position.distance_to(b.position) > merge_r:
					continue
				var max_stack := items.max_stack(a.stack["id"])
				var room: int = max_stack - a.stack["count"]
				if room <= 0:
					continue
				var take: int = mini(room, b.stack["count"])
				var merged: Dictionary = a.stack.duplicate()
				Inventory.merge_into(merged, take, b.stack)   # freshness and heat average by count (ADR 0040, 0042)
				merged.erase("count")
				a.stack.merge(merged, true)
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


## A dropped stack that touched lava burns up (bug report 2026-10-25): gone, with a flare of
## flame and a puff of smoke where it went in (`item_burn` sizzles, data/sounds.json).
func _burn_item(e: ItemEntity) -> void:
	item_stats["burned"] += 1
	if fx:
		var at := e.position + Vector3(0, ItemEntity.SIZE * 0.5, 0)
		fx.burst("item_burn", at, {"material": 0, "normal": Vector3.UP})
		fx.burst("item_burn_smoke", at, {"material": 0, "normal": Vector3.UP})
	e.queue_free()


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
		"pieces": pieces.stats() if pieces else {},
		"build": builder.snapshot() if builder else {},
		"survival": survival.snapshot() if survival else {},
		"loop": survival_loop.snapshot() if survival_loop else {},
		"stains": stains.stats() if stains else {},
		"look": look,
		"player": player_inv.summary(),
		"player_stains": player.stains.summary(),
		"swim": {"air": snappedf(player.air, 0.01), "in_liquid": player.in_liquid, "on_floor": player.is_on_floor()},
		"picked_up": picked_up_total,
		"items_in_world": _item_entities.size(),
		"item_stats": item_stats,
		"items_sample": _items_sample(),
		"inventory_open": inv_screen.is_open,
		"browser": inv_screen.browser.summary(),
		"blocks": blocks.summary(),
		"work": work.summary() if work else {},
		"stone_age": stone_age.summary() if stone_age else {},
		"quests": quests.summary() if quests else {},
		"quest_check_ms_max": snappedf(quest_check_ms_max, 0.001),
		"projectiles": projectiles.stats if projectiles else {},
		"gadgets": gadgets.stats if gadgets else {},
		"tnt": tnt.snapshot() if tnt else {},
		"networks": Networks.stats,
		"brush": player.brush_tool.stats.merged({"shape": player.brush_shape, "size": player.brush_size, "dig_size": player.dig_size()}),
		"weather": weather.summary() if weather else {},
		"map": world_map.summary(),
		"torches": torch_lights.stats if torch_lights else {},
		"mobs": mobs.summary() if mobs else {},
		"farm": farming.summary() if farming else {},
		"particles": fx.stats if fx else {},
		"sound": Sound.stats(),
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


## Debug (farm_test=1): Minecraft's 9x9 field of 0.5 m blocks (4.5 m) around one block of
## water, its centre 6 m ahead, tilled and sown with wheat (carrots on the right half);
## farm_speed=N runs random ticks N x faster.
func _debug_farm() -> void:
	await get_tree().create_timer(0.5).timeout
	var fwd := -player.camera.global_transform.basis.z
	fwd.y = 0
	fwd = fwd.normalized()
	var c := player.position + fwd * Settings.m2v(6.0)
	var B := Settings.BLOCK
	var cx := floori(c.x / B) * B
	var cz := floori(c.z / B) * B
	var gy := world.surface_height(cx, cz)
	var tilled := 0
	var planted := 0
	for bx in range(-4, 5):
		for bz in range(-4, 5):
			for vx in B:
				for vz in B:
					var p := Vector3i(cx + bx * B + vx, gy, cz + bz * B + vz)
					for k in range(1, 3 * B):
						set_world_voxel(p + Vector3i(0, k, 0), GameMaterials.AIR)
					if not GameMaterials.is_grass(get_world_voxel(p)) and get_world_voxel(p) != GameMaterials.DIRT:
						set_world_voxel(p, GameMaterials.DIRT)
					if bx == 0 and bz == 0:
						set_world_voxel(p, GameMaterials.WATER_STATIC)
						continue
					if farming.till(p):
						tilled += 1
						if farming.plant(p, "carrots" if bx > 0 else "wheat"):
							planted += 1
	print("debug farm at %s: tilled %d planted %d" % [Vector3i(cx, gy, cz), tilled, planted])
	if player.autopilot:
		player.autopilot_action = "look"
		player.set_look(player._yaw, DebugCapture.pitch)


# ---------------- debug: swimming (swim_test=1) ----------------

## Debug (swim_test=1): digs a 5 x 5 pool four deep with its water one voxel
## under the bank, drops the player in (survival, real input), and checks it
## sinks to the floor, loses air, drowns, and hops out onto the bank holding
## forward and jump. Prints one JSON line.
func _debug_swim() -> void:
	var r := {}
	player.set_game_mode(GameMode.SURVIVAL)
	# a stone tub of 0.5 m blocks (ADR 0034): 5 x 5 blocks of water 4 blocks deep, its surface one block under the rim
	var B := Settings.BLOCK
	var base := Vector3i(player.position.floor())
	var c := Vector3i(base.x, 0, base.z - 4 * B)
	var g: int = world.surface_height(c.x, c.z)
	_debug_fill(Vector3i(c.x - 3 * B, g - 5 * B, c.z - 3 * B), Vector3i(c.x + 4 * B, g + B, c.z + 4 * B), GameMaterials.STONE)
	_debug_fill(Vector3i(c.x - 2 * B, g - 4 * B, c.z - 2 * B), Vector3i(c.x + 3 * B, g, c.z + 3 * B), GameMaterials.WATER_STATIC)
	_debug_fill(Vector3i(c.x - 2 * B, g, c.z - 2 * B), Vector3i(c.x + 3 * B, g + 4 * B, c.z + 3 * B), GameMaterials.AIR)
	_debug_fill(Vector3i(c.x - 3 * B, g + B, c.z - 3 * B), Vector3i(c.x + 4 * B, g + 5 * B, c.z + 4 * B), GameMaterials.AIR)
	await get_tree().create_timer(0.5).timeout
	player.autopilot = false
	player.teleport(Vector3(c.x + 0.5, g - 1.5 * B, c.z + 0.5))
	await get_tree().create_timer(3.0).timeout
	r["floor_y"] = g - 4 * B
	r["y_after_sink"] = snappedf(player.position.y, 0.01)
	r["stands_on_floor"] = player.is_on_floor() and player.in_liquid and absf(player.position.y - (g - 4 * B)) < 0.05
	r["air_after_3s"] = snappedf(player.air, 0.01)
	var hp := player_inv.health
	player.air = 0.0
	await get_tree().create_timer(2.2).timeout
	r["drowned_hp"] = hp - player_inv.health
	player_inv.heal(20)
	# swim to the +z bank (its top is one block over the water) holding forward and jump
	player.set_look(PI, 0.0)
	Input.action_press("move_forward")
	Input.action_press("jump")
	var t := 0.0
	while t < 6.0 and not (player.is_on_floor() and not player.in_liquid and player.position.y >= g + B - 0.01):
		await get_tree().process_frame
		t += get_process_delta_time()
	Input.action_release("move_forward")
	Input.action_release("jump")
	r["climbed_out"] = player.position.y >= g + B - 0.01 and not player.in_liquid
	r["climb_s"] = snappedf(t, 0.01)
	await get_tree().create_timer(1.2).timeout   # breath comes back at half a bar per second
	r["air_refilled"] = player.air > 0.5
	player.autopilot = true
	print("debug swim " + JSON.stringify(r))


# ---------------- debug: the player's own hands (hands_test=1) ----------------

func _hold(id: String) -> void:
	for i in PlayerInventory.HOTBAR:
		var st: Dictionary = player_inv.inv.get_slot(i)
		if not st.is_empty() and st["id"] == id:
			player_inv.select(i)
			return
	if player_inv.pickup(id, 1) > 0:
		push_error("_hold: no room for %s" % id)   # (a full inventory recursed forever)
		return
	for i in PlayerInventory.HOTBAR:
		var st: Dictionary = player_inv.inv.get_slot(i)
		if not st.is_empty() and st["id"] == id:
			player_inv.select(i)
			return
	# a full hotbar: swap it in from the main grid, into an empty hotbar slot or the selected one
	for j in range(PlayerInventory.HOTBAR, player_inv.inv.size()):
		var st: Dictionary = player_inv.inv.get_slot(j)
		if st.is_empty() or st["id"] != id:
			continue
		var i: int = player_inv.selected
		for h in PlayerInventory.HOTBAR:
			if player_inv.inv.is_empty_slot(h):
				i = h
				break
		player_inv.inv.set_slot(j, player_inv.inv.get_slot(i))
		player_inv.inv.set_slot(i, st)
		player_inv.select(i)
		return
	push_error("_hold: %s went to the main grid, not the hotbar" % id)


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


## Debug: fills the world box [lo, hi) through the bubble in one native call, voxel by voxel when it cannot.
func _debug_fill(lo: Vector3i, hi: Vector3i, m: int) -> void:
	if bubble.fill_box(lo, hi, m) >= 0:
		return
	for z in range(lo.z, hi.z):
		for y in range(lo.y, hi.y):
			for x in range(lo.x, hi.x):
				set_world_voxel(Vector3i(x, y, z), m)


## Debug (hands_test=1): till, sow, place a chair and a bed, sit, feed, shear,
## kill a sheep and eat, all through the player's input code; prints one JSON line.
func _debug_hands() -> void:
	var r := {}
	await get_tree().create_timer(0.6).timeout
	player.autopilot_action = "look"
	# a flat 9 x 9 block patch of grass on the block grid (ADR 0034), so the aim ray hits the ground
	var B := Settings.BLOCK
	var base := BlockEntities.anchor_for(Vector3i(player.position.floor()))
	var g0 := floori(float(world.surface_height(base.x, base.z)) / B) * B - 1
	var lo := Vector3i(base.x - 4 * B, g0 - B, base.z - 8 * B)
	var hi := Vector3i(base.x + 5 * B, g0 + 1, base.z + B)
	_debug_fill(lo, Vector3i(hi.x, g0, hi.z), GameMaterials.DIRT)
	_debug_fill(Vector3i(lo.x, g0, lo.z), hi, GameMaterials.GRASS)
	_debug_fill(Vector3i(lo.x, g0 + 1, lo.z), Vector3i(hi.x, g0 + 1 + 4 * B, hi.z), GameMaterials.AIR)
	await get_tree().create_timer(1.0).timeout   # the cleared patch reaches the terrain the aim ray reads
	var soil := Vector3i(base.x, g0, base.z - 3 * B)
	_hold("flint_hoe")
	r["hoe_aim"] = _aim_use(Vector3(soil) + Vector3(0.5, 0.99, 0.5)) == soil
	r["tilled"] = GameMaterials.name_of(get_world_voxel(soil))
	_hold("wheat_seeds")
	_aim_use(Vector3(soil) + Vector3(0.5, 0.99, 0.5))
	r["sown"] = GameMaterials.name_of(get_world_voxel(soil + Vector3i.UP))
	r["crops"] = farming.crops.size()
	# chair two to the right, bed three to the left
	var cp := Vector3i(base.x + 2 * B, g0, base.z - 3 * B)
	_hold("chair")
	r["chair_aim"] = _aim_use(Vector3(cp) + Vector3(0.5, 0.99, 0.5)) == cp
	var ch := blocks.furniture_at(cp + Vector3i.UP)
	r["chair"] = ch.summary() if ch != null else "not placed"
	await get_tree().create_timer(0.3).timeout
	if ch != null:
		_aim_use(Vector3(cp + Vector3i.UP) + Vector3(0.5, 0.5, 0.5) * B)
		r["sitting"] = player.seat != Vector3.INF
		player.stand_up()
		r["stood_up_at"] = player.position.snapped(Vector3.ONE * 0.1)
	var bp := Vector3i(base.x - 3 * B, g0, base.z - 2 * B)
	_hold("bed")
	r["bed_aim"] = _aim_use(Vector3(bp) + Vector3(0.5, 0.99, 0.5)) == bp
	var bed := blocks.furniture_at(bp + Vector3i.UP)
	r["bed"] = bed.summary() if bed != null else "not placed"
	await get_tree().create_timer(0.3).timeout   # the bed's voxels reach the terrain the aim ray reads
	if bed != null:
		atmosphere.set_time_of_day(0.9)
		_aim_use(Vector3(bed.pos) + Vector3(0.5, 0.9, 0.5) * B)
		r["time_after_sleep"] = snappedf(atmosphere.time_of_day, 0.01)
		r["spawn_after_sleep"] = _spawn_pos.snapped(Vector3.ONE * 0.1)
	# a sheep ahead: feed, shear, kill
	var sp := Vector3i(base.x, g0 + 1, base.z - 5 * B)
	var sheep := mobs.spawn("sheep", Vector3(sp) + Vector3(0.5, 0.05, 0.5) * B)
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
		sheep.position = Vector3(sp) + Vector3(0.5, 0.05, 0.5) * B   # it panics away between hits: bring it back in reach
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


## Debug (building_test=1, ADR 0094): a planks room on a flat patch; places every door, fence, ladder
## and decoration piece through the player's input code, films the room with the door shut and open,
## stands at the ladder; prints one JSON line and saves out/building.png (shut | open).
func _debug_building() -> void:
	var r := {}
	await get_tree().create_timer(0.6).timeout
	player.autopilot_action = "look"
	var B := Settings.BLOCK
	var base := BlockEntities.anchor_for(Vector3i(player.position.floor()))
	var g0 := floori(float(world.surface_height(base.x, base.z)) / B) * B - 1
	var lo := Vector3i(base.x - 6 * B, g0 - B, base.z - 10 * B)
	var hi := Vector3i(base.x + 6 * B, g0 + 1, base.z + 2 * B)
	_debug_fill(lo, Vector3i(hi.x, g0 + 1, hi.z), GameMaterials.PLANKS)
	_debug_fill(Vector3i(lo.x, g0 + 1, lo.z), Vector3i(hi.x, g0 + 1 + 8 * B, hi.z), GameMaterials.AIR)
	var back := base.z - 9 * B   # the back wall, one voxel thick, and a doorframe off the block grid
	var zp := base.z - 5 * B - 1
	_debug_fill(Vector3i(lo.x, g0 + 1, back), Vector3i(hi.x, g0 + 1 + 6 * B, back + 1), GameMaterials.PLANKS)
	_debug_fill(Vector3i(base.x + 2 * B, g0 + 1, zp), Vector3i(base.x + 6 * B, g0 + 1 + 5 * B, zp + 1), GameMaterials.PLANKS)
	_debug_fill(Vector3i(base.x + 3 * B, g0 + 1, zp), Vector3i(base.x + 5 * B, g0 + 1 + 4 * B, zp + 1), GameMaterials.AIR)
	await get_tree().create_timer(1.0).timeout
	var stand := Vector3(base) + Vector3(0.5 * B, g0 + 1 - base.y, -3.5 * B)   # in reach of the back wall
	var floor_at := func(x: int, z: int) -> Vector3: return Vector3(base.x + x * B + 0.5, g0 + 0.99, base.z + z * B + 0.5)
	var wall_at := func(x: int, y: float) -> Vector3: return Vector3(base.x + x * B + 0.5, g0 + 1 + y * B, back + 0.01)
	var placed := {}
	var put := func(id: String, at: Vector3) -> void:
		_hold(id)
		if FurnitureEntity.KINDS[id].get("wall", false):   # wall pieces face the way you look: stand square in front
			player.position = Vector3(at.x, g0 + 1, minf(at.z + 3 * B, base.z + 2 * B))
		else:
			player.position = stand
		r["hit_" + id] = GameMaterials.name_of(get_world_voxel(_aim_use(at)))
		placed[id] = blocks.all_furniture().filter(func(e: FurnitureEntity) -> bool: return e.furniture == id).size() > 0
	put.call("painting", wall_at.call(-4, 2.5))
	put.call("ladder", wall_at.call(1, 0.1))
	put.call("shelf", floor_at.call(-1, -8))
	put.call("table", floor_at.call(-5, -6))
	await get_tree().create_timer(0.3).timeout   # the table's voxels reach the terrain the aim ray reads
	var top := Vector3i(0, -1000000, 0)
	for e in blocks.all_furniture():
		if e.furniture == "table":
			for c in e.cells:
				if c.y > top.y:
					top = c
	put.call("flower_pot", Vector3(top) + Vector3(0.5, 0.99, 0.5))
	put.call("lantern", floor_at.call(1, -6))
	put.call("window", floor_at.call(-1, -4))
	put.call("trapdoor", floor_at.call(2, -1))
	put.call("fence", floor_at.call(-5, -2))
	put.call("fence_gate", floor_at.call(-3, -2))
	player.position = Vector3(base.x + 4 * B, g0 + 1, zp + 3 * B)   # a door faces the way you look: face the doorway
	put.call("door", Vector3(base.x + 4 * B + 0.5, g0 + 0.99, zp + 0.5))
	r["placed"] = placed
	await get_tree().create_timer(0.5).timeout
	var door: FurnitureEntity = null
	for e in blocks.all_furniture():
		if e.furniture == "door":
			door = e
	var eye := Vector3(base) + Vector3(0.5 * B, g0 + 1 - base.y, 3.5 * B)
	player.position = eye
	player.set_look(0.0, -0.3)
	await get_tree().create_timer(1.0).timeout
	var shut := get_viewport().get_texture().get_image()
	if door != null:
		_hold("stick")
		player.position = Vector3(base.x + 4 * B, g0 + 1, zp + 3 * B)
		_aim_use(Vector3(door.cells[door.cells.size() / 2]) + Vector3(0.5, 0.5, 0.5))
		r["door_open"] = door.open
		r["doorway_voxel"] = GameMaterials.name_of(get_world_voxel(Vector3i(base.x + 4 * B, g0 + 1 + B, zp)))
		player.position = eye
		player.set_look(0.0, -0.3)
	await get_tree().create_timer(1.0).timeout
	var open := get_viewport().get_texture().get_image()
	var sheet := Image.create(shut.get_width() * 2, shut.get_height(), false, shut.get_format())
	sheet.blit_rect(shut, Rect2i(Vector2i.ZERO, shut.get_size()), Vector2i.ZERO)
	sheet.blit_rect(open, Rect2i(Vector2i.ZERO, open.get_size()), Vector2i(shut.get_width(), 0))
	DirAccess.make_dir_recursive_absolute(DebugCapture.out_dir)
	sheet.save_png(DebugCapture.out_dir + "/building.png")
	# stand at the ladder: the climb check sees it
	player.position = Vector3(base.x + 1 * B + 2, g0 + 1, back + 2 + Settings.m2v(Settings.player_radius_m) + 0.2)
	r["on_ladder"] = player._on_ladder()
	r["lights"] = torch_lights.stats
	print("debug building " + JSON.stringify(r))
