class_name World
extends Node3D

## Owns the godot_voxel LOD terrain (static world + horizon), the blocky
## library with the texture atlas, and exposes copy/paste for the sim bubble.

var terrain: VoxelLodTerrain
var generator: TerrainGenerator
var atlas_material: ShaderMaterial
var _tool: VoxelTool


func _ready() -> void:
	var atlas: Texture2D = load("res://assets/textures/atlas.png")
	# One material for terrain, micro models, item cubes and the viewmodel:
	# nearest-filtered atlas, vertex colour = baked AO from the mesher, and the
	# water / lava / fire tiles scroll inside their tile (rivers, falls, lava flow).
	atlas_material = ShaderMaterial.new()
	atlas_material.shader = load("res://assets/shaders/terrain.gdshader")
	atlas_material.set_shader_parameter("atlas", atlas)
	atlas_material.set_shader_parameter("atlas_tiles", Vector2(GameMaterials.ATLAS_TILES))
	atlas_material.set_shader_parameter("water_tile", float(GameMaterials.defs()[GameMaterials.WATER - 1]["tiles"][0]))
	atlas_material.set_shader_parameter("lava_tile", float(GameMaterials.defs()[GameMaterials.LAVA - 1]["tiles"][0]))
	atlas_material.set_shader_parameter("fire_tile", float(GameMaterials.defs()[GameMaterials.FIRE - 1]["tiles"][0]))
	atlas_material.set_shader_parameter("glow_tile", float(GameMaterials.defs()[GameMaterials.GLOW_MUSHROOM - 1]["tiles"][0]))
	generator = TerrainGenerator.new()
	generator.set_seed(Settings.world_seed)

	var mesher := VoxelMesherBlocky.new()
	mesher.library = GameMaterials.build_library(atlas_material)

	terrain = VoxelLodTerrain.new()
	terrain.name = "Terrain"
	terrain.mesher = mesher
	terrain.generator = generator
	terrain.lod_count = Settings.lod_count
	terrain.lod_distance = Settings.render_distance_voxels()
	terrain.view_distance = Settings.view_distance_voxels()
	terrain.streaming_system = 1  # CLIPBOX (P2: octree collapses at long range)
	terrain.mesh_block_size = Settings.terrain_mesh_block
	terrain.material = atlas_material
	# Bodies collide against voxel data (VoxelBody, ADR 0011); mesh colliders
	# rebuilt per edited block on the main thread were the frame spikes (ADR 0010).
	terrain.generate_collisions = Settings.terrain_mesh_collisions
	terrain.collision_lod_count = 1
	terrain.full_load_mode_enabled = false
	# Keep generated data for all LODs in range so edits near the player cascade
	# to the horizon rings (otherwise godot_voxel logs "destination block not
	# found when cascading edits" and far LODs never show the edit).
	terrain.cache_generated_blocks = Settings.cache_generated_blocks
	if Settings.persist_world:
		# Edited blocks are saved to user://worlds/<seed>.sqlite when they unload
		# (ADR 0002: save format must be chunk-based and versioned; SQLite is the
		# placeholder until the custom region format exists).
		var stream := VoxelStreamSQLite.new()
		DirAccess.make_dir_recursive_absolute(ProjectSettings.globalize_path("user://worlds"))
		stream.database_path = "user://worlds/world_%d_g%d.sqlite" % [Settings.world_seed, TerrainGenerator.GEN_VERSION]
		terrain.stream = stream
	terrain.voxel_bounds = AABB(Vector3(-1e7, 0, -1e7), Vector3(2e7, Settings.WORLD_HEIGHT_VOXELS, 2e7))
	add_child(terrain)
	var spray := WaterfallSpray.new()
	spray.name = "WaterfallSpray"
	spray.gen = generator.gen
	add_child(spray)
	_tool = terrain.get_voxel_tool()
	_tool.channel = VoxelBuffer.CHANNEL_TYPE


## Copies a 16^3 chunk (world chunk coords) out of the terrain as 16-bit bytes.
func read_chunk(wc: Vector3i) -> PackedByteArray:
	var vb := VoxelBuffer.new()
	vb.create(Settings.CHUNK, Settings.CHUNK, Settings.CHUNK)
	_tool.copy(wc * Settings.CHUNK, vb, 1 << VoxelBuffer.CHANNEL_TYPE)
	return vb.get_channel_as_byte_array(VoxelBuffer.CHANNEL_TYPE)


## Writes a 16^3 chunk back (VoxelTool.paste: correct under load, see P5 §C).
func write_chunk(wc: Vector3i, bytes16: PackedByteArray) -> void:
	var vb := VoxelBuffer.new()
	vb.create(Settings.CHUNK, Settings.CHUNK, Settings.CHUNK)
	vb.set_channel_from_byte_array(VoxelBuffer.CHANNEL_TYPE, bytes16)
	_tool.paste(wc * Settings.CHUNK, vb, 1 << VoxelBuffer.CHANNEL_TYPE)


func is_area_loaded(wc: Vector3i) -> bool:
	return _tool.is_area_editable(AABB(Vector3(wc * Settings.CHUNK), Vector3.ONE * Settings.CHUNK))


func get_voxel(p: Vector3i) -> int:
	return _tool.get_voxel(p)


func set_voxel(p: Vector3i, material: int) -> void:
	_tool.set_voxel(p, material)


func raycast(from: Vector3, dir: Vector3, max_dist: float) -> VoxelRaycastResult:
	return _tool.raycast(from, dir, max_dist)


func surface_height(wx: int, wz: int) -> int:
	return generator.height_at(wx, wz)
