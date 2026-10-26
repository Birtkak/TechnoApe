extends GdUnitTestSuite

## Ruin kinds (ADR 0062): the data and its generated header agree, every kind is found and stands on level
## ground (no plinth, no cut into a slope, nothing hanging in the air), holds 1..4 pots (never more than its
## kind allows, never side by side, no pot voxel anywhere else) and its pots roll the kind's own loot table.

const S := preload("res://scripts/settings.gd")
const SEEDS := [1866545922, 1579829108, 1337]   # the landforms suite's, the 2026-10-25 bug report's, the default
const FAR := 64000
const FS := 4

var gen: FineGen
var gens := {}      # seed -> FineGen
var table: RuinTable
var found := []     # {seed, kind, r = ruin_at dictionary (fine voxels)}: the nearest of each kind per seed


func before() -> void:
	table = RuinTable.load_default()
	for sd in SEEDS:
		var g := FineGen.new()
		g.configure(sd, S.SEA_LEVEL_VOXEL, S.WORLD_HEIGHT_VOXELS)
		gens[sd] = g
		for k in table.kinds:
			var p: Vector3i = g.find_nearest("ruin_" + String(k[0]), 0, 0, FAR)
			if p.y >= 0:
				found.append({"seed": sd, "kind": String(k[0]), "r": g.ruin_at(p.x, p.z)})
	gen = gens[SEEDS[0]]


func _use(e: Dictionary) -> void:
	gen = gens[e["seed"]]


func _block(org: Vector3i, size: Vector3i) -> PackedInt32Array:
	var b: PackedByteArray = gen.generate_block(org, size, 0, 2)
	var out := PackedInt32Array()
	out.resize(size.x * size.y * size.z)
	if b.is_empty():
		return out
	for i in out.size():
		out[i] = b.decode_u16(i * 2)
	return out


## The voxels of a ruin's region (footprint + 2 cells, its whole reach): [origin, size, ids].
func _region(r: Dictionary) -> Array:
	var k: Dictionary = table.kinds[table.by_name[r["kind"]]][1]
	var hx := int(r["half_x"]) + 3 * FS
	var hz := int(r["half_z"]) + 3 * FS
	var y0 := int(r["y"]) - int(round(float(k.get("depth_m", 0.0)) * 2.0 + 2.0)) * FS
	var y1 := int(r["y"]) + int(round(float(k["top_m"]) * 2.0 + 2.0)) * FS
	var org := Vector3i(int(r["x"]) - 2 - hx, y0, int(r["z"]) - 2 - hz)
	var size := Vector3i(2 * hx + FS, y1 - y0, 2 * hz + FS)
	return [org, size, _block(org, size)]


static func _at(reg: Array, p: Vector3i) -> int:
	var o: Vector3i = reg[0]
	var s: Vector3i = reg[1]
	var q := p - o
	if q.x < 0 or q.y < 0 or q.z < 0 or q.x >= s.x or q.y >= s.y or q.z >= s.z:
		return -1
	return (reg[2] as PackedInt32Array)[q.y + s.y * (q.x + s.x * q.z)]


func test_data_loads_and_the_header_is_current() -> void:
	assert_array(table.errors).is_empty()
	assert_int(table.kinds.size()).is_greater_equal(7)
	var path := ProjectSettings.globalize_path("res://").path_join("..").path_join(RuinTable.HEADER_PATH)
	var f := FileAccess.open(path, FileAccess.READ)
	assert_object(f).override_failure_message("missing %s; run tools/ruin_codegen.gd" % path).is_not_null()
	if f != null:
		assert_str(f.get_as_text().replace("\r\n", "\n")).override_failure_message("header stale; run tools/ruin_codegen.gd").is_equal(table.generate())


func test_bad_data_is_rejected() -> void:
	var t := RuinTable.new()
	t.load_json(JSON.stringify({"kinds": {"x": {"shape": "pyramid", "weight": 0, "biomes": ["moon"], "half_u_m": [4, 9],
		"top_m": 20, "wall": "mithril", "trim": "stone", "floor": "stone", "extra": "stone", "pots": 6, "loot": "nope"}}}))
	assert_int(t.errors.size()).is_greater_equal(7)


func test_every_kind_is_found() -> void:
	for sd in SEEDS:
		for k in table.kinds:
			var hit := found.filter(func(e): return e["seed"] == sd and e["kind"] == k[0])
			assert_int(hit.size()).override_failure_message("seed %d: no %s ruin within 8 km" % [sd, k[0]]).is_equal(1)
			for e in hit:
				assert_str(String(e["r"].get("kind", ""))).is_equal(String(k[0]))


