class_name Fishing
extends Node

## Fishing with a rod and the land traps (ADR 0163), a child of Main.
## The rod (StoneAge hands every first right click with a fishing line here): the bobber is cast
## along the aim onto the first water, floats on its real surface, and gets a bite after a wait
## that n fish within fish_near_m divide by 1 + n. A right click inside the bite's window sets the
## hook; then the reel: holding right click pulls the fish in and tightens the line, the fish
## surges by its pull, a full line snaps and a slack one lets it throw the hook. The species comes
## from fishing.json by water (river, lake, sea) and biome; fouled water (ADR 0155) gives junk or nothing.
## Traps are TrapEntity work pieces; every TRAP_S this node springs them on what stands in them:
## small animals in snares, the rest down pits (Mob.trap_at holds them), and the player too (real
## damage). Tamed companions are never caught and step round them (MobManager.safe_ahead).

enum State { IDLE, FLYING, WAITING, BITE, REEL }

const DATA := "res://data/fishing.json"
const TRAP_S := 0.2
const FLIGHT_S := 0.6

var main: Node
var state := State.IDLE
var auto := false              # automation: sets the hook at once and reels to keep the tension mid-band
var species: Array = []
var junk: Array = []
var tension := 0.0             # the reel: 1 snaps the line
var progress := 0.0            # 1 lands the fish
var fish := {}                 # the species entry on the hook ({} for junk)
var catch_id := ""             # the item it lands as
var water := ""                # river | lake | sea, where the bobber is
var fouled := false
var stats := {"casts": 0, "bites": 0, "hooked": 0, "caught": [], "lost": 0, "snapped": 0, "missed": 0,
	"trapped": [], "player_caught": 0, "taken": []}
var _t := 0.0
var _surge_t := 0.0
var _slack_t := 0.0
var _from := Vector3.ZERO
var _to := Vector3.ZERO        # where the bobber floats (x, z); y follows the water
var _bob_y := 0.0
var _rng := RandomNumberGenerator.new()
var _bobber: MeshInstance3D
var _line: MeshInstance3D
var _ui: CanvasLayer
var _label: Label
var _tension_bar: ColorRect
var _progress_bar: ColorRect
var _trap_t := 0.0
var _traps: Array = []
var _traps_t := 0.0
var scene_at := Vector3.ZERO  # the last /fish scene's middle (the command films it)
var _held: TrapEntity = null   # the snare round the player's foot
var _hold_t := 0.0


func setup(p_main: Node) -> void:
	main = p_main
	_rng.randomize()
	var data = JSON.parse_string(FileAccess.get_file_as_string(DATA))
	if data is Dictionary:
		species = data.get("species", [])
		junk = data.get("junk", [])
	var bm := StandardMaterial3D.new()
	bm.albedo_color = Color(0.85, 0.12, 0.08)
	var sm := SphereMesh.new()
	sm.radius = Settings.m2v(0.05)
	sm.height = Settings.m2v(0.1)
	sm.radial_segments = 8
	sm.rings = 4
	sm.material = bm
	_bobber = _mesh(sm, "Bobber")
	var lm := StandardMaterial3D.new()
	lm.albedo_color = Color(0.86, 0.84, 0.76)
	lm.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	var cm := CylinderMesh.new()
	cm.top_radius = Settings.m2v(0.006)
	cm.bottom_radius = cm.top_radius
	cm.height = 1.0
	cm.radial_segments = 4
	cm.rings = 1
	cm.material = lm
	_line = _mesh(cm, "FishingLine")
	_build_ui()


func warm_materials() -> Array:
	return [(_bobber.mesh as SphereMesh).material, (_line.mesh as CylinderMesh).material]


func _mesh(m: Mesh, n: String) -> MeshInstance3D:
	var mi := MeshInstance3D.new()
	mi.name = n
	mi.mesh = m
	mi.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	mi.visible = false
	add_child(mi)
	return mi


