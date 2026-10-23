class_name BiomeCodegen
extends RefCounted

## Turns data/biomes.json into a C++ header for the native generator so both
## sides decide biomes from the same tables. Pure text generation (testable).
## Material ids follow BiomeRegistry.MATERIALS order (= GameMaterials / WorldGen::Material).

const HEADER_PATH := "prototypes/p5-sim-spike/src/biome_table.gen.h"


static func generate(reg: BiomeRegistry) -> String:
	var rules := BiomeRules.new(reg)
	var n := reg.count()
	var L := PackedStringArray()
	L.append("// GENERATED from game/data/biomes.json by game/tools/biome_codegen.gd. Do not edit.")
	L.append("// Biome ids, names, surface materials and the climate tables the generator uses")
	L.append("// (see docs/design/biomes.md). Regenerate:")
	L.append("//   Godot_console.exe --headless --path game --script res://tools/biome_codegen.gd")
	L.append("#pragma once")
	L.append("")
	L.append("namespace technosim_biome {")
	L.append("")
	L.append("enum Id : int {")
	for d in reg.biomes:
		L.append("    %s = %d," % [String(d["name"]).to_upper(), int(d["id"])])
	L.append("    COUNT = %d" % n)
	L.append("};")
	L.append("")
	L.append("static const char *const NAMES[COUNT] = {%s};" % ", ".join(_map(reg.biomes, func(d): return "\"%s\"" % d["name"])))
	L.append("static const unsigned char COLORS[COUNT][3] = {%s};" % ", ".join(_map(reg.biomes, func(d): return "{%d, %d, %d}" % [int(d["color"][0]), int(d["color"][1]), int(d["color"][2])])))
	L.append("static const float TREE_DENSITY[COUNT] = {%s};" % ", ".join(_map(reg.biomes, func(d): return _f(float(d["tree_density"])))))
	L.append("// Tree shape and wood: %s." % ", ".join(_map(BiomeRegistry.TREE_KINDS, func(k): return "%d %s" % [BiomeRegistry.TREE_KINDS.find(k), k])))
	L.append("static const int TREE_KIND[COUNT] = {%s};" % ", ".join(_map(reg.biomes, func(d): return str(maxi(0, BiomeRegistry.TREE_KINDS.find(d.get("tree_kind", "oak")))))))
	L.append("// Material ids (WorldGen::Material); -1 = none.")
	L.append("static const int SURFACE_TOP[COUNT] = {%s};" % ", ".join(_map(reg.biomes, func(d): return str(_mat(d["surface"].get("top", ""))))))
	L.append("static const int SURFACE_UNDER[COUNT] = {%s};" % ", ".join(_map(reg.biomes, func(d): return str(_mat(d["surface"].get("under", ""))))))
	L.append("static const int SURFACE_STEEP[COUNT] = {%s};" % ", ".join(_map(reg.biomes, func(d): return str(_mat(d["surface"].get("steep", ""))))))
	L.append("// Caves: the ceiling over a cave cell and static pools on cave floors (-1 = plain stone, no pools).")
	L.append("static const int SURFACE_CEILING[COUNT] = {%s};" % ", ".join(_map(reg.biomes, func(d): return str(_mat(d["surface"].get("ceiling", ""))))))
	L.append("static const int SURFACE_POOL[COUNT] = {%s};" % ", ".join(_map(reg.biomes, func(d): return str(_mat(d["surface"].get("pool", ""))))))
	L.append("// What the top voxel of open water freezes into (-1 = it stays water).")
	L.append("static const int SURFACE_ICE[COUNT] = {%s};" % ", ".join(_map(reg.biomes, func(d): return str(_mat(d["surface"].get("ice", ""))))))
	L.append("")
	var c := reg.climate
	var a: Dictionary = c["altitude"]
	L.append("// Climate: temperature = clamp(NOISE_WEIGHT * temp_noise - LATITUDE_WEIGHT * clamp(north / LATITUDE_FULL_VOXELS, -1, 1), -1, 1)")
	L.append("// with north = %s, then minus LAPSE_PER_VOXEL per voxel above LAPSE_START_ABOVE_SEA." % ("-z" if c.get("north_is_negative_z", true) else "z"))
	L.append("static const bool NORTH_IS_NEGATIVE_Z = %s;" % ("true" if c.get("north_is_negative_z", true) else "false"))
	L.append("static const float LATITUDE_FULL_VOXELS = %s;" % _f(float(c["latitude_full_voxels"])))
	L.append("static const float LATITUDE_WEIGHT = %s;" % _f(float(c["latitude_weight"])))
	L.append("static const float NOISE_WEIGHT = %s;" % _f(float(c["noise_weight"])))
	L.append("static const int LAPSE_START_ABOVE_SEA = %d;" % int(c["lapse_start_above_sea"]))
	L.append("static const float LAPSE_PER_VOXEL = %s;" % _f(float(c["lapse_per_voxel"])))
	L.append("static const float JITTER = %s;" % _f(float(c["jitter"])))
	L.append("static const int BAND_COUNT = %d; // temperature: frozen, cold, temperate, warm, hot; humidity: arid, dry, neutral, humid, wet" % BiomeRegistry.TEMP_BANDS.size())
	L.append("static const float TEMP_BAND_MAX[BAND_COUNT] = {%s}; // band i = first i with value < max" % ", ".join(_map(c["temperature_bands"], func(b): return _f(float(b["max"])))))
	L.append("static const float HUM_BAND_MAX[BAND_COUNT] = {%s};" % ", ".join(_map(c["humidity_bands"], func(b): return _f(float(b["max"])))))
	L.append("")
	L.append("// Altitude (voxels above sea level)")
	L.append("static const float DEEP_OCEAN_MAX_CONTINENTALNESS = %s;" % _f(float(a["deep_ocean_max_continentalness"])))
	L.append("static const int COAST_MAX = %d;" % int(a["coast_max"]))
	L.append("static const int LOWLAND_MAX = %d;" % int(a["lowland_max"]))
	L.append("static const int HIGHLAND_MAX = %d;" % int(a["highland_max"]))
	L.append("static const int TREE_LINE_BASE = %d;  // tree_line = base - cold_shift * (-temperature)" % int(a["tree_line_base"]))
	L.append("static const int TREE_LINE_COLD_SHIFT = %d;" % int(a["tree_line_cold_shift"]))
	L.append("static const int SNOW_LINE_BASE = %d;" % int(a["snow_line_base"]))
	L.append("static const int SNOW_LINE_COLD_SHIFT = %d;" % int(a["snow_line_cold_shift"]))
	L.append("static const float STONY_SHORE_MIN_SLOPE = %s;" % _f(float(a["stony_shore_min_slope"])))
	L.append("")
	var t := reg.tables
	L.append("// Tables [temperature band][humidity band] -> Id")
	L.append("static const int LOWLAND[BAND_COUNT][BAND_COUNT] = {")
	for tb in BiomeRegistry.TEMP_BANDS:
		L.append("    {%s}, // %s" % [", ".join(_map(t["lowland"][tb], func(nm): return str(reg.id_of(nm)))), tb])
	L.append("};")
	L.append("static const int HIGHLAND[BAND_COUNT][BAND_COUNT] = {")
	for tb in BiomeRegistry.TEMP_BANDS:
		L.append("    {%s}, // %s" % [", ".join(_map(t["highland"][tb], func(nm): return str(reg.id_of(nm)))), tb])
	L.append("};")
	L.append("// Lowland overrides, checked in order before LOWLAND: bit masks over band indices.")
	L.append("struct Override { int biome; int temp_mask; int hum_mask; int max_above_sea; float min_erosion; };")
	var ovs: Array = t.get("lowland_overrides", [])
	L.append("static const int LOWLAND_OVERRIDE_COUNT = %d;" % ovs.size())
	var ov_lines := PackedStringArray()
	for o in ovs:
		var w: Dictionary = o["when"]
		ov_lines.append("{%d, %d, %d, %d, %s}" % [reg.id_of(o["biome"]), _mask(w.get("temperature", BiomeRegistry.TEMP_BANDS), BiomeRegistry.TEMP_BANDS), _mask(w.get("humidity", BiomeRegistry.HUM_BANDS), BiomeRegistry.HUM_BANDS), int(w.get("max_above_sea", 1000000)), _f(float(w.get("min_erosion", -2.0)))])
	L.append("static const Override LOWLAND_OVERRIDES[%d] = {%s};" % [maxi(1, ovs.size()), ", ".join(ov_lines)])
	L.append("")
	var ug := reg.underground
	var dnames := reg.depth_band_names()
	var nnames := reg.cave_noise_band_names()
	L.append("// Underground layer: depth = surface height - y. Below SOIL_DEPTH a cave biome")
	L.append("// comes from CAVE_TABLE[depth band][surface humidity band] after CAVE_OVERRIDES.")
	L.append("static const int SOIL_DEPTH = %d;" % int(ug["soil_depth"]))
	L.append("static const int DEPTH_BAND_COUNT = %d; // %s" % [dnames.size(), ", ".join(dnames)])
	L.append("static const float DEPTH_BAND_MAX[DEPTH_BAND_COUNT] = {%s};" % ", ".join(_map(ug["depth_bands"], func(b): return _f(float(b["max"])))))
	L.append("static const int CAVE_NOISE_BAND_COUNT = %d; // %s" % [nnames.size(), ", ".join(nnames)])
	L.append("static const float CAVE_NOISE_BAND_MAX[CAVE_NOISE_BAND_COUNT] = {%s};" % ", ".join(_map(ug["cave_noise_bands"], func(b): return _f(float(b["max"])))))
	L.append("static const float CAVE_NOISE_PERIOD_M = %s;" % _f(float(ug["cave_noise_period_m"])))
	L.append("static const int CAVE_TABLE[DEPTH_BAND_COUNT][BAND_COUNT] = {")
	for dn in dnames:
		L.append("    {%s}, // %s" % [", ".join(_map(t["cave"][dn], func(nm): return str(reg.id_of(nm)))), dn])
	L.append("};")
	L.append("struct CaveOverride { int biome; int depth_mask; int temp_mask; int hum_mask; int noise_mask; };")
	var covs: Array = t.get("cave_overrides", [])
	L.append("static const int CAVE_OVERRIDE_COUNT = %d;" % covs.size())
	var cov_lines := PackedStringArray()
	for o in covs:
		var w: Dictionary = o["when"]
		cov_lines.append("{%d, %d, %d, %d, %d}" % [reg.id_of(o["biome"]), _mask(w.get("depth", dnames), dnames), _mask(w.get("temperature", BiomeRegistry.TEMP_BANDS), BiomeRegistry.TEMP_BANDS), _mask(w.get("humidity", BiomeRegistry.HUM_BANDS), BiomeRegistry.HUM_BANDS), _mask(w.get("cave_noise", nnames), nnames)])
	L.append("static const CaveOverride CAVE_OVERRIDES[%d] = {%s};" % [maxi(1, covs.size()), ", ".join(cov_lines)])
	L.append("")
	L.append("static const int MOUNTAIN_ID = %d;" % reg.id_of(t["mountain"]))
	L.append("static const int ALPINE_ID = %d;" % reg.id_of(t["alpine"]))
	L.append("static const int SNOW_ID = %d;" % reg.id_of(t["snow"]))
	L.append("static const int DEEP_OCEAN_ID = %d;" % reg.id_of(t["deep_ocean"]))
	L.append("static const int OCEAN_ID = %d;" % reg.id_of(t["ocean"]["default"]))
	L.append("static const int OCEAN_FROZEN_ID = %d;" % reg.id_of(t["ocean"]["frozen"]))
	L.append("static const int COAST_ID = %d;" % reg.id_of(t["coast"]["default"]))
	L.append("static const int COAST_FROZEN_ID = %d;" % reg.id_of(t["coast"]["frozen"]))
	L.append("static const int COAST_STEEP_ID = %d;" % reg.id_of(t["coast"]["steep"]))
	L.append("static const int RIVER_ID = %d;" % reg.id_of(t["river"]))
	L.append("static const int LAKE_ID = %d;" % reg.id_of(t["lake"]))
	L.append("")
	L.append("// Reference classifier (mirrors game/scripts/world/biome_rules.gd::classify).")
	L.append("inline int band_of(const float *maxes, float v, int count = BAND_COUNT) {")
	L.append("    for (int i = 0; i < count; i++) if (v < maxes[i]) return i;")
	L.append("    return count - 1;")
	L.append("}")
	L.append("inline float temperature(float noise_t, float z, int above_sea) {")
	L.append("    float north = NORTH_IS_NEGATIVE_Z ? -z : z;")
	L.append("    float lat = north / LATITUDE_FULL_VOXELS;")
	L.append("    lat = lat < -1.0f ? -1.0f : (lat > 1.0f ? 1.0f : lat);")
	L.append("    float t = NOISE_WEIGHT * noise_t - LATITUDE_WEIGHT * lat;")
	L.append("    if (above_sea > LAPSE_START_ABOVE_SEA) t -= (float)(above_sea - LAPSE_START_ABOVE_SEA) * LAPSE_PER_VOXEL;")
	L.append("    return t < -1.0f ? -1.0f : (t > 1.0f ? 1.0f : t);")
	L.append("}")
	L.append("inline int tree_line(float t) { return TREE_LINE_BASE - (int)((float)TREE_LINE_COLD_SHIFT * (-t)); }")
	L.append("inline int snow_line(float t) { return SNOW_LINE_BASE - (int)((float)SNOW_LINE_COLD_SHIFT * (-t)); }")
	L.append("// t, h: final climate; above: height - sea; river mask 0..1; lake: under lake water.")
	L.append("inline int classify(float t, float h, int above, float continentalness, float erosion, float slope, float river, bool lake) {")
	L.append("    if (lake) return LAKE_ID;")
	L.append("    if (river > 0.5f) return RIVER_ID;")
	L.append("    const int ti = band_of(TEMP_BAND_MAX, t), hi = band_of(HUM_BAND_MAX, h);")
	L.append("    const bool frozen = ti == 0;")
	L.append("    if (continentalness < DEEP_OCEAN_MAX_CONTINENTALNESS) return DEEP_OCEAN_ID;")
	L.append("    if (above < -1) return frozen ? OCEAN_FROZEN_ID : OCEAN_ID;")
	L.append("    if (above <= COAST_MAX) return slope >= STONY_SHORE_MIN_SLOPE ? COAST_STEEP_ID : (frozen ? COAST_FROZEN_ID : COAST_ID);")
	L.append("    if (above > snow_line(t)) return SNOW_ID;")
	L.append("    if (above > tree_line(t)) return ALPINE_ID;")
	L.append("    if (above > HIGHLAND_MAX) return MOUNTAIN_ID;")
	L.append("    if (above > LOWLAND_MAX) return HIGHLAND[ti][hi];")
	L.append("    for (int i = 0; i < LOWLAND_OVERRIDE_COUNT; i++) {")
	L.append("        const Override &o = LOWLAND_OVERRIDES[i];")
	L.append("        if ((o.temp_mask >> ti) & 1 && (o.hum_mask >> hi) & 1 && above <= o.max_above_sea && erosion >= o.min_erosion) return o.biome;")
	L.append("    }")
	L.append("    return LOWLAND[ti][hi];")
	L.append("}")
	L.append("// Cave biome from band indices; cheap enough per voxel (bands change every few metres).")
	L.append("inline int cave_biome(int ti, int hi, int di, int ni) {")
	L.append("    for (int i = 0; i < CAVE_OVERRIDE_COUNT; i++) {")
	L.append("        const CaveOverride &o = CAVE_OVERRIDES[i];")
	L.append("        if ((o.depth_mask >> di) & 1 && (o.temp_mask >> ti) & 1 && (o.hum_mask >> hi) & 1 && (o.noise_mask >> ni) & 1) return o.biome;")
	L.append("    }")
	L.append("    return CAVE_TABLE[di][hi];  // CAVE_TABLE: the enum already has Id::CAVE")
	L.append("}")
	L.append("// surface_biome = classify(...) for the column; depth = surface height - y; cave_noise: slow 3D noise -1..1.")
	L.append("inline int classify_3d(int surface_biome, float t, float h, int depth, float cave_noise) {")
	L.append("    if (depth < SOIL_DEPTH) return surface_biome;")
	L.append("    return cave_biome(band_of(TEMP_BAND_MAX, t), band_of(HUM_BAND_MAX, h), band_of(DEPTH_BAND_MAX, (float)depth, DEPTH_BAND_COUNT), band_of(CAVE_NOISE_BAND_MAX, cave_noise, CAVE_NOISE_BAND_COUNT));")
	L.append("}")
	L.append("")
	L.append("} // namespace technosim_biome")
	L.append("")
	return "\n".join(L)


static func _map(arr: Array, f: Callable) -> PackedStringArray:
	var out := PackedStringArray()
	for x in arr:
		out.append(f.call(x))
	return out


static func _f(v: float) -> String:
	var s := "%.4f" % v
	while s.ends_with("0") and not s.ends_with(".0"):
		s = s.left(s.length() - 1)
	return s + "f"


static func _mat(name: String) -> int:
	if name == "" or not BiomeRegistry.MATERIALS.has(name):
		return -1
	return OreVeinRegistry.material_id(name)   # GameMaterials id (air = 0)


static func _mask(names: Array, all: Array) -> int:
	var m := 0
	for n in names:
		m |= 1 << all.find(n)
	return m
