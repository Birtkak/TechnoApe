extends GdUnitTestSuite

## Plan item D10 (ADR 0055): waypoints, the explored map's tiles and the material palette.


func test_waypoints_add_rename_remove_and_save() -> void:
	var w := Waypoints.new()
	var a := w.add("Base", Vector3i(10, 1040, -20))
	var b := w.add("", Vector3i(400, 1100, 30), "grave")
	assert_int(w.list.size()).is_equal(2)
	assert_str(String(w.get_waypoint(b)["name"])).is_equal("Waypoint 2")
	assert_bool(w.get_waypoint(b)["colour"] == Waypoints.KIND_COLOURS["grave"]).is_true()
	assert_bool(w.rename(a, "  Home ")).is_true()
	assert_str(String(w.get_waypoint(a)["name"])).is_equal("Home")
	var before: Color = w.get_waypoint(a)["colour"]
	w.cycle_colour(a)
	assert_bool(w.get_waypoint(a)["colour"] != before).is_true()
	var saved: Array = JSON.parse_string(JSON.stringify(w.to_array()))   # through JSON, as the player file is
	var w2 := Waypoints.new()
	w2.load_array(saved)
	assert_int(w2.list.size()).is_equal(2)
	assert_bool(w2.get_waypoint(a)["pos"] == Vector3i(10, 1040, -20)).is_true()
	assert_int(w2.add("Next", Vector3i.ZERO)).is_greater(b)   # ids never repeat after a load
	assert_bool(w2.remove(b)).is_true()
	assert_bool(w2.remove(b)).is_false()
	assert_str(String(w2.nearest(Vector2(12, -18), 8.0).get("name", ""))).is_equal("Home")
	assert_bool(w2.nearest(Vector2(200, 200), 8.0).is_empty()).is_true()


func test_tiles_explore_and_masked_surface() -> void:
	var m: WorldMap = auto_free(WorldMap.new())
	var c := Vector2i(-WorldMap.CELL, 0)   # a cell left of the origin: tile (-1, 0)
	assert_bool(m.explored(c + Vector2i(3, 3))).is_false()
	var gen := Image.create(WorldMap.CELL, WorldMap.CELL, false, Image.FORMAT_RGBA8)
	gen.fill(Color8(40, 160, 60, 255))   # generated terrain
	m._write(c, gen, false)
	assert_bool(m.explored(c + Vector2i(3, 3))).is_true()
	assert_bool(WorldMap.tile_of_px(c) == Vector2i(-1, 0)).is_true()
	# a real surface with one column known: only that pixel changes
	var surf := Image.create(WorldMap.CELL, WorldMap.CELL, false, Image.FORMAT_RGBA8)
	var y := 1060
	surf.set_pixel(5, 7, Color8(GameMaterials.STONE, 0, y & 255, y >> 8))
	m._write(c, surf, true)
	var img := m.tile_image(Vector2i(-1, 0))
	var p := Vector2i(posmod(c.x, WorldMap.TILE), 0)
	var got := img.get_pixel(p.x + 5, 7)
	assert_int(got.r8).is_equal(GameMaterials.STONE)
	assert_int(got.b8 + 256 * got.a8).is_equal(y)
	assert_int(img.get_pixel(p.x + 6, 7).a8).is_equal(255)   # its neighbour keeps the generated colour
	assert_object(m.tile_texture(Vector2i(-1, 0))).is_not_null()
	assert_object(m.tile_texture(Vector2i(5, 5))).is_null()


func test_palette_has_material_colours() -> void:
	var pal := WorldMap.build_palette()
	assert_int(pal.get_width()).is_equal(256)
	for id in [GameMaterials.GRASS, GameMaterials.STONE, GameMaterials.SAND, GameMaterials.WATER]:
		var c := pal.get_pixel(id, 0)
		assert_float(c.a).is_greater(0.5)
	assert_bool(pal.get_pixel(GameMaterials.GRASS, 0).g > pal.get_pixel(GameMaterials.GRASS, 0).b).is_true()   # green


func test_px_of_rounds_down() -> void:
	assert_bool(WorldMap.px_of(-1.0, 7.9) == Vector2i(-1, 1)).is_true()
	assert_bool(WorldMap.px_of(4.0, 0.0) == Vector2i(1, 0)).is_true()
