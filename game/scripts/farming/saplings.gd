class_name Saplings
extends RefCounted

## Saplings (ADR 0112): leaves drop them (mining.json), a sapling set on soil is tracked here and,
## after its grow time (data/saplings.json), grows into its kind's tree: the instant-tree gadget's
## stamp (a leaf crown filled into the air, then a bark trunk with the log inside) sized per kind.
## A sapling waits while its spot is outside the simulated area or the trunk's column is blocked;
## one that is no longer there (dug, burnt, built over) is forgotten.

const SAVE_VERSION := 1
const RETRY_S := 60.0         # a blocked sapling tries again this much later

var trees := {}               # kind -> definition (data/saplings.json)
var grow_s := Vector2(600, 1200)
var planted := {}             # Vector3i -> {kind, left_s}
var by_material := {}         # sapling material id -> kind
var errors: Array[String] = []
var get_voxel: Callable       # (Vector3i) -> material id, -1 where nothing is simulated
var fill_box: Callable        # (Vector3i lo, Vector3i hi exclusive, material id) -> int, < 0 when it failed
var fill_ball: Callable       # (Vector3 centre, float radius in voxels, material id): fills the air only
var rng := RandomNumberGenerator.new()
var dirty := false
var stats := {"planted": 0, "grown": 0, "lost": 0, "blocked": 0}


static func load_default() -> Saplings:
	var s := Saplings.new()
	var fa := FileAccess.open("res://data/saplings.json", FileAccess.READ)
	if fa == null:
		s.errors.append("cannot open data/saplings.json")
	else:
		s.load_json(fa.get_as_text())
	return s


func load_json(text: String) -> bool:
	var data = JSON.parse_string(text)
	if typeof(data) != TYPE_DICTIONARY or not data.has("trees"):
		errors.append("saplings.json: expected {trees: {...}}")
		return false
	var g: Array = data.get("grow_s", [600, 1200])
	grow_s = Vector2(float(g[0]), float(g[1]))
	for kind in data["trees"]:
		var t: Dictionary = data["trees"][kind]
		var ok := true
		for key in ["sapling", "log", "bark", "leaves"]:
			if GameMaterials.id_of(String(t.get(key, ""))) <= 0:
				errors.append("saplings.json: %s %s is not a material" % [kind, key])
				ok = false
		if ok:
			trees[kind] = t
			by_material[GameMaterials.id_of(t["sapling"])] = kind
	return errors.is_empty()


## Something was placed at p: a sapling starts growing.
func on_placed(p: Vector3i, m: int) -> void:
	if by_material.has(m):
		planted[p] = {"kind": by_material[m], "left_s": rng.randf_range(grow_s.x, grow_s.y)}
		stats["planted"] += 1
		dirty = true


## Advances every sapling by dt game seconds and grows the ripe ones. Returns the spots that grew.
func tick(dt: float) -> Array:
	var grown := []
	for p: Vector3i in planted.keys():
		var s: Dictionary = planted[p]
		s["left_s"] = float(s["left_s"]) - dt
		if s["left_s"] > 0.0:
			continue
		var v: int = get_voxel.call(p)
		if v < 0:
			continue   # not simulated: wait until the player comes near
		if not by_material.has(v):
			planted.erase(p)
			stats["lost"] += 1
		elif grow(p, by_material[v]):
			planted.erase(p)
			grown.append(p)
		else:
			s["left_s"] = RETRY_S
			stats["blocked"] += 1
		dirty = true
	return grown


## Grows `kind`'s tree with its trunk's foot on p. False (nothing changed) when part of the tree is
## not simulated or the trunk's column holds anything but air, plants or the sapling.
func grow(p: Vector3i, kind: String) -> bool:
	var t: Dictionary = trees[kind]
	var w := maxi(1, roundi(Settings.m2v(float(t["trunk_m"]))))
	var h := roundi(Settings.m2v(rng.randf_range(float(t["height_m"][0]), float(t["height_m"][1]))))
	var crown := Settings.m2v(float(t["crown_m"]))
	var base := p - Vector3i(w / 2, 0, w / 2)
	var c := ceili(crown)
	if get_voxel.call(base - Vector3i(c, 0, c)) < 0 or get_voxel.call(base + Vector3i(w + c, h + c, w + c)) < 0:
		return false
	var defs := GameMaterials.defs()
	for y in range(1, h):   # the sapling's own layer is the ground it replaces
		for x in w:
			for z in w:
				var v: int = get_voxel.call(base + Vector3i(x, y, z))
				if v < 0:
					return false
				if v != GameMaterials.AIR and not (defs[v - 1].get("replaceable", false) or defs[v - 1].has("sapling") or defs[v - 1].has("clutter")):
					return false
	var top := Vector3(base) + Vector3(w * 0.5, h, w * 0.5)
	var leaves := GameMaterials.id_of(t["leaves"])
	if t.get("crown", "ball") == "cone":
		for i in 4:   # small at the tip, widest at the bottom
			fill_ball.call(top - Vector3(0, i * crown * 0.7, 0), crown * (0.45 + i * 0.25), leaves)
	else:
		fill_ball.call(top - Vector3(0, crown * 0.3, 0), crown, leaves)
	if fill_box.call(base, base + Vector3i(w, h, w), GameMaterials.id_of(t["bark"])) < 0:
		return false
	if w > 2:
		fill_box.call(base + Vector3i(1, 0, 1), base + Vector3i(w - 1, h, w - 1), GameMaterials.id_of(t["log"]))
	stats["grown"] += 1
	return true


func to_dict() -> Dictionary:
	var out := []
	for p: Vector3i in planted:
		out.append([p.x, p.y, p.z, planted[p]["kind"], planted[p]["left_s"]])
	return {"version": SAVE_VERSION, "planted": out}


func load_dict(d: Dictionary) -> bool:
	if int(d.get("version", 0)) != SAVE_VERSION:
		return false
	planted.clear()
	for e in d.get("planted", []):
		if trees.has(String(e[3])):
			planted[Vector3i(int(e[0]), int(e[1]), int(e[2]))] = {"kind": String(e[3]), "left_s": float(e[4])}
	return true
