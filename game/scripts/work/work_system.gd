class_name WorkSystem
extends Node

## The player's hands on Vintage Story work pieces (ADR 0023). Player calls
## use() on right click and hit() on left click before its own handling;
## both return true when a work piece took the click. Places new pieces on
## top faces (flint -> knapping, clay -> clay forming, ground_place ware ->
## ground storage, firepit and sieve items), works existing ones, applies
## their results to the player's hand, and keeps one view node per piece,
## rebuilt from the entity's view_parts() when it changes.

var main: Node          # Main: blocks, items, recipes, player, set/get_world_voxel, spawn_item, chat, inv_screen
var picker: WorkPicker
var _root: Node3D
var _views := {}        # Vector3i -> Node3D
var _dirty := {}        # Vector3i -> true: rebuild next frame
var _picking: RefCounted = null
var _rng := RandomNumberGenerator.new()
var stats := {"placed": 0, "chips": 0, "clay_cells": 0, "finished": [], "kilns_lit": 0, "casts": 0, "sifts": 0,
	"rebuild_ms_max": 0.0, "use_ms_max": 0.0, "hit_ms_max": 0.0}   # one piece's mesh, one right / left click


func setup(p_main: Node) -> void:
	main = p_main
	_rng.seed = 2323
	_root = Node3D.new()
	_root.name = "WorkPieces"
	add_child(_root)
	picker = WorkPicker.new()
	picker.name = "WorkPicker"
	add_child(picker)
	picker.chosen.connect(_on_chosen)
	picker.closed.connect(func() -> void:
		if not main.inv_screen.is_open:
			main.player.set_ui_open(false))
	_prewarm()
	for e in main.blocks.all_work():
		_watch(e)


func _process(_delta: float) -> void:
	for p in _dirty:
		_rebuild(p)
	_dirty.clear()


# ---------------- views ----------------

## Draws both work materials once right in front of the camera while the
## loading screen covers it: their pipelines (front-face culling, the
## see-through ghost, shadows) compile there instead of in a ~1 s frame at the
## first knapping piece. Removed when the loading screen goes.
func _prewarm() -> void:
	var size := Vector3i(8, 1, 8)
	var c := WorkPalette.new_cells(size)
	WorkPalette.box(c, size, Vector3i(3, 0, 3), Vector3i(5, 1, 5), WorkPalette.FLINT)
	var warm := WorkPalette.make_view(Vector3i.ZERO, {"cells": c, "size": size}, {"cells": c, "size": size})
	warm.name = "WorkPrewarm"
	warm.position = Vector3(-0.5, -0.1, -1.2)
	main.player.camera.add_child(warm)
	var loading: Node = main.get("_loading")
	if loading != null and loading.has_signal("finished"):
		loading.finished.connect(func() -> void: warm.queue_free())
	else:
		get_tree().create_timer(3.0).timeout.connect(func() -> void: warm.queue_free())


func _watch(e: RefCounted) -> void:
	var p: Vector3i = e.pos
	if not e.changed.is_connected(_mark.bind(p)):
		e.changed.connect(_mark.bind(p))
	_dirty[p] = true


func _mark(p: Vector3i) -> void:
	_dirty[p] = true


func _rebuild(p: Vector3i) -> void:
	var t0 := Time.get_ticks_usec()
	_rebuild_view(p)
	var ms := (Time.get_ticks_usec() - t0) / 1000.0
	stats["rebuild_ms_max"] = maxf(stats["rebuild_ms_max"], ms)
	if ms > 4.0:
		stats["slow_rebuilds"] = stats.get("slow_rebuilds", []) + [[Engine.get_process_frames(), snappedf(ms, 0.1), String(main.blocks.at(p).kind) if main.blocks.at(p) else "gone"]]


