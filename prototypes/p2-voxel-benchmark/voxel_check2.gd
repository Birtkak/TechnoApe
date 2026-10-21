extends SceneTree
func _init() -> void:
	for c in ["VoxelMesherBlocky","VoxelMesherCubes","VoxelMesherTransvoxel"]:
		var m = ClassDB.instantiate(c)
		var has: bool = m.has_method("supports_lod")
		print("%s supports_lod=%s" % [c, m.supports_lod() if has else "n/a"])
	for c in ["VoxelMesher","VoxelNode","VoxelLodTerrain","VoxelGeneratorHeightmap","VoxelBlockyLibraryBase","VoxelBlockyModel","VoxelBlockyModelCube","VoxelTerrainLarge"]:
		var names := []
		for p in ClassDB.class_get_property_list(c, true): names.append(p.name)
		var meths := []
		for mm in ClassDB.class_get_method_list(c, true): meths.append(mm.name)
		print("%s props: %s | methods: %s" % [c, ", ".join(names), ", ".join(meths)])
	quit(0)
