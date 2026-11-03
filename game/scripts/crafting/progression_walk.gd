extends RefCounted

## The progression walk (ADR 0139): the survival road from a fresh spawn (empty hands) to the last
## LV quest, walked over the game data alone. Unlike RecipeRegistry.reachable_from it gates every
## step on what the game needs besides the ingredients: the station, the tool class and tier a
## material wants before it drops, the heat (firepit, forge, anvil tier), fuel and a firestarter,
## steam (boiler and pipe) and electric power (a dynamo and a cable).
## walk() returns, per quest chapter, the dead ends, the raw totals of the main road, a play time
## estimate (mining speeds per tool tier, vein sizes, the seconds below) and the outliers.
## The seconds are guesses written down once, so the numbers can be compared between data
## changes; they are reported, never asserted. Report: tools/progression_report.tscn.

const CHAPTER_AGE := {"flint_age": "flint", "middle_stone_age": "flint", "new_stone_age": "flint",
	"copper_stone_age": "copper", "copper_age": "copper", "bronze_age": "bronze", "iron_age": "iron",
	"steam_age": "steam / steel", "electrical_age": "LV electrical", "lv_workshop": "LV electrical"}
## Materials that only exist where a player placed them (their drop gives the item back).
const PLACED := ["rope", "furniture", "work_spot", "work_block", "tnt", "carpet", "torch", "torch_stick", "torch_wrap",
	"storage_pot", "furnace_lit"]
## What StoneAge.WORLD_SOURCES need before the world gives them.
const WORLD_GATES := {"birch_bark": ["#knife"], "tinder_fungus": ["birch_bark"], "fire_ember": ["tinder_fungus", "@fire"],
	"flatbread_dough": ["flour"], "brine_pot": ["cooking_pot"], "dugout_canoe": ["#axe"], "einkorn": ["einkorn_seeds"],
	"emmer": ["emmer_seeds"], "flax": ["flax_seeds"]}
## Stations that run on a boiler's steam (ADR 0097, 0099).
const STEAM_WORKS := ["ore_crusher", "steam_hammer", "steam_sieve", "blast_furnace"]
## Item tags that mark a thing made once and kept (tools, stations, molds, vessels).
const CAPITAL_TAGS := ["tool", "station", "mold", "vessel", "machine", "container", "pan", "clothing", "armor"]

# ---- time assumptions, seconds of play (ADR 0139) ----
const PICKUP_S := 5.0        # walk to one loose piece on the ground and pick it up
const PLANT_S := 2.0         # cut one plant for a chance drop (grass, ferns, leaves)
const HUNT_S := 90.0         # find, chase and kill one animal
const MILK_S := 30.0
const EGG_S := 60.0
const CROP_S := 20.0         # sow, tend and reap one plant (the growing days are not counted)
const LOOT_S := 600.0        # find and search one ruin container
const WORLD_S := 30.0        # bark, tinder, brine ... (StoneAge.WORLD_SOURCES)
const FILL_S := 10.0         # fill a container at water or lava
const CLICK_S := 0.4         # one shake, stroke or hammer blow
const HANDS := {"shaped": 3.0, "shapeless": 3.0, "spellcraft": 3.0, "assemble": 5.0, "knap": 30.0, "clayform": 40.0,
	"smith": 60.0, "cast": 5.0, "kiln": 15.0, "charcoal_pit": 5.0, "dry": 5.0, "process": 3.0, "smelt": 3.0, "melt": 5.0,
	"bury": 20.0, "rack": 5.0, "soak": 5.0, "trap": 10.0, "sift": 0.0, "scrape": 0.0, "stroke": 0.0}
const GROUND_Y := 280        # voxels: mean ground height the walk digs down from (sea level 256)
const SEARCH := 3.0          # a vein is found after walking SEARCH x the mean vein spacing (and back)
const INDICATOR_FADE := 0.5  # mean of the indicator chance's fade with vein depth
const CHARCOAL_PER_PIT := 64 # charcoal one sealed pit gives in one burn
const GAME_HOUR_S := 50.0    # a 1200 s day
const OUTLIER := 10.0

var items: ItemRegistry
var recipes: RecipeRegistry
var mining: MiningTable
var book: QuestBook
var veins: Dictionary
var sources := {}            # item id -> Array of {kind, what, gates, per, ...}
var have := {}               # item id -> true: reachable at all
var how := {}                # item id -> recipe id or a source dict: how it first became reachable
var cost := {}               # item id -> seconds per unit, cheapest way, best tools
var pick := {}               # item id -> cheapest recipe id ("" = a world source)
var owned := {}              # capital made so far on the walk (tools, stations, molds)
var _vein_yield := {}        # material -> {vein, blocks}
var _memo := {}              # _eff while `owned` stays the same
var _busy := {}
var _cache := {}             # per recipe: consumed, capital; per item: ways (fixed once `have` is)
var _qmemo := {}             # _eff values seen through a cycle, for one top-level query
var _cycled := false


static func load_default() -> RefCounted:
	var w = load("res://scripts/crafting/progression_walk.gd").new()
	w.items = ItemRegistry.load_default()
	w.recipes = RecipeRegistry.load_default(w.items)
	w.mining = MiningTable.load_default()
	w.book = QuestBook.load_default(w.items)
	var f := FileAccess.open("res://data/ore_veins.json", FileAccess.READ)
	w.veins = JSON.parse_string(f.get_as_text())
	return w


## The whole walk: {chapters: [...], dead: [...], off_road: [...], alternatives: [...], veins: [...], sources: {...}}.
func walk() -> Dictionary:
	_collect_sources()
	_vein_yields()
	_reach()
	_costs()
	var out := {"chapters": [], "dead": _quest_dead_ends(), "off_road": _off_road(), "alternatives": _alternatives(),
		"veins": _vein_table()}
	owned = {}
	var prev_s := 0.0
	for c in book.chapters:
		var ch := _walk_chapter(c)
		ch["outliers"].append_array(_chapter_outliers(ch, prev_s))
		prev_s = ch["total_s"]
		out["dead"].append_array(ch["dead"])
		out["chapters"].append(ch)
	return out


