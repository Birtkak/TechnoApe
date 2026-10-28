extends GdUnitTestSuite

## The Gelvin monolith (ADR 0098) cannot be destroyed: not mined, not blasted, not dug, brushed or
## painted, even in creative.


func test_monolith_materials_are_indestructible() -> void:
	var mining := MiningTable.load_default()
	var dig := BrushTool.dig_mask(true, {}, mining, func(_m: int) -> bool: return false)
	var paint := BrushTool.paint_mask()
	for m in [GameMaterials.MONOLITH, GameMaterials.MONOLITH_RUNE, GameMaterials.MONOLITH_SEAM]:
		var d: Dictionary = GameMaterials.defs()[m - 1]
		assert_bool(GameMaterials.is_monolith(m)).is_true()
		assert_float(mining.break_time(String(d["name"]))).is_equal(-1.0)
		assert_float(float(d["hp"])).is_greater_equal(1e6)   # SimBubble.blast keeps it (laser, crater, TNT)
		assert_int(dig[m]).is_equal(0)
		assert_int(paint[m]).is_equal(0)
	assert_bool(GameMaterials.is_monolith(GameMaterials.TNT)).is_false()
	assert_bool(GameMaterials.is_monolith(GameMaterials.STONE)).is_false()
