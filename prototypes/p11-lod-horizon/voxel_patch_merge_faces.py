# --- merged cube faces (ADR 0106) ---
# Part of the TechnoApe godot_voxel patches: run after voxel_patch_fluid.py.
# The blocky mesher drew two triangles for every visible voxel face, and at 12.5 cm voxels a flat field or a cliff
# is thousands of identical faces: the GPU's triangle count set the floor of a 72-100 m box. Faces of plain cubes
# (one surface, no inside geometry, not a fluid, not transparent, full side) are now merged per side and slice
# (greedy meshing) when the result shades exactly as the single faces did: faces with the same ambient occlusion on all
# four corners grow into rectangles, faces whose occlusion changes only across one axis (the step of a hillside, the
# foot of a wall) grow into strips along the other. The terrain shader picks each voxel's colour from its world
# position, so the picture does not change. VoxelMesherBlocky.set_merge_faces(false) turns it off; a tint mode turns it off too (colours per voxel).
import sys
root = sys.argv[1]

REVERT = len(sys.argv) > 2 and sys.argv[2] == 'revert'

def patch(rel, old, new):
    if REVERT:
        old, new = new, old
    p = root + '/' + rel
    s = open(p, encoding='utf-8').read()
    assert s.count(old) == 1, (rel, old)
    s = s.replace(old, new, 1)
    open(p, 'w', encoding='utf-8', newline='\n').write(s)

H = 'meshers/blocky/voxel_mesher_blocky.h'
patch(H, '''	TintMode get_tint_mode() const;
	void set_tint_mode(const TintMode new_mode);
''', '''	TintMode get_tint_mode() const;
	void set_tint_mode(const TintMode new_mode);

	// TechnoApe ADR 0106: merge unshaded faces of plain cubes into rectangles
	void set_merge_faces(bool enable);
	bool get_merge_faces() const;
''')
patch(H, '''		TintMode tint_mode = TINT_NONE;
	};''', '''		TintMode tint_mode = TINT_NONE;
		bool merge_faces = true; // TechnoApe ADR 0106
	};''')

C = 'meshers/blocky/voxel_mesher_blocky.cpp'
patch(C, '''StdVector<int> &get_tls_index_offsets() {
	static thread_local StdVector<int> tls_index_offsets;
	return tls_index_offsets;
}
''', '''StdVector<int> &get_tls_index_offsets() {
	static thread_local StdVector<int> tls_index_offsets;
	return tls_index_offsets;
}

// TechnoApe ADR 0106: per voxel and side, the merge code of a face that waits for the merge pass (0 = none)
StdVector<uint8_t> &get_tls_merge_mask() {
	static thread_local StdVector<uint8_t> tls_merge_mask;
	return tls_merge_mask;
}

// per model, the sides that may merge (0xff = not computed yet)
StdVector<uint8_t> &get_tls_merge_sides() {
	static thread_local StdVector<uint8_t> tls_merge_sides;
	return tls_merge_sides;
}

// Sides of a model whose face is the full unit square of a plain cube: one surface, nothing inside, not a fluid,
// opaque, no cutouts.
uint8_t compute_merge_sides(const BakedModel &v) {
	const BakedModel::Model &m = v.model;
	if (v.empty || v.fluid_index != NULL_FLUID_INDEX || v.cutout_sides_enabled || v.is_transparent ||
		m.surface_count != 1 || m.surfaces[0].positions.size() != 0) {
		return 0;
	}
	uint8_t sides = 0;
	for (unsigned int side = 0; side < Cube::SIDE_COUNT; ++side) {
		const BakedModel::SideSurface &s = m.sides_surfaces[side][0];
		if ((m.full_sides_mask & (1 << side)) == 0 || s.positions.size() != 4 || s.indices.size() != 6) {
			continue;
		}
		bool unit = true;
		for (const Vector3f &p : s.positions) {
			const float c[3] = { p.x, p.y, p.z };
			for (unsigned int a = 0; a < 3; ++a) {
				unit = unit && (c[a] == 0.f || c[a] == 1.f);
			}
		}
		if (unit) {
			sides |= 1 << side;
		}
	}
	return sides;
}

// The face's axes: the normal, then u and v across it.
inline void side_axes(unsigned int side, int &na, int &ua, int &va) {
	const Vector3i n = Cube::g_side_normals[side];
	na = n.x != 0 ? 0 : (n.y != 0 ? 1 : 2);
	ua = (na + 1) % 3;
	va = (na + 2) % 3;
}

// 1-4: the same occlusion a on every corner (merges both ways); 5-20: a at v=0, b at v=1 on both u (merges along u);
// 21-36: a at u=0, b at u=1 on both v (merges along v); 0: does not merge.
inline uint8_t merge_code(unsigned int side, const int8_t shaded_corner[8]) {
	int na, ua, va;
	side_axes(side, na, ua, va);
	int ao[2][2];
	for (unsigned int j = 0; j < 4; ++j) {
		const unsigned int corner = Cube::g_side_corners[side][j];
		const Vector3f cp = Cube::g_corner_position[corner];
		const float c[3] = { cp.x, cp.y, cp.z };
		ao[c[ua] > 0.5f][c[va] > 0.5f] = shaded_corner[corner];
	}
	if (ao[0][0] == ao[1][0] && ao[0][1] == ao[1][1]) {
		return ao[0][0] == ao[0][1] ? 1 + ao[0][0] : 5 + ao[0][0] * 4 + ao[0][1];
	}
	if (ao[0][0] == ao[0][1] && ao[1][0] == ao[1][1]) {
		return 21 + ao[0][0] * 4 + ao[1][0];
	}
	return 0;
}

// Occlusion at the face's corner (cu, cv) for a merge code.
inline int merge_code_ao(uint8_t code, int cu, int cv) {
	if (code <= 4) {
		return code - 1;
	}
	if (code <= 20) {
		return cv == 0 ? (code - 5) / 4 : (code - 5) % 4;
	}
	return cu == 0 ? (code - 21) / 4 : (code - 21) % 4;
}
''')

