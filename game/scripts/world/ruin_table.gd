class_name RuinTable
extends RefCounted

## Ruin kinds (ADR 0062), loaded from data/ruins.json and validated. The native generator reads the
## same data through the generated header prototypes/p5-sim-spike/src/ruin_table.gen.h
## (tools/ruin_codegen.gd); the game reads it here for each kind's loot table.

const HEADER_PATH := "prototypes/p5-sim-spike/src/ruin_table.gen.h"
const MACRO_PER_M := 2           # the generator lays ruins out in 0.5 m macro cells (FineGen::FS voxels)
const MAX_HALF_M := 7.0          # WorldGen::RUIN_MAX_HALF macro cells
const MAX_TOP_M := 8.0
const MAX_DEPTH_M := 4.5
const MAX_POTS := 4              # the developer's limit (2026-10-25 bug report)
const SHAPES := ["house", "watchtower", "stone_circle", "crypt", "mud_house", "shrine", "wreck", "mine"]
const MATS := ["wall", "trim", "floor", "extra"]

var data := {}
var kinds: Array = []            # [name, def]
var by_name := {}                # name -> index (the generator's kind id)
var errors: Array[String] = []


static var _default: RuinTable


static func load_default() -> RuinTable:
	var t := RuinTable.new()
	var f := FileAccess.open("res://data/ruins.json", FileAccess.READ)
	if f == null:
		t.errors.append("cannot open data/ruins.json")
		return t
	t.load_json(f.get_as_text())
	return t


## The shared default table (loaded once).
static func get_default() -> RuinTable:
	if _default == null:
		_default = load_default()
	return _default


## The loot table the pots of ruin kind `kind` roll, or "".
func loot_of(kind: String) -> String:
	return String(kinds[by_name[kind]][1].get("loot", "")) if by_name.has(kind) else ""


func load_json(text: String) -> bool:
	var d = JSON.parse_string(text)
	if typeof(d) != TYPE_DICTIONARY or not (d as Dictionary).has("kinds"):
		errors.append("ruins.json: not an object with kinds")
		return false
	data = d
	var biomes := BiomeRegistry.load_default()
	var loot := {}
	var lf = JSON.parse_string(FileAccess.get_file_as_string("res://data/loot.json"))
	if lf is Dictionary:
		loot = lf.get("tables", {})
	for name in data["kinds"]:
		var k: Dictionary = data["kinds"][name]
		by_name[name] = kinds.size()
		kinds.append([name, k])
		if not String(k.get("shape", "")) in SHAPES:
			errors.append("ruin %s: unknown shape %s" % [name, k.get("shape", "")])
		if int(k.get("weight", 0)) <= 0:
			errors.append("ruin %s: weight > 0" % name)
		var bl: Array = k.get("biomes", [])
		if bl.is_empty():
			errors.append("ruin %s: no biomes" % name)
		for b in bl:
			if not biomes.by_name.has(b):
				errors.append("ruin %s: unknown biome %s" % [name, b])
			elif int(biomes.by_name[b]) >= 64:
				errors.append("ruin %s: biome %s past the 64-bit mask" % [name, b])
		for key in ["half_u_m", "half_v_m"]:
			if key == "half_v_m" and not k.has(key):
				continue
			var r = k.get(key, null)
			if not (r is Array) or (r as Array).size() != 2 or float(r[0]) > float(r[1]) or float(r[0]) < 1.5 or float(r[1]) > MAX_HALF_M:
				errors.append("ruin %s: %s [min, max] within 1.5..%.1f m" % [name, key, MAX_HALF_M])
		if float(k.get("top_m", -1.0)) < 0.0 or float(k.get("top_m", 0.0)) > MAX_TOP_M:
			errors.append("ruin %s: top_m 0..%.1f" % [name, MAX_TOP_M])
		if float(k.get("depth_m", 0.0)) < 0.0 or float(k.get("depth_m", 0.0)) > MAX_DEPTH_M:
			errors.append("ruin %s: depth_m 0..%.1f" % [name, MAX_DEPTH_M])
		for m in MATS:
			if OreVeinRegistry.material_id(String(k.get(m, ""))) <= 0:
				errors.append("ruin %s: unknown %s material %s" % [name, m, k.get(m, "")])
		if int(k.get("pots", 0)) < 1 or int(k.get("pots", 0)) > MAX_POTS:
			errors.append("ruin %s: pots 1..%d" % [name, MAX_POTS])
		if not loot.has(String(k.get("loot", ""))):
			errors.append("ruin %s: unknown loot table %s" % [name, k.get("loot", "")])
	if kinds.is_empty():
		errors.append("ruins.json: no kinds")
	return errors.is_empty()


