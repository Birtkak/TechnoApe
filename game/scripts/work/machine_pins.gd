class_name MachinePins
extends RefCounted

## Machine pins (ADR 0113, 0117): a machine that needs the material simulation (`needs_sim()`, the pump)
## keeps a SimBubble pin alive around it while the main window does not cover it, nearest to the player
## first, at most Settings.machine_pins_max at once. Checked once a second by WorkSystem. Voxel reads and
## writes for such machines go through `get_voxel` / `set_voxel` here: the main window, then a pin, then
## the terrain.

var pins := {}   # machine pos -> SimBubble


func update(main: Node) -> void:
	var bubble: SimBubble = main.bubble
	if bubble == null:
		return
	var wanting := []
	for e in main.blocks.all_entities():
		if e.has_method("needs_sim") and e.needs_sim():
			wanting.append(e)
	var eye: Vector3 = main.player.position
	wanting.sort_custom(func(a, b) -> bool: return Vector3(a.pos).distance_squared_to(eye) < Vector3(b.pos).distance_squared_to(eye))
	var keep := {}
	for e in wanting:
		if keep.size() >= Settings.machine_pins_max:
			break
		var wc := Vector3i((Vector3(e.pos) / Settings.CHUNK).floor())
		if bubble.is_chunk_in_window(wc - bubble.origin):
			continue   # the main window simulates it
		keep[e.pos] = true
		if not pins.has(e.pos) or not is_instance_valid(pins[e.pos]):
			var p: SimBubble = bubble.pin_at(Vector3(e.pos) + Vector3.ONE * (Settings.BLOCK * 0.5))
			if p != null:
				p.keep_alive = true
				pins[e.pos] = p
	for k in pins.keys():
		if not keep.has(k):
			if is_instance_valid(pins[k]):
				pins[k].keep_alive = false   # it lets go when quiet, like any pin
			pins.erase(k)


func get_voxel(main: Node, p: Vector3i) -> int:
	var m: int = main.bubble.get_voxel(p) if main.bubble else -1
	if m >= 0:
		return m
	for k in pins:
		if is_instance_valid(pins[k]):
			m = pins[k].get_voxel(p)
			if m >= 0:
				return m
	return main.world.get_voxel(p)


func set_voxel(main: Node, p: Vector3i, m: int) -> void:
	if main.bubble and main.bubble.set_voxel(p, m):
		return
	for k in pins:
		if is_instance_valid(pins[k]) and pins[k].set_voxel(p, m):
			return
	main.world.set_voxel(p, m)


## The vent fan's push (ADR 0148) through the main window or a pin; -1 when no sim covers the box.
func blow(main: Node, lo: Vector3i, hi: Vector3i, dir: Vector3i) -> int:
	var n: int = main.bubble.blow(lo, hi, dir) if main.bubble else -1
	if n >= 0:
		return n
	for k in pins:
		if is_instance_valid(pins[k]):
			n = pins[k].blow(lo, hi, dir)
			if n >= 0:
				return n
	return -1


func summary() -> Dictionary:
	return {"pins": pins.size()}
