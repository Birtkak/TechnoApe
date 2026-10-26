class_name RuinLoot
extends RefCounted

## Pieces that shatter (ADR 0057): a material with "shatter": "<table>" breaks at any hit, digging or a
## held right click alike, with a burst of shards, and drops a roll of that loot table (data/loot.json)
## instead of itself. Deterministic: the roll's seed is the world seed and the piece's lowest voxel.

static var _tables := {}


static func tables() -> Dictionary:
	if _tables.is_empty():
		var d = JSON.parse_string(FileAccess.get_file_as_string("res://data/loot.json"))
		_tables = d.get("tables", {}) if d is Dictionary else {}
	return _tables


## The loot table a material shatters into, or "".
static func table_of(material: int) -> String:
	if material <= 0 or material > GameMaterials.defs().size():
		return ""
	return String(GameMaterials.defs()[material - 1].get("shatter", ""))


## The table a pot at `cell` rolls: a ruin pot rolls its ruin kind's own table (data/ruins.json "loot",
## ADR 0062); anything else, or a pot outside every ruin (one the player placed), keeps `table`.
## `gen` = the FineGen (anything with ruin_at(x, z)), null = no lookup.
static func table_at(table: String, gen, cell: Vector3i) -> String:
	if table != "ruin_pot" or gen == null or not gen.has_method("ruin_at"):
		return table
	var r: Dictionary = gen.ruin_at(cell.x, cell.z)
	if r.is_empty():
		return table
	var t := RuinTable.get_default().loot_of(String(r.get("kind", "")))
	return t if t != "" and tables().has(t) else table


## Stacks from `table` for the piece whose lowest voxel is `key`, depth_m below the surface.
static func roll(table: String, world_seed: int, key: Vector3i, depth_m: float, items: ItemRegistry) -> Array:
	var t: Dictionary = tables().get(table, {})
	if t.has("deeper") and depth_m >= float(t["deeper"].get("depth_m", 1e9)):
		t = tables().get(String(t["deeper"]["table"]), t)
	var out := []
	if t.is_empty():
		return out
	var rng := RandomNumberGenerator.new()
	rng.seed = hash([world_seed, key.x, key.y, key.z, 0x7007])
	if rng.randf() < float(t.get("empty_chance", 0.0)):
		return out
	var entries: Array = t.get("entries", [])
	var total := 0
	for e in entries:
		total += int(e.get("weight", 1))
	var r: Array = t.get("rolls", [1, 1])
	for _i in rng.randi_range(int(r[0]), int(r[1])):
		var pick := rng.randi_range(0, total - 1)
		for e in entries:
			pick -= int(e.get("weight", 1))
			if pick >= 0:
				continue
			var c = e.get("count", 1)
			var n := rng.randi_range(int(c[0]), int(c[1])) if c is Array else int(c)
			var data := {}
			if e.has("wear") and items != null and items.get_def(String(e["id"])).has("durability"):
				var dur := int(items.get_def(String(e["id"]))["durability"])
				data["wear"] = int(dur * rng.randf_range(float(e["wear"][0]), float(e["wear"][1])))
			out.append(Inventory.make_stack(items, String(e["id"]), n, data) if items != null else {"id": e["id"], "count": n})
			break
	return out


## Main: `cells` are about to be dug or popped. Shatters every shatter piece among them: shards, and
## its loot where it stood (when the game mode drops items). Returns the cells it shattered.
static func shatter(main: Node, cells: Array) -> Dictionary:
	var done := {}
	for c in cells:
		var cell: Vector3i = c
		if done.has(cell):
			continue
		var m: int = main.get_world_voxel(cell)
		var table := table_of(m)
		if table == "":
			continue
		var piece := GroundPickups.piece(main.get_world_voxel, cell)
		var key := cell
		var centre := Vector3.ZERO
		for p in piece:
			done[p] = true
			centre += Vector3(p) + Vector3(0.5, 0.5, 0.5)
			if p.y < key.y or (p.y == key.y and (p.x < key.x or (p.x == key.x and p.z < key.z))):
				key = p
		centre /= maxf(1.0, piece.size())
		for p in piece:
			main.set_world_voxel(p, GameMaterials.AIR)
		if main.fx:
			for k in 3:
				main.fx.block_break(piece[k * piece.size() / 3], m)   # a burst of shards
		var snd := main.get_node_or_null("/root/Sound")
		if snd:
			snd.play("block/pot_shatter", centre)   # its own sound: the material's group is "none"
		if not GameMode.rule(main.player.game_mode, "drops"):
			continue
		var depth_m := Settings.v2m(maxf(0.0, main.world.surface_height(key.x, key.z) - key.y))
		var gen = main.world.generator.gen if main.world and main.world.generator else null
		for s in roll(table_at(table, gen, key), Settings.world_seed, key, depth_m, main.items):
			main.spawn_item(s, centre, Vector3(randf_range(-1.0, 1.0), 2.5, randf_range(-1.0, 1.0)))
	return done
