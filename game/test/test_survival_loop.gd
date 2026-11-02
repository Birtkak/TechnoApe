extends GdUnitTestSuite

## The survival loop (ADR 0061): food groups (fill, drain, the balanced bonus, the penalty), seasonal
## crop and drop rules, root cellar spoilage, the discovery journal's entries and triggers, HUD
## trackers and the prospecting pick's readings.

var items: ItemRegistry
var recipes: RecipeRegistry


func before() -> void:
	items = ItemRegistry.load_default()
	recipes = RecipeRegistry.load_default(items)


func after_test() -> void:
	Seasons.season = -1


# ---------------- nutrition ----------------

func test_nutrition_data_loads_and_every_food_feeds_a_group() -> void:
	var n := Nutrition.load_default()
	assert_array(n.errors).is_empty()
	assert_array(n.group_ids()).contains_exactly(["meat", "grain", "fruit", "vegetable", "dairy"])
	for id in items.items_with_tag("food"):
		var g := n.food_groups(items.get_def(id))
		assert_bool(g.is_empty()).override_failure_message("%s feeds no food group: give it \"nutrition\" or a tag rule" % id).is_false()
	for id in items.all_ids():   # explicit groups name real groups
		var nd: Variant = items.get_def(id).get("nutrition", null)
		if nd is Dictionary:
			for g in nd:
				assert_bool(String(g) in n.group_ids()).override_failure_message("%s: unknown group %s" % [id, g]).is_true()


func test_eating_fills_its_groups_stale_half_and_capped() -> void:
	var n := Nutrition.load_default()
	n.load_dict({"meat": 0, "grain": 0, "fruit": 0, "vegetable": 0, "dairy": 0})
	var g := n.eat(items.get_def("cooked_meat"))
	assert_float(float(g["meat"])).is_equal(5.0)
	assert_float(float(n.bars["meat"])).is_equal(5.0)
	n.eat(items.get_def("stew"))   # two groups at once
	assert_float(float(n.bars["meat"])).is_equal(8.0)
	assert_float(float(n.bars["vegetable"])).is_equal(3.0)
	n.eat(items.get_def("blueberry"), 0.6)   # stale: half
	assert_float(float(n.bars["fruit"])).is_equal(1.0)
	for i in 5:
		n.eat(items.get_def("cooked_meat"))
	assert_float(float(n.bars["meat"])).is_equal(n.max_points)
	# a food without "nutrition" falls back to its tags: a berry is fruit worth its hunger
	var wild := {"id": "wild_strawberry", "tags": ["food", "berry"], "food": {"hunger": 2}}
	assert_dict(n.food_groups(wild)).is_equal({"fruit": 2.0})
	assert_dict(n.food_groups({"id": "x", "tags": ["food"], "food": {"hunger": 2}})).is_empty()
	# other parts' foods by id in nutrition.json (part F's cured meat): no line of theirs to touch
	assert_dict(n.food_groups({"id": "smoked_meat", "tags": ["food"], "food": {"hunger": 6}})).is_equal({"meat": 4.0})
	assert_dict(n.food_groups({"id": "smoked_meat", "nutrition": {"meat": 5}})).is_equal({"meat": 5.0})   # its own wins


func test_bars_drain_with_time_and_effort() -> void:
	var n := Nutrition.load_default()
	n.load_dict({"meat": 5, "grain": 5, "fruit": 5, "vegetable": 5, "dairy": 5})
	n.drain(n.drain_s)   # one point per drain_s at rest
	assert_float(float(n.bars["meat"])).is_equal_approx(4.0, 1e-4)
	n.drain(n.drain_s, Settings.hunger_fast_mult)   # sprinting drains like hunger does
	assert_float(float(n.bars["grain"])).is_equal_approx(4.0 - Settings.hunger_fast_mult, 1e-4)
	n.drain(100000.0)
	assert_float(float(n.bars["dairy"])).is_equal(0.0)


