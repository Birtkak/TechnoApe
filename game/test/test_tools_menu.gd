extends GdUnitTestSuite

## The world tools panel (ADR 0071): its buttons show the player's brush and builder state and set
## them, and grey out what the mode or the empty hand rules out.


class StubMain extends Node:
	var player: Player
	var builder: ShapeBuilder


func _menu() -> ToolsMenu:
	var main := StubMain.new()
	main.player = auto_free(Player.new())   # off the tree: no world, camera or HUD
	main.player._crack = auto_free(Node3D.new())
	main.builder = auto_free(ShapeBuilder.new())
	main.builder.player = main.player
	add_child(auto_free(main))
	var m := ToolsMenu.new()
	m.main = main
	main.add_child(m)
	m.open(true)
	return m


func test_buttons_show_the_brush_and_set_it() -> void:
	var m := _menu()
	var p: Player = m.main.player
	assert_bool(m.is_open).is_true()
	assert_bool(m._buttons["shape:cube"].button_pressed).is_true()
	assert_bool(m._buttons["size:4"].button_pressed).is_true()
	assert_bool(m._buttons["mode:brush"].button_pressed).is_true()
	m._buttons["shape:flat"].pressed.emit()
	m._buttons["size:8"].pressed.emit()
	assert_str(p.brush_shape).is_equal("flat")
	assert_int(p.brush_size).is_equal(8)
	m._refresh()
	assert_bool(m._buttons["shape:flat"].button_pressed).is_true()
	assert_bool(m._buttons["shape:cube"].button_pressed).is_false()
	assert_bool(m._buttons["size:8"].button_pressed).is_true()
	var was := Settings.brush_ghost
	m._ghost.button_pressed = not was
	assert_bool(Settings.brush_ghost).is_equal(not was)
	Settings.brush_ghost = was
	Settings.save_user_settings()


func test_sizes_follow_the_mode_and_shapes_need_a_block() -> void:
	var m := _menu()
	var p: Player = m.main.player
	p.game_mode = GameMode.SURVIVAL
	m._refresh()
	assert_bool(m._buttons["size:32"].disabled).is_true()
	assert_str(m._buttons["size:32"].tooltip_text).contains("creative")
	p.game_mode = GameMode.CREATIVE
	m._refresh()
	assert_bool(m._buttons["size:32"].disabled).is_false()
	# no block in hand: shapes and box fill are out, box dig and select are not
	assert_bool(m._buttons["mode:shape:box"].disabled).is_true()
	assert_bool(m._buttons["mode:box"].disabled).is_true()
	assert_str(m._buttons["mode:shape:box"].tooltip_text).contains("hold a block")
	assert_bool(m._buttons["mode:dig"].disabled).is_false()
	assert_bool(m._buttons["mode:select"].disabled).is_false()
	for s in Brush.SHAPES:
		assert_str(m._buttons["shape:" + s].tooltip_text).is_not_empty()


func test_close_hides_and_g_toggles() -> void:
	var m := _menu()
	m.close()
	assert_bool(m.is_open).is_false()
	assert_bool(m._center.visible).is_false()
	var ev := InputEventKey.new()
	ev.keycode = KEY_G
	ev.pressed = true
	m._unhandled_input(ev)
	assert_bool(m.is_open).is_true()
