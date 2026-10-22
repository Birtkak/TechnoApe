extends SceneTree

## Writes prototypes/p5-sim-spike/src/biome_table.gen.h from data/biomes.json.
##   godot_console.exe --headless --path game --script res://tools/biome_codegen.gd
## Exit code 1 when the data has errors (printed).


func _init() -> void:
	var reg := BiomeRegistry.load_default()
	if not reg.errors.is_empty():
		for e in reg.errors:
			printerr(e)
		quit(1)
		return
	var text := BiomeCodegen.generate(reg)
	var path := ProjectSettings.globalize_path("res://").path_join("..").path_join(BiomeCodegen.HEADER_PATH)
	var f := FileAccess.open(path, FileAccess.WRITE)
	if f == null:
		printerr("cannot write %s" % path)
		quit(1)
		return
	f.store_string(text)
	f.close()
	print(JSON.stringify({"test": "biome_codegen", "biomes": reg.count(), "header": path, "bytes": text.length()}))
	quit(0)
