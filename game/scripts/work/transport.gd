class_name Transport
extends RefCounted

## Item transport (ADR 0100): what hoppers and belts can put into a piece and take out of one. Each
## kind of block entity has its own loading API, so this is the one place that knows them all.
## Facings: 0 = -z, then a quarter turn each (WorkPalette.rotate_y's sense), the way pieces are drawn.

const DIRS := [Vector3i(0, 0, -1), Vector3i(1, 0, 0), Vector3i(0, 0, 1), Vector3i(-1, 0, 0)]


static func dir_of(facing: int) -> Vector3i:
	return DIRS[posmod(facing, 4)] * Settings.BLOCK


## The facing a player standing at `yaw` degrees lays a piece in (the way they look).
static func facing_from_yaw(yaw_deg: float) -> int:
	return posmod(-roundi(yaw_deg / 90.0), 4)


## Puts one item of `stack` into `e`. Returns how many were taken (0 when it will not have it).
## shortcut: tanks and turbines take nothing; a tank's empty bucket would have nowhere to go.
static func give(e: RefCounted, stack: Dictionary, items: ItemRegistry) -> int:
	if e == null or stack.is_empty():
		return 0
	var one := {"id": stack["id"], "count": 1}
	for k in stack:
		if k != "id" and k != "count":
			one[k] = stack[k]
	if e is HopperEntity or e is BeltEntity:
		return 1 if e.accept(one) else 0
	if e is ChestEntity:
		return 1 - e.inv.add(one["id"], 1, one)
	if e is FirepitEntity:
		return int(e.use(one, items.get_def(one["id"]))["consume"])
	if e is SteamSieveEntity:   # the steam sieve and the ore crusher load by the block
		return int(e.use(items.get_def(one["id"]), null)["consume"])
	if e is AnvilEntity and Inventory.heat_of(one) >= Settings.smith_min_heat and not e.has_piece():
		return 0   # shortcut: a hot ingot on a hammer needs a shape picked; a player does that
	return 0


## Takes one finished item out of `e`: a firepit's output, a sieve's or crusher's finds, a hammer's
## finished piece, a chest's or hopper's first stack. {} when there is nothing.
static func pull(e: RefCounted) -> Dictionary:
	if e == null:
		return {}
	if e is SteamHammerEntity and not e.ready.is_empty():
		return e.take_piece()
	if e is SteamSieveEntity:
		if e.found.is_empty():
			return {}
		var f: Dictionary = e.found[0]
		f["count"] = int(f["count"]) - 1
		var out := {"id": f["id"], "count": 1}
		if f["count"] <= 0:
			e.found.pop_front()
		e.changed.emit()
		return out
	if e is FirepitEntity:
		return e.inv.take(FirepitEntity.OUTPUT, 1)
	if e is ChestEntity or e is HopperEntity:
		for i in e.inv.size():
			if not e.inv.is_empty_slot(i):
				return e.inv.take(i, 1)
	return {}
