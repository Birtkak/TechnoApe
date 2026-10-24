extends GdUnitTestSuite

## PrefabStore (ADR 0038): prefab files by material name, run-length coded, trimmed to what they hold.

const BASE := "user://test_prefabs"


func after() -> void:
	_remove(BASE)


func _remove(path: String) -> void:
	var d := DirAccess.open(path)
	if d == null:
		return
	for f in d.get_files():
		d.remove(f)
	for sub in d.get_directories():
		_remove(path.path_join(sub))
	DirAccess.remove_absolute(ProjectSettings.globalize_path(path))


func _grid() -> PackedByteArray:
	var g := PackedByteArray()
	g.resize(4 * 3 * 2)
	for i in g.size():
		g[i] = GameMaterials.WOOD if i % 3 == 0 else (GameMaterials.STONE if i % 5 == 0 else 0)
	return g


func test_encode_and_decode_round_trip_by_name() -> void:
	var e := PrefabStore.encode(Vector3i(4, 3, 2), _grid())
	assert_str(e["palette"][0]).is_equal("")
	assert_bool((e["palette"] as Array).has("wood")).is_true()
	var d := PrefabStore.decode(JSON.parse_string(JSON.stringify(e)))
	assert_that(d["size"]).is_equal(Vector3i(4, 3, 2))
	assert_that(d["grid"]).is_equal(_grid())


func test_a_material_that_is_gone_loads_as_nothing() -> void:
	var d := PrefabStore.decode({"size": [2, 1, 1], "palette": ["", "no_such_material", "wood"], "runs": [[1, 1], [2, 1]]})
	assert_that(d["grid"]).is_equal(PackedByteArray([0, GameMaterials.WOOD]))
	assert_bool(PrefabStore.decode({"size": [3, 1, 1], "palette": [""], "runs": [[0, 2]]}).is_empty()).is_true()


func test_save_list_and_load() -> void:
	var store := PrefabStore.new(BASE)
	assert_str(store.save("houses", "my hut", Vector3i(4, 3, 2), _grid())).is_not_empty()
	store.save_last(Vector3i(4, 3, 2), _grid())
	var all := store.list()
	assert_that(all["houses"]).is_equal(["my hut"])
	assert_that(all[PrefabStore.LAST_CATEGORY]).is_equal([PrefabStore.LAST_NAME])
	assert_that(store.load_prefab("houses", "my hut")["grid"]).is_equal(_grid())
	assert_bool(store.load_prefab("houses", "nope").is_empty()).is_true()


func test_trim_keeps_the_box_around_the_build() -> void:
	var g := PackedByteArray()
	g.resize(5 * 5 * 5)
	g[2 + 5 * (1 + 5 * 3)] = GameMaterials.WOOD   # x 2, z 1, y 3
	g[3 + 5 * (2 + 5 * 3)] = GameMaterials.STONE  # x 3, z 2, y 3
	var t: Dictionary = ShapeTool.new().trim(Vector3i(5, 5, 5), g)
	assert_that(t["size"]).is_equal(Vector3i(2, 1, 2))
	assert_that(t["offset"]).is_equal(Vector3i(2, 3, 1))
	assert_that(t["grid"]).is_equal(PackedByteArray([GameMaterials.WOOD, 0, 0, GameMaterials.STONE]))
