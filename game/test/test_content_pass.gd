extends GdUnitTestSuite

## Content pass: tree kinds with their own logs and leaves (biomes.json
## tree_kind), badlands terracotta strata, magma cave basalt, frozen seas,
## ice field spikes and ferns. Ids 76..87 are append-only like every material.

const S := preload("res://scripts/settings.gd")
const NEW := ["spruce_log", "spruce_leaves", "acacia_log", "acacia_leaves", "jungle_log", "jungle_leaves",
	"terracotta", "terracotta_red", "terracotta_cream", "basalt", "packed_ice", "fern"]


func test_material_ids_are_stable() -> void:
	for i in NEW.size():
		assert_str(GameMaterials.name_of(GameMaterials.SPRUCE_LOG + i)).is_equal(NEW[i])
	assert_int(GameMaterials.FERN).is_equal(87)
	assert_int(GameMaterials.defs().size()).is_greater_equal(GameMaterials.FERN)
	# every tile a material names fits the atlas
	var n := GameMaterials.ATLAS_TILES.x * GameMaterials.ATLAS_TILES.y
	for d in GameMaterials.defs():
		for t in d["tiles"]:
			assert_int(int(t)).override_failure_message("%s tile %d outside the atlas" % [d["name"], t]).is_less(n)


func test_new_blocks_are_minable_and_logs_make_planks() -> void:
	var mining := MiningTable.load_default()
	var items := ItemRegistry.load_default()
	var recipes := RecipeRegistry.load_default(items)
	for name in NEW:
		assert_bool(mining.is_minable(name)).override_failure_message("%s not minable" % name).is_true()
	for id in ["spruce_log", "acacia_log", "jungle_log", "terracotta", "terracotta_red", "terracotta_cream", "basalt", "packed_ice"]:
		assert_bool(items.has(id)).override_failure_message("no item %s" % id).is_true()
		assert_str(String(mining.entry(id).get("drops", ""))).is_equal(id)
	for id in ["spruce_log", "acacia_log", "jungle_log"]:
		assert_bool(id in items.items_with_tag("log")).is_true()
	assert_bool("planks_from_log" in recipes.uses_of("jungle_log")).is_true()


func test_fern_is_a_small_plant() -> void:
	var cells := GameMaterials.micro_pattern("fern", [72, 29, 72])
	var filled := 0
	var top := 0
	for i in cells.size():
		if cells[i] != 0:
			filled += 1
			top = maxi(top, (i / GameMaterials.MICRO_GRID) % GameMaterials.MICRO_GRID)
	assert_int(filled).is_between(20, 120)
	assert_int(top).is_between(3, 6)   # knee-high, under tall grass


func _count(g: FineMacro, origin: Vector3i, size: Vector3i, mats: Array) -> Dictionary:
	var bytes: PackedByteArray = g.generate_block(origin, size, 0, 2)
	var out := {}
	for m in mats:
		out[m] = 0
	for i in range(0, bytes.size(), 2):
		var m := bytes.decode_u16(i)
		if out.has(m):
			out[m] += 1
	return out


func _gen() -> FineMacro:
	return FineMacro.new()


func _near(g: FineMacro, kind: String) -> Vector3i:
	# continents are ~8 km across (2026-10-23): some biomes lie a continent away
	var p: Vector3i = g.find_nearest(kind, 0, 0, 32000)
	assert_int(p.y).override_failure_message("no %s within 16 km" % kind).is_greater(0)
	return p


func test_tree_kinds_grow_their_own_wood() -> void:
	var g := _gen()
	for k in [["taiga", GameMaterials.SPRUCE_LOG, GameMaterials.SPRUCE_LEAVES],
			["savanna", GameMaterials.ACACIA_LOG, GameMaterials.ACACIA_LEAVES],
			["jungle", GameMaterials.JUNGLE_LOG, GameMaterials.JUNGLE_LEAVES],
			["forest", GameMaterials.WOOD, GameMaterials.LEAVES]]:
		var p := _near(g, k[0])
		# savanna trees are sparse: a wider window
		var half := 48 if k[0] == "savanna" else 24
		var c := _count(g, p - Vector3i(half, 8, half), Vector3i(half * 2, 48, half * 2), [k[1], k[2], GameMaterials.WOOD])
		assert_int(c[k[1]]).override_failure_message("%s: no %s" % [k[0], GameMaterials.name_of(k[1])]).is_greater(0)
		assert_int(c[k[2]]).override_failure_message("%s: no %s" % [k[0], GameMaterials.name_of(k[2])]).is_greater(c[k[1]])
		if k[1] != GameMaterials.WOOD and k[0] != "savanna":
			# a bordering forest may reach into the window, but the biome's own wood dominates (not in
			# sparse savanna, whose nearest point borders forests of thick oaks, ADR 0035)
			assert_int(c[GameMaterials.WOOD]).override_failure_message("%s grew mostly oak" % k[0]).is_less(c[k[1]])


