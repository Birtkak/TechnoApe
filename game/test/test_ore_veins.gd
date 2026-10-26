extends GdUnitTestSuite

## GregTech-style ore veins and ground pickups (ADR 0019, ore_veins.json):
## the data, the generated header, and what the native generator makes of them.

const S := preload("res://scripts/settings.gd")
const ORES := [GameMaterials.COAL_ORE, GameMaterials.IRON_ORE, GameMaterials.COPPER_ORE, GameMaterials.TIN_ORE]
const PICKUPS := ["pebbles", "flint_stones", "twigs", "copper_bearing_stones", "tin_bearing_stones", "iron_bearing_stones"]

var gen: FineGen
const F := FineGen.FS   # fine voxels per macro unit: ore_veins.json counts macro units
var reg: OreVeinRegistry


func before() -> void:
	gen = FineGen.new()
	gen.configure(1337, S.SEA_LEVEL_VOXEL, S.WORLD_HEIGHT_VOXELS)
	reg = OreVeinRegistry.load_default()


func _cell_veins(radius_cells: int) -> Array:
	var out := []
	var c := int(reg.grid["cell_voxels"]) * F
	for cz in range(-radius_cells, radius_cells):
		for cx in range(-radius_cells, radius_cells):
			var v: Dictionary = gen.vein_at(cx * c + c / 2, cz * c + c / 2)
			if not v.is_empty():
				out.append(v)
	return out


func _block(origin: Vector3i, size: Vector3i) -> PackedInt32Array:
	var bytes: PackedByteArray = gen.generate_block(origin, size, 0, 2)
	var ids := PackedInt32Array()
	ids.resize(size.x * size.y * size.z)
	if bytes.is_empty():
		return ids
	for i in ids.size():
		ids[i] = bytes.decode_u16(i * 2)
	return ids


func _count(ids: PackedInt32Array, wanted: Array) -> int:
	var n := 0
	for id in ids:
		if id in wanted:
			n += 1
	return n


func test_data_loads() -> void:
	assert_array(reg.errors).is_empty()
	assert_int(reg.count()).is_equal(5)
	assert_int(reg.total_weight()).is_greater(0)
	for v in reg.veins:
		# the lens never leaves its cell, so a column only looks at one cell
		assert_int(int(v["size_voxels"]) / 2 + reg.jitter_of(v)).is_less(int(reg.grid["cell_voxels"]) / 2)


func test_header_is_current() -> void:
	var path := ProjectSettings.globalize_path("res://").path_join("..").path_join(OreVeinCodegen.HEADER_PATH)
	var f := FileAccess.open(path, FileAccess.READ)
	assert_object(f).override_failure_message("missing %s; run tools/ore_vein_codegen.gd" % path).is_not_null()
	if f == null:
		return
	assert_str(f.get_as_text().replace("\r\n", "\n")).override_failure_message("header stale; run tools/ore_vein_codegen.gd").is_equal(OreVeinCodegen.generate(reg))


func test_bad_data_is_rejected() -> void:
	var bad := OreVeinRegistry.new()
	bad.load_json(JSON.stringify({
		"grid": {"cell_voxels": 192, "fill": 0.5, "size_voxels": 64, "thickness_voxels": 16, "density": 0.6, "sporadic_chance": 0.1},
		"indicators": {"radius_voxels": 40, "max_depth_voxels": 100, "chance": 0.01},
		"veins": [
			{"name": "upside_down", "y_min": 300, "y_max": 100, "weight": 1, "primary": "iron_ore", "between": "iron_ore", "secondary": "iron_ore", "sporadic": "iron_ore"},
			{"name": "made_up", "y_min": 100, "y_max": 200, "weight": 1, "primary": "mithril_ore", "between": "iron_ore", "secondary": "iron_ore", "sporadic": "iron_ore"},
			{"name": "big_indicator", "y_min": 100, "y_max": 200, "weight": 1, "primary": "iron_ore", "between": "iron_ore", "secondary": "iron_ore", "sporadic": "iron_ore", "indicator": "stone"},
		]}))
	assert_int(bad.errors.size()).is_greater_equal(3)
	assert_int(bad.count()).is_equal(0)


