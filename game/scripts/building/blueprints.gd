class_name Blueprints
extends Node3D

## Blueprints and builders (ADR 0161). A blank `blueprint` in hand: right click opens the shape builder's Select box
## (its handles), a click copies the box onto a `written_blueprint` (Blueprint.capture). A written one: right click
## shows its ghost (the prefab ghost: R turns, X mirrors, no tipping), a click marks the site. A site stays marked
## (a see-through ghost of what is still to build, saved per world) until it is all built. A powered builder within
## Settings.builder_reach_m builds it voxel by voxel from a chest beside it (Settings.builder_voxels_per_s), skipping
## and listing what the chest lacks and never clearing more than air and plants; without one the hint names the next
## thing to place by hand. Voxels go in through ShapeTool.fill and, connected, the edit journal (ADR 0145).

const SCAN_PER_FRAME := 512     # cells a site checks a frame (GDScript reads: keep it bounded)
const HINT_M := 6.0             # the manual hint shows within this of a site

var main: Node
var st := ShapeTool.new()
var sites: Array = []           # [{data, lo, rot, mirror, lay, k, ps (the pass so far), node, remaining, next, worked}]
var dirty := false
var stats := {"marked": 0, "built": 0, "placed": 0, "entities": 0, "copied": 0}

var _ghost_mat: Material
var _hint: Label
var _marker: MeshInstance3D
var _item_of := {}              # material id -> item id
var _budget := {}               # BuilderEntity -> voxels it may still place (rate accumulator)


func setup(p_main: Node) -> void:
	main = p_main
	var layer := CanvasLayer.new()
	layer.layer = 5
	add_child(layer)
	_hint = Label.new()
	_hint.set_anchors_and_offsets_preset(Control.PRESET_CENTER_TOP)
	_hint.offset_top = 64.0
	_hint.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	_hint.mouse_filter = Control.MOUSE_FILTER_IGNORE
	_hint.add_theme_font_size_override("font_size", 15)
	_hint.add_theme_color_override("font_outline_color", Color.BLACK)
	_hint.add_theme_constant_override("outline_size", 5)
	layer.add_child(_hint)
	_marker = MeshInstance3D.new()
	var bm := BoxMesh.new()
	bm.size = Vector3.ONE * 1.2
	_marker.mesh = bm
	var mm := StandardMaterial3D.new()   # the handles' look (ShapeBuilder): unshaded, see-through, already compiled
	mm.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	mm.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
	mm.no_depth_test = true
	mm.cull_mode = BaseMaterial3D.CULL_DISABLED
	mm.vertex_color_use_as_albedo = true
	mm.render_priority = 3
	mm.albedo_color = Color(1.0, 0.85, 0.2, 0.8)
	bm.material = mm
	_marker.visible = false
	add_child(_marker)


func _ghost() -> Material:
	if _ghost_mat == null:   # the builder's ghost shader pair (warmed under the loading screen), in blueprint blue
		var g := ShaderMaterial.new()
		g.shader = preload("res://assets/shaders/brush_ghost.gdshader")
		g.render_priority = 1
		_ghost_mat = ShaderMaterial.new()
		_ghost_mat.shader = preload("res://assets/shaders/brush_ghost_depth.gdshader")
		_ghost_mat.next_pass = g
		var src: ShaderMaterial = main.player.brush_tool.ghost_mat
		for pn in BrushTool.ATLAS_PARAMS:
			g.set_shader_parameter(pn, src.get_shader_parameter(pn))
		g.set_shader_parameter("tint", Color(0.35, 0.6, 1.0))
		g.set_shader_parameter("tint_mix", 1.0)
		g.set_shader_parameter("rim", 0.3)
		g.set_shader_parameter("opacity", 0.35)
	return _ghost_mat


# ---------------- the item ----------------

