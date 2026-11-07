extends SceneTree

## Prints a server's edit journal (ADR 0145) as one JSON line per op: kind, cell count, box.
##   godot --headless --path game --script res://tools/net_journal_dump.gd -- file=<path to .ops> [all=1]


func _init() -> void:
	var path := ""
	var all := false
	for a in OS.get_cmdline_user_args():
		if a.begins_with("file="):
			path = a.substr(5)
		if a == "all=1":
			all = true
	var f := FileAccess.open_compressed(path, FileAccess.READ, FileAccess.COMPRESSION_ZSTD)
	if f == null:
		print("cannot open " + path)
		quit(1)
		return
	var ops: Array = f.get_var()
	var kinds := {}
	for op in ops:
		var k: String = op["k"]
		kinds[k] = int(kinds.get(k, 0)) + 1
		if all:
			var n: int = op["cells"].size() / 3 if op.has("cells") else -1
			print(JSON.stringify({"k": k, "cells": n, "m": op.get("m", -1), "mats": Array(op.get("mats", PackedByteArray())).slice(0, 4), "box": [str(op["box"][0]), str(op["box"][1])]}))
	print(JSON.stringify({"ops": ops.size(), "kinds": kinds}))
	quit(0)