func generate() -> String:
	var biomes := BiomeRegistry.load_default()
	var L: Array[String] = []
	L.append("// GENERATED from game/data/ruins.json by game/tools/ruin_codegen.gd. Do not edit.")
	L.append("// Ruin kinds (ADR 0062). Regenerate:")
	L.append("//   Godot_console.exe --headless --path game --script res://tools/ruin_codegen.gd")
	L.append("#pragma once")
	L.append("")
	L.append("#include <cstdint>")
	L.append("")
	L.append("namespace technosim_ruin {")
	L.append("")
	var shape_enum := PackedStringArray()
	for s in SHAPES:
		shape_enum.append("S_" + String(s).to_upper())
	L.append("enum Shape : int { %s, NSHAPE };" % ", ".join(shape_enum))
	L.append("enum KindMat : int { M_WALL, M_TRIM, M_FLOOR, M_EXTRA };")
	L.append("static const int NKIND = %d;" % kinds.size())
	L.append("static const char *const KIND_NAMES[NKIND] = {%s};" % ", ".join(_map(kinds, func(k): return "\"%s\"" % k[0])))
	L.append("static const int KIND_SHAPE[NKIND] = {%s};" % ", ".join(_map(kinds, func(k): return "S_" + String(k[1]["shape"]).to_upper())))
	L.append("static const int KIND_WEIGHT[NKIND] = {%s};" % ", ".join(_map(kinds, func(k): return str(int(k[1]["weight"])))))
	L.append("static const bool KIND_COAST[NKIND] = {%s};" % ", ".join(_map(kinds, func(k): return "true" if bool(k[1].get("coast", false)) else "false")))
	L.append("// bit b set = biome id b (world_gen.h / biome_table.gen.h) may hold the kind")
	L.append("static const uint64_t KIND_BIOMES[NKIND] = {%s};" % ", ".join(_map(kinds, func(k): return "0x%016xULL" % _biome_mask(biomes, k[1]["biomes"]))))
	L.append("// sizes in macro cells (0.5 m)")
	L.append("static const int KIND_HALF_U[NKIND][2] = {%s};" % ", ".join(_map(kinds, func(k): return _pair(k[1]["half_u_m"]))))
	L.append("static const int KIND_HALF_V[NKIND][2] = {%s};" % ", ".join(_map(kinds, func(k): return _pair(k[1].get("half_v_m", k[1]["half_u_m"])))))
	L.append("static const int KIND_TOP[NKIND] = {%s};" % ", ".join(_map(kinds, func(k): return str(_macro(float(k[1]["top_m"]))))))
	L.append("static const int KIND_DEPTH[NKIND] = {%s};" % ", ".join(_map(kinds, func(k): return str(_macro(float(k[1].get("depth_m", 0.0)))))))
	L.append("// material ids: wall, trim, floor, extra")
	L.append("static const uint16_t KIND_MAT[NKIND][4] = {%s};" % ", ".join(_map(kinds, func(k): return "{%s}" % ", ".join(_map(MATS, func(m): return str(OreVeinRegistry.material_id(String(k[1][m]))))))))
	L.append("static const int KIND_POTS[NKIND] = {%s};" % ", ".join(_map(kinds, func(k): return str(int(k[1]["pots"])))))
	L.append("")
	L.append("} // namespace technosim_ruin")
	L.append("")
	return "\n".join(L)


static func _biome_mask(biomes: BiomeRegistry, names: Array) -> int:
	var m := 0
	for b in names:
		m |= 1 << int(biomes.by_name[b])
	return m


static func _macro(m: float) -> int:
	return int(round(m * MACRO_PER_M))


static func _pair(r: Array) -> String:
	return "{%d, %d}" % [_macro(float(r[0])), _macro(float(r[1]))]


static func _map(a: Array, f: Callable) -> PackedStringArray:
	var out := PackedStringArray()
	for x in a:
		out.append(f.call(x))
	return out
