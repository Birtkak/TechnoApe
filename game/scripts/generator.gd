class_name TerrainGenerator
extends VoxelGeneratorScript

## Bridge between godot_voxel and the native FineGen (technosim extension):
## the 12.5 cm voxel layer over WorldGen's 0.5 m climate, biomes and hydrology
## (ADR 0033). All work happens in C++: one native call per block, one buffer
## copy. Heights are in voxels (12.5 cm). Sea level = Settings.SEA_LEVEL_VOXEL.

## Bump whenever generated terrain changes: saves are keyed by seed AND this
## version, so a world saved by an older generator is never loaded over a
## newer one (stale chunks would mix old terrain with new).
const GEN_VERSION := 13  # 13: continents, plains / hills / mountain ranges, forests with young and old trees, forest floor; 12: 12.5 cm voxels (ADR 0033); 11: boat-sized rivers, deep sloped ocean floor; 5: lakes/marshes/waterfalls/vegetation; 6: biomes.json classifier (latitude climate, 36 biomes, cave layer); 9: GregTech ore veins + ground pickups (ADR 0019); 10: sulfur caves, cave pools and ceilings

var seed_value := 1337
var gen: FineGen = FineGen.new()


func _init() -> void:
	set_seed(seed_value)


func set_seed(s: int) -> void:
	seed_value = s
	gen.configure(s, Settings.SEA_LEVEL_VOXEL, Settings.WORLD_HEIGHT_VOXELS)


func height_at(wx: int, wz: int) -> int:
	return gen.height_at(wx, wz)


func biome_at(wx: int, wz: int) -> int:
	return gen.biome_at(wx, wz)


func column_info(wx: int, wz: int) -> Dictionary:
	return gen.column_info(wx, wz)


func _get_used_channels_mask() -> int:
	return 1 << VoxelBuffer.CHANNEL_TYPE


func _generate_block(out_buffer: VoxelBuffer, origin: Vector3i, lod: int) -> void:
	var size := out_buffer.get_size()
	var depth := out_buffer.get_channel_depth(VoxelBuffer.CHANNEL_TYPE)
	var bpv := 1 if depth == VoxelBuffer.DEPTH_8_BIT else (2 if depth == VoxelBuffer.DEPTH_16_BIT else 4)
	var bytes: PackedByteArray = gen.generate_block(origin, size, lod, bpv)
	if bytes.is_empty():
		return  # all air; buffer already zero
	out_buffer.set_channel_from_byte_array(VoxelBuffer.CHANNEL_TYPE, bytes)
