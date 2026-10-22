class_name BiomeRules
extends RefCounted

## Reference biome placement logic over data/biomes.json. Four parts:
##  1. climate: temperature from noise + latitude (north = -Z is colder) and an
##     altitude lapse; band lookup.
##  2. classify(): climate + altitude + hydrology -> surface biome id, the same
##     decision the native generator makes from biome_table.gen.h.
##  3. classify_3d(): the same column at a given y: surface biome down to
##     soil_depth, then a cave biome from depth band x surface humidity, with
##     overrides on surface temperature and a slow 3D cave noise.
##  4. adjacency: which biomes can border each other, derived from where they
##     sit in climate space (surface: temperature, humidity, altitude band;
##     caves: depth, humidity, temperature, cave-noise band; surface biomes
##     touch the shallow caves beneath them). Noise is continuous, so a border
##     can only exist between cells that touch.
## GDScript never iterates voxels here: this is per-column / per-band logic for
## tests, tools and the generator's data tables, not the per-voxel path.

var reg: BiomeRegistry
var _cells_cache := {}     # name -> Array of cells (see cells_of)
var _neighbors_cache := {} # name -> {common, rare}


func _init(p_reg: BiomeRegistry) -> void:
	reg = p_reg


# ---------- climate ----------

## Latitude term in -1 (far south, hot) .. 1 (far north, cold) from a Z coordinate (voxels).
func latitude(z: float) -> float:
	var full := float(reg.climate["latitude_full_voxels"])
	var north := -z if reg.climate.get("north_is_negative_z", true) else z
	return clampf(north / full, -1.0, 1.0)


## Temperature in -1 (frozen) .. 1 (hot) from the temperature noise sample
## (-1..1), the Z coordinate and the height above sea level (voxels).
func temperature(noise_t: float, z: float, above_sea: int = 0) -> float:
	var c := reg.climate
	var t := float(c["noise_weight"]) * noise_t - float(c["latitude_weight"]) * latitude(z)
	var lapse_from := int(c["lapse_start_above_sea"])
	if above_sea > lapse_from:
		t -= float(above_sea - lapse_from) * float(c["lapse_per_voxel"])
	return clampf(t, -1.0, 1.0)


func temp_band(t: float) -> int:
	return _band(reg.climate["temperature_bands"], t)


func hum_band(h: float) -> int:
	return _band(reg.climate["humidity_bands"], h)


func depth_band(depth: int) -> int:
	return _band(reg.underground["depth_bands"], float(depth))


func cave_noise_band(n: float) -> int:
	return _band(reg.underground["cave_noise_bands"], n)


static func _band(bands: Array, v: float) -> int:
	for i in bands.size():
		if v < float(bands[i]["max"]):
			return i
	return bands.size() - 1


## Tree line and snow line (voxels above sea) for a temperature.
func tree_line(t: float) -> int:
	var a: Dictionary = reg.climate["altitude"]
	return int(a["tree_line_base"]) - int(float(a["tree_line_cold_shift"]) * (-t))


func snow_line(t: float) -> int:
	var a: Dictionary = reg.climate["altitude"]
	return int(a["snow_line_base"]) - int(float(a["snow_line_cold_shift"]) * (-t))


## Altitude band index (BiomeRegistry.ALT_BANDS) for a column.
func alt_band(above_sea: int, continentalness: float, t: float) -> int:
	var a: Dictionary = reg.climate["altitude"]
	if continentalness < float(a["deep_ocean_max_continentalness"]):
		return 0
	if above_sea < -1:
		return 1
	if above_sea <= int(a["coast_max"]):
		return 2
	if above_sea > snow_line(t):
		return 7
	if above_sea > tree_line(t):
		return 6
	if above_sea > int(a["highland_max"]):
		return 5
	if above_sea > int(a["lowland_max"]):
		return 4
	return 3


# ---------- surface classification ----------

