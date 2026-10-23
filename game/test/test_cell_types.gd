extends GdUnitTestSuite

## The four kinds of matter (plus fire): solids stay, liquids spread flat,
## gases rise, powders (sand, gravel, dirt, snow, ash) fall and pile up.

const N := 3
const TYPES := ["solid", "liquid", "gas", "powder", "fire"]


func _world(defs: Array) -> SimWorld:
	var w := SimWorld.new()
	w.configure(Vector3i(N, N, N), 2, 5)
	w.set_materials(defs)
	w.set_reactions(GameMaterials.reactions())
	w.mark_all_loaded(true)
	w.fill_box(Vector3i(0, 0, 0), Vector3i(48, 1, 48), GameMaterials.STONE)
	return w


func test_every_material_has_one_of_the_types() -> void:
	for d in GameMaterials.defs():
		assert_bool(String(d.get("cell_type", "")) in TYPES).override_failure_message("%s: cell_type %s" % [d["name"], d.get("cell_type")]).is_true()
		assert_bool(d.has("liquid_sand")).override_failure_message("%s still says liquid_sand; use cell_type powder" % d["name"]).is_false()
	for m in [GameMaterials.SAND, GameMaterials.GRAVEL, GameMaterials.DIRT, GameMaterials.SNOW]:
		assert_str(String(GameMaterials.defs()[m - 1]["cell_type"])).is_equal("powder")
	for m in [GameMaterials.WATER, GameMaterials.LAVA, GameMaterials.OIL]:
		assert_str(String(GameMaterials.defs()[m - 1]["cell_type"])).is_equal("liquid")
	assert_int(SimWorld.CELL_POWDER).is_equal(4)


## Pours a 1x12x1 column of sand and one of water on a flat floor.
func _pour(w: SimWorld) -> void:
	w.fill_box(Vector3i(12, 1, 24), Vector3i(13, 13, 25), GameMaterials.SAND)
	w.fill_box(Vector3i(36, 1, 24), Vector3i(37, 13, 25), GameMaterials.WATER)
	for i in 300:
		w.tick(1)


func test_sand_piles_and_water_spreads() -> void:
	var w := _world(GameMaterials.defs())
	_pour(w)
	assert_int(w.count_in_box(Vector3i(0, 1, 0), Vector3i(24, 48, 48), GameMaterials.SAND)).is_equal(12)   # none lost
	assert_int(w.count_in_box(Vector3i(10, 2, 22), Vector3i(15, 6, 27), GameMaterials.SAND)).override_failure_message("sand spread flat like a liquid").is_greater(0)
	assert_int(w.count_in_box(Vector3i(24, 2, 0), Vector3i(48, 48, 48), GameMaterials.WATER)).override_failure_message("water kept a heap").is_equal(0)


func test_noita_spelling_still_reads_as_powder() -> void:
	var defs: Array = GameMaterials.defs().duplicate(true)
	var sand: Dictionary = defs[GameMaterials.SAND - 1]
	sand["cell_type"] = "liquid"
	sand["liquid_sand"] = true
	var w := _world(defs)
	_pour(w)
	assert_int(w.count_in_box(Vector3i(10, 2, 22), Vector3i(15, 6, 27), GameMaterials.SAND)).is_greater(0)
