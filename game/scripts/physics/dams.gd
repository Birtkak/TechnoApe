class_name Dams
extends Node

## Weak dams leak, then break (ADR 0154). Every 0.5 m block the player builds into is remembered; once a
## second the native `SimWorld.dam_scan` looks at those blocks for soft cells (data/dams.json `hold`: dirt, sand,
## gravel, planks...) with water against one face and drier ground past the run of wall behind it. A cell's load is
## the head of water over it divided by what the run holds (each cell's `hold` summed over the thickness); stone
## anywhere in the run backs it. Load >= `seep_load` seeps (a cell of water now and then on the dry face); load >= 1
## creaks and seeps harder for a warning time, then the overloaded rows wash out through the whole thickness and the
## river pours through (native water; nothing here touches more than the breached cells). Counters: stone or
## brick, a thicker dam, a spillway that keeps the level (the head) down.

const MAX_BLOCKS := 512

var main: Node
var hold := PackedFloat32Array()
var seep_load := 0.5
var warn_max_s := 40.0
var warn_min_s := 8.0
var _blocks := {}             # Vector3i block -> true, insertion order = age
var _warn := {}               # Vector3i block -> seconds left before it breaks
var _t := 0.0
var _rng := RandomNumberGenerator.new()
var _seeps_left := 0          # seeps add water: a few a scan over every dam
var stats := {"blocks": 0, "stressed": 0, "warning": 0, "seeps": 0, "breaches": 0, "scan_ms": 0.0}


func setup(p_main: Node) -> void:
	main = p_main
	_rng.seed = 154
	var d: Dictionary = JSON.parse_string(FileAccess.get_file_as_string("res://data/dams.json"))
	hold.resize(256)
	for n: String in d["hold"]:
		var id := GameMaterials.id_of(n)
		if id > 0:
			hold[id] = float(d["hold"][n])
	seep_load = float(d.get("seep_load", seep_load))
	warn_max_s = float(d.get("warn_max_s", warn_max_s))
	warn_min_s = float(d.get("warn_min_s", warn_min_s))


## Main._on_placed / _on_placed_many: a built cell makes its block a dam candidate.
func placed(p: Vector3i) -> void:
	var b := Vector3i((Vector3(p) / Settings.BLOCK).floor())
	if _blocks.has(b):
		return
	if _blocks.size() >= MAX_BLOCKS:
		_blocks.erase(_blocks.keys()[0])
	_blocks[b] = true


func _process(delta: float) -> void:
	_t += delta
	for b in _warn.keys():
		_warn[b] -= delta
	if _t < 1.0 or _blocks.is_empty() or main.bubble == null:
		return
	_t = 0.0
	scan()


## One scan over every remembered block; returns the stressed cells by block (for tests and /dam).
func scan() -> Dictionary:
	var B := Settings.BLOCK
	var boxes := PackedInt32Array()
	for b: Vector3i in _blocks:
		boxes.append_array([b.x * B, b.y * B, b.z * B, b.x * B + B, b.y * B + B, b.z * B + B])
	var t0 := Time.get_ticks_usec()
	var r: PackedInt32Array = main.bubble.dam_scan(boxes, hold, seep_load)
	stats["scan_ms"] = (Time.get_ticks_usec() - t0) / 1000.0
	var by := {}
	for i in range(0, r.size(), 7):
		var p := Vector3i(r[i], r[i + 1], r[i + 2])
		var b := Vector3i((Vector3(p) / B).floor())
		if not by.has(b):
			by[b] = []
		by[b].append([p, r[i + 3] / 1000.0, Vector3i(r[i + 4], r[i + 5], r[i + 6])])
	stats["blocks"] = _blocks.size()
	_seeps_left = 3
	stats["stressed"] = by.size()
	for b in _warn.keys():
		if not by.has(b):
			_warn.erase(b)   # it holds now (the water fell, someone thickened it)
	for b: Vector3i in by:
		_judge(b, by[b])
	stats["warning"] = _warn.size()
	return by


func _judge(b: Vector3i, cells: Array) -> void:
	var worst := 0.0
	for c in cells:
		worst = maxf(worst, c[1])
	if worst < 1.0:
		if _rng.randf() < 0.2:
			_seep(cells[_rng.randi() % cells.size()])   # marginal: a slow weep, it holds
		return
	if not _warn.has(b):
		_warn[b] = lerpf(warn_max_s, warn_min_s, clampf((worst - 1.0) / 2.0, 0.0, 1.0))
	var left: float = _warn[b]
	var k := 1.0 - clampf(left / warn_max_s, 0.0, 1.0)
	if _rng.randf() < 0.3 + 0.7 * k:
		_seep(cells[_rng.randi() % cells.size()])
	Sound.play("tree/creak", Vector3(b * Settings.BLOCK) + Vector3.ONE * 2.0, {"pitch": 0.45 + 0.2 * k, "db": lerpf(-4.0, 8.0, k)})
	if left <= 0.0:
		_breach(cells)
		_warn.erase(b)


## The warning sign: a cell of water on the dry face, with a drip.
func _seep(c: Array) -> void:
	if _seeps_left <= 0:
		return
	_seeps_left -= 1
	var ex: Vector3i = c[2]
	if main.bubble.get_voxel(ex) == GameMaterials.AIR:
		main.bubble.set_voxel(ex, GameMaterials.WATER)
	stats["seeps"] += 1
	if main.fx:
		main.fx.burst("drip", Vector3(ex) + Vector3(0.5, 0.8, 0.5), {"material": GameMaterials.WATER})


## The overloaded rows wash out from the wet face to the dry one.
func _breach(cells: Array) -> void:
	for c in cells:
		if c[1] < 1.0:
			continue
		var p: Vector3i = c[0]
		var ex: Vector3i = c[2]
		var d := (ex - p).sign()
		while p != ex:
			main.bubble.set_voxel(p, GameMaterials.AIR)
			p += d
	stats["breaches"] += 1
	var at: Vector3 = Vector3(cells[0][0])
	if main.fx:
		main.fx.burst("splash", at, {"material": GameMaterials.WATER, "count_bonus": 20})
	Sound.play("tree/creak", at, {"pitch": 0.35, "db": 12.0})
	if main.get("chat") and main.player and at.distance_to(main.player.position) < Settings.m2v(48.0):
		main.chat.add_message("The dam gave way!")