func test_a_balanced_diet_raises_max_health_one_food_does_not() -> void:
	var n := Nutrition.load_default()
	var base := 20
	n.load_dict({"meat": 4, "grain": 4, "fruit": 4, "vegetable": 4, "dairy": 4})   # a new player: nothing either way
	assert_int(n.max_health(base)).is_equal(base)
	n.load_dict({"meat": 10, "grain": 4, "fruit": 4, "vegetable": 4, "dairy": 4})   # meat only
	assert_int(n.bonus()).is_equal(0)
	assert_int(n.max_health(base)).is_equal(base)
	n.load_dict({"meat": 10, "grain": 6, "fruit": 4, "vegetable": 4, "dairy": 4})   # two groups
	assert_int(n.max_health(base)).is_equal(base + 2 * n.bonus_hp)
	n.load_dict({"meat": 9, "grain": 9, "fruit": 9, "vegetable": 9, "dairy": 9})   # all five: +5 hearts
	assert_int(n.max_health(base)).is_equal(base + 10)


func test_empty_groups_cost_a_heart_each_but_never_below_the_floor() -> void:
	var n := Nutrition.load_default()
	n.load_dict({"meat": 6, "grain": 6, "fruit": 4, "vegetable": 0, "dairy": 0})
	assert_int(n.bonus()).is_equal(4)
	assert_int(n.penalty()).is_equal(4)
	assert_int(n.max_health(20)).is_equal(20)
	n.load_dict({"meat": 0, "grain": 0, "fruit": 0, "vegetable": 0, "dairy": 0})
	assert_int(n.max_health(20)).is_equal(n.min_health)   # 20 - 10 would be 10: the floor holds at 12
	var copy := Nutrition.load_default()
	copy.load_dict(n.to_dict())
	assert_dict(copy.bars).is_equal(n.bars)


# ---------------- seasons ----------------

func _farm() -> Array:
	var vox := {}
	var f := Farming.load_default()
	f.block = 1   # the Minecraft rules on one-voxel blocks (crop stamps need a 4 m cell and farmland under the anchor)
	f.get_voxel = func(p: Vector3i) -> int: return int(vox.get(p, GameMaterials.AIR))
	f.set_voxel = func(p: Vector3i, m: int) -> void:
		if m == GameMaterials.AIR:
			vox.erase(p)
		else:
			vox[p] = m
	for x in range(0, 40):
		vox[Vector3i(x, 0, 0)] = GameMaterials.FARMLAND_MOIST
	return [f, vox]


func test_frost_kills_tender_crops_spares_hardy_and_sheltered_ones() -> void:
	var fv := _farm()
	var f: Farming = fv[0]
	var vox: Dictionary = fv[1]
	assert_bool(f.plant(Vector3i(0, 0, 0), "wheat")).is_true()
	assert_bool(f.plant(Vector3i(1, 0, 0), "carrots")).is_true()
	assert_bool(f.plant(Vector3i(36, 0, 0), "wheat")).is_true()   # another 4 m cell: a greenhouse
	var s := Seasons.new()
	s.farming = f
	var temp := [2.0]   # lambdas capture locals by value: hold the air in an array
	s.temp_at = func(_p: Vector3i) -> float: return temp[0]
	s.sheltered = func(p: Vector3i) -> bool: return p.x >= 32
	assert_array(s.check_frost(0.0)).is_empty()   # 2 C: no frost
	temp[0] = -3.0
	var killed := s.check_frost(1.0)
	assert_array(killed).contains_exactly([Vector3i(0, 1, 0)])   # wheat (frost_c -1) dies
	assert_bool(f.crops.has(Vector3i(0, 1, 0))).is_false()
	assert_int(int(vox[Vector3i(0, 1, 0)])).is_equal(GameMaterials.id_of(Seasons.DEAD_CROP))
	assert_bool(f.crops.has(Vector3i(1, 1, 0))).is_true()        # carrots (frost_c -8) wait for spring
	assert_bool(f.crops.has(Vector3i(36, 1, 0))).is_true()       # the greenhouse wheat lives
	assert_int(int(s.stats["sheltered"])).is_equal(1)
	temp[0] = -10.0
	s.sheltered = Callable()
	s.check_frost(2.0)
	assert_bool(f.crops.has(Vector3i(1, 1, 0))).is_false()       # a hard frost takes the carrots too
	assert_bool(Seasons.kills({}, -0.5)).is_true()              # default frost_c is 0 C
	assert_bool(Seasons.kills({"frost_c": -8.0}, -5.0)).is_false()


func test_winter_rests_crops_and_spring_grows_them() -> void:
	var cal := Calendar.new()
	cal.set_date(0, 4)   # January
	assert_float(cal.crop_growth()).is_equal(0.0)
	cal.set_date(4, 4)   # May
	assert_float(cal.crop_growth()).is_greater(0.0)


