# --- what the player can see loads first (ADR 0136) ---
# Part of the TechnoApe godot_voxel patches: run after voxel_patch_shell.py and voxel_patch_sky.py.
# Flying low at 20 m/s the shell's near cube (shell_near around the viewer, any direction) asks for ~13 k data blocks a
# second of rock under the ground, the same order as the generators' whole throughput, and generation priority is
# distance only: the ground band at the far side of the box waited behind buried rock next to the player and the
# player saw through the world (the /holes census: 94 % of the ground in view not drawn was still generating).
# While a drawing viewer stands above its column's lowest ground, a lod 0 block under its column's ground band (only
# the near cube asked for it) gets PriorityDependency::distance_bias = BURIED_BIAS voxels: it still loads (caves, mines,
# the simulation bubble), after every block of the box a player above ground can see. Underground (a cave, a mine)
# nothing changes. A band block under its own column's lowest ground gets its depth there as bias (behind the surface).
import sys
root = sys.argv[1]
if 'distance_bias' in open(root + '/engine/priority_dependency.h', encoding='utf-8').read():
    sys.exit(0)  # already applied (later steps below edit what earlier ones wrote, so they are applied as a whole)

def patch(rel, old, new):
    p = root + '/' + rel
    s = open(p, encoding='utf-8').read()
    if s.count(new) == 1 and (new.count(old) or s.count(old) == 0):
        return  # already done
    assert s.count(old) == 1, (rel, old)
    s = s.replace(old, new, 1)
    open(p, 'w', encoding='utf-8', newline='\n').write(s)

PH = 'engine/priority_dependency.h'
PC = 'engine/priority_dependency.cpp'
D = 'terrain/variable_lod/voxel_lod_terrain_update_data.h'
T = 'terrain/variable_lod/voxel_lod_terrain_update_task.cpp'
C = 'terrain/variable_lod/voxel_lod_terrain_update_clipbox_streaming.cpp'

patch(PH, '''	float drop_distance_squared;
''', '''	float drop_distance_squared;
	// TechnoApe ADR 0136: added to the distance the priority is taken from (not to the drop distance), in voxels
	float distance_bias = 0.f;
''')
patch(PC, '''	const int distance = static_cast<int>(Math::sqrt(closest_distance_sq));
''', '''	float d = Math::sqrt(closest_distance_sq);
	if (view_weight > 0.f && has_view) {
		// TechnoApe ADR 0136: what the camera looks at and flies towards first (view_weight, below)
		const Vector3f a = block_position - lead_position;
		d = Math::sqrt(a.x * a.x + a.y * a.y + a.z * a.z);
		const Vector3f v = block_position - view_position;
		const float l = Math::sqrt(v.x * v.x + v.y * v.y + v.z * v.z);
		if (l > 1.f) {
			const float c = (v.x * view_direction.x + v.y * view_direction.y + v.z * view_direction.z) / l;
			d *= 1.f + view_weight * (1.f - c);
		}
	}
	const int distance = static_cast<int>(d + distance_bias);
''')
patch(D, '''		StdUnorderedMap<uint64_t, std::pair<int, int>> shell_band;
''', '''		StdUnorderedMap<uint64_t, std::pair<int, int>> shell_band;
		// TechnoApe ADR 0136: a drawing viewer is over its column's lowest ground: buried blocks load last
		bool shell_viewer_above = false;
''')
patch(C, '''			const Vector3i shell_vpos_prev = paired_viewer.prev_state.local_position_voxels >> lod_data_block_size_po2;
''', '''			const Vector3i shell_vpos_prev = paired_viewer.prev_state.local_position_voxels >> lod_data_block_size_po2;
			if (shell) {
				// TechnoApe ADR 0136 (voxel_patch_visible_first.py)
				state.shell_viewer_above = paired_viewer.state.local_position_voxels.y >=
						shell_column_ground(state, settings, shell_vpos.x, shell_vpos.z, 1 << lod_data_block_size_po2).first;
			}
''')
patch(T, '''		BufferedTaskScheduler &task_scheduler,
		const TaskCancellationToken cancellation_token
) {
	CRASH_COND(data_block_size > 255);''', '''		BufferedTaskScheduler &task_scheduler,
		const TaskCancellationToken cancellation_token,
		const float distance_bias = 0.f
) {
	CRASH_COND(data_block_size > 255);''')
patch(T, '''			settings.lod_distance
	);

	IThreadedTask *task = stream_dependency->generator->create_block_task(params);''', '''			settings.lod_distance
	);
	params.priority_dependency.distance_bias = distance_bias; // TechnoApe ADR 0136

	IThreadedTask *task = stream_dependency->generator->create_block_task(params);''')
