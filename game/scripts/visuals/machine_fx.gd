class_name MachineFx
extends Node3D

## Running machines read as running (ADR 0153): steam puffs, chimney smoke, sparks on a blow, spinning
## rollers and fans, gauge needles, a trembling sieve and a run lamp, all from data/machine_fx.json per
## block entity kind. Idle machines stay still; a starved one (it wants to work and has no power or steam)
## blinks a red lamp. Pure presentation: it reads entities and never changes them. Machines within
## Settings.machine_fx_m of the camera are found twice a second; only those are evaluated and drawn.
## Particles go through ParticleFx's two pools (no new pool); the parts share one material with
## per-instance colour, compiled under the loading screen (warm_materials). A new machine kind is a JSON
## entry, no code: see the file's _doc.

const PATH := "res://data/machine_fx.json"
const SCAN_S := 0.5
const VIEW_M := 96.0          # WorkPalette draws a piece's view this far: a machine's carved parts must be there too
const CANDIDATES_S := 5.0
const EVAL_HZ := 10.0
const BUILD_US := 2000         # part meshes built a frame, at least one
const BLINK_HZ := 1.5
const RUN_COLOR := Color("40e060")
const STARVED_COLOR := Color("ff3020")
const OFF_COLOR := Color("2a2a30")

static var _data := {}
static var _exprs := {}       # expression text -> Expression (null when it does not parse)
static var errors: Array[String] = []
static var _material: ShaderMaterial
static var _meshes := {}      # Vector3 size -> BoxMesh
static var _failed := {}      # expression texts that failed once

var blocks: BlockEntities
var fx: ParticleFx            # null: no particles (particles=0), the rest still runs
var work: WorkSystem          # trembling views; null in tests
var force := false            # /machinefx force: every machine near counts as working (the effects' cost bench)
var stats := {"near": 0, "working": 0, "starved": 0, "parts": 0, "bursts": 0, "ms_max": 0.0, "ms_avg": 0.0}
var _recs := {}               # Vector3i -> record (see _record)
var _scan_t := SCAN_S
var _eval_t := 0.0
var _time := 0.0
var _rng := RandomNumberGenerator.new()
var _ms_sum := 0.0
var _ms_n := 0
var _cand: Array = []
var _cand_size := -1
var _cand_t := 0.0
var _build_until := 0         # usec: parts are built until then each frame (30 machines at once cost 23 ms in one scan)
var _built := 0


# ---------------- data ----------------

static func data() -> Dictionary:
	if _data.is_empty():
		var f := FileAccess.open(PATH, FileAccess.READ)
		var d = JSON.parse_string(f.get_as_text()) if f else null
		_data = d if d is Dictionary else {"kinds": {}}
		if not (d is Dictionary):
			errors.append("machine_fx.json does not parse")
	return _data


static func spec(kind: String) -> Dictionary:
	return data()["kinds"].get(kind, {})


## True for kinds with an entry: ParticleFx leaves their smoke and TorchLights their light to this data.
static func handles(kind: String) -> bool:
	return data()["kinds"].has(kind)


## "fire", "lamp" or "none": the light TorchLights gives a lit entity of this kind (fire with no entry).
static func light_of(kind: String) -> String:
	return String(spec(kind).get("light", "fire"))


static func eval(text: String, e: Object) -> Variant:
	if text == "":
		return null
	if not _exprs.has(text):
		var x := Expression.new()
		if x.parse(text) != OK:
			errors.append("machine_fx: %s: %s" % [text, x.get_error_text()])
			x = null
		_exprs[text] = x
	var ex: Expression = _exprs[text]
	if ex == null:
		return null
	var v = ex.execute([], e, false)
	if ex.has_execute_failed():
		if not _failed.has(text):   # once per text: a typo in the data shows in the test, not as log spam
			_failed[text] = true
			errors.append("machine_fx: %s fails on %s" % [text, e.get("kind")])
		return null
	return v


## Whether `e` works now by its kind's `when` (is_lit() without one). GameAudio's machine loops ask this too.
static func working(e: Object) -> bool:
	var s := spec(String(e.get("kind")))
	return _truth(eval(String(s.get("when", "is_lit()")), e)) if not s.is_empty() else (e.has_method("is_lit") and e.is_lit())