# ---------------- world sources ----------------

func _add_src(id: String, s: Dictionary) -> void:
	if not items.has(id):
		return
	if not sources.has(id):
		sources[id] = []
	sources[id].append(s)


static func _avg(d: Dictionary) -> float:
	var c = d.get("count", 1)
	var n := (float(c[0]) + float(c[1])) * 0.5 if c is Array else float(c)
	if d.has("binomial"):
		n += float(d["binomial"][0]) * float(d["binomial"][1])
	return n * float(d.get("chance", 1.0))


func _collect_sources() -> void:
	sources = {}
	for name in mining.materials:
		var e: Dictionary = mining.materials[name]
		if not e.has("drops") or e.get("built", false) or name in PLACED:
			continue
		var list: Array = e["drops"] if e["drops"] is Array else [{"id": e["drops"], "count": int(e.get("count", 1))}]
		for d in list:
			var id := String(d["id"])
			if id == name and not recipes.makes(id).is_empty():
				continue   # a placed item gives itself back
			var gates := []
			var want := String(e.get("tool", ""))
			if String(d.get("tool", "")) != "":
				gates.append("~%s:0" % d["tool"])
				want = String(d["tool"])
			elif e.get("requires", false) or e.has("drop_tier"):
				var tier := maxi(int(e.get("min_tier", 0)) if e.get("requires", false) else 0, int(e.get("drop_tier", 0)))
				gates.append("~%s:%d" % [want, tier])
			var per := _avg(d)
			if per <= 0.0:
				continue
			var plant: bool = e["drops"] is Array and float(d.get("chance", 1.0)) < 1.0
			_add_src(id, {"kind": "mine", "what": name, "want": want, "gates": gates, "per": per, "plant": plant})
	var mobs: Dictionary = _json("res://data/mobs.json").get("mobs", {})
	for m in mobs:
		var def: Dictionary = mobs[m]
		for key in ["drops", "drops_unsheared"]:
			for d in def.get(key, []):
				_add_src(String(d["id"]), {"kind": "mob", "what": m, "gates": [], "per": _avg(d)})
		for d in def.get("shear", []):
			_add_src(String(d["id"]), {"kind": "mob", "what": m + " (shorn)", "gates": ["~shears:0"], "per": _avg(d)})
		if def.has("milk"):
			_add_src(String(def["milk"]["gives"]), {"kind": "milk", "what": m, "gates": [String(def["milk"]["with"])], "per": 1.0})
		if def.has("lays"):
			_add_src(String(def["lays"]["item"]), {"kind": "egg", "what": m, "gates": [], "per": 1.0})
	var crops: Dictionary = _json("res://data/crops.json").get("crops", {})
	for c in crops:
		for d in crops[c].get("drops_ripe", []):
			_add_src(String(d["id"]), {"kind": "crop", "what": c, "gates": [String(crops[c]["seed"]), "#hoe"], "per": _avg(d)})
	var tables: Dictionary = _json("res://data/loot.json").get("tables", {})
	for t in tables:
		var tab: Dictionary = tables[t]
		var total := 0.0
		for d in tab.get("entries", []):
			total += float(d.get("weight", 1))
		var rolls = tab.get("rolls", 1)
		var n_rolls := (float(rolls[0]) + float(rolls[1])) * 0.5 if rolls is Array else float(rolls)
		for d in tab.get("entries", []):
			var per := _avg(d) * float(d.get("weight", 1)) / maxf(total, 1.0) * n_rolls * (1.0 - float(tab.get("empty_chance", 0.0)))
			_add_src(String(d["id"]), {"kind": "loot", "what": t, "gates": [], "per": per})
	for id in items.all_ids():
		var fills = items.get_def(id).get("fills", {})
		if fills is Dictionary:
			for liquid in fills:
				_add_src(String(fills[liquid]), {"kind": "fill", "what": liquid, "gates": [id], "per": 1.0})
	for id in StoneAge.WORLD_SOURCES:
		_add_src(id, {"kind": "world", "what": StoneAge.WORLD_SOURCES[id], "gates": WORLD_GATES.get(id, []), "per": 1.0})


func _json(path: String) -> Dictionary:
	var f := FileAccess.open(path, FileAccess.READ)
	if f == null:
		return {}
	var d = JSON.parse_string(f.get_as_text())
	return d if d is Dictionary else {}


# ---------------- gates ----------------

func _is(spec: String, pool: Dictionary) -> bool:
	if spec.begins_with("~"):
		var p := spec.substr(1).split(":")
		return _best_tool(p[0], int(p[1]), pool) != ""
	if spec.begins_with("@"):
		return not _pseudo(spec.substr(1), pool).is_empty()
	if spec.begins_with("#"):
		for id in items.items_with_tag(spec.substr(1)):
			if pool.has(id):
				return true
		return false
	return pool.has(spec)


func _all(specs: Array, pool: Dictionary) -> bool:
	for s in specs:
		if not _is(String(s), pool):
			return false
	return true