func _rebuild_view(p: Vector3i) -> void:
	if _views.has(p):
		_views[p].queue_free()
		_views.erase(p)
	var e: RefCounted = main.blocks.at(p)
	if not BlockEntities.is_work(e):
		return
	var parts: Dictionary = e.view_parts()
	var v := WorkPalette.make_view(p, parts.get("solid", {}), parts.get("ghost", {}))
	_root.add_child(v)
	_views[p] = v


## The piece at p is gone (broken, finished): drop its view.
func forget(p: Vector3i) -> void:
	_dirty[p] = true


func view_count() -> int:
	return _views.size()


# ---------------- placing ----------------

func _consumes() -> bool:
	return GameMode.rule(main.player.game_mode, "consumes_items")


func _say(text: String) -> void:
	if text != "" and main.chat:
		main.chat.add_message(text)


## A new piece goes on the voxel above a solid top face (or, for a sieve,
## in front of any face); returns the voxel or (-1, -1, -1).
func _spot_for(hit: VoxelRaycastResult, any_face: bool) -> Vector3i:
	var t: Vector3i = hit.position
	var p: Vector3i = hit.previous_position
	if not any_face and p != t + Vector3i.UP:
		return Vector3i(-1, -1, -1)
	if not MobManager.is_solid(main.get_world_voxel(t)):
		return Vector3i(-1, -1, -1)
	var m: int = main.get_world_voxel(p)
	if m != GameMaterials.AIR and not (m > 0 and GameMaterials.defs()[m - 1].get("replaceable", false)):
		return Vector3i(-1, -1, -1)
	if main.player._overlaps_player(p) and any_face:
		return Vector3i(-1, -1, -1)
	return p


func _create(p: Vector3i, kind: String, material: int) -> RefCounted:
	var e: RefCounted = main.blocks.create(p, kind)
	main.set_world_voxel(p, material)
	_watch(e)
	stats["placed"] += 1
	return e


func _try_place(hit: VoxelRaycastResult, held: Dictionary) -> bool:
	var id: String = held.get("id", "")
	var tags: Array = held.get("tags", [])
	var work: String = held.get("work", "")
	var kind := ""
	if id == "flint":
		kind = "knap"
	elif id == "clay_ball":
		kind = "clayform"
	elif "ground_place" in tags:
		kind = "ground"
	elif work != "":
		kind = work
	if kind == "":
		return false
	var p := _spot_for(hit, kind == "sieve")
	if p.x == -1 and p.y == -1:
		return true   # took the click (no block placement for these items) but there is no room
	var e := _create(p, kind, GameMaterials.WORK_BLOCK if kind == "sieve" else GameMaterials.WORK_SPOT)
	match kind:
		"knap", "clayform":
			# clay is paid for while forming; flint and everything else now
			if kind == "knap" and _consumes():
				main.player_inv.consume_held(1)
			_open_picker(e)
		"ground":
			e.item = id
			_mark(p)
			if _consumes():
				main.player_inv.consume_held(1)
		_:
			if _consumes():
				main.player_inv.consume_held(1)
	return true


func _open_picker(e: RefCounted) -> void:
	var t := "knap" if e is KnapEntity else "clayform"
	var opts := []
	for r in main.recipes.of_type(t):
		if main.items.matches(e.input, r["input"]):
			opts.append(r)
	if opts.is_empty():
		return
	_picking = e
	main.player.set_ui_open(true, false)
	var extra := Callable()
	if t == "clayform":
		extra = func(r: Dictionary) -> String: return "%d clay" % ClayFormEntity.balls_for(r)
	var t0 := Time.get_ticks_usec()
	picker.open("Knapping" if t == "knap" else "Clay forming", opts, main.items, extra)
	stats["picker_ms_max"] = maxf(stats.get("picker_ms_max", 0.0), (Time.get_ticks_usec() - t0) / 1000.0)
	if main.player.autopilot:
		picker.close()   # automation picks through choose_id


func _on_chosen(r: Dictionary) -> void:
	if _picking != null and main.blocks.at(_picking.pos) == _picking:
		_picking.set_recipe(r)
	_picking = null


