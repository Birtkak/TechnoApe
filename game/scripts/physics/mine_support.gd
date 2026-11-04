class_name MineSupport
extends Node

## ADR 0142, mines need shoring. A roof the player dug must lie within its rock's span of a wall, or under a
## prop (timber prop, stone pillar, iron beam: FurnitureEntity kinds with "prop"); the rule itself is native
## (RigidPieces.support_*, prototypes/p5-sim-spike/src/rigid_support.cpp) and runs on the sim's driver thread.
## This node orchestrates: digs mark their 0.5 m blocks and are scanned together every support_scan_s; a roof
## that nothing holds creaks, sheds dust and pebbles and bends its overloaded props for a warning time (long for a
## small overload, short for a big one), then breaks off as one rigid rock piece and hurts what is under it.
## A prop placed in time (any scan that finds the roof held) stops it. Only survival digs count unless
## Settings.cave_ins_all_modes; generated caves have no dug blocks, so they stand.

const PEBBLES_MAX := 6        # real gravel cells a warning drops before the fall
const PROBE_EVERY_S := 0.4    # the aimed-at roof is re-checked this often while a prop is held
const NO_PROBE := Vector3i(0, -1048576, 0)
const SAVE_VERSION := 1
const SHAKE_S := 4.0          # the camera shakes for the last seconds before a fall, within SHAKE_M
const SHAKE_M := 14.0

var main: Node
var rp: RigidPieces
var warnings := {}            # id -> {cells, center, ratio, count, mat, t0, t_fail, next_fx, pebbles, props: [anchor]}
var stats_d := {"scans": 0, "warnings": 0, "collapses": 0, "held": 0, "props_broken": 0, "dug_calls": 0, "dug_ms_max": 0.0}
var scan_ms := PackedFloat32Array()   # the native scan times (driver thread), last 200
var dig_ms := PackedFloat32Array()    # the main thread's part of a dig (marks), last 200
var _dirty_lo := Vector3i.ZERO
var _dirty_hi := Vector3i.ZERO
var _dirty := false
var _accum := 0.0
var _now := 0.0
var _next_id := 1
var _jobs := {}               # tag -> [prop anchors] of a queued scan
var _tag := 0
var _probe_cell := NO_PROBE
var _probe_t := 0.0
var probe := {}               # the last probe: {cell, found, wall, span, prop, dug, held}
var _reach := 32
var _pending_hurt: Array = [] # [time, cells AABB, damage]
var dirty := false            # dug marks or warnings changed since the last save
var crack_material: ShaderMaterial   # the template of each warning's cracks (MaterialWarmup draws it)
var _shake_cam: Camera3D
var _shake_end := 0.0         # a fall shakes the camera briefly too


func setup(p_main: Node) -> void:
	main = p_main
	rp = main.pieces.rp
	rp.support_setup(spans(main.mining))
	main.pieces.support = self
	var r := Settings.support_span_max_m
	for k in FurnitureEntity.KINDS:
		if FurnitureEntity.is_prop(k):
			r = maxf(r, FurnitureEntity.KINDS[k]["prop"]["radius_m"])
	_reach = int(ceil(Settings.m2v(r)))
	make_crack_material()


func make_crack_material() -> void:
	crack_material = ShaderMaterial.new()
	crack_material.shader = preload("res://assets/shaders/roof_cracks.gdshader")
	crack_material.set_shader_parameter("crack_map", ImageTexture.create_from_image(Player.crack_image()))


## Per material id: -1 open (air, liquids, plants, furniture, stations), 0 solid that holds and never fails (wood,
## what cannot be mined), else the span in voxels from its mining hardness.
static func spans(mining: MiningTable) -> PackedFloat32Array:
	var defs := GameMaterials.defs()
	var out := PackedFloat32Array()
	out.resize(defs.size() + 1)
	out[0] = -1.0
	for i in defs.size():
		var d: Dictionary = defs[i]
		var ct := String(d.get("cell_type", "solid"))
		var open: bool = not ct in ["solid", "powder"] or d.get("replaceable", false) or d.has("clutter") \
			or d.has("interact") or d.get("model", "") == "empty" or d.get("rigid", "") == "leaf" \
			or (d.has("micro") and not GameMaterials.SOLID_MICRO.has(d["micro"]))
		var s := -1.0
		if not open:
			var e: Dictionary = mining.entry(String(d["name"])) if mining else {}
			s = 0.0
			if d.get("rigid", "") == "" and not e.is_empty():
				s = Settings.m2v(span_m(float(e["hardness"])))
		out[i + 1] = s
	return out


