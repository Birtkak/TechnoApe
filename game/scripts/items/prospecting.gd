class_name Prospecting
extends RefCounted

## The prospecting pick (ADR 0061). Right click on rock with a pick whose items.json def has
## "prospects": {range_m, exact}: the generator's ore veins (FineGen.vein_at, one per vein cell, and
## data/ore_veins.json for what a vein holds) within range_m of the tapped rock.
##   - exact (copper): every vein, its share of each ore in the lens and its centre, nearest first;
##     each find becomes a map waypoint (part D's Waypoints) and a discovery;
##   - not exact (flint, a Flint Age teaser): only "something" or "nothing".
## A vein is a flat lens size x thickness around its centre; distance is from the tap to the lens.
## No per-voxel work: a tap reads the few vein cells that can reach it.
## Settings (ADR 0091, Shift + right click opens ProspectMenu): the search distance (up to the pick's
## range_m), one ore to look for, and what a tap shows: the nearest vein, every vein in range, or a
## density map of the square around the tap (row 0 north) the menu draws. The flint pick only takes
## the distance: it cannot tell ores apart.

const WAYPOINT_COLOUR := Color(0.95, 0.7, 0.25)
const RANGES_M := [12.0, 24.0, 48.0, 96.0]
const MODES := {"nearest": "Nearest vein", "all": "Every vein in range", "map": "Density map", "density": "Ore in the rock below"}
const MAP_N := 9   # density map cells per side

var veins := {}              # vein name -> ore_veins.json entry
var density := 0.6
var sporadic := 0.12
var cell_v := 768            # fine voxels per vein cell
var max_size_v := 320
var errors: Array[String] = []
var last := {}               # the last reading (snapshot, tests)
var marked := {}             # "x,y,z" of vein centres already put on the map
var taps := 0
var range_m := 0.0           # the chosen search distance; 0 = the pick's whole range
var ore := ""                # only veins holding this ore material; "" = any
var mode := "all"            # a key of MODES
var map := PackedFloat32Array()   # the last density map, MAP_N x MAP_N shares of the ore, row 0 north
var map_range_m := 0.0
var _st := ShapeTool.new()


static func load_default() -> Prospecting:
	var p := Prospecting.new()
	var fa := FileAccess.open("res://data/ore_veins.json", FileAccess.READ)
	if fa == null:
		p.errors.append("cannot open ore_veins.json")
		return p
	var d = JSON.parse_string(fa.get_as_text())
	p.load_data(d if d is Dictionary else {})
	return p


func load_data(d: Dictionary) -> void:
	var g: Dictionary = d.get("grid", {})
	density = float(g.get("density", 0.6))
	sporadic = float(g.get("sporadic_chance", 0.12))
	cell_v = int(g.get("cell_voxels", 192)) * FineGen.FS
	max_size_v = int(g.get("size_voxels", 64)) * FineGen.FS
	for v in d.get("veins", []):
		veins[String(v["name"])] = v
		max_size_v = maxi(max_size_v, int(v.get("size_voxels", g.get("size_voxels", 64))) * FineGen.FS)


## What a vein holds: ore material name -> share of the lens (0..1). The top, middle and bottom
## thirds hold primary, between and secondary ore at the grid density; any ore voxel is the sporadic
## ore at sporadic_chance.
func composition(vein_name: String) -> Dictionary:
	var v: Dictionary = veins.get(vein_name, {})
	var out := {}
	if v.is_empty():
		return out
	for k in ["primary", "between", "secondary"]:
		var m := String(v.get(k, ""))
		if m != "":
			out[m] = float(out.get(m, 0.0)) + density * (1.0 - sporadic) / 3.0
	var s := String(v.get("sporadic", ""))
	if s != "":
		out[s] = float(out.get(s, 0.0)) + density * sporadic
	return out


