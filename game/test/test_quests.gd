extends GdUnitTestSuite

## Headless tests for the progression book (ADR 0028): quests.json against
## the item list, every task reachable from ground pickups, latching item
## tasks, deps cascading, "any" quests, the pit kiln event, saving, and the
## main road of the four Stone Age chapters (ADR 0059) walked through a player inventory to a copper tool.

const GROUND := ["flint", "stick", "loose_stone", "loose_copper_ore", "clay_ball", "dry_grass", "oak_log", "gravel", "sand",
	"plant_fiber", "dirt", "raw_hide", "raw_meat", "carrot", "wool", "potato", "onion", "beans", "wild_strawberry",
	# drops and finds the Stone Age chapters lean on (ADR 0059): animals, tall grass, part E's stones and fish
	"bone", "feather", "bear_hide", "animal_fat", "antler", "pyrite", "red_ochre", "yellow_ochre", "floorstone_flint",
	"einkorn_seeds", "emmer_seeds", "flax_seeds", "birch_log",
	# ore-bearing stones above veins and ore mined out of them (ADR 0070)
	"loose_tin_ore", "loose_iron_ore", "tin_ore", "iron_ore",
	"sulfur",   # mined from sulfur cave crust (mining.json): the electrical age's rubber (ADR 0113)
	"crystal",  # geodes (ADR 0052): the miner's helmet's lamp
	"bowl_of_milk",   # a goat milked into a clay bowl
	"coal",     # mined from coal ore: the coke oven (ADR 0117)
	"oak_sapling", "brown_mushroom", "blueberry"]   # leaves' drops, wild mushrooms and berry bushes: replanting and meals (ADR 0121)
const CHAPTERS := ["flint_age", "middle_stone_age", "new_stone_age", "copper_stone_age", "copper_age", "bronze_age", "iron_age", "steam_age", "electrical_age", "lv_workshop"]

var items: ItemRegistry
var recipes: RecipeRegistry
var book: QuestBook
var p: PlayerInventory


func before_test() -> void:
	items = ItemRegistry.load_default()
	recipes = RecipeRegistry.load_default(items)
	book = QuestBook.load_default(items)
	p = PlayerInventory.new(items, recipes)
	p.changed.connect(func() -> void: book.check_inventory(p.inv, p.cursor))


func _give(id: String, n: int = 1) -> void:
	assert_int(p.pickup(id, n)).override_failure_message("no room for " + id).is_equal(0)


func test_data_loads_clean() -> void:
	assert_array(book.errors).is_empty()
	assert_array(book.chapters.map(func(c: Dictionary) -> String: return c["id"])).is_equal(CHAPTERS)
	var finals := []
	for c in CHAPTERS:
		for q in book.chapter_quests(c):
			if q.get("final", false):
				finals.append(q["id"])
	assert_array(finals).is_equal(["tar_haft", "bow", "polished_axe", "copper_tool", "saw", "bronze_tool", "iron_tool", "ore_train", "first_machine", "canned_food"])


func test_bad_data_is_reported() -> void:
	var b := QuestBook.new(items)
	b.load_json(JSON.stringify({"chapters": [{"id": "c", "title": "C", "icon": "flint", "quests": [
		{"id": "a", "title": "A", "icon": "nope", "pos": [0, 0], "deps": ["b"], "tasks": [{"type": "item", "item": "nope"}]},
		{"id": "b", "title": "B", "icon": "flint", "pos": [1, 0], "deps": ["a"], "tasks": [{"type": "event", "event": "boom", "item": "#no_tag"}]}]}]}))
	var text := "\n".join(b.errors)
	for want in ["icon nope", "unknown item nope", "unknown event boom", "no item has tag #no_tag", "cycle"]:
		assert_str(text).contains(want)


