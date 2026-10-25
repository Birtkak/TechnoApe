extends GdUnitTestSuite

## Headless tests for the four Stone Age stages (ADR 0059): the data (items, recipes of the new
## types, no orphan item), the book's graph (every quest reachable, no cycle, one final per chapter),
## and each new mechanic's rules: earth ovens, drying and smoking racks, stroke work (grinding with
## grit and water, sewing, the loom), things set in water (retting, traps), tar and tempered flint in
## durability, clothing warmth, stone circles and the solstice sunrise.

var items: ItemRegistry
var recipes: RecipeRegistry
var mining: MiningTable
var store: BlockEntities
var book: QuestBook


func before_test() -> void:
	items = ItemRegistry.load_default()
	recipes = RecipeRegistry.load_default(items)
	mining = MiningTable.load_default()
	store = BlockEntities.new(items, recipes)
	book = QuestBook.load_default(items)
	StoneAge.now_days = 10.0


func _def(id: String) -> Dictionary:
	return items.get_def(id)


# ---------- data ----------

func test_data_loads_clean() -> void:
	assert_array(items.errors).is_empty()
	assert_array(recipes.errors).is_empty()
	assert_array(book.errors).is_empty()
	for t in ["bury", "rack", "stroke", "soak", "trap"]:
		assert_array(recipes.of_type(t)).override_failure_message("no %s recipes" % t).is_not_empty()
	var crops: Dictionary = JSON.parse_string(FileAccess.get_file_as_string("res://data/crops.json"))["crops"]
	for c in ["einkorn", "emmer", "flax"]:
		assert_str(String(_def(crops[c]["seed"]).get("plants", ""))).is_equal(c)


func test_bad_stone_age_recipes_are_reported() -> void:
	var r := RecipeRegistry.new(items)
	r.load_json(JSON.stringify({"recipes": [
		{"id": "a", "type": "bury", "ingredients": ["flint"], "output": {"id": "tempered_flint"}},
		{"id": "b", "type": "stroke", "ingredients": ["flint"], "on": "moon", "strokes": 4, "output": {"id": "flour"}},
		{"id": "c", "type": "rack", "ingredients": ["raw_meat"], "input": "raw_meat", "mode": "boil", "hours": 2, "output": {"id": "dried_meat"}},
		{"id": "d", "type": "trap", "ingredients": [], "output": {"id": "raw_fish"}}]}))
	var text := "\n".join(r.errors)
	for want in ["bury recipe a needs hours", "stroke recipe b needs strokes and on", "rack recipe c: mode", "trap recipe d needs ingredients"]:
		assert_str(text).contains(want)


## Stubs of another part's items load without a duplicate error, and step aside when that part's own
## definition came first.
func test_stub_items_step_aside() -> void:
	var r := ItemRegistry.new()
	r.load_json(JSON.stringify({"items": [{"id": "antler", "name": "Part E Antler", "tags": ["antler"]}]}))
	r.load_json(JSON.stringify({"items": [{"id": "antler", "name": "Stub", "stub": "part_e"}]}))
	assert_array(r.errors).is_empty()
	assert_str(r.display_name("antler")).is_equal("Part E Antler")
	r.load_json(JSON.stringify({"items": [{"id": "antler", "name": "Again"}]}))
	assert_str("\n".join(r.errors)).contains("duplicate item id antler")


