class_name World
extends Node3D

## Owns the godot_voxel LOD terrain (static world + horizon), the blocky
## library with the texture atlas, and exposes copy/paste for the sim bubble.

var terrain: VoxelLodTerrain
var generator: TerrainGenerator
var atlas_material: ShaderMaterial
var liquid_material: ShaderMaterial
var gas_material: ShaderMaterial    # steam and smoke as soft clouds (terrain_gas.gdshader)
var edits := 0                      # terrain writes so far: the autosave skips the terrain when none came since the last
var spray: WaterfallSpray          # mist at waterfall feet; ParticleFx reads its falls (ADR 0026)
var prop_material: ShaderMaterial   # atlas material without stains: item cubes, viewmodel (they are not at their voxel)
var _tool: VoxelTool


func _ready() -> void:
	# The atlas is imported without mipmaps (its .import file is generated, not
	# tracked), so they are built here: without them every far block samples
	# full-resolution texels and the horizon turns to pixel noise (ADR 0017).
	var atlas_img: Image = (load("res://assets/textures/atlas.png") as Texture2D).get_image()
	if atlas_img.generate_mipmaps() != OK:
		push_warning("atlas: mipmaps not generated (format %d)" % atlas_img.get_format())
	var atlas: Texture2D = ImageTexture.create_from_image(atlas_img)
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
	atlas_material.set_shader_parameter("grass_top_tile", float(GameMaterials.defs()[GameMaterials.GRASS - 1]["tiles"][0]))
	atlas_material.set_shader_parameter("grass_side_tile", float(GameMaterials.defs()[GameMaterials.GRASS - 1]["tiles"][1]))
	atlas_material.set_shader_parameter("pattern_tiles", GameMaterials.pattern_tiles())
	atlas_material.set_shader_parameter("far_fade_start", Settings.m2v(Settings.far_fade_start_m))
	atlas_material.set_shader_parameter("far_fade_end", Settings.m2v(Settings.far_fade_end_m))
	# liquids and gases: same uniforms, alpha + depth write + water glint (terrain_liquid.gdshader)
	liquid_material = ShaderMaterial.new()
	liquid_material.shader = load("res://assets/shaders/terrain_liquid.gdshader")
	for k in ["atlas", "atlas_tiles", "water_tile", "lava_tile", "fire_tile", "glow_tile", "grass_top_tile", "grass_side_tile", "pattern_tiles", "far_fade_start", "far_fade_end"]:
		liquid_material.set_shader_parameter(k, atlas_material.get_shader_parameter(k))
	gas_material = ShaderMaterial.new()
	gas_material.shader = load("res://assets/shaders/terrain_gas.gdshader")
	for k in ["atlas", "atlas_tiles", "far_fade_start", "far_fade_end", "pattern_tiles"]:
		gas_material.set_shader_parameter(k, atlas_material.get_shader_parameter(k))
	prop_material = atlas_material.duplicate()
	prop_material.shader = load("res://assets/shaders/terrain_prop.gdshader")
	generator = TerrainGenerator.new()
	generator.set_seed(Settings.world_seed)

	# Minecraft-style liquid surfaces with levels (terrain_fluid.gdshader), one material per liquid
	var fluid_materials := {}
	for d in GameMaterials.defs():
		if d.has("fluid_slot") and not d.get("liquid_static", false):
			var fm := ShaderMaterial.new()
			fm.shader = load("res://assets/shaders/terrain_fluid.gdshader" if d.get("transparent", false) else "res://assets/shaders/terrain_fluid_opaque.gdshader")
			for k in ["atlas", "atlas_tiles", "water_tile", "lava_tile", "fire_tile", "glow_tile"]:
				fm.set_shader_parameter(k, atlas_material.get_shader_parameter(k))
			fm.set_shader_parameter("fluid_tile", float(d["tiles"][0]))
			fluid_materials[int(d["fluid_slot"])] = fm
	var mesher := VoxelMesherBlocky.new()
	mesher.library = GameMaterials.build_library(atlas_material, liquid_material, fluid_materials, gas_material)

	terrain = VoxelLodTerrain.new()
	terrain.name = "Terrain"
	terrain.mesher = mesher
	terrain.generator = generator
	terrain.lod_count = Settings.lod_count
	terrain.lod_distance = Settings.render_distance_voxels()
	terrain.secondary_lod_distance = Settings.m2v(Settings.lod_ring_m)
	if "lod_vertical_ratio" in terrain:
		terrain.lod_vertical_ratio = Settings.lod_vertical_ratio
	terrain.threaded_update_enabled = Settings.terrain_threaded_update
	terrain.view_distance = Settings.view_distance_voxels()
	terrain.streaming_system = 1  # CLIPBOX (P2: octree collapses at long range)
	terrain.lod_fade_duration = Settings.lod_fade_s   # blocks blend between LODs (shader: lod_fade_discard)
	terrain.mesh_block_size = Settings.terrain_mesh_block
	if "mesh_upload_budget_mb" in terrain:   # our godot_voxel build only (ADR 0020)
		terrain.mesh_upload_budget_mb = Settings.mesh_upload_budget_mb
	if "edit_remesh_rate" in terrain:
		terrain.edit_remesh_rate = Settings.edit_remesh_rate
	# No terrain.material: VoxelLodTerrain would copy it per block and override
	# every surface of the block mesh with it, so the liquid shader from the
	# library never ran on the LOD terrain (water drew opaque) and every block
	# carried its own ShaderMaterial. Surfaces keep the library's materials; the
	# LOD cross-fade reaches the shaders as an instance parameter (ADR 0020).
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
		# Every block of every LOD is asked from the stream before it is generated. Without
		# the key cache each miss is a database query on one thread: ~12,000 queued blocks,
		# the world never settles and detail trickles in for a minute (ADR 0017).
		stream.set_key_cache_enabled(true)
		terrain.stream = stream
	terrain.voxel_bounds = AABB(Vector3(-1e7, 0, -1e7), Vector3(2e7, Settings.WORLD_HEIGHT_VOXELS, 2e7))
	add_child(terrain)
	spray = WaterfallSpray.new()
	spray.name = "WaterfallSpray"
	spray.gen = generator.gen
	add_child(spray)
	_tool = terrain.get_voxel_tool()
	_tool.channel = VoxelBuffer.CHANNEL_TYPE