## Items in `pool` that stand for a pseudo spec: fuel, coal_fuel (the forge, bloomery and blast
## furnace burn only #coal), fire (a firestarter), dynamo (a power source), boiler.
func _pseudo(name: String, pool: Dictionary) -> Array:
	var out := []
	var watts := 0.0
	if name.begins_with("dynamo:"):   # a source that alone gives the machine's watts, if any does
		watts = float(name.substr(7))
		name = "dynamo"
		var strong := []
		for id in _pseudo("dynamo", have):
			if dynamo_watts(id) >= watts:
				strong.append(id)
		if strong.is_empty():
			watts = 0.0
	for id in pool:
		var d := items.get_def(id)
		var ok := false
		match name:
			"fuel": ok = int(d.get("fuel_ticks", 0)) > 0
			"coal_fuel": ok = int(d.get("fuel_ticks", 0)) > 0 and "coal" in d.get("tags", [])
			"fire": ok = d.get("ignites", false) is bool and d.get("ignites", false)
			"dynamo": ok = d.has("dynamo") and dynamo_watts(id) >= watts
			"boiler": ok = String(d.get("work", "")) == "boiler"
		if ok:
			out.append(id)
	return out


## Most watts a dynamo gives one matched load: V^2 / 4R.
func dynamo_watts(id: String) -> float:
	var dy: Dictionary = items.get_def(id).get("dynamo", {})
	return float(dy.get("volts", 0.0)) ** 2 / (4.0 * maxf(float(dy.get("ohms", 1.0)), 0.001))


## The fastest item in `pool` of tool class `cls` at tier >= `tier`, or "".
func _best_tool(cls: String, tier: int, pool: Dictionary) -> String:
	var best := ""
	var best_v := -1.0
	for id in items.items_with_tag(cls):
		if not pool.has(id):
			continue
		var d := items.get_def(id)
		if int(d.get("tier", 0)) < tier:
			continue
		var v := float(mining.tier_speed[clampi(int(d.get("tier", 0)), 0, mining.tier_speed.size() - 1)]) * float(d.get("speed", 1.0))
		if v > best_v:
			best_v = v
			best = id
	return best


func _fuel_spec(r: Dictionary) -> String:
	match r["type"]:
		"melt":
			return "@fuel"
		"smelt":
			match String(r.get("station", "")):
				"firepit", "furnace":
					return "@fuel"
				"bloomery", "blast_furnace":
					return "@coal_fuel"
	return ""


func _heat_s(r: Dictionary) -> float:
	return float(r.get("ticks", 200)) / 20.0


## What a recipe uses up per batch: [[spec, count], ...]; the fuel it burns is _fuel_eff.
func _consumed(r: Dictionary) -> Array:
	if not _cache.has("c" + r["id"]):
		_cache["c" + r["id"]] = _consumed_raw(r)
	return _cache["c" + r["id"]]


func _consumed_raw(r: Dictionary) -> Array:
	var tally := {}
	var add := func(spec: String, n: float) -> void: tally[spec] = float(tally.get(spec, 0.0)) + n
	match r["type"]:
		"shaped", "shapeless", "spellcraft", "process", "assemble":
			for s in recipes.ingredient_specs(r):
				if not s in r.get("keep", []) and not r.get("remainder", {}).has(s):
					add.call(String(s), 1.0)
		"bury", "rack", "stroke", "soak":
			for s in r["ingredients"]:
				add.call(String(s), 1.0)
			if r.has("cover"):
				add.call(String(r["cover"]), float(r.get("cover_count", 1)))
			if r.has("fuel"):
				add.call(String(r["fuel"]), float(r.get("fuel_count", 1)))
			if String(r.get("grit", "")) != "":
				add.call(String(r["grit"]), ceilf(float(r.get("strokes", 1)) / maxf(float(r.get("grit_every", 1)), 1.0)))
		"trap":
			pass
		"smelt":
			add.call(String(r["input"]), float(r.get("count", 1)))
		"melt":
			add.call(String(r["input"]), float(r.get("count", 1)))
			if not _vessel_kept(r):
				add.call(String(r["vessel"]), 1.0)
		"cast", "knap", "dry", "sift", "scrape":
			add.call(String(r["input"]), 1.0)
		"kiln":
			add.call(String(r["input"]), 1.0)
			add.call("dry_grass", float(GroundEntity.KILN_GRASS))
			add.call("stick", float(GroundEntity.KILN_STICKS))
		"smith":
			add.call(String(r["input"]), float(AnvilEntity.ingots_for(r)))
		"clayform":
			add.call(String(r["input"]), float(ClayFormEntity.balls_for(r)))
		"charcoal_pit":
			add.call(String(r["input"]), float(r.get("per", 1)))
	var out := []
	for s in tally:
		out.append([s, tally[s]])
	return out


## A molten vessel (crucible_copper) is used up by the alloy melt; an empty crucible comes back.
func _vessel_kept(r: Dictionary) -> bool:
	return recipes.first_of("melt", {"output": String(r["vessel"])}).is_empty()


## What a recipe needs present but does not use up: station, tools, molds, anvil, heat, power.
func _capital(r: Dictionary) -> Array:
	if not _cache.has("k" + r["id"]):
		_cache["k" + r["id"]] = _capital_raw(r)
	return _cache["k" + r["id"]]


func _capital_raw(r: Dictionary) -> Array:
	var out := []
	var st := String(r.get("station", ""))
	if st != "":
		out.append(st)
	for s in r.get("keep", []):
		out.append(String(s))
	for s in r.get("remainder", {}):
		out.append(String(s))
	if String(r.get("tool", "")) != "":
		out.append(String(r["tool"]))
	match r["type"]:
		"process":
			var d := items.get_def(st)
			var watts := float(d.get("machine", d.get("load", {})).get("watts", 0.0))
			if watts > 0.0:
				out.append_array(["@dynamo:%d" % watts, "#cable"])
			if String(d.get("work", "")) in STEAM_WORKS:
				out.append_array(["@boiler", "#pipe", "water_bucket"])
		"smelt", "melt":
			if st != "furnace":
				out.append("@fire")
			if st in STEAM_WORKS:
				out.append_array(["@boiler", "#pipe", "water_bucket"])
			if r["type"] == "melt" and _vessel_kept(r):
				out.append(String(r["vessel"]))
		"kiln", "charcoal_pit", "bury":
			out.append("@fire")
		"rack":
			out.append("drying_rack")
			if r.get("mode", "") == "smoke":
				out.append("@fire")
		"trap":
			for s in r["ingredients"]:
				out.append(String(s))
		"cast":
			out.append(String(r["mold"]))
		"smith":
			out.append("#hammer")
			var d := items.get_def(String(r["input"]))
			out.append("stone_anvil" if int(d.get("anvil_tier", 0)) <= 0 else "bronze_anvil")
			if "forge_heat" in d.get("tags", []):
				out.append_array(["forge", "@fire"])
			elif "heatable" in d.get("tags", []):
				out.append_array(["firepit", "@fire"])
	return out


