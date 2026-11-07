class_name ContraptionEntity
extends RefCounted

## The drive of a moving contraption (ADR 0160): a lift winch, a drawbridge hinge or a crane. The part it
## moves is what the player builds against it, plain voxels while it rests; `Contraptions` cuts it out as a
## driven rigid piece when it moves and pastes it back at the end. A right click flips the lever (which end it
## heads for); it moves while its `load` gets power on the LV network (a hand crank, a battery, any source,
## through a knife switch), and draws only while it has somewhere to go. Saved: facing, lever, pose; the part
## itself is in the world's voxels, so a blueprint that copies the voxels and this entity copies the contraption.

signal changed

const KINDS := ["lift_winch", "drawbridge_hinge", "crane"]
const PAINT := {"lift_winch": WorkPalette.IRON_DARK, "drawbridge_hinge": WorkPalette.BRONZE, "crane": WorkPalette.STEEL}

var kind: String
var pos: Vector3i
var facing := 0             # quarter turns (Transport.dir_of): the side the part is built on
var lever := false          # false: home (lift down, bridge lowered, crane arm in); true: the other end
var pose := 0.0             # lift: voxels above home; bridge, crane: degrees from home
var arrived := true         # at the end the lever names (or stuck): wants no power
var load: PowerLoad
var status := ""            # why it last stopped or would not start
# the moving part (Contraptions; not saved, it is settled back into voxels before a save on exit)
var piece := 0              # driven rigid piece id, 0 at rest
var base := Transform3D()   # that piece's grid -> the home frame
var size := Vector3i.ZERO   # its grid
var cargo := 0              # crane: the piece on the hook
var cargo_base := Transform3D()
var cargo_size := Vector3i.ZERO
var cargo_up := 0.0         # crane: voxels the cargo is above where the hook took it
var stage := ""             # crane: "", "raise", "swing", "lower"
var tip_y := -INF            # crane: the jib's underside at its tip (home frame), found when it is cut out


func _init(p_items: ItemRegistry, p_pos: Vector3i, p_kind: String) -> void:
	pos = p_pos
	kind = p_kind
	load = PowerLoad.new(float(p_items.get_def(kind).get("load", {}).get("watts", 40.0)))


func net(channel: String) -> Dictionary:
	return load.net(not arrived) if channel == "energy" else {}


func net_in(channel: String, d) -> bool:
	if channel != "energy":
		return false
	var was := load.on
	var started := load.net_in(d)
	if load.on != was:
		changed.emit()
	return started


func tick() -> bool:
	return false


func is_lit() -> bool:
	return load.on


func moving() -> bool:
	return piece != 0


## A right click: flip the lever. It sets off once the network gives it power.
func use(_stack: Dictionary) -> Dictionary:
	lever = not lever
	arrived = false
	status = ""
	changed.emit()
	var where: String = {"lift_winch": ["down", "up"], "drawbridge_hinge": ["down", "up"], "crane": ["in", "out"]}[kind][int(lever)]
	return {"consume": 0, "give": [], "msg": "Lever %s (%s). It moves on %d W: a hand crank, a battery or a generator by cable." % [where, load.status(), int(load.watts)]}


func contents() -> Array:
	return [{"id": kind, "count": 1}]


func look_lines() -> Array[String]:
	var out: Array[String] = ["Lever %s, %s" % ["on" if lever else "off", "moving" if moving() else ("at its end" if arrived else "waiting for power")], load.status()]
	if status != "":
		out.append(status)
	return out


func view_parts() -> Dictionary:
	return MachineEntity.turned(MachineEntity.box_model(int(PAINT.get(kind, WorkPalette.IRON_DARK)), load.on), facing)


func to_dict() -> Dictionary:
	return {"kind": kind, "pos": [pos.x, pos.y, pos.z], "facing": facing, "lever": lever, "pose": pose, "arrived": arrived}


func load_from(d: Dictionary) -> void:
	facing = int(d.get("facing", 0))
	lever = bool(d.get("lever", false))
	pose = float(d.get("pose", 0.0))
	arrived = bool(d.get("arrived", true))


func summary() -> Dictionary:
	return {"kind": kind, "lever": lever, "pose": snappedf(pose, 0.1), "moving": moving(), "arrived": arrived, "power": load.on, "status": status}