## Sets a piece's recipe without the picker (automation).
func choose(p: Vector3i, rid: String) -> bool:
	var e: RefCounted = main.blocks.at(p)
	if not (e is KnapEntity or e is ClayFormEntity) or not main.recipes.recipes.has(rid):
		return false
	e.set_recipe(main.recipes.recipes[rid])
	return true


# ---------------- working ----------------

## Where the camera ray crosses the horizontal plane `cells` above voxel p's
## floor, in cells of that voxel (may be outside 0..7); null when it misses.
static func plane_cell(p: Vector3i, cells: float, from: Vector3, dir: Vector3):
	if absf(dir.y) < 1e-5:
		return null
	var y := float(p.y) + cells / GameMaterials.MICRO_GRID
	var t := (y - from.y) / dir.y
	if t <= 0.0:
		return null
	var q := from + dir * t
	return Vector2((q.x - p.x) * GameMaterials.MICRO_GRID, (q.z - p.z) * GameMaterials.MICRO_GRID)


func _finish(e: RefCounted, out: Dictionary) -> void:
	var p: Vector3i = e.pos
	main.blocks.remove(p)
	main.set_world_voxel(p, GameMaterials.AIR)
	forget(p)
	if not out.is_empty():
		main.spawn_item({"id": out["id"], "count": int(out.get("count", 1))}, Vector3(p) + Vector3(0.5, 0.3, 0.5), Vector3(0, 1.5, 0))
		stats["finished"].append(out["id"])


func _give(stacks: Array, at: Vector3) -> void:
	for s in stacks:
		var left: int = main.player_inv.pickup(s["id"], int(s["count"]))
		if left > 0:
			main.spawn_item({"id": s["id"], "count": left}, at, Vector3(0, 1.5, 0))


func _is_work_voxel(p: Vector3i) -> bool:
	var m: int = main.get_world_voxel(p)
	return m == GameMaterials.WORK_SPOT or m == GameMaterials.WORK_BLOCK


## The work voxel a ray hit: its hit voxel, or the voxel it passed through
## last when that holds a piece the terrain does not show yet (a new piece
## lives in the simulation bubble until the next push; the aim ray reads the
## terrain). (-1, -1, -1) when neither is one.
func _work_at(hit: VoxelRaycastResult) -> Vector3i:
	if _is_work_voxel(hit.position):
		return hit.position
	if _is_work_voxel(hit.previous_position):
		return hit.previous_position
	return Vector3i(-1, -1, -1)


## Right click. True when a work piece (or a work item) took it.
func use(hit: VoxelRaycastResult, first_press: bool, from: Vector3, dir: Vector3) -> bool:
	var t0 := Time.get_ticks_usec()
	var took := _use(hit, first_press, from, dir)
	stats["use_ms_max"] = maxf(stats["use_ms_max"], (Time.get_ticks_usec() - t0) / 1000.0)
	return took


