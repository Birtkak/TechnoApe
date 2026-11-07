extends GdUnitTestSuite

## Blueprints (ADR 0161): the layout turns entity anchors and facings with the voxels; the bill and the ghost
## grid count the voxels and the pieces; the data survives JSON (it rides on a stack and in the world save).
## The world side (copy, mark, build by a builder, compare) is the scripted run blueprint_test=1.

var items: ItemRegistry


func before_test() -> void:
	items = ItemRegistry.load_default()


## A 4 x 1 x 2 blueprint: planks at local (3, 0, 1), a hopper (facing 0 = -z) in a one-cell box at (3, 0, 0),
## a cable along x at (0, 0, 0).
func _data() -> Dictionary:
	var g := PackedByteArray()
	g.resize(8)
	g[3 + 4 * 1] = GameMaterials.PLANKS
	var d := PrefabStore.encode(Vector3i(4, 1, 2), g)
	d["entities"] = [
		{"kind": "hopper", "item": "hopper", "mat": "work_spot", "off": [3, 0, 0], "box": [1, 1, 1], "facing": 0},
		{"kind": "cable", "item": "copper_cable", "sub": "copper_cable", "mat": "work_spot", "off": [0, 0, 0], "box": [1, 1, 1], "axis": 0}]
	return JSON.parse_string(JSON.stringify(d))


func _cell(lay: Dictionary, k: int) -> Vector3i:
	var c: PackedInt32Array = lay["cells"]
	return Vector3i(c[3 * k], c[3 * k + 1], c[3 * k + 2])


func test_layout_turns_pieces_with_the_voxels() -> void:
	var st := ShapeTool.new()
	var rot := 0
	for q in 4:
		var lay := Blueprint.layout(_data(), Vector3i(100, 10, 100), rot, false)
		assert_int((lay["mats"] as PackedByteArray).size()).is_equal(1)
		var planks := _cell(lay, 0)
		var hop: Dictionary = lay["entities"][0]
		# the hopper faced away from the planks; it still does after any turn
		assert_that(Transport.DIRS[int(hop["facing"])]).is_equal(hop["anchor"] - planks)
		var cable: Dictionary = lay["entities"][1]
		assert_int(int(cable["axis"])).is_equal(q % 2)
		assert_str(String(cable["sub"])).is_equal("copper_cable")
		rot = st.turn(rot, 1)


func test_mirror_keeps_facing_toward_z() -> void:
	var lay := Blueprint.layout(_data(), Vector3i.ZERO, 0, true)
	var hop: Dictionary = lay["entities"][0]
	assert_int(int(hop["facing"])).is_equal(0)
	assert_that(Transport.DIRS[int(hop["facing"])]).is_equal(hop["anchor"] - _cell(lay, 0))


func test_bill_and_ghost() -> void:
	var d := _data()
	var bill := Blueprint.bill(d, items)
	assert_int(int(bill.get(items.item_for_material("planks"), 0))).is_equal(1)
	assert_int(int(bill.get("hopper", 0))).is_equal(1)
	assert_int(int(bill.get("copper_cable", 0))).is_equal(1)
	var g := Blueprint.ghost_grid(d)
	assert_int(g.size()).is_equal(8)
	assert_int(g[3]).is_equal(GameMaterials.id_of("work_spot"))   # the hopper's box shows in its material
	assert_str(Blueprint.describe(d, items)).contains("Hopper 1")


func test_items_and_recipes_exist() -> void:
	assert_int(int(items.get_def("written_blueprint").get("stack", 0))).is_equal(1)   # each one holds its own build
	assert_str(String(items.get_def("builder").get("work", ""))).is_equal("builder")
	var recipes := RecipeRegistry.load_default(items)
	assert_bool(recipes.recipes.has("blueprint")).is_true()
	assert_bool(recipes.recipes.has("builder")).is_true()
