# --- the full-detail data map is sized for the box up front (ADR 0105) ---
# Part of the TechnoApe godot_voxel patches: run in any order.
# In 100 m flight the main thread waited 13-33 ms a few times per 20 s in try_set_block: the LOD 0 data map is a
# std::unordered_map of up to ~1 M blocks, and an insert past a new size peak rehashed all of it under the map's write
# lock. VoxelLodTerrain now reserves buckets for the whole box ((2 view / block + 3)^3, capped at 4 M) when the view
# distance is set and when the maps are reset, so the map never rehashes while streaming.
import sys
root = sys.argv[1]

def patch(rel, old, new):
    p = root + '/' + rel
    s = open(p, encoding='utf-8').read()
    if s.count(new) == 1 and (new.count(old) or s.count(old) == 0):
        return  # already done
    assert s.count(old) == 1, (rel, old)
    s = s.replace(old, new, 1)
    open(p, 'w', encoding='utf-8', newline='\n').write(s)

patch('storage/voxel_data_map.h', '''	StdUnorderedMap<Vector3i, VoxelDataBlock> _blocks_map;
''', '''	StdUnorderedMap<Vector3i, VoxelDataBlock> _blocks_map;

public:
	// TechnoApe ADR 0105: buckets for n blocks, so inserts never rehash the whole map under its lock
	void reserve(size_t n) {
		_blocks_map.reserve(n);
	}

private:
''')
patch('storage/voxel_data.h', '''	bool has_block(Vector3i bpos, unsigned int lod_index) const;''', '''	bool has_block(Vector3i bpos, unsigned int lod_index) const;
	// TechnoApe ADR 0105
	void reserve_blocks(unsigned int lod_index, size_t n) {
		Lod &lod = _lods[lod_index];
		RWLockWrite wlock(lod.map_lock);
		lod.map.reserve(n);
	}''')
C = 'terrain/variable_lod/voxel_lod_terrain.cpp'
patch(C, '''	_update_data->settings.view_distance_voxels = p_distance_in_voxels;
	_update_data->state.octree_streaming.force_update_octrees_next_update = true;
}''', '''	_update_data->settings.view_distance_voxels = p_distance_in_voxels;
	_update_data->state.octree_streaming.force_update_octrees_next_update = true;
	reserve_data_map(); // TechnoApe ADR 0105
}

// TechnoApe ADR 0105: buckets for every data block of the full-detail box
void VoxelLodTerrain::reserve_data_map() {
	if (_data == nullptr || _update_data == nullptr) {
		return;
	}
	const uint64_t across = uint64_t(2 * _update_data->settings.view_distance_voxels / get_data_block_size() + 3);
	_data->reserve_blocks(0, size_t(math::min(across * across * across, uint64_t(4) << 20)));
}''')
patch(C, '''void VoxelLodTerrain::reset_maps() {''', '''void VoxelLodTerrain::reset_maps() {
	struct ReserveAfter { // TechnoApe ADR 0105: the reset maps get their buckets back
		VoxelLodTerrain *self;
		~ReserveAfter() { self->reserve_data_map(); }
	} reserve_after{ this };''')
patch('terrain/variable_lod/voxel_lod_terrain.h', '''	void set_shadow_caster_distance(float d);''', '''	void set_shadow_caster_distance(float d);
	void reserve_data_map(); // TechnoApe ADR 0105''')
