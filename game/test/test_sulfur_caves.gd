extends GdUnitTestSuite

## Sulfur caves (biomes.json sulfur_cave) and the Noita chemistry around them:
## water dilutes toxic sludge into water, water quenches lava into obsidian.

const S := preload("res://scripts/settings.gd")
const N := 3


func _sim() -> SimWorld:
	var w := SimWorld.new()
	w.configure(Vector3i(N, N, N), 2, 5)
	w.set_materials(GameMaterials.defs())
	w.set_reactions(GameMaterials.reactions())
	w.mark_all_loaded(true)
	w.fill_box(Vector3i(0, 0, 0), Vector3i(48, 1, 48), GameMaterials.STONE)
	# a basin so nothing runs off
	w.fill_box(Vector3i(14, 1, 14), Vector3i(26, 8, 26), GameMaterials.STONE)
	w.fill_box(Vector3i(15, 1, 15), Vector3i(25, 8, 25), 0)
	return w


func _run(w: SimWorld, ticks: int) -> void:
	for i in ticks:
		w.tick(1)


func test_water_turns_sludge_into_water() -> void:
	var w := _sim()
	var basin_min := Vector3i(15, 1, 15)
	var basin_max := Vector3i(25, 8, 25)
	w.fill_box(Vector3i(15, 1, 15), Vector3i(25, 3, 25), GameMaterials.TOXIC_SLUDGE)   # 200 cells
	_run(w, 5)
	w.fill_box(Vector3i(15, 3, 15), Vector3i(25, 5, 25), GameMaterials.WATER)          # poured on top
	_run(w, 600)
	var sludge := w.count_in_box(basin_min, basin_max, GameMaterials.TOXIC_SLUDGE)
	var water := w.count_in_box(basin_min, basin_max, GameMaterials.WATER)
	assert_int(sludge).override_failure_message("%d sludge left" % sludge).is_less(20)
	assert_int(water).is_greater(360)


func test_static_sludge_pools_react_too() -> void:
	var w := _sim()
	w.fill_box(Vector3i(15, 1, 15), Vector3i(25, 2, 25), GameMaterials.TOXIC_SLUDGE_STATIC)
	_run(w, 3)
	w.fill_box(Vector3i(15, 2, 15), Vector3i(25, 4, 25), GameMaterials.WATER)
	_run(w, 600)
	assert_int(w.count_in_box(Vector3i(15, 1, 15), Vector3i(25, 8, 25), GameMaterials.TOXIC_SLUDGE_STATIC)).is_less(10)


func test_water_on_lava_makes_obsidian() -> void:
	var w := _sim()
	w.fill_box(Vector3i(15, 1, 15), Vector3i(25, 2, 25), GameMaterials.LAVA)
	_run(w, 3)
	w.fill_box(Vector3i(15, 3, 15), Vector3i(25, 4, 25), GameMaterials.WATER)
	_run(w, 200)
	var obsidian := w.count_in_box(Vector3i(15, 1, 15), Vector3i(25, 8, 25), GameMaterials.OBSIDIAN)
	assert_int(obsidian).is_greater(20)
	assert_int(w.count_in_box(Vector3i(15, 1, 15), Vector3i(25, 8, 25), GameMaterials.STONE)).is_equal(0)   # obsidian, not stone


func test_sulfur_cave_generates_near_spawn() -> void:
	var g := FineMacro.new()
	var p: Vector3i = g.find_nearest("sulfur_cave", 0, 0, 4096)
	assert_int(p.y).override_failure_message("no sulfur cave within 2 km").is_greater(0)
	var size := Vector3i(48, 24, 48)
	var bytes: PackedByteArray = g.generate_block(p - Vector3i(24, 8, 24), size, 0, 2)
	var sulfur := 0
	var sludge := 0
	for i in range(0, bytes.size(), 2):
		var m := bytes.decode_u16(i)
		if m == GameMaterials.SULFUR:
			sulfur += 1
		elif m == GameMaterials.TOXIC_SLUDGE_STATIC:
			sludge += 1
	assert_int(sulfur).is_greater(200)
	assert_int(sludge).is_greater(20)


func test_new_drops_and_bucket() -> void:
	var mining := MiningTable.load_default()
	var items := ItemRegistry.load_default()
	assert_str(String(mining.entry("sulfur").get("drops", ""))).is_equal("sulfur")
	assert_str(String(mining.entry("obsidian").get("drops", ""))).is_equal("obsidian")
	assert_bool(items.has("sulfur") and items.has("obsidian") and items.has("toxic_sludge_bucket")).is_true()
	assert_str(String(items.get_def("bucket")["fills"]["toxic_sludge"])).is_equal("toxic_sludge_bucket")