func test_ferns_grow_in_jungles_and_taiga() -> void:
	var g := _gen()
	for kind in ["jungle", "taiga"]:
		var p := _near(g, kind)
		var c := _count(g, p - Vector3i(24, 8, 24), Vector3i(48, 24, 48), [GameMaterials.FERN])
		assert_int(c[GameMaterials.FERN]).override_failure_message("%s: no ferns" % kind).is_greater(5)


func test_badlands_show_strata() -> void:
	var g := _gen()
	var p := _near(g, "badlands")
	var t := [GameMaterials.TERRACOTTA, GameMaterials.TERRACOTTA_RED, GameMaterials.TERRACOTTA_CREAM]
	var c := _count(g, p - Vector3i(24, 24, 24), Vector3i(48, 48, 48), t)
	for m in t:
		assert_int(c[m]).override_failure_message("badlands: no %s" % GameMaterials.name_of(m)).is_greater(0)


func test_frozen_sea_and_ice_spikes() -> void:
	var g := _gen()
	var p := _near(g, "frozen_ocean")
	var c := _count(g, Vector3i(p.x - 24, FineMacro.SEA - 4, p.z - 24), Vector3i(48, 8, 48), [GameMaterials.ICE, GameMaterials.WATER])
	assert_int(c[GameMaterials.ICE]).override_failure_message("frozen ocean: no sea ice").is_greater(1000)
	p = _near(g, "ice_fields")
	# spikes stand on the ground: packed ice above the surface height somewhere in a 64 m square
	var above := 0
	for z in range(-64, 64, 16):
		for x in range(-64, 64, 16):
			var o := Vector3i(p.x + x, 0, p.z + z)
			var h: int = g.height_at(o.x + 8, o.z + 8)
			above += _count(g, Vector3i(o.x, h + 2, o.z), Vector3i(16, 24, 16), [GameMaterials.PACKED_ICE])[GameMaterials.PACKED_ICE]
	assert_int(above).override_failure_message("ice fields: no spikes").is_greater(20)


func test_magma_caves_are_basalt() -> void:
	var g := _gen()
	var p := _near(g, "magma_cave")
	var c := _count(g, p - Vector3i(24, 8, 24), Vector3i(48, 24, 48), [GameMaterials.BASALT])
	assert_int(c[GameMaterials.BASALT]).is_greater(100)


## Voxels of `mats` from `lo` to `hi` voxels above the local ground, over a
## 128 x 128 window around p, sampled per 16 x 16 tile from the tile's ground.
func _above_ground(g: FineMacro, p: Vector3i, mats: Array, lo: int, hi: int) -> Dictionary:
	var out := {}
	for m in mats:
		out[m] = 0
	for z in range(-64, 64, 16):
		for x in range(-64, 64, 16):
			var o := Vector3i(p.x + x, 0, p.z + z)
			var h: int = g.height_at(o.x + 8, o.z + 8)
			var c := _count(g, Vector3i(o.x, h + lo, o.z), Vector3i(16, hi - lo, 16), mats)
			for m in mats:
				out[m] += c[m]
	return out


