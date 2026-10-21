class_name TerrainGenerator
extends VoxelGeneratorScript

## Placeholder layered world generator (GDScript, column fills only).
## Phase 2 replaces this with the native Minecraft-1.18-style generator
## (continentalness / erosion / peaks splines, caves, biomes).
## Heights are in voxels (0.5 m). Sea level = Settings.SEA_LEVEL_VOXEL.

var seed_value := 1337
var _continent := FastNoiseLite.new()
var _hills := FastNoiseLite.new()
var _detail := FastNoiseLite.new()
var _sand := FastNoiseLite.new()


func _init() -> void:
	_continent.seed = seed_value
	_continent.frequency = 0.0008
	_continent.fractal_octaves = 3
	_hills.seed = seed_value + 1
	_hills.frequency = 0.004
	_hills.fractal_octaves = 4
	_detail.seed = seed_value + 2
	_detail.frequency = 0.03
	_sand.seed = seed_value + 3
	_sand.frequency = 0.02


func set_seed(s: int) -> void:
	seed_value = s
	_init()


func height_at(wx: int, wz: int) -> int:
	var c := _continent.get_noise_2d(wx, wz)          # -1..1 large scale
	var h := _hills.get_noise_2d(wx, wz)              # -1..1
	var d := _detail.get_noise_2d(wx, wz)             # -1..1
	# continent: below sea for c < -0.2, plains around 0, mountains for c > 0.4
	var base := float(Settings.SEA_LEVEL_VOXEL)
	var land := c * 60.0
	var mountains := maxf(0.0, c - 0.35) * 350.0 * (0.6 + 0.4 * h)
	var hills := h * 30.0 * (0.5 + 0.5 * clampf(c + 0.5, 0.0, 1.0))
	return int(base + land + mountains + hills + d * 6.0)


func _get_used_channels_mask() -> int:
	return 1 << VoxelBuffer.CHANNEL_TYPE


func _generate_block(out_buffer: VoxelBuffer, origin: Vector3i, lod: int) -> void:
	var size := out_buffer.get_size()
	var ch := VoxelBuffer.CHANNEL_TYPE
	var step := 1 << lod
	var sea := Settings.SEA_LEVEL_VOXEL
	# Fast reject: whole block above the highest possible terrain and above sea -> air.
	if origin.y > sea + 420:
		return
	if origin.y + size.y * step <= 0:
		out_buffer.fill(GameMaterials.BEDROCK, ch)
		return
	for z in size.z:
		var wz := origin.z + z * step
		for x in size.x:
			var wx := origin.x + x * step
			var h := height_at(wx, wz)
			# local layer boundaries in buffer voxels (lod-scaled)
			var top := clampi(int(ceil(float(h - origin.y) / step)), 0, size.y)
			var stone_top := clampi(int(ceil(float(h - 4 - origin.y) / step)), 0, size.y)
			var bedrock_top := clampi(int(ceil(float(2 - origin.y) / step)), 0, size.y)
			var water_top := clampi(int(ceil(float(sea - origin.y) / step)), 0, size.y)
			var beach := h <= sea + 2 and _sand.get_noise_2d(wx, wz) > -0.3
			if bedrock_top > 0:
				out_buffer.fill_area(GameMaterials.BEDROCK, Vector3i(x, 0, z), Vector3i(x + 1, bedrock_top, z + 1), ch)
			if stone_top > bedrock_top:
				out_buffer.fill_area(GameMaterials.STONE, Vector3i(x, bedrock_top, z), Vector3i(x + 1, stone_top, z + 1), ch)
			if top > stone_top:
				var soil := GameMaterials.SAND if beach else GameMaterials.DIRT
				var soil_top := top if beach else maxi(stone_top, top - maxi(1, 1 / step))
				out_buffer.fill_area(soil, Vector3i(x, stone_top, z), Vector3i(x + 1, soil_top, z + 1), ch)
				if not beach and top > soil_top:
					out_buffer.fill_area(GameMaterials.GRASS, Vector3i(x, soil_top, z), Vector3i(x + 1, top, z + 1), ch)
			if water_top > top:
				out_buffer.fill_area(GameMaterials.WATER, Vector3i(x, top, z), Vector3i(x + 1, water_top, z + 1), ch)