func _build_ui() -> void:
	_ui = CanvasLayer.new()
	_ui.layer = 4
	_ui.visible = false
	add_child(_ui)
	var box := Control.new()
	box.set_anchors_preset(Control.PRESET_CENTER_BOTTOM)
	box.position = Vector2(-160, -190)
	box.size = Vector2(320, 60)
	box.mouse_filter = Control.MOUSE_FILTER_IGNORE
	box.anchor_left = 0.5
	box.anchor_right = 0.5
	box.anchor_top = 1.0
	box.anchor_bottom = 1.0
	box.offset_left = -160
	box.offset_right = 160
	box.offset_top = -200
	box.offset_bottom = -140
	_ui.add_child(box)
	_label = Label.new()
	_label.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	_label.size = Vector2(320, 20)
	_label.add_theme_color_override("font_outline_color", Color.BLACK)
	_label.add_theme_constant_override("outline_size", 4)
	box.add_child(_label)
	for i in 2:
		var bg := ColorRect.new()
		bg.color = Color(0, 0, 0, 0.55)
		bg.position = Vector2(0, 24 + i * 18)
		bg.size = Vector2(320, 12)
		box.add_child(bg)
		var fill := ColorRect.new()
		fill.position = Vector2(1, 1)
		fill.size = Vector2(0, 10)
		bg.add_child(fill)
		if i == 0:
			_tension_bar = fill
			var mark := ColorRect.new()   # past here the line is close to snapping
			mark.color = Color(1, 1, 1, 0.8)
			mark.position = Vector2(318 * 0.85, 0)
			mark.size = Vector2(2, 12)
			bg.add_child(mark)
		else:
			_progress_bar = fill
			fill.color = Color(0.35, 0.65, 1.0)


# ---------------- the rod ----------------

func _wet(v: int) -> bool:
	var b := GameMaterials.material_of_voxel(v)
	return b == GameMaterials.WATER or b == GameMaterials.WATER_STATIC or b == GameMaterials.FOULED_WATER


## A first right click with a fishing line (from StoneAge.use): casts, reels in, or sets the hook.
func use_rod(from: Vector3, dir: Vector3) -> bool:
	match state:
		State.IDLE:
			_cast(from, dir)
		State.BITE:
			_hook()
		State.FLYING, State.WAITING:
			_end("You reel your line in.")
	return true


func _cast(from: Vector3, dir: Vector3) -> void:
	var step := 0.5
	var p := from
	var hit := Vector3.INF
	for _i in int(Settings.m2v(Settings.fish_cast_m) / step):
		p += dir * step
		var v: int = main.get_world_voxel(Vector3i(p.floor()))
		if _wet(v):
			hit = p
			break
		if MobManager.is_solid(v):
			break
	if hit == Vector3.INF:
		main.work._say("No water to cast into there.")
		return
	var top := _surface(Vector3i(hit.floor()))
	if top < -1e8:
		return
	_from = _hand()
	_to = Vector3(hit.x, top, hit.z)
	_bob_y = top
	_set_state(State.FLYING, FLIGHT_S)
	stats["casts"] += 1
	main.work._wear_held()


## The top of the water column at cell c (world y of its surface), or -INF when no water is there.
func _surface(c: Vector3i) -> float:
	var y := c.y
	if not _wet(main.get_world_voxel(Vector3i(c.x, y, c.z))):
		y -= 1
		if not _wet(main.get_world_voxel(Vector3i(c.x, y, c.z))):
			return -INF
	for _i in 32:
		if not _wet(main.get_world_voxel(Vector3i(c.x, y + 1, c.z))):
			return y + 1.0
		y += 1
	return y + 1.0


func _hand() -> Vector3:
	var cam: Camera3D = main.player.camera
	var b := cam.global_transform.basis
	return cam.global_position + b.x * Settings.m2v(0.3) - b.y * Settings.m2v(0.15) - b.z * Settings.m2v(0.9)


func _set_state(s: State, t: float = 0.0) -> void:
	state = s
	_t = t


