class_name RockTable
extends RefCounted

## Rock kinds in the world, their ore bias and the new clutter pieces (ADR 0057), loaded from
## data/rock_types.json and validated. The native generator reads the same data through the
## generated header prototypes/p5-sim-spike/src/rock_table.gen.h (tools/rock_codegen.gd).

const HEADER_PATH := "prototypes/p5-sim-spike/src/rock_table.gen.h"
const VOXELS_PER_M := 8          # ADR 0033: a voxel is 12.5 cm (Settings is an autoload, not here at codegen)
const MAX_LAYERS := 4
const WHERE := ["any", "open", "forest", "beach", "snow"]
const SHAPES := ["strawberry", "nuts", "pine_cone", "bones", "feather", "driftwood", "antler", "shard"]
const ROCKS := ["stone", "granite", "diorite", "limestone", "slate", "marble", "chalk", "basalt"]

var data := {}
var provinces: Array = []        # [name, def]
var clutter: Array = []          # [material name, def]
var errors: Array[String] = []


static func load_default() -> RockTable:
	var t := RockTable.new()
	var f := FileAccess.open("res://data/rock_types.json", FileAccess.READ)
	if f == null:
		t.errors.append("cannot open data/rock_types.json")
		return t
	t.load_json(f.get_as_text())
	return t


func load_json(text: String) -> bool:
	var d = JSON.parse_string(text)
	if typeof(d) != TYPE_DICTIONARY:
		errors.append("rock_types.json: not an object")
		return false
	data = d
	for key in ["province_m", "warp_m", "provinces", "ore_bias", "pyrite", "ochre", "bedding", "clutter", "crystals"]:
		if not data.has(key):
			errors.append("rock_types.json: missing %s" % key)
	if not errors.is_empty():
		return false
	for name in data["provinces"]:
		var p: Dictionary = data["provinces"][name]
		provinces.append([name, p])
		var layers: Array = p.get("layers", [])
		if layers.is_empty() or layers.size() > MAX_LAYERS:
			errors.append("province %s: 1..%d layers" % [name, MAX_LAYERS])
		for i in layers.size():
			var l: Dictionary = layers[i]
			if not String(l.get("rock", "")) in ROCKS:
				errors.append("province %s: unknown rock %s" % [name, l.get("rock", "")])
			if i < layers.size() - 1 and (not l.has("thick_m") or float(l["thick_m"][0]) > float(l["thick_m"][1])):
				errors.append("province %s layer %d: thick_m [min, max]" % [name, i])
		if int(p.get("weight", 0)) <= 0:
			errors.append("province %s: weight > 0" % name)
	var veins := OreVeinRegistry.load_default()
	for v in data["ore_bias"]:
		if not veins.by_name.has(v):
			errors.append("ore_bias: unknown vein %s" % v)
		for r in data["ore_bias"][v]:
			if not r in ROCKS:
				errors.append("ore_bias %s: unknown rock %s" % [v, r])
			elif float(data["ore_bias"][v][r]) <= 0.0:
				errors.append("ore_bias %s/%s: a bias, never 0" % [v, r])
	var biomes := BiomeRegistry.load_default()
	for m in data["clutter"]:
		var c: Dictionary = data["clutter"][m]
		clutter.append([m, c])
		if OreVeinRegistry.material_id(m) <= 0:
			errors.append("clutter: unknown material %s" % m)
		if not String(c.get("shape", "")) in SHAPES:
			errors.append("clutter %s: shape one of %s" % [m, SHAPES])
		if not String(c.get("where", "")) in WHERE:
			errors.append("clutter %s: where one of %s" % [m, WHERE])
		for b in c.get("biomes", []):
			if not biomes.has(b):
				errors.append("clutter %s: unknown biome %s" % [m, b])
	for r in data["pyrite"]["rocks"] + data["ochre"]["rocks"]:
		if not r in ROCKS:
			errors.append("pyrite/ochre: unknown rock %s" % r)
	var partner: Dictionary = data.get("bedding", {}).get("partner", {})
	for r in ROCKS:
		if not partner.has(r) or not String(partner[r]) in ROCKS:
			errors.append("bedding: every rock needs a known partner rock (%s)" % r)
	return errors.is_empty()


## Rock name -> loose stones material (the pickup lying on that rock's ground).
static func loose_of(rock: String) -> String:
	return "pebbles" if rock == "stone" else rock + "_stones"


