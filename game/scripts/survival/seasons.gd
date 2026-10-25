class_name Seasons
extends RefCounted

## Seasons that matter (ADR 0061). The calendar (ADR 0025) already rests crops in winter; this adds:
##   - frost: a crop whose column is colder than its crops.json "frost_c" (default 0 C) dies back to a
##     dead plant; hardy crops (a low frost_c) wait for spring. A crop in a closed room (a greenhouse,
##     the ClimateProbe fill) is spared;
##   - wild food follows the season: a drop entry with "seasons": ["summer", ...] only drops then
##     (blueberry bushes bear in summer and autumn);
##   - animals are leaner in winter: a drop entry with "winter_mult" gives that share (at least one);
##   - the root cellar: a container in a closed room at least Settings.root_cellar_m under the surface
##     spoils at the cellar's 8 C and Settings.root_cellar_mult of that again.
## The rules are static (tested headless); a Seasons instance runs the frost checks over Farming.

const DEAD_CROP := "dead_bush"     # what a frozen crop leaves: a dry, brown plant (no new material)
## The current season (Calendar.season(): 0 winter .. 3 autumn), set by Main; -1 = no seasons (tests, tools).
static var season := -1

var farming: Farming
var temp_at: Callable        # (Vector3i) -> outdoor C at that voxel's column now
var sheltered: Callable      # (Vector3i) -> bool: in a closed room; invalid = never
var stats := {"frost_checks": 0, "killed": 0, "hardy": 0, "sheltered": 0}
var _shelter := {}           # block -> [sheltered, valid until days]
const PROBES_PER_CHECK := 4  # new room fills per frost check (a fill is up to ~1 ms)
const SHELTER_DAYS := 0.5


# ---------------- rules ----------------

## Does a temperature of temp_c kill this crop?
static func kills(crop_def: Dictionary, temp_c: float) -> bool:
	return temp_c < float(crop_def.get("frost_c", 0.0))


## Does a drop entry drop in this season? Entries without "seasons" always do.
static func in_season(entry: Dictionary, p_season: int) -> bool:
	if p_season < 0 or not entry.has("seasons"):
		return true
	return String(Calendar.SEASONS[p_season]).to_lower() in entry["seasons"]


## The count n of a drop entry after winter leanness ("winter_mult"): rounded at random, never below one.
static func lean(n: int, entry: Dictionary, p_season: int, rng: RandomNumberGenerator) -> int:
	if p_season != 0 or n <= 0 or not entry.has("winter_mult"):
		return n
	return maxi(1, floori(n * float(entry["winter_mult"]) + rng.randf()))


## How fast food in a container spoils (1 = 16 C): below_m under the surface, in a closed room or not,
## outdoor_c the air at its column. The storage pot halves it (ADR 0040).
static func container_rate(below_m: float, room: bool, outdoor_c: float, kind: String) -> float:
	var r: float
	if room and below_m >= Settings.root_cellar_m:
		r = Survival.spoil_rate(Settings.cellar_c) * Settings.root_cellar_mult   # a root cellar
	elif below_m >= 3.0:
		r = Survival.spoil_rate(Settings.cellar_c)
	else:
		r = Survival.spoil_rate(outdoor_c)
	return r * (0.5 if kind == "storage_pot" else 1.0)


static func is_root_cellar(below_m: float, room: bool) -> bool:
	return room and below_m >= Settings.root_cellar_m


# ---------------- frost over the farm ----------------

## Kills every tracked crop the cold reaches. Returns the positions killed.
func check_frost(now_days: float) -> Array:
	var out := []
	if farming == null or not temp_at.is_valid():
		return out
	stats["frost_checks"] += 1
	var temps := {}   # 4 m column cell -> C (the crops of a field share it)
	var probes := PROBES_PER_CHECK
	for p: Vector3i in farming.crops.keys():
		var c: Dictionary = farming.crops[p]
		var def: Dictionary = farming.crops_def.get(c["kind"], {})
		var key := Vector2i(p.x >> 5, p.z >> 5)
		if not temps.has(key):
			temps[key] = float(temp_at.call(p))
		var t: float = temps[key]
		if not kills(def, t):
			if t < 0.0:
				stats["hardy"] += 1
			continue
		if sheltered.is_valid():
			var b := Vector3i(p.x >> 2, p.y >> 2, p.z >> 2)
			var hit: Array = _shelter.get(b, [])
			if hit.is_empty() or now_days > float(hit[1]):
				if probes <= 0:
					continue   # look again next check
				probes -= 1
				hit = [bool(sheltered.call(p)), now_days + SHELTER_DAYS]
				_shelter[b] = hit
			if hit[0]:
				stats["sheltered"] += 1
				continue
		kill(p)
		out.append(p)
	if _shelter.size() > 4096:
		_shelter.clear()
	return out


## The crop at p dies: it leaves a dead plant and is no longer tracked (nothing drops).
func kill(p: Vector3i) -> void:
	var c: Dictionary = farming.crops[p]
	farming.crops.erase(p)
	farming.dirty = true
	stats["killed"] += 1
	if farming.get_voxel.call(p) == farming.look_of(c["kind"], c["age"]):
		farming.set_voxel.call(p, GameMaterials.id_of(DEAD_CROP))