static func span_m(hardness: float) -> float:
	return clampf(Settings.support_span_base_m + Settings.support_span_per_hardness_m * hardness, 0.5, Settings.support_span_max_m)


## Creak to fall for a roof `ratio` times its span from a support: support_warn_max_s just past it, min at twice.
static func warn_s(ratio: float) -> float:
	return lerpf(Settings.support_warn_max_s, Settings.support_warn_min_s, clampf(ratio - 1.0, 0.0, 1.0))


func active() -> bool:
	return Settings.cave_ins and (Settings.cave_ins_all_modes or GameMode.rule(main.player.game_mode, "cave_ins"))


# ---------------- digs and props ----------------

## Cells the player emptied (world voxels): their blocks count as dug, the roof over them is checked soon.
func dug(cells: Array) -> void:
	if not active() or cells.is_empty():
		return
	var t0 := Time.get_ticks_usec()
	var p := PackedInt32Array()
	p.resize(cells.size() * 3)
	var lo: Vector3i = cells[0]
	var hi: Vector3i = cells[0]
	for i in cells.size():
		var c: Vector3i = cells[i]
		p[i * 3] = c.x
		p[i * 3 + 1] = c.y
		p[i * 3 + 2] = c.z
		lo = lo.min(c)
		hi = hi.max(c)
	rp.support_mark(p)
	dirty = true
	_touch(lo, hi)
	_note_dig(t0)


## x, y, z triples (a box dig).
func dug_packed(cells: PackedInt32Array) -> void:
	if not active() or cells.size() < 3:
		return
	var t0 := Time.get_ticks_usec()
	rp.support_mark(cells)
	dirty = true
	var a := Vector3i(cells[0], cells[1], cells[2])
	var b := Vector3i(cells[cells.size() - 3], cells[cells.size() - 2], cells[cells.size() - 1])   # a box dig runs corner to corner
	_touch(a.min(b), a.max(b))
	_note_dig(t0)


## A sphere emptied (the laser, the crater rod, TNT): its bounding box of blocks.
func dug_sphere(center: Vector3, r: float) -> void:
	if not active():
		return
	var t0 := Time.get_ticks_usec()
	var lo := Vector3i((center - Vector3.ONE * r).floor())
	var hi := Vector3i((center + Vector3.ONE * r).floor())
	rp.support_mark_box(lo, hi)
	dirty = true
	_touch(lo, hi)
	_note_dig(t0)


## A prop was placed or removed at `anchor`: the roof around it is checked again.
func prop_changed(anchor: Vector3i) -> void:
	if rp != null:
		_touch(anchor, anchor + Vector3i(Settings.BLOCK, Settings.BLOCK * 6, Settings.BLOCK))


func _touch(lo: Vector3i, hi: Vector3i) -> void:
	if _dirty:
		_dirty_lo = _dirty_lo.min(lo)
		_dirty_hi = _dirty_hi.max(hi)
	else:
		_dirty_lo = lo
		_dirty_hi = hi
		_dirty = true


func _note_dig(t0: int) -> void:
	var ms := (Time.get_ticks_usec() - t0) / 1000.0
	stats_d["dug_calls"] += 1
	stats_d["dug_ms_max"] = maxf(stats_d["dug_ms_max"], ms)
	dig_ms.append(ms)
	if dig_ms.size() > 200:
		dig_ms = dig_ms.slice(-200)


# ---------------- scans ----------------

