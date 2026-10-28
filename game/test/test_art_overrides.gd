extends GdUnitTestSuite

## Hand-made art overrides (game/art/README.md): cell models and UI skins.


func test_art_model_parses_layers_and_keeps_live_cells() -> void:
	var m = ArtModels.parse({"size": [2, 2, 1], "layers": [["0*"], [".A"]]})
	assert_object(m).is_not_null()
	var c: PackedInt32Array = m["cells"]
	assert_array(Array(c)).is_equal([1, ArtModels.LIVE, 0, 11])   # x + size.x * (y + size.y * z)
	assert_object(ArtModels.parse({"size": [2, 2, 1], "layers": [["0"], [".A"]]})).is_null()   # short row
	assert_object(ArtModels.parse({"size": [2, 2, 1], "layers": [["0!"], [".A"]]})).is_null()  # not a paint


func test_art_model_cells_fill_live_cells_from_the_game_and_skip_other_sizes() -> void:
	ArtModels._models["test_part"] = ArtModels.parse({"size": [2, 2, 1], "layers": [["0*"], [".z"]]})
	var game := PackedInt32Array([5, 6, 7, 8])
	assert_array(Array(ArtModels.cells("test_part", game, Vector3i(2, 2, 1), 40))).is_equal([1, 6, 0, 0])   # z = 61 is past 40 paints
	assert_array(Array(ArtModels.cells("test_part", game, Vector3i(4, 1, 1), 40))).is_equal([5, 6, 7, 8])
	assert_array(Array(ArtModels.cells("no_such_model", game, Vector3i(2, 2, 1), 40))).is_equal([5, 6, 7, 8])
	ArtModels._models.erase("test_part")


func test_missing_ui_skin_keeps_the_flat_box() -> void:
	var sb := StyleBoxFlat.new()
	assert_object(UiStyle.skin("no_such_skin", sb)).is_same(sb)


class FakePiece:
	extends RefCounted
	var kind := "test_piece"
	var facing := 1


func test_work_model_is_built_facing_zero_and_turned_with_the_piece() -> void:
	# a 2 x 1 x 2 piece: the model paints its (0, 0, 0) corner, "*" keeps the game's (1, 0, 1) corner
	ArtModels._models["work_test_piece"] = ArtModels.parse({"size": [2, 1, 2], "layers": [["0.", ".*"]]})
	var size := Vector3i(2, 1, 2)
	var game := WorkPalette.rotate_y(PackedInt32Array([0, 0, 0, 9]), size, 1)   # the game's cells, already turned
	var parts := WorkPalette.art_parts(FakePiece.new(), {"solid": {"cells": game, "size": size, "grid": 2}})
	assert_array(Array(parts["solid"]["cells"])).is_equal(Array(WorkPalette.rotate_y(PackedInt32Array([1, 0, 0, 9]), size, 1)))
	ArtModels._models.erase("work_test_piece")
