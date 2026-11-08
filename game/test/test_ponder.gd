extends GdUnitTestSuite

## The guide scenes (ADR 0132): every file loads and names only real pieces, items, materials and
## fields; the rules the "connecting" scene teaches hold on the real solver.

const B := Settings.BLOCK

var items: ItemRegistry
var recipes: RecipeRegistry


func before_test() -> void:
	items = ItemRegistry.load_default()
	recipes = RecipeRegistry.load_default(items)
	Ponder.items = items
	Ponder.recipes = recipes


func test_every_scene_is_sound() -> void:
	var scenes := Ponder.scenes()
	assert_bool(scenes.size() >= 8).override_failure_message("scenes: %s" % str(scenes.keys())).is_true()
	for id in ["steam", "electricity", "connecting"]:
		assert_bool(scenes.has(id)).override_failure_message("missing scene " + id).is_true()
	for id in scenes:
		var bad := Ponder.check(scenes[id], items, recipes)
		assert_array(Array(bad)).override_failure_message("%s: %s" % [id, ", ".join(bad)]).is_empty()


## Every machine with process recipes has its own sound guide: what it is, power, and IN / OUT with each recipe.
func test_every_machine_has_its_own_guide() -> void:
	var n := 0
	for id in items.all_ids():
		var d := items.get_def(id)
		if not d.has("machine") or Ponder.process_recipes(String(d.get("work", ""))).is_empty():
			continue
		n += 1
		assert_str(Ponder.for_item(id)).is_equal(Ponder.MACHINE + id)
		var sc := Ponder.get_scene(Ponder.for_item(id))
		var bad := Ponder.check(sc, items, recipes)
		assert_array(Array(bad)).override_failure_message("%s: %s" % [id, ", ".join(bad)]).is_empty()
		assert_array(sc["recipes"]).is_not_empty()
		var labels := []
		for st in sc["steps"]:
			for f in st.get("flow", []):
				labels.append(String(f.get("label", "")).get_slice(" ", 0))
		assert_array(labels).override_failure_message(id + " " + str(labels)).contains(["IN", "OUT"])
	assert_int(n).is_greater_equal(12)
	assert_str(Ponder.recipe_line(recipes.recipes["lathe_iron_rod"])).is_equal("Iron Ingot  ->  2 Iron Rod (15 s)")


func test_items_lead_to_their_guide() -> void:
	assert_str(Ponder.for_item("small_boiler")).is_equal("steam")
	assert_str(Ponder.for_item("steam_turbine")).is_equal("electricity")
	assert_str(Ponder.for_item("copper_cable")).is_equal("connecting")
	assert_str(Ponder.for_item("bronze_pipe")).is_equal("connecting")
	assert_str(Ponder.for_item("flint")).is_equal("")
	# every work item has a guide, so the tooltip's hint and P always lead somewhere
	var missing := []
	for id in items.all_ids():
		if items.get_def(id).has("work") and Ponder.for_item(id) == "" and not id in ["firepit", "sieve", "firewood", "stone_anvil", "bronze_anvil", "forge", "bloomery", "drying_rack", "loom", "rail", "smokehouse", "salt_pan", "snare", "trading_post"]:
			missing.append(id)
	assert_array(missing).override_failure_message("work items without a guide: %s" % str(missing)).is_empty()


## What "connecting" teaches, on the solver: face to face joins, a corner joins, a diagonal does not, a
## voxel up does not, a pipe on a step beside a tall boiler does (its upper blocks).
func test_connecting_rules_hold() -> void:
	var s := NetworkSolver.new()
	var C := NetworkSolver.CONDUCTOR
	var box := func(x: int, y: int, z: int, w: int = B, h: int = B) -> PackedInt32Array: return PackedInt32Array([x, y, z, w, h, w])
	# source at 0, pipe at B (face), pipe at (2B, z B) (corner), pipe at (3B, z 2B) (diagonal), pipe a voxel up at 3B
	var boxes: PackedInt32Array = box.call(0, 0, 0) + box.call(B, 0, 0) + box.call(2 * B, 0, 0) + box.call(2 * B, 0, B) + box.call(3 * B, 0, 2 * B) + box.call(3 * B, 1, 0)
	var roles := PackedInt32Array([NetworkSolver.SOURCE, C, C, C, NetworkSolver.SINK, NetworkSolver.SINK])
	var r := s.solve(boxes, roles, PackedFloat32Array([1, 1, 1, 1, 1, 1]))
	assert_float(r[0]).is_equal(r[2])      # face to face
	assert_float(r[0]).is_equal(r[6])      # round the corner
	assert_float(r[9]).is_equal(0.0)       # the diagonal gets nothing
	assert_float(r[11]).is_equal(0.0)      # a voxel higher gets nothing
	# a boiler (5 blocks tall) with a sink a block up on a step beside it
	var boxes2: PackedInt32Array = PackedInt32Array([0, 0, 0, 5 * B, 5 * B, 5 * B]) + box.call(5 * B, B, 2 * B)
	var r2 := s.solve(boxes2, PackedInt32Array([NetworkSolver.SOURCE, NetworkSolver.SINK]), PackedFloat32Array([1, 1]))
	assert_float(r2[3]).is_equal(1.0)