func generate() -> String:
	var veins := OreVeinRegistry.load_default()
	var biomes := BiomeRegistry.load_default()
	var V := VOXELS_PER_M
	var L := PackedStringArray()
	L.append("// GENERATED from game/data/rock_types.json by game/tools/rock_codegen.gd. Do not edit.")
	L.append("// Rock provinces, strata, ore bias and clutter pieces (ADR 0057). Regenerate:")
	L.append("//   Godot_console.exe --headless --path game --script res://tools/rock_codegen.gd")
	L.append("#pragma once")
	L.append("")
	L.append("namespace technosim_rock {")
	L.append("")
	L.append("static const int PROVINCE_CELL = %d;   // voxels" % int(float(data["province_m"]) * V))
	L.append("static const int WARP = %d;            // voxels" % int(float(data["warp_m"]) * V))
	L.append("static const int MAX_LAYERS = %d;" % MAX_LAYERS)
	L.append("enum Rock : int { %s, NROCK };" % ", ".join(_map(ROCKS, func(r): return "R_" + String(r).to_upper())))
	L.append("static const int ROCK_MAT[NROCK] = {%s};" % ", ".join(_map(ROCKS, func(r): return str(OreVeinRegistry.material_id(r)))))
	L.append("static const int LOOSE_MAT[NROCK] = {%s};" % ", ".join(_map(ROCKS, func(r): return str(OreVeinRegistry.material_id(loose_of(r))))))
	L.append("")
	L.append("static const int NPROV = %d;" % provinces.size())
	var total := 0
	for p in provinces:
		total += int(p[1]["weight"])
	L.append("static const int PROV_WEIGHT_TOTAL = %d;" % total)
	L.append("static const char *const PROV_NAMES[NPROV] = {%s};" % ", ".join(_map(provinces, func(p): return "\"%s\"" % p[0])))
	L.append("static const int PROV_WEIGHT[NPROV] = {%s};" % ", ".join(_map(provinces, func(p): return str(int(p[1]["weight"])))))
	L.append("static const int PROV_TOP[NPROV] = {%s};      // top boundary above sea level (voxels)" % ", ".join(_map(provinces, func(p): return str(int(float(p[1].get("top_m", 8)) * V)))))
	L.append("static const float PROV_FOLLOW[NPROV] = {%s}; // share of the land's height the boundaries rise with" % ", ".join(_map(provinces, func(p): return _f(float(p[1].get("follow", 0.6))))))
	L.append("static const float PROV_DIP[NPROV] = {%s};" % ", ".join(_map(provinces, func(p): return _f(float(p[1].get("dip", 0.05))))))
	L.append("static const float PROV_WOBBLE[NPROV] = {%s};  // voxels" % ", ".join(_map(provinces, func(p): return _f(float(p[1].get("wobble_m", 2.0)) * V))))
	L.append("static const int PROV_NLAYERS[NPROV] = {%s};" % ", ".join(_map(provinces, func(p): return str((p[1]["layers"] as Array).size()))))
	var rock_rows := PackedStringArray()
	var tmin_rows := PackedStringArray()
	var tmax_rows := PackedStringArray()
	for p in provinces:
		var r := PackedStringArray()
		var a := PackedStringArray()
		var b := PackedStringArray()
		var layers: Array = p[1]["layers"]
		for i in MAX_LAYERS:
			var l: Dictionary = layers[mini(i, layers.size() - 1)]
			r.append("R_" + String(l["rock"]).to_upper())
			var t: Array = l.get("thick_m", [0, 0])
			a.append(str(int(float(t[0]) * V)))
			b.append(str(int(float(t[1]) * V)))
		rock_rows.append("{%s}" % ", ".join(r))
		tmin_rows.append("{%s}" % ", ".join(a))
		tmax_rows.append("{%s}" % ", ".join(b))
	L.append("static const int LAYER_ROCK[NPROV][MAX_LAYERS] = {%s};" % ", ".join(rock_rows))
	L.append("static const int THICK_MIN[NPROV][MAX_LAYERS] = {%s};  // voxels" % ", ".join(tmin_rows))
	L.append("static const int THICK_MAX[NPROV][MAX_LAYERS] = {%s};" % ", ".join(tmax_rows))
	L.append("")
	L.append("// ORE_BIAS[rock][vein]: factor on the vein's nugget density in that host rock")
	L.append("static const int NVEIN = %d;" % veins.count())
	var bias_rows := PackedStringArray()
	for r in ROCKS:
		var row := PackedStringArray()
		for v in veins.veins:
			row.append(_f(float(data["ore_bias"].get(v["name"], {}).get(r, 1.0))))
		bias_rows.append("{%s}" % ", ".join(row))
	L.append("static const float ORE_BIAS[NROCK][NVEIN] = {%s};" % ", ".join(bias_rows))
	L.append("")
	L.append("static const int PYRITE = %d, RED_OCHRE = %d, YELLOW_OCHRE = %d, CAVE_CRYSTAL = %d;" % [
		OreVeinRegistry.material_id("pyrite"), OreVeinRegistry.material_id("red_ochre"), OreVeinRegistry.material_id("yellow_ochre"), OreVeinRegistry.material_id("cave_crystal")])
	L.append("static const int CLAY_POT = %d, CLAY_POT_CRACKED = %d;" % [OreVeinRegistry.material_id("clay_pot"), OreVeinRegistry.material_id("clay_pot_cracked")])
	L.append("static const int PYRITE_PER_MILLE = %d;" % int(data["pyrite"]["per_mille"]))
	L.append("static const unsigned PYRITE_ROCKS = %du;  // bit per Rock" % _mask(data["pyrite"]["rocks"]))
	L.append("static const unsigned OCHRE_ROCKS = %du;" % _mask(data["ochre"]["rocks"]))
	L.append("static const float OCHRE_SHARE = %s;" % _f(float(data["ochre"]["share"])))
	var bed: Dictionary = data["bedding"]
	L.append("static const int BED_PARTNER[NROCK] = {%s};" % ", ".join(_map(ROCKS, func(r): return "R_" + String(bed["partner"][r]).to_upper())))
	L.append("static const int BED_BAND = %d;   // voxels" % int(float(bed["band_m"]) * V))
	L.append("static const float BED_SHARE = %s;" % _f(float(bed["share"])))
	L.append("")
	L.append("enum Where : int { %s };" % ", ".join(_map(WHERE, func(w): return "W_" + String(w).to_upper())))
	L.append("enum Shape : int { %s };" % ", ".join(_map(SHAPES, func(s): return "S_" + String(s).to_upper())))
	L.append("static const int NCLUT = %d;" % clutter.size())
	L.append("static const int CLUT_MAT[NCLUT] = {%s};" % ", ".join(_map(clutter, func(c): return str(OreVeinRegistry.material_id(c[0])))))
	L.append("static const int CLUT_SHAPE[NCLUT] = {%s};" % ", ".join(_map(clutter, func(c): return "S_" + String(c[1]["shape"]).to_upper())))
	L.append("static const int CLUT_WHERE[NCLUT] = {%s};" % ", ".join(_map(clutter, func(c): return "W_" + String(c[1]["where"]).to_upper())))
	L.append("static const int NBIOME = %d;" % biomes.count())
	var ch_rows := PackedStringArray()
	for c in clutter:
		var row := PackedStringArray()
		for b in biomes.count():
			row.append(_f(float(c[1]["chance"]) if String(biomes.biomes[b]["name"]) in c[1].get("biomes", []) else 0.0))
		ch_rows.append("{%s}" % ", ".join(row))
	L.append("static const float CLUT_CHANCE[NCLUT][NBIOME] = {%s};" % ", ".join(ch_rows))
	L.append("")
	var cr: Dictionary = data["crystals"]
	L.append("static const int CRYSTAL_PER_MILLE = %d, CRYSTAL_CAVE_PER_MILLE = %d;" % [int(cr["per_mille"]), int(cr["crystal_cave_per_mille"])])
	L.append("static const int CRYSTAL_MIN_DEPTH = %d;  // voxels" % int(float(cr["min_depth_m"]) * V))
	L.append("static const int CRYSTAL_CAVE_BIOME = %d;" % int(biomes.by_name.get("crystal_cave", -1)))
	L.append("")
	L.append("} // namespace technosim_rock")
	L.append("")
	return "\n".join(L)


static func _mask(rocks: Array) -> int:
	var m := 0
	for r in rocks:
		m |= 1 << ROCKS.find(r)
	return m


static func _map(a: Array, f: Callable) -> PackedStringArray:
	var out := PackedStringArray()
	for x in a:
		out.append(f.call(x))
	return out


static func _f(x: float) -> String:
	var s := String.num(x, 6)
	if not "." in s:
		s += ".0"
	return s + "f"
