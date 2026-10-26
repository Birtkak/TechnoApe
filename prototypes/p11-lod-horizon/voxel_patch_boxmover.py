# Patch for the custom godot_voxel build (ADR 0067): VoxelBoxMover reads the voxels of its query box
# block by block instead of one locked VoxelData::get_voxel per voxel. A bear's box (1.4 m = 11 voxels
# a side, plus its motion) took ~0.5 ms a move that way; 100 bears held the game at 4 fps (bug report
# 2026-10-25 21-11-51). Blocks without LOD0 data fall back to get_voxel per voxel, so what the mover
# sees is unchanged. Run with the godot_voxel root as argv[1], after voxel_patch.py.
import sys
root = sys.argv[1]

p = root + '/terrain/fixed_lod/voxel_box_mover.cpp'
s = open(p, encoding='utf-8').read()
old = '''	const blocky::BakedLibrary &baked_data = library_ref->get_baked_data();

	Vector3i i = minp;

	// TODO Optimization: read the whole box of voxels at once, querying individually is slower
	for (i.z = minp.z; i.z < maxp.z; ++i.z) {
		for (i.y = minp.y; i.y < maxp.y; ++i.y) {
			for (i.x = minp.x; i.x < maxp.x; ++i.x) {
				const int type_id = voxels.get_voxel(i, channel, defval).i;

				if (baked_data.has_model(type_id)) {
					const blocky::BakedModel &model = baked_data.models[type_id];

					if ((model.box_collision_mask & collision_mask) == 0) {
						continue;
					}

					for (const AABB &aabb : model.box_collision_aabbs) {
						AABB world_box = aabb;
						world_box.position += i;
						potential_boxes.push_back(world_box);
					}
				}
			}
		}
	}
}
'''
new = '''	const blocky::BakedLibrary &baked_data = library_ref->get_baked_data();

	auto add_boxes = [&baked_data, collision_mask, &potential_boxes](const int type_id, const Vector3i i) {
		if (baked_data.has_model(type_id)) {
			const blocky::BakedModel &model = baked_data.models[type_id];
			if ((model.box_collision_mask & collision_mask) == 0) {
				return;
			}
			for (const AABB &aabb : model.box_collision_aabbs) {
				AABB world_box = aabb;
				world_box.position += i;
				potential_boxes.push_back(world_box);
			}
		}
	};

	// TechnoApe (ADR 0067): one lookup per LOD0 block instead of one locked get_voxel per voxel
	const Box3i vbox = Box3i::from_min_max(minp, maxp);
	if (vbox.size.x <= 0 || vbox.size.y <= 0 || vbox.size.z <= 0) {
		return;
	}
	const int bs = voxels.get_block_size();
	const Box3i bbox = vbox.downscaled(bs);
	StdVector<std::shared_ptr<VoxelBuffer>> blocks;
	blocks.resize(Vector3iUtil::get_volume_u64(bbox.size));
	voxels.get_blocks_with_voxel_data(bbox, 0, to_span(blocks));

	unsigned int index = 0;
	bbox.for_each_cell_zxy([&](const Vector3i bpos) {
		const std::shared_ptr<VoxelBuffer> &buf = blocks[index];
		++index;
		const Vector3i origin = bpos * bs;
		const Box3i sub = Box3i(origin, Vector3i(bs, bs, bs)).clipped(vbox);
		const Vector3i smin = sub.position;
		const Vector3i smax = sub.position + sub.size;
		Vector3i i;
		if (buf == nullptr) {
			// not loaded at LOD0: the old path (lower LODs, generator) voxel by voxel
			for (i.z = smin.z; i.z < smax.z; ++i.z) {
				for (i.y = smin.y; i.y < smax.y; ++i.y) {
					for (i.x = smin.x; i.x < smax.x; ++i.x) {
						add_boxes(voxels.get_voxel(i, channel, defval).i, i);
					}
				}
			}
		} else if (buf->is_uniform(channel)) {
			const int type_id = buf->get_voxel(0, 0, 0, channel);
			if (!baked_data.has_model(type_id) ||
				(baked_data.models[type_id].box_collision_mask & collision_mask) == 0) {
				return;
			}
			for (i.z = smin.z; i.z < smax.z; ++i.z) {
				for (i.y = smin.y; i.y < smax.y; ++i.y) {
					for (i.x = smin.x; i.x < smax.x; ++i.x) {
						add_boxes(type_id, i);
					}
				}
			}
		} else {
			for (i.z = smin.z; i.z < smax.z; ++i.z) {
				for (i.y = smin.y; i.y < smax.y; ++i.y) {
					for (i.x = smin.x; i.x < smax.x; ++i.x) {
						add_boxes(buf->get_voxel(i - origin, channel), i);
					}
				}
			}
		}
	});
}
'''
assert s.count(old) == 1, s.count(old)
s = s.replace(old, new, 1)
open(p, 'w', encoding='utf-8', newline='\n').write(s)
print('box mover patched')