func _land() -> void:
	main.work._particles("splash", _to, GameMaterials.WATER)
	var c := Vector3i(_to.floor()) - Vector3i(0, 1, 0)
	fouled = false
	for dz in [-1, 0, 1]:
		for dx in [-1, 0, 1]:
			if GameMaterials.material_of_voxel(main.get_world_voxel(c + Vector3i(dx * 2, 0, dz * 2))) == GameMaterials.FOULED_WATER:
				fouled = true
	water = water_kind(c)
	_set_state(State.WAITING, bite_wait(fish_near(_to)))


## river, sea or lake, from the generator's column at c.
func water_kind(c: Vector3i) -> String:
	var info: Dictionary = main.world.generator.column_info(c.x, c.z)
	if float(info.get("river", 0.0)) > 0.05:
		return "river"
	if int(info.get("height", Settings.SEA_LEVEL_VOXEL)) <= Settings.SEA_LEVEL_VOXEL - Settings.m2v(1.0) and c.y <= Settings.SEA_LEVEL_VOXEL + 1:
		return "sea"
	return "lake"


## Live swimmers (fish, salmon, mackerel ...) within fish_near_m of p.
func fish_near(p: Vector3) -> int:
	var n := 0
	var r := Settings.m2v(Settings.fish_near_m)
	for m in main.mobs.active():
		if m.def.get("moves", "") == "swim" and m.species != "jellyfish" and not m.is_dead() and m.position.distance_to(p) < r:
			n += 1
	return n


## Seconds to the bite with n fish near: the long wait divided by 1 + n, never under the minimum.
static func bite_wait_for(n: int, roll: float) -> float:
	return maxf(Settings.fish_bite_min_s, Settings.fish_bite_max_s / (1.0 + n) * roll)


func bite_wait(n: int) -> float:
	return bite_wait_for(n, _rng.randf_range(0.6, 1.4))


## The species entries that bite in `p_water` in biome `biome`.
func candidates(p_water: String, biome: String) -> Array:
	var out := []
	for s in species:
		if p_water in s.get("water", []) and (s.get("biomes", []).is_empty() or biome in s["biomes"]):
			out.append(s)
	return out


static func pick(list: Array, roll: float) -> Dictionary:
	var total := 0.0
	for s in list:
		total += float(s.get("weight", 1.0))
	var at := roll * total
	for s in list:
		at -= float(s.get("weight", 1.0))
		if at <= 0.0:
			return s
	return list[-1] if not list.is_empty() else {}


func _bite() -> void:
	stats["bites"] += 1
	if fouled:
		var j := pick(junk, _rng.randf())
		fish = {"name": "", "pull": 0.5}
		catch_id = String(j.get("id", ""))
	else:
		var info: Dictionary = main.world.generator.column_info(int(_to.x), int(_to.z))
		var c := candidates(water, String(info.get("biome_name", "")))
		if c.is_empty():
			c = candidates("lake", "")
		fish = pick(c, _rng.randf())
		catch_id = String(fish.get("item", "raw_fish"))
	main.work._particles("splash", _to, GameMaterials.WATER)
	_set_state(State.BITE, Settings.fish_hook_s)
	if auto or main.player.autopilot:
		_hook()


func _hook() -> void:
	stats["hooked"] += 1
	tension = 0.35
	progress = 0.0
	_slack_t = 0.0
	_surge_t = _rng.randf_range(0.6, 1.4)
	_set_state(State.REEL)
	main.work._say("Hooked! Hold right click to reel; ease off before the line snaps.")


func _reeling() -> bool:
	if auto or main.player.autopilot:
		return tension < 0.55
	return Input.is_action_pressed("action_place") and not main.player.ui_open


