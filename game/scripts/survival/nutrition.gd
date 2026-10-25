class_name Nutrition
extends RefCounted

## Food groups (ADR 0061, Vintage Story nutrition), data in data/nutrition.json: one bar per group,
## filled by eating (the food's items.json "nutrition", or a tag rule), drained over time; a balanced
## diet raises max health, empty groups lower it a little. Pure model: Survival owns one, ticks it in
## survival mode and applies max_health(); the inventory screen's NutritionPanel draws it.

var groups: Array = []        # [{id, name, color}] in data order
var bars := {}                # group id -> points 0..max
var max_points := 10.0
var start := 4.0
var drain_s := 150.0
var good := 5.0
var empty := 0.5
var bonus_hp := 2
var bonus_min_groups := 2
var penalty_hp := 2
var min_health := 12
var tag_rules: Array = []
var by_id := {}               # item id -> {group: points}: other parts' foods, kept in nutrition.json
var errors: Array[String] = []

static var _default := {}


static func load_default() -> Nutrition:
	var n := Nutrition.new()
	if _default.is_empty():
		var fa := FileAccess.open("res://data/nutrition.json", FileAccess.READ)
		if fa != null:
			var d = JSON.parse_string(fa.get_as_text())
			if d is Dictionary:
				_default = d
	n.load_data(_default)
	return n


func load_data(d: Dictionary) -> bool:
	max_points = float(d.get("max", 10.0))
	start = float(d.get("start", 4.0))
	drain_s = maxf(1.0, float(d.get("drain_s", 150.0)))
	good = float(d.get("good", 5.0))
	empty = float(d.get("empty", 0.5))
	bonus_hp = int(d.get("bonus_hp", 2))
	bonus_min_groups = int(d.get("bonus_min_groups", 2))
	penalty_hp = int(d.get("penalty_hp", 2))
	min_health = int(d.get("min_health", 12))
	groups = d.get("groups", [])
	tag_rules = d.get("tag_rules", [])
	by_id.clear()
	var ids_in: Dictionary = d.get("items", {})
	for id in ids_in:
		if ids_in[id] is Dictionary:
			by_id[String(id)] = ids_in[id]
	if groups.is_empty():
		errors.append("nutrition.json: no groups")
	var ids := group_ids()
	for id in by_id:
		for g in by_id[id]:
			if not String(g) in ids:
				errors.append("nutrition.json: %s names unknown group %s" % [id, g])
	for r in tag_rules:
		if not String(r.get("group", "")) in ids:
			errors.append("nutrition.json: tag rule %s names unknown group %s" % [r.get("tag", "?"), r.get("group", "?")])
	reset()
	return errors.is_empty()


func group_ids() -> Array:
	return groups.map(func(g: Dictionary) -> String: return String(g["id"]))


func reset() -> void:
	bars.clear()
	for id in group_ids():
		bars[id] = start


## What eating one of `def` gives, {group: points}, before freshness.
func food_groups(def: Dictionary) -> Dictionary:
	var out := {}
	var n: Variant = def.get("nutrition", by_id.get(String(def.get("id", "")), null))
	if n is Dictionary:
		for g in n:
			if bars.has(String(g)):
				out[String(g)] = float(n[g])
		return out
	if not def.has("food"):
		return out
	var tags: Array = def.get("tags", [])
	var hunger := float(def["food"].get("hunger", 1))
	for r in tag_rules:
		if String(r["tag"]) in tags:
			out[String(r["group"])] = maxf(1.0, roundf(hunger * float(r.get("share", 0.8))))
			break
	return out


## Fills the bars for eating `def` at `spoil` (stale food, 0.5 or more, gives half). Returns what it gave.
func eat(def: Dictionary, spoil: float = 0.0) -> Dictionary:
	var k := 0.5 if spoil >= 0.5 else 1.0
	var given := food_groups(def)
	for g in given:
		given[g] = float(given[g]) * k
		bars[g] = minf(max_points, float(bars[g]) + float(given[g]))
	return given


## Drains every bar by `seconds` of time at `mult` (Survival passes the hunger multiplier: sprinting, cold).
func drain(seconds: float, mult: float = 1.0) -> void:
	var d := seconds * mult / drain_s
	for g in bars:
		bars[g] = maxf(0.0, float(bars[g]) - d)


func groups_good() -> int:
	var n := 0
	for g in bars:
		if float(bars[g]) >= good:
			n += 1
	return n


func groups_empty() -> int:
	var n := 0
	for g in bars:
		if float(bars[g]) < empty:
			n += 1
	return n


## Extra max health from a balanced diet: bonus_hp per well-fed group once bonus_min_groups are.
func bonus() -> int:
	var n := groups_good()
	return n * bonus_hp if n >= bonus_min_groups else 0


func penalty() -> int:
	return groups_empty() * penalty_hp


## Max health for a body whose base is `base` (half-hearts), never below min_health.
func max_health(base: int) -> int:
	return maxi(mini(base, min_health), base + bonus() - penalty())


func to_dict() -> Dictionary:
	var d := {}
	for g in bars:
		d[g] = snappedf(float(bars[g]), 0.001)
	return d


func load_dict(d: Dictionary) -> void:
	reset()
	for g in d:
		if bars.has(String(g)):
			bars[String(g)] = clampf(float(d[g]), 0.0, max_points)


func snapshot() -> Dictionary:
	var b := {}
	for g in bars:
		b[g] = snappedf(float(bars[g]), 0.01)
	return {"bars": b, "good": groups_good(), "empty": groups_empty(), "bonus_hp": bonus(), "penalty_hp": penalty()}