func _process(delta: float) -> void:
	var t0 := Time.get_ticks_usec()
	_now += delta
	_accum += delta
	if _dirty and _accum >= Settings.support_scan_s:
		_accum = 0.0
		_queue_scan(_dirty_lo, _dirty_hi, NO_PROBE)
		_dirty = false
	for id in warnings.keys():
		_warn_step(id, warnings[id])
	_shake()
	for i in range(_pending_hurt.size() - 1, -1, -1):
		if _now >= _pending_hurt[i][0]:
			_hurt(_pending_hurt[i][1], _pending_hurt[i][2])
			_pending_hurt.remove_at(i)
	DebugCapture.note_proc("support", t0)


func _queue_scan(lo: Vector3i, hi: Vector3i, probe_cell: Vector3i) -> void:
	_tag += 1
	var anchors := []
	var props := PackedInt32Array()
	var c := (Vector3(lo + hi) * 0.5)
	var far := Settings.m2v(16.0) + Vector3(hi - lo).length() * 0.5
	for e in main.blocks.all_furniture():
		if not FurnitureEntity.is_prop(e.furniture) or Vector3(e.pos).distance_to(c) > far:
			continue
		var pd: Dictionary = FurnitureEntity.KINDS[e.furniture]["prop"]
		var h: int = e.height if e.height > 0 else FurnitureEntity.KINDS[e.furniture]["size"].y * Settings.BLOCK
		var mid: Vector3i = e.pos + Vector3i(Settings.BLOCK / 2, h, Settings.BLOCK / 2)
		props.append_array([mid.x, mid.y, mid.z, int(Settings.m2v(pd["radius_m"])), int(pd["capacity_m2"] * Settings.m2v(1.0) * Settings.m2v(1.0))])
		anchors.append(e.pos)
	_jobs[_tag] = anchors
	var job := {"lo": lo, "hi": hi, "reach": _reach, "props": props, "tag": _tag, "check": check_cells()}
	if probe_cell != NO_PROBE:
		job["probe"] = probe_cell
	rp.queue_support_scan(job)


## One roof cell per creaking roof (its middle one): a scan says whether it still fails, holds now or was not reached.
func check_cells() -> PackedInt32Array:
	var out := PackedInt32Array()
	for wid in warnings:
		out.append_array(_sample(warnings[wid]))
	return out


func _sample(w: Dictionary) -> PackedInt32Array:
	var cells: PackedInt32Array = w["cells"]
	var i := (cells.size() / 3 / 2) * 3
	return cells.slice(i, i + 3)


## Native results, handed over by FallingPieces.tick (between sim ticks).
func take(results: Array) -> void:
	for r in results:
		if r.has("drop"):
			continue
		stats_d["scans"] += 1
		scan_ms.append(float(r.get("ms", 0.0)))
		if scan_ms.size() > 200:
			scan_ms = scan_ms.slice(-200)
		var anchors: Array = _jobs.get(int(r.get("tag", 0)), [])
		_jobs.erase(int(r.get("tag", 0)))
		if r.has("probe"):
			probe = r["probe"]
			probe["cell"] = _probe_cell
		if r.has("clusters"):
			_apply(r, anchors)


## A scan's verdict on its zone: new unsupported roofs start creaking, known ones are updated, those it found
## held are saved.
func _apply(r: Dictionary, anchors: Array) -> void:
	var failed := []
	var props: Array = r.get("props", [])
	for i in mini(props.size(), anchors.size()):
		if props[i]["failed"]:
			failed.append(anchors[i])
	var seen := {}
	for cl in r["clusters"]:
		var center: Vector3 = cl["center"]
		var id := 0
		for wid in warnings:
			if warnings[wid]["center"].distance_to(center) < Settings.m2v(2.0):
				id = wid
				break
		var ratio := float(cl["ratio"])
		var cells: PackedInt32Array = cl["cells"]
		var w: Dictionary
		if id == 0:
			id = _next_id
			_next_id += 1
			w = {"t0": _now, "t_fail": _now + warn_s(ratio), "next_fx": _now, "pebbles": 0}
			warnings[id] = w
			stats_d["warnings"] += 1
			dirty = true
		else:
			w = warnings[id]
			w["t_fail"] = maxf(minf(w["t_fail"], w["t0"] + warn_s(ratio)), _now + 1.0)
		w["cells"] = cells
		w["center"] = center
		w["ratio"] = ratio
		w["count"] = int(cl["count"])
		w["mat"] = GameMaterials.material_of_voxel(main.get_world_voxel(Vector3i(cells[0], cells[1], cells[2])))
		w["props"] = failed.filter(func(a: Vector3i) -> bool: return Vector3(a).distance_to(center) < Settings.m2v(6.0))
		_crack_view(w)
		seen[id] = true
	var checked: PackedInt32Array = r.get("checked", PackedInt32Array())
	for wid in warnings.keys():
		if seen.has(wid):
			continue
		var at := _sample(warnings[wid])
		for k in range(0, checked.size(), 4):
			if checked[k + 3] == 0 and checked[k] == at[0] and checked[k + 1] == at[1] and checked[k + 2] == at[2]:
				_end_warning(wid)   # propped in time
				stats_d["held"] += 1
				break


