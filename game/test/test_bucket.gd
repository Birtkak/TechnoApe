extends GdUnitTestSuite

## Buckets hold a body of liquid (ADR 0091, bug report 2026-10-27_17-53-22).

const WATER := 1
const STONE := 2


func _world(cells: Dictionary) -> Callable:
	return func(c: Vector3i) -> int: return int(cells.get(c, 0))


func test_a_bucket_holds_a_cubic_metre() -> void:
	assert_int(Bucket.capacity()).is_equal(512)
	assert_int(Bucket.fill_of({"id": "water_bucket", "count": 1})).is_equal(512)
	assert_int(Bucket.fill_of({"id": "water_bucket", "count": 1, "fill": 40})).is_equal(40)


func test_scoop_takes_the_connected_body_up_to_the_capacity() -> void:
	var w := {}
	for x in 20:
		for z in 20:
			for y in 3:
				w[Vector3i(x, 50 + y, z)] = WATER   # 1200 cells
	w[Vector3i(40, 50, 40)] = WATER   # a pond apart
	var is_water := func(m: int) -> bool: return m == WATER
	var got := Bucket.scoop_cells(_world(w), Vector3i(10, 52, 10), is_water, Bucket.capacity())
	assert_int(got.size()).is_equal(512)
	assert_bool(got.has(Vector3i(40, 50, 40))).is_false()
	assert_object(got[0]).is_equal(Vector3i(10, 52, 10))
	var small := Bucket.scoop_cells(_world(w), Vector3i(40, 50, 40), is_water, 512)
	assert_int(small.size()).is_equal(1)
	assert_array(Bucket.scoop_cells(_world(w), Vector3i(0, 0, 0), is_water, 512)).is_empty()


func test_pour_fills_a_pit_from_the_bottom() -> void:
	var w := {}
	# stone everywhere under y 60 except a 4 x 4 x 4 pit; the pour starts at the pit's mouth
	for x in range(-8, 12):
		for z in range(-8, 12):
			for y in range(50, 60):
				var in_pit := x >= 0 and x < 4 and z >= 0 and z < 4 and y >= 56
				if not in_pit:
					w[Vector3i(x, y, z)] = STONE
	var free := func(m: int) -> bool: return m == 0
	var got := Bucket.pour_cells(_world(w), Vector3i(1, 59, 1), free, 32)
	assert_int(got.size()).is_equal(32)
	for c in got:
		assert_int(c.y).is_less(58)   # the bottom two layers of the pit
	assert_array(Bucket.pour_cells(_world(w), Vector3i(-3, 55, -3), free, 10)).is_empty()   # into stone
