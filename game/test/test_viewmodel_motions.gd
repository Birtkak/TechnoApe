extends GdUnitTestSuite

## Craft motions of the first-person hand (bug report 2026-10-27_18-03-10): the held tool picks
## the motion, and every motion starts and ends at rest so the hand never jumps.


func test_held_tool_picks_the_motion() -> void:
	assert_str(Viewmodel.motion_for({"id": "firestarter", "tags": ["tool", "firestarter"]})).is_equal("drill")
	assert_str(Viewmodel.motion_for({"id": "flint_and_steel", "tags": ["tool", "firestarter"]})).is_equal("strike")
	assert_str(Viewmodel.motion_for({"id": "copper_hammer", "tags": ["tool", "hammer"]})).is_equal("hammer")
	assert_str(Viewmodel.motion_for({"id": "flint", "tags": ["flint", "knappable"]})).is_equal("strike")
	assert_str(Viewmodel.motion_for({"id": "flint_knife", "tags": ["tool", "knife"]})).is_equal("scrape")
	assert_str(Viewmodel.motion_for({"id": "clay_ball", "tags": ["clay"]})).is_equal("press")
	assert_str(Viewmodel.motion_for({"id": "flint_pickaxe", "tags": ["tool", "pickaxe"]})).is_equal("")
	assert_str(Viewmodel.motion_for({})).is_equal("")


func test_every_motion_starts_and_ends_at_rest() -> void:
	for m in Viewmodel.MOTIONS:
		for t in [0.0, 1.0]:
			var pose := Viewmodel.motion_pose(m, t)
			assert_float((pose[0] as Vector3).length()).override_failure_message("%s at %s" % [m, t]).is_less(0.001)
			assert_float((pose[1] as Vector3).length()).override_failure_message("%s at %s" % [m, t]).is_less(0.01)
		assert_float((Viewmodel.motion_pose(m, 0.5)[1] as Vector3).length()).is_greater(1.0)   # it moves
