# --- occlusion computed in the shader, so every cube face merges (ADR 0107) ---
# Part of the TechnoApe godot_voxel patches: run after voxel_patch_merge_faces.py.
# voxel_patch_merge_faces.py could merge only faces whose occlusion (baked into vertex colours) a rectangle's four
# vertices can carry; with occlusion off the box drew 42 % fewer triangles. With VoxelMesherBlocky set_ao_pages(true)
# every face of a plain cube merges with its neighbours of the same model, regardless of occlusion, and the shader
# computes the occlusion per pixel: each mesh block also outputs (as the mesher output's atlas_image) one bit per
# voxel of its block plus one voxel around it, set where the voxel darkens corners (contributes_to_ao), an R8 image
# of edge x (edge * bytes) with edge = block + 2, bytes = ceil(edge / 8): texel (x, z * bytes + y / 8), bit y % 8.
# Merged faces carry UV2 = (1, 0) (the shader computes their occlusion), everything else UV2 = (0, 0) and its
# vertex colours as before. VoxelLodTerrain.set_ao_pages([texture_2d_array_rids], layers_per_texture, layers) gives
# each mesh block a layer (layer n is layer n % layers_per_texture of texture n / layers_per_texture: GPUs allow
# 2048 per array), uploads the image into it and sets the block's `ao_layer` instance uniform. (An earlier version
# packed per-rectangle occlusion texels into pages: forest canopies, shaded nearly everywhere, overflowed them.)
import sys
root = sys.argv[1]
REVERT = len(sys.argv) > 2 and sys.argv[2] == 'revert'

def patch(rel, old, new):
    if REVERT:
        old, new = new, old
    p = root + '/' + rel
    s = open(p, encoding='utf-8').read()
    if s.count(new) == 1 and (new.count(old) or s.count(old) == 0):
        return  # already done
    assert s.count(old) == 1, (rel, old)
    s = s.replace(old, new, 1)
    open(p, 'w', encoding='utf-8', newline='\n').write(s)

H = 'meshers/blocky/voxel_mesher_blocky.h'
patch(H, '''	void set_merge_faces(bool enable);
	bool get_merge_faces() const;
''', '''	void set_merge_faces(bool enable);
	bool get_merge_faces() const;
	// TechnoApe ADR 0107: occlusion computed in the shader from a bit volume per block, so every cube face merges
	void set_ao_pages(bool enable);
	bool get_ao_pages() const;
''')
patch(H, '''		bool merge_faces = true; // TechnoApe ADR 0106
	};''', '''		bool merge_faces = true; // TechnoApe ADR 0106
		bool ao_pages = false; // TechnoApe ADR 0107
	};''')
patch(H, '''		StdVector<Vector2f> uvs;
		StdVector<Color> colors;
		StdVector<int> indices;
		StdVector<float> tangents;

		void clear() {
			positions.clear();
			normals.clear();
			uvs.clear();''', '''		StdVector<Vector2f> uvs;
		StdVector<Vector2f> uvs2; // TechnoApe ADR 0107: (1, 0) on merged faces (may be shorter: padded with (0, 0))
		StdVector<Color> colors;
		StdVector<int> indices;
		StdVector<float> tangents;

		void clear() {
			positions.clear();
			normals.clear();
			uvs.clear();
			uvs2.clear();''')

C = 'meshers/blocky/voxel_mesher_blocky.cpp'
patch(C, '''		const TintSampler tint_sampler,
		const bool merge_faces
) {''', '''		const TintSampler tint_sampler,
		const bool merge_faces,
		StdVector<uint8_t> *ao_volume
) {''')
for old in ['''						tint_sampler,
						params.merge_faces
				);
				if (input.lod_index > 0) {
					blocky::append_skirts(
							raw_channel,''', '''						tint_sampler,
						params.merge_faces
				);
				if (input.lod_index > 0) {
					blocky::append_skirts(model_ids,''']:
    patch(C, old, old.replace('params.merge_faces\n', 'params.merge_faces,\n\t\t\t\t\t\tao_volume\n', 1))

