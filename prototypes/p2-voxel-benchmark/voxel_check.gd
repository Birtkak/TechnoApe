extends SceneTree
func _init() -> void:
	for c in ["VoxelTerrain","VoxelLodTerrain","VoxelViewer","VoxelMesherBlocky","VoxelBlockyLibrary","VoxelBlockyModelCube","VoxelGeneratorNoise2D","VoxelGeneratorScript","VoxelBuffer","VoxelTerrainMultiplayerSynchronizer"]:
		print("%s exists=%s" % [c, ClassDB.class_exists(c)])
	for c in ["VoxelGeneratorNoise2D","VoxelViewer","VoxelTerrain","VoxelBlockyLibrary","VoxelMesherBlocky"]:
		var names := []
		for p in ClassDB.class_get_property_list(c, true): names.append(p.name)
		print("%s props: %s" % [c, ", ".join(names)])
	print("VoxelBlockyLibrary methods: ", ", ".join(ClassDB.class_get_method_list("VoxelBlockyLibrary", true).map(func(m): return m.name)))
	print("VoxelTerrain methods: ", ", ".join(ClassDB.class_get_method_list("VoxelTerrain", true).map(func(m): return m.name)))
	quit(0)
