class_name TerrainGenerator
extends VoxelGeneratorScript

## Bridge between godot_voxel and the native FineGen (technosim extension):
## the 12.5 cm voxel layer over WorldGen's 0.5 m climate, biomes and hydrology
## (ADR 0033). All work happens in C++: one native call per block, one buffer
## copy. Heights are in voxels (12.5 cm). Sea level = Settings.SEA_LEVEL_VOXEL.

## Bump whenever generated terrain changes: saves are keyed by seed AND this
## version, so a world saved by an older generator is never loaded over a
## newer one (stale chunks would mix old terrain with new).
const GEN_VERSION := 33  # 33: rope bridges over ravines, a flora of its own in every land biome (sagebrush, marigold, campion, fire lily, lingonberry, bromeliad, cattail, aloe, bluebell, frost lichen, sea aster, dandelion), igloos, jungle temples, desert tombs, mountain and volcanic cairns; 32: wooden lookouts, mines lined below the floor, dune grass / heather / tundra sedge; 31: abandoned camps, natural arches and hoodoos, rails in mine pots; 30: abandoned mines (head-frame, spoil heap, timbered shaft); 29: cave biome dressing (dripstone, icicles, cave vines, glow berries, fungi, magma crust); 28: conduits (aquifers, magma tubes; ADR 0110), stone lips on cave water sheets; 27: bigger, longer caves with cave water as floor sheets (ADR 0109), no plants over opened cave tops, giant meadow flowers, lush jungle with vines, crooked sticks; 26: beaches only along the sea, sloped beach / berm / foreshore / sandbar / shelf, sand-mud-gravel sea floor, birch forest, flower meadow, volcanic peaks, mangrove; 25: bigger stone circles, ore stringers in clumps (ADR 0086); 24: gallery storeys of followable passages, no grass skins over open cave tops, sticks only on level ground, half the sticks and loose stones (ADR 0080); 23: cave storeys (tall rifts deep down), winding mouths, sinkholes, chimneys, hall pillars, ore stringers on cave walls (ADR 0077); 22: river surfaces continuous across lattice cells, still lava pools, no lone liquid cells (ADR 0069 C); 21: no waterfall sheets (ADR 0068); 20: deeper sea, mountain cliff bands and aretes, ledges on the ground and in caves, flared ravines, nothing standing over a ravine, bigger cave pools; 19: bunched ore lumps, dry savanna floor (ADR 0064); 18: ruin kinds (watchtower, stone circle, crypt, mud house, shrine, wreck; ADR 0062), level ground and a ramp round each ruin; 17: ruin pots only in the corners, 4 at most; 16: 640 m world, massif mountains, big caves (caverns, ravines, mouths, geodes), sea cliffs and stacks, monoliths, meanders, deltas, lake rims, enclosed cave pools, scarcer wild food (ADR 0051, 0052); 15: flowers, mushrooms, seashells, kelp and seagrass as voxel pieces, ore veins as nuggets, bigger varied pickups (ADR 0047); 14: round blueberry bushes, rivers interpolated per voxel with filled trenches and banks, the sea static (reservoirs, ADR 0045); 13: continents, plains / hills / mountain ranges, forests with young and old trees, forest floor; 12: 12.5 cm voxels (ADR 0033); 11: boat-sized rivers, deep sloped ocean floor; 5: lakes/marshes/waterfalls/vegetation; 6: biomes.json classifier (latitude climate, 36 biomes, cave layer); 9: GregTech ore veins + ground pickups (ADR 0019); 10: sulfur caves, cave pools and ceilings

var seed_value := 1337
var gen: FineGen = FineGen.new()


func _init() -> void:
	gen.set_uniform_blocks(true)   # deep rock comes back as one voxel; _generate_block fills it
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
	if bytes.size() == bpv:
		# one voxel: the whole block is that material (deep rock), stored uniform by godot_voxel
		out_buffer.fill(bytes.decode_u8(0) if bpv == 1 else bytes.decode_u16(0), VoxelBuffer.CHANNEL_TYPE)
		return
	out_buffer.set_channel_from_byte_array(VoxelBuffer.CHANNEL_TYPE, bytes)
