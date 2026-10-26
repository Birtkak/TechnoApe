extends GdUnitTestSuite

## TNT (ADR 0065): the material, item, recipe and sprite, the blast's hardness shells and sizes,
## charges on the block grid, and the explosion against a stub world (the native blast is checked
## by the tnt_test=1 run: game/out/tnt*).

const StubMain := preload("res://test/tnt_stub_main.gd")

var items: ItemRegistry


func before() -> void:
	items = ItemRegistry.load_default()


func test_tnt_is_an_appended_plain_voxel() -> void:
	assert_int(GameMaterials.TNT).is_equal(203)
	assert_int(GameMaterials.id_of("tnt")).is_equal(GameMaterials.TNT)
	assert_bool(GameMaterials.is_plain_voxel(GameMaterials.TNT)).is_true()
	var d: Dictionary = GameMaterials.defs()[GameMaterials.TNT - 1]
	assert_bool(d.get("burnable", false)).is_false()   # the sim must not burn it away: Tnt lights it
	assert_array(d["tiles"]).is_equal([172, 171, 172])
	assert_int(GameMaterials.ATLAS_TILES.x * GameMaterials.ATLAS_TILES.y).is_greater(172)


func test_items_recipes_and_sprites() -> void:
	assert_bool(items.has("tnt")).is_true()
	assert_bool(items.has("igniter")).is_true()
	assert_int(items.units("tnt")).is_equal(Settings.BLOCK * Settings.BLOCK * Settings.BLOCK)   # a voxel item: the brush places it
	assert_str(str(items.get_def("igniter").get("ignites", ""))).is_equal("tnt")
	var recipes := RecipeRegistry.load_default(items)
	assert_bool(recipes.recipes.has("tnt")).is_true()
	assert_bool(recipes.recipes.has("igniter")).is_true()
	var sprites: Dictionary = JSON.parse_string(FileAccess.get_file_as_string("res://data/item_sprites.json"))
	assert_bool(sprites["sprites"].has("tnt")).is_true()
	assert_bool(sprites["sprites"].has("igniter")).is_true()
	var mining := MiningTable.load_default()
	assert_str(str(mining.entry("tnt").get("drops", ""))).is_equal("tnt")


func test_blast_shells_follow_hardness() -> void:
	assert_float(Tnt.shell_factor(GameMaterials.DIRT)).is_equal(1.0)
	assert_float(Tnt.shell_factor(GameMaterials.WOOD)).is_equal(1.0)
	assert_float(Tnt.shell_factor(GameMaterials.TNT)).is_equal(1.0)
	assert_float(Tnt.shell_factor(GameMaterials.STONE)).is_equal(0.75)
	assert_float(Tnt.shell_factor(GameMaterials.BRICKS)).is_equal(0.75)
	assert_float(Tnt.shell_factor(GameMaterials.OBSIDIAN)).is_equal(0.0)
	assert_float(Tnt.shell_factor(GameMaterials.BEDROCK)).is_equal(0.0)
	assert_float(Tnt.shell_factor(GameMaterials.WATER)).is_equal(0.0)   # liquids stay
	assert_float(Tnt.shell_factor(GameMaterials.CHEST)).is_equal(0.0)   # block entities stay


func test_radius_grows_with_the_cube_root() -> void:
	var full := Tnt.radius_of(1.0)
	assert_float(full).is_equal_approx(Settings.m2v(Settings.tnt_radius_m), 0.001)
	assert_float(Tnt.radius_of(1.0 / 8.0)).is_equal_approx(full / 2.0, 0.001)
	assert_float(Tnt.radius_of(8.0)).is_equal_approx(full * 2.0, 0.001)


func test_charges_sit_on_the_block_grid() -> void:
	assert_object(Tnt.anchor_of(Vector3i(5, -1, 9))).is_equal(Vector3i(4, -4, 8))
	assert_object(Tnt.anchor_of(Vector3i(4, 0, 8))).is_equal(Vector3i(4, 0, 8))
	var t: Tnt = auto_free(Tnt.new())
	t.register_cells([Vector3i(1, 1, 1), Vector3i(2, 3, 0), Vector3i(4, 0, 0)])
	assert_int(t.charges.size()).is_equal(2)
	var saved := t.to_array()
	var u: Tnt = auto_free(Tnt.new())
	u.load_array(saved)
	assert_dict(u.charges).is_equal(t.charges)


