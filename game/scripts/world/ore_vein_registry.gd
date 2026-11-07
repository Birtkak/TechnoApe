class_name OreVeinRegistry
extends RefCounted

## GregTech-style ore veins loaded from data/ore_veins.json: the grid, the
## surface indicator tunables and the vein types. Engine-agnostic, testable
## headless. The native generator reads the same data through the generated
## header prototypes/p5-sim-spike/src/ore_veins.gen.h (tools/ore_vein_codegen.gd).

const NAME_RE := "^[a-z][a-z0-9_]*$"
const ORE_KEYS := ["primary", "between", "secondary", "sporadic"]
const WORLD_HEIGHT := 1280   # macro voxels (0.5 m): Settings.WORLD_HEIGHT_VOXELS / 4

var grid := {}
var indicators := {}
var veins: Array = []         # index = native id -> Dictionary (validated, defaults filled in)
var by_name := {}
var errors: Array[String] = []


static func load_default() -> OreVeinRegistry:
	var r := OreVeinRegistry.new()
	r.load_file("res://data/ore_veins.json")
	return r


## GameMaterials id of a material name (0 = air), -1 when unknown.
static func material_id(mat_name: String) -> int:
	if mat_name == "air":
		return 0
	var d := GameMaterials.defs()
	for i in d.size():
		if d[i]["name"] == mat_name:
			return i + 1
	return -1


func load_file(path: String) -> bool:
	var f := FileAccess.open(path, FileAccess.READ)
	if f == null:
		errors.append("cannot open %s" % path)
		return false
	return load_json(f.get_as_text())


func load_json(text: String) -> bool:
	var data = JSON.parse_string(text)
	if typeof(data) != TYPE_DICTIONARY or not data.has("grid") or not data.has("indicators") or not data.has("veins"):
		errors.append("ore_veins.json: expected {grid, indicators, veins}")
		return false
	grid = data["grid"]
	indicators = data["indicators"]
	_validate_grid()
	var re := RegEx.create_from_string(NAME_RE)
	for v in data["veins"]:
		if typeof(v) != TYPE_DICTIONARY or not v.has("name"):
			errors.append("vein without a name: %s" % str(v))
			continue
		var vname := String(v["name"])
		if re.search(vname) == null:
			errors.append("vein %s: names use lowercase letters, digits and underscores" % vname)
			continue
		if by_name.has(vname):
			errors.append("duplicate vein %s" % vname)
			continue
		var d: Dictionary = v.duplicate()
		for k in ["size_voxels", "thickness_voxels", "density"]:
			if not d.has(k):
				d[k] = grid.get(k, 0)
		if _validate_vein(d):
			by_name[vname] = veins.size()
			veins.append(d)
	if veins.is_empty():
		errors.append("ore_veins.json: no valid veins")
	return errors.is_empty()


func count() -> int:
	return veins.size()


func total_weight() -> int:
	var t := 0
	for v in veins:
		t += int(v["weight"])
	return t


## Horizontal jitter of a vein centre so the lens stays inside its cell.
func jitter_of(v: Dictionary) -> int:
	return maxi(0, (int(grid["cell_voxels"]) - int(v["size_voxels"])) / 2 - 4)


func _validate_grid() -> void:
	for k in ["cell_voxels", "fill", "size_voxels", "thickness_voxels", "density", "sporadic_chance"]:
		if not grid.has(k):
			errors.append("grid.%s missing" % k)
			return
	if int(grid["cell_voxels"]) < 32:
		errors.append("grid.cell_voxels must be at least 32")
	for k in ["fill", "density", "sporadic_chance"]:
		var x := float(grid[k])
		if x < 0.0 or x > 1.0:
			errors.append("grid.%s must be between 0 and 1" % k)
	for k in ["radius_voxels", "max_depth_voxels", "chance"]:
		if not indicators.has(k):
			errors.append("indicators.%s missing" % k)
			return
	if int(indicators["radius_voxels"]) < 1 or int(indicators["max_depth_voxels"]) < 1:
		errors.append("indicators: radius and max depth must be at least 1")
	if float(indicators["chance"]) < 0.0 or float(indicators["chance"]) > 1.0:
		errors.append("indicators.chance must be between 0 and 1")


func _validate_vein(v: Dictionary) -> bool:
	var n := String(v["name"])
	var ok := true
	for k in ["y_min", "y_max", "weight"] + ORE_KEYS:
		if not v.has(k):
			errors.append("vein %s: %s missing" % [n, k])
			return false
	var y0 := int(v["y_min"])
	var y1 := int(v["y_max"])
	if y0 < 4 or y1 > WORLD_HEIGHT or y0 > y1:
		errors.append("vein %s: need 4 <= y_min <= y_max <= %d" % [n, WORLD_HEIGHT])
		ok = false
	if int(v["weight"]) < 1:
		errors.append("vein %s: weight must be at least 1" % n)
		ok = false
	if v.has("pocket_gas") and material_id(String(v["pocket_gas"])) <= 0:   # ADR 0148: whole lumps of a gas instead of ore
		errors.append("vein %s: unknown pocket_gas %s" % [n, v["pocket_gas"]])
		ok = false
	if float(v.get("pocket_gas_chance", 0.0)) < 0.0 or float(v.get("pocket_gas_chance", 0.0)) > 1.0:
		errors.append("vein %s: pocket_gas_chance must be between 0 and 1" % n)
		ok = false
	var size := int(v["size_voxels"])
	var thick := int(v["thickness_voxels"])
	if size < 4 or size > int(grid.get("cell_voxels", 0)) - 16:
		errors.append("vein %s: size_voxels must be 4 .. cell_voxels - 16" % n)
		ok = false
	if thick < 2 or thick > size:
		errors.append("vein %s: thickness_voxels must be 2 .. size_voxels" % n)
		ok = false
	if float(v["density"]) <= 0.0 or float(v["density"]) > 1.0:
		errors.append("vein %s: density must be more than 0 and at most 1" % n)
		ok = false
	for k in ORE_KEYS:
		var id := material_id(String(v[k]))
		if id <= 0:
			errors.append("vein %s: %s material %s is unknown" % [n, k, v[k]])
			ok = false
	var ind := String(v.get("indicator", ""))
	if ind != "":
		var iid := material_id(ind)
		if iid <= 0:
			errors.append("vein %s: indicator material %s is unknown" % [n, ind])
			ok = false
		elif not GameMaterials.defs()[iid - 1].has("micro"):
			errors.append("vein %s: indicator %s must be a micro detail material" % [n, ind])
			ok = false
	return ok