## Right click with a blueprint in hand (Player.item_use, before the Stone Age). True when it took the click.
func use(_hit: VoxelRaycastResult, first_press: bool) -> bool:
	if not first_press:
		return false
	var held: Dictionary = main.player_inv.held()
	var b: ShapeBuilder = main.builder
	match String(held.get("id", "")):
		"blueprint":
			b.start_box("select")
			b.on_select = _copy_selection
			_toast("Blueprint: mark the box to copy")
			return true
		"written_blueprint":
			return start_placing(held.get("blueprint", {}))
	return false


func _copy_selection() -> void:
	var b: ShapeBuilder = main.builder
	if String(main.player_inv.held().get("id", "")) != "blueprint":
		_toast("Hold a blank blueprint to copy onto")
		return
	var d := Blueprint.capture(main, b.lo, b.wsize)
	if d.is_empty() or d.has("error"):
		_toast(d.get("error", "Nothing here to copy"))
		return
	main.player_inv.replace_held_one("written_blueprint", {"blueprint": d})
	stats["copied"] += 1
	_toast("Copied: " + Blueprint.describe(d, main.items))
	b._set_state(ShapeBuilder.State.OFF)


func start_placing(d: Dictionary) -> bool:
	var grid := Blueprint.ghost_grid(d)
	if grid.is_empty():
		_toast("This blueprint is blank or damaged")
		return true
	var b: ShapeBuilder = main.builder
	b.start_grid(grid, Blueprint.size_of(d), "Blueprint")
	b.on_commit = func(lo: Vector3i, rot: int, mirror: bool) -> void: mark(d, lo, rot, mirror)
	return true


# ---------------- sites ----------------

## Marks a site: the blueprint at world `lo`, turned and mirrored. Returns its index.
func mark(d: Dictionary, lo: Vector3i, rot := 0, mirror := false) -> int:
	var lay := Blueprint.layout(d, lo, rot, mirror)
	if lay.is_empty():
		return -1
	var s := {"data": d, "lo": lo, "rot": rot, "mirror": mirror, "lay": lay, "k": 0, "ps": _new_pass(), "remaining": -1, "next": {}, "worked": false}
	s["node"] = MeshInstance3D.new()
	s["node"].cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	s["node"].position = Vector3(lo)
	add_child(s["node"])
	_set_ghost(s, lay["cells"])
	sites.append(s)
	dirty = true
	stats["marked"] += 1
	_toast("Site marked: " + Blueprint.describe(d, main.items))
	return sites.size() - 1


func clear_sites() -> void:
	for s in sites:
		s["node"].queue_free()
	sites.clear()
	dirty = true


func _new_pass() -> Dictionary:
	return {"left": PackedInt32Array(), "missing": {}, "blocked": 0, "unloaded": 0, "first": {}, "placed": 0}


func _set_ghost(s: Dictionary, cells: PackedInt32Array) -> void:
	var node: MeshInstance3D = s["node"]
	if cells.is_empty():
		node.mesh = null
		return
	var arr: Array = st.mesh(cells, s["lo"], s["lay"]["size"])
	var m := ArrayMesh.new()
	if not (arr[Mesh.ARRAY_INDEX] as PackedInt32Array).is_empty():
		m.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arr)
		m.surface_set_material(0, _ghost())
	node.mesh = m


func _item_for(m: int) -> String:
	if not _item_of.has(m):
		_item_of[m] = main.items.item_for_material(GameMaterials.name_of(m))
	return _item_of[m]


## The chest beside a builder (any side or on top), or null.
func chest_of(b: BuilderEntity) -> ChestEntity:
	var w := BlockEntities.width_of(b.kind)
	var h := BlockEntities.height_of(b.kind)
	var c := w / 2
	for o in [Vector3i(-1, 0, c), Vector3i(w, 0, c), Vector3i(c, 0, -1), Vector3i(c, 0, w), Vector3i(c, h, c)]:
		var e: RefCounted = main.blocks.at(b.pos + o)
		if e is ChestEntity:
			return e
	return null


## The index of the nearest site within reach of a builder, or -1.
func _site_for(b: BuilderEntity) -> int:
	var best := -1
	var best_d := Settings.m2v(Settings.builder_reach_m)
	for i in sites.size():
		var box := AABB(Vector3(sites[i]["lo"]), Vector3(sites[i]["lay"]["size"]))
		var p := Vector3(b.pos)
		var d := (p.clamp(box.position, box.end) - p).length()
		if d <= best_d:
			best_d = d
			best = i
	return best