## Veins whose lens comes within range_v voxels of pos: [{name, centre, dist_m, ores}], nearest first.
## vein_at is FineGen.vein_at (x, z) -> {name, x, y, z, size, thickness} or {} (tests pass a fake).
func veins_near(vein_at: Callable, pos: Vector3i, range_v: float) -> Array:
	return _lenses(vein_at, pos, range_v, false)


## The lenses near pos: within range_v of it (3D), or with `ground` across the ground at any depth.
func _lenses(vein_at: Callable, pos: Vector3i, range_v: float, ground: bool) -> Array:
	var out := []
	var r := range_v + max_size_v / 2.0
	for cx in range(floori((pos.x - r) / cell_v), floori((pos.x + r) / cell_v) + 1):
		for cz in range(floori((pos.z - r) / cell_v), floori((pos.z + r) / cell_v) + 1):
			var d: Dictionary = vein_at.call(cx * cell_v + 1, cz * cell_v + 1)
			if d.is_empty() or not d.has("x"):
				continue
			var c := Vector3i(int(d["x"]), int(d["y"]), int(d["z"]))
			var size_v := float(d.get("size", 64)) * FineGen.FS
			var thick_v := float(d.get("thickness", 16)) * FineGen.FS
			var dh := maxf(0.0, Vector2(pos.x - c.x, pos.z - c.z).length() - size_v / 2.0)
			var dv := maxf(0.0, absf(pos.y - c.y) - thick_v / 2.0)
			var dist := sqrt(dh * dh + dv * dv)
			if (dh if ground else dist) <= range_v:
				out.append({"name": String(d["name"]), "centre": c, "dist_m": Settings.v2m(dist), "ores": composition(String(d["name"])), "size_v": size_v})
	out.sort_custom(func(a: Dictionary, b: Dictionary) -> bool: return a["dist_m"] < b["dist_m"])
	return out


## Every ore any vein holds, sorted (the menu's ore choices).
func ore_names() -> Array:
	var out := {}
	for v in veins:
		for m in composition(v):
			out[m] = true
	var names := out.keys()
	names.sort()
	return names


## The distance a tap searches with a pick's "prospects" def: the chosen one, capped by the pick.
func reach_m(prospects: Dictionary) -> float:
	var top := float(prospects.get("range_m", 12.0))
	return minf(range_m, top) if range_m > 0.0 else top


## What a share means for the chosen ore: its part of the lens, or every ore's with no ore chosen.
func _share(f: Dictionary) -> float:
	var ores: Dictionary = f["ores"]
	if ore != "":
		return float(ores.get(ore, 0.0))
	var t := 0.0
	for m in ores:
		t += float(ores[m])
	return t


## The density map around pos: MAP_N x MAP_N cells over the square range_v around it, each the
## biggest share of the chosen ore among the lenses over that cell (any depth), row 0 north (-z).
func density_map(vein_at: Callable, pos: Vector3i, range_v: float) -> PackedFloat32Array:
	var out := PackedFloat32Array()
	out.resize(MAP_N * MAP_N)
	var lenses := _lenses(vein_at, pos, range_v * sqrt(2.0), true)
	var cell := 2.0 * range_v / MAP_N
	for row in MAP_N:
		for col in MAP_N:
			var p := Vector2(pos.x - range_v + (col + 0.5) * cell, pos.z - range_v + (row + 0.5) * cell)
			var best := 0.0
			for f in lenses:
				var c: Vector3i = f["centre"]
				if p.distance_to(Vector2(c.x, c.z)) <= float(f["size_v"]) / 2.0:
					best = maxf(best, _share(f))
			out[row * MAP_N + col] = best
	return out