## A failed or non-bool expression counts as false.
static func _truth(v: Variant) -> bool:
	return typeof(v) == TYPE_BOOL and v


static func starved(e: Object) -> bool:
	var s := spec(String(e.get("kind")))
	return _truth(eval(String(s.get("starved", "")), e)) if s.has("starved") else false


## The one material every part and lamp draws with: colour and glow per instance, so one pipeline.
static func part_material() -> ShaderMaterial:
	if _material == null:
		var sh := Shader.new()
		sh.code = """shader_type spatial;
instance uniform vec4 tint : source_color = vec4(1.0);
instance uniform float glow = 0.0;
void fragment() {
	ALBEDO = tint.rgb;
	ROUGHNESS = 0.75;
	METALLIC = 0.3;
	EMISSION = tint.rgb * glow;
}"""
		_material = ShaderMaterial.new()
		_material.shader = sh
	return _material


func warm_materials() -> Array:
	return [part_material()]


static func _mesh(size_v: Vector3) -> BoxMesh:
	if not _meshes.has(size_v):
		var m := BoxMesh.new()
		m.size = size_v
		m.material = part_material()
		_meshes[size_v] = m
	return _meshes[size_v]


# ---------------- running ----------------

func setup(p_blocks: BlockEntities, p_fx: ParticleFx, p_work: WorkSystem) -> void:
	blocks = p_blocks
	fx = p_fx
	work = p_work
	_rng.seed = 153
	data()


func _process(delta: float) -> void:
	var cam := get_viewport().get_camera_3d()
	if cam == null or blocks == null:
		return
	var t0 := Time.get_ticks_usec()
	step(delta, cam.global_position)
	var ms := (Time.get_ticks_usec() - t0) / 1000.0
	stats["ms_max"] = maxf(stats["ms_max"], ms)
	_ms_sum += ms
	_ms_n += 1
	stats["ms_avg"] = snappedf(_ms_sum / _ms_n, 0.001)
	DebugCapture.note_proc("machine_fx", t0)


## One frame seen from `eye` (world voxels). Tests call it directly.
func step(delta: float, eye: Vector3) -> void:
	_time += delta
	_scan_t += delta
	if _scan_t >= SCAN_S:
		_scan_t = 0.0
		var t0 := Time.get_ticks_usec()
		_scan(eye)
		stats["scan_ms_max"] = maxf(float(stats.get("scan_ms_max", 0.0)), (Time.get_ticks_usec() - t0) / 1000.0)
	_eval_t += delta
	var eval_now := _eval_t >= 1.0 / EVAL_HZ
	if eval_now:
		_eval_t = 0.0
	var n_work := 0
	var n_starved := 0
	_build_until = Time.get_ticks_usec() + BUILD_US
	_built = 0
	for p in _recs.keys():
		var r: Dictionary = _recs[p]
		var e: Object = r["e"]
		if blocks.at(p) != e:
			_drop(p)
			continue
		if r["parts"].size() > 0:
			_refresh_parts(r)
		if not r["near"]:
			continue   # beyond machine_fx_m: drawn (its carved parts), still, not evaluated
		if eval_now:
			r["working"] = force or working(e)
			r["starved"] = not r["working"] and starved(e)
			for part in r["parts"]:
				if part["def"].get("motion", "") == "needle":
					var v = eval(String(part["def"].get("value", "0")), e)
					part["target"] = float(v) if v != null else 0.0
		n_work += int(r["working"])
		n_starved += int(r["starved"])
		_animate(r, delta)
	stats["working"] = n_work
	stats["starved"] = n_starved


## Entities whose kind has something to draw. A base of thousands of cables and torches is walked only when
## the entity count changes or every CANDIDATES_S, not twice a second.
func _candidates() -> Array:
	if blocks.size() != _cand_size or _time - _cand_t > CANDIDATES_S:
		_cand_size = blocks.size()
		_cand_t = _time
		_cand = []
		for e in blocks.all_entities():
			var kind = e.get("kind")
			if kind == null or not handles(kind):
				continue
			var s := spec(kind)
			if s.has("emit") or s.has("parts") or s.has("pulse") or s.has("lamp") or s.has("shake_m"):
				_cand.append(e)   # not an entry only for the light (belts, rails, the lamp)
	return _cand