func test_berries_follow_the_season_and_winter_meat_is_lean() -> void:
	var mining := MiningTable.load_default()
	var rng := RandomNumberGenerator.new()
	rng.seed = 7
	var bush: Array = mining.entry("blueberry_bush")["drops"]
	Seasons.season = 0   # winter: only the lean off-season entry (bug report 2026-10-27_15-26-49)
	var lean := MiningTable.roll_list(bush, rng)
	assert_int(lean.size()).is_equal(1)
	assert_int(int(lean[0]["count"])).is_less_equal(3)
	Seasons.season = 2   # summer
	var ripe := MiningTable.roll_list(bush, rng)
	assert_int(ripe.size()).is_equal(1)
	assert_int(int(ripe[0]["count"])).is_greater_equal(6)
	var meat := [{"id": "raw_meat", "count": [4, 4], "winter_mult": 0.5}]
	assert_int(int(MiningTable.roll_list(meat, rng)[0]["count"])).is_equal(4)
	Seasons.season = 0
	assert_int(int(MiningTable.roll_list(meat, rng)[0]["count"])).is_equal(2)
	assert_int(Seasons.lean(1, meat[0], 0, rng)).is_equal(1)   # never less than one
	Seasons.season = -1
	assert_array(MiningTable.roll_list(bush, rng)).is_not_empty()   # no calendar: always


func test_a_root_cellar_keeps_food_longest() -> void:
	var summer_air := 24.0
	var surface := Seasons.container_rate(0.0, false, summer_air, "chest")
	var shallow_room := Seasons.container_rate(2.0, true, summer_air, "chest")    # a dug cellar with a door
	var deep := Seasons.container_rate(4.0, false, summer_air, "chest")          # open, deep under ground
	var deep_room := Seasons.container_rate(4.0, true, summer_air, "chest")
	assert_float(surface).is_equal_approx(1.5, 1e-4)
	assert_float(deep).is_equal_approx(0.5, 1e-4)
	assert_float(shallow_room).is_equal_approx(0.5 * Settings.root_cellar_mult, 1e-4)
	assert_float(deep_room).is_equal_approx(shallow_room, 1e-4)
	assert_float(Seasons.container_rate(2.0, true, summer_air, "storage_pot")).is_equal_approx(shallow_room * 0.5, 1e-4)
	assert_float(Seasons.container_rate(1.0, true, summer_air, "chest")).is_equal_approx(surface, 1e-4)   # a hut is not a cellar
	assert_bool(Seasons.is_root_cellar(2.0, true)).is_true()
	# raw meat (2 days) in a root cellar: about 13 days instead of 2
	var inv := Inventory.new(items, 2)
	inv.add("raw_meat", 4)
	Survival.age_inventory(inv, 10.0, shallow_room)
	assert_str(String(inv.get_slot(0)["id"])).is_equal("raw_meat")
	Survival.age_inventory(inv, 4.0, shallow_room)
	assert_str(String(inv.get_slot(0)["id"])).is_equal("rot")


func test_preserved_food_spoils_slower() -> void:
	var reg := ItemRegistry.new()
	reg.load_json(JSON.stringify({"items": [
		{"id": "raw_meat", "name": "Raw Meat", "tags": ["food", "meat"], "stack": 64, "food": {"hunger": 2}, "spoil_days": 2},
		{"id": "jerky", "name": "Jerky", "tags": ["food", "meat", "preserved"], "stack": 64, "food": {"hunger": 2}, "spoil_days": 2},
		{"id": "rot", "name": "Rot", "tags": [], "stack": 64}]}))
	var inv := Inventory.new(reg, 2)
	inv.add("raw_meat", 1)
	inv.add("jerky", 1)
	Survival.age_inventory(inv, 1.0, 1.0)
	assert_float(float(inv.get_slot(0)["spoil"])).is_equal_approx(0.5, 1e-4)
	assert_float(float(inv.get_slot(1)["spoil"])).is_equal_approx(0.5 * Settings.preserved_spoil_mult, 1e-4)


# ---------------- discoveries ----------------