func test_pickups_are_minable_and_drop_items() -> void:
	assert_int(OreVeinRegistry.material_id("copper_bearing_stones")).is_equal(GameMaterials.COPPER_BEARING_STONES)
	assert_int(OreVeinRegistry.material_id("twigs")).is_equal(GameMaterials.TWIGS)
	var mining := MiningTable.load_default()
	var items := ItemRegistry.load_default()
	for m in PICKUPS:
		assert_bool(mining.is_minable(m)).override_failure_message("%s not in mining.json" % m).is_true()
		var drop := String(mining.entry(m).get("drops", ""))
		assert_bool(items.has(drop)).override_failure_message("%s drops unknown item %s" % [m, drop]).is_true()


func test_veins_fill_the_grid_deterministically() -> void:
	var veins := _cell_veins(10)   # 400 cells, 3.8 km square
	var share := float(veins.size()) / 400.0
	assert_float(share).is_between(float(reg.grid["fill"]) - 0.1, float(reg.grid["fill"]) + 0.1)
	var names := {}
	for v in veins:
		names[v["name"]] = true
		var d: Dictionary = reg.veins[int(v["type"])]
		assert_int(int(v["y"]) / F).is_between(int(d["y_min"]), int(d["y_max"]))
	assert_int(names.size()).is_equal(reg.count())
	var again := FineGen.new()
	again.configure(1337, S.SEA_LEVEL_VOXEL, S.WORLD_HEIGHT_VOXELS)
	var v0: Dictionary = veins[0]
	assert_dict(again.vein_at(int(v0["x"]), int(v0["z"]))).is_equal(gen.vein_at(int(v0["x"]), int(v0["z"])))


func test_vein_is_a_dense_mixed_ore_lens() -> void:
	# a vein buried well below the ground at its centre; the probe column (16 x the lens thickness x 16
	# fine voxels) spans the whole lens, so all three GregTech layers show
	var vein := {}
	for v in _cell_veins(10):
		if gen.height_at(int(v["x"]), int(v["z"])) - int(v["y"]) > 40 * F and v["name"] != "coal":
			vein = v
			break
	assert_dict(vein).is_not_empty()
	var t := int(vein["thickness"]) * F
	var ids := _block(Vector3i(int(vein["x"]) - 8, int(vein["y"]) - t / 2, int(vein["z"]) - 8), Vector3i(16, t, 16))
	var d: Dictionary = reg.veins[int(vein["type"])]
	var own := []
	for k in OreVeinRegistry.ORE_KEYS:
		own.append(OreVeinRegistry.material_id(String(d[k])))
	var ore := _count(ids, ORES)
	# a share of the rock: big caves (ADR 0052) may hollow part of the box
	var rock := 16 * t * 16 - _count(ids, [0])
	assert_int(rock).is_greater(16 * t * 16 / 2)
	assert_int(ore).override_failure_message("only %d ore voxels in %d of rock in the vein centre" % [ore, rock]).is_greater(rock / 8)
	assert_int(_count(ids, own)).is_equal(ore)   # nothing but the vein's own ores
	assert_int(_count(ids, [OreVeinRegistry.material_id(String(d["primary"]))])).is_greater(0)


func test_no_ore_between_veins() -> void:
	var c := int(reg.grid["cell_voxels"]) * F
	var empty := Vector2i(-1, -1)
	for cz in 10:
		for cx in 10:
			if gen.vein_at(cx * c + c / 2, cz * c + c / 2).is_empty():
				empty = Vector2i(cx * c + c / 2, cz * c + c / 2)
				break
		if empty.x >= 0:
			break
	assert_int(empty.x).is_greater_equal(0)
	var y := mini(gen.height_at(empty.x, empty.y) - 40 * F, 200 * F)
	var ids := _block(Vector3i(empty.x - 8, y - 8, empty.y - 8), Vector3i(16, 16, 16))
	assert_int(_count(ids, ORES)).is_equal(0)


func test_ore_bearing_stones_mark_shallow_veins() -> void:
	var vein := {}
	for v in _cell_veins(10):
		var top := int(v["y"]) + int(v["thickness"]) * F / 2
		var h := gen.height_at(int(v["x"]), int(v["z"]))
		if int(v["indicator"]) > 0 and h - top < 30 * F and h > S.SEA_LEVEL_VOXEL + 4 * F:
			vein = v
			break
	assert_dict(vein).override_failure_message("no shallow vein with indicators near spawn").is_not_empty()
	var ind := int(vein["indicator"])
	var r := int(reg.indicators["radius_voxels"]) * F
	var found := 0
	for bz in range(-r, r, 16):
		for bx in range(-r, r, 16):
			var x := int(vein["x"]) + bx
			var z := int(vein["z"]) + bz
			var h := gen.height_at(x + 8, z + 8)
			found += _count(_block(Vector3i(x, h - 6, z), Vector3i(16, 12, 16)), [ind])
	assert_int(found).override_failure_message("no ore-bearing stones above the %s vein" % vein["name"]).is_greater(3)