func _reel(delta: float) -> void:
	var pull := float(fish.get("pull", 1.0))
	var reel := _reeling()
	tension += (Settings.fish_tension_up if reel else -Settings.fish_tension_down) * delta
	if reel:
		progress += delta / (Settings.fish_reel_s * pull)
	_surge_t -= delta
	if _surge_t <= 0.0:
		_surge_t = _rng.randf_range(0.7, 1.6) / pull
		tension += Settings.fish_surge * pull
		main.work._particles("splash", Vector3(_to.x, _bob_y, _to.z), GameMaterials.WATER)
	tension = maxf(tension, 0.0)
	_slack_t = _slack_t + delta if tension < 0.05 else 0.0
	if tension >= 1.0:
		stats["snapped"] += 1
		main.work._wear_held()
		_end("The line snaps!")
	elif _slack_t > Settings.fish_slack_s:
		stats["lost"] += 1
		_end("The line went slack and it threw the hook.")
	elif progress >= 1.0:
		_catch()


func _catch() -> void:
	main.work._wear_held()
	if catch_id == "":
		_end("Only weed on the hook: the water here is foul.")
		return
	main.work._give([{"id": catch_id, "count": 1}], main.player.position)
	stats["caught"].append(catch_id)
	main.work._note("caught", catch_id)
	var nm: String = fish.get("name", "")
	_end(("A %s!" % nm) if nm != "" else "Something comes up: %s." % main.items.display_name(catch_id))


func _end(msg: String) -> void:
	if msg != "":
		main.work._say(msg)
	_set_state(State.IDLE)
	tension = 0.0
	progress = 0.0
	_bobber.visible = false
	_line.visible = false
	_ui.visible = false


func _holds_rod() -> bool:
	var tags: Array = main.player_inv.held_def().get("tags", [])
	return "fishing" in tags and not "net" in tags


func _process(delta: float) -> void:
	if state == State.IDLE:
		return
	if not _holds_rod():
		_end("")
		return
	if main.player.position.distance_to(_to) > Settings.m2v(Settings.fish_line_m):
		_end("You walked off too far: the line snaps.")
		return
	_t -= delta
	match state:
		State.FLYING:
			if _t <= 0.0:
				_land()
		State.WAITING:
			if _t <= 0.0:
				_bite()
		State.BITE:
			if _t <= 0.0:
				stats["missed"] += 1
				main.work._say("It took the bait and swam off.")
				_set_state(State.WAITING, bite_wait(fish_near(_to)))
		State.REEL:
			_reel(delta)
	if state == State.IDLE:
		return
	_draw(delta)


func _draw(_delta: float) -> void:
	var top := _surface(Vector3i(_to.floor()))
	if top < -1e8 and state != State.FLYING:
		_end("Your bobber lies on dry ground.")
		return
	if top > -1e8:
		_bob_y = top
	var now := Time.get_ticks_msec() * 0.001
	var p: Vector3
	if state == State.FLYING:
		var f := clampf(1.0 - _t / FLIGHT_S, 0.0, 1.0)
		p = _from.lerp(_to, f) + Vector3(0, sin(f * PI) * Settings.m2v(1.5), 0)
	else:
		var dip := 0.0
		if state == State.BITE:
			dip = -Settings.m2v(0.06) * absf(sin(now * 18.0))
		elif state == State.REEL:
			dip = -Settings.m2v(0.08) * tension
		p = Vector3(_to.x, _bob_y + Settings.m2v(0.02) + sin(now * 2.2) * Settings.m2v(0.012) + dip, _to.z)
	_bobber.global_position = p
	_bobber.visible = true
	var hand := _hand()
	var v := p - hand
	if v.length() > 0.05:
		var y := v.normalized()
		var x := y.cross(Vector3.UP if absf(y.y) < 0.99 else Vector3.RIGHT).normalized()
		_line.global_transform = Transform3D(Basis(x, y * v.length(), x.cross(y)), hand + v * 0.5)
		_line.visible = true
	_ui.visible = state == State.BITE or state == State.REEL
	if state == State.BITE:
		_label.text = "A bite! Right click!"
		_tension_bar.size.x = 0
		_progress_bar.size.x = 0
	elif state == State.REEL:
		_label.text = "Hold right click to reel"
		_tension_bar.size.x = 318.0 * clampf(tension, 0.0, 1.0)
		_tension_bar.color = Color(0.3, 0.85, 0.3).lerp(Color(0.95, 0.15, 0.1), clampf(tension, 0.0, 1.0))
		_progress_bar.size.x = 318.0 * clampf(progress, 0.0, 1.0)


