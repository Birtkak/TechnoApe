class_name GameCommands
extends RefCounted

## The commands a player can type in chat (ADR 0018). Holds the game objects
## the handlers touch; any of them may be null (headless tests), and a
## command that needs a missing one fails with a message instead of crashing.
## Coordinates are voxels, as F3 shows them ("voxel ..."); one voxel = 0.5 m.

## Minecraft's named times on our clock (0 midnight, 0.25 sunrise, 0.5 noon).
const TIMES := {"sunrise": 0.25, "day": 0.29, "noon": 0.5, "sunset": 0.75, "night": 0.79, "midnight": 0.0}

## Playtest kits (/kit): item id -> count. test_commands.gd checks every id exists.
const KITS := {
	"tools": {"iron_pickaxe": 1, "copper_pickaxe": 1, "copper_axe": 1, "copper_shovel": 1, "flint_knife": 1, "bucket": 2, "water_bucket": 1, "lava_bucket": 1, "coal": 32, "oak_log": 64},
	"builder": {"stone": 64, "cobblestone": 64, "dirt": 64, "grass_block": 64, "sand": 64, "oak_planks": 64, "oak_log": 64, "glass": 64, "sandstone": 64},
	"stations": {"crafting_table": 4, "furnace": 4, "chest": 4, "iron_ore": 16, "copper_ore": 16, "tin_ore": 16, "coal": 32},
	"farm": {"iron_hoe": 1, "wheat_seeds": 32, "carrot": 16, "wheat": 32, "water_bucket": 2, "shears": 1, "bed": 1, "chair": 4, "wool": 16},
	"stone": {"flint": 8, "stick": 16, "clay_ball": 16, "dry_grass": 12, "loose_stone": 4, "loose_copper_ore": 8, "firestarter": 1, "oak_log": 8, "cordage": 6},   # Vintage Story early game (ADR 0023)
	"flint": {"flint_spear": 1, "flint_knife": 1, "flint_axe": 1, "raw_hide": 2, "firewood": 32, "dirt": 32, "firestarter": 1, "torch": 16,
		"pan": 1, "cooking_pot": 1, "clay_bowl": 3, "raw_meat": 4, "dry_grass": 24, "wattle": 8, "daub": 8, "leather_bag": 1},   # the deeper Flint Age (ADR 0030)
	"magic": {"wand_basic": 1, "spell_spark_bolt": 1, "spell_fire_bolt": 1, "spell_speed_up": 1, "mana_dust": 64},
}

## What WorldGen.find_nearest can look for (biome names work too).
const FEATURES := ["cave", "deep_cave", "ruin", "waterfall", "lake", "river", "spring", "marsh", "alpine", "snow", "mountain", "forest", "old_growth_forest", "taiga", "jungle", "plains", "desert",
	"coal_vein", "copper_vein", "cassiterite_vein", "iron_vein", "copper_iron_vein",
	"sulfur_cave", "lush_cave", "magma_cave", "crystal_cave", "fungal_cave"]

## /fill's limit: 64^3, about one bubble chunk column's worth of remeshing.
const FILL_MAX := 262144

var reg: CommandRegistry
var player: Player
var player_inv: PlayerInventory
var main: Node   # Main: atmosphere, spawn_item, spawn position
var sky: SkyCommands   # /date, /moon, /eclipse, /constellations (ADR 0025)