## Copies a 16^3 chunk (world chunk coords) out of the terrain as 16-bit bytes.
## The Settings screen changed render or view distance (ADR 0032): the terrain re-rings live.
func apply_distances() -> void:
	terrain.lod_distance = Settings.render_distance_voxels()
	terrain.secondary_lod_distance = Settings.m2v(Settings.lod_ring_m)
	if "lod_vertical_ratio" in terrain:
		terrain.lod_vertical_ratio = Settings.lod_vertical_ratio
	terrain.view_distance = Settings.view_distance_voxels()


func read_chunk(wc: Vector3i) -> PackedByteArray:
	var vb := VoxelBuffer.new()
	vb.create(Settings.CHUNK, Settings.CHUNK, Settings.CHUNK)
	_tool.copy(wc * Settings.CHUNK, vb, 1 << VoxelBuffer.CHANNEL_TYPE)
	return vb.get_channel_as_byte_array(VoxelBuffer.CHANNEL_TYPE)


## Copies a column of `count` chunks from world chunk `wc` up as 16-bit bytes
## (16 x 16*count x 16, y fastest): one terrain copy for the whole column.
func read_column(wc: Vector3i, count: int) -> PackedByteArray:
	var vb := VoxelBuffer.new()
	vb.create(Settings.CHUNK, Settings.CHUNK * count, Settings.CHUNK)
	_tool.copy(wc * Settings.CHUNK, vb, 1 << VoxelBuffer.CHANNEL_TYPE)
	return vb.get_channel_as_byte_array(VoxelBuffer.CHANNEL_TYPE)


func is_box_loaded(wmin: Vector3i, size: Vector3i) -> bool:
	return _tool.is_area_editable(AABB(Vector3(wmin), Vector3(size)))


## Writes a 16^3 chunk back (VoxelTool.paste: correct under load, see P5 §C).
func write_chunk(wc: Vector3i, bytes16: PackedByteArray) -> void:
	var vb := VoxelBuffer.new()
	vb.create(Settings.CHUNK, Settings.CHUNK, Settings.CHUNK)
	vb.set_channel_from_byte_array(VoxelBuffer.CHANNEL_TYPE, bytes16)
	_tool.paste(wc * Settings.CHUNK, vb, 1 << VoxelBuffer.CHANNEL_TYPE)
	edits += 1


func is_area_loaded(wc: Vector3i) -> bool:
	return _tool.is_area_editable(AABB(Vector3(wc * Settings.CHUNK), Vector3.ONE * Settings.CHUNK))


## The material at a world voxel (a partly full liquid reads as its liquid).
func get_voxel(p: Vector3i) -> int:
	var v := _tool.get_voxel(p)
	return v if v < GameMaterials.FLUID_LEVEL_BASE else GameMaterials.material_of_voxel(v)


func set_voxel(p: Vector3i, material: int) -> void:
	_tool.set_voxel(p, material)
	edits += 1


## False while the voxel's block is not loaded (set_voxel would be dropped).
func is_editable(p: Vector3i) -> bool:
	return _tool.is_area_editable(AABB(Vector3(p), Vector3.ONE))


## The first voxel along the ray whose collision box matches `mask`: by default also the aim-only
## boxes (GameMaterials.MASK_TARGET: work spots, ground pickups); VoxelBody.MASK_SOLID sees only
## what blocks bodies.
func raycast(from: Vector3, dir: Vector3, max_dist: float, mask: int = 0xFFFFFFFF) -> VoxelRaycastResult:
	return _tool.raycast(from, dir, max_dist, mask)


func surface_height(wx: int, wz: int) -> int:
	return generator.height_at(wx, wz)
