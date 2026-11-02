class_name MenuTest
extends RefCounted

## Debug (menu_test=1, ADR 0032): the menus through their own code paths,
## across scene changes (DebugCapture, an autoload, holds this driver).
## Title -> worlds -> new world -> settings screen -> create and play; in the
## world: pick up flint, Esc menu (the tree pauses, the clock stands still),
## the Settings screen there, a graphics preset applied live; save and quit
## to the title; play the same world again (the flint and the position come
## back); quit to the title and delete the test world. Screenshots go to
## res://out/menu_test; one JSON line "debug menu_test {...}".

const OUT := "res://out/menu_test"

var tree: SceneTree
var r := {}
var shots := []


func run(dc: Node, title: Control) -> void:
	tree = dc.get_tree()
	Settings.user_settings_path = "user://menu_test_settings.cfg"   # never the player's own settings
	await _frames(20)
	await _shot("title")
	title.show_page("worlds")
	await _shot("worlds")
	title.show_page("create")
	title._name_edit.text = "Menu Test"
	title._seed_edit.text = "menu test %d" % (randi() % 100000)
	await _shot("create")
	title.show_page("settings")
	await _shot("settings")
	title.show_page("main")
	title._create_and_play()
	var main = await _wait_main()
	r["loaded"] = main != null
	if main == null:
		return _end()
	main.player_inv.pickup("flint", 5)
	await _grid_fill(main)
	await _qol_hud(main)
	main.player.position += Vector3(3, 0, 0)
	await _frames(10)
	var before: Vector3 = main.player.position
	main._on_pause_requested()
	await _frames(5)
	r["paused"] = tree.paused and main.pause_menu.is_open
	var t0: float = main.atmosphere.time_of_day
	await tree.create_timer(1.0, true).timeout
	r["clock_stopped"] = is_equal_approx(main.atmosphere.time_of_day, t0)
	await _shot("pause")
	main.pause_menu._open_settings()
	await _shot("pause_settings")
	var tabs: Array = main.pause_menu.find_children("*", "TabContainer", true, false)
	if not tabs.is_empty():
		var tc: TabContainer = tabs[0]
		tc.current_tab = tc.get_tab_count() - 1   # Interface (ADR 0130)
		r["interface_tab"] = tc.get_tab_title(tc.current_tab)
		await _shot("settings_interface")
		tc.current_tab = 0
	Settings.apply_graphics_preset("low")
	Settings.user_settings_edited()
	r["low_shadows"] = main.atmosphere.sun.directional_shadow_mode == DirectionalLight3D.SHADOW_PARALLEL_2_SPLITS and not main.atmosphere.env.ssao_enabled
	r["low_render_distance"] = main.world.terrain.lod_distance == Settings.view_distance_voxels()
	Settings.apply_graphics_preset("high")
	Settings.user_settings_edited()
	r["high_again"] = main.atmosphere.sun.directional_shadow_mode == DirectionalLight3D.SHADOW_PARALLEL_4_SPLITS and main.atmosphere.env.ssao_enabled
	main.pause_menu._close_settings()
	main.pause_menu.resume()
	r["resumed"] = not tree.paused
	await _frames(5)
	before = main.player.position
	main.pause_menu.quit_to_title.emit()
	var title2 = await _wait_title()
	r["back_at_title"] = title2 != null
	if title2 == null:
		return _end()
	var entry: Dictionary = title2.worlds.find_seed(Settings.world_seed)
	r["listed"] = not entry.is_empty() and entry["name"] == "Menu Test"
	title2.play(entry)
	var main2 = await _wait_main()
	r["reloaded"] = main2 != null
	if main2 != null:
		r["flint_kept"] = main2.player_inv.inv.count_of("flint")
		r["position_kept_m"] = snappedf(Settings.v2m(main2.player.position.distance_to(before)), 0.01)
		main2.pause_menu.quit_to_title.emit()
		var title3 = await _wait_title()
		if title3 != null:
			r["deleted_files"] = title3.worlds.delete(title3.worlds.find_seed(Settings.world_seed))
			r["gone"] = title3.worlds.find_seed(Settings.world_seed).is_empty()
	_end()


