extends GdUnitTestSuite

## Every item tagged "machine" is at least furnace sized (the 1 m cube of StationEntity.SIZES["furnace"]); the
## coke oven's controller is the one exception, its brick shell is the multiblock round it (ADR 0117).

func test_machines_are_furnace_sized() -> void:
	var items := ItemRegistry.load_default()
	var want: Vector3i = StationEntity.SIZES["furnace"]
	var bad := []
	for id in items.all_ids():
		var d := items.get_def(id)
		if not items.has_tag(id, "machine") or String(d.get("work", "")) in ["", "coke_oven"]:
			continue
		var kind := String(d["work"])
		var cells := BlockEntities.footprint_of(kind, Vector3i.ZERO)
		var hi := Vector3i.ZERO
		for c in cells:
			hi = Vector3i(maxi(hi.x, c.x + 1), maxi(hi.y, c.y + 1), maxi(hi.z, c.z + 1))
		if hi.x < want.x or hi.y < want.y or hi.z < want.z:
			bad.append("%s %s" % [kind, hi])
	assert_array(bad).is_empty()
