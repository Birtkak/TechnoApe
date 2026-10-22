extends GdUnitTestSuite

## SimWorld.shift (in place since the bubble polish): every surviving chunk
## lands at old + delta's source, vacated chunks come back as air, in all six
## directions, diagonally and beyond the window.

const N := 3
const PROBES := [Vector3i(0, 0, 0), Vector3i(15, 15, 15), Vector3i(7, 3, 11), Vector3i(15, 0, 0), Vector3i(0, 15, 0)]


## Chunk (cx, cy, cz) gets material 1 + cx + 3 cy + 9 cz at a few probe voxels.
func _mat(c: Vector3i) -> int:
	return 1 + c.x + 3 * c.y + 9 * c.z


func _world() -> SimWorld:
	var w := SimWorld.new()
	w.configure(Vector3i(N, N, N), 2, 1)
	w.set_materials(GameMaterials.defs())
	w.mark_all_loaded(true)
	for cz in N:
		for cy in N:
			for cx in N:
				var c := Vector3i(cx, cy, cz)
				for p in PROBES:
					w.set_voxel(c * 16 + p, _mat(c))
	return w


func _check(d: Vector3i) -> void:
	var w := _world()
	var vac: Array = w.shift(d)
	var expect_vac := 0
	for cz in N:
		for cy in N:
			for cx in N:
				var c := Vector3i(cx, cy, cz)
				var src := c + d
				var inside := src.x >= 0 and src.y >= 0 and src.z >= 0 and src.x < N and src.y < N and src.z < N
				if not inside:
					expect_vac += 1
					assert_bool(vac.has(c)).override_failure_message("%s: %s should be vacated" % [d, c]).is_true()
				for p in PROBES + [Vector3i(8, 8, 8)]:
					var want := _mat(src) if inside and p != Vector3i(8, 8, 8) else 0
					assert_int(w.get_voxel(c * 16 + p)).override_failure_message("shift %s chunk %s voxel %s" % [d, c, p]).is_equal(want)
	assert_int(vac.size()).is_equal(expect_vac)


func test_shift_each_axis_and_diagonals() -> void:
	for d in [Vector3i(1, 0, 0), Vector3i(-1, 0, 0), Vector3i(0, 1, 0), Vector3i(0, -1, 0), Vector3i(0, 0, 1), Vector3i(0, 0, -1), Vector3i(1, -1, 1), Vector3i(-1, 1, -1)]:
		_check(d)


func test_shift_far_clears_everything() -> void:
	var w := _world()
	var vac: Array = w.shift(Vector3i(5, 0, 0))
	assert_int(vac.size()).is_equal(N * N * N)
	assert_int(w.get_voxel(Vector3i(16, 16, 16))).is_equal(0)