func _init(p_reg: CommandRegistry, p_player: Player, p_inv: PlayerInventory, p_main: Node = null) -> void:
	reg = p_reg
	player = p_player
	player_inv = p_inv
	main = p_main
	reg.register("gamemode", "/gamemode <survival|creative>", "Switch game mode (0, 1, s and c work too)",
		_gamemode, [GameMode.names()], ["gm"])
	reg.register("give", "/give <item> [count]", "Give yourself items", _give, [_item_ids])
	reg.register("clear", "/clear [item]", "Empty your inventory, or remove one item", _clear, [_item_ids])
	reg.register("tp", "/tp <x> <y> <z>", "Teleport to a voxel position; ~ is relative (~ ~10 ~)",
		_tp, [["~"], ["~"], ["~"]], ["teleport"])
	reg.register("time", "/time set <day|noon|sunset|night|midnight|sunrise|0..1>", "Set or show (/time query) the time of day",
		_time, [["set", "query"], TIMES.keys()])
	reg.register("kill", "/kill", "Die and respawn at the world spawn", _kill)
	reg.register("heal", "/heal", "Restore full health", _heal)
	reg.register("stain", "/stain <wet|bloody|oily|toxic|burning|clear>", "Coat the ape like touching that liquid (Noita stains)", _stain, [PlayerStains.KINDS + ["clear"]])
	reg.register("seed", "/seed", "Show the world seed", _seed)
	reg.register("kit", "/kit <%s>" % "|".join(KITS.keys()), "Give a playtest kit", _kit, [KITS.keys()])
	reg.register("spawn", "/spawn", "Teleport to the world spawn", _spawn)
	reg.register("locate", "/locate <feature>", "Find the nearest cave, ruin, lake, biome ... (within 4 km)", _locate, [FEATURES])
	reg.register("goto", "/goto <feature>", "Teleport to the nearest cave, ruin, lake, biome ...", _goto, [FEATURES])
	reg.register("daycycle", "/daycycle <on|off|seconds>", "Run the sun (on = 20 min days) or freeze it", _daycycle, [["on", "off"]])
	reg.register("blast", "/blast <radius_m> [material]", "Carve a crater where you look, or drop a ball of a material (water, sand, lava ...)",
		_blast, [["1", "2", "4", "8"], _material_names])
	reg.register("setblock", "/setblock <x> <y> <z> <material>", "Set one voxel; ~ is relative to you (~ ~-1 ~ is under your feet)",
		_setblock, [["~"], ["~"], ["~"], _material_names])
	reg.register("fill", "/fill <x1> <y1> <z1> <x2> <y2> <z2> <material>", "Fill a box (corners included, up to %d voxels) inside the sim bubble" % FILL_MAX,
		_fill, [["~"], ["~"], ["~"], ["~"], ["~"], ["~"], _material_names])
	reg.register("stations", "/stations", "Place a crafting table, chest and furnace ahead", _stations)
	reg.register("summon", "/summon <sheep|pig|chicken|bear> [count] [baby|hungry]", "Spawn animals where you look (ADR 0022); hungry predators hunt at once (ADR 0027)",
		_summon, [["sheep", "pig", "chicken", "bear"], ["1", "2", "4", "8"], ["baby", "hungry"]])
	reg.register("quests", "/quests [list|complete <id|all>|reset]", "Show or change progression book progress (ADR 0028)",
		_quests, [["list", "complete", "reset"], _quest_ids])
	reg.register("fell", "/fell [direction_deg|pillar]", "Fell the thickest tree 8-20 m away, across your view (rigid_pieces=1)", _fell)
	reg.register("bug", "/bug <what went wrong>", "Save a bug report (screenshot, state, log) for Claude to read", _bug)
	var atmo: Node = main.get("atmosphere") if main != null else null
	if atmo != null:
		sky = SkyCommands.new(atmo)
		sky.register(reg)


func _item_ids() -> Array:
	if player_inv == null:
		return []
	var ids := player_inv.items.all_ids()
	ids.sort()
	return ids


func _gamemode(args: PackedStringArray) -> Dictionary:
	if player == null:
		return CommandRegistry.fail("No player")
	if args.is_empty():
		return CommandRegistry.ok("Game mode: %s" % player.game_mode)
	var mode := GameMode.parse(args[0])
	if mode == "":
		return CommandRegistry.fail("Unknown game mode: %s (survival, creative)" % args[0])
	player.set_game_mode(mode)
	return CommandRegistry.ok("Set own game mode to %s Mode" % mode.capitalize())


func _give(args: PackedStringArray) -> Dictionary:
	if player_inv == null:
		return CommandRegistry.fail("No inventory")
	if args.is_empty():
		return CommandRegistry.fail("Usage: /give <item> [count]")
	var id := args[0].to_lower()
	if not player_inv.items.has(id):
		return CommandRegistry.fail("Unknown item: %s" % args[0])
	var count := 1
	if args.size() > 1:
		if not args[1].is_valid_int() or int(args[1]) < 1:
			return CommandRegistry.fail("Count must be a whole number of 1 or more: %s" % args[1])
		count = mini(int(args[1]), PlayerInventory.SIZE * player_inv.items.max_stack(id))
	var left := player_inv.pickup(id, count)
	if left > 0 and main != null and player != null:
		# inventory full: the rest lands at the feet, as vanilla /give does
		main.spawn_item({"id": id, "count": left}, player.position + Vector3(0, 1, 0))
	return CommandRegistry.ok("Gave %d [%s]" % [count, player_inv.items.display_name(id)])


