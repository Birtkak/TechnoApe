extends GdUnitTestSuite

## The player's body (ApeModel): worn clothing shows on it, and Shift crouches it.


func _model() -> ApeModel:
	var m: ApeModel = auto_free(ApeModel.new())
	add_child(m)
	return m


func test_worn_gear_is_built_and_cleared() -> void:
	var m := _model()
	m.set_worn({"head": {"id": "miners_helmet"}, "back": {"id": "jetpack"}, "feet": {"id": "leather_boots"}})
	assert_int(m._gear.size()).is_greater_equal(5)   # helmet, jetpack, flame, two boots
	assert_object(m._flame).is_not_null()
	m.jet_flame(true, 0.0)
	assert_bool(m._flame.visible).is_true()
	m.set_worn({})
	assert_int(m._gear.size()).is_equal(0)
	assert_object(m._flame).is_null()


func test_gear_color_comes_from_the_sprite() -> void:
	var c := ApeModel.gear_color("leather_tunic")
	assert_float(c.r).is_greater(c.b)   # leather is brown


func test_sneak_leans_the_body_forward() -> void:
	var m := _model()
	m.animate(0.016, 0.0, true, false, 0.0, 0.0)
	var stand := m.head.position
	for i in 30:
		m.animate(0.016, 0.0, true, false, 0.0, 0.0, true)
	assert_float(m.head.position.z).is_less(stand.z - 0.1)   # forward is -Z
	assert_float(m.head.position.y).is_less(stand.y)


func test_portrait_wears_the_same_gear() -> void:
	var p: CharacterPreview = auto_free(CharacterPreview.new())
	add_child(p)
	p.set_worn({"body": {"id": "leather_tunic"}})
	assert_int(p.model._gear.size()).is_equal(6)   # vest and two sleeves, each a tinted mesh and its dark hem