## ADR 0130: the coordinates line and the red edge after a hit, on one shot.
func _qol_hud(main) -> void:
	var was := Settings.hud_coords
	Settings.hud_coords = true
	await _frames(2)
	main.player_inv.damage(2, "menu test")
	await _frames(1)
	var hud: GameHud = main.game_hud
	r["qol_hud"] = {"coords": hud._coords.text if hud._coords.visible else "", "hurt_edge": snappedf(hud.edge_color().a, 0.01)}
	await _shot("hud_qol")
	main.player_inv.heal(2)
	Settings.hud_coords = was


## The crafting table screen's Fill and Clear buttons (bug report 2026-10-25): the bug report's
## 10 sticks and 21 dry grass, the firestarter's recipe view, Fill, then Clear.
func _grid_fill(main) -> void:
	var pi: PlayerInventory = main.player_inv
	pi.pickup("stick", 10)
	pi.pickup("dry_grass", 21)
	var sticks := pi.inv.count_of("stick")
	var grass := pi.inv.count_of("dry_grass")
	main.player.set_ui_open(true, false)
	main.inv_screen.open(3, "crafting_table", "Crafting")
	var bv: RecipeBrowserView = main.inv_screen.browser_view
	bv.show_item("firestarter", "recipes")
	var f := {"button": bv.fill_button != null}
	if bv.fill_button != null:
		f["enabled"] = not bv.fill_button.disabled
		f["tooltip"] = bv.fill_button.tooltip_text
		await _shot("craft_fill_button")
		bv.fill_button.pressed.emit()
		await _frames(2)
		f["recipe_view_closed"] = not bv.is_showing_recipes()
		f["grid_makes"] = pi.craft.current_recipe().get("id", "")
		f["sticks_left"] = pi.inv.count_of("stick")
		await _shot("craft_filled")
		var shift := pi.fill_craft_grid(pi.recipes.recipes["firestarter"], true)
		f["shift_crafts"] = shift["crafts"]
		main.inv_screen._clear_button.pressed.emit()
		f["cleared"] = pi.inv.count_of("stick") == sticks and pi.inv.count_of("dry_grass") == grass
		bv.show_item("firestarter", "recipes")   # ADR 0130: Craft makes one straight into the inventory
		await _shot("craft_button")
		if bv.craft_button != null:
			bv.craft_button.pressed.emit()
			await _frames(2)
		f["crafted"] = pi.inv.count_of("firestarter")
		f["grid_empty_after_craft"] = pi.craft.current_recipe().is_empty()
	main.inv_screen.close()
	main.inv_screen.open()   # the personal 2x2 grid has its Clear button too
	await _shot("inventory_clear")
	main.inv_screen.close()
	main.player.set_ui_open(false, false)   # as the player's own E / Esc does
	await _frames(2)
	r["grid_fill"] = f


func _end() -> void:
	DirAccess.remove_absolute(ProjectSettings.globalize_path("user://menu_test_settings.cfg"))
	r["shots"] = shots
	print("debug menu_test " + JSON.stringify(r))
	tree.quit()


func _frames(n: int) -> void:
	for i in n:
		await tree.process_frame


## The game scene once its loading screen is gone (30 s at most), or null.
func _wait_main():
	for i in 3000:
		await tree.process_frame
		var s := tree.current_scene
		if s != null and s.has_method("_save_blocks") and s.get("pause_menu") != null and not is_instance_valid(s.get("_loading")):
			await _frames(10)
			return s
	return null


func _wait_title():
	for i in 600:
		await tree.process_frame
		var s := tree.current_scene
		if s is TitleScreen and s.worlds != null:
			await _frames(10)
			return s
	return null


func _shot(shot_name: String) -> void:
	await _frames(6)
	DirAccess.make_dir_recursive_absolute(ProjectSettings.globalize_path(OUT))
	var path := OUT + "/%s.png" % shot_name
	tree.root.get_viewport().get_texture().get_image().save_png(path)
	shots.append(path)