patch(C, '''	StdVector<uint8_t> &merge_sides = get_tls_merge_sides();
	if (merging) {''', '''	StdVector<uint8_t> &merge_sides = get_tls_merge_sides();
	// TechnoApe ADR 0107: with the occlusion volume every face of a plain cube merges, the shader shades it
	const bool paging = merging && ao_volume != nullptr && block_size.x == block_size.y && block_size.y == block_size.z;
	if (merging) {''')
patch(C, '''							const uint8_t code = merge_code(side, shaded_corner);''', '''							const uint8_t code = paging ? 1 : merge_code(side, shaded_corner);''')
patch(C, '''					for (unsigned int i = 0; i < 4; ++i) {
						const Vector3f &v = ss.positions[i];
						arrays.positions.push_back(Vector3f(v.x * scale[0], v.y * scale[1], v.z * scale[2]) + pos);
						arrays.uvs.push_back(ss.uvs[i]);
						arrays.normals.push_back(to_vec3f(nrm));
						// the single face's vertex colour (meshing above): 1 - darkness * occlusion at that corner
						const float c[3] = { v.x, v.y, v.z };''', '''					if (paging) {
						// TechnoApe ADR 0107: the shader shades this face (UV2.x = 1); vertices before it read 0
						arrays.uvs2.resize(arrays.positions.size(), Vector2f(0.f, 0.f));
					}
					for (unsigned int i = 0; i < 4; ++i) {
						const Vector3f &v = ss.positions[i];
						arrays.positions.push_back(Vector3f(v.x * scale[0], v.y * scale[1], v.z * scale[2]) + pos);
						arrays.uvs.push_back(ss.uvs[i]);
						arrays.normals.push_back(to_vec3f(nrm));
						if (paging) {
							arrays.uvs2.push_back(Vector2f(1.f, 0.f));
							arrays.colors.push_back(voxel.color);
							continue;
						}
						// the single face's vertex colour (meshing above): 1 - darkness * occlusion at that corner
						const float c[3] = { v.x, v.y, v.z };''')

# the bit volume, after the merge pass
patch(C, '''	if (!merging) {
		return;
	}''', '''	if (paging) {
		// TechnoApe ADR 0107: one bit per voxel of the block and one voxel around it, set where it darkens corners
		const int edge = block_size.x - 2 * VoxelMesherBlocky::PADDING + 2;
		const int bytes = (edge + 7) / 8;
		ao_volume->assign(edge * edge * bytes, 0);
		const int o = VoxelMesherBlocky::PADDING - 1;
		for (int z = 0; z < edge; ++z) {
			for (int x = 0; x < edge; ++x) {
				uint8_t *col = ao_volume->data() + (z * bytes) * edge + x;
				for (int y = 0; y < edge; ++y) {
					const uint32_t id = type_buffer[(y + o) + (x + o) * row_size + (z + o) * deck_size];
					if (contributes_to_ao(library, id)) {
						col[(y >> 3) * edge] |= uint8_t(1 << (y & 7));
					}
				}
			}
		}
	}

	if (!merging) {
		return;
	}''')

patch(C, '''void VoxelMesherBlocky::build(VoxelMesher::Output &output, const VoxelMesher::Input &input) {''', '''void VoxelMesherBlocky::set_ao_pages(bool enable) {
	RWLockWrite wlock(_parameters_lock);
	_parameters.ao_pages = enable;
}

bool VoxelMesherBlocky::get_ao_pages() const {
	RWLockRead rlock(_parameters_lock);
	return _parameters.ao_pages;
}

void VoxelMesherBlocky::build(VoxelMesher::Output &output, const VoxelMesher::Input &input) {''')

patch(C, '''		const blocky::TintSampler tint_sampler =
				blocky::TintSampler::create(voxels, static_cast<blocky::TintSampler::Mode>(params.tint_mode));''', '''		const blocky::TintSampler tint_sampler =
				blocky::TintSampler::create(voxels, static_cast<blocky::TintSampler::Mode>(params.tint_mode));
		// TechnoApe ADR 0107
		static thread_local StdVector<uint8_t> tls_ao_volume;
		StdVector<uint8_t> *ao_volume = nullptr;
		if (params.ao_pages && params.merge_faces && params.bake_occlusion && !tint_sampler.is_valid() &&
			input.lod_index == 0) {
			ao_volume = &tls_ao_volume;
		}''')

