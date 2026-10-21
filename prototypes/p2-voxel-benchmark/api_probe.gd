extends SceneTree
func _init() -> void:
	for c in ["VoxelToolLodTerrain","VoxelToolTerrain","VoxelStreamSQLite","VoxelStreamRegionFiles","VoxelGeneratorGraph","VoxelBlockyModelCube","VoxelBlockyModel","VoxelTerrainMultiplayerSynchronizer"]:
		var ms := []
		for m in ClassDB.class_get_method_list(c, true): ms.append(m.name)
		var ps := []
		for p in ClassDB.class_get_property_list(c, true): ps.append(p.name)
		print(c, " exists=", ClassDB.class_exists(c), " methods=", ", ".join(ms), " props=", ", ".join(ps))
	print("VoxelTool methods: ", ", ".join(ClassDB.class_get_method_list("VoxelTool", true).map(func(m): return m.name)))
	print("VoxelBlockyModelCube side enum: ", ClassDB.class_get_enum_constants("VoxelBlockyModel", "Side"))
	quit(0)
