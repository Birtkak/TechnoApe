#pragma once
// ADR 0038: shape building. Rasterizes the build shapes (box, wall, cylinder, cone, sphere, stairs,
// slope, or a prefab's material grid) into world voxels, builds their preview mesh, and writes them
// into a SimWorld, so GDScript only orchestrates (principle 8).
//
// A shape lives in a local box of `size` voxels (x width, y height, z depth). `rot` (0..23) is one of
// the 24 cube rotations, `mirror` flips x after it. The oriented shape stands on `lo` (its world min
// corner): the placement is always an axis-aligned world box [lo, lo + oriented size).

#include "sim_world.h"

#include <godot_cpp/classes/ref_counted.hpp>
#include <godot_cpp/variant/array.hpp>
#include <godot_cpp/variant/dictionary.hpp>
#include <godot_cpp/variant/packed_byte_array.hpp>
#include <godot_cpp/variant/packed_int32_array.hpp>
#include <godot_cpp/variant/vector3i.hpp>

#include <array>
#include <vector>

namespace godot {

class ShapeTool : public RefCounted {
    GDCLASS(ShapeTool, RefCounted)

public:
    // params: shape ("box", "wall", "cylinder", "cone", "sphere", "stairs", "slope", "grid"),
    // size (Vector3i, local), hollow (wall thickness, 0 = solid), step (stairs: rise per step, default 4),
    // rot (0..23), mirror (bool), grid (PackedByteArray, shape "grid": a material per local cell, x
    // fastest, then z, then y; 0 = nothing). Returns {cells: PackedInt32Array (x, y, z world triples),
    // mats: PackedByteArray (grid only, one per cell), size: Vector3i (oriented)}.
    Dictionary cells(const Dictionary &p_params, Vector3i p_lo) const;
    // The oriented size of a local size under rot (and mirror, which does not change it).
    Vector3i oriented_size(Vector3i p_size, int p_rot) const;
    // The local size whose oriented size under rot is p_world_size (handles resize in world axes).
    Vector3i local_size(Vector3i p_world_size, int p_rot) const;
    // rot after turning 90 degrees about world up (+y), or about world x / z (tipping).
    int turn(int p_rot, int p_axis) const;   // axis 0 = x, 1 = y, 2 = z
    // Preview mesh of the cells (surface faces only), vertices relative to p_lo: Mesh.ARRAY_MAX arrays.
    Array mesh(const PackedInt32Array &p_cells, Vector3i p_lo, Vector3i p_size) const;
    // How many cells a fill would write: those whose current material has mask[material] != 0 (empty
    // or replaceable). Cells outside the window or unloaded count as blocked.
    int count_free(const Ref<SimWorld> &p_sim, const PackedInt32Array &p_cells, Vector3i p_window, const PackedByteArray &p_mask) const;
    // Writes `material` (or mats[i] when mats is not empty) into every free cell; returns how many.
    // p_window: world voxel of the SimWorld's local (0, 0, 0).
    int fill(const Ref<SimWorld> &p_sim, const PackedInt32Array &p_cells, int p_material, const PackedByteArray &p_mats,
            Vector3i p_window, const PackedByteArray &p_mask) const;
    // Materials of the world box [lo, lo + size) (x fastest, then z, then y), 0 outside the window.
    PackedByteArray read(const Ref<SimWorld> &p_sim, Vector3i p_lo, Vector3i p_size, Vector3i p_window) const;
    // The cells of the world box [lo, lo + size) whose material has mask[material] != 0 (box dig: what
    // the held tool can take), as x, y, z triples.
    PackedInt32Array select(const Ref<SimWorld> &p_sim, Vector3i p_lo, Vector3i p_size, Vector3i p_window, const PackedByteArray &p_mask) const;
    // {material id: count} of the cells' current materials.
    Dictionary histogram(const Ref<SimWorld> &p_sim, const PackedInt32Array &p_cells, Vector3i p_window) const;
    // A grid (x fastest, then z, then y) of the box [lo, lo + size) holding `material` (or mats[i]) at the
    // cells: a committed shape as a prefab ("last build").
    PackedByteArray grid_of(const PackedInt32Array &p_cells, int p_material, const PackedByteArray &p_mats, Vector3i p_lo, Vector3i p_size) const;
    // Prefab files (PrefabStore): runs of equal bytes as [value, count, value, count, ...], and back.
    PackedInt32Array runs(const PackedByteArray &p_grid) const;
    PackedByteArray unruns(const PackedInt32Array &p_runs) const;
    // A grid (x fastest, then z, then y) cut to the box around its non-zero cells:
    // {size, grid, offset (Vector3i of the kept box in the old one)}, or {} when it is empty.
    Dictionary trim(Vector3i p_size, const PackedByteArray &p_grid) const;
    // Per material counts of the cells a fill of mats would write (for paying per material): {id: count}.
    Dictionary count_free_by_material(const Ref<SimWorld> &p_sim, const PackedInt32Array &p_cells, const PackedByteArray &p_mats,
            Vector3i p_window, const PackedByteArray &p_mask) const;

protected:
    static void _bind_methods();

private:
    using Mat3 = std::array<int, 9>;   // row-major integer rotation
    static const std::vector<Mat3> &rotations();
    static int index_of(const Mat3 &m);
    bool inside(const String &shape, int x, int y, int z, Vector3i s, int hollow, int step) const;
};

} // namespace godot