## Every new item can be had (ground pickups, drops, the world's own sources, recipes) and is good for
## something: an ingredient, a tool, food, clothing, a light, a quest task, or a placed piece.
func test_no_orphan_item() -> void:
	var start: Array = ["flint", "stick", "loose_stone", "loose_copper_ore", "clay_ball", "dry_grass", "oak_log", "birch_log",
		"gravel", "sand", "plant_fiber", "dirt", "raw_hide", "raw_meat", "bone", "feather", "bear_hide", "animal_fat",
		"antler", "pyrite", "red_ochre", "yellow_ochre", "floorstone_flint", "einkorn_seeds", "emmer_seeds", "flax_seeds", "chalk_stone"]
	start.append_array(StoneAge.WORLD_SOURCES.keys())
	start.append(mining.drops("stone", _def("copper_pickaxe")).get("id", ""))
	var have := recipes.reachable_from(start)
	var used := {}
	for rid in recipes.recipes:
		for spec in recipes.ingredient_specs(recipes.recipes[rid]):
			for id in items.all_ids():
				if items.matches(id, spec):
					used[id] = true
	for qid in book.quests:
		for t in book.quests[qid]["tasks"]:
			for spec in QuestBook.task_specs(t):
				for id in items.all_ids():
					if items.matches(id, spec):
						used[id] = true
	var data: Dictionary = JSON.parse_string(FileAccess.get_file_as_string("res://data/stone_age_items.json"))
	var end_uses := ["tool", "food", "clothing", "light", "paint", "boat", "bag", "jewellery", "station", "container_small"]
	for d in data["items"]:
		var id: String = d["id"]
		if d.has("stub") or d.get("found_only", false):
			continue
		assert_bool(have.has(id)).override_failure_message("%s cannot be had" % id).is_true()
		var end: bool = d.has("work") or d.has("clothing") or d.has("launch") or d.has("shoots")
		for t in d.get("tags", []):
			if t in end_uses:
				end = true
		assert_bool(end or used.has(id)).override_failure_message("%s is good for nothing" % id).is_true()


# ---------- the book ----------

## Every quest can be completed: its deps exist and none loops back on itself (a topological order
## takes them all), and every chapter of the Stone Age has one final quest that is not a side road.
func test_quest_graph_reaches_every_quest() -> void:
	var done := {}
	var left: Array = book.quests.keys()
	var guard := 0
	while not left.is_empty() and guard < 500:
		guard += 1
		var next := []
		for id in left:
			var ok := true
			for d in book.quests[id].get("deps", []):
				if not done.has(d):
					ok = false
			if ok:
				done[id] = true
			else:
				next.append(id)
		if next.size() == left.size():
			break
		left = next
	assert_array(left).override_failure_message("unreachable quests: %s" % [left]).is_empty()
	for c in ["flint_age", "middle_stone_age", "new_stone_age", "copper_stone_age"]:
		var finals := book.chapter_quests(c).filter(func(q: Dictionary) -> bool: return q.get("final", false))
		assert_int(finals.size()).override_failure_message(c).is_equal(1)
		assert_bool(finals[0].get("optional", false)).is_false()
	# every quest of today's Flint Age kept its id (saves keep progress)
	for id in ["gather_flint", "knapping", "lashing", "flint_spear", "pit_kiln", "melt_copper", "copper_tool", "storage_pot", "charcoal_pit"]:
		assert_bool(book.quests.has(id)).override_failure_message(id).is_true()
	assert_int(book.quests.size()).is_equal(78)


func test_tar_hafting_event_counts() -> void:
	for q in ["gather_flint", "sticks_stones", "knapping", "lashing", "flint_knife", "flint_shovel", "dry_grass", "firestarter", "firepit", "birch_tar"]:
		book.complete(q)
	book.note("tar_hafted", "flint_axe")
	assert_bool(book.is_done("tar_haft")).is_true()
	assert_bool(book.is_done("microliths")).is_false()


# ---------- tar, tempered flint and durability ----------

