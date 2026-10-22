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
	"tools": {"iron_pickaxe": 1, "stone_pickaxe": 1, "bucket": 2, "water_bucket": 1, "lava_bucket": 1, "coal": 32, "oak_log": 64},
	"builder": {"stone": 64, "cobblestone": 64, "dirt": 64, "grass_block": 64, "sand": 64, "oak_planks": 64, "oak_log": 64, "glass": 64, "sandstone": 64},
	"stations": {"crafting_table": 4, "furnace": 4, "chest": 4, "iron_ore": 16, "copper_ore": 16, "tin_ore": 16, "coal": 32},
	"magic": {"wand_basic": 1, "spell_spark_bolt": 1, "spell_fire_bolt": 1, "spell_speed_up": 1, "mana_dust": 64},
}

## What WorldGen.find_nearest can look for (biome names work too).
const FEATURES := ["cave", "deep_cave", "ruin", "waterfall", "lake", "river", "spring", "marsh", "alpine", "snow", "mountain", "forest", "plains", "desert"]

var reg: CommandRegistry
var player: Player
var player_inv: PlayerInventory
var main: Node   # Main: atmosphere, spawn_item, spawn position


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
	reg.register("seed", "/seed", "Show the world seed", _seed)
	reg.register("kit", "/kit <%s>" % "|".join(KITS.keys()), "Give a playtest kit", _kit, [KITS.keys()])
	reg.register("spawn", "/spawn", "Teleport to the world spawn", _spawn)
	reg.register("locate", "/locate <feature>", "Find the nearest cave, ruin, lake, biome ... (within 2 km)", _locate, [FEATURES])
	reg.register("goto", "/goto <feature>", "Teleport to the nearest cave, ruin, lake, biome ...", _goto, [FEATURES])
	reg.register("daycycle", "/daycycle <on|off|seconds>", "Run the sun (on = 20 min days) or freeze it", _daycycle, [["on", "off"]])
	reg.register("blast", "/blast <radius_m> [material]", "Carve a crater where you look, or drop a ball of a material (water, sand, lava ...)",
		_blast, [["1", "2", "4", "8"], _material_names])
	reg.register("stations", "/stations", "Place a crafting table, chest and furnace ahead", _stations)
	reg.register("bug", "/bug <what went wrong>", "Save a bug report (screenshot, state, log) for Claude to read", _bug)


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
	var cur := player.position
	var out := Vector3.ZERO
	for i in 3:
		var a := args[i]
		var rel := a.begins_with("~")
		var num := a.substr(1) if rel else a
		if num != "" and not num.is_valid_float():
			return CommandRegistry.fail("Not a coordinate: %s" % a)
		var v := float(num) if num != "" else 0.0
		if rel:
			out[i] = cur[i] + v
		else:
			out[i] = v + (0.5 if i != 1 and num.is_valid_int() else 0.0)   # whole x/z = the voxel's centre
	out.y = clampf(out.y, 0.0, float(Settings.WORLD_HEIGHT_VOXELS))
	player.teleport(out)
	return CommandRegistry.ok("Teleported to %.1f, %.1f, %.1f" % [out.x, out.y, out.z])


func _time(args: PackedStringArray) -> Dictionary:
	var atmo: Node = main.get("atmosphere") if main != null else null
	if atmo == null:
		return CommandRegistry.fail("No sky to change")
	if args.is_empty() or args[0] == "query":
		var now: float = atmo.time_of_day
		var hours := fposmod(now * 24.0, 24.0)
		return CommandRegistry.ok("The time is %02d:%02d (%.3f)" % [int(hours), int(fmod(hours, 1.0) * 60.0), now])
	if args[0] != "set" or args.size() < 2:
		return CommandRegistry.fail("Usage: /time set <day|noon|sunset|night|midnight|sunrise|0..1>")
	var key := args[1].to_lower()
	var t := 0.0
	if TIMES.has(key):
		t = TIMES[key]
	elif key.is_valid_float():
		t = fposmod(float(key), 1.0)
	else:
		return CommandRegistry.fail("Unknown time: %s" % args[1])
	atmo.set_time_of_day(t)
	return CommandRegistry.ok("Set the time to %s" % key)


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
	var t: Vector3i = main.world.generator.gen.find_nearest(kind, int(p.x), int(p.z), 4096)
	if t.y < 0:
		return CommandRegistry.fail("No %s within 2 km" % kind)
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
	var out := []
	for d in GameMaterials.defs():
		out.append(d["name"])
	return out


func _blast(args: PackedStringArray) -> Dictionary:
	if main == null or player == null:
		return CommandRegistry.fail("No world")
	if args.is_empty() or not args[0].is_valid_float() or float(args[0]) <= 0.0 or float(args[0]) > 16.0:
		return CommandRegistry.fail("Usage: /blast <radius_m 0..16> [material]")
	var mat := GameMaterials.AIR
	if args.size() > 1:
		mat = -1
		var defs := GameMaterials.defs()
		for i in defs.size():
			if defs[i]["name"] == args[1].to_lower():
				mat = i + 1   # material ids are 1 + definition index
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