# the parameter
for old in ['''						baked_occlusion_darkness,
						tint_sampler
				);
				if (input.lod_index > 0) {
					blocky::append_skirts(
							raw_channel,''', '''						baked_occlusion_darkness,
						tint_sampler
				);
				if (input.lod_index > 0) {
					blocky::append_skirts(model_ids,''']:
    patch(C, old, old.replace('tint_sampler\n', 'tint_sampler,\n\t\t\t\t\t\tparams.merge_faces\n', 1))
patch(C, '''		const float baked_occlusion_darkness,
		const TintSampler tint_sampler
) {''', '''		const float baked_occlusion_darkness,
		const TintSampler tint_sampler,
		const bool merge_faces
) {''')

# set up the buffers
patch(C, '''	int collision_surface_index_offset = 0;
''', '''	int collision_surface_index_offset = 0;

	// TechnoApe ADR 0106: faces that merge are marked here and drawn after the voxel loop
	const bool merging = merge_faces && !tint_sampler.is_valid();
	StdVector<uint8_t> &merge_mask = get_tls_merge_mask();
	StdVector<uint8_t> &merge_sides = get_tls_merge_sides();
	if (merging) {
		merge_mask.assign(type_buffer.size() * Cube::SIDE_COUNT, 0);
		merge_sides.assign(library.models.size(), 0xff);
	}
''')

# mark a face instead of drawing it
patch(C, '''					// Subtracting 1 because the data is padded
''', '''					if (merging) {
						uint8_t &ms = merge_sides[voxel_id];
						if (ms == 0xff) {
							ms = compute_merge_sides(voxel);
						}
						if ((ms & (1 << side)) != 0) {
							const uint8_t code = merge_code(side, shaded_corner);
							if (code != 0) {
								merge_mask[voxel_index * Cube::SIDE_COUNT + side] = code;
								continue;
							}
						}
					}

					// Subtracting 1 because the data is padded
''')