func test_the_journal_lists_entries_from_the_data() -> void:
	var d := Discoveries.load_default(items)
	assert_array(d.errors).is_empty()
	for id in ["biome:forest", "biome:plains", "landmark:ruin", "landmark:cave", "animal:sheep", "animal:bear", "ore:copper_ore",
			"ore:loose_tin_ore", "vein:copper", "vein:copper_iron"]:
		assert_bool(d.has(id)).override_failure_message("missing " + id).is_true()
	assert_bool(d.has("biome:lush_cave")).is_false()   # underground biomes are the Caves kind's (FineGen.cave_biome_at)
	for id in ["cave:lush_cave", "cave:dripstone_cave", "cave:deep_dark", "cave:sulfur_cave"]:
		assert_bool(d.has(id)).override_failure_message("missing " + id).is_true()
	assert_bool(d.has("cave:plains")).is_false()
	assert_str(String(d.entries["cave:lush_cave"]["title"])).is_equal("Lush Cave")
	assert_str(String(d.entries["biome:forest"]["text"])).contains("Oak")
	assert_str(String(d.entries["animal:pig"]["title"])).is_equal("Pig")
	for id in d.entries:
		var icon := String(d.entries[id]["icon"])
		assert_bool(icon == "" or items.has(icon)).override_failure_message("%s: unknown icon %s" % [id, icon]).is_true()


func test_new_content_appears_without_code() -> void:
	var d := Discoveries.new()
	var data := {"kinds": [{"id": "landform", "from": "landforms", "text": "Stood on a %s."},
		{"id": "animal", "from": "mobs", "text": "Met a %s."}, {"id": "stone", "from": "items", "tags": ["stone_type"]}]}
	var reg := ItemRegistry.new()
	reg.load_json(JSON.stringify({"items": [{"id": "chalk", "name": "Chalk", "tags": ["stone_type"], "stack": 64, "lore": "Soft and white."}]}))
	d.build(data, {"biomes": {"landforms": ["canyon", {"name": "sea_stack", "display": "Sea Stack"}]},
		"mobs": {"wolf": {"name": "Wolf"}}, "items": reg})
	assert_str(String(d.entries["landform:canyon"]["text"])).is_equal("Stood on a Canyon.")
	assert_str(String(d.entries["landform:sea_stack"]["title"])).is_equal("Sea Stack")
	assert_str(String(d.entries["animal:wolf"]["text"])).is_equal("Met a Wolf.")
	assert_str(String(d.entries["stone:chalk"]["text"])).is_equal("Soft and white.")


func test_a_discovery_triggers_once_and_saves() -> void:
	var d := Discoveries.load_default(items)
	d.date_text = func() -> String: return "3 April, year 1 (Spring)"
	d.clock_days = func() -> float: return 3.25
	var seen := []
	d.discovered.connect(func(id: String) -> void: seen.append(id))
	assert_bool(d.note("biome:forest")).is_true()
	assert_bool(d.note("biome:forest")).is_false()   # once
	assert_bool(d.note("biome:no_such_place")).is_false()
	assert_array(seen).contains_exactly(["biome:forest"])
	assert_bool(d.dirty).is_true()
	assert_str(String(d.found["biome:forest"]["date"])).contains("April")
	assert_vector(d.kind_progress("biome")).is_equal(Vector2i(1, d.kind_progress("biome").y))
	var d2 := Discoveries.load_default(items)
	d2.load_dict(d.to_dict())
	assert_bool(d2.is_found("biome:forest")).is_true()
	assert_bool(d2.note("biome:forest")).is_false()


# ---------------- trackers ----------------

func test_a_pinned_recipe_ticks_off_as_the_inventory_fills() -> void:
	var b := RecipeBrowser.new(items, recipes)
	var inv := Inventory.new(items, 36)
	b.have = func(id: String) -> int: return inv.count_of(id)
	var t := Trackers.new(b, null)
	assert_bool(t.toggle("recipe", "firepit")).is_true()
	var v := t.view(t.pins[0])
	assert_str(String(v["title"])).is_equal(items.display_name("firepit"))
	assert_bool(v["done"]).is_false()
	assert_int(v["rows"].size()).is_equal(2)   # loose stones and sticks (laid on the ground, ADR 0083)
	inv.add("stick", 2)
	inv.add("loose_stone", 3)
	v = t.view(t.pins[0])
	assert_bool(v["done"]).is_false()
	inv.add("loose_stone", 1)
	v = t.view(t.pins[0])
	assert_bool(v["done"]).is_true()
	assert_bool(t.toggle("recipe", "no_such_recipe")).is_false()
	assert_bool(t.toggle("recipe", "firepit")).is_false()   # a second press unpins
	assert_array(t.pins).is_empty()


