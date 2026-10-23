class_name FineMacro
extends RefCounted

## Test helper: the world's generator (FineGen, ADR 0033) seen in the 0.5 m macro coordinates the
## biome and content tests were written in. Positions in and out are macro voxels; a block of
## macro size s is the fine block of FS * s, so material counts grow 64-fold ("more than N" holds).

const S := preload("res://scripts/settings.gd")
const FS := 4
const SEA := S.SEA_LEVEL_VOXEL / FS   # macro sea level

var fine: FineGen
var macro: WorldGen


func _init(seed := 1337) -> void:
	fine = FineGen.new()
	fine.configure(seed, S.SEA_LEVEL_VOXEL, S.WORLD_HEIGHT_VOXELS)
	macro = fine.get_macro()


func find_nearest(kind: String, x: int, z: int, radius: int) -> Vector3i:
	var p: Vector3i = fine.find_nearest(kind, x * FS, z * FS, radius * FS)
	return p if p.y < 0 else Vector3i(p.x / FS, p.y / FS, p.z / FS)


func height_at(x: int, z: int) -> int:
	return fine.height_at(x * FS + 2, z * FS + 2) / FS


func biome_at(x: int, z: int) -> int:
	return fine.biome_at(x * FS + 2, z * FS + 2)


func generate_block(origin: Vector3i, size: Vector3i, lod: int, bytes_per_voxel := 2) -> PackedByteArray:
	return fine.generate_block(origin * FS, size * FS, lod, bytes_per_voxel)


func water_floor_map(x0: int, z0: int, n: int, step: int) -> PackedInt32Array:
	return macro.water_floor_map(x0, z0, n, step)