## Every task's items can be made from what lies on the ground (ADR 0023),
## what animals drop and what a pan or a charcoal pit gives (ADR 0030), and,
## once a copper pickaxe is made, what it mines out of rock (the Copper Age's anvil, ADR 0042).
func test_every_task_is_reachable_from_ground_pickups() -> void:
	var have := recipes.reachable_from(GROUND)
	assert_bool(have.has("copper_pickaxe")).is_true()
	var start := GROUND.duplicate()
	start.append_array(StoneAge.WORLD_SOURCES.keys())
	var mined := MiningTable.load_default().drops("stone", items.get_def("copper_pickaxe"))
	assert_str(String(mined.get("id", ""))).is_equal("cobblestone")
	start.append(mined["id"])
	have = recipes.reachable_from(start)
	for g in start:
		have[g] = true
	for id in book.quests:
		for t in book.quests[id]["tasks"]:
			var ok := false
			for spec in QuestBook.task_specs(t):
				for h in have:
					if items.matches(h, spec):
						ok = true
			assert_bool(ok).override_failure_message("%s: %s not reachable" % [id, QuestBook.task_specs(t)]).is_true()


## The main road leads to its chapter's final quest and the round side quests do not:
## a quest is marked optional exactly when its chapter's final quest does not need it
## (deps into an earlier chapter end the road there).
func test_optional_marks_the_side_roads() -> void:
	for c in book.chapters:
		var needed := {}
		var stack := []
		for q in c["quests"]:
			if q.get("final", false):
				stack.append(q["id"])
		assert_int(stack.size()).override_failure_message("%s: one final quest" % c["id"]).is_equal(1)
		while not stack.is_empty():
			var id: String = stack.pop_back()
			if needed.has(id) or book.quests[id]["chapter"] != c["id"]:
				continue
			needed[id] = true
			stack.append_array(book.quests[id].get("deps", []))
		for q in c["quests"]:
			var id: String = q["id"]
			assert_bool(q.get("optional", false)).override_failure_message(
				"%s: optional should be %s" % [id, not needed.has(id)]).is_equal(not needed.has(id))
		if c["id"] == "flint_age":
			assert_int(needed.size()).is_equal(11)


## No two quests share a spot on the page, and no dependency line runs
## through a third quest's node (the book stays readable).
func test_layout_has_no_overlaps_or_lines_through_nodes() -> void:
	for c in CHAPTERS:
		_check_layout(c)


func _check_layout(chapter: String) -> void:
	var qs := book.chapter_quests(chapter)
	var spots := {}
	for q in qs:
		var k := "%s,%s" % [q["pos"][0], q["pos"][1]]
		assert_bool(spots.has(k)).override_failure_message("%s and %s share %s" % [q["id"], spots.get(k), k]).is_false()
		spots[k] = q["id"]
	for q in qs:
		var b := Vector2(q["pos"][0], q["pos"][1])
		for d in q.get("deps", []):
			var a := Vector2(book.quests[d]["pos"][0], book.quests[d]["pos"][1])
			if book.quests[d]["chapter"] != chapter:
				continue   # a road from an earlier chapter is not drawn on this page
			for o in qs:
				if o["id"] == q["id"] or o["id"] == d:
					continue
				var c := Vector2(o["pos"][0], o["pos"][1])
				var t := clampf((c - a).dot(b - a) / (b - a).length_squared(), 0.0, 1.0)
				assert_float(c.distance_to(a + (b - a) * t)).override_failure_message(
					"%s -> %s runs through %s" % [d, q["id"], o["id"]]).is_greater(0.4)


func test_item_task_latches_and_waits_for_deps() -> void:
	_give("flint_knife_blade")        # knapping's task, but gather_flint is not done
	assert_bool(book.is_done("knapping")).is_false()
	assert_bool(book.task_done("knapping", 0)).is_true()
	p.inv.remove("flint_knife_blade", 1)
	_give("flint", 3)                 # gather_flint completes and knapping follows: the blade still counts
	assert_bool(book.is_done("gather_flint")).is_true()
	assert_bool(book.is_done("knapping")).is_true()


func test_counts_add_up_across_stacks_and_tags() -> void:
	_give("stick", 4)
	_give("loose_stone", 2)
	assert_bool(book.is_done("sticks_stones")).is_false()
	assert_int(book.task_progress("sticks_stones", 0)).is_equal(4)
	_give("stick", 2)
	assert_bool(book.is_done("sticks_stones")).is_true()