## Column -> surface biome id. `col` keys (all optional except temperature/humidity/above_sea):
##   temperature, humidity: final climate values (-1..1, latitude and lapse applied)
##   above_sea: height - sea level (voxels)
##   continentalness, erosion, slope: generator noises / derived slope
##   river: 0..1 river mask; lake: bool (column is under lake water)
func classify(col: Dictionary) -> int:
	var tb := reg.tables
	if col.get("lake", false):
		return reg.id_of(tb["lake"])
	if float(col.get("river", 0.0)) > 0.5:
		return reg.id_of(tb["river"])
	var t := float(col["temperature"])
	var h := float(col["humidity"])
	var above := int(col["above_sea"])
	var ti := temp_band(t)
	var hi := hum_band(h)
	var frozen := ti == 0
	match alt_band(above, float(col.get("continentalness", 0.0)), t):
		0:
			return reg.id_of(tb["deep_ocean"])
		1:
			return reg.id_of(tb["ocean"]["frozen" if frozen else "default"])
		2:
			if float(col.get("slope", 0.0)) >= float(reg.climate["altitude"]["stony_shore_min_slope"]):
				return reg.id_of(tb["coast"]["steep"])
			return reg.id_of(tb["coast"]["frozen" if frozen else "default"])
		3:
			for o in tb.get("lowland_overrides", []):
				if _override_matches(o["when"], ti, hi, above, col):
					return reg.id_of(o["biome"])
			return reg.id_of(tb["lowland"][BiomeRegistry.TEMP_BANDS[ti]][hi])
		4:
			return reg.id_of(tb["highland"][BiomeRegistry.TEMP_BANDS[ti]][hi])
		5:
			return reg.id_of(tb["mountain"])
		6:
			return reg.id_of(tb["alpine"])
		_:
			return reg.id_of(tb["snow"])


func classify_name(col: Dictionary) -> String:
	return reg.name_of(classify(col))


func _override_matches(when: Dictionary, ti: int, hi: int, above: int, col: Dictionary) -> bool:
	if when.has("temperature") and not when["temperature"].has(BiomeRegistry.TEMP_BANDS[ti]):
		return false
	if when.has("humidity") and not when["humidity"].has(BiomeRegistry.HUM_BANDS[hi]):
		return false
	if when.has("max_above_sea") and above > int(when["max_above_sea"]):
		return false
	if when.has("min_erosion") and float(col.get("erosion", 0.0)) < float(when["min_erosion"]):
		return false
	return true


# ---------- 3D classification ----------

## Column + depth below the surface (voxels) + cave noise sample (-1..1) -> biome id.
## Depth < soil_depth keeps the surface biome (classify(col)).
func classify_3d(col: Dictionary, depth: int, cave_noise: float = 0.0) -> int:
	if depth < int(reg.underground["soil_depth"]):
		return classify(col)
	return cave_biome(temp_band(float(col["temperature"])), hum_band(float(col["humidity"])), depth_band(depth), cave_noise_band(cave_noise))


func classify_3d_name(col: Dictionary, depth: int, cave_noise: float = 0.0) -> String:
	return reg.name_of(classify_3d(col, depth, cave_noise))


## Cave biome from band indices (surface temperature, surface humidity, depth, cave noise).
func cave_biome(ti: int, hi: int, di: int, ni: int) -> int:
	var tb := reg.tables
	var dname: String = reg.depth_band_names()[di]
	for o in tb.get("cave_overrides", []):
		if _cave_override_matches(o["when"], ti, hi, di, ni):
			return reg.id_of(o["biome"])
	return reg.id_of(tb["cave"][dname][hi])


func _cave_override_matches(when: Dictionary, ti: int, hi: int, di: int, ni: int) -> bool:
	if when.has("depth") and not when["depth"].has(reg.depth_band_names()[di]):
		return false
	if when.has("temperature") and not when["temperature"].has(BiomeRegistry.TEMP_BANDS[ti]):
		return false
	if when.has("humidity") and not when["humidity"].has(BiomeRegistry.HUM_BANDS[hi]):
		return false
	if when.has("cave_noise") and not when["cave_noise"].has(reg.cave_noise_band_names()[ni]):
		return false
	return true


# ---------- adjacency ----------

## Climate cells each biome occupies.
##  surface biomes: [temp_band, hum_band, alt_band]
##  cave biomes:    [temp_band, hum_band, depth_band, noise_band]
## Water biomes get no cells (they are overlays); see neighbors().
func cells_of(name: String) -> Array:
	if _cells_cache.has(name):
		return _cells_cache[name]
	var out := []
	var tb := reg.tables
	var is_cave: bool = reg.get_def(name).get("band", "") == "cave"
	for ti in BiomeRegistry.TEMP_BANDS.size():
		var trow: String = BiomeRegistry.TEMP_BANDS[ti]
		for hi in BiomeRegistry.HUM_BANDS.size():
			if is_cave:
				for di in reg.depth_band_names().size():
					for ni in reg.cave_noise_band_names().size():
						if cave_biome(ti, hi, di, ni) == reg.id_of(name):
							out.append([ti, hi, di, ni])
				continue
			var frozen := ti == 0
			var here := {
				0: [tb["deep_ocean"]],
				1: [tb["ocean"]["frozen" if frozen else "default"]],
				2: [tb["coast"]["frozen" if frozen else "default"], tb["coast"]["steep"]],
				3: [tb["lowland"][trow][hi]],
				4: [tb["highland"][trow][hi]],
				5: [tb["mountain"]],
				6: [tb["alpine"]],
				7: [tb["snow"]],
			}
			# Overrides occupy their cells too (they share them with the table biome).
			for o in tb.get("lowland_overrides", []):
				var w: Dictionary = o["when"]
				if (not w.has("temperature") or w["temperature"].has(trow)) and (not w.has("humidity") or w["humidity"].has(BiomeRegistry.HUM_BANDS[hi])):
					here[3].append(o["biome"])
			for ai in here:
				if here[ai].has(name):
					out.append([ti, hi, ai])
	_cells_cache[name] = out
	return out