func usable(r: Dictionary, pool: Dictionary) -> bool:
	if not _all(recipes.ingredient_specs(r), pool):
		return false
	for c in _consumed(r):
		if not _is(c[0], pool):
			return false
	var f := _fuel_spec(r)
	return (f == "" or _is(f, pool)) and _all(_capital(r), pool)


## The first unmet need of a recipe against `pool`, "" when none.
func missing(r: Dictionary, pool: Dictionary) -> String:
	var all := recipes.ingredient_specs(r)
	for c in _consumed(r):
		all.append(c[0])
	if _fuel_spec(r) != "":
		all.append(_fuel_spec(r))
	all.append_array(_capital(r))
	for s in all:
		if not _is(String(s), pool):
			return String(s)
	return ""


# ---------------- reachability ----------------

func _reach() -> void:
	have = {}
	_cache = {}
	how = {}
	var changed := true
	while changed:
		changed = false
		for id in sources:
			if have.has(id):
				continue
			for s in sources[id]:
				if _all(s["gates"], have):
					have[id] = true
					how[id] = s
					changed = true
					break
		for rid in recipes.recipes:
			var r: Dictionary = recipes.recipes[rid]
			var out := String(r["output"]["id"])
			if not have.has(out) and usable(r, have):
				have[out] = true
				how[out] = rid
				changed = true


# ---------------- costs ----------------

func _out(r: Dictionary) -> float:
	var n := float(r["output"].get("count", 1))
	if r["type"] == "sift":
		n *= float(r.get("chance", 1.0))
	return maxf(n, 0.0001)


func _hands(r: Dictionary) -> float:
	var t: float = HANDS.get(r["type"], 3.0)
	match r["type"]:
		"sift":
			t = (Settings.pan_shakes if String(r.get("station", "")) == "pan" else SieveEntity.SHAKES) * CLICK_S
		"stroke", "scrape":
			t = float(r.get("strokes", 1)) * CLICK_S
	return t


func _wait(r: Dictionary) -> float:
	match r["type"]:
		"smelt", "melt":
			return _heat_s(r)
		"process", "dry":
			return float(r.get("seconds", 0.0))
		"kiln":
			return Settings.pit_kiln_burn_s
		"bury", "rack", "soak":
			return float(r.get("hours", 0.0)) * GAME_HOUR_S
		"trap":
			return float(r.get("every_hours", 1.0)) * GAME_HOUR_S / maxf(float(r.get("chance", 1.0)), 0.01)
		"smith":
			return Settings.metal_heat_s
		"charcoal_pit":
			return Settings.charcoal_pit_burn_s / CHARCOAL_PER_PIT * float(r.get("per", 1))
	return 0.0


## The cheapest item in `pool` matching spec by cost (pseudo specs: their items), "" when none.
func _resolve(spec: String, pool: Dictionary) -> String:
	var cands := []
	if spec.begins_with("@"):
		cands = _pseudo(spec.substr(1), pool)
	elif spec.begins_with("~"):
		var p := spec.substr(1).split(":")
		for id in items.items_with_tag(p[0]):
			if pool.has(id) and int(items.get_def(id).get("tier", 0)) >= int(p[1]):
				cands.append(id)
	elif spec.begins_with("#"):
		for id in items.items_with_tag(spec.substr(1)):
			if pool.has(id):
				cands.append(id)
	elif pool.has(spec):
		cands = [spec]
	cands.sort()
	var best := ""
	var best_c := INF
	for id in cands:
		var c: float = cost.get(id, INF)
		if best == "" or c < best_c:
			best = id
			best_c = c
	return best


func _spec_cost(spec: String) -> float:
	var id := _resolve(spec, have)
	return INF if id == "" else float(cost.get(id, INF))


## Fuel a batch burns: [fuel item, units], or [] for none.
func _fuel_use(r: Dictionary, pool: Dictionary) -> Array:
	var f := _fuel_spec(r)
	if f == "":
		return []
	var best := ""
	var best_c := INF
	for id in _pseudo(f.substr(1), pool):
		var c := float(cost.get(id, INF)) / float(items.get_def(id)["fuel_ticks"])
		if best == "" or c < best_c:
			best = id
			best_c = c
	if best == "":
		return []
	return [best, float(r.get("ticks", 200)) / float(items.get_def(best)["fuel_ticks"])]


func _recipe_s(r: Dictionary) -> float:
	var s := _hands(r) + 0.25 * _wait(r)
	for c in _consumed(r):
		s += float(c[1]) * _spec_cost(c[0])
	var fu := _fuel_use(r, have)
	if not fu.is_empty():
		s += float(fu[1]) * float(cost.get(fu[0], INF))
	return s / _out(r)