# ---------------- warning and fall ----------------

func _warn_step(id: int, w: Dictionary) -> void:
	var span := maxf(w["t_fail"] - w["t0"], 0.1)
	var k := clampf((_now - w["t0"]) / span, 0.0, 1.0)
	if _now >= w["t_fail"]:
		_collapse(id, w)
		return
	for a in w["props"]:   # an overloaded prop bends more and more
		var v: Node3D = main._furniture_views.get(a)
		if v:
			v.rotation.z = sin(_now * 23.0) * 0.03 * k
	var v: MeshInstance3D = w.get("view")
	if v:
		(v.material_override as ShaderMaterial).set_shader_parameter("progress", lerpf(0.15, 1.0, k))
	if _now < w["next_fx"]:
		return
	w["next_fx"] = _now + lerpf(3.0, 0.3, k) * randf_range(0.7, 1.3)
	var cells: PackedInt32Array = w["cells"]
	var i := randi() % (cells.size() / 3) * 3
	var cell := Vector3i(cells[i], cells[i + 1], cells[i + 2])
	# louder and lower as the fall nears, a rumble under it for the last quarter
	Sound.play("tree/creak", Vector3(cell), {"pitch": 0.5 + 0.25 * k, "db": lerpf(0.0, 12.0, k)})
	if k > 0.75:
		Sound.play("tree/thud", Vector3(cell), {"pitch": 0.5, "db": lerpf(-4.0, 6.0, (k - 0.75) * 4.0)})
	if main.fx:
		for n in 1 + int(k * 3.0):   # more puffs toward the end, each off another roof cell
			var j := i if n == 0 else randi() % (cells.size() / 3) * 3
			var at := Vector3(cells[j], cells[j + 1], cells[j + 2]) + Vector3(0.5, -0.5, 0.5)
			main.fx.burst("roof_dust", at, {"material": w["mat"], "count_bonus": int(k * 10.0)})
			main.fx.burst("roof_grit", at, {"material": w["mat"], "count_bonus": int(k * 14.0)})
	if k > 0.25 and w["pebbles"] < PEBBLES_MAX and main.get_world_voxel(cell + Vector3i.DOWN) == GameMaterials.AIR:
		main.set_world_voxel(cell + Vector3i.DOWN, GameMaterials.GRAVEL)   # a real pebble of powder falls
		w["pebbles"] += 1


func _collapse(id: int, w: Dictionary) -> void:
	_end_warning(id)
	_shake_end = _now + 0.8
	stats_d["collapses"] += 1
	for a in w["props"]:
		var e: FurnitureEntity = main.blocks.furniture_at(a)
		if e:
			main._remove_furniture(e, false)   # snapped
			if main.fx:
				main.fx.block_break(a, GameMaterials.WOOD)
			stats_d["props_broken"] += 1
	var thick := Settings.BLOCK * (2 if w["ratio"] > 1.5 else 1)
	rp.queue_support_drop(w["cells"], thick, w["center"] + Vector3.UP * 64.0)
	Sound.play("tree/crash", w["center"], {"db": 3.0})
	var cells: PackedInt32Array = w["cells"]
	var box := AABB(Vector3(cells[0], cells[1], cells[2]), Vector3.ONE)
	for i in range(0, cells.size(), 3):
		box = box.expand(Vector3(cells[i], cells[i + 1], cells[i + 2]) + Vector3.ONE)
	_pending_hurt.append([_now + 0.4, box, clampi(2 + w["count"] / 64, 4, 16) * (2 if thick > Settings.BLOCK else 1)])
	if main.chat and main.player.position.distance_to(w["center"]) < Settings.m2v(24.0):
		main.chat.add_message("The roof caved in!")