func _clear(args: PackedStringArray) -> Dictionary:
	if player_inv == null:
		return CommandRegistry.fail("No inventory")
	var only := args[0].to_lower() if args.size() > 0 else ""
	if only != "" and not player_inv.items.has(only):
		return CommandRegistry.fail("Unknown item: %s" % args[0])
	var removed := 0
	for i in PlayerInventory.SIZE:
		var s: Dictionary = player_inv.inv.get_slot(i)
		if s.is_empty() or (only != "" and s["id"] != only):
			continue
		removed += int(s["count"])
		player_inv.inv.set_slot(i, {})
	if removed == 0:
		return CommandRegistry.fail("No items were found")
	return CommandRegistry.ok("Removed %d item(s)" % removed)


func _tp(args: PackedStringArray) -> Dictionary:
	if player == null:
		return CommandRegistry.fail("No player")
	if args.size() != 3:
		return CommandRegistry.fail("Usage: /tp <x> <y> <z>")
	var p: Variant = _coords(args, 0, player.position, true)
	if p is Dictionary:
		return p
	var out: Vector3 = p
	out.y = clampf(out.y, 0.0, float(Settings.WORLD_HEIGHT_VOXELS))
	player.teleport(out)
	return CommandRegistry.ok("Teleported to %.1f, %.1f, %.1f" % [out.x, out.y, out.z])


## Three coordinates from args[start..start+2] as a Vector3, or a fail() result.
## `~` / `~n` are relative to `cur`. centre: whole x/z name the voxel's centre (/tp);
## without it the result is floored to the voxel that holds it (/setblock, /fill).
func _coords(args: PackedStringArray, start: int, cur: Vector3, centre := false) -> Variant:
	var out := Vector3.ZERO
	for i in 3:
		var a := args[start + i]
		var rel := a.begins_with("~")
		var num := a.substr(1) if rel else a
		if num != "" and not num.is_valid_float():
			return CommandRegistry.fail("Not a coordinate: %s" % a)
		var v := float(num) if num != "" else 0.0
		if rel:
			out[i] = cur[i] + v
		else:
			out[i] = v + (0.5 if centre and i != 1 and num.is_valid_int() else 0.0)
	return out if centre else out.floor()


## Material id for a GameMaterials name (ids are 1 + definition index, 0 = air), or -1.
static func material_id(name: String) -> int:
	if name.to_lower() == "air":
		return GameMaterials.AIR
	var defs := GameMaterials.defs()
	for i in defs.size():
		if defs[i]["name"] == name.to_lower():
			return i + 1
	return -1


## Material id for /setblock and /fill, or a fail() result. Block-entity voxels
## (chest, furnace ...) are refused: the voxel alone would have no entity behind it.
func _placeable(name: String) -> Variant:
	var m := material_id(name)
	if m < 0:
		return CommandRegistry.fail("Unknown material: %s" % name)
	if m != GameMaterials.AIR and GameMaterials.defs()[m - 1].has("entity"):
		return CommandRegistry.fail("%s is a block entity: place its item or use /stations" % name)
	return m


## World edits typed before the bubble has loaded (start commands, the F4 menu's
## "extra commands") wait for it, then report in chat; ~ resolves at that point.
func _queue_until_loaded(cb: Callable, args: PackedStringArray) -> Dictionary:
	_run_when_loaded(cb, args)
	return CommandRegistry.ok("Waiting for the world to load ...")


func _run_when_loaded(cb: Callable, args: PackedStringArray) -> void:
	var tree: SceneTree = main.get_tree()
	while not main.bubble.is_loaded():
		await tree.process_frame
	var r: Dictionary = cb.call(args)
	var chat: Node = main.get("chat")
	if chat != null:
		chat.add_message(r["text"], ChatBox.FEEDBACK if r["ok"] else ChatBox.ERROR)


func _setblock(args: PackedStringArray) -> Dictionary:
	if main == null or player == null:
		return CommandRegistry.fail("No world")
	if args.size() != 4:
		return CommandRegistry.fail("Usage: /setblock <x> <y> <z> <material>")
	if not main.bubble.is_loaded():
		return _queue_until_loaded(_setblock, args)
	var p: Variant = _coords(args, 0, player.position)
	if p is Dictionary:
		return p
	var m: Variant = _placeable(args[3])
	if m is Dictionary:
		return m
	var wp := Vector3i(p)
	if not main.bubble.set_voxel(wp, m):
		if not main.world.is_editable(wp):
			return CommandRegistry.fail("%d, %d, %d is not loaded" % [wp.x, wp.y, wp.z])
		main.world.set_voxel(wp, m)   # outside the bubble: straight to the frozen terrain
	return CommandRegistry.ok("Set %d, %d, %d to %s" % [wp.x, wp.y, wp.z, args[3].to_lower()])