## A reading at pos with a pick's "prospects" def and the chosen settings: {found, veins, lines}.
## `census` (density mode): pos -> {material id: voxels} of the rock column below pos.
func read(vein_at: Callable, pos: Vector3i, prospects: Dictionary, census: Callable = Callable()) -> Dictionary:
	var range_m := reach_m(prospects)
	var exact := bool(prospects.get("exact", false))
	if exact and mode == "density" and census.is_valid():
		return _read_density(pos, range_m, census)
	var found := veins_near(vein_at, pos, Settings.m2v(range_m))
	if exact and ore != "":
		found = found.filter(func(f: Dictionary) -> bool: return float(f["ores"].get(ore, 0.0)) > 0.0)
	if exact and mode == "nearest" and found.size() > 1:
		found = found.slice(0, 1)
	map = PackedFloat32Array()
	if exact and mode == "map":
		map = density_map(vein_at, pos, Settings.m2v(range_m))
		map_range_m = range_m
	var what := "ore" if ore == "" or not exact else String(ore).capitalize()
	var lines: Array[String] = []
	if found.is_empty():
		lines.append("No %s in the rock within %d m." % [what, roundi(range_m)] if what != "ore" else "Nothing in the rock within %d m." % roundi(range_m))
	elif not exact:
		lines.append("Something lies in the rock within %d m." % roundi(range_m))
	else:
		for f in found:
			lines.append(describe(f, pos))
	last = {"pos": [pos.x, pos.y, pos.z], "exact": exact, "range_m": range_m, "found": found.size(), "mode": mode, "ore": ore,
		"veins": found.map(func(f: Dictionary) -> String: return f["name"]) if exact else [], "lines": lines}
	return {"found": not found.is_empty(), "veins": found if exact else [], "lines": lines}


func _read_density(pos: Vector3i, range_m: float, census: Callable) -> Dictionary:
	map = PackedFloat32Array()
	var shares := ore_shares(census.call(pos))
	var what := "ore" if ore == "" else String(ore).capitalize()
	var where := "in the rock below (%d m wide, %d m down)" % [roundi(Settings.prospect_width_m), roundi(Settings.prospect_depth_m)]
	var line: String
	if shares.has("rock") and shares["rock"] <= 0.0:
		line = "No rock below."
	elif shares.size() <= 1:
		line = "No %s %s." % [what, where]
	else:
		var names := shares.keys()
		names.erase("rock")
		names.sort_custom(func(a: String, b: String) -> bool: return shares[a] > shares[b])
		line = "%s %s: %s" % [what.capitalize(), where, ", ".join(names.map(func(m: String) -> String: return "%s %.1f%%" % [m.capitalize().replace("_", " "), shares[m] * 100.0]))]
	last = {"pos": [pos.x, pos.y, pos.z], "exact": true, "range_m": range_m, "found": maxi(0, shares.size() - 1), "mode": mode, "ore": ore,
		"veins": [], "shares": shares, "lines": [line]}
	return {"found": shares.size() > 1, "veins": [], "lines": [line]}


## Ore -> share (0..1) of the rock among `counts` (material id -> rock voxels), plus "rock": the rock voxel
## total. Only ores some vein holds count, and only the chosen one when `ore` is set.
func ore_shares(counts: Dictionary) -> Dictionary:
	var total := 0
	for id in counts:
		total += int(counts[id])
	var out := {"rock": total}
	var ores := ore_names()
	for id in counts:
		var n := GameMaterials.name_of(int(id))
		if total > 0 and n in ores and (ore == "" or ore == n):
			out[n] = float(out.get(n, 0.0)) + float(counts[id]) / total
	return out


## The rock column under `target`, counted natively: {material id: voxels} of the voxels `is_rock` accepts.
func column_census(gen: FineGen, st: ShapeTool, target: Vector3i, is_rock: Callable) -> Dictionary:
	var w := maxi(1, Settings.m2v(Settings.prospect_width_m))
	var y0 := maxi(0, target.y + 1 - Settings.m2v(Settings.prospect_depth_m))
	var h := target.y + 1 - y0
	var bytes: PackedByteArray = gen.generate_block(Vector3i(target.x - w / 2, y0, target.z - w / 2), Vector3i(w, h, w), 0, 2)
	var counts := {}
	if bytes.size() == 2:   # one material all through (deep rock)
		counts[bytes.decode_u16(0)] = w * w * h
	elif not bytes.is_empty():
		counts = st.tally(bytes)
	for id in counts.keys():
		if not is_rock.call(int(id)):
			counts.erase(id)
	return counts