func _end_warning(id: int) -> void:
	var v: Node = warnings[id].get("view")
	if v:
		v.queue_free()
	warnings.erase(id)
	dirty = true


## Cracks under a creaking roof: one quad per 0.5 m block of its cells, just below the lowest roof cell there.
func _crack_view(w: Dictionary) -> void:
	if crack_material == null:
		return
	var b := Settings.BLOCK
	var low := {}   # block xz -> lowest roof y
	var cells: PackedInt32Array = w["cells"]
	for i in range(0, cells.size(), 3):
		var k := Vector2i(floori(cells[i] / float(b)), floori(cells[i + 2] / float(b)))
		low[k] = mini(int(low.get(k, cells[i + 1])), cells[i + 1])
	var st := SurfaceTool.new()
	st.begin(Mesh.PRIMITIVE_TRIANGLES)
	st.set_normal(Vector3.DOWN)
	for k in low:
		var y := float(low[k]) - 0.03
		var x0 := float(k.x * b)
		var z0 := float(k.y * b)
		for c in [[0, 0], [1, 0], [1, 1], [0, 0], [1, 1], [0, 1]]:
			st.set_uv(Vector2(c[0], c[1]))
			st.add_vertex(Vector3(x0 + c[0] * b, y, z0 + c[1] * b))
	st.generate_tangents()
	var v: MeshInstance3D = w.get("view")
	if v == null:
		v = MeshInstance3D.new()
		v.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
		v.material_override = crack_material.duplicate()
		add_child(v)
		w["view"] = v
	v.mesh = st.commit()


## The last SHAKE_S before a fall near the camera, and the fall itself, shake it (Settings.screen_shake).
func _shake() -> void:
	var cam := get_viewport().get_camera_3d()
	var a := 0.0
	if Settings.screen_shake and cam:
		for wid in warnings:
			var w: Dictionary = warnings[wid]
			var left: float = w["t_fail"] - _now
			if left < SHAKE_S and cam.global_position.distance_to(w["center"]) < Settings.m2v(SHAKE_M):
				a = maxf(a, 1.0 - left / SHAKE_S)
		if _now < _shake_end:
			a = 1.5
	if _shake_cam and is_instance_valid(_shake_cam) and (a == 0.0 or _shake_cam != cam):
		_shake_cam.h_offset = 0.0
		_shake_cam.v_offset = 0.0
		_shake_cam = null
	if a > 0.0:
		_shake_cam = cam
		var m := Settings.m2v(0.03) * a   # a few centimetres: felt, not sickening
		cam.h_offset = randf_range(-m, m)
		cam.v_offset = randf_range(-m, m)


# ---------------- save ----------------

## The dug marks (per 16-voxel chunk, a bit per 0.5 m block) and the creaking roofs with the time they have left,
## so an unpropped room does not outlive a save and reload.
func to_save() -> Dictionary:
	var ws := []
	for wid in warnings:
		var w: Dictionary = warnings[wid].duplicate()
		w.erase("view")
		w["t0"] -= _now
		w["t_fail"] -= _now
		ws.append(w)
	return {"version": SAVE_VERSION, "chunks": rp.support_dug_chunks(), "warnings": ws}


func load_save(d: Dictionary) -> bool:
	if int(d.get("version", 0)) != SAVE_VERSION:
		push_error("mine support: unsupported save version %s" % d.get("version"))
		return false
	rp.support_clear()
	rp.support_load_chunks(d.get("chunks", PackedInt32Array()))
	for id in warnings.keys():
		_end_warning(id)
	for w in d.get("warnings", []):
		w["t0"] += _now
		w["t_fail"] += _now
		w["next_fx"] = _now
		warnings[_next_id] = w
		_next_id += 1
		_crack_view(w)
	dirty = false
	return true