func test_trackers_keep_three_and_drop_finished_quests() -> void:
	var book := QuestBook.load_default(items)
	var b := RecipeBrowser.new(items, recipes)
	var t := Trackers.new(b, book)
	var q: String = book.quests.keys()[0]
	t.pin("quest", q)
	for rid in ["firepit", "flint_axe", "flint_pickaxe"]:
		t.pin("recipe", rid)
	assert_int(t.pins.size()).is_equal(Trackers.MAX)
	assert_bool(t.is_pinned("quest", q)).is_false()   # the oldest made room
	t.toggle("recipe", "flint_axe")
	t.pin("quest", q)
	var v := t.view(t.pins[-1])
	assert_str(String(v["title"])).is_equal(String(book.quests[q]["title"]))
	book.complete(q)
	t.prune()
	assert_bool(t.is_pinned("quest", q)).is_false()
	var t2 := Trackers.new(b, book)
	t2.load_dict(t.to_dict())
	assert_array(t2.pins).is_equal(t.pins)


# ---------------- prospecting ----------------

func _fake_veins(veins: Dictionary, cell_v: int) -> Callable:
	return func(x: int, z: int) -> Dictionary: return veins.get(Vector2i(floori(float(x) / cell_v), floori(float(z) / cell_v)), {})


func test_a_vein_holds_its_ores_in_the_data_shares() -> void:
	var p := Prospecting.load_default()
	assert_array(p.errors).is_empty()
	var c := p.composition("copper")
	var total := 0.0
	for m in c:
		total += float(c[m])
	assert_float(total).is_equal_approx(p.density, 1e-5)
	assert_float(float(c["copper_ore"])).is_greater(float(c["tin_ore"]))
	assert_bool(c.has("iron_ore")).is_true()   # sporadic


func test_the_copper_pick_reads_veins_in_range_exactly_the_flint_pick_only_senses() -> void:
	var p := Prospecting.load_default()
	var near := {"name": "copper", "x": 100, "y": 900, "z": 100, "size": 64, "thickness": 16}         # 256 x 64 voxel lens
	var far := {"name": "iron", "x": p.cell_v + 600, "y": 900, "z": 100, "size": 64, "thickness": 16}
	var va := _fake_veins({Vector2i(0, 0): near, Vector2i(1, 0): far}, p.cell_v)
	var tap := Vector3i(100 + 128 + 40, 900, 100)   # 40 voxels (5 m) beside the copper lens
	var found := p.veins_near(va, tap, Settings.m2v(12.0))
	assert_int(found.size()).is_equal(1)
	assert_str(String(found[0]["name"])).is_equal("copper")
	assert_float(float(found[0]["dist_m"])).is_equal_approx(5.0, 0.01)
	var exact := p.read(va, tap, {"range_m": 12.0, "exact": true})
	assert_bool(exact["found"]).is_true()
	assert_str(String(exact["lines"][0])).contains("Copper vein 5 m away")
	assert_str(String(exact["lines"][0])).contains("Copper Ore")
	assert_str(String(exact["lines"][0])).contains("%")
	var flint := p.read(va, tap, {"range_m": 12.0, "exact": false})
	assert_array(flint["veins"]).is_empty()
	assert_str(String(flint["lines"][0])).starts_with("Something")
	var nothing := p.read(va, Vector3i(100, 400, 100), {"range_m": 12.0, "exact": true})   # 60 m under it
	assert_bool(nothing["found"]).is_false()
	assert_str(String(nothing["lines"][0])).starts_with("Nothing")


