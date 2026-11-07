class_name LoopCommands
extends RefCounted

## Part G's slash commands (ADR 0061), registered by SurvivalLoop:
##   /diet [set <group> <points> | reset]     the food group bars and what they do to max health
##   /eat <item>                              eat one from the hotbar as a held right click does
##   /frost [plant <crop> [dx]]               check the farm for frost now, or till and plant a 0.5 m patch at your feet
##   /season [spring|summer|autumn|winter] [now]   the season's state, or jump to the middle of one (now: snow / ice / thaw the bubble at once)
##   /prospect [flint|copper|menu [mode]]     tap the rock at the crosshair (or under you) with a pick; menu opens its panel
##   /discover [list | reset | <kind:name>]   the discovery journal
##   /pin <recipe id> | clear                 pin a recipe to the HUD
##   /track <quest id> | clear                track a quest on the HUD

var loop: SurvivalLoop


func _init(p_loop: SurvivalLoop) -> void:
	loop = p_loop


func register(reg: CommandRegistry) -> void:
	reg.register("diet", "/diet [set <group> <points> | reset]", "Food groups (ADR 0061): the bars, the balanced-diet bonus, set one", _diet, [["set", "reset"], loop.main.survival.nutrition.group_ids()])
	reg.register("eat", "/eat <item>", "Eat one of an item from the hotbar, as a held right click does", _eat)
	reg.register("frost", "/frost [plant <crop> [dx]]", "Check the farm for frost now (crops below their frost_c die), or till and plant a patch dx blocks east of you", _frost, [["plant"], loop.main.farming.crops_def.keys()])
	reg.register("season", "/season [spring|summer|autumn|winter] [now]", "Seasons (ADR 0156): mean temperature, snow / ice mode, drought, the frost forecast; or jump to a season (now: lay or clear its snow and ice over the sim area at once)", _season, [["spring", "summer", "autumn", "winter"], ["now"]])
	reg.register("sapling", "/sapling <kind> [dx] [dz] | grow", "Plant a sapling dx blocks east (dz south) of you (ADR 0112), or grow every planted one now", _sapling, [["grow"] + loop.main.saplings.trees.keys()])
	reg.register("furniture", "/furniture <kind> [dx] [dz]", "Set a piece of furniture (or a barrel) on the ground dx blocks east, dz south of you, facing you", _furniture, [FurnitureEntity.KINDS.keys() + ["barrel", "storage_shelf"]])
	reg.register("sign", "/sign <text>", "Write on the sign you last right clicked; an empty text wipes it", _sign)
	reg.register("prospect", "/prospect [flint|copper|menu [nearest|all|map|density]]", "Tap the rock you look at (or stand on) with the held, or the named, prospecting pick; menu opens the pick's settings", _prospect, [["flint", "copper", "menu"]])
	reg.register("discover", "/discover [list | reset | <kind:name>]", "The discovery journal: progress, every entry, forget all, or mark one found", _discover, [["list", "reset"]])
	reg.register("pin", "/pin <recipe id> | clear", "Pin a recipe's ingredients to the HUD (also: Pin in the recipe view)", _pin, [["clear"]])
	reg.register("track", "/track <quest id> | clear", "Track a quest on the HUD (also: Track in the book)", _track, [["clear"]])


func _diet(args: PackedStringArray) -> Dictionary:
	var n: Nutrition = loop.main.survival.nutrition
	if args.size() == 3 and args[0] == "set" and n.bars.has(args[1]) and args[2].is_valid_float():
		n.bars[args[1]] = clampf(float(args[2]), 0.0, n.max_points)
	elif args.size() == 1 and args[0] == "reset":
		n.reset()
	elif not args.is_empty():
		return CommandRegistry.fail("Usage: /diet [set <group> <points> | reset]")
	loop.main.survival.apply_max_health()
	var parts := []
	for g in n.group_ids():
		parts.append("%s %.1f" % [g, float(n.bars[g])])
	return CommandRegistry.ok("Diet: %s; max health %d (+%d, -%d)" % [", ".join(parts), loop.main.player_inv.max_health, n.bonus(), n.penalty()])