func _physics_process(delta: float) -> void:
	if main == null or main.blocks == null or main.bubble == null:
		return
	var working := {}   # site index -> the builder working it this frame
	for s in sites:
		s["worked"] = false
	for b in main.blocks.all_of("builder"):
		var i := _site_for(b)
		var chest := chest_of(b)
		b.has_site = i >= 0 and chest != null
		if not b.has_site:
			b.working = false
		if i < 0:
			b.status = "No blueprint site in reach"
		elif chest == null:
			b.status = "Needs a chest beside it"
		elif not b.powered():
			b.status = "Waiting for power"   # it asks the network for power while it has work (has_site)
		elif not working.has(i):
			b.status = "Building"
			working[i] = b
			_budget[b] = minf(float(_budget.get(b, 0.0)) + Settings.builder_voxels_per_s * delta, Settings.builder_voxels_per_s)
			sites[i]["chest"] = chest
			sites[i]["worked"] = true
	var list := sites.duplicate()   # a finished site leaves the list
	for i in list.size():
		_scan(list[i], working.get(i))
	_update_hint()


## Walks up to SCAN_PER_FRAME cells of a site; with a builder it places as it goes (while its budget lasts).
## A finished pass rebuilds the ghost from what is left and, when nothing is, the site is done.
func _scan(s: Dictionary, b: BuilderEntity) -> void:
	var lay: Dictionary = s["lay"]
	var cells: PackedInt32Array = lay["cells"]
	var mats: PackedByteArray = lay["mats"]
	var ents: Array = lay["entities"]
	var n := mats.size()
	var ps: Dictionary = s["ps"]
	var mask: PackedByteArray = main.builder._mask_free
	var bubble: SimBubble = main.bubble
	var put_cells := PackedInt32Array()
	var put_mats := PackedByteArray()
	var budget: float = _budget.get(b, 0.0) if b != null else 0.0
	var chest: ChestEntity = s.get("chest") if b != null else null
	for _i in SCAN_PER_FRAME:
		var k: int = s["k"]
		if k >= n + ents.size():
			_end_pass(s, b)
			break
		if k >= n:   # the pieces come after the voxels: the floor is down by then
			var e: Dictionary = ents[k - n]
			if _entity_done(e):
				s["k"] = k + 1
				continue
			if not _entity_free(e):
				ps["blocked"] += 1
			elif b == null or budget < 1.0:
				if b != null:
					break   # out of budget: come back next frame
				_note_left(ps, e["anchor"], e["item"])
			elif chest.inv.remove(e["item"], 1) == 1:
				_place_entity(e)
				budget -= 1.0
				b.placed += 1
				ps["placed"] += 1
				stats["entities"] += 1
			else:
				ps["missing"][e["item"]] = int(ps["missing"].get(e["item"], 0)) + 1
				_note_left(ps, e["anchor"], e["item"])
			s["k"] = k + 1
			continue
		var p := Vector3i(cells[3 * k], cells[3 * k + 1], cells[3 * k + 2])
		var m := mats[k]
		var cur := bubble.get_voxel(p)
		if cur < 0:
			ps["unloaded"] += 1
		elif cur == m or cur == GameMaterials.still_of(m):
			pass   # built
		elif mask[cur] == 0:
			ps["blocked"] += 1   # stone, a wall, someone's chest: never cleared
		else:
			var item := _item_for(m)
			if b != null and budget < 1.0:
				break
			if b != null and item != "" and chest.inv.remove(item, 1) == 1:
				put_cells.append_array([p.x, p.y, p.z])
				put_mats.append(m)
				budget -= 1.0
			else:
				if b != null:
					ps["missing"][item] = int(ps["missing"].get(item, 0)) + 1
				ps["left"].append_array([p.x, p.y, p.z])
				_note_left(ps, p, item)
		s["k"] = k + 1
	if b != null:
		_budget[b] = budget
	if not put_cells.is_empty():
		var window: Vector3i = bubble.origin * Settings.CHUNK
		var got := st.fill(bubble.sim, put_cells, 0, put_mats, window, mask)
		if Net.active:
			Net.send_op(NetOps.fill(Vector3i.ZERO, put_cells, 0, mask, put_mats))   # ADR 0145: the normal edit journal
		b.placed += got
		if not b.working:
			b.working = true
			b.changed.emit()
		ps["placed"] += got
		stats["placed"] += got


