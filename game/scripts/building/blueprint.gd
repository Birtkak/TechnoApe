class_name Blueprint
extends RefCounted

## A blueprint (ADR 0161): a copied build carried on a `written_blueprint` stack under "blueprint". The voxels
## are a prefab (PrefabStore.encode: material names and run-length runs, so the stack stays small and JSON-safe);
## the block entities (stations, machines, pipes) are a list beside it, each with the item that places it, its
## footprint box (offset and size in the blueprint) and its facing / axis / item. Entity cells are left out of
## the voxel grid: the entity puts its own voxels down. Furniture (with its facing and a sign's words) and torches
## carry their anchor ("at"): their placement works out their cells. Chest and machine contents are not copied:
## a builder refilling them from nothing would make items. Work in progress (knapping, molds) is not copied.

const MAX_CELLS := 64 * 1024   # voxels a blueprint may hold: a builder walks them in GDScript (a few hundred a frame)


## Reads the world box [lo, lo + size) into a blueprint dictionary, or {} when nothing in it can be copied.
static func capture(main: Node, lo: Vector3i, size: Vector3i) -> Dictionary:
	var st := ShapeTool.new()
	var bubble: SimBubble = main.bubble
	var g: PackedByteArray = main.builder._clean_grid(st.read(bubble.sim, lo, size, bubble.origin * Settings.CHUNK))
	var ents := []
	for e in main.blocks.all_entities():
		var rec := _entity_record(main, e)
		if rec.is_empty():
			continue
		var fp: Array = BlockEntities.footprint(e)
		var a: Vector3i = fp[0]
		var b: Vector3i = fp[0]
		for c in fp:
			a = a.min(c)
			b = b.max(c)
		if a.x < lo.x or a.y < lo.y or a.z < lo.z or b.x >= lo.x + size.x or b.y >= lo.y + size.y or b.z >= lo.z + size.z:
			continue
		for c in fp:   # the entity places its own voxels
			var q: Vector3i = c - lo
			g[q.x + size.x * (q.z + size.z * q.y)] = 0
		rec["off"] = [a.x - lo.x, a.y - lo.y, a.z - lo.z]
		rec["box"] = [b.x - a.x + 1, b.y - a.y + 1, b.z - a.z + 1]
		if e is FurnitureEntity or e is TorchEntity:
			rec["at"] = [e.pos.x - lo.x, e.pos.y - lo.y, e.pos.z - lo.z]
		ents.append(rec)
	if g.count(0) == g.size() and ents.is_empty():
		return {}
	if g.size() - g.count(0) > MAX_CELLS:
		return {"error": "Too big: a blueprint holds up to %d voxels" % MAX_CELLS}
	var d := PrefabStore.encode(size, g)
	d["entities"] = ents
	return d


## {kind, item, mat, facing?, axis?, sub?} for an entity a blueprint can copy, else {}.
static func _entity_record(main: Node, e: RefCounted) -> Dictionary:
	var items: ItemRegistry = main.items
	if e is TorchEntity:
		return {"kind": "torch", "item": "torch", "mat": GameMaterials.name_of(main.get_world_voxel(e.pos))}   # floor or wall: placing decides
	if e is FurnitureEntity:
		var fid := ""
		for id in items.all_ids():
			if String(items.get_def(id).get("furniture", "")) == e.furniture:
				fid = id
				break
		if fid == "":
			return {}
		var fr := {"kind": "furniture", "furniture": e.furniture, "item": fid, "mat": "furniture", "facing": int(e.facing)}
		if e.text != "":
			fr["text"] = e.text
		return fr
	var m: int = main.get_world_voxel(e.pos)
	if m == GameMaterials.FURNACE_LIT:
		m = GameMaterials.FURNACE   # a burning furnace is copied as a furnace
	if m <= 0:
		return {}
	var rec := {"kind": e.kind, "mat": GameMaterials.name_of(m)}
	if StationEntity.SIZES.has(e.kind) and GameMaterials.is_station(m):
		rec["item"] = items.item_for_material(GameMaterials.name_of(m))   # the chest, furnace, crafting table
	elif BlockEntities.is_work(e):
		var sub: String = String(e.item) if "item" in e and e.item is String and e.item != "" else ""
		var id := sub if sub != "" else String(e.kind)
		if String(items.get_def(id).get("work", "")) != e.kind:
			return {}   # a work piece no item places (knapping, molds, a grave)
		rec["item"] = id
		if sub != "":
			rec["sub"] = sub
	else:
		return {}
	if rec["item"] == "":
		return {}
	if "facing" in e:
		rec["facing"] = int(e.facing)
	if "axis" in e:
		rec["axis"] = int(e.axis)
	return rec


