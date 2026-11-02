extends GdUnitTestSuite

## Per-wood planks and saplings (ADR 0112): ids 207..215, each log makes its own planks, leaves
## drop their sapling, and a planted sapling grows into its tree (Saplings, data/saplings.json).

const KINDS := ["spruce", "acacia", "jungle", "birch"]
const LOGS := {"oak": "oak_log", "spruce": "spruce_log", "acacia": "acacia_log", "jungle": "jungle_log", "birch": "birch_log"}
const PLANKS := {"oak": "oak_planks", "spruce": "spruce_planks", "acacia": "acacia_planks", "jungle": "jungle_planks", "birch": "birch_planks"}
const LEAVES := {"oak": "leaves", "spruce": "spruce_leaves", "acacia": "acacia_leaves", "jungle": "jungle_leaves", "birch": "birch_leaves"}


func test_material_ids_are_stable() -> void:
	for i in KINDS.size():
		assert_str(GameMaterials.name_of(GameMaterials.SPRUCE_PLANKS + i)).is_equal("%s_planks" % KINDS[i])
		assert_int(int(GameMaterials.defs()[GameMaterials.SPRUCE_PLANKS + i - 1]["tiles"][0])).is_equal(176 + i)
	for i in GameMaterials.WOOD_KINDS.size():
		var id: int = GameMaterials.OAK_SAPLING + i
		assert_str(GameMaterials.name_of(id)).is_equal("%s_sapling" % GameMaterials.WOOD_KINDS[i])
		assert_bool(GameMaterials.is_plain_voxel(id)).is_false()   # one micro voxel, not a brush block
	assert_int(GameMaterials.OAK_SAPLING).is_equal(211)
	assert_int(GameMaterials.defs().size()).is_greater_equal(215)
	assert_int(GameMaterials.micro_pattern("sapling", [20, 85, 85]).count(0)).is_less(512)


func test_each_log_makes_its_own_planks() -> void:
	var items := ItemRegistry.load_default()
	var recipes := RecipeRegistry.load_default(items)
	var mining := MiningTable.load_default()
	for k in LOGS:
		var g := ["", "", "", "", LOGS[k], "", "", "", ""]
		var r := recipes.match_grid(g)
		assert_str(String(r.get("output", {}).get("id", ""))).override_failure_message("%s log" % k).is_equal(PLANKS[k])
		assert_bool(items.has_tag(PLANKS[k], "planks")).is_true()   # so every #planks recipe takes them
		var saw := recipes.match_grid([LOGS[k], "copper_saw", "", "", "", "", "", "", ""])
		assert_str(String(saw.get("output", {}).get("id", ""))).is_equal(PLANKS[k])
		assert_int(int(saw["output"]["count"])).is_equal(4)
		assert_str(String(mining.entry(GameMaterials.name_of(GameMaterials.id_of(items.get_def(PLANKS[k])["material"]))).get("drops", ""))).is_equal(PLANKS[k])
	# two different planks still make sticks
	assert_str(recipes.match_grid(["birch_planks", "", "", "spruce_planks", "", "", "", "", ""]).get("id", "")).is_equal("sticks")


func test_leaves_drop_their_sapling() -> void:
	var mining := MiningTable.load_default()
	var items := ItemRegistry.load_default()
	for k in LEAVES:
		var ids := []
		for d in mining.entry(LEAVES[k])["drops"]:
			ids.append(d["id"])
		assert_array(ids).contains(["%s_sapling" % k])
		assert_bool(items.has("%s_sapling" % k)).is_true()
		assert_str(String(mining.entry("%s_sapling" % k).get("drops", ""))).is_equal("%s_sapling" % k)


