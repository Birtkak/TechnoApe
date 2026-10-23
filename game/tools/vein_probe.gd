extends SceneTree

## Lists the ore veins around a point as JSON (ADR 0019): type, centre, depth of
## the vein top below the ground, and how many ore-bearing stones lie above it.
##   godot_console.exe --headless --path game --script res://tools/vein_probe.gd -- x=0 z=0 cells=6 seed=1337

const S := preload("res://scripts/settings.gd")


func _init() -> void:
	var args := {}
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		if kv.size() == 2:
			args[kv[0]] = kv[1]
	var g := WorldGen.new()
	g.configure(int(args.get("seed", "1337")), S.SEA_LEVEL_VOXEL, S.WORLD_HEIGHT_VOXELS)
	var reg := OreVeinRegistry.load_default()
	var c := int(reg.grid["cell_voxels"])
	var x0 := int(args.get("x", "0"))
	var z0 := int(args.get("z", "0"))
	var n := int(args.get("cells", "6"))
	var veins := []
	var counts := {}
	for cz in range(-n, n):
		for cx in range(-n, n):
			var v: Dictionary = g.vein_at(x0 + cx * c, z0 + cz * c)
			if v.is_empty():
				continue
			var h := g.height_at(int(v["x"]), int(v["z"]))
			var e := {"name": v["name"], "x": v["x"], "y": v["y"], "z": v["z"], "ground": h, "depth": h - int(v["y"]) - int(v["thickness"]) / 2}
			if int(v["indicator"]) > 0 and int(e["depth"]) < int(reg.indicators["max_depth_voxels"]):
				e["indicators"] = _indicators(g, v, int(reg.indicators["radius_voxels"]))
			veins.append(e)
			counts[v["name"]] = int(counts.get(v["name"], 0)) + 1
	print(JSON.stringify({"test": "vein_probe", "cells": 4 * n * n, "veins": veins.size(), "by_type": counts, "list": veins}))
	quit(0)


func _indicators(g: WorldGen, v: Dictionary, r: int) -> int:
	var found := 0
	var ind := int(v["indicator"])
	for bz in range(-r, r, 16):
		for bx in range(-r, r, 16):
			var x := int(v["x"]) + bx
			var z := int(v["z"]) + bz
			var h := g.height_at(x + 8, z + 8)
			var bytes: PackedByteArray = g.generate_block(Vector3i(x, h - 6, z), Vector3i(16, 12, 16), 0, 2)
			for i in range(0, bytes.size(), 2):
				if bytes.decode_u16(i) == ind:
					found += 1
	return found