func _eat(args: PackedStringArray) -> Dictionary:
	if args.is_empty():
		return CommandRegistry.fail("Usage: /eat <item>")
	var inv: PlayerInventory = loop.main.player_inv
	var id := args[0].to_lower()
	var def := inv.items.get_def(id)
	if not def.has("food"):
		return CommandRegistry.fail("Not food: %s" % args[0])
	var slot := -1
	for i in 9:
		if String(inv.inv.get_slot(i).get("id", "")) == id:
			slot = i
			break
	if slot < 0:
		return CommandRegistry.fail("No %s in the hotbar" % inv.items.display_name(id))
	inv.select(slot)
	var sv: Survival = loop.main.survival
	sv.eat(def, float(inv.held().get("spoil", 0.0)))
	if def.has("empties_to"):
		inv.replace_held_one(def["empties_to"])
	else:
		inv.consume_held(1)
	loop.main.player.ate.emit(id)
	var given := []
	for g in sv.last_meal:
		given.append("%s +%.1f" % [g, float(sv.last_meal[g])])
	return CommandRegistry.ok("Ate %s: hunger %.1f, %s" % [inv.items.display_name(id), sv.hunger, ", ".join(given) if not given.is_empty() else "no food group"])


func _season(args: PackedStringArray) -> Dictionary:
	var sw: SeasonWorld = loop.season_world
	var cal: Calendar = loop.main.atmosphere.calendar
	if not args.is_empty():
		var i := ["winter", "spring", "summer", "autumn"].find(args[0].to_lower())
		if i < 0:
			return CommandRegistry.fail("Usage: /season [spring|summer|autumn|winter] [now]")
		cal.set_date([0, 3, 6, 9][i], 0)   # January, April, July, October: the heart of each season
		loop.main.atmosphere.set_time_of_day(cal.time)
		loop.main.survival._measure()
		loop.main.bubble.ambient_c = loop.main.survival.outside_c
		Seasons.season = cal.season()
		if args.size() > 1 and args[1] == "now":
			var flags := 4 if i != 0 else (3 | (255 << 8))   # winter: freeze and a full snow cover; else the thaw
			var n := sw.sweep(flags, -1)
			return CommandRegistry.ok("It is now %s; %d cells of snow and ice %s" % [cal.date_text(), n, "laid" if i == 0 else "cleared"])
	var d := sw.frost_in_days()
	return CommandRegistry.ok("%s: day's mean %.1f C, outside %.1f C; %s%s%s" % [cal.season_name(), sw.mean_c(), loop.main.survival.outside_c,
		String(sw.stats["mode"]) if String(sw.stats["mode"]) != "" else "no snow or ice work",
		"; drought" if loop.main.farming.drought else "", "; frost in %d days" % d if d > 0 else ""])


func _frost(args: PackedStringArray) -> Dictionary:
	if args.size() >= 2 and args[0] == "plant":
		return _plant(args[1], int(args[2]) if args.size() > 2 and args[2].is_valid_int() else 0)
	var killed := loop.frost_now()
	var s: Dictionary = loop.seasons.stats
	return CommandRegistry.ok("Frost check: %d killed now (%d in all), %d hardy, %d sheltered; %s" % [killed.size(), s["killed"], s["hardy"], s["sheltered"],
		loop.main.atmosphere.calendar.date_text()])


## Tills a BLOCK x BLOCK patch of the ground under the player (dx blocks east) and plants `kind` on it.
func _plant(kind: String, dx: int) -> Dictionary:
	var main := loop.main
	var farm: Farming = main.farming
	if not farm.crops_def.has(kind):
		return CommandRegistry.fail("No such crop: %s" % kind)
	var b := Settings.BLOCK
	var p := Vector3i(main.player.position.floor()) + Vector3i(dx * b, 0, 0)
	var top := -1
	for dy in range(0, 16):   # the first body-blocking voxel under the feet (grass tufts and snow cover are not ground)
		var q := p - Vector3i(0, dy, 0)
		if GameMaterials.blocks_bodies(main.world.get_voxel(q)):
			top = q.y
			break
	if top < 0:
		return CommandRegistry.fail("No ground here")
	var n := 0
	for i in b:
		for j in b:
			var soil := Vector3i(p.x + i, top, p.z + j)
			for up in range(1, 3):   # clear plants and snow above, as a hoe user would
				if main.world.get_voxel(soil + Vector3i(0, up, 0)) != GameMaterials.AIR and not GameMaterials.blocks_bodies(main.world.get_voxel(soil + Vector3i(0, up, 0))):
					farm.set_voxel.call(soil + Vector3i(0, up, 0), GameMaterials.AIR)
			var m: int = main.world.get_voxel(soil)
			if not GameMaterials.is_grass(m) and m != GameMaterials.DIRT:
				farm.set_voxel.call(soil, GameMaterials.DIRT)   # a debug patch: any ground will do
			farm.till(soil)
			if farm.plant(soil, kind):
				n += 1
	return CommandRegistry.ok("Planted %d %s voxels at %d %d %d" % [n, kind, p.x, top, p.z]) if n > 0 else CommandRegistry.fail("Could not till here (grass or dirt with air above)")