patch(C, '''				ERR_PRINT("Unsupported voxel depth");
				return;
		}
''', '''				ERR_PRINT("Unsupported voxel depth");
				return;
		}
		if (ao_volume != nullptr && ao_volume->size() > 0 && voxels.get_size().x == voxels.get_size().y &&
			voxels.get_size().y == voxels.get_size().z) {
			// TechnoApe ADR 0107: the block's occlusion volume as an R8 image, edge x (edge * bytes)
			const int edge = voxels.get_size().x - 2 * PADDING + 2;
			PackedByteArray bytes;
			bytes.resize(ao_volume->size());
			memcpy(bytes.ptrw(), ao_volume->data(), ao_volume->size());
			output.atlas_image = Image::create_from_data(edge, int(ao_volume->size()) / edge, false, Image::FORMAT_R8, bytes);
		}
''')

patch(C, '''				mesh_arrays[Mesh::ARRAY_TEX_UV] = uvs;''', '''				mesh_arrays[Mesh::ARRAY_TEX_UV] = uvs;
				if (arrays.uvs2.size() > 0) {
					// TechnoApe ADR 0107: vertices after the last merged face read (0, 0) too
					StdVector<Vector2f> uv2 = arrays.uvs2;
					uv2.resize(arrays.positions.size(), Vector2f(0.f, 0.f));
					PackedVector2Array uvs2;
					copy_to(uvs2, to_span_const(uv2));
					mesh_arrays[Mesh::ARRAY_TEX_UV2] = uvs2;
				}''')
patch(C, '''#include "../../util/godot/core/packed_arrays.h"''', '''#include "../../util/godot/core/packed_arrays.h"
#include "../../util/godot/classes/image.h"''')
patch(C, '''	ClassDB::bind_method(D_METHOD("get_merge_faces"), &VoxelMesherBlocky::get_merge_faces);''', '''	ClassDB::bind_method(D_METHOD("get_merge_faces"), &VoxelMesherBlocky::get_merge_faces);
	ClassDB::bind_method(D_METHOD("set_ao_pages", "enable"), &VoxelMesherBlocky::set_ao_pages);
	ClassDB::bind_method(D_METHOD("get_ao_pages"), &VoxelMesherBlocky::get_ao_pages);''')

# --- the terrain hands out layers ---
MB = 'terrain/voxel_mesh_block.h'
patch(MB, '''	Ref<Mesh> get_mesh() const;
	bool has_mesh() const;''', '''	Ref<Mesh> get_mesh() const;
	bool has_mesh() const;
	// TechnoApe ADR 0107
	void set_instance_shader_parameter(StringName key, Variant value) {
		_mesh_instance.set_shader_instance_parameter(key, value);
	}''')

TH = 'terrain/variable_lod/voxel_lod_terrain.h'
patch(TH, '''	void set_shadow_caster_distance(float d);''', '''	void set_shadow_caster_distance(float d);
	// TechnoApe ADR 0107: Texture2DArrays (RIDs) of occlusion pages; layer 0 is never handed out
	void set_ao_pages(Array textures, int layers_per_texture, int layers);
	int get_ao_pages_used() const;
	StdVector<RID> _ao_pages;
	int _ao_layers_per_texture = 1;
	StdVector<int> _ao_free_layers;
	StdUnorderedMap<Vector3i, int> _ao_layer_of;
	bool _ao_full_warned = false;
	uint64_t _ao_collect_usec = 0;
	void collect_ao_layers();''')
s = open(root + '/' + TH, encoding='utf-8').read()
if 'std_unordered_map.h' not in s and not REVERT:
    s = s.replace('#include', '#include "../../util/containers/std_unordered_map.h"\n#include', 1)
    open(root + '/' + TH, 'w', encoding='utf-8', newline='\n').write(s)