## A fake world: a dict of voxels, everything outside a box unsimulated (-1).
class FakeWorld:
	var v := {}
	func get_voxel(p: Vector3i) -> int:
		if p.x < -64 or p.x > 64 or p.z < -64 or p.z > 64 or p.y < -8 or p.y > 160:
			return -1
		return v.get(p, GameMaterials.GRASS if p.y < 0 else GameMaterials.AIR)
	func fill_box(lo: Vector3i, hi: Vector3i, m: int) -> int:
		for x in range(lo.x, hi.x):
			for y in range(lo.y, hi.y):
				for z in range(lo.z, hi.z):
					v[Vector3i(x, y, z)] = m
		return 1
	func fill_ball(c: Vector3, r: float, m: int) -> void:
		for x in range(floori(c.x - r), ceili(c.x + r) + 1):
			for y in range(floori(c.y - r), ceili(c.y + r) + 1):
				for z in range(floori(c.z - r), ceili(c.z + r) + 1):
					var p := Vector3i(x, y, z)
					if (Vector3(p) + Vector3(0.5, 0.5, 0.5)).distance_to(c) <= r and get_voxel(p) == GameMaterials.AIR:
						v[p] = m


func _saplings(w: FakeWorld) -> Saplings:
	var s := Saplings.load_default()
	assert_array(s.errors).is_empty()
	s.get_voxel = w.get_voxel
	s.fill_box = w.fill_box
	s.fill_ball = w.fill_ball
	return s


func test_a_planted_sapling_grows_into_its_tree() -> void:
	var w := FakeWorld.new()
	var s := _saplings(w)
	assert_int(s.trees.size()).is_equal(5)
	var p := Vector3i(0, 0, 0)
	w.v[p] = GameMaterials.id_of("birch_sapling")
	s.on_placed(p, w.v[p])
	s.on_placed(Vector3i(5, 0, 5), GameMaterials.STONE)   # not a sapling: ignored
	assert_int(s.planted.size()).is_equal(1)
	assert_array(s.tick(s.grow_s.x - 1.0)).is_empty()   # not yet
	assert_array(s.tick(s.grow_s.y)).contains_exactly([p])
	assert_int(s.planted.size()).is_equal(0)
	assert_int(w.get_voxel(p)).is_equal(GameMaterials.BIRCH_LOG)   # the 0.5 m trunk: log inside bark
	assert_int(w.get_voxel(p - Vector3i(2, 0, 2))).is_equal(GameMaterials.BIRCH_BARK)
	assert_int(w.get_voxel(p + Vector3i(0, 40, 0))).is_equal(GameMaterials.BIRCH_LOG)   # >= 6 m tall
	assert_bool(w.v.values().has(GameMaterials.BIRCH_LEAVES)).is_true()


func test_blocked_dug_and_far_saplings() -> void:
	var w := FakeWorld.new()
	var s := _saplings(w)
	var sap := GameMaterials.id_of("oak_sapling")
	var blocked := Vector3i(0, 0, 0)
	var dug := Vector3i(20, 0, 0)
	var far := Vector3i(200, 0, 0)
	for p in [blocked, dug, far]:
		w.v[p] = sap
		s.on_placed(p, sap)
	w.v[blocked + Vector3i(0, 10, 0)] = GameMaterials.STONE   # a roof over it
	w.v.erase(dug)
	s.tick(s.grow_s.y + 1.0)
	assert_bool(s.planted.has(blocked)).is_true()   # waits, tries again later
	assert_float(float(s.planted[blocked]["left_s"])).is_equal(Saplings.RETRY_S)
	assert_bool(s.planted.has(dug)).is_false()      # forgotten
	assert_bool(s.planted.has(far)).is_true()       # not simulated: waits
	assert_int(w.get_voxel(blocked)).is_equal(sap)
	# the save keeps what is still growing
	var again := _saplings(w)
	assert_bool(again.load_dict(JSON.parse_string(JSON.stringify(s.to_dict())))).is_true()
	assert_array(again.planted.keys()).contains_exactly_in_any_order([blocked, far])


func test_soil() -> void:
	for m in [GameMaterials.GRASS, GameMaterials.DIRT, GameMaterials.SAVANNA_GRASS, GameMaterials.LEAF_LITTER]:
		assert_bool(GameMaterials.is_soil(m)).is_true()
	for m in [GameMaterials.STONE, GameMaterials.SAND, GameMaterials.PLANKS, GameMaterials.AIR]:
		assert_bool(GameMaterials.is_soil(m)).is_false()