func _note_left(ps: Dictionary, p: Vector3i, item: String) -> void:
	if ps["first"].is_empty():
		ps["first"] = {"at": p, "item": item}


func _end_pass(s: Dictionary, b: BuilderEntity) -> void:
	var ps: Dictionary = s["ps"]
	var left: PackedInt32Array = ps["left"]
	var todo: int = left.size() / 3 + int(ps["blocked"]) + int(ps["unloaded"])
	for e in s["lay"]["entities"]:
		if not _entity_done(e):
			todo += 1
	if b != null:
		b.working = int(ps["placed"]) > 0   # sticky over a pass: the lit window does not flicker per frame
		b.missing = ps["missing"]
		b.blocked = int(ps["blocked"])
		if todo == 0:
			b.status = "Done"
	s["remaining"] = todo
	s["next"] = ps["first"]
	_set_ghost(s, left)
	s["k"] = 0
	s["ps"] = _new_pass()
	if todo == 0:
		s["node"].queue_free()
		sites.erase(s)
		dirty = true
		stats["built"] += 1
		_toast("Blueprint built")


func _entity_done(e: Dictionary) -> bool:
	var at: RefCounted = main.blocks.at(e["anchor"])
	return at != null and at.kind == e["kind"] and at.pos == e["anchor"] and (not at is FurnitureEntity or at.furniture == e["furniture"])


func _entity_free(e: Dictionary) -> bool:
	var mask: PackedByteArray = main.builder._mask_free
	var cells: Array = [e["anchor"]] if e["kind"] == "torch" else (FurnitureEntity.footprint(e["furniture"], e["anchor"], int(e.get("facing", 0)))
		if e["kind"] == "furniture" else BlockEntities.footprint_of(e["kind"], e["anchor"]))
	for c in cells:
		var m: int = main.bubble.get_voxel(c)
		if m < 0 or mask[m] == 0:
			return false
	return true


func _place_entity(e: Dictionary) -> void:
	Net.record(_place_entity_now.bind(e))   # ADR 0145: its voxels are shared like a hand placing it (entities are not, yet)


func _place_entity_now(e: Dictionary) -> void:
	var anchor: Vector3i = e["anchor"]
	if e["kind"] == "torch":
		main._place_torch(anchor)
		return
	if e["kind"] == "furniture":
		if main._place_furniture(e["furniture"], anchor, int(e.get("facing", 0))) and e["text"] != "":
			main.blocks.at(anchor).set_text(e["text"])
		return
	if StationEntity.SIZES.has(e["kind"]):
		main._on_placed(anchor, int(e["mat"]))   # the chest, furnace, crafting table: as a hand places one
		return
	var w: RefCounted = main.work._create(anchor, e["kind"], int(e["mat"]))
	if e["sub"] != "" and "item" in w:
		w.item = e["sub"]
	if e.has("facing") and "facing" in w:
		w.facing = int(e["facing"])
	if e.has("axis") and "axis" in w:
		w.axis = int(e["axis"])
	main.work._mark(anchor)