func test_starting_pickups_lie_in_forests() -> void:
	var f: Vector3i = gen.find_nearest("forest", 0, 0, 4096)
	assert_int(f.y).is_greater(0)
	var twigs := 0
	var stones := 0
	for bz in range(-128, 128, 16):
		for bx in range(-128, 128, 16):
			var h := gen.height_at(f.x + bx + 8, f.z + bz + 8)
			var ids := _block(Vector3i(f.x + bx, h - 6, f.z + bz), Vector3i(16, 12, 16))
			twigs += _count(ids, [GameMaterials.TWIGS])
			stones += _count(ids, [GameMaterials.PEBBLES, GameMaterials.FLINT_STONES])
	assert_int(twigs).is_greater(5)
	assert_int(stones).is_greater(0)


## A single 12.5 cm voxel cannot be seen or told apart: pickups are Vintage Story style shapes of
## touching voxels (stone chunks filling their 0.5 m patch, sticks 1-1.5 m long). A slope can clip
## the odd one, so the median counts.
func test_pickups_are_big_enough_to_see() -> void:
	var f: Vector3i = gen.find_nearest("plains", 0, 0, 4096)
	var sizes := {}   # material -> cluster sizes
	for bz in range(-128, 128, 32):
		for bx in range(-128, 128, 32):
			var h := gen.height_at(f.x + bx + 16, f.z + bz + 16)
			var o := Vector3i(f.x + bx, h - 16, f.z + bz)
			var ids := _block(o, Vector3i(32, 32, 32))
			var seen := {}
			for i in ids.size():
				var m := ids[i]
				# loose stones come in the local rock's colours (ADR 0057, ids 181..187): all count as pebbles
				var pebble := m == GameMaterials.PEBBLES or (m != GameMaterials.FLINT_STONES and GameMaterials.name_of(m).ends_with("_stones"))
				if not (pebble or m in [GameMaterials.FLINT_STONES, GameMaterials.TWIGS]) or seen.has(i):
					continue
				var cluster := _cluster(ids, i, m)
				if pebble:
					m = GameMaterials.PEBBLES
				seen.merge(cluster)
				# voxel order is y fastest, then x, then z; skip shapes that may run over the block edge
				var x := (i / 32) % 32
				var z := i / 1024
				if x < 4 or x > 27 or z < 4 or z > 27:
					continue
				if not sizes.has(m):
					sizes[m] = []
				sizes[m].append(cluster.size())
	assert_int(sizes.size()).override_failure_message("found %s" % sizes).is_equal(3)
	for m in sizes:
		var s: Array = sizes[m]
		s.sort()
		var median: int = s[s.size() / 2]
		assert_int(median).override_failure_message("%s: median %d voxels of %s" % [GameMaterials.name_of(m), median, s]).is_greater_equal(8)


## Voxels of material m touching index i (26-neighbourhood) in a 32^3 block, as a set.
func _cluster(ids: PackedInt32Array, i: int, m: int) -> Dictionary:
	var seen := {i: true}
	var todo := [i]
	while not todo.is_empty():
		var c: int = todo.pop_back()
		var y := c % 32
		var x := (c / 32) % 32
		var z := c / 1024
		for dz in range(-1, 2):
			for dx in range(-1, 2):
				for dy in range(-1, 2):
					var q := Vector3i(x + dx, y + dy, z + dz)
					if q.x < 0 or q.y < 0 or q.z < 0 or q.x > 31 or q.y > 31 or q.z > 31:
						continue
					var k := q.y + 32 * (q.x + 32 * q.z)
					if not seen.has(k) and ids[k] == m:
						seen[k] = true
						todo.append(k)
	return seen


func test_locate_finds_a_vein() -> void:
	var p: Vector3i = gen.find_nearest("copper_vein", 0, 0, 4096)
	assert_int(p.y).is_greater(0)
	assert_str(String(gen.vein_at(p.x, p.z).get("name", ""))).is_equal("copper")
	assert_int(gen.find_nearest("unobtainium_vein", 0, 0, 4096).y).is_equal(-1)