func test_ruins_stand_on_level_ground() -> void:
	for e in found:
		_use(e)
		var kind: String = e["kind"]
		var r: Dictionary = e["r"]
		var base := int(r["y"])
		var cx := int(r["x"]) - 2
		var cz := int(r["z"]) - 2
		var hx := int(r["half_x"])
		var hz := int(r["half_z"])
		# the floor is level: every column inside the footprint stands at the ruin's floor
		var off := 0
		for x in range(cx - hx + FS, cx + hx + FS, FS):
			for z in range(cz - hz + FS, cz + hz + FS, FS):
				if gen.height_at(x, z) != base:
					off += 1
		assert_int(off).override_failure_message("%s: %d footprint columns off its floor" % [kind + "@" + str(e["seed"]), off]).is_equal(0)
		# round it the ground meets the floor: within a cell up or down per cell out (no plinth, no cut)
		var bad := 0
		var first := ""
		for d in range(1, FineGen.RUIN_RAMP + 1):
			var ring := []
			for side in [-1, 1]:
				for i in range(-hx - d * FS, hx + d * FS + 1, FS):
					ring.append(Vector2i(cx + i, cz + side * (hz + d * FS)))
				for i in range(-hz - d * FS, hz + d * FS + 1, FS):
					ring.append(Vector2i(cx + side * (hx + d * FS), cz + i))
			for p in ring:
				var h := gen.height_at(p.x, p.y)
				if absi(h - base) > d * FS + 3:
					bad += 1
					first = "%s d=%d off %d" % [p, d, h - base] if first == "" else first
		assert_int(bad).override_failure_message("%s: %d ring columns step more than the ramp allows (%s)" % [kind + "@" + str(e["seed"]), bad, first]).is_equal(0)


func test_nothing_hangs_in_the_air() -> void:
	# every voxel of a ruin's blocks is joined (face to face, through anything solid) to the ground under
	# it or to the land round it: no stone, beam, plank or pot floats
	for e in found:
		_use(e)
		var kind: String = e["kind"]
		var reg := _region(e["r"])
		var s: Vector3i = reg[1]
		var ids: PackedInt32Array = reg[2]
		var seen := PackedByteArray()
		seen.resize(ids.size())
		var stack := PackedInt32Array()
		for x in s.x:
			for z in s.z:
				for y in s.y:
					var edge := y == 0 or y == s.y - 1 or x == 0 or z == 0 or x == s.x - 1 or z == s.z - 1
					var i := y + s.y * (x + s.x * z)
					if edge and ids[i] != 0:
						seen[i] = 1
						stack.append(i)
		var steps := [1, -1, s.y, -s.y, s.y * s.x, -s.y * s.x]
		while not stack.is_empty():
			var i: int = stack[stack.size() - 1]
			stack.resize(stack.size() - 1)
			var y := i % s.y
			var x := (i / s.y) % s.x
			var z := i / (s.y * s.x)
			for k in 6:
				if (k == 0 and y == s.y - 1) or (k == 1 and y == 0) or (k == 2 and x == s.x - 1) or (k == 3 and x == 0) or (k == 4 and z == s.z - 1) or (k == 5 and z == 0):
					continue
				var j: int = i + steps[k]
				if seen[j] == 0 and ids[j] != 0:
					seen[j] = 1
					stack.append(j)
		# the ruin's own blocks (a tree's crown reaching in from outside the region is not its business)
		var k: Dictionary = table.kinds[table.by_name[kind]][1]
		var mine := {GameMaterials.id_of("clay_pot"): true, GameMaterials.id_of("clay_pot_cracked"): true}
		for m in RuinTable.MATS:
			mine[GameMaterials.id_of(String(k[m]))] = true
		mine.erase(GameMaterials.id_of("leaves"))   # a shrine's leaves are an oak's too: crowns leave loose bits
		var floating := 0
		var what := {}
		var r: Dictionary = e["r"]
		var o: Vector3i = reg[0]
		for i in ids.size():
			if ids[i] != 0 and seen[i] == 0 and mine.has(ids[i]):
				# inside the footprint (round it, leaves and moss may be a tree's crown from outside)
				var wx: int = o.x + (i / s.y) % s.x
				var wz: int = o.z + i / (s.y * s.x)
				if absi(floori(wx / float(FS)) - (int(r["x"]) - 2) / FS) > int(r["half_x"]) / FS or absi(floori(wz / float(FS)) - (int(r["z"]) - 2) / FS) > int(r["half_z"]) / FS:
					continue
				floating += 1
				var nm := String(GameMaterials.defs()[ids[i] - 1]["name"])
				what[nm] = what.get(nm, 0) + 1
				if not what.has("at"):
					what["at"] = (reg[0] as Vector3i) + Vector3i((i / s.y) % s.x, i % s.y, i / (s.y * s.x))
		assert_int(floating).override_failure_message("%s: %d voxels float %s (seed %d, ruin %s)" % [kind + "@" + str(e["seed"]), floating, what, e["seed"], e["r"]]).is_less_equal(6)