func test_hafted_tools_last_longer() -> void:
	assert_float(StoneAge.hafted_x(items, ["flint_axe_head", "stick", "cordage"], [{}, {}, {}])).is_equal(1.0)
	assert_float(StoneAge.hafted_x(items, ["flint_axe_head", "stick", "birch_tar"], [{}, {}, {}])).is_equal(2.0)
	assert_float(StoneAge.hafted_x(items, ["flint_axe_head", "stick", "birch_tar"], [{"dur_x": 1.5}, {}, {}])).is_equal(3.0)
	# assembled on the ground: the tool gets dur_x and so twice the uses
	var a := AssemblyEntity.new(recipes, Vector3i.ZERO)
	a.parts.append("flint_axe_head")
	a.part_data.append({"dur_x": 1.5})
	a.use(_def("stick"), {"id": "stick", "count": 3})
	var r := a.use(_def("birch_tar"), {"id": "birch_tar", "count": 2})
	assert_str(r["done"]["id"]).is_equal("flint_axe")
	assert_float(float(r["done"]["dur_x"])).is_equal(3.0)
	var tool := Inventory.make_stack(items, "flint_axe", 1, r["done"])
	assert_int(Inventory.durability_of(items, tool)).is_equal(int(_def("flint_axe")["durability"]) * 3)
	assert_int(Inventory.uses_left(items, tool)).is_equal(int(_def("flint_axe")["durability"]) * 3)
	# the head comes back with its data when taken off
	var b := AssemblyEntity.new(recipes, Vector3i.ZERO)
	b.parts.append("flint_axe_head")
	b.part_data.append({"dur_x": 1.5})
	assert_float(float(b.use({})["give"][0].get("dur_x", 1.0))).is_equal(1.5)


func test_tempered_heads_average_on_merge() -> void:
	var inv := Inventory.new(items, 4)
	inv.add("flint_axe_head", 1, {"dur_x": 1.5})
	inv.add("flint_axe_head", 1)
	assert_float(float(inv.get_slot(0)["dur_x"])).is_equal_approx(1.25, 0.001)


func test_knapping_takes_bone_antler_copper_and_tempered_flint() -> void:
	for pair in [["tempered_flint", "knap_microliths"], ["bone", "knap_bone_needle"], ["antler", "knap_antler_pick"], ["copper_nugget", "hammer_copper_awl"]]:
		var ok := false
		for r in recipes.of_type("knap"):
			if r["id"] == pair[1] and items.matches(pair[0], r["input"]):
				ok = true
		assert_bool(ok).override_failure_message(pair[0]).is_true()
	assert_bool(items.matches("bone", recipes.recipes["knap_flint_axe_head"]["input"])).is_false()


# ---------- the earth oven ----------

func _oven_cycle(first: String, cover: String) -> EarthOvenEntity:
	var o := store.create(Vector3i(0, 0, 0), "earth_oven") as EarthOvenEntity
	for i in 4:
		assert_int(o.use(_def(first))["consume"]).is_equal(1)
	assert_int(o.use(_def(first))["consume"]).is_equal(0)   # full: the cover is next
	for i in 2:
		assert_int(o.use(_def(cover))["consume"]).is_equal(items.units(cover))
	for i in 4:
		assert_int(o.use(_def("stick"))["consume"]).is_equal(1)
	assert_bool(o.use(_def("firestarter"))["lit"]).is_true()
	return o


func test_earth_oven_bakes_birch_tar_in_game_hours() -> void:
	var o := _oven_cycle("birch_bark", "dirt")
	var got := []
	o.baked.connect(func(id: String) -> void: got.append(id))
	assert_bool(o.is_lit()).is_true()
	StoneAge.now_days += 1.9 / 24.0
	store.tick()
	assert_array(got).is_empty()
	StoneAge.now_days += 0.2 / 24.0
	store.tick()
	assert_array(got).is_equal(["birch_tar"])
	assert_bool(o.is_lit()).is_false()
	var r := o.use({})
	assert_array(r["give"]).is_equal([{"id": "birch_tar", "count": 2}])
	assert_bool(o.output.is_empty()).is_true()   # the pit is ready for the next batch


func test_earth_oven_tempers_flint_under_sand_only() -> void:
	var o := store.create(Vector3i(0, 0, 0), "earth_oven") as EarthOvenEntity
	for i in 4:
		o.use(_def("flint"))
	assert_int(o.use(_def("dirt"))["consume"]).is_equal(0)   # flint wants sand
	assert_str(o.use(_def("dirt"))["msg"]).contains("sand")
	var t := _oven_cycle("flint", "sand")
	StoneAge.now_days += 3.01 / 24.0
	store.tick()
	assert_dict(t.output).is_equal({"id": "tempered_flint", "count": 4})


