extends Node

## Times first-use costs on the work and item paths (ADR 0023): sprite data
## load, one sprite texture, a held-item mesh, and the work piece meshes.
## Run: Godot_..._console.exe --headless --path game res://tools/work_probe.tscn

func _ready() -> void:
	var t0 := Time.get_ticks_usec()
	var sprites := ItemSprites.get_default()
	var t1 := Time.get_ticks_usec()
	sprites.texture("flint")
	var t2 := Time.get_ticks_usec()
	BlockMeshes.sprite("flint")
	var t3 := Time.get_ticks_usec()
	BlockMeshes.sprite("flint_axe_head")
	var t4 := Time.get_ticks_usec()
	print("sprite json %.2f ms, first texture %.2f, first held mesh %.2f, next held mesh %.2f" % [(t1 - t0) / 1000.0, (t2 - t1) / 1000.0, (t3 - t2) / 1000.0, (t4 - t3) / 1000.0])
	var items := ItemRegistry.load_default()
	var recipes := RecipeRegistry.load_default(items)
	var store := BlockEntities.new(items, recipes)
	WorkPalette.material()
	var k := store.create(Vector3i(0, 0, 0), "knap") as KnapEntity
	k.set_recipe(recipes.recipes["knap_flint_axe_head"])
	var c := store.create(Vector3i(1, 0, 0), "clayform") as ClayFormEntity
	c.set_recipe(recipes.recipes["form_crucible"])
	for i in 60:
		c.add((i * 2) % 8, (i / 4 * 2) % 8, 9)
	var g := store.create(Vector3i(2, 0, 0), "ground") as GroundEntity
	g.item = "raw_crucible"
	g.grass = 4
	g.sticks = 8
	store.create(Vector3i(3, 0, 0), "firepit")
	store.create(Vector3i(4, 0, 0), "sieve")
	for e in store.all_work():
		var a := Time.get_ticks_usec()
		var parts: Dictionary = e.view_parts()
		var b := Time.get_ticks_usec()
		var v := WorkPalette.make_view(e.pos, parts["solid"], parts.get("ghost", {}))
		var d := Time.get_ticks_usec()
		print("%-9s parts %.2f ms  view %.2f ms  size %s" % [e.kind, (b - a) / 1000.0, (d - b) / 1000.0, parts["solid"]["size"]])
		v.free()
	get_tree().quit(0)