## By hand: near a site no builder works, the hint names what goes next and a marker shows where.
func _update_hint() -> void:
	var near := {}
	var pp: Vector3 = main.player.position
	for s in sites:
		var box := AABB(Vector3(s["lo"]), Vector3(s["lay"]["size"])).grow(Settings.m2v(HINT_M))
		if box.has_point(pp):
			near = s
			break
	var nx: Dictionary = near.get("next", {})
	if near.is_empty() or nx.is_empty() or near["worked"]:
		_hint.text = ""
		_marker.visible = false
		return
	var item: String = nx["item"]
	var have: int = main.builder._have(item) if item != "" else 0
	_hint.text = "Blueprint: %d left. Next: %s (you have %d)" % [int(near["remaining"]), main.items.display_name(item) if item != "" else "?", have]
	_marker.visible = true
	_marker.position = Vector3(nx["at"]) + Vector3.ONE * 0.5


func _toast(text: String) -> void:
	if main.player and main.player.hud:
		main.player.hud.show_toast(text)


# ---------------- saves ----------------

func to_dict() -> Dictionary:
	var list := []
	for s in sites:
		list.append({"data": s["data"], "lo": [s["lo"].x, s["lo"].y, s["lo"].z], "rot": s["rot"], "mirror": s["mirror"]})
	return {"version": 1, "sites": list}


func load_dict(d: Dictionary) -> void:
	clear_sites()
	for e in d.get("sites", []):
		var a: Array = e.get("lo", [])
		if a.size() == 3:
			mark(e["data"], Vector3i(int(a[0]), int(a[1]), int(a[2])), int(e.get("rot", 0)), bool(e.get("mirror", false)))
	dirty = false
	stats["marked"] = 0


func summary() -> Dictionary:
	var list := []
	for s in sites:
		list.append({"lo": s["lo"], "remaining": s["remaining"], "next": s["next"]})
	return {"sites": list, "stats": stats}


# ---------------- scripted check (blueprint_test=1) ----------------

## blueprint_test=save: a small furnished hut (planks, a furnace, a chair, a torch) copied onto a written blueprint
## held in slot 0 and a site marked beside it, then the world saved. blueprint_test=load (the same seed): the stack
## and the site came back with the same data. Prints "debug blueprint_save {...}".
func debug_save_test(stage: String) -> void:
	var r := {"stage": stage}
	await get_tree().create_timer(0.6).timeout
	var p: Player = main.player
	if stage == "save":
		p.autopilot_action = "look"
		var B := Settings.BLOCK
		var base := BlockEntities.anchor_for(Vector3i(p.position.floor()))
		var g0 := floori(float(main.world.surface_height(base.x, base.z)) / B) * B
		var lo := Vector3i(base.x - 3 * B, g0, base.z - 10 * B)
		var size := Vector3i(6 * B, 4 * B, 6 * B)
		await _fill(lo - Vector3i(B, 1, B), lo + size + Vector3i(B, B, B), GameMaterials.AIR)
		await _fill(lo, lo + Vector3i(size.x, 1, size.z), GameMaterials.PLANKS)
		await get_tree().create_timer(0.3).timeout
		main._on_placed(lo + Vector3i(B, 1, B), GameMaterials.FURNACE)
		main._place_furniture("chair", lo + Vector3i(4 * B, 1, 3 * B), 1)
		main._place_torch(lo + Vector3i(B, 1, 4 * B))
		await get_tree().create_timer(0.3).timeout
		var d := Blueprint.capture(main, lo, size)
		main.player_inv.inv.set_slot(0, {"id": "written_blueprint", "count": 1, "blueprint": d})
		mark(d, lo + Vector3i(0, 0, -8 * B), 1, false)
		main._save_blocks()
		r["kinds"] = d.get("entities", []).map(func(e: Dictionary) -> String: return String(e.get("furniture", e["kind"])))
		r["runs"] = (d["runs"] as Array).size()
		r["site_lo"] = str(sites[0]["lo"])
	else:
		var s: Dictionary = main.player_inv.inv.get_slot(0)
		var d: Dictionary = s.get("blueprint", {})
		r["held"] = s.get("id", "")
		r["kinds"] = d.get("entities", []).map(func(e: Dictionary) -> String: return String(e.get("furniture", e["kind"])))
		r["runs"] = (d.get("runs", []) as Array).size()
		r["sites"] = sites.size()
		r["site_lo"] = str(sites[0]["lo"]) if not sites.is_empty() else ""
		r["site_rot"] = int(sites[0]["rot"]) if not sites.is_empty() else -1
		r["site_entities"] = (sites[0]["lay"]["entities"] as Array).size() if not sites.is_empty() else 0
	print("debug blueprint_save " + JSON.stringify(r))