## Seconds per unit from a world source with the tools in `pool`; trips to veins amortized over a
## vein's yield unless `trips` is false (the chapter walk counts whole trips instead).
func _src_s(s: Dictionary, pool: Dictionary, trips := true) -> float:
	match s["kind"]:
		"mine":
			var e := mining.entry(s["what"])
			if float(e["hardness"]) <= 0.0:
				var t := PLANT_S if s["plant"] else PICKUP_S
				if trips and _vein_yield.has(s["what"]):
					var y: Dictionary = _vein_yield[s["what"]]
					t += float(y["walk_s"]) / float(y["blocks"])
				return t / float(s["per"])
			var tool := _best_tool(String(s["want"]), 0, pool) if String(s["want"]) != "" else ""
			var bt := mining.break_time(s["what"], items.get_def(tool) if tool != "" else {})
			if bt < 0.0:
				return INF
			var t := maxf(bt, 0.1)
			if s["plant"]:
				t = maxf(t, PLANT_S)
			if trips and _vein_yield.has(s["what"]):
				t += _trip_s(s["what"], pool) / float(_vein_yield[s["what"]]["blocks"])
			return t / float(s["per"])
		"mob":
			return HUNT_S / maxf(float(s["per"]), 0.01)
		"milk":
			return MILK_S
		"egg":
			return EGG_S
		"crop":
			return CROP_S / maxf(float(s["per"]), 0.01)
		"loot":
			return LOOT_S / maxf(float(s["per"]), 0.001)
		"fill":
			return FILL_S
	return WORLD_S


func _costs() -> void:
	cost = {}
	pick = {}
	for loot in [false, true]:   # ruin loot prices only what nothing else gives
		for id in sources:
			if not have.has(id) or (loot and cost.has(id)):
				continue
			var s: Variant = _best_source(id, have, loot)
			if s != null:
				cost[id] = _src_s(s, have)
				pick[id] = ""
		_relax()


func _relax() -> void:
	for _pass in 60:
		var changed := false
		for rid in recipes.recipes:
			var r: Dictionary = recipes.recipes[rid]
			var out := String(r["output"]["id"])
			if not have.has(out) or not usable(r, have):
				continue
			var c := _recipe_s(r)
			if c < float(cost.get(out, INF)) * 0.999:
				cost[out] = c
				pick[out] = rid
				changed = true
		if not changed:
			break


# ---------------- veins ----------------

## Blocks (64 voxels, one recipe unit) of each ore one vein holds: a lens (ellipsoid) of radius
## 0.4 x size (the 0.8..1.0 wobble's mean) and the vein's thickness; primary in the top third,
## between in the middle, secondary in the bottom, a sporadic share of every ore voxel.
func _vein_yields() -> void:
	_vein_yield = {}
	var g: Dictionary = veins["grid"]
	var total_w := 0.0
	for v in veins["veins"]:
		total_w += float(v["weight"])
	var mid := 1.5 / 3.0 - 0.5 / 27.0   # volume share of an ellipsoid within a third of its height round the middle
	var sp := float(g["sporadic_chance"])
	for v in veins["veins"]:
		var size := float(v.get("size_voxels", g["size_voxels"]))
		var thick := float(v.get("thickness_voxels", g["thickness_voxels"]))
		var a := 0.4 * size
		var ore := 4.0 / 3.0 * PI * a * a * thick * 0.5 * float(g["density"]) / 64.0
		var share := {}
		for k in [["primary", (1.0 - mid) * 0.5], ["secondary", (1.0 - mid) * 0.5], ["between", mid]]:
			var m := String(v[k[0]])
			share[m] = float(share.get(m, 0.0)) + ore * float(k[1]) * (1.0 - sp)
		share[String(v["sporadic"])] = float(share.get(String(v["sporadic"]), 0.0)) + ore * sp
		var spacing_m := float(g["cell_voxels"]) / 8.0 / sqrt(float(g["fill"]) * float(v["weight"]) / total_w)
		var top := (float(v["y_min"]) + float(v["y_max"])) * 0.5 + thick * 0.5
		var depth_m := maxf(0.0, GROUND_Y - top) / 8.0
		var walk_s := SEARCH * spacing_m / Settings.walk_speed_m
		for m in share:
			var y: Dictionary = _vein_yield.get(m, {})
			if y.is_empty() or float(share[m]) / (walk_s + depth_m) > float(y["blocks"]) / (float(y["walk_s"]) + float(y["depth_m"])):
				_vein_yield[m] = {"vein": v["name"], "blocks": float(share[m]), "walk_s": walk_s, "depth_m": depth_m}
		var ind := String(v.get("indicator", ""))
		if ind != "":
			var ig: Dictionary = veins["indicators"]
			var lo := float(v["y_min"]) + thick * 0.5
			var hi := float(v["y_max"]) + thick * 0.5
			var shown := clampf((hi - (GROUND_Y - float(ig["max_depth_voxels"]))) / maxf(hi - lo, 1.0), 0.0, 1.0)
			var pieces := PI * pow(float(ig["radius_voxels"]), 2) * float(ig["chance"]) * INDICATOR_FADE
			if shown > 0.0 and (not _vein_yield.has(ind) or pieces * shown / walk_s > float(_vein_yield[ind]["blocks"]) / float(_vein_yield[ind]["walk_s"])):
				_vein_yield[ind] = {"vein": v["name"], "blocks": pieces, "walk_s": walk_s / shown, "depth_m": 0.0}


## Seconds for one trip to a vein of `material`: the search walk, then a stair down to its top
## (1 m x 2 m, 1.4 m of stair per metre down) cut with the best pickaxe in `pool`.
func _trip_s(material: String, pool: Dictionary) -> float:
	var y: Dictionary = _vein_yield[material]
	var pick_id := _best_tool("pickaxe", 0, pool)
	var stone := mining.break_time("stone", items.get_def(pick_id) if pick_id != "" else {})
	return float(y["walk_s"]) + float(y["depth_m"]) * 1.4 * 2.0 * 8.0 * stone