# ---------------- traps ----------------

## A right click (Main's item_use, before the Stone Age): a trap aimed at, or a pit cover laid into a pit.
func use_world(hit: VoxelRaycastResult, first: bool) -> bool:
	if not first or hit == null:
		return false
	var p: Vector3i = main.work._work_at(hit)
	var e: RefCounted = main.blocks.at(p) if p.y >= 0 else null
	var held: Dictionary = main.player_inv.held_def()
	if e is TrapEntity:
		_use_trap(e, held)
		return true
	if held.get("id", "") == "pit_cover":
		_lay_cover(hit)
		return true
	return false


func _use_trap(e: TrapEntity, held: Dictionary) -> void:
	if e.catch != "":
		var m := held_mob(e)
		var what := e.catch
		if m != null:
			m.trap_at = Vector3.INF
			m.position = main.player.position + Vector3(0, Settings.m2v(0.5), 0)
			main.mobs.on_died(m)   # its drops, at the player's feet
		stats["taken"].append(what)
		if e.is_pit():
			e.catch = ""
			e.changed.emit()
		else:
			e.reset()
		main.work._say("You take the %s." % what.replace("_", " "))
		return
	if e.sprung and e.is_pit():
		if held.get("id", "") == "pit_cover":
			if main.work._consumes():
				main.player_inv.consume_held(1)
			e.reset()
			main.work._say("The pit is covered again.")
		else:
			main.work._say(e.look_lines()[0])
		return
	if e.sprung:
		e.reset()
		main.work._say("You set the snare again.")
		return
	if held.get("id", "") == "":
		var back := e.contents()
		main.work.remove_piece(e)
		main.work._give(back, main.player.position)
		_traps.erase(e)


## The trapped animal a trap holds (null if it is not loaded or gone).
func held_mob(e: TrapEntity) -> Mob:
	var c := e.centre()
	for m in main.mobs.active():
		if m.trap_at != Vector3.INF and Vector2(m.trap_at.x - c.x, m.trap_at.z - c.z).length() < Settings.BLOCK and not m.is_dead():
			return m
	return null


## Where a 1 m pit cover would lie for a pit around the aimed spot: {anchor, depth} or {}.
func pit_spot(hit: VoxelRaycastResult) -> Dictionary:
	var c := hit.previous_position
	var B := Settings.BLOCK
	var a0 := BlockEntities.anchor_for(c)
	for off in [Vector3i(-B, 0, -B), Vector3i(0, 0, -B), Vector3i(-B, 0, 0), Vector3i(0, 0, 0)]:
		var a: Vector3i = a0 + off
		var mid := Vector3i(a.x + B, c.y, a.z + B)
		var y := mid.y
		var lo := y - 48
		while y > lo and not MobManager.is_solid(main.get_world_voxel(Vector3i(mid.x, y - 1, mid.z))):
			y -= 1
		var floor_y := y
		var hi := floor_y + 48
		while y < hi and _square_air(a, y) and _ring_solid(a, y):
			y += 1
		if y - floor_y >= int(Settings.m2v(Settings.pit_min_m)) and _square_air(a, y):
			return {"anchor": Vector3i(a.x, y - 2, a.z), "depth": y - floor_y}
	return {}


func _square_air(a: Vector3i, y: int) -> bool:
	for z in 2 * Settings.BLOCK:
		for x in 2 * Settings.BLOCK:
			if MobManager.is_solid(main.get_world_voxel(Vector3i(a.x + x, y, a.z + z))):
				return false
	return true