func test_decor_bushes_boulders_dead_trees() -> void:
	var g := _gen()
	# jungle bushes: leaves and logs within 2 voxels of the ground (jungle crowns start 12+ up)
	var c := _above_ground(g, _near(g, "jungle"), [GameMaterials.JUNGLE_LEAVES, GameMaterials.JUNGLE_LOG], 0, 3)
	assert_int(c[GameMaterials.JUNGLE_LEAVES]).override_failure_message("jungle: no bushes").is_greater(50)
	# highland boulders: stone standing on the grass
	c = _above_ground(g, _near(g, "highlands"), [GameMaterials.STONE], 0, 4)
	assert_int(c[GameMaterials.STONE]).override_failure_message("highlands: no boulders").is_greater(20)
	# savanna dead trees: bare acacia trunks, no leaves around them at head height
	c = _above_ground(g, _near(g, "savanna"), [GameMaterials.ACACIA_LOG], 0, 6)
	assert_int(c[GameMaterials.ACACIA_LOG]).override_failure_message("savanna: no trunks").is_greater(0)


func test_spires_in_badlands_and_deserts() -> void:
	var g := _gen()
	var c := _above_ground(g, _near(g, "desert"), [GameMaterials.SANDSTONE], 2, 14)
	assert_int(c[GameMaterials.SANDSTONE]).override_failure_message("desert: no hoodoos").is_greater(0)
	var t := [GameMaterials.TERRACOTTA, GameMaterials.TERRACOTTA_RED, GameMaterials.TERRACOTTA_CREAM]
	c = _above_ground(g, _near(g, "badlands"), t, 3, 26)
	assert_int(c[t[0]] + c[t[1]] + c[t[2]]).override_failure_message("badlands: no spires").is_greater(20)


# ---------- content pass 2: birch, fallen logs, blueberries, pumpkins, flowers ----------

const NEW2 := ["birch_log", "birch_leaves", "blueberry_bush", "pumpkin", "cornflower", "oxeye_daisy", "allium",
	"oak_bark", "spruce_bark", "acacia_bark", "jungle_bark", "birch_bark"]


func test_content2_ids_items_and_drops() -> void:
	for i in NEW2.size():
		assert_str(GameMaterials.name_of(GameMaterials.BIRCH_LOG + i)).is_equal(NEW2[i])
	assert_int(GameMaterials.BIRCH_LOG).is_equal(90)
	var mining := MiningTable.load_default()
	var items := ItemRegistry.load_default()
	var recipes := RecipeRegistry.load_default(items)
	for n in NEW2:
		assert_bool(mining.is_minable(n)).override_failure_message("%s not minable" % n).is_true()
	for id in ["birch_log", "blueberry", "pumpkin", "pumpkin_pie"]:
		assert_bool(items.has(id)).override_failure_message("no item %s" % id).is_true()
	assert_str(String(mining.entry("oak_bark")["drops"])).is_equal("oak_log")   # fallen logs give logs
	assert_bool("pumpkin_pie" in recipes.makes("pumpkin_pie")).is_true()
	var cells := GameMaterials.micro_pattern("berry_bush", [76, 29, 29])
	var berries := 0
	for c in cells:
		if c == 77:
			berries += 1
	assert_int(berries).is_greater(0)


func test_birch_fallen_logs_berries_and_flowers_generate() -> void:
	var g := _gen()
	var c := _above_ground(g, _near(g, "forest"), [GameMaterials.BIRCH_LOG, GameMaterials.WOOD], 0, 16)
	assert_int(c[GameMaterials.BIRCH_LOG]).override_failure_message("forest: no birches").is_greater(0)
	assert_int(c[GameMaterials.WOOD]).override_failure_message("forest: no oaks left").is_greater(c[GameMaterials.BIRCH_LOG])
	# fallen logs, stumps and trunks: bark near the ground over a 64 m square
	c = _above_ground(g, _near(g, "old_growth_forest"), [GameMaterials.OAK_BARK], 0, 4)
	assert_int(c[GameMaterials.OAK_BARK]).override_failure_message("old growth: no fallen logs").is_greater(15)
	c = _above_ground(g, _near(g, "taiga"), [GameMaterials.BLUEBERRY_BUSH], 0, 2)
	assert_int(c[GameMaterials.BLUEBERRY_BUSH]).override_failure_message("taiga: no blueberries").is_greater(0)
	var f := [GameMaterials.FLOWER, GameMaterials.FLOWER_RED, GameMaterials.OXEYE_DAISY, GameMaterials.CORNFLOWER]
	c = _above_ground(g, _near(g, "plains"), f, 0, 2)
	var kinds := 0
	for m in f:
		if c[m] > 0:
			kinds += 1
	assert_int(kinds).override_failure_message("plains flowers: %s" % str(c)).is_greater(2)
