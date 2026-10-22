extends SceneTree

## Settle probe: does freshly generated terrain stay still under the material
## simulation? Generates an 8x8x8-chunk region around a point (or the nearest
## lake / ruin), loads it into a SimWorld, ticks, and reports what moved and why
## (material, position, neighbours before the tick). Prints JSON.
##   godot_console.exe --headless --path game --script res://tools/settle_probe.gd -- target=lake ticks=60
##   ... -- x=196 y=272 z=-466

var target := ""
var cx := 0
var cy := 0
var cz := 0
var ticks := 60
const N := 8  # chunks per axis


func _init() -> void:
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		if kv.size() != 2:
			continue
		match kv[0]:
			"target": target = kv[1]
			"x": cx = int(kv[1])
			"y": cy = int(kv[1])
			"z": cz = int(kv[1])
			"ticks": ticks = int(kv[1])
	var gen := WorldGen.new()
	gen.configure(1337, 256, 768)  # defaults; autoloads are not available in --script mode
	if target != "":
		var t: Vector3i = gen.find_nearest(target, 0, 0, 4096)
		cx = t.x
		cy = t.y
		cz = t.z
	elif cy == 0:
		cy = gen.height_at(cx, cz)
	# region origin in chunks, centred on (cx, cy, cz)
	var oc := Vector3i(floori(cx / 16.0) - N / 2, floori(cy / 16.0) - N / 2, floori(cz / 16.0) - N / 2)
	var sim := SimWorld.new()
	sim.configure(Vector3i(N, N, N), 6, 1)
	sim.set_materials(GameMaterials.defs())
	sim.set_reactions(GameMaterials.reactions())
	var before := {}
	for z in N:
		for y in N:
			for x in N:
				var origin := (oc + Vector3i(x, y, z)) * 16
				var bytes: PackedByteArray = gen.generate_block(origin, Vector3i(16, 16, 16), 0, 2)
				if bytes.is_empty():
					continue
				sim.set_chunk_bytes(Vector3i(x, y, z), bytes, 2)
				before[Vector3i(x, y, z)] = bytes
	var hist_before: PackedInt32Array = sim.material_histogram()
	var r: Dictionary = sim.tick(ticks)
	var hist_after: PackedInt32Array = sim.material_histogram()
	# Find changed cells and describe the first few.
	var examples := []
	var changed_total := 0
	var changed_by_material := {}
	for lc in before:
		var b: PackedByteArray = before[lc]
		var a: PackedByteArray = sim.get_chunk_bytes(lc, 2)
		for i in 4096:
			var mb := b[i * 2] | (b[i * 2 + 1] << 8)
			var ma := a[i * 2] | (a[i * 2 + 1] << 8)
			if mb == ma:
				continue
			changed_total += 1
			var nm := GameMaterials.name_of(mb)
			changed_by_material[nm] = changed_by_material.get(nm, 0) + 1
			if examples.size() < 8 and mb != 0:
				# VoxelBuffer order: y fastest, then x, then z
				var ly := i % 16
				var lx := (i / 16) % 16
				var lz := i / 256
				var lcv: Vector3i = lc
				var wp: Vector3i = (oc + lcv) * 16 + Vector3i(lx, ly, lz)
				examples.append({"pos": wp, "was": nm, "now": GameMaterials.name_of(ma),
					"below": GameMaterials.name_of(_mat_at(before, oc, wp + Vector3i(0, -1, 0))),
					"x+": GameMaterials.name_of(_mat_at(before, oc, wp + Vector3i(1, 0, 0))),
					"x-": GameMaterials.name_of(_mat_at(before, oc, wp + Vector3i(-1, 0, 0))),
					"z+": GameMaterials.name_of(_mat_at(before, oc, wp + Vector3i(0, 0, 1))),
					"z-": GameMaterials.name_of(_mat_at(before, oc, wp + Vector3i(0, 0, -1)))})
	print(JSON.stringify({"test": "settle_probe", "target": target, "centre": Vector3i(cx, cy, cz), "ticks": ticks,
		"moved_cells_total": r["moved_cells_total"], "changed_cells": changed_total,
		"changed_by_material": changed_by_material, "examples": examples}))
	quit(0)


func _mat_at(before: Dictionary, oc: Vector3i, wp: Vector3i) -> int:
	var lc := Vector3i(floori(wp.x / 16.0), floori(wp.y / 16.0), floori(wp.z / 16.0)) - oc
	if not before.has(lc):
		return 0
	var l := wp - (oc + lc) * 16
	var i := l.y + 16 * (l.x + 16 * l.z)
	var b: PackedByteArray = before[lc]
	return b[i * 2] | (b[i * 2 + 1] << 8)