func _vein_table() -> Array:
	var out := []
	for m in _vein_yield:
		var y: Dictionary = _vein_yield[m]
		out.append({"material": m, "vein": y["vein"], "blocks": y["blocks"], "walk_s": y["walk_s"], "depth_m": y["depth_m"]})
	out.sort_custom(func(a: Dictionary, b: Dictionary) -> bool: return a["material"] < b["material"])
	return out


# ---------------- the chapter walk ----------------

func _new_acc() -> Dictionary:
	return {"raw": {}, "raw_s": {}, "step_s": {}, "gather_s": 0.0, "hands_s": 0.0, "wait_s": 0.0, "steps": {}, "dead": [], "ore": {}, "made": [], "loot": {}}


func _is_capital_item(id: String) -> bool:
	var d := items.get_def(id)
	if d.has("durability") or d.has("work") or d.has("machine") or d.has("furniture"):
		return true
	for t in d.get("tags", []):
		if t in CAPITAL_TAGS:
			return true
	return false


## The main road of a chapter: its final quest and the deps inside the chapter, deps first.
func main_road(chapter: Dictionary) -> Array:
	var order := []
	var seen := {}
	var visit := func(id: String, f: Callable) -> void:
		if seen.has(id) or not book.quests.has(id) or book.quests[id]["chapter"] != chapter["id"]:
			return
		seen[id] = true
		for d in book.quests[id].get("deps", []):
			f.call(String(d), f)
		order.append(id)
	for q in chapter["quests"]:
		if q.get("final", false):
			visit.call(String(q["id"]), visit)
	return order


func _walk_chapter(c: Dictionary) -> Dictionary:
	var acc := _new_acc()
	var tools := {}
	for cls in ["pickaxe", "axe", "shovel", "knife"]:
		tools[cls] = _best_tool(cls, 0, owned)
	var quest_s := {}
	var road := main_road(c)
	for qid in road:
		var before: float = acc["gather_s"] + acc["hands_s"]
		for t in book.quests[qid]["tasks"]:
			var specs: Array = QuestBook.task_specs(t)
			var spec := String(specs[0])
			for s in specs:   # an "any of" task: the cheapest
				if _spec_cost(String(s)) < _spec_cost(spec):
					spec = String(s)
			var id := _resolve(spec, have)
			if id == "":
				acc["dead"].append({"quest": qid, "need": spec, "path": []})
				continue
			var n := float(QuestBook.task_count(t)) / float(items.units(id))
			if _is_capital_item(id) and n <= 1.0:
				_own(spec, acc, 0, [qid])
			else:
				_expand(spec, n, acc, 0, [qid])
		quest_s[qid] = acc["gather_s"] + acc["hands_s"] - before
	# whole trips to veins for the ore this chapter dug, with the pickaxe it had by then
	var trips := {}
	var trip_s := 0.0
	for m in acc["ore"]:
		var y: Dictionary = _vein_yield[m]
		var n := ceili(float(acc["ore"][m]) / float(y["blocks"]) - 0.001)
		var t := _trip_s(m, owned) if float(y["depth_m"]) > 0.0 else float(y["walk_s"])
		trips[m] = {"trips": n, "blocks": acc["ore"][m], "vein_blocks": y["blocks"], "vein": y["vein"], "trip_s": t}
		trip_s += n * t
	var total: float = acc["gather_s"] + acc["hands_s"] + trip_s
	var outliers := []
	for m in acc["made"]:   # power: the strongest dynamo owned against each machine made (max transfer V^2 / 4R)
		var watts := float(items.get_def(m).get("machine", items.get_def(m).get("load", {})).get("watts", 0.0))
		var best := ""
		var best_w := 0.0
		for d in _pseudo("dynamo", owned):
			var w := dynamo_watts(d)
			if w > best_w:
				best = d
				best_w = w
		if watts > 0.0 and best != "" and watts > best_w:
			outliers.append("%s draws %.0f W; the walk's %s gives at most %.0f W: %d of them" % [m, watts, best, best_w, ceili(watts / best_w)])
	return {"id": c["id"], "title": c.get("title", c["id"]), "age": CHAPTER_AGE.get(c["id"], "?"), "road": road,
		"quest_s": quest_s, "raw": acc["raw"], "raw_s": acc["raw_s"], "step_s": acc["step_s"], "loot": acc["loot"].keys(), "steps": acc["steps"], "dead": acc["dead"], "trips": trips,
		"gather_s": acc["gather_s"], "hands_s": acc["hands_s"], "wait_s": acc["wait_s"], "trip_s": trip_s,
		"total_s": total, "tools": tools, "made": acc["made"], "outliers": outliers}


## Make sure the walk owns something matching `spec` (made once, kept).
func _own(spec: String, acc: Dictionary, depth: int, path: Array) -> void:
	if _is(spec, owned):
		return
	var id := _resolve_cap(spec)
	if id == "":
		acc["dead"].append({"quest": path[0], "need": spec, "path": path.slice(1)})
		return
	owned[id] = true
	_memo = {}
	acc["made"].append(id)
	_expand(id, 1.0, acc, depth + 1, path)


func _best_source(id: String, pool: Dictionary, loot := false) -> Variant:
	var best: Variant = null
	var best_c := INF
	for s in sources.get(id, []):
		if (s["kind"] == "loot") != loot or not _all(s["gates"], have):
			continue
		var c := _src_s(s, pool)
		if c < best_c:
			best = s
			best_c = c
	return best


## Items in `pool` a spec stands for (ids, #tags, ~tool:tier, @pseudo).
func _cands(spec: String, pool: Dictionary) -> Array:
	if is_same(pool, have) and _cache.has("n" + spec):
		return _cache["n" + spec]
	var out := []
	if spec.begins_with("@"):
		out = _pseudo(spec.substr(1), pool)
	elif spec.begins_with("~"):
		var p := spec.substr(1).split(":")
		for id in items.items_with_tag(p[0]):
			if pool.has(id) and int(items.get_def(id).get("tier", 0)) >= int(p[1]):
				out.append(id)
	elif spec.begins_with("#"):
		for id in items.items_with_tag(spec.substr(1)):
			if pool.has(id):
				out.append(id)
	elif pool.has(spec):
		out = [spec]
	out.sort()
	if is_same(pool, have):
		_cache["n" + spec] = out
	return out


