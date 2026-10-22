class_name BiomeRegistry
extends RefCounted

## Biomes loaded from data/biomes.json: the list, the climate tunables and the
## placement tables. Engine-agnostic (no nodes), testable headless. The native
## generator uses the same data through the generated header
## prototypes/p5-sim-spike/src/biome_table.gen.h (see tools/biome_codegen.gd).

const TEMP_BANDS := ["frozen", "cold", "temperate", "warm", "hot"]
const HUM_BANDS := ["arid", "dry", "neutral", "humid", "wet"]
## Altitude bands from the sea floor up. Adjacent entries can border each other.
const ALT_BANDS := ["deep_ocean", "ocean", "coast", "lowland", "highland", "mountain", "alpine", "snow"]
## Material names the surface entries may use (GameMaterials names).
const MATERIALS := ["air", "stone", "dirt", "grass", "sand", "water", "wood", "lava", "ash", "oil", "steam", "smoke", "fire", "bedrock", "snow", "gravel", "coal_ore", "iron_ore", "copper_ore", "tin_ore", "leaves", "sand_static", "gravel_static", "water_static"]

var biomes: Array = []        # index = id -> Dictionary (raw definition, validated)
var by_name := {}             # name -> id
var climate := {}             # the "climate" section
var tables := {}              # the "tables" section
var errors: Array[String] = []


static func load_default() -> BiomeRegistry:
	var r := BiomeRegistry.new()
	r.load_file("res://data/biomes.json")
	return r


func load_file(path: String) -> bool:
	var f := FileAccess.open(path, FileAccess.READ)
	if f == null:
		errors.append("cannot open %s" % path)
		return false
	return load_json(f.get_as_text())


func load_json(text: String) -> bool:
	var data = JSON.parse_string(text)
	if typeof(data) != TYPE_DICTIONARY or not data.has("biomes") or not data.has("climate") or not data.has("tables"):
		errors.append("biomes.json: expected {climate, tables, biomes}")
		return false
	climate = data["climate"]
	tables = data["tables"]
	for d in data["biomes"]:
		if typeof(d) != TYPE_DICTIONARY or not d.has("id") or not d.has("name"):
			errors.append("biome without id/name: %s" % str(d))
			continue
		if int(d["id"]) != biomes.size():
			errors.append("biome %s: id %d out of order (expected %d)" % [d["name"], int(d["id"]), biomes.size()])
			continue
		if by_name.has(d["name"]):
			errors.append("duplicate biome name %s" % d["name"])
			continue
		biomes.append(d)
		by_name[d["name"]] = int(d["id"])
	for d in biomes:
		_validate(d)
	_validate_climate()
	_validate_tables()
	return errors.is_empty()


func _validate(d: Dictionary) -> void:
	var n: String = d["name"]
	if not ALT_BANDS.has(d.get("band", "")) and d.get("band", "") != "water":
		errors.append("biome %s: band must be one of %s or water" % [n, ALT_BANDS])
	var s: Dictionary = d.get("surface", {})
	if not s.has("top") or not s.has("under"):
		errors.append("biome %s: surface needs top and under" % n)
	for k in s:
		if not MATERIALS.has(s[k]):
			errors.append("biome %s: unknown surface material %s" % [n, s[k]])
	var col = d.get("color", null)
	if typeof(col) != TYPE_ARRAY or col.size() != 3:
		errors.append("biome %s: color must be [r, g, b]" % n)
	var td := float(d.get("tree_density", -1.0))
	if td < 0.0 or td > 1.0:
		errors.append("biome %s: tree_density must be 0..1" % n)
	for b in d.get("borders", []):
		if not by_name.has(b):
			errors.append("biome %s: unknown border biome %s" % [n, b])


func _validate_climate() -> void:
	for key in ["latitude_full_voxels", "latitude_weight", "noise_weight", "lapse_start_above_sea", "lapse_per_voxel", "jitter", "temperature_bands", "humidity_bands", "altitude"]:
		if not climate.has(key):
			errors.append("climate: missing %s" % key)
	_validate_bands("temperature_bands", TEMP_BANDS)
	_validate_bands("humidity_bands", HUM_BANDS)
	if climate.get("latitude_weight", 0.0) + climate.get("noise_weight", 0.0) < 1.0:
		errors.append("climate: latitude_weight + noise_weight must reach 1.0 so every band is reachable")


func _validate_bands(key: String, names: Array) -> void:
	var bands: Array = climate.get(key, [])
	if bands.size() != names.size():
		errors.append("climate.%s: expected %d bands" % [key, names.size()])
		return
	var prev := -1.0
	for i in bands.size():
		var b: Dictionary = bands[i]
		if b.get("name", "") != names[i]:
			errors.append("climate.%s[%d]: expected name %s" % [key, i, names[i]])
		if float(b.get("min", 9.0)) != prev:
			errors.append("climate.%s[%d]: bands must be contiguous from -1" % [key, i])
		prev = float(b.get("max", 9.0))
	if prev < 1.0:
		errors.append("climate.%s: last band must reach 1.0" % key)


func _validate_tables() -> void:
	for tname in ["lowland", "highland"]:
		var t: Dictionary = tables.get(tname, {})
		for tb in TEMP_BANDS:
			var row: Array = t.get(tb, [])
			if row.size() != HUM_BANDS.size():
				errors.append("tables.%s.%s: expected %d entries" % [tname, tb, HUM_BANDS.size()])
				continue
			for i in row.size():
				_check_name("tables.%s.%s[%d]" % [tname, tb, i], row[i], tname)
	for o in tables.get("lowland_overrides", []):
		_check_name("lowland_overrides", o.get("biome", ""), "lowland")
	_check_name("tables.mountain", tables.get("mountain", ""), "mountain")
	_check_name("tables.alpine", tables.get("alpine", ""), "alpine")
	_check_name("tables.snow", tables.get("snow", ""), "snow")
	_check_name("tables.deep_ocean", tables.get("deep_ocean", ""), "deep_ocean")
	_check_name("tables.river", tables.get("river", ""), "water")
	_check_name("tables.lake", tables.get("lake", ""), "water")
	for k in ["default", "frozen", "steep"]:
		_check_name("tables.coast.%s" % k, tables.get("coast", {}).get(k, ""), "coast")
	for k in ["default", "frozen"]:
		_check_name("tables.ocean.%s" % k, tables.get("ocean", {}).get(k, ""), "ocean")


func _check_name(where: String, name: String, band: String) -> void:
	if not by_name.has(name):
		errors.append("%s: unknown biome %s" % [where, name])
	elif biomes[by_name[name]].get("band", "") != band:
		errors.append("%s: %s is a %s biome, table expects %s" % [where, name, biomes[by_name[name]].get("band", ""), band])


func count() -> int:
	return biomes.size()


func has(name: String) -> bool:
	return by_name.has(name)


func id_of(name: String) -> int:
	return by_name.get(name, -1)


func name_of(id: int) -> String:
	if id < 0 or id >= biomes.size():
		return "unknown"
	return biomes[id]["name"]


func get_def(name: String) -> Dictionary:
	return biomes[by_name[name]] if by_name.has(name) else {}


func has_tag(name: String, tag: String) -> bool:
	return get_def(name).get("tags", []).has(tag)


func with_tag(tag: String) -> Array:
	var out := []
	for d in biomes:
		if d.get("tags", []).has(tag):
			out.append(d["name"])
	return out


func names() -> Array:
	var out := []
	for d in biomes:
		out.append(d["name"])
	return out
