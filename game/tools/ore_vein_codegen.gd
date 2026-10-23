extends SceneTree

## Writes prototypes/p5-sim-spike/src/ore_veins.gen.h from data/ore_veins.json.
##   godot_console.exe --headless --path game --script res://tools/ore_vein_codegen.gd
## Exit code 1 when the data has errors (printed).


func _init() -> void:
	var reg := OreVeinRegistry.load_default()
	if not reg.errors.is_empty():
		for e in reg.errors:
			printerr(e)
		quit(1)
		return
	var text := OreVeinCodegen.generate(reg)
	var path := ProjectSettings.globalize_path("res://").path_join("..").path_join(OreVeinCodegen.HEADER_PATH)
	var f := FileAccess.open(path, FileAccess.WRITE)
	if f == null:
		printerr("cannot write %s" % path)
		quit(1)
		return
	f.store_string(text)
	f.close()
	print(JSON.stringify({"test": "ore_vein_codegen", "veins": reg.count(), "header": path, "bytes": text.length()}))
	quit(0)