func _fill(args: PackedStringArray) -> Dictionary:
	if main == null or player == null:
		return CommandRegistry.fail("No world")
	if args.size() != 7:
		return CommandRegistry.fail("Usage: /fill <x1> <y1> <z1> <x2> <y2> <z2> <material>")
	if not main.bubble.is_loaded():
		return _queue_until_loaded(_fill, args)
	var a: Variant = _coords(args, 0, player.position)
	if a is Dictionary:
		return a
	var b: Variant = _coords(args, 3, player.position)
	if b is Dictionary:
		return b
	var m: Variant = _placeable(args[6])
	if m is Dictionary:
		return m
	var lo := Vector3i((a as Vector3).min(b))
	var hi := Vector3i((a as Vector3).max(b)) + Vector3i.ONE   # corners included, as in Minecraft
	var d := hi - lo
	if d.x * d.y * d.z > FILL_MAX:
		return CommandRegistry.fail("Too many voxels: %d (limit %d)" % [d.x * d.y * d.z, FILL_MAX])
	var n: int = main.bubble.fill_box(lo, hi, m)
	if n < 0:
		return CommandRegistry.fail("The box must lie inside the loaded sim bubble around you")
	return CommandRegistry.ok("Filled %d voxels with %s" % [n, args[6].to_lower()])


func _time(args: PackedStringArray) -> Dictionary:
	var atmo: Node = main.get("atmosphere") if main != null else null
	if atmo == null:
		return CommandRegistry.fail("No sky to change")
	if args.is_empty() or args[0] == "query":
		var now: float = atmo.time_of_day
		var hours := fposmod(now * 24.0, 24.0)
		return CommandRegistry.ok("The time is %02d:%02d (%.3f), %s" % [int(hours), int(fmod(hours, 1.0) * 60.0), now, atmo.calendar.date_text()])
	if args[0] != "set" or args.size() < 2:
		return CommandRegistry.fail("Usage: /time set <day|noon|sunset|night|midnight|sunrise|0..1>")
	var key := args[1].to_lower()
	var t := 0.0
	if TIMES.has(key):
		t = TIMES[key]
		# sunrise and sunset move with the seasons (ADR 0025); day / night are just after them
		var cal: Calendar = atmo.calendar
		match key:
			"sunrise": t = cal.sunrise()
			"day": t = cal.sunrise() + 0.04
			"sunset": t = cal.sunset()
			"night": t = cal.sunset() + 0.04
	elif key.is_valid_float():
		t = fposmod(float(key), 1.0)
	else:
		return CommandRegistry.fail("Unknown time: %s" % args[1])
	atmo.set_time_of_day(t)
	return CommandRegistry.ok("Set the time to %s" % key)


func _summon(args: PackedStringArray) -> Dictionary:
	if main == null or player == null or not ("mobs" in main) or main.mobs == null:
		return CommandRegistry.fail("No animals here")
	var sp := args[0].to_lower() if not args.is_empty() else ""
	if not main.mobs.defs.has(sp):
		return CommandRegistry.fail("Usage: /summon <%s> [count] [baby]" % "|".join(main.mobs.defs.keys()))
	var n := clampi(int(args[1]) if args.size() > 1 and args[1].is_valid_int() else 1, 1, 32)
	var baby := args.size() > 1 and args[args.size() - 1].to_lower() == "baby"
	var hungry := args.size() > 1 and args[args.size() - 1].to_lower() == "hungry"
	var fwd := -player.camera.global_transform.basis.z
	fwd.y = 0
	var at := player.position + fwd.normalized() * Settings.m2v(3.0)
	for i in n:
		var q := at + Vector3(randf_range(-2, 2), 0, randf_range(-2, 2))
		var m: Mob = main.mobs.spawn(sp, Vector3(q.x, q.y + 0.5, q.z), baby, randf() * TAU)
		if hungry and m != null:
			m.make_hungry()
	return CommandRegistry.ok("Summoned %d %s%s" % [n, "baby " if baby else "", sp])


func _book() -> QuestBook:
	return main.get("quests") if main != null else null


func _quest_ids() -> Array:
	var b := _book()
	return (b.quests.keys() + ["all"]) if b else []


