extends GdUnitTestSuite

## Lava lights (TorchLights.note_lava): sampled lava cells become one spot per grid column at the
## top of the pool; other materials are skipped; spots expire.


func test_lava_hits_make_one_spot_per_column_at_the_top() -> void:
	var t := TorchLights.new()
	var hits := PackedInt32Array([
		10, 100, 10, GameMaterials.LAVA, 0, 0, 0,
		11, 104, 11, GameMaterials.LAVA_STATIC, 0, 0, 0,   # same column, higher: wins
		10, 200, 10, GameMaterials.FIRE, 0, 0, 0,          # burning cells come back too: not lava
		500, 100, 500, GameMaterials.LAVA, 0, 0, 0])
	t.note_lava(hits, 0.0)
	assert_int(t._lava.size()).is_equal(2)
	var top: Array = t._lava[Vector3i((Vector3(11.5, 105.0 + Settings.m2v(0.5), 11.5) / Settings.m2v(Settings.lava_light_grid_m)).floor()) * Vector3i(1, 0, 1)]
	assert_float(top[1].y).is_equal(105.0 + Settings.m2v(0.5))
	t.free()


func test_lava_spots_light_until_they_expire() -> void:
	var t := TorchLights.new()
	var stub := GDScript.new()
	stub.source_code = "extends Node\nvar bubble = null\n"   # Main without a sim: no sampling, only the spots noted
	stub.reload()
	t.main = stub.new()
	t.note_lava(PackedInt32Array([10, 100, 10, GameMaterials.LAVA, 0, 0, 0]), 0.0)
	assert_int(t._lava_lights(Vector3.ZERO, 1.0).size()).is_equal(1)
	assert_int(t._lava_lights(Vector3.ZERO, TorchLights.LAVA_TTL_S + 1.0).size()).is_equal(0)
	t.main.free()
	t.free()