## A box fill once the bubble has loaded every chunk it touches (it loads nearest first after the world).
func _fill(lo: Vector3i, hi: Vector3i, m: int) -> void:
	for _i in 80:
		if main.bubble.fill_box(lo, hi, m) >= 0:
			return
		await get_tree().create_timer(0.25).timeout
	push_error("blueprint test: the bubble never loaded %s..%s" % [lo, hi])

## Builds a planks hut with a furnace, a chest and a hopper on a stone pad, copies it, marks it 6 m away,
## sets a powered builder and a full chest beside the site, films the build and compares the copy voxel for
## voxel (the two boxes read back as blueprints must match). Prints "debug blueprint {...}"; sheet blueprint.png.
func debug_test() -> void:
	var r := {}
	var tree := get_tree()
	await tree.create_timer(0.6).timeout
	var p: Player = main.player
	p.autopilot_action = "look"
	var B := Settings.BLOCK
	var base := BlockEntities.anchor_for(Vector3i(p.position.floor()))
	var g0 := floori(float(main.world.surface_height(base.x, base.z)) / B) * B   # the floor's y
	var hut := Vector3i(6 * B, 4 * B, 6 * B)   # 3 x 2 x 3 m
	var a_lo := Vector3i(base.x - 3 * B, g0, base.z - 12 * B)
	var b_lo := a_lo + Vector3i(10 * B, 0, 0)
	# a level pad under both, air above
	await _fill(a_lo + Vector3i(-2 * B, -B, -2 * B), a_lo + Vector3i(22 * B, 0, hut.z + 6 * B), GameMaterials.id_of("stone"))
	await _fill(a_lo + Vector3i(-2 * B, 0, -2 * B), a_lo + Vector3i(22 * B, 6 * B, hut.z + 6 * B), GameMaterials.AIR)
	var planks := GameMaterials.PLANKS
	var cobble := GameMaterials.id_of("cobblestone")
	await _fill(a_lo, a_lo + Vector3i(hut.x, 1, hut.z), cobble)   # the floor
	await _fill(a_lo + Vector3i(0, 1, 0), a_lo + Vector3i(hut.x, hut.y, hut.z), planks)
	await _fill(a_lo + Vector3i(1, 1, 1), a_lo + Vector3i(hut.x - 1, hut.y - 1, hut.z - 1), GameMaterials.AIR)
	await _fill(a_lo + Vector3i(2 * B, 1, hut.z - 1), a_lo + Vector3i(4 * B, 1 + 3 * B, hut.z), GameMaterials.AIR)   # the door
	await tree.create_timer(0.5).timeout
	main._on_placed(a_lo + Vector3i(B, 1, B), GameMaterials.FURNACE)
	main._on_placed(a_lo + Vector3i(3 * B, 1, B), GameMaterials.CHEST)
	var hop: RefCounted = main.work._create(a_lo + Vector3i(3 * B, 7, B), "hopper", GameMaterials.WORK_SPOT)
	hop.facing = 2
	main._place_furniture("chair", a_lo + Vector3i(B, 1, 4 * B), 1)
	main._place_torch(a_lo + Vector3i(4 * B, 1, 4 * B))
	await tree.create_timer(0.5).timeout
	var d := Blueprint.capture(main, a_lo, hut)
	r["bill"] = Blueprint.bill(d, main.items)
	r["entities"] = d.get("entities", []).map(func(e: Dictionary) -> String: return e["kind"])
	# through the item and the stack's save form: a written blueprint survives JSON
	d = JSON.parse_string(JSON.stringify(d))
	mark(d, b_lo)
	# the builder and its chest stand in front of the site; a cranked hand crank beside them powers it
	var bpos := b_lo + Vector3i(0, 0, hut.z + 2 * B)
	var builder: BuilderEntity = main.work._create(bpos, "builder", GameMaterials.WORK_BLOCK)
	var crank: PowerSourceEntity = main.work._create(bpos + Vector3i(-B, 0, 0), "hand_crank", GameMaterials.WORK_BLOCK)
	crank.cranked_until = Inventory.clock_s + 600.0
	main._on_placed(bpos + Vector3i(2 * B, 0, 0), GameMaterials.CHEST)
	var chest: ChestEntity = main.blocks.at(bpos + Vector3i(2 * B, 0, 0))
	var bill: Dictionary = r["bill"]
	var slot := 0
	for id in bill:
		var left := int(bill[id]) if id != "furnace" else 0   # no furnace: the builder lists it missing
		while left > 0 and slot < chest.inv.size():
			var n := mini(left, main.items.max_stack(id))
			chest.inv.set_slot(slot, {"id": id, "count": n})
			left -= n
			slot += 1
	r["power_ok"] = false
	p.position = Vector3(b_lo) + Vector3(hut.x * 0.5, 2 * B, hut.z + 9 * B)
	p.set_look(0.0, -0.35)
	var frames: Array[Image] = []
	var t0 := Time.get_ticks_msec()
	var furnace_given := false
	while Time.get_ticks_msec() - t0 < 60000 and not sites.is_empty():
		await tree.create_timer(1.0).timeout
		r["power_ok"] = r["power_ok"] or builder.powered()
		if frames.size() < 15 and int((Time.get_ticks_msec() - t0) / 1000) % 4 == 0:   # one frame every 4 s
			frames.append(get_viewport().get_texture().get_image())
		if not furnace_given and not builder.missing.is_empty():
			r["missing_seen"] = builder.missing.duplicate()
			chest.inv.add("furnace", 1)   # now it can finish
			furnace_given = true
	r["seconds"] = (Time.get_ticks_msec() - t0) / 1000.0
	r["builder"] = builder.summary()
	r["sites_left"] = sites.size()
	await tree.create_timer(0.5).timeout
	frames.append(get_viewport().get_texture().get_image())
	var copy := Blueprint.capture(main, b_lo, hut)
	var ga: PackedByteArray = PrefabStore.decode(d).get("grid", PackedByteArray())
	var gb: PackedByteArray = PrefabStore.decode(copy).get("grid", PackedByteArray())
	var diffs := 0
	var sample := []
	for i in mini(ga.size(), gb.size()):
		if ga[i] != gb[i]:
			diffs += 1
			if sample.size() < 6:
				sample.append("%d:%s->%s" % [i, GameMaterials.name_of(ga[i]), GameMaterials.name_of(gb[i])])
	r["voxel_diffs"] = diffs
	r["diff_sample"] = sample
	r["voxels_match"] = ga.size() == gb.size() and diffs == 0
	var ek := func(x: Dictionary) -> Array:
		var l: Array = x.get("entities", []).map(func(e: Dictionary) -> String: return "%s@%d,%d,%d f%d" % [e.get("furniture", e["kind"]), int(e["off"][0]), int(e["off"][1]), int(e["off"][2]), int(e.get("facing", -1))])   # JSON made the ints floats
		l.sort()
		return l
	r["entities_match"] = ek.call(copy) == ek.call(d)
	r["copy_entities"] = ek.call(copy)
	r["stats"] = stats
	if not frames.is_empty():
		var w := frames[0].get_width() / 3
		var h := frames[0].get_height() / 3
		var cols := 4
		var sheet := Image.create(w * cols, h * ceili(frames.size() / float(cols)), false, frames[0].get_format())
		for i in frames.size():
			var f := frames[i]
			f.resize(w, h)
			sheet.blit_rect(f, Rect2i(Vector2i.ZERO, Vector2i(w, h)), Vector2i((i % cols) * w, (i / cols) * h))
		DirAccess.make_dir_recursive_absolute(DebugCapture.out_dir)
		sheet.save_png(DebugCapture.out_dir + "/blueprint.png")
	print("debug blueprint " + JSON.stringify(r))
