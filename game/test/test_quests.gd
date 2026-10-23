extends GdUnitTestSuite

## Headless tests for the progression book (ADR 0028): quests.json against
## the item list, every task reachable from ground pickups, latching item
## tasks, deps cascading, "any" quests, the pit kiln event, saving, and the
## whole Flint Age walked through a player inventory to a copper tool.

const GROUND := ["flint", "stick", "loose_stone", "loose_copper_ore", "clay_ball", "dry_grass", "oak_log", "gravel", "sand",
	"plant_fiber", "dirt", "raw_hide", "raw_meat", "carrot"]

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
	assert_array(book.chapter_quests("flint_age")).is_not_empty()
	var finals := []
	for q in book.chapter_quests("flint_age"):
		if q.get("final", false):
			finals.append(q["id"])
	assert_array(finals).is_equal(["copper_tool"])


func test_bad_data_is_reported() -> void:
	var b := QuestBook.new(items)
	b.load_json(JSON.stringify({"chapters": [{"id": "c", "title": "C", "icon": "flint", "quests": [
		{"id": "a", "title": "A", "icon": "nope", "pos": [0, 0], "deps": ["b"], "tasks": [{"type": "item", "item": "nope"}]},
		{"id": "b", "title": "B", "icon": "flint", "pos": [1, 0], "deps": ["a"], "tasks": [{"type": "event", "event": "boom", "item": "#no_tag"}]}]}]}))
	var text := "\n".join(b.errors)
	for want in ["icon nope", "unknown item nope", "unknown event boom", "no item has tag #no_tag", "cycle"]:
		assert_str(text).contains(want)


## Every task's items can be made from what lies on the ground (ADR 0023),
## what animals drop and what a pan or a charcoal pit gives (ADR 0030).
func test_every_task_is_reachable_from_ground_pickups() -> void:
	var have := recipes.reachable_from(GROUND)
	for g in GROUND:
		have[g] = true
	for id in book.quests:
		for t in book.quests[id]["tasks"]:
			var ok := false
			for spec in QuestBook.task_specs(t):
				for h in have:
					if items.matches(h, spec):
						ok = true
			assert_bool(ok).override_failure_message("%s: %s not reachable" % [id, QuestBook.task_specs(t)]).is_true()


## The main road leads to the final quest and the round side quests do not:
## a quest is marked optional exactly when copper_tool does not need it.
func test_optional_marks_the_side_roads() -> void:
	var needed := {}
	var stack := ["copper_tool"]
	while not stack.is_empty():
		var id: String = stack.pop_back()
		if needed.has(id):
			continue
		needed[id] = true
		stack.append_array(book.quests[id].get("deps", []))
	for id in book.quests:
		assert_bool(book.quests[id].get("optional", false)).override_failure_message(
			"%s: optional should be %s" % [id, not needed.has(id)]).is_equal(not needed.has(id))
	assert_int(needed.size()).is_equal(17)


## No two quests share a spot on the page, and no dependency line runs
## through a third quest's node (the book stays readable).
func test_layout_has_no_overlaps_or_lines_through_nodes() -> void:
	var qs := book.chapter_quests("flint_age")
	var spots := {}
	for q in qs:
		var k := "%s,%s" % [q["pos"][0], q["pos"][1]]
		assert_bool(spots.has(k)).override_failure_message("%s and %s share %s" % [q["id"], spots.get(k), k]).is_false()
		spots[k] = q["id"]
	for q in qs:
		var b := Vector2(q["pos"][0], q["pos"][1])
		for d in q.get("deps", []):
			var a := Vector2(book.quests[d]["pos"][0], book.quests[d]["pos"][1])
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


## The Flint Age main road in play order, through the inventory: each step
## holds what the player would, spends what they would, and the book ends at
## copper with only the optional side quests left.
func test_flint_age_walkthrough() -> void:
	var steps := [
		["flint", 6], ["stick", 12], ["loose_stone", 2],
		["flint_knife_blade", 1], ["cordage", 3], ["-cordage", 1], ["-flint_knife_blade", 1], ["flint_knife", 1],
		["-cordage", 1], ["flint_shovel", 1],
		["dry_grass", 9], ["firestarter", 1], ["clay_ball", 8], ["-clay_ball", 8],
		["raw_pickaxe_mold", 1], ["raw_crucible", 1], ["-raw_pickaxe_mold", 1], ["-raw_crucible", 1],
		["!fired", "pickaxe_mold"], ["!fired", "crucible"], ["crucible", 1], ["firepit", 1], ["-firepit", 1],
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
	for id in book.quests:
		assert_bool(book.is_done(id)).override_failure_message("%s: done should be %s" % [id, not book.quests[id].get("optional", false)]).is_equal(
			not book.quests[id].get("optional", false))


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
