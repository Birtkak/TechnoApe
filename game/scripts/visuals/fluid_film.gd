class_name FluidFilm
extends RefCounted

## Debug (fluid_film=1, ADR 0138): how water feels. On a flattened patch beside the player, one scene after
## another, each filmed from a fixed free camera: a filmstrip (a thumbnail every `fluid_every` s) and a burst
## (consecutive frames while it moves), plus a text log of a side slice of the simulation through the scene's
## middle every strip frame (digits = eighths of moving water, W = a full cell, S = still water, # = solid).
##   fluid_scenes=pit_lake,pour_pit,cave,dam,slope,channel   fluid_s=10   fluid_every=0.33
## Out: <out>/fluid_<scene>_strip.png, _burst.png, _slice.txt; one JSON line "debug fluid_film {...}".
## The scenes live in FluidScenes, so tools/fluid_feel.gd runs them on the bare kernel.

const W := FluidScenes.W
const D := FluidScenes.D
const H := FluidScenes.H
const DEEP := FluidScenes.DEEP

var main: Node
var r := {}
var _o: Vector3i   # the patch corner at ground level: y is the first air layer
var _log := PackedStringArray()


# --- the in-game run ---------------------------------------------------------------------------------------------

func run(p_main: Node) -> void:
	main = p_main
	var tree: SceneTree = main.get_tree()
	var args := {}
	for a in OS.get_cmdline_user_args():
		if "=" in a:
			args[a.get_slice("=", 0)] = a.get_slice("=", 1)
	var scenes: PackedStringArray = String(args.get("fluid_scenes", ",".join(FluidScenes.ALL))).split(",", false)
	var secs := float(args.get("fluid_s", "10"))
	var every := float(args.get("fluid_every", "0.33"))
	await tree.create_timer(1.0).timeout
	main.player.autopilot_action = "look"
	main.player.autopilot_speed_m = 0.0
	main.chat.send("/gamemode creative")
	main.chat.send("/time set noon")
	main.chat.send("/daycycle off")
	main.chat.send("/weather clear 600")
	main.player.flying = true
	var me := Vector3i(main.player.position.floor())
	_o = me + Vector3i(-W / 2, 0, 12)
	_o.y = _ground(me) + 1
	# the player hovers over the patch's middle (the sim ticks every tick within sim_lod_m of it), out of the shots
	main.player.position = Vector3(_o) + Vector3(W * 0.5, H + 8, D + 8)
	main.toggle_hud()
	main.chat.visible = false
	await tree.create_timer(1.0).timeout
	for s in scenes:
		r[s] = await _scene(tree, s, secs, every)
	print("debug fluid_film ", JSON.stringify(r))


func _scene(tree: SceneTree, name: String, secs: float, every: float) -> Dictionary:
	_log = PackedStringArray()
	_flatten()
	FluidScenes.build(name, _box)
	var cam: Array = FluidScenes.camera(name)
	var at: Vector3 = Vector3(_o) + cam[0]
	var look: Vector3 = Vector3(_o) + cam[1]
	main.chat.send("/cam %.1f %.1f %.1f %.1f %.1f %.1f" % [at.x, at.y, at.z, look.x, look.y, look.z])
	await tree.create_timer(1.5).timeout   # the scene pasted and meshed, the camera in place
	var strip := FrameSheet.new(480, 270, 4)
	var burst := FrameSheet.new(480, 270, 4)
	var vp: Viewport = main.get_viewport()
	var t0 := Time.get_ticks_msec()
	var next := 0.0
	var done := {}
	var out := {"start_mass": _mass()}
	var burst_at: float = {"pit_lake": 1.6, "pour_pit": 1.2, "cave": 1.5, "dam": 1.3, "slope": 1.5, "channel": 2.0}.get(name, 1.0)
	var t := 0.0
	while t < secs:
		await tree.process_frame
		t = (Time.get_ticks_msec() - t0) / 1000.0
		FluidScenes.act(name, t, done, _box)
		if t >= burst_at and burst.count() < 36:
			burst.add(vp)
		if t >= next:
			next += every
			strip.add(vp)
			_log.append("t=%.2f" % t)
			_log.append_array(FluidScenes.slice(_cell))
	var dir: String = DebugCapture.out_dir
	DirAccess.make_dir_recursive_absolute(ProjectSettings.globalize_path(dir))
	strip.save(dir + "/fluid_%s_strip.png" % name)
	burst.save(dir + "/fluid_%s_burst.png" % name)
	FileAccess.open(dir + "/fluid_%s_slice.txt" % name, FileAccess.WRITE).store_string("\n".join(_log))
	out["end_mass"] = _mass()
	out["active_chunks"] = main.bubble.stats().get("active_chunks", -1)
	main.chat.send("/cam off")
	return out


func _box(a: Vector3i, b: Vector3i, m: int) -> void:
	if main.bubble.fill_box(_o + a, _o + b, m) < 0:
		for z in range(a.z, b.z):
			for y in range(a.y, b.y):
				for x in range(a.x, b.x):
					main.set_world_voxel(_o + Vector3i(x, y, z), m)


func _cell(p: Vector3i) -> Dictionary:
	return main.bubble.cell_info(_o + p)


func _flatten() -> void:
	_box(Vector3i(-8, 0, -32), Vector3i(W + 8, H, D + 8), GameMaterials.AIR)
	_box(Vector3i(-8, -DEEP - 4, -8), Vector3i(W + 8, -DEEP, D + 8), GameMaterials.STONE)   # whatever the terrain has under it
	_box(Vector3i(-8, -DEEP, -8), Vector3i(W + 8, -1, D + 8), GameMaterials.DIRT)
	_box(Vector3i(-8, -1, -8), Vector3i(W + 8, 0, D + 8), GameMaterials.GRASS)


## Moving water in the patch, in eighths.
func _mass() -> int:
	var n := 0
	for z in range(-8, D + 8):
		for y in range(-DEEP, H):
			for x in range(-8, W + 8):
				var c: Dictionary = _cell(Vector3i(x, y, z))
				if c.get("material", 0) == GameMaterials.WATER:
					n += int(c["eighths"])
	return n


func _ground(p: Vector3i) -> int:
	for dy in range(16, -64, -1):
		if MobManager.is_solid(main.get_world_voxel(p + Vector3i(0, dy, 0))):
			return p.y + dy
	return p.y - 1