func _scan(eye: Vector3) -> void:
	var r2 := pow(Settings.m2v(Settings.machine_fx_m), 2)
	var v2 := pow(Settings.m2v(maxf(VIEW_M, Settings.machine_fx_m)), 2)
	var seen := {}
	for e in _candidates():
		var s := spec(e.kind)
		var d2 := _centre(e).distance_squared_to(eye)
		if blocks.at(e.pos) != e or d2 > (v2 if s.has("parts") else r2):
			continue
		seen[e.pos] = true
		if not _recs.has(e.pos) or _recs[e.pos]["e"] != e:
			if _recs.has(e.pos):
				_drop(e.pos)
			_recs[e.pos] = _record(e, s)
		var r: Dictionary = _recs[e.pos]
		if r["near"] and d2 > r2:
			r["working"] = false   # left the radius: stop where it is
			r["starved"] = false
			_still(r)
			_animate(r, 0.0)   # the lamp goes dark
		r["near"] = d2 <= r2
	for p in _recs.keys():
		if not seen.has(p):
			_drop(p)
	var near := 0
	for p in _recs:
		near += int(_recs[p]["near"])
	stats["near"] = near
	var n := 0
	for p in _recs:
		n += _recs[p]["parts"].size()
	stats["parts"] = n


static func _box(e: Object) -> Vector3:
	var w := BlockEntities.width_of(e.kind)
	return Vector3(w, BlockEntities.height_of(e.kind), w)


func _centre(e: Object) -> Vector3:
	var b := _box(e)
	return Vector3(e.pos) + Vector3(b.x * 0.5, b.y * 0.5, b.z * 0.5)


## A rig at the footprint's floor centre, turned with the entity (WorkPalette.rotate_y: a quarter turn
## takes cell (x, z) to (n-1-z, x), a -90 degree turn about +y); `at` fractions are placed inside it.
func _record(e: Object, s: Dictionary) -> Dictionary:
	var b := _box(e)
	var rig := Node3D.new()
	rig.position = Vector3(e.pos) + Vector3(b.x * 0.5, 0.0, b.z * 0.5)
	rig.basis = Basis(Vector3.UP, -PI / 2.0 * posmod(int(e.get("facing") if e.get("facing") != null else 0), 4))
	add_child(rig)
	var r := {"e": e, "s": s, "rig": rig, "box": b, "parts": [], "lamp": null, "working": false, "starved": false,
		"acc": [], "prev": [], "near": false, "view": null}
	for em in s.get("emit", []):
		r["acc"].append(_rng.randf())   # staggered: thirty machines do not puff in step
	for pu in s.get("pulse", []):
		r["prev"].append(0.0)
	for pd in s.get("parts", []):
		var pivot := Node3D.new()
		rig.add_child(pivot)
		r["parts"].append({"def": pd, "pivot": pivot, "base": Vector3.ZERO, "angle": 0.0, "value": 0.0, "target": 0.0})
	if not r["parts"].is_empty() and work == null:
		_build_parts(r)   # in game _refresh_parts builds them, a few a frame
	if s.has("lamp"):
		var d: Dictionary = data().get("defaults", {})
		var ls := float(s["lamp"].get("size", d.get("lamp_size", 1.5)))
		var lamp := MeshInstance3D.new()
		lamp.mesh = _mesh(Vector3.ONE * ls)
		lamp.position = _local(r, s["lamp"].get("at", d.get("lamp_at", [0.85, 1.0, 0.15]))) + Vector3(0, ls * 0.5, 0)
		lamp.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
		lamp.set_instance_shader_parameter("tint", OFF_COLOR)
		rig.add_child(lamp)
		r["lamp"] = lamp
	return r


# ---------------- parts: the model's own cells ----------------

static func _paint(name: String) -> int:
	return int((WorkPalette as Script).get_script_constant_map().get(name, WorkPalette.IRON_DARK))