## The ways the world and the recipes give `id`; ruin loot only when there is no other.
func _ways(id: String) -> Array:
	if _cache.has("w" + id):
		return _cache["w" + id]
	var ways := []
	for s in sources.get(id, []):
		if s["kind"] != "loot" and _all(s["gates"], have):
			ways.append(s)
	for rid in recipes.makes(id):
		if usable(recipes.recipes[rid], have):
			ways.append(rid)
	if ways.is_empty():
		for s in sources.get(id, []):
			if _all(s["gates"], have):
				ways.append(s)
	_cache["w" + id] = ways
	return ways


## [per unit, set-up] seconds to get `id` from what the walk owns now: the set-up is the kept
## things (stations, tools, molds) it still lacks, each with its own set-up, paid once. [INF, INF]
## when only a cycle through `id` itself would make it (copper ore before any copper pickaxe).
func _eff(id: String, depth := 0) -> Array:
	if _busy.has(id) or depth > 14:
		_cycled = true
		return [INF, INF]
	if _memo.has(id):
		return _memo[id]
	if _qmemo.has(id):
		return _qmemo[id]
	_busy[id] = true
	var outer := _cycled
	_cycled = false
	var best := [INF, INF]
	for w in _ways(id):
		var e := _eff_way(w, depth)
		if e[0] + e[1] < best[0] + best[1]:
			best = e
	_busy.erase(id)
	if not _cycled:   # a value seen through a cycle still in work depends on the path: kept for this query only
		_memo[id] = best
	else:
		_qmemo[id] = best
	_cycled = _cycled or outer
	return best


func _eff_way(w: Variant, depth: int) -> Array:
	var u := 0.0
	var s := 0.0
	var caps: Array
	if w is Dictionary:
		u = _src_s(w, owned if _all(w["gates"], owned) else have)
		caps = w["gates"]
		if w["kind"] in ["milk", "fill"]:   # the bowl or bucket goes out full
			for g in caps:
				var e := _eff_spec(String(g), owned, depth + 1)
				u += e[0]
				s += e[1]
			caps = []
	else:
		var r: Dictionary = recipes.recipes[w]
		u = _hands(r) + 0.25 * _wait(r)
		for c in _consumed(r):
			var e := _eff_spec(String(c[0]), {}, depth + 1)
			u += float(c[1]) * e[0]
			s += e[1]
		var fu := _fuel_eff(r, depth + 1)
		if not fu.is_empty():
			u += fu[0]
			s += fu[1]
		u /= _out(r)
		caps = _capital(r)
	for g in caps:
		if not _is(String(g), owned):
			var e := _eff_spec(String(g), {}, depth + 1)
			s += e[0] + e[1]
	return [u, s]


## The fuel a batch burns, cheapest from what the walk owns now: [per batch, set-up, fuel id, units], or [].
func _fuel_eff(r: Dictionary, depth: int) -> Array:
	var f := _fuel_spec(r)
	var best := []
	if f == "":
		return best
	if not _cache.has("f" + f):   # the four cheapest fuels per tick: enough to choose from, and fast
		var fuels := _pseudo(f.substr(1), have)
		fuels.sort_custom(func(a: String, b: String) -> bool: return float(cost.get(a, INF)) / float(items.get_def(a)["fuel_ticks"]) < float(cost.get(b, INF)) / float(items.get_def(b)["fuel_ticks"]))
		_cache["f" + f] = fuels.slice(0, 4)
	for id in _cache["f" + f]:
		var units := float(r.get("ticks", 200)) / float(items.get_def(id)["fuel_ticks"])
		var e := _eff(id, depth)
		if best.is_empty() or e[0] * units + e[1] < best[0] + best[1]:
			best = [e[0] * units, e[1], id, units]
	return best


## The cheapest candidate of a spec: [per unit, set-up, id]; owned candidates (in `free`) cost nothing.
func _eff_spec(spec: String, free: Dictionary, depth: int) -> Array:
	var best := [INF, INF, ""]
	for id in _cands(spec, have):
		if free.has(id):
			return [0.0, 0.0, id]
		var e := _eff(id, depth)
		if best[2] == "" or e[0] + e[1] < best[0] + best[1]:
			best = [e[0], e[1], id]
	return best


## The candidate for a kept spec that is cheapest to make from what the walk owns now.
func _resolve_cap(spec: String) -> String:
	_qmemo = {}
	return String(_eff_spec(spec, {}, 0)[2])


## How the walk makes `n` of `id` now: the way cheapest for n with the set-up it still lacks counted
## in once (a bending press pays off for forty plates, not for one).
func _choose(id: String, n := 1.0) -> Variant:
	_qmemo = {}
	var best: Variant = null
	var best_c := INF
	for w in _ways(id):
		var e := _eff_way(w, 0)
		if best == null or n * e[0] + e[1] < best_c:
			best = w
			best_c = n * e[0] + e[1]
	return best