patch(T, '''	if (stream_dependency->stream.is_valid() && !known_absent) {
		PriorityDependency priority_dependency;''', '''	// TechnoApe ADR 0136: a block wholly under its column's lowest ground (less shell_below) waits behind everything
	// a viewer above the ground can see
	constexpr float BURIED_BIAS = 1024.f;
	float distance_bias = 0.f;
	if (lod_index == 0) {
		++PriorityDependency::requests_lod0;
	}
	if (lod_index == 0 && state.shell_viewer_above) {
		const uint64_t key = (uint64_t(uint32_t(block_pos.x)) << 32) | uint32_t(block_pos.z);
		// under its column's ground band (shell_column_ranges): only the near cube asked for it
		auto b = state.shell_band.find(key);
		if (b != state.shell_band.end() && block_pos.y < b->second.first) {
			distance_bias = BURIED_BIAS;
			++PriorityDependency::requests_buried;
		} else {
			// in the band but under its own column's lowest ground: behind the surface from above, seen only on a
			// cliff face or from a lower neighbour; the deeper, the later (1 voxel of priority distance per voxel)
			auto g = state.shell_ground.find(key);
			if (g != state.shell_ground.end()) {
				distance_bias = float(math::max(g->second.first - (block_pos.y + 1) * int(data_block_size), 0));
			}
		}
	}

	if (stream_dependency->stream.is_valid() && !known_absent) {
		PriorityDependency priority_dependency;''')
patch(T, '''				settings.lod_distance
		);

		const bool request_instances = false;''', '''				settings.lod_distance
		);
		priority_dependency.distance_bias = distance_bias;

		const bool request_instances = false;''')
patch(T, '''				nullptr,
				true,
				task_scheduler,
				cancellation_token
		);

	} else {
		ZN_PRINT_WARNING("Requesting a block load when it should not have been necessary");''', '''				nullptr,
				true,
				task_scheduler,
				cancellation_token,
				distance_bias
		);

	} else {
		ZN_PRINT_WARNING("Requesting a block load when it should not have been necessary");''')

# counters for the /holes census: lod 0 data requests and how many of them waited as buried
patch(PH, '''	static inline bool viewer_chain_first = false;
''', '''	static inline bool viewer_chain_first = false;
	// TechnoApe ADR 0136: lod 0 data requests, and those biased as buried (VoxelLodTerrain.debug_request_counts)
	static inline std::atomic<int64_t> requests_lod0{ 0 };
	static inline std::atomic<int64_t> requests_buried{ 0 };
''')
H = 'terrain/variable_lod/voxel_lod_terrain.h'
LC = 'terrain/variable_lod/voxel_lod_terrain.cpp'
patch(H, '''	Dictionary debug_get_data_block_info(Vector3 fbpos, int lod_index) const;
''', '''	Dictionary debug_get_data_block_info(Vector3 fbpos, int lod_index) const;
	Vector2i debug_request_counts() const; // TechnoApe ADR 0136
''')
patch(LC, '''Dictionary VoxelLodTerrain::debug_get_data_block_info(Vector3 fbpos, int lod_index) const {
''', '''Vector2i VoxelLodTerrain::debug_request_counts() const {
	return Vector2i(int(PriorityDependency::requests_lod0.load()), int(PriorityDependency::requests_buried.load()));
}

Dictionary VoxelLodTerrain::debug_get_data_block_info(Vector3 fbpos, int lod_index) const {
''')
patch(LC, '''	ClassDB::bind_method(D_METHOD("debug_get_data_block_info", "block_pos", "lod"), &Self::debug_get_data_block_info);
''', '''	ClassDB::bind_method(D_METHOD("debug_get_data_block_info", "block_pos", "lod"), &Self::debug_get_data_block_info);
	ClassDB::bind_method(D_METHOD("debug_request_counts"), &Self::debug_request_counts);
''')

# the camera's view: a block's priority distance grows with its angle off the camera's forward direction,
# d * (1 + view_priority_weight * (1 - cos)): ahead x1, beside x(1 + w), behind x(1 + 2w). The terrain writes the
# camera every frame (main thread; workers read it unlocked, like the viewer positions); 0 turns it off.
patch(PH, '''	static inline std::atomic<int64_t> requests_buried{ 0 };
''', '''	static inline std::atomic<int64_t> requests_buried{ 0 };
	static inline float view_weight = 0.f;
	static inline bool has_view = false;
	static inline Vector3f view_position;
	static inline Vector3f view_direction;
	static inline Vector3f lead_position; // the camera plus its velocity times LEAD_S: where it will be
''')

