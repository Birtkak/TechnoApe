class_name ShellTest
extends RefCounted

## Debug (shell_test=1, ADR 0124): the voxels around the player exist underground. Teleports into the nearest
## cave room, then to the surface above it, digs a 4 m cube there, stands on its floor and sinks 16 m through the
## rock a metre at a time (a shaft dug step by step moves the streaming shell the way a teleport does not); at each spot it waits
## for the terrain to settle and counts, within RADIUS_M of the player, the data blocks godot_voxel has not
## loaded, the mesh blocks it has not drawn, and block centres that read air where the generator has a solid
## (the ADR 0124 regression: the streaming shell left the ground under a hole or a cave empty), plus the 8 m cubes
## out to the render distance that are not drawn (a cavern's far walls). bubble_verify skips unloaded chunks and
## the near sphere always loads 32 m, so neither saw this. One JSON line "debug shell {...}", ok = every count 0.

const RADIUS_M := 16.0
const SETTLE_S := 60.0
const HOLE_M := 4.0
const DESCENT_M := 16.0

var main: Node
var r := {}


func run(p_main: Node) -> void:
	main = p_main
	var tree: SceneTree = main.get_tree()
	await tree.create_timer(1.0).timeout
	main.player.autopilot_action = "look"
	main.player.autopilot_speed_m = 0.0
	main.chat.send("/gamemode creative")
	main.player.flying = true
	var me := Vector3i(main.player.position.floor())
	var cave: Vector3i = main.world.generator.gen.find_nearest("cave", me.x, me.z, int(Settings.m2v(4000.0)))
	if cave.y < 0:
		_done("no cave within 4 km")
		return
	r["cave"] = await _visit(tree, cave)
	# the surface above the cave: dig a 4 m cube from the ground down and stand on its floor
	var g: int = main.world.surface_height(cave.x, cave.z)
	var top := Vector3i(cave.x, g + 2, cave.z)
	await _visit(tree, top)
	var h := int(Settings.m2v(HOLE_M))
	var lo := Vector3i(cave.x - h / 2, g + 1 - h, cave.z - h / 2)
	main._debug_fill(lo, lo + Vector3i(h, h, h), GameMaterials.AIR)
	await tree.create_timer(1.0).timeout
	r["hole"] = await _visit(tree, Vector3i(cave.x, lo.y, cave.z), AABB(Vector3(lo), Vector3.ONE * h))
	# then down through the rock a metre at a time, as a player digging a shaft moves the shell step by step
	# (a teleport rebuilds the streaming box, a walk only diffs it)
	var step := int(Settings.m2v(1.0))
	for i in int(DESCENT_M):
		main.player.position.y -= step
		await tree.create_timer(0.5).timeout
	r["descent"] = await _visit(tree, Vector3i(main.player.position.floor()), AABB(Vector3(lo), Vector3.ONE * h))
	r["bubble_unloaded_chunks"] = main.bubble.stats().get("unloaded_chunks", -1)
	r["ok"] = _clean(r["cave"]) and _clean(r["hole"]) and _clean(r["descent"])
	_done("")


func _clean(spot: Dictionary) -> bool:
	return spot["unloaded"] == 0 and spot["unmeshed"] == 0 and spot["air_where_solid"] == 0 and spot["far_undrawn_8m"] == 0


## Teleport to `at`, wait until nothing within RADIUS_M is missing (SETTLE_S at most), report the counts.
func _visit(tree: SceneTree, at: Vector3i, skip := AABB()) -> Dictionary:
	if at != Vector3i(main.player.position.floor()):
		main.chat.send("/tp %d %d %d" % [at.x, at.y, at.z])
	var t0 := Time.get_ticks_msec()
	var c := {}
	while true:
		await tree.create_timer(0.5).timeout
		c = _count(skip)
		if _clean(c) or Time.get_ticks_msec() - t0 > SETTLE_S * 1000.0:
			break
	c["at"] = at
	c["settle_s"] = snappedf((Time.get_ticks_msec() - t0) / 1000.0, 0.1)
	c["pos"] = Vector3i(main.player.position.floor())
	return c