static func _v3(a: Array) -> Vector3:
	return Vector3(float(a[0]), float(a[1]), float(a[2]))


static func _facing(e: Object) -> int:
	return posmod(int(e.get("facing") if e.get("facing") != null else 0), 4)


## The view's solid part as drawn (art models applied), turned back to facing 0: {cells, size, grid, offset}.
static func model_of(e: Object) -> Dictionary:
	var s: Dictionary = WorkPalette.art_parts(e, e.view_parts()).get("solid", {})
	if s.is_empty():
		return {}
	s = s.duplicate()
	s["cells"] = WorkPalette.rotate_y(s["cells"], s["size"], -_facing(e))
	s["grid"] = int(s.get("grid", GameMaterials.MICRO_GRID))
	s["offset"] = s.get("offset", Vector3.ZERO)
	return s


## A kind's parts take their `cut` boxes (model cells, facing 0) out of its view: WorkSystem draws what is left,
## MachineFx moves the rest. Returns `parts` with the solid cells cleared there.
static func carve(e: Object, parts: Dictionary) -> Dictionary:
	var cuts := []
	for pd in spec(String(e.get("kind"))).get("parts", []):
		if pd.has("cut"):
			cuts.append(pd["cut"])
	if cuts.is_empty() or not parts.has("solid"):
		return parts
	var s: Dictionary = parts["solid"].duplicate()
	var size: Vector3i = s["size"]
	var q := _facing(e)
	var c: PackedInt32Array = WorkPalette.rotate_y(s["cells"], size, -q).duplicate()
	for ab in cuts:
		WorkPalette.box(c, size, Vector3i(_v3(ab[0])), Vector3i(_v3(ab[1])), -1)
	s["cells"] = WorkPalette.rotate_y(c, size, q)
	parts = parts.duplicate()
	parts["solid"] = s
	return parts


## A part's cells in the model's grid: drawn `boxes` [a, b, paint], `drums` along x {x: [x0, x1], cy, cz, r, paint,
## rivets: [x...]} and a `disc` across z {z, cx, cy, r, paint, teeth}; with none of those, the model's own cells in its
## `cut` box. Returns {cells, size, origin} (origin in model cells) or {} when empty.
static func part_cells(pd: Dictionary, model: Dictionary) -> Dictionary:
	var lo := Vector3(INF, INF, INF)
	var hi := -lo
	var bounds: Array = []
	for bx in pd.get("boxes", []):
		bounds.append([_v3(bx[0]), _v3(bx[1])])
	for d in pd.get("drums", []):
		bounds.append([Vector3(d["x"][0], d["cy"] - d["r"], d["cz"] - d["r"]), Vector3(d["x"][1], d["cy"] + d["r"], d["cz"] + d["r"])])
	if pd.has("disc"):
		var k: Dictionary = pd["disc"]
		bounds.append([Vector3(k["cx"] - k["r"], k["cy"] - k["r"], k["z"]), Vector3(k["cx"] + k["r"], k["cy"] + k["r"], k["z"] + 1)])
	var from_model := bounds.is_empty()
	if from_model:
		if not pd.has("cut") or model.is_empty():
			return {}
		bounds.append([_v3(pd["cut"][0]), _v3(pd["cut"][1])])
	for b in bounds:
		lo = lo.min(b[0])
		hi = hi.max(b[1])
	var o := Vector3i(lo.floor())
	var size := Vector3i(hi.ceil()) - o
	var c := WorkPalette.new_cells(size)
	if from_model:
		var ms: Vector3i = model["size"]
		var any := false
		for z in size.z:
			for y in size.y:
				for x in size.x:
					var m := o + Vector3i(x, y, z)
					if m.x >= 0 and m.y >= 0 and m.z >= 0 and m.x < ms.x and m.y < ms.y and m.z < ms.z:
						var v: int = model["cells"][WorkPalette.idx(ms, m.x, m.y, m.z)]
						c[WorkPalette.idx(size, x, y, z)] = v
						any = any or v > 0
		if not any:
			return {}
	for bx in pd.get("boxes", []):
		WorkPalette.box(c, size, Vector3i(_v3(bx[0])) - o, Vector3i(_v3(bx[1])) - o, _paint(String(bx[2])))
	for d in pd.get("drums", []):
		var cy: float = d["cy"] - o.y
		var cz: float = d["cz"] - o.z
		WorkPalette.drum(c, size, int(d["x"][0]) - o.x, int(d["x"][1]) - o.x, cy, cz, d["r"], _paint(String(d["paint"])))
		for rx in d.get("rivets", []):
			WorkPalette.drum_rivets(c, size, int(rx) - o.x, cy, cz, d["r"])
	if pd.has("disc"):
		var k: Dictionary = pd["disc"]
		var tooth := _paint(String(k.get("teeth", "RIVET")))
		for y in size.y:
			for x in size.x:
				var dv := Vector2(x + o.x + 0.5 - float(k["cx"]), y + o.y + 0.5 - float(k["cy"]))
				var l := dv.length()
				if l <= float(k["r"]):
					var rim := l > float(k["r"]) - 1.0 and posmod(int(round(dv.angle() / TAU * 12.0)), 2) == 0
					c[WorkPalette.idx(size, x, y, int(k["z"]) - o.z)] = (tooth if rim else _paint(String(k["paint"]))) + 1
	return {"cells": c, "size": size, "origin": Vector3(o)}