patch(LC, '''	push_deferred_mesh_updates(); // TechnoApe P11: new frame, new upload budget
''', '''	push_deferred_mesh_updates(); // TechnoApe P11: new frame, new upload budget
	if (PriorityDependency::view_weight > 0.f) {
		// TechnoApe ADR 0136: task priorities follow the camera's view (voxel_patch_visible_first.py)
		const LocalCameraInfo cam = get_local_camera_info();
		// the camera's velocity, smoothed over ~5 frames; a jump (teleport) or a long frame resets it
		const uint64_t now = Time::get_singleton()->get_ticks_usec();
		const float dt = float(now - _view_prev_usec) * 1e-6f;
		const Vector3 step = cam.position - _view_prev_position;
		if (dt > 0.f && dt < 0.25f && step.length() < 64.f) {
			_view_velocity = _view_velocity.lerp(step / dt, 0.2f);
		} else {
			_view_velocity = Vector3();
		}
		_view_prev_usec = now;
		_view_prev_position = cam.position;
		constexpr float LEAD_S = 1.f;
		constexpr float LEAD_MAX = 256.f; // voxels (32 m)
		const Vector3 lead = (_view_velocity * LEAD_S).limit_length(LEAD_MAX);
		PriorityDependency::view_position = to_vec3f(cam.position);
		PriorityDependency::lead_position = to_vec3f(cam.position + lead);
		PriorityDependency::view_direction = to_vec3f(cam.forward);
		PriorityDependency::has_view = cam.forward != Vector3();
	}
''')
patch(H, '''	Vector2i debug_request_counts() const; // TechnoApe ADR 0136
''', '''	Vector2i debug_request_counts() const; // TechnoApe ADR 0136
	void set_view_priority_weight(float w);
	float get_view_priority_weight() const;
''')
patch(H, '''	uint64_t _mesh_upload_bytes_this_frame = 0;
''', '''	uint64_t _mesh_upload_bytes_this_frame = 0;
	// TechnoApe ADR 0136: the camera's motion for PriorityDependency::lead_position
	uint64_t _view_prev_usec = 0;
	Vector3 _view_prev_position;
	Vector3 _view_velocity;
''')
patch(LC, '''Vector2i VoxelLodTerrain::debug_request_counts() const {''', '''void VoxelLodTerrain::set_view_priority_weight(float w) {
	PriorityDependency::view_weight = math::max(w, 0.f);
}

float VoxelLodTerrain::get_view_priority_weight() const {
	return PriorityDependency::view_weight;
}

Vector2i VoxelLodTerrain::debug_request_counts() const {''')
patch(LC, '''	ClassDB::bind_method(D_METHOD("debug_request_counts"), &Self::debug_request_counts);
''', '''	ClassDB::bind_method(D_METHOD("debug_request_counts"), &Self::debug_request_counts);
	ClassDB::bind_method(D_METHOD("set_view_priority_weight", "weight"), &Self::set_view_priority_weight);
	ClassDB::bind_method(D_METHOD("get_view_priority_weight"), &Self::get_view_priority_weight);
''')

# a mesh block meshes only once every data block of it and the ring around is in: a lod 0 data request takes the
# priority position of the mesh block it belongs to, so the data of the nearest mesh blocks completes together instead
# of every nearby mesh block waiting on a few far data blocks
patch(T, '''void init_sparse_octree_priority_dependency(''', '''// TechnoApe ADR 0136
void set_mesh_block_priority_position(
		PriorityDependency &dep,
		const Vector3i block_pos,
		const int lod_index,
		const int data_block_size,
		const VoxelLodTerrainUpdateData::Settings &settings,
		const Transform3D &volume_transform
) {
	const int ms = 1 << settings.mesh_block_size_po2;
	if (lod_index != 0 || ms <= data_block_size) {
		return;
	}
	const Vector3i o = block_pos * data_block_size;
	const Vector3i c(math::floordiv(o.x, ms) * ms + ms / 2, math::floordiv(o.y, ms) * ms + ms / 2,
			math::floordiv(o.z, ms) * ms + ms / 2);
	dep.world_position = to_vec3f(volume_transform.xform(Vector3(c.x, c.y, c.z)));
}

void init_sparse_octree_priority_dependency(''')
patch(T, '''	params.priority_dependency.distance_bias = distance_bias; // TechnoApe ADR 0136
''', '''	params.priority_dependency.distance_bias = distance_bias; // TechnoApe ADR 0136
	set_mesh_block_priority_position(
			params.priority_dependency, block_pos, lod_index, data_block_size, settings, volume_transform
	);
''')
patch(T, '''		priority_dependency.distance_bias = distance_bias;
''', '''		priority_dependency.distance_bias = distance_bias;
		set_mesh_block_priority_position(
				priority_dependency, block_pos, lod_index, data_block_size, settings, volume_transform
		);
''')

# a lod 0 mesh shows as it is applied: with one LOD the update task activates every loaded block anyway, but only on
# its next pass (up to ~150 ms in flight), and meanwhile the mesh was there and hidden (the census: "hidden")
patch(LC, '''			block->visual_active = visual_active;
			block->set_visible(visual_active);''', '''			// TechnoApe ADR 0136: one LOD: shown now, the update task's activation that follows changes nothing
			const bool show = visual_active ||
					(get_lod_count() == 1 &&
					 _update_data->settings.streaming_system == VoxelLodTerrainUpdateData::STREAMING_SYSTEM_CLIPBOX);
			block->visual_active = show;
			block->set_visible(show);''')

# meshing a block whose data is all in is the last step before it is drawn: lod 0 mesh tasks run before generation at
# the same distance (stock: the same band 2)
patch('constants/voxel_constants.h', '''static const uint8_t TASK_PRIORITY_MESH_BAND2 = 10;''',
      '''static const uint8_t TASK_PRIORITY_MESH_BAND2 = 11; // TechnoApe ADR 0136 (stock 10, as generation)''')
