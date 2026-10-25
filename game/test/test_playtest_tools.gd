extends GdUnitTestSuite

## Playtest gadgets (PlaytestTools, /kit gadgets): the kit hands out real gadgets, every gadget's
## kind is one PlaytestTools handles, and bombs name a material that exists. The cutting itself is
## checked in game: later=/kit gadgets;/gadget instant_tree;/laser 2 25 (a felled test tree in pieces).

const KINDS := ["laser", "crater", "bomb", "tree", "measure", "vacuum"]

var items: ItemRegistry


func before() -> void:
	items = ItemRegistry.load_default()


func test_kit_holds_gadgets() -> void:
	var kit: Dictionary = GameCommands.KITS["gadgets"]
	assert_bool(kit.has("laser_cutter")).is_true()
	for id in kit:
		assert_bool(items.has(id)).override_failure_message("kit item %s missing" % id).is_true()
		assert_bool(items.get_def(id).has("playtest")).override_failure_message("%s is no gadget" % id).is_true()


func test_gadget_kinds_and_materials() -> void:
	var n := 0
	for id in items.all_ids():
		var def := items.get_def(id)
		if not def.has("playtest"):
			continue
		n += 1
		var kind := PlaytestTools.kind_of(def)
		assert_bool(kind in KINDS).override_failure_message("%s: unknown kind %s" % [id, kind]).is_true()
		assert_bool(def.get("found_only", false)).override_failure_message("%s should stay out of progression" % id).is_true()
		if kind == "bomb":
			assert_int(GameMaterials.id_of(def["playtest"]["material"])).override_failure_message("%s: no such material" % id).is_greater(0)
	assert_int(n).is_greater_equal(7)