## One exact find as a line: "Copper vein 9 m away, centre 120 980 -55 (14 m down): Copper Ore 27%, ..."
static func describe(f: Dictionary, from: Vector3i) -> String:
	var c: Vector3i = f["centre"]
	var ores: Dictionary = f["ores"]
	var names := ores.keys()
	names.sort_custom(func(a: String, b: String) -> bool: return ores[a] > ores[b])
	var parts := []
	for m in names:
		parts.append("%s %d%%" % [String(m).capitalize(), roundi(float(ores[m]) * 100.0)])
	var dy := Settings.v2m(from.y - c.y)
	var depth := ("%d m down" % roundi(dy)) if dy >= 0.5 else (("%d m up" % roundi(-dy)) if dy <= -0.5 else "level")
	return "%s vein %d m away, centre %d %d %d (%s): %s" % [vein_title(f["name"]), roundi(float(f["dist_m"])), c.x, c.y, c.z, depth, ", ".join(parts)]


static func vein_title(vein_name: String) -> String:
	return vein_name.replace("_", " & ").capitalize().replace(" & ", "-")


## A right click with the held pick on `hit` (a VoxelRaycastResult). True when it tapped rock.
func use(main: Node, hit: Object, def: Dictionary, journal: Discoveries = null) -> bool:
	if hit == null:
		return false
	return use_at(main, hit.position, def, journal)


## A tap on the voxel at `target` with a pick def (the /prospect command calls this directly).
func use_at(main: Node, target: Vector3i, def: Dictionary, journal: Discoveries = null) -> bool:
	if not def.has("prospects"):
		return false
	var m: int = main.world.get_voxel(target)
	if m <= 0 or main.mining == null or String(main.mining.entry(GameMaterials.name_of(m)).get("tool", "")) != "pickaxe":
		return false   # only rock answers the pick
	taps += 1
	var gen: FineGen = main.world.generator.gen
	var is_rock := func(id: int) -> bool: return String(main.mining.entry(GameMaterials.name_of(id)).get("tool", "")) == "pickaxe"
	var census := func(p: Vector3i) -> Dictionary: return column_census(gen, _st, p, is_rock)
	var r := read(func(x: int, z: int) -> Dictionary: return gen.vein_at(x, z), target, def["prospects"], census)
	for l in r["lines"]:
		main.chat.add_message(l)
	for f in r["veins"]:
		var c: Vector3i = f["centre"]
		var key := "%d,%d,%d" % [c.x, c.y, c.z]
		if journal != null:
			journal.note("vein:" + String(f["name"]))
		if marked.has(key):
			continue
		marked[key] = true
		var wp: Variant = main.get("waypoints")
		if wp != null and wp.has_method("add"):
			wp.add("%s vein" % vein_title(f["name"]), c, "prospect", WAYPOINT_COLOUR)
	if GameMode.rule(main.player.game_mode, "consumes_items") and main.player_inv.held_def().get("id", "") == def.get("id", ""):
		main.player_inv.wear_held(1)   # one use of the pick per tap
	Sound.play("craft/knap", Vector3(target) + Vector3(0.5, 0.5, 0.5))
	return true


func to_dict() -> Dictionary:
	return {"marked": marked.keys(), "range_m": range_m, "ore": ore, "mode": mode}


func load_dict(d: Dictionary) -> void:
	range_m = float(d.get("range_m", 0.0))
	ore = String(d.get("ore", ""))
	mode = String(d.get("mode", "all")) if MODES.has(String(d.get("mode", "all"))) else "all"
	marked.clear()
	for k in d.get("marked", []):
		marked[String(k)] = true