## {item id: count} the blueprint takes: voxels in voxel units, entities one each.
static func bill(d: Dictionary, items: ItemRegistry) -> Dictionary:
	var out := {}
	var g: Dictionary = PrefabStore.decode(d)
	if not g.is_empty():
		var r := ShapeTool.new().runs(g["grid"])
		for k in range(0, r.size(), 2):
			if r[k] == 0:
				continue
			var id := items.item_for_material(GameMaterials.name_of(r[k]))
			id = id if id != "" else GameMaterials.name_of(r[k])
			out[id] = int(out.get(id, 0)) + r[k + 1]
	for e in d.get("entities", []):
		out[e["item"]] = int(out.get(e["item"], 0)) + 1
	return out


static func size_of(d: Dictionary) -> Vector3i:
	var s: Array = d.get("size", [1, 1, 1])
	return Vector3i(int(s[0]), int(s[1]), int(s[2]))


## "2.0 x 1.5 x 2.0 m, 1200 voxels, 1 furnace": the line a tooltip or toast shows.
static func describe(d: Dictionary, items: ItemRegistry) -> String:
	var s := Vector3(size_of(d)) * Settings.VOXEL_M
	var parts := PackedStringArray()
	var b := bill(d, items)
	for id in b:
		parts.append("%s %d" % [items.display_name(id), b[id]])
	return "%s x %s x %s m: %s" % [snappedf(s.x, 0.01), snappedf(s.y, 0.01), snappedf(s.z, 0.01), ", ".join(parts)]


## The grid the ghost shows: the voxels plus each entity's box in its own material.
static func ghost_grid(d: Dictionary) -> PackedByteArray:
	var g: Dictionary = PrefabStore.decode(d)
	if g.is_empty():
		return PackedByteArray()
	var grid: PackedByteArray = g["grid"]
	var size := size_of(d)
	for e in d.get("entities", []):
		var m := GameMaterials.id_of(String(e["mat"]))
		var o := Vector3i(int(e["off"][0]), int(e["off"][1]), int(e["off"][2]))
		var b := Vector3i(int(e["box"][0]), int(e["box"][1]), int(e["box"][2]))
		for c in BlockEntities.box(o, b):
			if c.x < size.x and c.y < size.y and c.z < size.z:
				grid[c.x + size.x * (c.z + size.z * c.y)] = m
	return grid