func _quests(args: PackedStringArray) -> Dictionary:
	var b := _book()
	if b == null:
		return CommandRegistry.fail("No progression book")
	var sub := args[0] if not args.is_empty() else "list"
	match sub:
		"list":
			var lines := []
			for c in b.chapters:
				var p := b.chapter_progress(c["id"])
				lines.append("%s %d/%d" % [c["title"], p.x, p.y])
				for q in c["quests"]:
					if not b.is_done(q["id"]) and b.is_unlocked(q["id"]):
						lines.append("  open: %s (%s)" % [q["title"], q["id"]])
			return CommandRegistry.ok("
".join(lines))
		"complete":
			if args.size() < 2:
				return CommandRegistry.fail("Usage: /quests complete <id|all>")
			if args[1] == "all":
				for id in b.quests:
					b.complete(id)
				return CommandRegistry.ok("Completed every quest")
			if not b.quests.has(args[1]):
				return CommandRegistry.fail("No quest %s" % args[1])
			b.complete(args[1])
			return CommandRegistry.ok("Completed %s" % b.quests[args[1]]["title"])
		"reset":
			b.reset()
			return CommandRegistry.ok("Progression book reset")
	return CommandRegistry.fail("Usage: /quests [list|complete <id|all>|reset]")


func _kill(_args: PackedStringArray) -> Dictionary:
	if player == null or player_inv == null:
		return CommandRegistry.fail("No player")
	player_inv.damage(player_inv.health)   # bypasses creative: /kill works in every mode
	player.died.emit()
	return CommandRegistry.ok("Killed the ape")


func _heal(_args: PackedStringArray) -> Dictionary:
	if player_inv == null:
		return CommandRegistry.fail("No player")
	player_inv.heal(player_inv.max_health)
	return CommandRegistry.ok("Healed")


func _stain(args: PackedStringArray) -> Dictionary:
	if player == null:
		return CommandRegistry.fail("No player")
	var k := args[0].to_lower() if not args.is_empty() else ""
	if k == "clear":
		player.stains.clear()
		return CommandRegistry.ok("Stains cleared")
	if not PlayerStains.KINDS.has(k):
		return CommandRegistry.fail("Usage: /stain <%s|clear>" % "|".join(PlayerStains.KINDS))
	player.stains.apply(k)
	return CommandRegistry.ok("Stained: %s" % ", ".join(player.stains.active()))


func _kit(args: PackedStringArray) -> Dictionary:
	if args.is_empty() or not KITS.has(args[0].to_lower()):
		return CommandRegistry.fail("Usage: /kit <%s>" % "|".join(KITS.keys()))
	var kit: Dictionary = KITS[args[0].to_lower()]
	for id in kit:
		var r := _give(PackedStringArray([id, str(kit[id])]))
		if not r["ok"]:
			return r
	return CommandRegistry.ok("Gave the %s kit" % args[0].to_lower())


func _spawn(_args: PackedStringArray) -> Dictionary:
	if player == null or main == null:
		return CommandRegistry.fail("No world")
	player.teleport(main.spawn_position())
	return CommandRegistry.ok("Teleported to spawn")


## Nearest feature as a voxel position, or a fail() result.
func _find(args: PackedStringArray) -> Variant:
	if args.is_empty():
		return CommandRegistry.fail("Usage: /locate <feature>")
	if main == null or player == null:
		return CommandRegistry.fail("No world")
	var kind := args[0].to_lower()
	var p := player.position
	# continents are ~8 km across (ADR 0035): a biome can be a few km away
	var t: Vector3i = main.world.generator.gen.find_nearest(kind, int(p.x), int(p.z), int(Settings.m2v(4000.0)))
	if t.y < 0:
		return CommandRegistry.fail("No %s within 4 km" % kind)
	return t


func _locate(args: PackedStringArray) -> Dictionary:
	var t: Variant = _find(args)
	if t is Dictionary:
		return t
	var d := Settings.v2m(Vector2(t.x - player.position.x, t.z - player.position.z).length())
	return CommandRegistry.ok("The nearest %s is at %d, %d, %d (%d m away)" % [args[0], t.x, t.y, t.z, int(d)])


func _goto(args: PackedStringArray) -> Dictionary:
	var t: Variant = _find(args)
	if t is Dictionary:
		return t
	var kind := args[0].to_lower()
	# caves: stand in the room itself; surface features: drop in from a little above
	var up := 0.1 if kind.ends_with("cave") else 3.0
	player.teleport(Vector3(t.x + 0.5, t.y + up, t.z + 0.5))
	return CommandRegistry.ok("Teleported to the nearest %s at %d, %d, %d" % [kind, t.x, t.y, t.z])


func _daycycle(args: PackedStringArray) -> Dictionary:
	var atmo: Node = main.get("atmosphere") if main != null else null
	if atmo == null:
		return CommandRegistry.fail("No sky")
	if args.is_empty():
		return CommandRegistry.ok("Day length: %s" % ("frozen" if atmo.day_length_s <= 0.0 else "%d s" % int(atmo.day_length_s)))
	match args[0].to_lower():
		"on": atmo.day_length_s = 1200.0
		"off": atmo.day_length_s = 0.0
		_:
			if not args[0].is_valid_float():
				return CommandRegistry.fail("Usage: /daycycle <on|off|seconds>")
			atmo.day_length_s = maxf(float(args[0]), 0.0)
	return CommandRegistry.ok("Day cycle %s" % ("frozen" if atmo.day_length_s <= 0.0 else "%d s per day" % int(atmo.day_length_s)))


func _material_names() -> Array:
	var out := ["air"]
	for d in GameMaterials.defs():
		out.append(d["name"])
	return out


func _blast(args: PackedStringArray) -> Dictionary:
	if main == null or player == null:
		return CommandRegistry.fail("No world")
	if args.is_empty() or not args[0].is_valid_float() or float(args[0]) <= 0.0 or float(args[0]) > 16.0:
		return CommandRegistry.fail("Usage: /blast <radius_m 0..16> [material]")
	var mat := material_id(args[1]) if args.size() > 1 else GameMaterials.AIR
	if mat < 0:
		return CommandRegistry.fail("Unknown material: %s" % args[1])
	var cam: Camera3D = player.camera
	var r := Settings.m2v(float(args[0]))
	var hit: VoxelRaycastResult = main.world.raycast(cam.global_position, -cam.global_transform.basis.z, 64.0)
	var at: Vector3 = Vector3(hit.position) + Vector3(0.5, 0.5, 0.5) if hit else cam.global_position - cam.global_transform.basis.z * 16.0
	if mat != GameMaterials.AIR:
		at.y += r + 2.0   # a ball above the spot, so it falls and spreads
	var n: int = main.bubble.blast(at, r, mat)
	return CommandRegistry.ok("Blast of %s m: %d voxels" % [args[0], n])


func _stations(_args: PackedStringArray) -> Dictionary:
	if main == null:
		return CommandRegistry.fail("No world")
	main.place_stations()
	return CommandRegistry.ok("Placing a crafting table, chest and furnace ahead")


func _fell(args: PackedStringArray) -> Dictionary:
	if main == null or player == null or main.pieces == null:
		return CommandRegistry.fail("Rigid pieces are off (start with rigid_pieces=1)")
	var deg := float(args[0]) if args.size() > 0 and args[0].is_valid_float() else 0.0
	var t: Dictionary
	if args.size() > 0 and args[0] == "pillar":
		t = main.pieces.fell_pillar(player.camera.global_position, -player.camera.global_transform.basis.z, main.world)
	else:
		t = main.pieces.fell_nearest(player.position, 20.0, deg, main.world)
	if t.is_empty():
		return CommandRegistry.fail("No standing trunk 8-20 m away")
	# face it, looking at 5 m up the trunk: it falls across the view
	var eye: Vector3 = player.camera.global_position
	var look: Vector3 = t["centre"] + Vector3(0, Settings.m2v(5.0), 0) - eye
	player._yaw = atan2(-look.x, -look.z)
	player._pitch = atan2(look.y, Vector2(look.x, look.z).length())
	player._apply_look()
	return CommandRegistry.ok("Felled a trunk of %d columns (%d voxels cut, %d pieces)" % [t["columns"], t["cut"], t["pieces"]])


func _bug(args: PackedStringArray) -> Dictionary:
	if main == null:
		return CommandRegistry.fail("No game to report on")
	var path := BugReport.write(main, " ".join(args), null)
	if path == "":
		return CommandRegistry.fail("Could not save the bug report")
	return CommandRegistry.ok("Bug report saved: %s" % path)


func _seed(_args: PackedStringArray) -> Dictionary:
	var text := " (\"%s\")" % Settings.world_seed_text if Settings.world_seed_text != "" else ""
	return CommandRegistry.ok("Seed: [%d]%s" % [Settings.world_seed, text])