func test_any_mode_and_events() -> void:
	_give("copper_nugget", 8)
	assert_bool(book.is_done("surface_copper")).is_true()
	book.note("panned", "flint")      # panning counts nuggets only
	book.note("panned", "tin_nugget", 3)
	assert_int(book.task_progress("panning", 1)).is_equal(3)
	assert_bool(book.task_done("pit_kiln", 1)).is_false()
	book.note("fired", "crucible")    # not a mold
	assert_bool(book.task_done("pit_kiln", 1)).is_false()
	book.note("fired", "pickaxe_mold")
	assert_bool(book.task_done("pit_kiln", 1)).is_true()


func test_completion_signals_once_and_save_round_trip() -> void:
	var got := []
	book.quest_completed.connect(func(id: String) -> void: got.append(id))
	_give("flint", 3)
	_give("flint", 1)
	assert_array(got).is_equal(["gather_flint"])
	_give("stick", 2)                 # partial progress saves too
	var d := book.to_dict()
	var b2 := QuestBook.load_default(items)
	b2.load_dict(JSON.parse_string(JSON.stringify(d)))
	assert_bool(b2.is_done("gather_flint")).is_true()
	assert_int(b2.task_progress("sticks_stones", 0)).is_equal(2)
	assert_bool(b2.dirty).is_false()
	b2.reset()
	assert_bool(b2.is_done("gather_flint")).is_false()


## The main road of the four Stone Age chapters in play order, through the inventory: each step
## holds what the player would, spends what they would, and the book ends at copper with only the
## optional side quests left.
func test_flint_age_walkthrough() -> void:
	var steps := [
		["flint", 6], ["stick", 12], ["loose_stone", 2],
		["flint_knife_blade", 1], ["cordage", 3], ["-cordage", 1], ["-flint_knife_blade", 1], ["flint_knife", 1],
		["-cordage", 1], ["flint_shovel", 1],
		["dry_grass", 9], ["firestarter", 1], ["!assembled", "firepit"], ["birch_tar", 2], ["-birch_tar", 1], ["!tar_hafted", "flint_axe"],
		["microlith", 8], ["-microlith", 2], ["bow", 1], ["arrow", 8],
		["clay_ball", 8], ["-clay_ball", 8], ["clay_bowl", 3], ["cooking_pot", 1], ["polished_stone_axe", 1],
		["copper_awl", 1], ["!fired", "copper_nugget"], ["clay_ball", 8], ["-clay_ball", 8],
		["raw_pickaxe_mold", 1], ["raw_crucible", 1], ["-raw_pickaxe_mold", 1], ["-raw_crucible", 1],
		["!fired", "pickaxe_mold"], ["!fired", "crucible"], ["crucible", 1],
		["loose_copper_ore", 4], ["-loose_copper_ore", 4], ["-crucible", 1], ["crucible_copper", 1],
		["-crucible_copper", 1], ["crucible", 1], ["copper_pickaxe_head", 1], ["-copper_pickaxe_head", 1], ["copper_pickaxe", 1],
	]
	for s in steps:
		var what: String = s[0]
		if what.begins_with("!"):
			book.note(what.substr(1), s[1])
		elif what.begins_with("-"):
			p.inv.remove(what.substr(1), s[1])
		else:
			_give(what, s[1])
	assert_bool(book.is_done("copper_tool")).is_true()
	for c in CHAPTERS.slice(0, 4):
		for q in book.chapter_quests(c):
			var id: String = q["id"]
			assert_bool(book.is_done(id)).override_failure_message("%s: done should be %s" % [id, not q.get("optional", false)]).is_equal(
				not q.get("optional", false))
	assert_bool(book.is_done("ingot")).is_false()   # the Copper Age opens after the Flint Age


