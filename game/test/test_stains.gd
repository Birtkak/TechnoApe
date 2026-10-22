extends GdUnitTestSuite

## Surface stains (scripts/visuals/stain_map.gd): blood as a material, the stain
## arithmetic, reading stain sources out of changed simulation chunks, water
## washing, and the GPU window reusing slots without leaking old stains.

const HZ := 30


func _sim() -> SimWorld:
	var w := SimWorld.new()
	w.configure(Vector3i(4, 2, 4), 2, 11)
	w.set_materials(GameMaterials.defs())
	w.set_reactions(GameMaterials.reactions())
	w.mark_all_loaded(true)
	w.fill_box(Vector3i(0, 0, 0), Vector3i(64, 4, 64), GameMaterials.STONE)
	return w


## A bubble over `w` without a terrain: enough for StainMap to read chunks.
func _bubble(w: SimWorld) -> SimBubble:
	var b: SimBubble = auto_free(SimBubble.new())
	b.sim = w
	b.origin = Vector3i.ZERO
	b.size_chunks = Vector3i(4, 2, 4)
	b._ready = true
	return b


func _stains(b: SimBubble) -> StainMap:
	var s: StainMap = auto_free(StainMap.new())
	s.bubble = b
	s._win = Vector3i.ZERO
	return s


func test_blood_is_a_short_lived_liquid() -> void:
	var d: Dictionary = GameMaterials.defs()[GameMaterials.BLOOD - 1]
	assert_str(d["name"]).is_equal("blood")
	assert_str(d["cell_type"]).is_equal("liquid")
	assert_bool(d.get("liquid_sand", false)).is_false()
	assert_int(int(d["lifetime"])).is_greater(0)
	assert_array(d["tiles"]).is_equal([47, 47, 47])


func test_blood_soaks_in() -> void:
	var w := _sim()
	w.fill_box(Vector3i(30, 4, 30), Vector3i(32, 5, 32), GameMaterials.BLOOD)
	assert_int(w.count_material(GameMaterials.BLOOD)).is_equal(4)
	for i in 12 * HZ:
		w.tick(1)
	assert_int(w.count_material(GameMaterials.BLOOD)).is_equal(0)


func test_deposit_accumulates_caps_and_mixes() -> void:
	var s := _stains(null)
	var p := Vector3i(5, 6, 7)
	var red := Color(0.3, 0.0, 0.0)
	var black := Color(0.0, 0.0, 0.0)
	s.deposit(p, red, 0.4)
	assert_float(s.amount_at(p)).is_equal_approx(0.4, 0.01)
	s.deposit(p, red, 0.4)
	s.deposit(p, red, 0.4)
	assert_float(s.amount_at(p)).is_equal(1.0)
	s.deposit(p, black, 1.0)   # half of the coating is now black: red halves
	var c: Color = s._chunks[Vector3i.ZERO].get_pixel(5, 6 * 16 + 7)
	assert_float(c.r).is_equal_approx(0.15, 0.01)
	assert_float(s.amount_at(p + Vector3i(1, 0, 0))).is_equal(0.0)
	# negative coordinates land in their own chunk
	s.deposit(Vector3i(-1, 2, -17), red, 0.5)
	assert_bool(s._chunks.has(Vector3i(-1, 0, -2))).is_true()
	assert_float(s.amount_at(Vector3i(-1, 2, -17))).is_equal_approx(0.5, 0.01)


func test_wash_and_clear() -> void:
	var s := _stains(null)
	var p := Vector3i(1, 1, 1)
	s.deposit(p, Color.RED, 0.8)
	s.wash(p, 0.3)
	assert_float(s.amount_at(p)).is_equal_approx(0.5, 0.01)
	s.clear_cell(p)
	assert_float(s.amount_at(p)).is_equal(0.0)


func test_changed_chunk_with_oil_stains_its_cells() -> void:
	var w := _sim()
	w.set_voxel(Vector3i(20, 4, 21), GameMaterials.OIL)
	w.set_voxel(Vector3i(22, 4, 21), GameMaterials.BLOOD)
	var s := _stains(_bubble(w))
	s._on_chunks_changed([Vector3i(1, 0, 1)])
	assert_int(s.chunks_read).is_equal(0)   # queued only: read on a later idle frame
	s._scan_queue()
	assert_float(s.amount_at(Vector3i(20, 4, 21))).is_greater(0.2)
	assert_float(s.amount_at(Vector3i(22, 4, 21))).is_greater(0.2)
	assert_float(s.amount_at(Vector3i(21, 4, 21))).is_equal(0.0)
	assert_int(s.chunks_read).is_equal(1)
	# chunks outside the GPU window are not read
	s._win = Vector3i(100, 0, 0)
	s._on_chunks_changed([Vector3i(1, 0, 1)])
	s._scan_queue()
	assert_int(s.chunks_read).is_equal(1)


func test_water_washes_a_stain() -> void:
	var w := _sim()
	var p := Vector3i(40, 4, 40)
	var s := _stains(_bubble(w))
	s.deposit(p, Color.RED, 1.0)
	w.set_voxel(p, GameMaterials.WATER)
	s._on_chunks_changed([Vector3i(2, 0, 2)])
	s._scan_queue()
	assert_float(s.amount_at(p)).is_less(1.0)


func test_window_slot_is_cleared_when_reused() -> void:
	var w := _sim()
	var s := _stains(_bubble(w))
	var mats: Array[ShaderMaterial] = []
	s.setup(null, s.bubble, mats)
	s.follow(Vector3(8, 8, 8))     # window chunks -4..3 (x, z), -2..1 (y)
	s.deposit(Vector3i(3, 3, 3), Color.RED, 1.0)
	s._flush_touched()
	assert_float(s._layers[3].get_pixel(3, 3).a).is_greater(0.9)
	assert_bool(s._dirty_layers.has(3)).is_true()
	# move 8 chunks east: chunk (8,0,0) takes chunk (0,0,0)'s slot and has no stains
	s.follow(Vector3(8 + 128, 8, 8))
	assert_float(s._layers[3].get_pixel(3, 3).a).is_equal(0.0)
	# and back: the stain is restored from the chunk map
	s.follow(Vector3(8, 8, 8))
	assert_float(s._layers[3].get_pixel(3, 3).a).is_greater(0.9)