func test_earth_oven_gives_back_and_saves() -> void:
	var o := store.create(Vector3i(0, 0, 0), "earth_oven") as EarthOvenEntity
	o.use(_def("birch_bark"))
	o.use(_def("birch_bark"))
	assert_array(o.use({})["give"]).is_equal([{"id": "birch_bark", "count": 1}])
	var lit := _oven_cycle("birch_bark", "dirt")
	var copy := EarthOvenEntity.new(recipes, Vector3i.ZERO)
	copy.load_from(JSON.parse_string(JSON.stringify(lit.to_dict())))
	assert_bool(copy.is_lit()).is_true()
	assert_str(String(copy.recipe["id"])).is_equal("bury_birch_tar")
	assert_int(copy.loaded.size()).is_equal(4)


# ---------- racks ----------

func test_rack_dries_slowly_and_smokes_fast() -> void:
	var dry := store.create(Vector3i(0, 0, 0), "rack") as RackEntity
	var smoke := store.create(Vector3i(8, 0, 0), "rack") as RackEntity
	smoke.smoky = true
	dry.use(_def("raw_meat"))
	smoke.use(_def("raw_meat"))
	assert_int(dry.use(_def("flint"))["consume"]).is_equal(0)
	store.tick()   # the first tick only starts the clock
	for h in 12:
		StoneAge.now_days += 1.0 / 24.0
		store.tick()
	assert_str(String(smoke.hung[0]["done"])).is_equal("smoked_meat")
	assert_str(String(dry.hung[0]["done"])).is_equal("")
	assert_float(float(dry.hung[0]["p"])).is_equal_approx(0.5, 0.01)
	for h in 12:
		StoneAge.now_days += 1.0 / 24.0
		store.tick()
	assert_str(String(dry.hung[0]["done"])).is_equal("dried_meat")
	assert_array(smoke.use({})["give"]).is_equal([{"id": "smoked_meat", "count": 1}])
	assert_float(float(_def("smoked_meat")["spoil_days"])).is_greater(float(_def("dried_meat")["spoil_days"]))
	assert_float(float(_def("dried_meat")["spoil_days"])).is_greater(float(_def("raw_meat")["spoil_days"]) * 3.0)


# ---------- stroke work ----------

func test_polishing_needs_grit_water_and_strokes() -> void:
	var e := store.create(Vector3i(0, 0, 0), "stroke") as StrokeEntity
	e.station = "sandstone"
	e.water_ok = false
	e.set_recipe(recipes.recipes["grind_polished_axe_head"])
	assert_int(e.use(_def("axe_roughout"))["consume"]).is_equal(1)
	assert_bool(e.ready()).is_true()
	assert_str(e.use(_def("loose_stone"))["msg"]).contains("water")
	e.water_ok = true
	assert_str(e.use(_def("loose_stone"))["msg"]).contains("sand")
	var done := {}
	var grit_added := 0
	for i in 200:
		if e.needs_grit():
			e.use(_def("sand"))
			grit_added += 1
		var r := e.use(_def("loose_stone"))
		if not r["done"].is_empty():
			done = r["done"]
			break
	assert_dict(done).is_equal({"id": "polished_axe_head", "count": 1})
	assert_int(grit_added).is_equal(3)   # 48 strokes, sand every 16


func test_stroke_options_by_surface() -> void:
	var ids := func(a: Array) -> Array: return a.map(func(r: Dictionary) -> String: return r["id"])
	assert_array(ids.call(StrokeEntity.options(recipes, "stone", "einkorn"))).is_equal(["grind_einkorn"])
	assert_array(ids.call(StrokeEntity.options(recipes, "ground", "einkorn"))).is_empty()
	assert_array(ids.call(StrokeEntity.options(recipes, "ground", "fur_pelt"))).contains(["sew_fur_hood", "sew_fur_cloak", "sew_fur_boots", "sew_bearskin_cap"])
	assert_array(ids.call(StrokeEntity.options(recipes, "sandstone", "axe_roughout"))).is_equal(["grind_polished_axe_head"])