func _use(hit: VoxelRaycastResult, first_press: bool, from: Vector3, dir: Vector3) -> bool:
	if hit == null:
		return false
	var held: Dictionary = main.player_inv.held_def()
	var p: Vector3i = _work_at(hit)
	var m: int = main.get_world_voxel(p) if p.y >= 0 else GameMaterials.AIR
	if m != GameMaterials.WORK_SPOT and m != GameMaterials.WORK_BLOCK:
		if not first_press:
			return false
		return _try_place(hit, held)
	var e: RefCounted = main.blocks.at(p)
	if not BlockEntities.is_work(e):
		main.set_world_voxel(p, GameMaterials.AIR)   # a stray work voxel without its piece
		return true
	if e is KnapEntity:
		if first_press:
			if not e.has_recipe():
				_open_picker(e)
			else:
				_say("Chip off the dark flint with left clicks.")
		return true
	if e is ClayFormEntity:
		if not e.has_recipe():
			if first_press:
				_open_picker(e)
			return true
		if held.get("id", "") != e.input:
			if first_press:
				_say("Add clay with right clicks while holding clay.")
			return true
		var c = plane_cell(p, e.active_layer(), from, dir)
		if c == null:
			return true
		var balls: int = main.player_inv.held().get("count", 0) if _consumes() else 1 << 20
		var res: Dictionary = e.add(int(floor(c.x - 0.5)), int(floor(c.y - 0.5)), balls)
		stats["clay_cells"] += res["added"]
		if res["balls_used"] > 0 and _consumes():
			main.player_inv.consume_held(res["balls_used"])
		if res["done"]:
			_finish(e, e.output())
		return true
	if not first_press:
		if e is SieveEntity and e.input != "":
			pass   # holding right click keeps shaking
		else:
			return true
	if e is GroundEntity:
		var res: Dictionary = e.use(held)
		if res["consume"] > 0 and _consumes():
			main.player_inv.consume_held(res["consume"])
		if res["replace_held"] != "":
			main.player_inv.replace_held_one(res["replace_held"])
			stats["casts"] += 1
		if e.is_lit():
			stats["kilns_lit"] += 1 if res["msg"] == "The pit kiln is lit." else 0
		_give(res["give"], Vector3(p) + Vector3(0.5, 0.5, 0.5))
		if res["remove"]:
			main.blocks.remove(p)
			main.set_world_voxel(p, GameMaterials.AIR)
			forget(p)
		_say(res["msg"])
		return true
	if e is FirepitEntity:
		if "firestarter" in held.get("tags", []):
			_say("The firepit is lit." if e.ignite() else "Put fuel in the firepit first.")
			return true
		main.player.set_ui_open(true, false)
		main.inv_screen.open_container(e)
		return true
	if e is SieveEntity:
		var res: Dictionary = e.use(held, _rng)
		if res["consume"] > 0 and _consumes():
			main.player_inv.consume_held(res["consume"])
		if e.input == "" and res["consume"] == 0 and res["msg"] == "":
			stats["sifts"] += 1
		for s in res["drops"]:
			main.spawn_item(s, Vector3(p) + Vector3(0.5, 1.1, 0.5), Vector3(_rng.randf_range(-1, 1), 2.0, _rng.randf_range(-1, 1)))
		if first_press:
			_say(res["msg"])
		return true
	return true


## Left click. True when it chipped (or aimed at) a knapping piece; sneaking
## (Shift) falls through to mining so a piece can be broken.
func hit(hit_r: VoxelRaycastResult, from: Vector3, dir: Vector3, sneaking: bool) -> bool:
	var t0 := Time.get_ticks_usec()
	var took := _hit(hit_r, from, dir, sneaking)
	stats["hit_ms_max"] = maxf(stats["hit_ms_max"], (Time.get_ticks_usec() - t0) / 1000.0)
	return took


func _hit(hit_r: VoxelRaycastResult, from: Vector3, dir: Vector3, sneaking: bool) -> bool:
	if hit_r == null or sneaking:
		return false
	var p := _work_at(hit_r)
	var e: RefCounted = main.blocks.at(p) if p.y >= 0 else null
	if not (e is KnapEntity) or not e.has_recipe():
		return false
	var c = plane_cell(p, 1.0, from, dir)
	if c == null:
		return true
	var cell: Vector2i = e.nearest_marked(int(floor(c.x)), int(floor(c.y)), 1)
	if cell.x < 0:
		return true
	var r: String = e.chip(cell.x, cell.y)
	if r != "":
		stats["chips"] += 1
	if r == "done":
		_finish(e, e.output())
	return true


func summary() -> Dictionary:
	var s := stats.duplicate(true)
	s["views"] = _views.size()
	s["pieces"] = main.blocks.all_work().map(func(e: RefCounted) -> Dictionary: return e.summary())
	return s
