extends SceneTree

## Writes prototypes/p5-sim-spike/src/rock_table.gen.h from data/rock_types.json (ADR 0057).
##   godot_console.exe --headless --path game --script res://tools/rock_codegen.gd
## Exit code 1 when the data has errors (printed).


func _init() -> void:
	var t := RockTable.load_default()
	if not t.errors.is_empty():
		for e in t.errors:
			printerr(e)
		quit(1)
		return
	var text := t.generate()
	var path := ProjectSettings.globalize_path("res://").path_join("..").path_join(RockTable.HEADER_PATH)
	var f := FileAccess.open(path, FileAccess.WRITE)
	if f == null:
		printerr("cannot write %s" % path)
		quit(1)
		return
	f.store_string(text)
	f.close()
	print(JSON.stringify({"test": "rock_codegen", "provinces": t.provinces.size(), "clutter": t.clutter.size(), "header": path, "bytes": text.length()}))
	quit(0)