func test_pots_are_few_apart_and_only_at_their_spots() -> void:
	var clay := [GameMaterials.id_of("clay_pot"), GameMaterials.id_of("clay_pot_cracked")]
	for e in found:
		_use(e)
		var kind: String = e["kind"]
		var r: Dictionary = e["r"]
		var spots: Array = gen.ruin_pots_near(int(r["x"]), int(r["z"]), int(S.m2v(12.0)))
		var mine := spots.filter(func(d): return String(d["kind"]) == kind and absi(int(d["x"]) - int(r["x"])) <= int(r["half_x"]) + 8 and absi(int(d["z"]) - int(r["z"])) <= int(r["half_z"]) + 8)
		var limit := int(table.kinds[table.by_name[kind]][1]["pots"])
		assert_int(mine.size()).override_failure_message("%s: %d pots" % [kind + "@" + str(e["seed"]), mine.size()]).is_between(1, mini(4, limit))
		for a in mine:
			for b in mine:
				if a != b:
					assert_int(maxi(absi(int(a["x"]) - int(b["x"])), absi(int(a["z"]) - int(b["z"])))).is_greater_equal(2 * FS)
		var reg := _region(r)
		var o: Vector3i = reg[0]
		var s: Vector3i = reg[1]
		var ids: PackedInt32Array = reg[2]
		var cells := {}
		for d in mine:
			cells[Vector2i(int(d["x"]) - 2, int(d["z"]) - 2)] = int(d["y"])
		var stray := 0
		var at_spot := {}
		for x in s.x:
			for z in s.z:
				for y in s.y:
					var m := ids[y + s.y * (x + s.x * z)]
					if not m in clay:
						continue
					var w := Vector3i(o.x + x, o.y + y, o.z + z)
					var cell := Vector2i(floori(w.x / float(FS)) * FS, floori(w.z / float(FS)) * FS)
					if cells.has(cell) and w.y >= int(cells[cell]) and w.y < int(cells[cell]) + 7:
						at_spot[cell] = true
					else:
						stray += 1
		assert_int(stray).override_failure_message("%s: %d pot voxels away from the spots" % [kind + "@" + str(e["seed"]), stray]).is_equal(0)
		assert_int(at_spot.size()).override_failure_message("%s: pots at %d of %d spots" % [kind + "@" + str(e["seed"]), at_spot.size(), cells.size()]).is_equal(cells.size())
		# a pot stands on something
		for cell in cells:
			var under := _at(reg, Vector3i(cell.x + 1, int(cells[cell]) - 1, cell.y + 1))
			assert_int(under).override_failure_message("%s: pot at %s over air" % [kind + "@" + str(e["seed"]), cell]).is_greater(0)


func test_pots_roll_their_kinds_loot() -> void:
	for e in found:
		_use(e)
		var kind: String = e["kind"]
		var r: Dictionary = e["r"]
		var want := table.loot_of(kind)
		assert_bool(RuinLoot.tables().has(want)).is_true()
		assert_str(RuinLoot.table_at("ruin_pot", gen, Vector3i(int(r["x"]), int(r["y"]) + 4, int(r["z"])))).is_equal(want)
	gen = gens[SEEDS[0]]
	# away from every ruin a pot keeps its own table (one a player set down), and other tables pass through
	assert_str(RuinLoot.table_at("ruin_pot", gen, Vector3i(-999999, 0, -999999))).is_equal("ruin_pot")
	assert_str(RuinLoot.table_at("deep_pot", gen, Vector3i(0, 0, 0))).is_equal("deep_pot")
	var items := ItemRegistry.load_default()
	for k in table.kinds:
		var n := 0
		for i in 200:
			n += RuinLoot.roll(table.loot_of(k[0]), 7, Vector3i(i, 1000, 5), 0.0, items).size()
		assert_int(n).override_failure_message("%s: loot almost always empty" % k[0]).is_greater(100)
