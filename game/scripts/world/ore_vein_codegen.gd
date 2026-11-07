class_name OreVeinCodegen
extends RefCounted

## Turns data/ore_veins.json into a C++ header for the native generator.
## Pure text generation (testable). Material ids are GameMaterials ids.

const HEADER_PATH := "prototypes/p5-sim-spike/src/ore_veins.gen.h"


static func generate(reg: OreVeinRegistry) -> String:
	var L := PackedStringArray()
	L.append("// GENERATED from game/data/ore_veins.json by game/tools/ore_vein_codegen.gd. Do not edit.")
	L.append("// GregTech-style ore veins (docs/design/progression.md \"Ores\"). Regenerate:")
	L.append("//   Godot_console.exe --headless --path game --script res://tools/ore_vein_codegen.gd")
	L.append("#pragma once")
	L.append("")
	L.append("namespace technosim_ore {")
	L.append("")
	L.append("static const int CELL = %d;               // voxels per vein cell (one vein at most)" % int(reg.grid["cell_voxels"]))
	L.append("static const float FILL = %s;           // chance a cell holds a vein" % _f(float(reg.grid["fill"])))
	L.append("static const float SPORADIC_CHANCE = %s; // ore voxel -> sporadic ore" % _f(float(reg.grid["sporadic_chance"])))
	L.append("static const int INDICATOR_RADIUS = %d;   // voxels from the vein centre" % int(reg.indicators["radius_voxels"]))
	L.append("static const int INDICATOR_MAX_DEPTH = %d; // vein top below the ground (voxels)" % int(reg.indicators["max_depth_voxels"]))
	L.append("static const float INDICATOR_CHANCE = %s; // per column at depth 0, fading to 0 at max depth" % _f(float(reg.indicators["chance"])))
	L.append("")
	L.append("enum Id : int {")
	for i in reg.count():
		L.append("    %s = %d," % [String(reg.veins[i]["name"]).to_upper(), i])
	L.append("    COUNT = %d" % reg.count())
	L.append("};")
	L.append("static const int WEIGHT_TOTAL = %d;" % reg.total_weight())
	L.append("static const char *const NAMES[COUNT] = {%s};" % ", ".join(_map(reg.veins, func(v): return "\"%s\"" % v["name"])))
	L.append("static const int WEIGHT[COUNT] = {%s};" % ", ".join(_map(reg.veins, func(v): return str(int(v["weight"])))))
	L.append("static const int Y_MIN[COUNT] = {%s};" % ", ".join(_map(reg.veins, func(v): return str(int(v["y_min"])))))
	L.append("static const int Y_MAX[COUNT] = {%s};" % ", ".join(_map(reg.veins, func(v): return str(int(v["y_max"])))))
	L.append("static const int SIZE[COUNT] = {%s};      // lens diameter (voxels)" % ", ".join(_map(reg.veins, func(v): return str(int(v["size_voxels"])))))
	L.append("static const int THICKNESS[COUNT] = {%s}; // lens thickness at the centre (voxels)" % ", ".join(_map(reg.veins, func(v): return str(int(v["thickness_voxels"])))))
	L.append("static const int JITTER[COUNT] = {%s};    // max centre offset, keeps the lens in its cell" % ", ".join(_map(reg.veins, func(v): return str(reg.jitter_of(v)))))
	L.append("static const float DENSITY[COUNT] = {%s};" % ", ".join(_map(reg.veins, func(v): return _f(float(v["density"])))))
	L.append("// Material ids (WorldGen::Mat); -1 = none.")
	for k in OreVeinRegistry.ORE_KEYS:
		L.append("static const int %s[COUNT] = {%s};" % [k.to_upper(), ", ".join(_map(reg.veins, func(v): return str(OreVeinRegistry.material_id(String(v[k])))))])
	L.append("static const int POCKET_GAS[COUNT] = {%s};       // ADR 0148: a lump of this gas instead of ore ..." % ", ".join(_map(reg.veins, func(v): return str(OreVeinRegistry.material_id(String(v["pocket_gas"]))) if v.has("pocket_gas") else "-1")))
	L.append("static const float POCKET_GAS_CHANCE[COUNT] = {%s}; // ... with this chance per lump" % ", ".join(_map(reg.veins, func(v): return _f(float(v.get("pocket_gas_chance", 0.0))))))
	L.append("static const int INDICATOR[COUNT] = {%s};" % ", ".join(_map(reg.veins, func(v): return str(OreVeinRegistry.material_id(String(v.get("indicator", "")))) if String(v.get("indicator", "")) != "" else "-1")))
	L.append("")
	L.append("} // namespace technosim_ore")
	L.append("")
	return "\n".join(L)


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