## The blueprint laid at world `lo` turned `rot` (ShapeTool's 0..23; blueprints only turn about up) and
## mirrored: {cells (x, y, z triples), mats, entities: [{kind, item, mat (id), anchor, facing, axis, sub}], size}.
## Entity anchors and facings go through the same ShapeTool transform as the voxels (a marker grid).
static func layout(d: Dictionary, lo: Vector3i, rot: int, mirror: bool) -> Dictionary:
	var g: Dictionary = PrefabStore.decode(d)
	if g.is_empty():
		return {}
	var st := ShapeTool.new()
	var size: Vector3i = g["size"]
	var p := {"shape": "grid", "size": size, "rot": rot, "mirror": mirror, "grid": g["grid"]}
	var r := st.cells(p, lo)
	var out := {"cells": r.get("cells", PackedInt32Array()), "mats": r.get("mats", PackedByteArray()),
		"size": st.oriented_size(size, rot), "entities": []}
	var ents: Array = d.get("entities", [])
	if ents.is_empty():
		return out
	# where each entity's box lands: mark its cells with its index (+1) and let ShapeTool turn the marks
	var marks := PackedByteArray()
	marks.resize(size.x * size.y * size.z)
	for i in mini(ents.size(), 255):
		var o := Vector3i(int(ents[i]["off"][0]), int(ents[i]["off"][1]), int(ents[i]["off"][2]))
		var b := Vector3i(int(ents[i]["box"][0]), int(ents[i]["box"][1]), int(ents[i]["box"][2]))
		if ents[i].has("at"):   # furniture turns about its anchor block, a torch is its anchor cell
			# shortcut: an anchor block that reaches into another piece's box can take that piece's mark; fine for rooms of furniture
			o = Vector3i(int(ents[i]["at"][0]), int(ents[i]["at"][1]), int(ents[i]["at"][2]))
			b = Vector3i(Settings.BLOCK, 1, Settings.BLOCK) if ents[i]["kind"] == "furniture" else Vector3i.ONE
		for c in BlockEntities.box(o, b):
			if c.x < size.x and c.y < size.y and c.z < size.z:
				marks[c.x + size.x * (c.z + size.z * c.y)] = i + 1
	p["grid"] = marks
	var mr := st.cells(p, lo)
	var mins := {}
	var cells: PackedInt32Array = mr.get("cells", PackedInt32Array())
	var mm: PackedByteArray = mr.get("mats", PackedByteArray())
	for k in mm.size():
		var c := Vector3i(cells[3 * k], cells[3 * k + 1], cells[3 * k + 2])
		mins[mm[k]] = (mins[mm[k]] as Vector3i).min(c) if mins.has(mm[k]) else c
	var turn := _turn_of(st, rot, mirror)
	for i in mini(ents.size(), 255):
		if not mins.has(i + 1):
			continue
		var e: Dictionary = ents[i]
		var w := {"kind": e["kind"], "item": e["item"], "mat": GameMaterials.id_of(String(e["mat"])), "anchor": mins[i + 1],
			"sub": String(e.get("sub", "")), "furniture": String(e.get("furniture", "")), "text": String(e.get("text", ""))}
		if e.has("facing"):
			var dirs: Array = FurnitureEntity.DIRS if e["kind"] == "furniture" else Transport.DIRS   # furniture's 0 is +z, pieces' -z
			w["facing"] = dirs.find(_apply(turn, dirs[posmod(int(e["facing"]), 4)]))
		if e.has("axis"):
			var ax := Vector3i(1, 0, 0) if int(e["axis"]) == 0 else Vector3i(0, 0, 1)
			w["axis"] = 0 if _apply(turn, ax).x != 0 else 1
		out["entities"].append(w)
	return out


## Where local +x and +z go under rot and mirror: [x', z'] unit vectors, read off a three-cell marker grid.
static func _turn_of(st: ShapeTool, rot: int, mirror: bool) -> Array:
	var g := PackedByteArray([1, 2, 3, 0])   # 2 x 1 x 2: origin 1, +x 2, +z 3
	var r := st.cells({"shape": "grid", "size": Vector3i(2, 1, 2), "rot": rot, "mirror": mirror, "grid": g}, Vector3i.ZERO)
	var at := {}
	var cells: PackedInt32Array = r["cells"]
	for k in (r["mats"] as PackedByteArray).size():
		at[r["mats"][k]] = Vector3i(cells[3 * k], cells[3 * k + 1], cells[3 * k + 2])
	return [at[2] - at[1], at[3] - at[1]]


static func _apply(turn: Array, v: Vector3i) -> Vector3i:
	return Vector3i((turn[0] * v.x + turn[1] * v.z).sign())