## Blocks within RADIUS_M of the player: data blocks not loaded, mesh blocks not drawn, and loaded block
## centres (outside `skip`, the dug hole) that read air where the generator puts a solid.
func _count(skip: AABB) -> Dictionary:
	var terrain = main.world.terrain
	var tool: VoxelTool = terrain.get_voxel_tool()
	var p: Vector3 = main.player.position
	var rad := Settings.m2v(RADIUS_M)
	var ds: int = terrain.get_data_block_size()
	var ms: int = terrain.get_mesh_block_size()
	var unloaded := 0
	var air_where_solid := 0
	var examples := []
	var n := int(ceil(rad / ds))
	var pb := Vector3i((p / ds).floor())
	for z in range(-n, n + 1):
		for y in range(-n, n + 1):
			for x in range(-n, n + 1):
				var b := pb + Vector3i(x, y, z)
				var centre := Vector3(b * ds) + Vector3.ONE * (ds * 0.5)
				if centre.distance_to(p) > rad:
					continue
				if not tool.is_area_editable(AABB(Vector3(b * ds), Vector3.ONE * ds)):
					unloaded += 1
					if examples.size() < 6:
						examples.append(["unloaded", b * ds])
					continue
				var cv := Vector3i(centre.floor())
				if skip.has_point(centre) or main.world.get_voxel(cv) != GameMaterials.AIR:
					continue
				# the whole block as godot_voxel asks for it (a 1-voxel block would sample the cave field elsewhere)
				var expect: PackedByteArray = main.world.generator.gen.generate_block(b * ds, Vector3i(ds, ds, ds), 0, 2)
				var m := 0
				if expect.size() == 2:   # uniform
					m = expect.decode_u16(0)
				elif not expect.is_empty():   # y fastest, then x, then z (as the bubble reads it)
					m = expect.decode_u16(2 * ((ds / 2) * ds * ds + (ds / 2) * ds + ds / 2))
				if m != GameMaterials.AIR and GameMaterials.blocks_bodies(m):
					air_where_solid += 1
					if examples.size() < 6:
						examples.append(["air_where_solid", cv, GameMaterials.name_of(m)])
	var unmeshed := 0
	n = int(ceil(rad / ms))
	pb = Vector3i((p / ms).floor())
	for z in range(-n, n + 1):
		for y in range(-n, n + 1):
			for x in range(-n, n + 1):
				var b := pb + Vector3i(x, y, z)
				var centre := Vector3(b * ms) + Vector3.ONE * (ms * 0.5)
				if centre.distance_to(p) > rad:
					continue
				if not DebugCapture._patch_drawn(AABB(Vector3(b * ms), Vector3.ONE * ms)):
					unmeshed += 1
					if examples.size() < 6:
						examples.append(["unmeshed", b * ms])
	# the far walls of a cavern: 8 m cubes out to the render distance (no higher than 16 m over the player,
	# the sky above the shell is never meshed) that are not drawn; what the player saw missing in bug report
	# 2026-11-03_16-37-55 stood 40-70 m away, past the near sphere, under the shell
	var far := 0
	var cube := int(Settings.m2v(8.0))
	var reach := Settings.m2v(Settings.view_distance_m) - 2 * cube   # the outermost cubes straddle the box edge
	n = int(ceil(reach / cube))
	pb = Vector3i((p / cube).floor())
	for z in range(-n, n + 1):
		for y in range(-n, n + 1):
			for x in range(-n, n + 1):
				var b := pb + Vector3i(x, y, z)
				var centre := Vector3(b * cube) + Vector3.ONE * (cube * 0.5)
				if centre.distance_to(p) > reach or centre.y > p.y + Settings.m2v(16.0):
					continue
				if not DebugCapture._patch_drawn(AABB(Vector3(b * cube), Vector3.ONE * cube)):
					far += 1
					if examples.size() < 6:
						examples.append(["far_undrawn", b * cube, snappedf(Settings.v2m(centre.distance_to(p)), 0.1)])
	return {"unloaded": unloaded, "unmeshed": unmeshed, "air_where_solid": air_where_solid, "far_undrawn_8m": far, "examples": examples}


func _done(why: String) -> void:
	if why != "":
		r["error"] = why
		r["ok"] = false
	print("debug shell ", JSON.stringify(r))
