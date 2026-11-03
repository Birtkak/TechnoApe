extends GdUnitTestSuite

## More of the electrical age (ADR 0131): six LV machines as data (an item, process recipes, a
## MachineEntity.DOOR entry), each built from circuits and a hull, each running a recipe on the cables.

const B := Settings.BLOCK
const MACHINES := ["extractor", "cutter", "extruder", "canning_machine", "electric_compressor", "electrolyzer"]

var items: ItemRegistry
var recipes: RecipeRegistry
var store: BlockEntities


func before_test() -> void:
	items = ItemRegistry.load_default()
	recipes = RecipeRegistry.load_default(items)
	Inventory.clock_s = 1000.0


func after_test() -> void:
	Inventory.clock_s = 0.0


## test_electrical_age's line: a lit boiler, pipe, turbine and cable; the machine goes at 10B.
func _powered_line() -> BoilerEntity:
	store = BlockEntities.new(items, recipes)
	var b: BoilerEntity = store.create(Vector3i.ZERO, "boiler")
	b.use({"id": "coal", "count": 8}, items.get_def("coal"), false, true)
	for i in 4:
		b.use({"id": "water_bucket", "count": 1}, items.get_def("water_bucket"))
	assert_bool(b.ignite()).is_true()
	store.create(Vector3i(5 * B, 0, 0), "pipe")
	store.create(Vector3i(6 * B, 0, 0), "turbine")
	store.create(Vector3i(9 * B, 0, 0), "cable")
	return b


func _ticks(n: int, b: BoilerEntity) -> void:
	for i in n:
		Inventory.clock_s += BlockEntities.TICK_S
		if b.pressure > 1.0:
			b.pressure = 1.0
		store.tick()


func test_machines_are_data() -> void:
	for m in MACHINES:
		var def := items.get_def(m)
		assert_str(String(def.get("work", ""))).override_failure_message(m).is_equal(m)
		assert_bool(def.has("machine")).override_failure_message(m).is_true()
		assert_bool(MachineEntity.DOOR.has(m)).override_failure_message(m + " has no DOOR entry").is_true()
		assert_int(BlockEntities.width_of(m)).is_equal(BlockEntities.width_of("lathe"))
		var needs: Array = recipes.ingredient_specs(recipes.recipes[m])
		assert_bool("primitive_circuit" in needs and "machine_hull" in needs).override_failure_message(m).is_true()
		assert_int(recipes.by_station(m).size()).override_failure_message(m + " runs nothing").is_greater(0)
	# canned food is the food that keeps
	for id in ["canned_meat", "canned_fish", "canned_vegetables"]:
		assert_bool(items.get_def(id).has("food")).is_true()
		assert_bool(items.get_def(id).has("spoil_days")).override_failure_message(id + " spoils").is_false()


## One recipe per machine, loaded by hand, run on the cables: the output comes out.
func test_each_machine_runs_on_power() -> void:
	var runs := {"extract_rubber": {"sticky_resin": 1}, "cut_oak_planks": {"oak_log": 0}, "extrude_tin_can": {"tin_ingot": 1},
		"can_meat": {"tin_can": 1, "cooked_meat": 2}, "compress_coke_electrode": {"coal_coke": 2},
		"electrolyze_copper": {"crushed_copper_ore": 2, "carbon_electrode": 1}, "assemble_insulated_wire": {"copper_wire": 3, "rubber": 1}}
	for rid in runs:
		var r: Dictionary = recipes.recipes[rid]
		var b := _powered_line()
		var m: MachineEntity = store.create(Vector3i(10 * B, 0, 0), String(r["station"]))
		for id in runs[rid]:
			var n: int = runs[rid][id] if runs[rid][id] > 0 else items.units(id)   # 0: a block of a voxel item
			assert_int(int(m.use({"id": id, "count": n})["consume"])).override_failure_message("%s takes no %s" % [rid, id]).is_greater(0)
		assert_str(String(m.current().get("id", ""))).override_failure_message(rid).is_equal(rid)
		_ticks(400 + int(float(r["seconds"]) / BlockEntities.TICK_S) + 100, b)
		var out: Array = m.use({})["give"]
		assert_int(out.size()).override_failure_message(rid + " made nothing").is_equal(1)
		assert_str(String(out[0]["id"])).is_equal(String(r["output"]["id"]))
		var id: String = r["output"]["id"]
		assert_int(int(out[0]["count"])).is_equal(int(r["output"]["count"]) * items.units(id))   # a voxel item counts voxels