## The top of the ground (the first body-blocking voxel from 2 m up) dx, dz blocks from the player, -1 y when none.
func _ground(dx: int, dz: int) -> Vector3i:
	var main := loop.main
	var p := Vector3i(main.player.position.floor()) + Vector3i(dx, 0, dz) * Settings.BLOCK
	for up in range(16, -17, -1):
		var q := p + Vector3i(0, up, 0)
		if GameMaterials.blocks_bodies(main.world.get_voxel(q)) and not GameMaterials.blocks_bodies(main.world.get_voxel(q + Vector3i.UP)):
			return q
	return Vector3i(p.x, -1, p.z)


func _furniture(args: PackedStringArray) -> Dictionary:
	var main := loop.main
	if args.is_empty() or not (FurnitureEntity.KINDS.has(args[0]) or args[0] in ["barrel", "storage_shelf"]):
		return CommandRegistry.fail("Usage: /furniture <kind> [dx] [dz]")
	var g := _ground(int(args[1]) if args.size() > 1 else 2, int(args[2]) if args.size() > 2 else 0)
	if g.y < 0:
		return CommandRegistry.fail("No ground here")
	var b := Settings.BLOCK
	var anchor := Vector3i(floori(float(g.x) / b) * b, g.y + 1, floori(float(g.z) / b) * b)   # on the block grid
	if args[0] in ["barrel", "storage_shelf"]:
		main._place_station(anchor, GameMaterials.id_of(args[0]))
	else:
		var look: Vector3 = Vector3(anchor) - main.player.position
		main._place_furniture(args[0], anchor, FurnitureEntity.facing_for(args[0], -look))
	return CommandRegistry.ok("Placed %s at %d %d %d" % [args[0], anchor.x, anchor.y, anchor.z])


func _sign(args: PackedStringArray) -> Dictionary:
	var main := loop.main
	var e: RefCounted = main.blocks.furniture_at(main.sign_at)
	if e == null or not FurnitureEntity.KINDS[e.furniture].get("write", false):
		return CommandRegistry.fail("Right click a sign first")
	e.set_text(" ".join(args).replace("|", "\n"))   # | starts a new line; SignText caps it (ADR 0164)
	main.things.refresh(e)
	Net.ent_text(e.pos, e.text)
	return CommandRegistry.ok("Wrote on the sign" if e.text != "" else "Wiped the sign")


func _sapling(args: PackedStringArray) -> Dictionary:
	var main := loop.main
	var s: Saplings = main.saplings
	if args.size() == 1 and args[0] == "grow":
		for p: Vector3i in s.planted:
			s.planted[p]["left_s"] = 0.0
		var grown := s.tick(0.0)
		return CommandRegistry.ok("Grew %d saplings, %d still planted%s" % [grown.size(), s.planted.size(), "" if s.planted.is_empty() else " (last blocked: %s)" % s.last_block])
	if args.is_empty() or not s.trees.has(args[0]):
		return CommandRegistry.fail("Usage: /sapling <%s> [dx] [dz] | grow" % "|".join(s.trees.keys()))
	var dx := int(args[1]) if args.size() > 1 else 2
	var dz := int(args[2]) if args.size() > 2 else 0
	var p := Vector3i(main.player.position.floor()) + Vector3i(dx, 0, dz) * Settings.BLOCK
	for up in range(16, -17, -1):   # the highest soil with air above, within 2 m
		var q := p + Vector3i(0, up, 0)
		if GameMaterials.is_soil(main.world.get_voxel(q)) and main.world.get_voxel(q + Vector3i.UP) == GameMaterials.AIR:
			var m := GameMaterials.id_of(s.trees[args[0]]["sapling"])
			main._on_placed(q + Vector3i.UP, m)
			return CommandRegistry.ok("Planted a %s sapling at %d %d %d" % [args[0], q.x, q.y + 1, q.z])
	return CommandRegistry.fail("No soil here")