## Model cells (facing 0) to rig-local voxels: a view's cell c sits at (c / grid + offset) widths from its anchor and
## turns about the footprint centre, where the rig stands.
func _cell_to_rig(r: Dictionary, c: Vector3) -> Vector3:
	var m: Dictionary = r["model"]
	var w: float = r["box"].x
	return (c / float(m["grid"]) + Vector3(m["offset"]) - Vector3(0.5, 0.0, 0.5)) * w


## (Re)builds every part's mesh from the model as drawn now: on creation and whenever WorkSystem rebuilt the view.
func _build_parts(r: Dictionary) -> void:
	r["model"] = model_of(r["e"])
	if r["model"].is_empty():
		return
	var w: float = r["box"].x
	var grid := float(r["model"]["grid"])
	for part in r["parts"]:
		var pd: Dictionary = part["def"]
		var pivot: Node3D = part["pivot"]
		for ch in pivot.get_children():
			pivot.remove_child(ch)
			ch.queue_free()
		var pc := part_cells(pd, r["model"])
		if pc.is_empty():
			continue
		var at: Vector3 = _v3(pd["pivot"]) if pd.has("pivot") else pc["origin"] + Vector3(pc["size"]) * 0.5
		part["base"] = _cell_to_rig(r, at)
		if pd.get("motion", "") != "bob" or part["angle"] == 0.0:
			pivot.position = part["base"]
		var mi := MeshInstance3D.new()
		mi.mesh = WorkPalette.mesh(pc["cells"], pc["size"], int(grid))
		mi.material_override = WorkPalette.material()
		mi.visibility_range_end = VIEW_M
		mi.scale = Vector3.ONE * w
		mi.position = (pc["origin"] - at) / grid * w
		pivot.add_child(mi)


func _refresh_parts(r: Dictionary) -> void:
	if work == null:
		return
	var v := work.view_at(r["e"].pos)
	if v != r["view"] and (_built == 0 or Time.get_ticks_usec() < _build_until):
		r["view"] = v
		_build_parts(r)
		_built += 1


func _local(r: Dictionary, at: Array) -> Vector3:
	var b: Vector3 = r["box"]
	return Vector3((float(at[0]) - 0.5) * b.x, float(at[1]) * b.y, (float(at[2]) - 0.5) * b.z)


func _drop(p: Vector3i) -> void:
	var r: Dictionary = _recs[p]
	_still(r)
	(r["rig"] as Node).queue_free()
	_recs.erase(p)


