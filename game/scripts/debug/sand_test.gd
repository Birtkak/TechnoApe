class_name SandTest
extends RefCounted

## Debug (sand_test=1): still ocean water reacts to placed sand (bug report 2026-11-03_16-37-26, "water no
## update"). Hovers over the nearest ocean, sets a 1 m ball of the sand a player places (sand_static,
## GameMaterials.still_of) into the water at its surface (the /blast ball, SimBubble.blast) and watches it:
## within WATCH_S half of it must sink at least 1 m under where it was set, and the simulation must have woken.
## A ball of live sand beside it is watched the same way (JSON "live"), to tell a wake-up problem from a water rule.
## One JSON line "debug sand {...}" (ok true); sand_before/after.png in the run's out dir unless water_shots=0.

const WATCH_S := 15
const BALL_M := 1.0

var main: Node
var r := {}


func run(p_main: Node) -> void:
	main = p_main
	var tree: SceneTree = main.get_tree()
	await tree.create_timer(1.0).timeout
	main.player.autopilot_action = "look"
	main.player.autopilot_speed_m = 0.0
	main.chat.send("/time set noon")
	main.chat.send("/gamemode creative")
	main.player.flying = true
	var me := Vector3i(main.player.position.floor())
	var sea: Vector3i = main.world.generator.gen.find_nearest("deep_ocean", me.x, me.z, int(Settings.m2v(4000.0)))
	if sea.y < 0:   # the shore of a plain ocean can be under a metre deep
		sea = main.world.generator.gen.find_nearest("ocean", me.x, me.z, int(Settings.m2v(4000.0)))
	if sea.y < 0:
		_done("no ocean within 4 km")
		return
	main.chat.send("/tp %d %d %d" % [sea.x, Settings.SEA_LEVEL_VOXEL + 24, sea.z])
	await tree.create_timer(8.0).timeout
	var top := -1
	for y in range(Settings.SEA_LEVEL_VOXEL + 8, sea.y - 1, -1):
		if _water(Vector3i(sea.x, y, sea.z)):
			top = y
			break
	r["at"] = sea
	r["surface_y"] = top
	r["depth_m"] = Settings.v2m(top - sea.y)
	if top < 0 or top - sea.y < int(Settings.m2v(4.0)):
		_done("no ocean water under the player, or under 4 m deep")
		return
	var rad := int(Settings.m2v(BALL_M))
	var cam_at := Vector3(sea.x + 20, top + 12, sea.z + 20)
	if DebugCapture.water_shots:
		await _shot(tree, "sand_before", cam_at, Vector3(sea.x, top, sea.z))
	# the sand a player places (sand_static) is the check; a ball of live sand 4 m aside tells a wake-up
	# problem from a water rule problem
	r["placed"] = await _drop(tree, GameMaterials.still_of(GameMaterials.SAND), sea.x, top, sea.z, rad)
	r["live"] = await _drop(tree, GameMaterials.SAND, sea.x + 4 * rad, top, sea.z, rad)
	r["sim_active_chunks"] = main.bubble.active_chunks
	if DebugCapture.water_shots:
		await _shot(tree, "sand_after", cam_at, Vector3(sea.x, top, sea.z))
	r["ok"] = r["placed"]["ok"]
	_done("" if r["ok"] else "the still water did not react: the placed sand did not sink")


## A ball of `material` set into the water with its top at the surface, watched for WATCH_S: ok when half of it
## sank at least 1 m under where it was set.
func _drop(tree: SceneTree, material: int, x: int, top: int, z: int, rad: int) -> Dictionary:
	var centre := Vector3(x, top - rad, z) + Vector3(0.5, 0.5, 0.5)
	var floor0 := top - 2 * rad   # the lowest sand as placed
	var d := {"material": GameMaterials.name_of(material), "cells": main.bubble.blast(centre, float(rad), material)}
	var t0 := Time.get_ticks_msec()
	var sunk := 0
	var sand := 0
	var lowest := top + 4
	var woke := false
	for s in WATCH_S:
		await tree.create_timer(1.0).timeout
		woke = woke or main.bubble.active_chunks > 0
		sunk = 0
		sand = 0
		lowest = top + 4
		for dz in range(-rad - 4, rad + 5):
			for dx in range(-rad - 4, rad + 5):
				for y in range(top + 4, floor0 - 64, -1):
					var m: int = main.world.get_voxel(Vector3i(x + dx, y, z + dz))
					if m != GameMaterials.SAND and m != GameMaterials.SAND_STATIC:
						continue
					sand += 1
					lowest = mini(lowest, y)
					if y <= floor0 - int(Settings.m2v(1.0)):
						sunk += 1
		if sunk >= d["cells"] / 2:
			break
	d["sand_left"] = sand
	d["sunk_1m"] = sunk
	d["sunk_m"] = Settings.v2m(floor0 - lowest)
	d["sim_woke"] = woke
	d["waited_ms"] = Time.get_ticks_msec() - t0
	d["ok"] = sunk >= d["cells"] / 2 and woke
	return d


func _water(c: Vector3i) -> bool:
	var m: int = main.world.get_voxel(c)
	return m == GameMaterials.WATER or m == GameMaterials.WATER_STATIC


## A frame from a free camera at `at` looking at `look` (the command only takes when it is off first).
func _shot(tree: SceneTree, name: String, at: Vector3, look: Vector3) -> void:
	var spot := "%d %d %d %d %d %d" % [at.x, at.y, at.z, look.x, look.y, look.z]
	main.chat.send("/cam off")
	main.chat.send("/cam " + spot)
	await tree.create_timer(0.5).timeout
	main.chat.send("/cam off")
	main.chat.send("/cam " + spot)
	await tree.create_timer(1.5).timeout
	main.chat.send("/shot " + name)
	await tree.create_timer(0.5).timeout
	main.chat.send("/cam off")


func _done(why: String) -> void:
	if why != "":
		r["error"] = why
		r["ok"] = false
	print("debug sand ", JSON.stringify(r))
