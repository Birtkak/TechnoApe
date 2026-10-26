extends SceneTree

## Brush costs (ADR 0065) without a world: GDScript cells for survival sizes, ShapeTool cells and the
## preview mesh for every shape and size. Run:
##   Godot_..._console.exe --headless --path game --script res://tools/brush_bench.gd


func _init() -> void:
	var bt := BrushTool.new()
	var st := ShapeTool.new()
	var out := {}
	for shape in Brush.SHAPES:
		for size in Brush.CREATIVE_SIZES:
			var hit := Vector3i(101, 57, -33)
			var front := hit + Vector3i.UP
			var t0 := Time.get_ticks_usec()
			var c := bt.rel_cells(hit, front, size, shape, false)   # the cached build (once per shape and size)
			var t1 := Time.get_ticks_usec()
			var r := Brush.region(hit, front, size, shape, false)
			var arr: Array = st.mesh(c, Vector3i.ZERO, r["size"])   # a full-resolution shell (the preview halves big ones)
			var t2 := Time.get_ticks_usec()
			var key := "%s %d" % [shape, size]
			out[key] = {"cells": c.size() / 3, "cells_ms": snappedf((t1 - t0) / 1000.0, 0.01), "mesh_ms": snappedf((t2 - t1) / 1000.0, 0.01),
				"verts": (arr[Mesh.ARRAY_VERTEX] as PackedVector3Array).size() if not arr.is_empty() else 0}
	print(JSON.stringify({"brush_bench": out}))
	quit()