func test_sewing_a_cloak_and_the_loom_stays() -> void:
	var e := store.create(Vector3i(0, 0, 0), "stroke") as StrokeEntity
	e.set_recipe(recipes.recipes["sew_fur_cloak"])
	for id in ["fur_pelt", "fur_pelt", "fur_pelt", "cordage", "cordage"]:
		assert_int(e.use(_def(id))["consume"]).is_equal(1)
	assert_str(e.use(_def("stick"))["msg"]).contains("needle")
	var r := {}
	for i in 24:
		r = e.use(_def("bone_needle"))
	assert_dict(r["done"]).is_equal({"id": "fur_cloak", "count": 1})
	assert_bool(r["remove"]).is_true()
	var loom := store.create(Vector3i(8, 0, 0), "loom") as StrokeEntity
	loom.set_recipe(recipes.recipes["weave_linen"])
	for i in 4:
		loom.use(_def("linen_thread"))
	for i in 16:
		r = loom.use(_def("stick"))
	assert_str(r["done"]["id"]).is_equal("linen_cloth")
	assert_bool(r["remove"]).is_false()


# ---------- water ----------

func test_flax_rets_and_traps_catch() -> void:
	var w := store.create(Vector3i(0, 0, 0), "water_work") as WaterWorkEntity
	for i in 4:
		w.use(_def("flax"))
	assert_bool(w.complete()).is_true()
	StoneAge.now_days += 35.0 / 24.0
	store.tick()
	assert_array(w.ready).is_empty()
	StoneAge.now_days += 1.1 / 24.0
	store.tick()
	assert_array(w.ready).is_equal([{"id": "retted_flax", "count": 4}])
	var res := w.use({})
	assert_bool(res["remove"]).is_true()
	var t := store.create(Vector3i(8, 0, 0), "water_work") as WaterWorkEntity
	t.use(_def("basket_trap"))
	StoneAge.now_days += 80.0 / 24.0   # 20 tries at 50 %
	store.tick()
	assert_int(t.ready.size()).is_equal(WaterWorkEntity.MAX_CAUGHT)
	var give: Array = t.use({})["give"]
	assert_str(give[0]["id"]).is_equal("raw_fish")
	assert_bool(t.complete()).is_true()   # the trap stays set
	# a weir in a river tries twice as often as in still water
	var r1 := store.create(Vector3i(16, 0, 0), "water_work") as WaterWorkEntity
	r1.use(_def("fish_weir"))
	r1.river = true
	StoneAge.now_days += 6.0 / 24.0
	store.tick()
	assert_int(r1.tries).is_equal(4)


# ---------- clothing ----------

func test_clothing_warms_in_the_cold_only() -> void:
	var c := Clothing.new(items)
	assert_dict(c.wear({"id": "fur_cloak", "count": 1})).is_empty()
	c.wear({"id": "fur_hood", "count": 1})
	var old := c.wear({"id": "bearskin_cap", "count": 1})
	assert_str(old["id"]).is_equal("fur_hood")
	assert_float(c.warmth_c()).is_equal(15.0)
	assert_float(Clothing.felt_c(-10.0, c.warmth_c())).is_equal(5.0)
	assert_float(Clothing.felt_c(25.0, c.warmth_c())).is_equal(Settings.comfort_max_c)
	assert_float(Clothing.felt_c(35.0, c.warmth_c())).is_equal(35.0)
	var c2 := Clothing.new(items)
	c2.load_dict(JSON.parse_string(JSON.stringify(c.to_dict())))
	assert_float(c2.warmth_c()).is_equal(15.0)


# ---------- megaliths ----------

func test_stone_circle_needs_a_ring_and_an_aligned_heel_stone() -> void:
	var r := Settings.m2v(4.0)
	var ring := []
	for i in 6:
		var a := TAU * i / 6.0
		ring.append(Vector3(cos(a) * r, 0, sin(a) * r))
	var sun := Vector2(0.6, -0.8)
	var heel := Vector3(sun.x, 0, sun.y) * r * 2.0
	var found := Megaliths.find_circle(ring + [heel], sun)
	assert_dict(found).is_not_empty()
	assert_int(found["ring"].size()).is_equal(6)
	assert_vector(found["heel_pos"]).is_equal(heel)
	# the heel stone pointing elsewhere: no circle
	assert_dict(Megaliths.find_circle(ring + [Vector3(-sun.x, 0, -sun.y) * r * 2.0], sun)).is_empty()
	# too few stones
	assert_dict(Megaliths.find_circle(ring.slice(0, 4) + [heel], sun)).is_empty()
	# a stone far out of the ring is not part of it, and the rest still counts
	var bent: Array = ring.duplicate()
	bent.append(Vector3(r * 0.2, 0, r * 0.1))
	assert_dict(Megaliths.find_circle(bent + [heel], sun)).is_not_empty()


