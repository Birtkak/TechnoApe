extends SceneTree

## Writes prototypes/p5-sim-spike/src/ruin_table.gen.h from data/ruins.json (ADR 0062).
##   godot_console.exe --headless --path game --script res://tools/ruin_codegen.gd
## Exit code 1 when the data has errors (printed).


func _init() -> void:
	var t := RuinTable.load_default()
	if not t.errors.is_empty():
		for e in t.errors:
			printerr(e)
		quit(1)
		return
	var text := t.generate()
	var path := ProjectSettings.globalize_path("res://").path_join("..").path_join(RuinTable.HEADER_PATH)
	var f := FileAccess.open(path, FileAccess.WRITE)
	if f == null:
		printerr("cannot write %s" % path)
		quit(1)
		return
	f.store_string(text)
	f.close()
	print(JSON.stringify({"test": "ruin_codegen", "kinds": t.kinds.size(), "header": path, "bytes": text.length()}))
	quit(0)
