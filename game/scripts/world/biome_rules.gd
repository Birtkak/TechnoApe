class_name BiomeRules
extends RefCounted

## Reference biome placement logic over data/biomes.json. Three parts:
##  1. climate: temperature from noise + latitude (north = -Z is colder) and an
##     altitude lapse; band lookup.
##  2. classify(): climate + altitude + hydrology -> biome id, the same decision
##     the native generator makes from biome_table.gen.h.
##  3. adjacency: which biomes can border each other, derived from where they
##     sit in (temperature band, humidity band, altitude band) space. Noise is
##     continuous, so a border can only exist between cells that touch.
## GDScript never iterates voxels here: this is per-column logic for tests,
## tools and the generator's data tables, not the per-voxel path.

var reg: BiomeRegistry


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


# ---------- classification ----------

## Column -> biome id. `col` keys (all optional except temperature/humidity/above_sea):
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


# ---------- adjacency ----------

## Climate cells each biome occupies: Array of [temp_band, hum_band, alt_band].
## Water biomes get no cells (they are overlays); see neighbors().
func cells_of(name: String) -> Array:
	var out := []
	var tb := reg.tables
	for ti in BiomeRegistry.TEMP_BANDS.size():
		var trow: String = BiomeRegistry.TEMP_BANDS[ti]
		for hi in BiomeRegistry.HUM_BANDS.size():
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
	return out


## Which biomes can border `name`, split into common (share a band edge: one
## climate step at the same altitude, or the next altitude band with the same
## climate) and rare (diagonal in climate space, or two things change at once).
## Water biomes border every land biome whose band they can cut through.
func neighbors(name: String) -> Dictionary:
	var common := {}
	var rare := {}
	var mine := cells_of(name)
	var water_names: Array = [reg.tables["river"], reg.tables["lake"]]
	if water_names.has(name):
		for other in reg.names():
			if other == name:
				continue
			if water_names.has(other):
				common[other] = true
			elif _water_touches(name, other):
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
			for a in mine:
				for b in cells_of(other):
					var dt: int = absi(a[0] - b[0])
					var dh: int = absi(a[1] - b[1])
					var da: int = absi(a[2] - b[2])
					if dt > 1 or dh > 1 or da > 1:
						continue
					var steps := dt + dh + da
					if steps == 0:
						steps = 1  # same cell (table biome + override, or beach + stony shore)
					best = mini(best, steps)
			if best == 1:
				common[other] = true
			elif best <= 3:
				rare[other] = true
	var c := common.keys()
	var r := rare.keys()
	c.sort()
	r.sort()
	return {"common": c, "rare": r}


func can_border(a: String, b: String) -> bool:
	var n := neighbors(a)
	return n["common"].has(b) or n["rare"].has(b)


func _water_touches(water: String, land: String) -> bool:
	var band: String = reg.get_def(land).get("band", "")
	if water == reg.tables["river"]:
		# Rivers run from springs in the hills down to the sea: not above the tree
		# line (no springs there) and not out into the deep ocean.
		return ["ocean", "coast", "lowland", "highland", "mountain"].has(band)
	# Lakes fill basins: lowland, highland and mountain valleys (the hydrology
	# pass makes them wherever a depression is), never on the coast or peaks.
	return ["lowland", "highland", "mountain"].has(band)


## Biomes you meet walking north from a start temperature band (colder bands first).
func colder_than(name: String) -> Array:
	var out := {}
	for c in cells_of(name):
		for d in reg.biomes:
			for c2 in cells_of(d["name"]):
				if c2[2] == c[2] and c2[0] < c[0]:
					out[d["name"]] = true
	var k := out.keys()
	k.sort()
	return k
