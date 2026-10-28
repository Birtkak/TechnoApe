# --- 64-voxel mesh blocks (ADR 0105) ---
# Part of the TechnoApe godot_voxel patches: run in any order.
# Draw calls and the main thread's per-mesh upload grow with the number of mesh blocks: a 100 m box drew 11.5 k of
# them at 32 voxels. VoxelLodTerrain allowed only 16 and 32; the clipbox code works with any power-of-two multiple
# of the data block size, so 64 is allowed too (an edit then remeshes a 64-voxel block).
import sys
root = sys.argv[1]

def patch(rel, old, new):
    p = root + '/' + rel
    s = open(p, encoding='utf-8').read()
    assert s.count(old) == 1, (rel, old)
    s = s.replace(old, new, 1)
    open(p, 'w', encoding='utf-8', newline='\n').write(s)

patch('constants/voxel_constants.h', 'static const unsigned int MAX_BLOCK_SIZE = 32;', 'static const unsigned int MAX_BLOCK_SIZE = 64; // TechnoApe: 64-voxel mesh blocks (ADR 0105)')
# a 64-voxel mesh block reads (64 / 16 + 2)^3 = 216 data blocks (the task counts them in a uint8_t)
patch('constants/voxel_constants.h', 'static const unsigned int MAX_BLOCK_COUNT_PER_REQUEST = 4 * 4 * 4;', 'static const unsigned int MAX_BLOCK_COUNT_PER_REQUEST = 6 * 6 * 6; // TechnoApe: 64-voxel mesh blocks')
patch('terrain/variable_lod/voxel_lod_terrain.cpp', '''		case 32:
			po2 = 5;
			break;
		default:
			mesh_block_size = 16;''', '''		case 32:
			po2 = 5;
			break;
		case 64: // TechnoApe ADR 0105
			po2 = 6;
			break;
		default:
			mesh_block_size = 16;''')
patch('meshers/mesh_block_task.cpp', '''		case 4 * 4 * 4:
			edge_size = 4;
			mesh_block_size_factor = 2;
			break;
		default:
			ZN_PRINT_ERROR("Unsupported block count");''', '''		case 4 * 4 * 4:
			edge_size = 4;
			mesh_block_size_factor = 2;
			break;
		case 6 * 6 * 6: // TechnoApe ADR 0105: a 64-voxel mesh block over 16-voxel data blocks
			edge_size = 6;
			mesh_block_size_factor = 4;
			break;
		default:
			ZN_PRINT_ERROR("Unsupported block count");''')