func _ring_solid(a: Vector3i, y: int) -> bool:
	var w := 2 * Settings.BLOCK
	var n := 0
	var solid := 0
	for i in range(-1, w + 1):
		for q in [Vector3i(i, 0, -1), Vector3i(i, 0, w), Vector3i(-1, 0, i), Vector3i(w, 0, i)]:
			n += 1
			if MobManager.is_solid(main.get_world_voxel(Vector3i(a.x, y, a.z) + q)):
				solid += 1
	return solid >= n * 0.75


func _lay_cover(hit: VoxelRaycastResult) -> void:
	var s := pit_spot(hit)
	if s.is_empty():
		main.work._say("Dig a pit 1 m wide and at least %.0f m deep, then lay the cover into it." % Settings.pit_min_m)
		return
	lay_pit(s["anchor"])
	if main.work._consumes():
		main.player_inv.consume_held(1)
	main.work._say("A pit trap: sticks and dry grass over the hole.")


func lay_pit(anchor: Vector3i) -> TrapEntity:
	var e: TrapEntity = main.work._create(anchor, "pit_trap", GameMaterials.WORK_SPOT)
	_traps_t = 0.0
	return e


## A snare or pit trap at the walking point p (a companion's probe ahead).
func trap_near(p: Vector3) -> bool:
	var c := Vector3i(p.floor())
	for k in 3:
		if main.blocks.at(c - Vector3i(0, k, 0)) is TrapEntity:
			return true
	return false


func _physics_process(delta: float) -> void:
	if main == null or main.mobs == null:
		return
	if _held != null:
		_hold_t -= delta
		var hc := _held.centre()
		main.player.position.x = hc.x
		main.player.position.z = hc.z
		if _hold_t <= 0.0:
			_held = null
			main.work._say("You work your foot out of the snare.")
	_trap_t += delta
	if _trap_t < TRAP_S:
		return
	_trap_t = 0.0
	_traps_t -= TRAP_S
	if _traps_t <= 0.0:
		_traps_t = 2.0
		_traps = main.blocks.all_of("snare") + main.blocks.all_of("pit_trap")
	for e in _traps:
		if e.armed() and main.blocks.at(e.pos) == e:
			_check(e)


func _small(m: Mob) -> bool:
	var s: Array = m.def.get("size_m", [0.5, 0.5])
	return float(s[0]) <= Settings.snare_max_m


func _check(e: TrapEntity) -> void:
	var c := e.centre()
	var half := BlockEntities.width_of(e.kind) * 0.5
	for m in main.mobs.active():
		if m.owner_id != "" or m.is_dead() or m.trap_at != Vector3.INF or m.def.get("moves", "") in ["swim", "fly"]:
			continue
		var d := Vector2(m.position.x - c.x, m.position.z - c.z)
		if e.is_pit():
			if absf(d.x) < half and absf(d.y) < half and m.position.y < c.y + 0.5 and m.position.y > c.y - Settings.m2v(3.0):
				_spring(e, m, Vector3(c.x, m.position.y, c.z))
				return
		elif _small(m) and d.length() < Settings.m2v(0.3) and absf(m.position.y - c.y) < 3.0:
			m.position = Vector3(c.x, m.position.y, c.z)
			_spring(e, m, m.position)
			return
	if not main.mobs.player_attackable():
		return
	var pp: Vector3 = main.player.position
	var dp := Vector2(pp.x - c.x, pp.z - c.z)
	if e.is_pit():
		if absf(dp.x) < half and absf(dp.y) < half and pp.y < c.y and pp.y > c.y - Settings.m2v(3.0):
			e.spring("")
			stats["player_caught"] += 1
			main.player_inv.damage(Settings.pit_hurt, "pit trap")
			main.work._say("The ground gives way: stakes in a pit!")
	elif dp.length() < Settings.m2v(0.25) and absf(pp.y - c.y) < 3.0:
		e.spring("")
		stats["player_caught"] += 1
		_held = e
		_hold_t = Settings.snare_hold_s
		main.player_inv.damage(Settings.snare_hurt, "snare")
		main.work._say("A snare snaps shut on your ankle!")