## Which biomes can border `name`, split into common (share a band edge: one
## climate step at the same altitude/depth, the next altitude or depth band with
## the same climate, or a surface biome and the shallow cave beneath it) and
## rare (diagonal in climate space: two things change at once).
## Water biomes border every biome whose band they can cut through.
func neighbors(name: String) -> Dictionary:
	if _neighbors_cache.has(name):
		return _neighbors_cache[name]
	var common := {}
	var rare := {}
	var water_names: Array = [reg.tables["river"], reg.tables["lake"]]
	if water_names.has(name):
		for other in reg.names():
			if other != name and (water_names.has(other) or _water_touches(name, other)):
				common[other] = true
	else:
		for other in reg.names():
			if other == name:
				continue
			if water_names.has(other):
				if _water_touches(other, name):
					common[other] = true
				continue
			var best := 99
			for a in cells_of(name):
				for b in cells_of(other):
					best = mini(best, _cell_steps(a, b))
			if best == 1:
				common[other] = true
			elif best <= 3:
				rare[other] = true
	var c := common.keys()
	var r := rare.keys()
	c.sort()
	r.sort()
	_neighbors_cache[name] = {"common": c, "rare": r}
	return _neighbors_cache[name]


## Steps between two cells in climate space (99 = cannot touch).
## Surface-surface: temperature, humidity, altitude each at most one band apart.
## Cave-cave: temperature, humidity, depth, noise each at most one band apart.
## Surface-cave: the shallow depth band under land, same temperature and
## humidity (within one band); altitude band does not matter (caves run under
## everything that is not sea floor).
static func _cell_steps(a: Array, b: Array) -> int:
	if a.size() != b.size():
		var s: Array = a if a.size() == 3 else b
		var c: Array = b if a.size() == 3 else a
		if s[2] <= 1 or c[2] != 0:  # sea floor above, or cave not shallow
			return 99
		var dt: int = absi(s[0] - c[0])
		var dh: int = absi(s[1] - c[1])
		if dt > 1 or dh > 1:
			return 99
		return maxi(1, dt + dh + 1)
	var steps := 0
	for i in a.size():
		var d: int = absi(a[i] - b[i])
		if d > 1:
			return 99
		steps += d
	return maxi(1, steps)  # same cell (table biome + override, or beach + stony shore)


func can_border(a: String, b: String) -> bool:
	var n := neighbors(a)
	return n["common"].has(b) or n["rare"].has(b)


func _water_touches(water: String, land: String) -> bool:
	# Rivers and lakes come from the hydrology pass, which runs on the height
	# field alone: springs and tarns above the tree line, basins next to beaches.
	# Rivers reach the sea; lakes never sit in the sea. Both cut into shallow caves.
	var band: String = reg.get_def(land).get("band", "")
	if band == "cave":
		return reg.has_tag(land, "underground") and not reg.has_tag(land, "deep")
	if water == reg.tables["river"]:
		return band != "deep_ocean"
	return band != "deep_ocean" and band != "ocean"


## Biomes you meet walking north from a start biome (colder bands, same altitude).
func colder_than(name: String) -> Array:
	var out := {}
	for c in cells_of(name):
		for d in reg.biomes:
			for c2 in cells_of(d["name"]):
				if c2.size() == c.size() and c2[2] == c[2] and c2[0] < c[0]:
					out[d["name"]] = true
	var k := out.keys()
	k.sort()
	return k


## Cave biomes that can lie straight below a surface biome.
func below(name: String) -> Array:
	var out := {}
	for c in cells_of(name):
		if c.size() != 3 or c[2] <= 1:
			continue
		for di in reg.depth_band_names().size():
			for ni in reg.cave_noise_band_names().size():
				out[reg.name_of(cave_biome(c[0], c[1], di, ni))] = true
	var k := out.keys()
	k.sort()
	return k
