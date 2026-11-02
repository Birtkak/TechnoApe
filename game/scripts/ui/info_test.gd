class_name InfoTest
extends CraftTest

## Debug (info_test=1, ADR 0093): the information systems on CraftTest's cleared strip. The look-at
## panel on a block of iron ore and on a lit forge with an iron ingot on it, R in the world on the
## forge, U on the iron ingot (station tabs), a recipe bookmark; one contact sheet info.png in the
## run's out dir and one JSON line "debug info {...}".


func run(p_main: Node) -> void:
	main = p_main
	var tree: SceneTree = main.get_tree()
	await tree.create_timer(0.6).timeout
	main.player.autopilot_action = "look"
	main.player.script_aim = true   # the look-at panel follows the aim without a captured mouse
	var B := Settings.BLOCK
	base = BlockEntities.anchor_for(Vector3i(main.player.position.floor()))
	g0 = floori(float(main.world.surface_height(base.x, base.z)) / B) * B - 1
	var lo := Vector3i(base.x - 8 * B, g0 - 2 * B, base.z - 8 * B)
	var hi := Vector3i(base.x + 8 * B, g0 + 1, base.z + B)
	_fill(lo, Vector3i(hi.x, g0, hi.z), GameMaterials.DIRT)
	_fill(Vector3i(lo.x, g0, lo.z), Vector3i(hi.x, g0 + 1, hi.z), GameMaterials.GRASS)
	_fill(Vector3i(lo.x, g0 + 1, lo.z), Vector3i(hi.x, g0 + 1 + 7 * B, hi.z), GameMaterials.AIR)
	await tree.create_timer(1.5).timeout
	var inv: PlayerInventory = main.player_inv
	var at := func(dx: int, dz: int) -> Vector3i: return Vector3i(base.x + dx * B, g0 + 1, base.z + dz * B)
	var mid := Vector3(0.5, 0.2, 0.5)

	# ---- the look-at panel on ore ----
	var o_at: Vector3i = at.call(-3, -3)
	_fill(o_at, o_at + Vector3i(B, B, B), GameMaterials.IRON_ORE)
	inv.pickup("flint_pickaxe", 1)
	main._hold("flint_pickaxe")
	await _shot(tree, "info_ore", Vector3(o_at) + Vector3(0.5, 0.6, 0.5) * B, 1.2)

	# ---- a lit forge with an iron ingot on it ----
	inv.off.set_slot(0, {"id": "tongs", "count": 1})
	var g_at: Vector3i = at.call(3, -3)
	inv.pickup("forge", 1)
	main._hold("forge")
	_click_top(g_at - Vector3i.UP)
	await tree.create_timer(0.2).timeout
	var fg = main.blocks.at(g_at)
	r["forge_placed"] = fg is ForgeEntity
	inv.pickup("charcoal", 2)
	main._hold("charcoal")
	await _put_on(g_at, mid, 2)
	inv.pickup("iron_ingot", 1)
	main._hold("iron_ingot")
	await _put_on(g_at, mid)
	inv.pickup("firestarter", 1)
	main._hold("firestarter")
	_use_on(g_at, mid)
	await _fast_forward(int(Settings.metal_heat_s * 20.0) + 4)
	await _shot(tree, "info_forge", Vector3(g_at) + mid * B, 1.1)

	# ---- R in the world on the forge: its recipes; U on the iron ingot: station tabs ----
	var bv: RecipeBrowserView = main.inv_screen.browser_view
	main._lookup(["forge", ""], "recipes")
	r["r_view"] = main.inv_screen.browser.summary().get("view", {})
	await _ui_shot(tree, "info_r")
	main.inv_screen.browser.close()
	bv.show_item("iron_ingot", "uses")
	r["u_tabs"] = main.inv_screen.browser.view["categories"].size()
	await _ui_shot(tree, "info_u")
	main.inv_screen.browser.select_category(main.inv_screen.browser.view["categories"].size() - 1)   # a click on the last tab
	bv._build_recipe()
	await _ui_shot(tree, "info_tabs")
	if bv.bookmark_button != null:
		bv.bookmark_button.pressed.emit()   # Mark: the shown recipe goes to the bookmarks
	main.inv_screen.browser.toggle_bookmark("iron_ingot")
	bv.refresh_bookmarks()
	r["bookmarks"] = main.inv_screen.browser.bookmarks
	await _ui_shot(tree, "info_bookmarks")
	main.inv_screen.browser.close()
	bv.show_item("crushed_iron_ore", "recipes")   # a machine's recipe from data alone (ADR 0115)
	r["machine_view"] = main.inv_screen.browser.summary().get("view", {})
	await _ui_shot(tree, "info_machine")
	main.inv_screen.browser.close()
	main.inv_screen.open_book()   # the progression book's last two chapters (bug report 2026-10-28_08-29-44)
	for c in ["steam_age", "electrical_age"]:
		main.inv_screen.quest_view.open_chapter(c)
		await _ui_shot(tree, "info_book_" + c)
	main.inv_screen.close_book()
	r["sheet"] = _sheet()
	print("debug info " + JSON.stringify(r))


func _ui_shot(tree: SceneTree, name: String) -> void:
	for i in 4:
		await tree.process_frame
	await tree.create_timer(0.2).timeout
	var path: String = DebugCapture.out_dir + "/craft_%s.png" % name
	DirAccess.make_dir_recursive_absolute(ProjectSettings.globalize_path(DebugCapture.out_dir))
	main.get_viewport().get_texture().get_image().save_png(path)
	shots.append(path)


## The shots side by side at half size, three to a row: info.png.
func _sheet() -> String:
	var imgs := []
	for p in shots:
		var im := Image.load_from_file(ProjectSettings.globalize_path(p))
		if im != null:
			im.resize(im.get_width() / 2, im.get_height() / 2)
			imgs.append(im)
	if imgs.is_empty():
		return ""
	var w: int = imgs[0].get_width()
	var h: int = imgs[0].get_height()
	var sheet := Image.create(w * 3, h * ceili(imgs.size() / 3.0), false, imgs[0].get_format())
	for i in imgs.size():
		var im: Image = imgs[i]
		sheet.blit_rect(im, Rect2i(0, 0, mini(w, im.get_width()), mini(h, im.get_height())), Vector2i((i % 3) * w, (i / 3) * h))
	var path: String = DebugCapture.out_dir + "/info.png"
	sheet.save_png(path)
	return path
