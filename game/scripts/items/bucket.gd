class_name Bucket
extends RefCounted

## Buckets hold a body of liquid, not one voxel (ADR 0091): Settings.bucket_m3 (1 m^3 = 512 voxels).
## A right click with an empty bucket scoops the connected cells of the aimed liquid, up to the
## capacity; a filled bucket ("fill" in its stack data, missing = full) pours that many cells into the
## free space at the aimed face, lowest first, so it fills a pit bottom up. The cells are written as
## ordinary liquid: the simulation moves them from there. Pure logic, headless testable.

const NEIGHBOURS := [Vector3i(1, 0, 0), Vector3i(-1, 0, 0), Vector3i(0, 0, 1), Vector3i(0, 0, -1), Vector3i(0, 1, 0), Vector3i(0, -1, 0)]


## Voxels a bucket holds.
static func capacity() -> int:
	var side := Settings.m2v(pow(Settings.bucket_m3, 1.0 / 3.0))
	return roundi(side * side * side)


## How much a filled bucket stack holds (kits and creative give full ones without "fill").
static func fill_of(stack: Dictionary) -> int:
	return int(stack.get("fill", capacity()))


## The connected cells from `start` that `is_liquid(material)` accepts, at most `cap`, nearest first.
## shortcut: a partly full cell counts as a whole voxel (World.get_voxel drops the eighths); read raw
## levels if bucket trips ever need to conserve water exactly.
static func scoop_cells(get_voxel: Callable, start: Vector3i, is_liquid: Callable, cap: int) -> Array[Vector3i]:
	var out: Array[Vector3i] = []
	if not is_liquid.call(get_voxel.call(start)):
		return out
	var seen := {start: true}
	var queue: Array[Vector3i] = [start]
	var i := 0
	while i < queue.size() and out.size() < cap:
		var c := queue[i]
		i += 1
		out.append(c)
		for d in NEIGHBOURS:
			var n: Vector3i = c + d
			if not seen.has(n) and n.y >= 0 and n.y < Settings.WORLD_HEIGHT_VOXELS and is_liquid.call(get_voxel.call(n)):
				seen[n] = true
				queue.append(n)
	return out


## Up to `count` free cells reachable from `start` through free cells (`is_free(material)`), the
## lowest first and nearest among equals, never above `start`: water poured into a hole fills it from
## the bottom. The search looks at no more than 4 x count cells, so pouring in the open makes a low
## heap at the face.
static func pour_cells(get_voxel: Callable, start: Vector3i, is_free: Callable, count: int) -> Array[Vector3i]:
	var found: Array[Vector3i] = []
	if count <= 0 or not is_free.call(get_voxel.call(start)):
		return found
	var seen := {start: true}
	var queue: Array[Vector3i] = [start]
	var i := 0
	while i < queue.size() and found.size() < count * 4:
		var c := queue[i]
		i += 1
		found.append(c)
		for d in NEIGHBOURS:
			var n: Vector3i = c + d
			if not seen.has(n) and n.y >= 0 and n.y <= start.y and is_free.call(get_voxel.call(n)):   # liquid never climbs
				seen[n] = true
				queue.append(n)
	# BFS order is nearest first; a stable sort by height keeps that among cells of one height
	var order := range(found.size())
	order.sort_custom(func(a: int, b: int) -> bool: return found[a].y < found[b].y or (found[a].y == found[b].y and a < b))
	var out: Array[Vector3i] = []
	for k in mini(count, order.size()):
		out.append(found[order[k]])
	return out
