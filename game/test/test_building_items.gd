extends GdUnitTestSuite

## Doors, gates, ladders and decoration (ADR 0094): thin furniture footprints, a door that moves its
## collision voxels when it opens, saving the open state, and the recipes that make them.

var items: ItemRegistry
var recipes: RecipeRegistry
var vox := {}   # Vector3i -> material id, the voxels Main would write; missing = air


func before_test() -> void:
	items = ItemRegistry.load_default()
	recipes = RecipeRegistry.load_default(items)
	vox = {}


## What Main.toggle_door writes, on the dictionary world.
func _swing(store: BlockEntities, e: FurnitureEntity) -> void:
	for c in store.set_open(e, not e.open):
		vox.erase(c)
	for c in e.cells:
		vox[c] = GameMaterials.FURNITURE


## A body of the player's size walking straight through the doorway: does it touch a solid voxel?
func _blocked(door: FurnitureEntity) -> bool:
	var r := Settings.m2v(Settings.player_radius_m)
	var h := Settings.m2v(Settings.player_height_m)
	var mid := Vector3(door.pos) + Vector3(Settings.BLOCK, 0, 0)   # facing +z: the doorway spans x
	for step in range(-8, 9):
		var body := AABB(mid + Vector3(-r, 0.01, step - r), Vector3(2 * r, h - 0.02, 2 * r))
		for c in vox:
			if body.intersects(AABB(Vector3(c), Vector3.ONE)):
				return true
	return false


func test_door_is_a_thin_two_by_four_block_slab() -> void:
	var b := Settings.BLOCK
	var shut := FurnitureEntity.footprint("door", Vector3i(0, 0, 0), 0)
	assert_int(shut.size()).is_equal(2 * b * 4 * b)        # 1 m wide, 2 m tall, one voxel thick
	for c in shut:
		assert_int(c.z).is_equal(0)
	var open := FurnitureEntity.footprint("door", Vector3i(0, 0, 0), 0, true)
	assert_int(open.size()).is_equal(shut.size())
	for c in open:
		assert_int(c.x).is_equal(0)                         # swung against the hinge side
	# every facing keeps the slab inside the space the door swings through
	for f in 4:
		var a := FurnitureEntity.footprint("door", Vector3i(40, 0, 40), f)
		var o := FurnitureEntity.footprint("door", Vector3i(40, 0, 40), f, true)
		var both := 0
		for c in a:
			if o.has(c):
				both += 1
		assert_int(both).override_failure_message("facing %d: the hinge column" % f).is_equal(4 * b)


func test_door_blocks_when_shut_and_lets_through_when_open() -> void:
	assert_bool(GameMaterials.defs()[GameMaterials.FURNITURE - 1].get("collision", false)).is_true()
	var store := BlockEntities.new(items, recipes)
	var e := store.create_furniture(Vector3i(0, 0, 0), "door", 0)
	for c in e.cells:
		vox[c] = GameMaterials.FURNITURE
	assert_bool(_blocked(e)).is_true()
	var shut := e.cells.duplicate()
	_swing(store, e)
	assert_bool(e.open).is_true()
	assert_bool(_blocked(e)).is_false()
	assert_object(store.furniture_at(shut[shut.size() - 1])).is_null()   # the freed latch side
	assert_object(store.furniture_at(e.cells[e.cells.size() - 1])).is_same(e)
	_swing(store, e)
	assert_bool(e.open).is_false()
	assert_bool(_blocked(e)).is_true()
	assert_array(e.cells).is_equal(shut)


func test_open_door_saves_and_loads() -> void:
	var store := BlockEntities.new(items, recipes)
	var e := store.create_furniture(Vector3i(8, 4, -12), "door", 3)
	_swing(store, e)
	var store2 := BlockEntities.new(items, recipes)
	assert_bool(store2.load_dict(JSON.parse_string(JSON.stringify(store.to_dict())))).is_true()
	var e2 := store2.furniture_at(e.cells[3])
	assert_object(e2).is_not_null()
	assert_bool(e2.open).is_true()
	assert_array(e2.cells).is_equal(e.cells)
	assert_array(store2.remove(e.cells[0])).is_equal([{"id": "door", "count": 1}])


func test_only_hinged_kinds_open() -> void:
	for k in FurnitureEntity.KINDS:
		var e := FurnitureEntity.new(Vector3i.ZERO, k, 0)
		assert_bool(e.can_open()).override_failure_message(k).is_equal(k in ["door", "trapdoor", "fence_gate"])
	var store := BlockEntities.new(items, recipes)
	var t := store.create_furniture(Vector3i.ZERO, "table", 0)
	store.set_open(t, true)
	assert_bool(t.open).is_false()


func test_every_new_item_is_placeable_and_craftable() -> void:
	for id in ["door", "trapdoor", "fence", "fence_gate", "ladder", "window", "table", "shelf", "lantern", "flower_pot", "painting"]:
		var def := items.get_def(id)
		assert_str(String(def.get("furniture", ""))).override_failure_message(id).is_equal(id)
		assert_bool(FurnitureEntity.KINDS.has(id)).is_true()
		var makes: Array = recipes.makes(id)
		assert_array(makes).override_failure_message("%s has no recipe" % id).is_not_empty()
		assert_str(String(recipes.recipes[makes[0]].get("station", ""))).is_equal("crafting_table")
	assert_bool(TorchLights.light_of(items, "lantern").is_empty()).is_false()


func _g(cells: Array) -> Array:
	var g := []
	g.resize(9)
	g.fill("")
	for i in cells.size():
		g[i] = cells[i]
	return g


func test_recipes_match_on_the_crafting_table() -> void:
	var p := "oak_planks"
	assert_str(recipes.match_grid(_g([p, p, "", p, p, "", p, p, ""]), "crafting_table").get("id", "")).is_equal("door")
	assert_dict(recipes.match_grid(_g([p, p, "", p, p, "", p, p, ""]), "")).is_empty()   # needs the table
	var s := "stick"
	assert_str(recipes.match_grid(_g([s, "", s, s, s, s, s, "", s]), "crafting_table").get("id", "")).is_equal("ladder")
	assert_str(recipes.match_grid(_g([s, p, s, s, p, s]), "crafting_table").get("id", "")).is_equal("fence")
	assert_str(recipes.match_grid(_g([p, s, p, p, s, p]), "crafting_table").get("id", "")).is_equal("fence_gate")
	assert_str(recipes.match_grid(_g(["iron_ingot", "", "", "torch", "", "", "glass"]), "crafting_table").get("id", "")).is_equal("lantern")
	assert_str(recipes.match_grid(_g(["clay_bowl", "poppy"]), "crafting_table").get("id", "")).is_equal("flower_pot")