## The pick's panel settings (ADR 0091, bug report 2026-10-27_17-51-34): distance, ore, mode.
func test_prospecting_settings_pick_distance_ore_and_mode() -> void:
	var p := Prospecting.load_default()
	var cu := {"name": "copper", "x": 100, "y": 900, "z": 100, "size": 64, "thickness": 16}
	var fe := {"name": "iron", "x": p.cell_v + 300, "y": 900, "z": 100, "size": 64, "thickness": 16}
	var va := _fake_veins({Vector2i(0, 0): cu, Vector2i(1, 0): fe}, p.cell_v)
	var tap := Vector3i(100 + 128 + 40, 900, 100)   # 5 m beside copper, 84 m from the iron lens
	var pick := {"range_m": 96.0, "exact": true}
	assert_float(p.reach_m(pick)).is_equal(96.0)
	assert_int(p.read(va, tap, pick)["veins"].size()).is_equal(2)
	p.mode = "nearest"
	var near: Array = p.read(va, tap, pick)["veins"]
	assert_int(near.size()).is_equal(1)
	assert_str(String(near[0]["name"])).is_equal("copper")
	p.ore = "iron_ore"
	p.mode = "all"
	var r := p.read(va, tap, pick)
	assert_int(r["veins"].size()).is_equal(2)   # copper veins hold sporadic iron too
	p.ore = "tin_ore"
	assert_int(p.read(va, tap, pick)["veins"].size()).is_equal(1)   # only the copper vein holds tin
	p.ore = "gold_ore"
	assert_str(String(p.read(va, tap, pick)["lines"][0])).starts_with("No Gold Ore")
	p.ore = ""
	p.range_m = 12.0
	assert_int(p.read(va, tap, pick)["veins"].size()).is_equal(1)
	assert_float(p.reach_m({"range_m": 24.0})).is_equal(12.0)
	p.range_m = 48.0
	assert_float(p.reach_m({"range_m": 24.0})).is_equal(24.0)   # capped by the pick
	# the density map: bright over the copper lens, dark away from both
	p.range_m = 48.0
	p.mode = "map"
	p.read(va, Vector3i(100, 600, 100), pick)
	var n := Prospecting.MAP_N
	assert_int(p.map.size()).is_equal(n * n)
	assert_float(p.map[(n / 2) * n + n / 2]).is_greater(0.5)   # straight over the lens, 37 m above it
	assert_float(p.map[0]).is_equal(0.0)
	assert_str(String(p.last["mode"])).is_equal("map")
	# the flint pick ignores ore and mode
	p.ore = "coal_ore"
	assert_str(String(p.read(va, tap, {"range_m": 24.0, "exact": false})["lines"][0])).starts_with("Something")
	# saved with the world
	var q := Prospecting.load_default()
	q.load_dict(p.to_dict())
	assert_str(q.ore).is_equal("coal_ore")
	assert_str(q.mode).is_equal("map")
	assert_float(q.range_m).is_equal(48.0)
	assert_array(p.ore_names()).contains(["copper_ore", "iron_ore", "tin_ore", "coal_ore"])


func test_the_prospecting_panel_shows_and_sets_the_settings() -> void:
	var p := Prospecting.load_default()
	var m := ProspectMenu.new()
	m.prospecting = p
	add_child(m)
	m.open(items.get_def("flint_prospecting_pick"), true)
	assert_bool(m.is_open).is_true()
	assert_bool(m._buttons["range:96"].disabled).is_true()   # past the flint pick's 24 m
	assert_bool(m._buttons["mode:map"].disabled).is_true()
	m.close()
	m.open(items.get_def("copper_prospecting_pick"), true)
	assert_bool(m._buttons["range:96"].button_pressed).is_true()
	m._buttons["mode:map"].pressed.emit()
	assert_str(p.mode).is_equal("map")
	assert_bool(m._map.visible).is_true()
	m._buttons["range:24"].pressed.emit()
	assert_float(p.range_m).is_equal(24.0)
	m.close()
	m.free()


func test_the_picks_are_made_and_marked_as_prospecting_tools() -> void:
	for id in ["flint_prospecting_pick", "copper_prospecting_pick"]:
		var def := items.get_def(id)
		assert_bool(def.has("prospects")).is_true()
		assert_bool(recipes.recipes.has(id)).override_failure_message("no recipe for " + id).is_true()
	assert_bool(bool(items.get_def("copper_prospecting_pick")["prospects"]["exact"])).is_true()
	assert_bool(bool(items.get_def("flint_prospecting_pick")["prospects"]["exact"])).is_false()
	assert_bool(recipes.recipes.has("knap_flint_prospecting_head")).is_true()
	assert_bool(recipes.recipes.has("smith_copper_prospecting_head")).is_true()
