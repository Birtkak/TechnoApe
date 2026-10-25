extends Node

## Dumps the wild animals' model cells (WildModels, ADR 0058) as JSON for an offline sheet, headless:
##   godot_console.exe --headless --path game res://tools/mob_cells_dump.tscn -- out=<file.json>
## Per species: parts placed as MobModel lays them out (cells, size, origin and position in cells) and
## the palette, so tools can draw a sheet without a window.


func _ready() -> void:
	var out_path := "user://mob_cells.json"
	for a in OS.get_cmdline_user_args():
		if a.begins_with("out="):
			out_path = a.substr(4)
	var res := {"palette": MobModel.PALETTE, "grid": MobModel.GRID, "species": {}}
	for sp in WildModels.LAYOUT:
		var parts := []
		for e in WildModels.LAYOUT[sp]:
			var spec: Array = WildModels.build(sp, e[0])
			var o: Vector3 = spec[2] * MobModel.GRID
			parts.append({"part": e[0], "role": e[1], "pos": [e[2].x, e[2].y, e[2].z], "origin": [o.x, o.y, o.z],
				"size": [spec[1].x, spec[1].y, spec[1].z], "cells": Array(spec[0])})
		res["species"][sp] = parts
	var f := FileAccess.open(out_path, FileAccess.WRITE)
	f.store_string(JSON.stringify(res))
	f.close()
	print(JSON.stringify({"test": "mob_cells_dump", "species": res["species"].size(), "out": ProjectSettings.globalize_path(out_path)}))
	get_tree().quit(0)