# the merge pass
patch(C, '''					index_offset += vertex_count;
				}
			}
		}
	}
}
''', '''					index_offset += vertex_count;
				}
			}
		}
	}

	if (!merging) {
		return;
	}
	// TechnoApe ADR 0106: per side and slice, grow each marked face along u, then rows along v, into one rectangle
	// (a strip when its occlusion changes across the other axis)
	const int stride[3] = { row_size, 1, deck_size };
	const int lo[3] = { min.x, min.y, min.z };
	const int hi[3] = { max.x, max.y, max.z };
	const int ns = Cube::SIDE_COUNT;
	for (unsigned int side = 0; side < Cube::SIDE_COUNT; ++side) {
		const Vector3i nrm = Cube::g_side_normals[side];
		int na, ua, va;
		side_axes(side, na, ua, va);
		const int su = stride[ua];
		const int sv = stride[va];
		int p[3];
		for (p[na] = lo[na]; p[na] < hi[na]; ++p[na]) {
			for (p[va] = lo[va]; p[va] < hi[va]; ++p[va]) {
				for (p[ua] = lo[ua]; p[ua] < hi[ua]; ++p[ua]) {
					const int i0 = p[0] * row_size + p[1] + p[2] * deck_size;
					const uint8_t code = merge_mask[i0 * ns + side];
					if (code == 0) {
						continue;
					}
					const uint32_t id = type_buffer[i0];
					int w = 1;
					if (code <= 20) {
						while (p[ua] + w < hi[ua] && merge_mask[(i0 + w * su) * ns + side] == code &&
							   type_buffer[i0 + w * su] == id) {
							++w;
						}
					}
					int h = 1;
					if (code <= 4 || code > 20) {
						for (; p[va] + h < hi[va]; ++h) {
							const int row = i0 + h * sv;
							int k = 0;
							while (k < w && merge_mask[(row + k * su) * ns + side] == code && type_buffer[row + k * su] == id) {
								++k;
							}
							if (k < w) {
								break;
							}
						}
					}
					for (int b = 0; b < h; ++b) {
						for (int a = 0; a < w; ++a) {
							merge_mask[(i0 + b * sv + a * su) * ns + side] = 0;
						}
					}

					const BakedModel &voxel = library.models[id];
					const BakedModel::Surface &surface = voxel.model.surfaces[0];
					const BakedModel::SideSurface &ss = voxel.model.sides_surfaces[side][0];
					VoxelMesherBlocky::Arrays &arrays = out_arrays_per_material[surface.material_id];
					ZN_ASSERT(surface.material_id < index_offsets.size());
					int &index_offset = index_offsets[surface.material_id];
					float scale[3] = { 1.f, 1.f, 1.f };
					scale[ua] = float(w);
					scale[va] = float(h);
					const Vector3f pos(
							p[0] - VoxelMesherBlocky::PADDING, p[1] - VoxelMesherBlocky::PADDING, p[2] - VoxelMesherBlocky::PADDING
					);
					for (unsigned int i = 0; i < 4; ++i) {
						const Vector3f &v = ss.positions[i];
						arrays.positions.push_back(Vector3f(v.x * scale[0], v.y * scale[1], v.z * scale[2]) + pos);
						arrays.uvs.push_back(ss.uvs[i]);
						arrays.normals.push_back(to_vec3f(nrm));
						// the single face's vertex colour (meshing above): 1 - darkness * occlusion at that corner
						const float c[3] = { v.x, v.y, v.z };
						const float gs = 1.f - baked_occlusion_darkness * float(merge_code_ao(code, c[ua] > 0.5f, c[va] > 0.5f));
						arrays.colors.push_back(Color(gs, gs, gs) * voxel.color);
					}
					for (const float t : ss.tangents) {
						arrays.tangents.push_back(t);
					}
					for (const int j : ss.indices) {
						arrays.indices.push_back(index_offset + j);
					}
					if (collision_surface != nullptr && surface.collision_enabled) {
						for (unsigned int i = 0; i < 4; ++i) {
							const Vector3f &v = ss.positions[i];
							collision_surface->positions.push_back(
									Vector3f(v.x * scale[0], v.y * scale[1], v.z * scale[2]) + pos
							);
						}
						for (const int j : ss.indices) {
							collision_surface->indices.push_back(collision_surface_index_offset + j);
						}
						collision_surface_index_offset += 4;
					}
					index_offset += 4;
				}
			}
		}
	}
}
''')

patch(C, '''void VoxelMesherBlocky::build(VoxelMesher::Output &output, const VoxelMesher::Input &input) {''', '''void VoxelMesherBlocky::set_merge_faces(bool enable) {
	RWLockWrite wlock(_parameters_lock);
	_parameters.merge_faces = enable;
}

bool VoxelMesherBlocky::get_merge_faces() const {
	RWLockRead rlock(_parameters_lock);
	return _parameters.merge_faces;
}

void VoxelMesherBlocky::build(VoxelMesher::Output &output, const VoxelMesher::Input &input) {''')
patch(C, '''	ClassDB::bind_method(D_METHOD("get_tint_mode"), &VoxelMesherBlocky::get_tint_mode);''', '''	ClassDB::bind_method(D_METHOD("get_tint_mode"), &VoxelMesherBlocky::get_tint_mode);
	ClassDB::bind_method(D_METHOD("set_merge_faces", "enable"), &VoxelMesherBlocky::set_merge_faces);
	ClassDB::bind_method(D_METHOD("get_merge_faces"), &VoxelMesherBlocky::get_merge_faces);''')