TC = 'terrain/variable_lod/voxel_lod_terrain.cpp'
patch(TC, '''float VoxelLodTerrain::get_shadow_caster_distance() const {''', '''void VoxelLodTerrain::set_ao_pages(Array textures, int layers_per_texture, int layers) {
	_ao_pages.clear();
	for (int i = 0; i < textures.size(); ++i) {
		_ao_pages.push_back(textures[i]);
	}
	_ao_layers_per_texture = math::max(layers_per_texture, 1);
	_ao_free_layers.clear();
	_ao_layer_of.clear();
	for (int i = layers - 1; i >= 1; --i) {
		_ao_free_layers.push_back(i);
	}
}

int VoxelLodTerrain::get_ao_pages_used() const {
	return int(_ao_layer_of.size());
}

// Layers of blocks that are gone go back to the free list.
void VoxelLodTerrain::collect_ao_layers() {
	for (auto it = _ao_layer_of.begin(); it != _ao_layer_of.end();) {
		if (_mesh_maps_per_lod[0].get_block(it->first) == nullptr) {
			_ao_free_layers.push_back(it->second);
			it = _ao_layer_of.erase(it);
		} else {
			++it;
		}
	}
}

float VoxelLodTerrain::get_shadow_caster_distance() const {''')
patch(TC, '''	if (_shadow_caster_distance > 0.f) {
		update_shadow_casters(); // TechnoApe ADR 0105
	}''', '''	if (_shadow_caster_distance > 0.f) {
		update_shadow_casters(); // TechnoApe ADR 0105
	}
	if (!_ao_pages.empty() && _shadow_caster_passes % 5 == 0 && _shadow_caster_next_usec != _ao_collect_usec) {
		_ao_collect_usec = _shadow_caster_next_usec;
		collect_ao_layers(); // TechnoApe ADR 0107: about once a second
	}''')

s = open(root + '/' + TC, encoding='utf-8').read()
anchor = '''		block->set_mesh(
				mesh,
				get_gi_mode(),'''
assert s.count(anchor) == 1 or REVERT
patch(TC, anchor, '''		// TechnoApe ADR 0107: the block's occlusion page into its layer of the texture array
		int ao_layer = 0;
		if (!_ao_pages.empty() && ob.surfaces.atlas_image.is_valid() && ob.lod == 0) {
			auto it = _ao_layer_of.find(ob.position);
			if (it != _ao_layer_of.end()) {
				ao_layer = it->second;
			} else if (!_ao_free_layers.empty()) {
				ao_layer = _ao_free_layers.back();
				_ao_free_layers.pop_back();
				_ao_layer_of[ob.position] = ao_layer;
			} else if (!_ao_full_warned) {
				_ao_full_warned = true;
				ZN_PRINT_WARNING("Out of occlusion page layers (ADR 0107): blocks draw without occlusion");
			}
			if (ao_layer > 0) {
				const int t = ao_layer / _ao_layers_per_texture;
				if (t < int(_ao_pages.size())) {
					RenderingServer::get_singleton()->texture_2d_update(
							_ao_pages[t], ob.surfaces.atlas_image, ao_layer % _ao_layers_per_texture
					);
				}
			}
		}

''' + anchor)
# after set_mesh the instance exists
s = open(root + '/' + TC, encoding='utf-8').read()
k = s.index('// TechnoApe ADR 0107: the block\'s occlusion page') if not REVERT else 0
if not REVERT and 'set_instance_shader_parameter("ao_layer"' not in s:
    j = s.index('block->set_mesh(', k)
    e = s.index(');', j) + 2
    s = s[:e] + '''
		if (!_ao_pages.empty() && mesh.is_valid()) {
			block->set_instance_shader_parameter("ao_layer", ao_layer); // TechnoApe ADR 0107
		}''' + s[e:]
    open(root + '/' + TC, 'w', encoding='utf-8', newline='\n').write(s)
else:
    s = s.replace('''
		if (!_ao_pages.empty() && mesh.is_valid()) {
			block->set_instance_shader_parameter("ao_layer", ao_layer); // TechnoApe ADR 0107
		}''', '', 1)
    open(root + '/' + TC, 'w', encoding='utf-8', newline='\n').write(s)
patch(TC, '''	ClassDB::bind_method(D_METHOD("get_shadow_caster_distance"), &Self::get_shadow_caster_distance);''', '''	ClassDB::bind_method(D_METHOD("get_shadow_caster_distance"), &Self::get_shadow_caster_distance);
	ClassDB::bind_method(D_METHOD("set_ao_pages", "textures", "layers_per_texture", "layers"), &Self::set_ao_pages);
	ClassDB::bind_method(D_METHOD("get_ao_pages_used"), &Self::get_ao_pages_used);''')
