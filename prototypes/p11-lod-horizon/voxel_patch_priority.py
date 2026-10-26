# Teleport priority for the custom godot_voxel build (ADR 0066). Run with the godot_voxel root as argv[1].
#
# With clipbox streaming a LOD n block only shows under an active LOD n+1 parent (all 8 siblings loaded),
# from the root LOD down. Stock priority puts every finer LOD ahead of every coarser one (band1 =
# MAX_LOD - lod), so after a teleport the coarse blocks over the viewer come last: the full-detail ground
# under the player is meshed in ~1 s but stays hidden until most of the 2 km horizon is generated
# (8-20 s). With VoxelLodTerrain.viewer_chain_first on (the game sets it while a teleport cover is up),
# the blocks within four mesh blocks of the viewer at each LOD (the chain of parents over the player and
# their siblings) go first; everything else keeps the stock order. It is a global switch (all terrains)
# and off by default: always on, it cost flight streaming ~1.3 ms p99 and a 25 ms worst frame (more
# coarse uploads mixed in).
import sys
root = sys.argv[1]


def patch(path, old, new, count=1):
    p = root + path
    s = open(p, encoding='utf-8').read()
    assert s.count(old) == count, (path, s.count(old))
    s = s.replace(old, new, 1)
    open(p, 'w', encoding='utf-8', newline='\n').write(s)


patch('/engine/priority_dependency.h', 'struct PriorityDependency {\n',
      'struct PriorityDependency {\n'
      '\t// TechnoApe P11: the blocks around the viewer at every LOD first (VoxelLodTerrain.viewer_chain_first, ADR 0066)\n'
      '\tstatic inline bool viewer_chain_first = false;\n\n')

patch('/engine/priority_dependency.cpp', '\tpriority.band1 = constants::MAX_LOD - lod_index;',
      '\t// TechnoApe P11: the blocks around the viewer at every LOD first: clipbox shows a LOD only under an\n'
      '\t// active parent, so the coarse chain over the viewer gates the near detail after a teleport (ADR 0066)\n'
      '\tpriority.band1 = viewer_chain_first && distance < (128 << lod_index) ? constants::MAX_LOD + 1\n'
      '\t\t\t\t\t\t\t\t\t\t\t\t\t\t\t\t\t\t\t: constants::MAX_LOD - lod_index;')

patch('/terrain/variable_lod/voxel_lod_terrain.h', '\tfloat get_mesh_upload_budget_mb() const;\n',
      '\tfloat get_mesh_upload_budget_mb() const;\n'
      '\t// TechnoApe P11: PriorityDependency::viewer_chain_first (global, ADR 0066)\n'
      '\tvoid set_viewer_chain_first(bool enabled);\n'
      '\tbool get_viewer_chain_first() const;\n')

patch('/terrain/variable_lod/voxel_lod_terrain.cpp', 'float VoxelLodTerrain::get_mesh_upload_budget_mb() const {',
      'void VoxelLodTerrain::set_viewer_chain_first(bool enabled) {\n'
      '\tPriorityDependency::viewer_chain_first = enabled;\n'
      '}\n\n'
      'bool VoxelLodTerrain::get_viewer_chain_first() const {\n'
      '\treturn PriorityDependency::viewer_chain_first;\n'
      '}\n\n'
      'float VoxelLodTerrain::get_mesh_upload_budget_mb() const {')

patch('/terrain/variable_lod/voxel_lod_terrain.cpp',
      '\tClassDB::bind_method(D_METHOD("get_mesh_upload_budget_mb"), &Self::get_mesh_upload_budget_mb);\n',
      '\tClassDB::bind_method(D_METHOD("get_mesh_upload_budget_mb"), &Self::get_mesh_upload_budget_mb);\n'
      '\tClassDB::bind_method(D_METHOD("set_viewer_chain_first", "enabled"), &Self::set_viewer_chain_first);\n'
      '\tClassDB::bind_method(D_METHOD("get_viewer_chain_first"), &Self::get_viewer_chain_first);\n')

patch('/terrain/variable_lod/voxel_lod_terrain.cpp',
      '\tADD_PROPERTY(PropertyInfo(Variant::FLOAT, "mesh_upload_budget_mb"), "set_mesh_upload_budget_mb", "get_mesh_upload_budget_mb");\n',
      '\tADD_PROPERTY(PropertyInfo(Variant::FLOAT, "mesh_upload_budget_mb"), "set_mesh_upload_budget_mb", "get_mesh_upload_budget_mb");\n'
      '\tADD_PROPERTY(PropertyInfo(Variant::BOOL, "viewer_chain_first"), "set_viewer_chain_first", "get_viewer_chain_first");\n')
print('patched priority')