## Binary (store_var) and atomic: the chunk masks are 32-bit halves, which JSON would round.
func save_file(path: String) -> bool:
	DirAccess.make_dir_recursive_absolute(path.get_base_dir())
	var f := FileAccess.open(path + ".tmp", FileAccess.WRITE)
	if f == null:
		push_error("mine support: cannot write %s" % path)
		return false
	f.store_var(to_save())
	f.close()
	if DirAccess.rename_absolute(path + ".tmp", path) != OK:
		push_error("mine support: cannot rename %s.tmp" % path)
		return false
	dirty = false
	return true


func load_file(path: String) -> bool:
	if not FileAccess.file_exists(path):
		return false
	var f := FileAccess.open(path, FileAccess.READ)
	var d = f.get_var() if f else null
	return d is Dictionary and load_save(d)


## What stands under a fallen roof (within 4 m below it) is hurt.
func _hurt(box: AABB, dmg: int) -> void:
	var under := AABB(box.position - Vector3(2, Settings.m2v(4.0), 2), box.size + Vector3(4, Settings.m2v(4.0), 4))
	var p: Vector3 = main.player.position
	if under.has_point(p) and GameMode.rule(main.player.game_mode, "takes_damage"):
		main.player_inv.damage(dmg, "cave_in")
	if main.mobs:
		for m in main.mobs.active():
			if under.has_point(m.position):
				m.hurt(dmg, box.get_center())


# ---------------- the hand tool and debug ----------------

## The look-at line for a roof cell while a prop is held: how far it is from support and what its rock spans.
func hint(cell: Vector3i) -> String:
	if rp == null:
		return ""
	if cell != _probe_cell or _now - _probe_t > PROBE_EVERY_S:
		if cell != _probe_cell:
			probe = {}
		_probe_cell = cell
		_probe_t = _now
		_queue_scan(cell - Vector3i(0, 4, 0), cell, cell)
	for wid in warnings:
		var w: Dictionary = warnings[wid]
		if w["center"].distance_to(Vector3(cell)) < Settings.m2v(3.0):
			return "Roof creaking: falls in %d s" % maxi(int(w["t_fail"] - _now), 0)
	if probe.is_empty() or not probe.get("found", false):
		return "Roof: checking..." if probe.is_empty() else ""
	var span_m_ := Settings.v2m(float(probe["span"]))
	if not probe["dug"]:
		return "Roof: as the rock was made, it stands"
	if probe["span"] <= 0.0:
		return "Roof: holds (this does not cave in)"
	var wall: float = probe["wall"]
	var d := "far" if wall < 0.0 else "%.1f m" % Settings.v2m(wall)
	if probe["prop"] >= 0:
		return "Roof: %s from a wall, held by a prop (rock spans %.1f m)" % [d, span_m_]
	if probe["held"]:
		return "Roof: %s from a wall, rock spans %.1f m: holds" % [d, span_m_]
	return "Roof: %s from a wall, rock spans %.1f m: needs a prop" % [d, span_m_]


func median(a: PackedFloat32Array) -> float:
	if a.is_empty():
		return 0.0
	var s := a.duplicate()
	s.sort()
	return s[s.size() / 2]


func stats() -> Dictionary:
	var d := stats_d.duplicate()
	d["active"] = active() if main else false
	d["open_warnings"] = warnings.size()
	d["scan_ms_median"] = snappedf(median(scan_ms), 0.001)
	d["scan_ms_max"] = snappedf(Array(scan_ms).max() if not scan_ms.is_empty() else 0.0, 0.001)
	d["dig_ms_median"] = snappedf(median(dig_ms), 0.001)
	d["native"] = rp.support_stats() if rp else {}
	var ws := []
	for wid in warnings:
		var w: Dictionary = warnings[wid]
		ws.append({"center_m": (w["center"] / Settings.m2v(1.0)).snapped(Vector3.ONE * 0.1), "cells": w["count"],
			"ratio": snappedf(w["ratio"], 0.01), "falls_in_s": snappedf(w["t_fail"] - _now, 0.1), "props": w["props"].size()})
	d["warnings_now"] = ws
	return d