## The Copper Age (ADR 0042) after it: ingots, a hammer, an anvil, hot metal, a smithed head, the saw.
func test_copper_age_walkthrough() -> void:
	for c in CHAPTERS.slice(0, 4):
		for q in book.chapter_quests(c):
			book.complete(q["id"])
	var steps := [["copper_ingot", 2], ["-copper_ingot", 1], ["copper_hammer", 1], ["stone_anvil", 1],
		["!heated", "copper_ingot"], ["!smithed", "copper_saw_blade"], ["copper_saw", 1]]
	for s in steps:
		var what: String = s[0]
		if what.begins_with("!"):
			book.note(what.substr(1), s[1])
		elif what.begins_with("-"):
			p.inv.remove(what.substr(1), s[1])
		else:
			_give(what, s[1])
	for q in book.chapter_quests("copper_age").filter(func(x: Dictionary) -> bool: return not x.get("optional", false)):   # side quests ask for more
		assert_bool(book.is_done(q["id"])).override_failure_message(q["id"]).is_true()


## Bronze then iron (ADR 0070): tin, molten bronze, a cast head, the pickaxe; ore, bloomery, bloom,
## forge, a bronze anvil, the smithed bar and the iron pickaxe close both chapters.
func test_bronze_and_iron_age_walkthrough() -> void:
	for c in CHAPTERS.slice(0, 5):
		for q in book.chapter_quests(c):
			book.complete(q["id"])
	for id in ["tin_nugget", "tin_nugget", "crucible_bronze", "bronze_pickaxe_head", "bronze_ingot", "bronze_ingot",
			"bronze_pickaxe", "bronze_anvil"]:
		_give(id)
	for q in book.chapter_quests("bronze_age").filter(func(x: Dictionary) -> bool: return not x.get("optional", false)):   # side quests ask for more
		assert_bool(book.is_done(q["id"])).override_failure_message(q["id"]).is_true()
	for id in ["loose_iron_ore", "loose_iron_ore", "loose_iron_ore", "loose_iron_ore", "bloomery", "iron_bloom", "forge"]:
		_give(id)
	assert_bool(book.is_done("wrought_iron")).is_false()
	book.note("smithed", "iron_ingot")
	_give("iron_pickaxe")
	for q in book.chapter_quests("iron_age").filter(func(x: Dictionary) -> bool: return not x.get("optional", false)):   # side quests ask for more
		assert_bool(book.is_done(q["id"])).override_failure_message(q["id"]).is_true()


## A real pit kiln firing a raw mold reaches the book through GroundEntity.fired
## (WorkSystem connects it the same way in play).
func test_pit_kiln_fired_signal_counts_for_the_book() -> void:
	var store := BlockEntities.new(items, recipes)
	var g := store.create(Vector3i(0, 0, 0), "ground") as GroundEntity
	g.item = "raw_pickaxe_mold"
	g.fired.connect(func(id: String) -> void: book.note("fired", id))
	for i in GroundEntity.KILN_GRASS:
		g.use(items.get_def("dry_grass"))
	for i in GroundEntity.KILN_STICKS:
		g.use(items.get_def("stick"))
	g.use(items.get_def("firestarter"))
	for i in int(Settings.pit_kiln_burn_s * 20.0):
		store.tick()
	assert_str(g.item).is_equal("pickaxe_mold")
	assert_bool(book.task_done("pit_kiln", 1)).is_true()


## The steam age ends on the ore train; the steel pickaxe is faster than iron; a mined ore block melts to one ingot (ADR 0139).
func test_steam_age_walkthrough_and_ore_counts() -> void:
	for c in CHAPTERS.slice(0, 7):
		for q in book.chapter_quests(c):
			book.complete(q["id"])
	for id in ["small_boiler", "fire_brick", "blast_furnace", "steel_ingot", "steel_pickaxe", "ore_crusher", "steam_turbine", "minecart", "hopper", "rail"]:
		_give(id)
	assert_bool(book.is_done("ore_train")).is_true()
	var mining := MiningTable.load_default()
	assert_float(mining.break_time("copper_ore", items.get_def("steel_pickaxe"))).is_less(mining.break_time("copper_ore", items.get_def("iron_pickaxe")))
	assert_int(int(recipes.first_of("melt", {"id": "melt_copper_ore"})["count"])).is_equal(1)