func _spring(e: TrapEntity, m: Mob, at: Vector3) -> void:
	m.trap_at = at
	e.spring(m.species)
	stats["trapped"].append(m.species)
	main.work._note("trapped", m.species)


func snapshot() -> Dictionary:
	return {"state": State.keys()[state], "tension": snappedf(tension, 0.01), "progress": snappedf(progress, 0.01),
		"water": water, "fouled": fouled, "on_hook": catch_id, "stats": stats, "traps": _traps.size()}


# ---------------- debug scenes (/fish) ----------------

## `/fish pond`: a stone tub of water 3 m ahead (where the camera looks) with fish, a rod in hand;
## `/fish foul`: the same fouled; `/fish traps`: a snare with a rabbit stepping in, a pit trap a deer
## drops into and a tamed wolf, on a grass slab ahead.
func debug_scene(what: String) -> String:
	var player: Node3D = main.player
	var B := Settings.BLOCK
	var fwd: Vector3 = -player.camera.global_transform.basis.z
	fwd.y = 0.0
	var at: Vector3 = player.position + fwd.normalized() * Settings.m2v(3.5)
	var o := Vector3i(floori(at.x / B) * B, int(floor(player.position.y)), floori(at.z / B) * B)
	scene_at = Vector3(o)
	if what in ["pond", "foul"]:
		main._debug_fill(o + Vector3i(-14, -10, -14), o + Vector3i(14, 0, 14), GameMaterials.STONE)
		main._debug_fill(o + Vector3i(-13, -9, -13), o + Vector3i(13, 0, 13), GameMaterials.FOULED_WATER if what == "foul" else GameMaterials.WATER_STATIC)
		main._debug_fill(o + Vector3i(-14, 0, -14), o + Vector3i(14, 3 * B, 14), GameMaterials.AIR)
		if what == "pond":
			for i in 4:
				main.mobs.spawn("fish", Vector3(o) + Vector3(-6.0 + 4.0 * i, -5.0, -3.0 + 6.0 * (i % 2)))
		main.player_inv.inv.set_slot(main.player_inv.selected, {"id": "fishing_rod", "count": 1})
		return "a %s pond ahead (look down at it and right click with the rod)" % what
	# traps: a dirt slab, a snare and a 1.5 m pit side by side, the wolf between them and the player
	main._debug_fill(o + Vector3i(-20, -16, -12), o + Vector3i(20, -1, 12), GameMaterials.DIRT)
	main._debug_fill(o + Vector3i(-20, -1, -12), o + Vector3i(20, 0, 12), GameMaterials.GRASS)
	main._debug_fill(o + Vector3i(-20, 0, -12), o + Vector3i(20, 4 * B, 12), GameMaterials.AIR)
	var pit := o + Vector3i(-12, 0, -4)
	main._debug_fill(pit + Vector3i(0, -12, 0), pit + Vector3i(8, 0, 8), GameMaterials.AIR)
	var pe := lay_pit(pit + Vector3i(0, -2, 0))
	var pit2 := o + Vector3i(-2, 0, 4)   # a second pit left armed: how the cover looks
	main._debug_fill(pit2 + Vector3i(0, -12, 0), pit2 + Vector3i(8, 0, 8), GameMaterials.AIR)
	lay_pit(pit2 + Vector3i(0, -2, 0))
	var sn: RefCounted = main.work._create(o + Vector3i(6, 0, -2), "snare", GameMaterials.WORK_SPOT)
	main.mobs.spawn("rabbit", sn.centre() + Vector3(Settings.m2v(0.1), 0.2, 0))
	main.mobs.spawn("deer", pe.centre() + Vector3(0, 1.0, 0), false, PI * 0.5)
	var w: Mob = main.mobs.spawn("wolf", Vector3(o) + Vector3(0, 0.2, 0))
	if w != null:
		w.set_companion(player)
	return "a snare (rabbit), a pit trap (deer) and a tamed wolf ahead"