## A charge lit in a stub world: its voxels go, it fuses, then explodes once with three shells.
func test_lit_charge_fuses_then_blasts() -> void:
	var stub = auto_free(StubMain.new())
	add_child(stub)
	var t: Tnt = auto_free(Tnt.new())
	stub.add_child(t)
	t.setup(stub)
	for y in 4:
		for z in 4:
			for x in 4:
				stub.voxels[Vector3i(x, y, z)] = GameMaterials.TNT
	for x in range(-8, 12):
		for z in range(-8, 12):
			stub.voxels[Vector3i(x, -1, z)] = GameMaterials.STONE
	t.register_cells([Vector3i.ZERO])
	assert_bool(t.light(Vector3i.ZERO, 0.5)).is_true()
	assert_int(stub.voxels.get(Vector3i(1, 1, 1), 0)).is_equal(GameMaterials.AIR)
	assert_int(t.primed.size()).is_equal(1)
	t._process(0.3)
	assert_int(stub.bubble.blasts.size()).is_equal(0)
	t._process(0.3)   # the fuse runs out: queued, the chain search runs this frame
	assert_int(t.primed.size()).is_equal(0)
	assert_int(stub.bubble.blasts.size()).is_equal(0)
	t._process(0.016)   # and the blast the next
	assert_int(stub.bubble.blasts.size()).is_equal(3)
	assert_float(stub.bubble.blasts[0][1]).is_equal_approx(Tnt.radius_of(1.0), 0.01)
	assert_float(stub.bubble.blasts[2][1]).is_equal_approx(Tnt.radius_of(1.0) * 0.5, 0.01)
	assert_int(t.stats["blasts"]).is_equal(1)


## Fuses that run out together go off one per two frames (chain search, then blast;
## Settings.tnt_blasts_per_frame stages a frame).
func test_simultaneous_fuses_spread_over_frames() -> void:
	var stub = auto_free(StubMain.new())
	add_child(stub)
	var t: Tnt = auto_free(Tnt.new())
	stub.add_child(t)
	t.setup(stub)
	for i in 5:
		t._prime(Vector3(i * 40, 10, 0), 1.0, 0.1)
	t._process(0.2)
	assert_int(t.stats["blasts"]).is_equal(0)
	assert_int(t.stats["queued_max"]).is_equal(5)
	t._process(0.016)
	assert_int(t.stats["blasts"]).is_equal(1)
	for i in 8:
		t._process(0.016)
	assert_int(t.stats["blasts"]).is_equal(5)


## Fire on a face lights a charge; the igniter's sneak use lights every charge in range, nearest first.
func test_fire_and_igniter_light_charges() -> void:
	var stub = auto_free(StubMain.new())
	add_child(stub)
	var t: Tnt = auto_free(Tnt.new())
	stub.add_child(t)
	t.setup(stub)
	for a in [Vector3i(0, 0, 0), Vector3i(40, 0, 0), Vector3i(400, 0, 0)]:
		for y in 4:
			for z in 4:
				for x in 4:
					stub.voxels[a + Vector3i(x, y, z)] = GameMaterials.TNT
		t.register_cells([a])
	stub.voxels[Vector3i(-1, 2, 1)] = GameMaterials.FIRE   # against the first block's -x face
	t._process(0.25)
	assert_int(t.stats["fire_lit"]).is_equal(1)
	assert_int(t.primed.size()).is_equal(1)
	assert_int(t.light_near(Vector3(0, 0, 0), 16.0)).is_equal(1)   # the block 5 m away; the one 50 m away stays
	assert_int(t.primed.size()).is_equal(2)
	assert_bool(t.charges.has(Vector3i(400, 0, 0))).is_true()
