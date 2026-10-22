#pragma once
// MicroMesher (ADR 0006): the one path for sub-voxel geometry.
//
// A cell grid (grid^3 cells per voxel, grid = 8 -> 6.25 cm cells) of material
// ids is turned into a mesh of greedy quads: coplanar same-material cells merge
// into one quad, but never across a voxel border, so every quad still maps onto
// a sub-rectangle of its material's atlas tile (a chiseled stone voxel keeps
// the stone texture aligned with its neighbours). Faces between two solid cells
// are skipped; faces on the grid border are drawn (the caller passes a halo if
// it wants culling against neighbours).
//
// Used for the library micro models (vegetation, later slabs/posts/machine
// parts: size = grid^3, origin 0, mesh inside the unit cube) and for chisel
// overlay meshes (size = a whole mesh block in cells).
//
// cells: material id per cell, index x + size.x * (y + size.y * z); 0 = air.
// tiles: material id -> atlas tile index, -1 = not drawn (treated as air).
// Positions are in voxel units (1 voxel = 1.0), origin added.

#include <godot_cpp/classes/array_mesh.hpp>
#include <godot_cpp/classes/ref_counted.hpp>
#include <godot_cpp/variant/array.hpp>
#include <godot_cpp/variant/packed_int32_array.hpp>
#include <godot_cpp/variant/vector2i.hpp>
#include <godot_cpp/variant/vector3.hpp>
#include <godot_cpp/variant/vector3i.hpp>

namespace godot {

class MicroMesher : public RefCounted {
    GDCLASS(MicroMesher, RefCounted)

public:
    // Surface arrays (Mesh::ARRAY_MAX entries) or an empty Array when nothing is visible.
    static Array build_arrays(const PackedInt32Array &cells, Vector3i size, int grid, const PackedInt32Array &tiles, Vector2i atlas, Vector3 origin);
    // Same, as an ArrayMesh with one surface; null when nothing is visible.
    static Ref<ArrayMesh> build(const PackedInt32Array &cells, Vector3i size, int grid, const PackedInt32Array &tiles, Vector2i atlas, Vector3 origin);
    // Fills a box of cells [min, max) with a material (helper for tests and patterns).
    static PackedInt32Array fill_box(PackedInt32Array cells, Vector3i size, Vector3i min, Vector3i max, int material);

protected:
    static void _bind_methods();
};

} // namespace godot