func test_megalith_footprint_follows_its_axis() -> void:
	var e := store.create(Vector3i(0, 0, 0), "megalith") as MegalithEntity
	assert_array(e.footprint_cells()).is_empty()   # no axis yet: nothing registered at creation
	e.axis = "x"
	var b := Settings.BLOCK
	assert_int(e.footprint_cells().size()).is_equal(MegalithEntity.LEN * b * b * b)
	assert_bool(e.footprint_cells().has(Vector3i(MegalithEntity.LEN * b - 1, b - 1, b - 1))).is_true()
	e.axis = "y"
	assert_bool(e.footprint_cells().has(Vector3i(0, MegalithEntity.LEN * b - 1, 0))).is_true()
	assert_bool(e.movable()).is_false()
	e.axis = "z"
	e.rollers = 2
	e.ropes = 2
	assert_bool(e.movable()).is_true()


func test_solstice_sunrise_is_in_the_northeast() -> void:
	var cal := Calendar.new()
	var best := -2.0
	var best_d := 0
	for d in cal.year_days():
		var y := cal.sun_dir_at(float(d) + 0.5).y
		if y > best:
			best = y
			best_d = d
	var lo := float(best_d)
	var hi := lo + 0.5
	for i in 30:
		var mid := (lo + hi) * 0.5
		if cal.sun_dir_at(mid).y < 0.0:
			lo = mid
		else:
			hi = mid
	var s := cal.sun_dir_at(hi)
	# north is -Z in the game: the midsummer sun rises north of east at 45 degrees north
	assert_float(s.z).is_less(0.0)
	assert_float(s.x).is_greater(0.0)


# ---------- the dugout ----------

func test_dugout_burns_then_takes_adze_strokes() -> void:
	var d := store.create(Vector3i(0, 0, 0), "dugout") as DugoutEntity
	d.axis = "x"
	d.length = 20
	assert_int(d.footprint_cells().size()).is_equal(20 * DugoutEntity.HIGH * DugoutEntity.WIDE)
	assert_str(d.use(_def("flint_adze"))["msg"]).contains("dry grass")
	for i in DugoutEntity.GRASS:
		d.use(_def("dry_grass"))
	assert_bool(d.use(_def("firestarter"))["lit"]).is_true()
	StoneAge.now_days += (Settings.dugout_burn_h + 0.1) / 24.0
	store.tick()
	assert_bool(d.burnt).is_true()
	var r := {}
	for i in DugoutEntity.ADZE_STROKES:
		r = d.use(_def("flint_adze"))
	assert_dict(r["done"]).is_equal({"id": "dugout_canoe", "count": 1})


# ---------- blueprints ----------

func test_longhouse_blueprints_load_as_prefabs() -> void:
	var base := "user://test_prefabs_%d" % Time.get_ticks_usec()
	assert_int(StoneAge.install_blueprints(base)).is_equal(StoneAge.BLUEPRINTS.size())
	assert_int(StoneAge.install_blueprints(base)).is_equal(0)   # already there: kept
	var store := PrefabStore.new(base)
	assert_array(store.list().get("blueprints", [])).contains(["longhouse", "small longhouse"])
	var p := store.load_prefab("blueprints", "longhouse")
	assert_dict(p).is_not_empty()
	var count := {}
	for m in p["grid"]:
		count[m] = int(count.get(m, 0)) + 1
	for name in ["daub_wall", "thatch", "wood"]:
		assert_int(int(count.get(GameMaterials.id_of(name), 0))).override_failure_message(name).is_greater(100)
