class_name Waypoints
extends RefCounted

## Waypoints (plan item D10): named, coloured spots the maps pin and the world marks with a beam
## (WaypointMarkers). Saved with the player (Main._player_dict "waypoints"). Other systems add their
## own: a grave (kind "grave"), prospecting finds (part G) - main.waypoints.add(name, pos, kind).
## Pure data, no nodes.

signal changed

## The colours a new waypoint cycles through (pins, beams).
const COLOURS: Array[Color] = [Color("#e8443a"), Color("#3a9be8"), Color("#f2c230"), Color("#48c25a"),
	Color("#b45ce0"), Color("#ff8a2a"), Color("#3ad6c8"), Color("#f0f0f0")]
## Kinds with a fixed colour.
const KIND_COLOURS := {"grave": Color("#9a9aa6"), "death": Color("#9a9aa6")}

var list: Array[Dictionary] = []   # {id, name, pos: Vector3i, colour: Color, kind: String}
var _next_id := 1


## Adds a waypoint and returns its id. A colour of alpha 0 picks the kind's or the next in COLOURS.
func add(p_name: String, pos: Vector3i, kind: String = "", colour := Color(0, 0, 0, 0)) -> int:
	if colour.a <= 0.0:
		colour = KIND_COLOURS.get(kind, COLOURS[(_next_id - 1) % COLOURS.size()])
	var w := {"id": _next_id, "name": p_name if p_name != "" else "Waypoint %d" % _next_id, "pos": pos, "colour": colour, "kind": kind}
	_next_id += 1
	list.append(w)
	changed.emit()
	return int(w["id"])


func remove(id: int) -> bool:
	for i in list.size():
		if int(list[i]["id"]) == id:
			list.remove_at(i)
			changed.emit()
			return true
	return false


func rename(id: int, p_name: String) -> bool:
	var w := get_waypoint(id)
	if w.is_empty() or p_name.strip_edges() == "":
		return false
	w["name"] = p_name.strip_edges()
	changed.emit()
	return true


## The next colour of COLOURS for waypoint id.
func cycle_colour(id: int) -> void:
	var w := get_waypoint(id)
	if w.is_empty():
		return
	var i := COLOURS.find(w["colour"])
	w["colour"] = COLOURS[(i + 1) % COLOURS.size()]
	changed.emit()


func get_waypoint(id: int) -> Dictionary:
	for w in list:
		if int(w["id"]) == id:
			return w
	return {}


## The waypoint nearest to world xz `at` within `max_dist` voxels, or {}.
func nearest(at: Vector2, max_dist: float) -> Dictionary:
	var best := {}
	var bd := max_dist
	for w in list:
		var p: Vector3i = w["pos"]
		var d := Vector2(p.x, p.z).distance_to(at)
		if d <= bd:
			bd = d
			best = w
	return best


func to_array() -> Array:
	var out := []
	for w in list:
		var p: Vector3i = w["pos"]
		out.append({"id": w["id"], "name": w["name"], "pos": [p.x, p.y, p.z], "colour": (w["colour"] as Color).to_html(false), "kind": w["kind"]})
	return out


func load_array(a: Array) -> void:
	list.clear()
	_next_id = 1
	for v in a:
		if not (v is Dictionary) or not v.has("pos") or (v["pos"] as Array).size() != 3:
			continue
		var p: Array = v["pos"]
		var id := int(v.get("id", _next_id))
		list.append({"id": id, "name": String(v.get("name", "Waypoint")), "pos": Vector3i(int(p[0]), int(p[1]), int(p[2])),
			"colour": Color.from_string(String(v.get("colour", "ffffff")), Color.WHITE), "kind": String(v.get("kind", ""))})
		_next_id = maxi(_next_id, id + 1)
	changed.emit()