func _prospect(args: PackedStringArray) -> Dictionary:
	var main := loop.main
	var def: Dictionary = main.player_inv.held_def()
	if not args.is_empty() and args[0] == "menu":
		if args.size() > 1 and Prospecting.MODES.has(args[1]):
			loop.prospecting.mode = args[1]
		loop.prospect_menu.open(def if def.has("prospects") else main.items.get_def("copper_prospecting_pick"), true)
		return CommandRegistry.ok("Prospecting panel open")
	if not args.is_empty():
		def = main.items.get_def(args[0] + "_prospecting_pick")
	if not def.has("prospects"):
		return CommandRegistry.fail("Hold a prospecting pick, or name one: /prospect flint|copper")
	var target := Vector3i(-1, -1, -1)
	var hit = main.player._aim()
	if hit != null:
		target = hit.position
	if hit == null or String(main.mining.entry(GameMaterials.name_of(main.world.get_voxel(target))).get("tool", "")) != "pickaxe":
		var p := Vector3i(main.player.position.floor())   # no rock in view: the first rock under the feet
		for dy in range(1, 96):
			var q := p - Vector3i(0, dy, 0)
			if String(main.mining.entry(GameMaterials.name_of(main.world.get_voxel(q))).get("tool", "")) == "pickaxe":
				target = q
				break
	if target.y < 0 or not loop.prospecting.use_at(main, target, def, loop.discoveries):
		return CommandRegistry.fail("Tap rock: %s is not" % GameMaterials.name_of(main.world.get_voxel(target)))
	return CommandRegistry.ok("Prospected at %d %d %d" % [target.x, target.y, target.z])


func _discover(args: PackedStringArray) -> Dictionary:
	var d := loop.discoveries
	if args.is_empty():
		var s := d.summary()
		var parts := []
		for k in s["kinds"]:
			parts.append("%s %d/%d" % [k, s["kinds"][k][0], s["kinds"][k][1]])
		return CommandRegistry.ok("Discoveries %d/%d: %s" % [s["found"], s["total"], ", ".join(parts)])
	match args[0]:
		"list":
			var found := d.found.keys()
			found.sort()
			return CommandRegistry.ok("Found: " + (", ".join(found) if not found.is_empty() else "nothing yet"))
		"reset":
			d.found.clear()
			d.dirty = true
			return CommandRegistry.ok("Discoveries forgotten")
	if not d.has(args[0]):
		return CommandRegistry.fail("No such entry: %s (kind:name, e.g. biome:forest)" % args[0])
	return CommandRegistry.ok("Discovered %s" % args[0]) if d.note(args[0]) else CommandRegistry.ok("Already found: %s" % args[0])


func _pin(args: PackedStringArray) -> Dictionary:
	return _toggle("recipe", args)


func _track(args: PackedStringArray) -> Dictionary:
	return _toggle("quest", args)


func _toggle(type: String, args: PackedStringArray) -> Dictionary:
	var t := loop.trackers
	if args.is_empty():
		return CommandRegistry.ok("Tracked: %s" % (", ".join(t.pins.map(func(p: Dictionary) -> String: return "%s %s" % [p["type"], p["id"]])) if not t.pins.is_empty() else "nothing"))
	if args[0] == "clear":
		t.clear()
		return CommandRegistry.ok("Trackers cleared")
	if t.is_pinned(type, args[0]):
		t.toggle(type, args[0])
		return CommandRegistry.ok("No longer tracking %s" % args[0])
	if not t.pin(type, args[0]):
		return CommandRegistry.fail("No such %s: %s" % [type, args[0]])
	return CommandRegistry.ok("Tracking %s %s" % [type, args[0]])
