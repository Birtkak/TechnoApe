class_name Waste
extends RefCounted

## Factory waste as matter (ADR 0141). A recipe's "waste" ([{material, cells}]) and an item's "firebox" ({ash: cells a
## fuel item, smoke_s: cells a second while lit}) fill an entity's `waste` (material id -> cells waiting). Once a second
## WorkSystem calls emit_all: every entity with waste lets it out as real cells into free cells (air) at its port:
## gases out of the top, or out of the top of a chimney stack standing on it; liquids and powders at the back face, at
## floor level (the back is Transport.DIRS[facing], where the copper terminal is). A liquid whose back port touches a
## pipe goes down the pipes instead: into a machine on them whose item takes it ("machine": {"intake": ...}, the
## scrubber), else out of the far end of the run. A port that cannot take it fills the buffer; at
## Settings.waste_port_cells the entity stalls and says so (MachineEntity.current, FirepitEntity._burn_next).
## Nothing runs without waste: no scans of the world, a few voxel reads per port a second.

const PIPE_KINDS := ["pipe"]

static var _firebox := {}   # item id -> its "firebox" def ({} for none)


static func add(waste: Dictionary, material: int, cells: int) -> void:
	if material > 0 and cells > 0:
		waste[material] = int(waste.get(material, 0)) + cells


## A finished recipe's waste into `waste`.
static func add_recipe(waste: Dictionary, r: Dictionary) -> void:
	for w in r.get("waste", []):
		add(waste, GameMaterials.id_of(String(w.get("material", ""))), int(w.get("cells", 0)))


static func total(waste: Dictionary) -> int:
	var n := 0
	for m in waste:
		n += int(waste[m])
	return n


static func blocked(waste: Dictionary) -> bool:
	return total(waste) >= Settings.waste_port_cells


## "Waste port blocked: slag, smoke (...)" for a look-at panel, or "".
static func status(waste: Dictionary) -> String:
	if not blocked(waste):
		return ""
	var names := []
	for m in waste:
		names.append(GameMaterials.name_of(int(m)).replace("_", " "))
	return "Waste port blocked: %s. Clear the back, pipe it away or stand a chimney on top" % ", ".join(names)


## The item def's firebox (boilers, the blast furnace), {} for none.
static func firebox(items: ItemRegistry, item_id: String) -> Dictionary:
	if not _firebox.has(item_id):
		_firebox[item_id] = items.get_def(item_id).get("firebox", {})
	return _firebox[item_id]


static func is_gas(m: int) -> bool:
	return m > 0 and m <= GameMaterials.defs().size() and GameMaterials.defs()[m - 1].get("cell_type", "solid") == "gas"


## The cells a port lets waste into, nearest first. Gas: a 2 x 2 over the middle of the top, two cells up. Else the
## back face (`dir`): a 2 x 2 at floor level across the middle of it, two cells out.
static func port_cells(pos: Vector3i, w: int, h: int, dir: Vector3i, gas: bool) -> Array[Vector3i]:
	var out: Array[Vector3i] = []
	if gas:
		for k in 2:
			for a: int in [-1, 0]:
				for b: int in [-1, 0]:
					out.append(pos + Vector3i(w / 2 + a, h + k, w / 2 + b))
		return out
	var side := Vector3i(absi(dir.z), 0, absi(dir.x))
	for k in 2:
		for y in 2:
			for a: int in [-1, 0]:
				var c := pos + Vector3i(w / 2, y, w / 2) + side * a
				if dir.x != 0:
					c.x = pos.x + w + k if dir.x > 0 else pos.x - 1 - k
				if dir.z != 0:
					c.z = pos.z + w + k if dir.z > 0 else pos.z - 1 - k
				out.append(c)
	return out


## Where the smoke of a piece with its top at `top` leaves: over the top of a chimney stack standing on the middle of it
## (a 0.5 m block of chimney, one or many), else `fallback`. get_voxel: world voxel -> material.
static func vent_cells(get_voxel: Callable, top: Array[Vector3i], fallback: Array[Vector3i]) -> Array[Vector3i]:
	var B := Settings.BLOCK
	for c: Vector3i in top.slice(0, 4):
		if int(get_voxel.call(c)) != GameMaterials.CHIMNEY:
			continue
		var y: int = c.y
		var limit: int = c.y + int(Settings.m2v(Settings.chimney_max_m))
		while y < limit and int(get_voxel.call(Vector3i(c.x, y + 1, c.z))) == GameMaterials.CHIMNEY:
			y += 1
		var bx := floori(float(c.x) / B) * B
		var bz := floori(float(c.z) / B) * B
		var out: Array[Vector3i] = []
		for k in 2:
			for x in [1, 2, 0, 3]:
				for z in [1, 2, 0, 3]:
					out.append(Vector3i(bx + x, y + 1 + k, bz + z))
		return out
	return fallback


static func _box(e: RefCounted) -> Array:   # [pos, width, height] in voxels
	return [e.pos, BlockEntities.width_of(e.kind), BlockEntities.height_of(e.kind)]


static func back_dir(e: RefCounted) -> Vector3i:
	return Transport.DIRS[posmod(int(e.get("facing")) if "facing" in e else 0, 4)]