func _expand(spec: String, n: float, acc: Dictionary, depth: int, path: Array) -> void:
	if n <= 0.0:
		return
	var id := _resolve(spec, have)
	if id == "" or depth > 40:
		acc["dead"].append({"quest": path[0], "need": spec + (" (too deep)" if id != "" else ""), "path": path.slice(1)})
		return
	var how_now: Variant = _choose(id, n)
	if how_now == null:
		acc["dead"].append({"quest": path[0], "need": id, "path": path.slice(1)})
		return
	if how_now is Dictionary:
		var s: Dictionary = how_now
		for g in s["gates"]:
			if s["kind"] in ["milk", "fill"]:   # the bowl or bucket goes out full
				_expand(String(g), n, acc, depth + 1, path + [id])
			else:   # tools, seed stock, a hoe
				_own(String(g), acc, depth, path + [id])
		acc["raw"][id] = float(acc["raw"].get(id, 0.0)) + n
		var t := n * _src_s(s, owned, false)
		acc["gather_s"] += t
		acc["raw_s"][id] = float(acc["raw_s"].get(id, 0.0)) + t
		if s["kind"] == "loot":
			acc["loot"][id] = true
		if _vein_yield.has(s.get("what", "")):
			acc["ore"][s["what"]] = float(acc["ore"].get(s["what"], 0.0)) + n / float(s["per"])
		return
	var r: Dictionary = recipes.recipes[how_now]
	var b := n / _out(r)
	for g in _capital(r):
		_own(String(g), acc, depth, path + [id])
	for c in _consumed(r):
		_expand(String(c[0]), float(c[1]) * b, acc, depth + 1, path + [id])
	_qmemo = {}
	var fu := _fuel_eff(r, 0)
	if not fu.is_empty():
		_expand(String(fu[2]), float(fu[3]) * ceilf(b - 0.001), acc, depth + 1, path + [id])
	var batches := ceilf(b - 0.001)
	acc["hands_s"] += _hands(r) * batches
	acc["wait_s"] += _wait(r) * batches
	acc["step_s"][how_now] = float(acc["step_s"].get(how_now, 0.0)) + _hands(r) * batches
	acc["steps"][how_now] = float(acc["steps"].get(how_now, 0.0)) + b


# ---------------- dead ends and outliers ----------------

func _quest_dead_ends() -> Array:
	var out := []
	for qid in book.quests:
		for t in book.quests[qid]["tasks"]:
			var ok := false
			for s in QuestBook.task_specs(t):
				ok = ok or _is(String(s), have)
			if not ok:
				out.append({"quest": qid, "need": " / ".join(QuestBook.task_specs(t)), "path": _why_not(String(QuestBook.task_specs(t)[0]))})
	return out


## The chain down to the first thing nothing makes or gives: [item, what its recipe lacks, ...].
func _why_not(spec: String) -> Array:
	var chain := []
	for _i in 6:
		var id := spec
		if spec.begins_with("#"):
			var members := items.items_with_tag(spec.substr(1))
			if members.is_empty():
				break
			id = String(members[0])
		var rids := recipes.makes(id)
		if rids.is_empty():
			break
		var lack := missing(recipes.recipes[rids[0]], have)
		if lack == "" or lack in chain:
			break
		chain.append(lack)
		spec = lack
	return chain


## Recipes nothing on the road can run, with the first thing they lack (reported, not failed).
func _off_road() -> Array:
	var out := []
	var ids: Array = recipes.recipes.keys()
	ids.sort()
	for rid in ids:
		var r: Dictionary = recipes.recipes[rid]
		if not usable(r, have):
			out.append({"recipe": rid, "lacks": missing(r, have)})
	return out


## Items with two recipes whose costs per unit lie OUTLIER x or more apart.
func _alternatives() -> Array:
	var out := []
	for id in have:
		var ways := []
		for rid in recipes.makes(id):
			if usable(recipes.recipes[rid], have):
				ways.append({"how": rid, "s": _recipe_s(recipes.recipes[rid])})
		if ways.size() < 2:
			continue
		ways.sort_custom(func(a: Dictionary, b: Dictionary) -> bool: return a["s"] < b["s"])
		var lo: float = ways[0]["s"]
		var hi: float = ways[-1]["s"]
		if lo > 0.0 and hi < INF and hi / lo >= OUTLIER:
			out.append({"item": id, "best": ways[0], "worst": ways[-1], "ratio": hi / lo})
	out.sort_custom(func(a: Dictionary, b: Dictionary) -> bool: return a["ratio"] > b["ratio"])
	return out


func _chapter_outliers(ch: Dictionary, prev_s: float) -> Array:
	var out := []
	var times: Array = ch["quest_s"].values()
	times.sort()
	if times.size() >= 3:
		var med: float = maxf(times[times.size() / 2], 1.0)
		for q in ch["quest_s"]:
			if ch["quest_s"][q] >= OUTLIER * med and ch["quest_s"][q] > 300.0:
				out.append("quest %s takes %s, %.0fx the chapter's median quest (%s)" % [q, hms(ch["quest_s"][q]), ch["quest_s"][q] / med, hms(med)])
	if prev_s > 0.0 and ch["total_s"] / prev_s >= OUTLIER:
		out.append("the chapter takes %.0fx the one before (%s against %s)" % [ch["total_s"] / prev_s, hms(ch["total_s"]), hms(prev_s)])
	if prev_s > 0.0 and ch["total_s"] > 0.0 and prev_s / ch["total_s"] >= OUTLIER:
		out.append("the chapter takes 1/%.0f of the one before (%s against %s)" % [prev_s / ch["total_s"], hms(ch["total_s"]), hms(prev_s)])
	for m in ch["trips"]:
		var t: Dictionary = ch["trips"][m]
		if float(t["vein_blocks"]) / maxf(float(t["blocks"]), 0.001) >= OUTLIER:
			out.append("one %s vein holds %.0f units of %s, %.0fx the %.1f the chapter needs" % [t["vein"], t["vein_blocks"], m, float(t["vein_blocks"]) / float(t["blocks"]), t["blocks"]])
	return out


static func hms(s: float) -> String:
	if s >= 3600.0:
		return "%.1f h" % (s / 3600.0)
	if s >= 60.0:
		return "%.0f min" % (s / 60.0)
	return "%.0f s" % s