func _animate(r: Dictionary, delta: float) -> void:
	var s: Dictionary = r["s"]
	var on: bool = r["working"]
	var rig: Node3D = r["rig"]
	if on:
		var ems: Array = s.get("emit", [])
		for i in ems.size():
			var em: Dictionary = ems[i]
			r["acc"][i] += float(em.get("per_s", 1.0)) * delta
			if r["acc"][i] >= 1.0:
				r["acc"][i] = fmod(r["acc"][i], 1.0)
				if em.has("if") and not _truth(eval(String(em["if"]), r["e"])):
					continue
				_burst(String(em["effect"]), rig.transform * _local(r, em.get("at", [0.5, 1.0, 0.5])))
	var pulses: Array = s.get("pulse", [])
	for i in pulses.size():   # every frame: a blow lasts a few ticks
		var v = eval(String(pulses[i].get("value", "0")), r["e"])
		var f := float(v) if v != null else 0.0
		if f > 0.0 and r["prev"][i] <= 0.0:
			_burst(String(pulses[i]["effect"]), rig.transform * _local(r, pulses[i].get("at", [0.5, 1.0, 0.5])))
		r["prev"][i] = f
	for part in r["parts"]:
		var d: Dictionary = part["def"]
		var pivot: Node3D = part["pivot"]
		var axis := _axis(String(d.get("axis", "y")))
		match String(d.get("motion", "")):
			"spin":
				if on:
					part["angle"] = fmod(part["angle"] + float(d.get("rpm", 60.0)) / 60.0 * TAU * delta, TAU)
					pivot.basis = Basis(axis, part["angle"])
			"bob":   # `amp` model cells along `axis` and back (negative: the other way), hz times a second
				if on:
					part["angle"] += float(d.get("hz", 1.0)) * TAU * delta
				var cell: float = r["box"].x / float(r["model"].get("grid", 8)) if r.has("model") else 1.0
				pivot.position = part["base"] + axis * float(d.get("amp", 1.0)) * cell * (0.5 - 0.5 * cos(part["angle"]))
			"swing":   # deg either side about `axis`, hz times a second
				if on:
					part["angle"] += float(d.get("hz", 0.5)) * TAU * delta
				pivot.basis = Basis(axis, deg_to_rad(float(d.get("deg", 30.0))) * sin(part["angle"]))
			"needle":
				var rg: Array = d.get("range", [0.0, 1.0])
				var sw: Array = d.get("sweep_deg", [-60, 60])
				part["value"] = lerpf(part["value"], part["target"], minf(1.0, delta * 4.0))
				var k := clampf(inverse_lerp(float(rg[0]), float(rg[1]), part["value"]), 0.0, 1.0)
				pivot.basis = Basis(axis, deg_to_rad(lerpf(float(sw[0]), float(sw[1]), k)))
	var lamp: MeshInstance3D = r["lamp"]
	if lamp != null:
		# never hidden: the first show of 15 lamps at once cost 26 ms; an unlit bulb is dark instead. Instance parameters
		# cross to the render thread, so they are set only on a change.
		var st := 1 if on else (2 if r["starved"] and fmod(_time * BLINK_HZ, 1.0) < 0.5 else 0)
		if lamp.get_meta("st", -1) != st:
			lamp.set_meta("st", st)
			lamp.set_instance_shader_parameter("tint", [OFF_COLOR, RUN_COLOR, STARVED_COLOR][st])
			lamp.set_instance_shader_parameter("glow", 0.0 if st == 0 else 1.5)
	if s.has("shake_m") and work != null:
		var view := work.view_at(r["e"].pos)
		if view != null:
			if not view.has_meta("fx_base"):
				view.set_meta("fx_base", view.position)
			var base: Vector3 = view.get_meta("fx_base")
			var a := Settings.m2v(float(s["shake_m"]))
			view.position = base + (Vector3(_rng.randf_range(-a, a), _rng.randf_range(0.0, a), _rng.randf_range(-a, a)) if on else Vector3.ZERO)


## Puts a trembled view back where it stands (the machine left the radius or was broken).
func _still(r: Dictionary) -> void:
	if work == null or not r["s"].has("shake_m"):
		return
	var view := work.view_at(r["e"].pos)
	if view != null and view.has_meta("fx_base"):
		view.position = view.get_meta("fx_base")


func _burst(effect: String, at: Vector3) -> void:
	stats["bursts"] += 1
	if fx != null:
		fx.burst(effect, at)


static func _axis(a: String) -> Vector3:
	match a:
		"x":
			return Vector3.RIGHT
		"z":
			return Vector3.BACK
	return Vector3.UP