## Lets every entity's waste out (WorkSystem, once a second). Returns cells placed (tests, debug JSON).
static func emit_all(main: Node) -> int:
	var store: BlockEntities = main.blocks
	var sources := []
	var liquid := false
	for e in store.all_entities():
		if not ("waste" in e):
			continue
		var fb := firebox(store.items, String(e.get("item")) if "item" in e and e.get("item") is String else e.kind)
		if not fb.is_empty() and e.is_lit():
			add(e.waste, GameMaterials.SMOKE, int(fb.get("smoke_s", 0)))
		if e.waste.is_empty():
			continue
		sources.append(e)
		for m in e.waste:
			liquid = liquid or GameMaterials.is_liquid(int(m))
	if sources.is_empty():
		return 0
	var get_voxel := func(c: Vector3i) -> int: return main.work.machine_pins.get_voxel(main, c)
	var set_voxel := func(c: Vector3i, m: int) -> void: main.work.machine_pins.set_voxel(main, c, m)
	var runs := pipe_runs(store, sources) if liquid else {}
	var placed := 0
	for e in sources:
		placed += emit(e, get_voxel, set_voxel, runs)
	return placed


## For the sources with liquid waste: entity -> {pipes: [pipe], intakes: [entity]} of the pipe run touching it (one
## native union of touching boxes over every pipe, source and intake machine, like Networks.solve).
static func pipe_runs(store: BlockEntities, sources: Array) -> Dictionary:
	var nodes := []
	for e in store.all_entities():
		if e.kind in PIPE_KINDS or e in sources or ("intake_material" in e and e.intake_material > 0):
			nodes.append(e)
	var boxes := PackedInt32Array()
	var roles := PackedInt32Array()
	var amounts := PackedFloat32Array()
	for e in nodes:
		var w := BlockEntities.width_of(e.kind)
		boxes.append_array(PackedInt32Array([e.pos.x, e.pos.y, e.pos.z, w, BlockEntities.height_of(e.kind), w]))
		roles.append(NetworkSolver.CONDUCTOR)
		amounts.append(1.0)
	if Networks.solver == null:
		Networks.solver = NetworkSolver.new()
	var r: PackedFloat32Array = Networks.solver.solve(boxes, roles, amounts)
	var comp := {}
	for i in nodes.size():
		var k := int(r[i * 2])
		if not comp.has(k):
			comp[k] = {"pipes": [], "intakes": []}
		if nodes[i].kind in PIPE_KINDS:
			comp[k]["pipes"].append(nodes[i])
		elif "intake_material" in nodes[i] and nodes[i].intake_material > 0:
			comp[k]["intakes"].append(nodes[i])
	var out := {}
	for i in nodes.size():
		if nodes[i] in sources:
			out[nodes[i]] = comp[int(r[i * 2])]
	return out


## One entity's waste out of its ports this second. Returns cells placed.
static func emit(e: RefCounted, get_voxel: Callable, set_voxel: Callable, runs: Dictionary) -> int:
	var b := _box(e)
	var placed := 0
	for m in e.waste.keys():
		var budget := mini(int(e.waste[m]), Settings.waste_cells_per_s)
		var gone := 0
		var cells: Array[Vector3i]
		if is_gas(m):
			var top := port_cells(b[0], b[1], b[2], Vector3i.ZERO, true)
			cells = vent_cells(get_voxel, top, top)
		else:
			cells = port_cells(b[0], b[1], b[2], back_dir(e), false)
			if GameMaterials.is_liquid(m) and runs.has(e) and not runs[e]["pipes"].is_empty():
				gone = _to_intakes(e, m, budget, runs[e]["intakes"])
				cells = _pipe_outlet(e, runs[e]["pipes"])
		for c in cells:
			if gone >= budget:
				break
			if int(get_voxel.call(c)) == GameMaterials.AIR:
				set_voxel.call(c, m)
				gone += 1
				placed += 1
		var left := int(e.waste[m]) - gone
		if left <= 0:
			e.waste.erase(m)
		else:
			e.waste[m] = left
	return placed


## Moves up to `n` cells of liquid `m` into the machines on the run that take it. Returns cells moved.
static func _to_intakes(from: RefCounted, m: int, n: int, intakes: Array) -> int:
	var moved := 0
	for t in intakes:
		if t == from or t.intake_material != m:
			continue
		var k := mini(n - moved, t.intake_room())
		if k > 0:
			t.intake += k
			moved += k
			t.changed.emit()
	return moved


## The cells past the far end of the pipe run (the pipe farthest from `e`, its end away from `e`).
static func _pipe_outlet(e: RefCounted, pipes: Array) -> Array[Vector3i]:
	var centre := Vector3(e.pos) + Vector3.ONE * BlockEntities.width_of(e.kind) * 0.5
	var far: RefCounted = pipes[0]
	for p in pipes:
		if Vector3(p.pos).distance_squared_to(centre) > Vector3(far.pos).distance_squared_to(centre):
			far = p
	var along := Vector3i(1, 0, 0) if int(far.axis) == 0 else Vector3i(0, 0, 1)
	var d := Vector3(far.pos) - centre
	if (d.x if int(far.axis) == 0 else d.z) < 0.0:
		along = -along
	return port_cells(far.pos, BlockEntities.width_of(far.kind), BlockEntities.height_of(far.kind), along, false)
