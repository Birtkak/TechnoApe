extends GdUnitTestSuite

## Item sprites (data/item_sprites.json): the data, that every item has a
## real look, and the meshes built from them.

var sprites: ItemSprites
var items: ItemRegistry


func before() -> void:
	sprites = ItemSprites.get_default()
	items = ItemRegistry.load_default()


func test_data_loads() -> void:
	assert_array(sprites.errors).is_empty()
	assert_int(sprites.sprites.size()).is_greater(100)


func test_every_item_has_a_look() -> void:
	# a sprite, or a solid block material drawn as a cube; never the initials tag
	for id in items.items:
		if sprites.has(id):
			continue
		var mat := ItemIcon.material_id_for(items, id)
		assert_int(mat).override_failure_message("item %s has neither a sprite nor a block material" % id).is_greater(0)
		var d: Dictionary = GameMaterials.defs()[mat - 1]
		var block := String(d.get("cell_type", "solid")) in ["solid", "powder"]
		assert_bool(block).override_failure_message("item %s would show a %s cube; give it a sprite" % [id, d["name"]]).is_true()


func test_sprite_images() -> void:
	var img := sprites.image("copper_pickaxe")
	assert_object(img).is_not_null()
	assert_int(img.get_width()).is_equal(ItemSprites.SIZE)
	var copper := _ramp("copper")
	var handle := _ramp("handle")
	var seen := {"copper": 0, "handle": 0, "clear": 0}
	for y in ItemSprites.SIZE:
		for x in ItemSprites.SIZE:
			var c := img.get_pixel(x, y)
			if c.a == 0.0:
				seen["clear"] += 1
			elif copper.any(func(k): return c.is_equal_approx(k)):
				seen["copper"] += 1
			elif handle.any(func(k): return c.is_equal_approx(k)):
				seen["handle"] += 1
	assert_int(seen["copper"]).is_greater(0)
	assert_int(seen["handle"]).is_greater(0)
	assert_int(seen["clear"]).is_greater(100)
	assert_object(sprites.texture("copper_pickaxe")).is_same(sprites.texture("copper_pickaxe"))   # cached
	assert_object(sprites.image("no_such_item")).is_null()


func _ramp(palette: String) -> Array:
	var out := []
	for k in sprites.palettes[palette]:
		out.append(Color.html(String(sprites.palettes[palette][k])))
	return out


func test_bad_data_is_rejected() -> void:
	var bad := ItemSprites.new()
	var row := "................"
	var rows := []
	for i in 16:
		rows.append(row if i != 8 else "......zz........")
	bad.load_json(JSON.stringify({
		"palettes": {"p": {"z": "#ff0000"}},
		"shapes": {"ok": {"rows": rows}, "short": {"rows": ["...."]}},
		"sprites": {
			"no_colour": {"layers": [{"shape": "ok"}]},
			"bad_shape": {"layers": [{"shape": "missing"}]},
			"bad_palette": {"layers": [{"shape": "ok", "palette": "nope"}]},
			"fine": {"layers": [{"shape": "ok", "palette": "p"}]},
		}}))
	assert_int(bad.errors.size()).is_equal(4)   # short shape + three bad sprites
	assert_bool(bad.has("fine")).is_true()
	assert_bool(bad.has("no_colour")).is_false()


func test_meshes() -> void:
	var flat := BlockMeshes.sprite("iron_ingot")
	var box := flat.get_aabb()
	assert_float(box.size.x).is_between(0.7, 1.0)
	assert_float(box.size.z).is_equal_approx(BlockMeshes.SPRITE_DEPTH_PX / ItemSprites.SIZE, 0.001)
	assert_int(flat.surface_get_array_len(0)).is_greater(8)   # front, back and edge walls
	assert_bool(sprites.is_cube("bricks")).is_true()
	var cube := BlockMeshes.sprite("bricks")
	assert_vector(cube.get_aabb().size).is_equal_approx(Vector3.ONE, Vector3.ONE * 0.001)
	assert_object(BlockMeshes.sprite("iron_ingot")).is_same(flat)   # cached


func test_station_items_draw_their_placed_model() -> void:
	# ADR 0084: the crafting table, chest and furnace are held, dropped and shown in the inventory from
	# the same cells as the placed model, so the three looks cannot drift apart
	for name in ["crafting_table", "chest", "furnace"]:
		var mat := GameMaterials.id_of(name)
		var held := BlockMeshes.cube(mat, StandardMaterial3D.new())
		assert_object(held).is_same(StationEntity.item_mesh(name))
		var box := held.get_aabb()
		assert_float(maxf(box.size.x, maxf(box.size.y, box.size.z))).is_equal_approx(1.0, 0.01)
		assert_vector(box.get_center()).is_equal_approx(Vector3.ZERO, Vector3.ONE * 0.01)
		var img := StationEntity.icon(name).get_image()
		var used := img.get_used_rect()
		assert_int(used.size.x).is_greater(StationEntity.ICON_PX * 3 / 4)   # the model fills the icon
		# its top centre shows the model's top cells, lit
		assert_float(img.get_pixel(StationEntity.ICON_PX / 2, used.position.y + 3).a).is_equal(1.0)
	assert_object(StationEntity.icon("stone")).is_null()


func test_loose_heads_know_where_they_sit_on_the_tool() -> void:
	# AssemblyEntity lays a loose head back at its place on the tool, so the stick meets its eye
	assert_vector(Vector2(sprites.loose_offset("flint_axe_head"))).is_equal(Vector2(-1, 4))
	assert_vector(Vector2(sprites.loose_offset("stick"))).is_equal(Vector2.ZERO)
	assert_vector(Vector2(sprites.loose_offset("flint_axe"))).is_equal(Vector2.ZERO)
