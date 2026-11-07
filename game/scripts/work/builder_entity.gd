class_name BuilderEntity
extends MachineEntity

## The builder (ADR 0161): an LV machine (its item's "machine": {"watts"}) that builds the nearest marked
## blueprint site voxel by voxel from a chest beside it. It holds only its state: power (MachineEntity's
## PowerLoad), what it placed and what the last pass found missing; Blueprints does the world work, since an
## entity has no world. It takes no process recipes, so it is no conductor of items: hoppers fill the chest.

var working := false        # it placed something this frame (the window shows lit)
var has_site := false       # a site in reach and a chest beside it: it draws power
var placed := 0             # voxels and pieces placed, ever
var missing := {}           # item id -> count the last full pass could not find in the chest
var blocked := 0            # cells the last pass found taken by something it will not clear
var status := "No blueprint site in reach"


func is_lit() -> bool:
	return powered() and working


func net(channel: String) -> Dictionary:
	if channel != "energy" or load == null:
		return {}
	return load.net(has_site)   # it draws while it has a site to work


func tick() -> bool:
	var was := is_lit()
	if power_ticks > 0:
		power_ticks -= 1
	return was != is_lit()


func takes(_id: String) -> bool:
	return false


func use(stack: Dictionary) -> Dictionary:
	return {"consume": 0, "give": [], "msg": "; ".join(look_lines()) if stack.get("id", "") == "" else "Put the materials in a chest beside it."}


func look_lines() -> Array[String]:
	var out: Array[String] = [status, "Placed %d" % placed]
	if not missing.is_empty():
		var parts := PackedStringArray()
		for id in missing:
			parts.append("%s %d" % [items.display_name(id), missing[id]])
		out.append("Missing: " + ", ".join(parts))
	if blocked > 0:
		out.append("%d cells in the way (it clears only air and plants)" % blocked)
	if load != null:
		out.append(("Running, " if powered() else "No power: ") + load.status())
	return out


func contents() -> Array:
	return []


func to_dict() -> Dictionary:
	return {"kind": kind, "pos": [pos.x, pos.y, pos.z], "facing": facing, "placed": placed}


func load_from(d: Dictionary) -> void:
	facing = int(d.get("facing", 0))
	placed = int(d.get("placed", 0))


func summary() -> Dictionary:
	return {"kind": kind, "placed": placed, "missing": missing, "blocked": blocked, "status": status, "powered": powered()}
