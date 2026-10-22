#pragma once
// BrickStore (P9, ADR 0012 option B): the world as bricks of 8x8x8 cells of
// 6.25 cm, where a brick is exactly one 0.5 m voxel of today. A brick is
// uniform (one material, one reference word) or expanded (512 cells from a
// pool). Uniform bricks are what the generator emits and what the far field
// meshes; expanded bricks hold chisel detail and simulated cells.
//
// Cell coordinates are in cells; brick coordinates = cell >> 3. Materials are
// 8-bit here (the sim is 8-bit too); 16-bit ids above 255 are clamped.

#include <godot_cpp/classes/ref_counted.hpp>
#include <godot_cpp/variant/array.hpp>
#include <godot_cpp/variant/dictionary.hpp>
#include <godot_cpp/variant/packed_byte_array.hpp>
#include <godot_cpp/variant/vector3i.hpp>

#include <cstdint>
#include <vector>

namespace godot {

class BrickStore : public RefCounted {
    GDCLASS(BrickStore, RefCounted)

public:
    static constexpr int B = 8;              // cells per brick edge
    static constexpr int CELLS = B * B * B;  // 512
    static constexpr uint32_t EXPANDED = 0x80000000u;

    void configure(Vector3i size_bricks);
    Vector3i get_size_bricks() const { return Vector3i(bx_, by_, bz_); }
    // 16-bit voxel ids in godot_voxel order (y fastest, x, z) become uniform bricks at origin_brick.
    void load_voxels(Vector3i origin_brick, Vector3i size, const PackedByteArray &ids16);
    void fill_uniform(Vector3i min_brick, Vector3i max_brick, int material);

    int brick_material(Vector3i b) const;   // -1 when expanded
    bool is_expanded(Vector3i b) const;
    bool expand(Vector3i b);                // false if outside / already expanded
    bool collapse(Vector3i b);              // true if it was homogeneous and got collapsed
    int collapse_all();                     // number collapsed (sweep, worst case)
    int collapse_dirty();                   // collapse only expanded bricks touched since the last clear_dirty (incremental)

    int get_cell(Vector3i c) const;
    void set_cell(Vector3i c, int material);          // expands the brick when needed
    void fill_cells(Vector3i min_cell, Vector3i max_cell, int material);
    int carve_sphere(Vector3i centre_cell, int radius_cells, int material); // bricks touched
    // expands every uniform brick that has a differently-filled neighbour (a "surface"): what chisel/sim would touch
    int expand_surfaces();

    PackedByteArray get_chunk_bytes(Vector3i chunk) const; // 16^3 cells, SimWorld layout (y fastest, x, z)

    // ---- native access for the simulation (not bound to GDScript) ----
    // Cells of a brick (expanded in place if needed), 512 bytes, x fastest then y then z; marks it dirty.
    uint8_t *raw_cells_mut(Vector3i b) { if (!in_bricks(b.x, b.y, b.z)) return nullptr; const int bi = bidx(b.x, b.y, b.z); expand_index(bi); mark(bi); return cells(bi); }
    const uint8_t *raw_cells(Vector3i b) const { if (!in_bricks(b.x, b.y, b.z)) return nullptr; const int bi = bidx(b.x, b.y, b.z); return (ref_[bi] & EXPANDED) ? cells(bi) : nullptr; }
    uint32_t raw_ref(Vector3i b) const { return in_bricks(b.x, b.y, b.z) ? ref_[bidx(b.x, b.y, b.z)] : 0; } // material, or EXPANDED | slot
    void mark_dirty(Vector3i b) { if (in_bricks(b.x, b.y, b.z)) mark(bidx(b.x, b.y, b.z)); }
    Array get_dirty_bricks() const;
    void clear_dirty();
    Dictionary stats() const;

protected:
    static void _bind_methods();

private:
    int bx_ = 0, by_ = 0, bz_ = 0;
    std::vector<uint32_t> ref_;              // per brick: material (uniform) or EXPANDED | pool index
    std::vector<uint8_t> pool_;              // CELLS bytes per expanded brick
    std::vector<uint32_t> free_;             // free pool slots
    std::vector<uint8_t> dirty_;             // per brick flag
    std::vector<int> dirty_list_;

    inline bool in_bricks(int x, int y, int z) const { return x >= 0 && y >= 0 && z >= 0 && x < bx_ && y < by_ && z < bz_; }
    inline int bidx(int x, int y, int z) const { return x + bx_ * (y + by_ * z); }
    inline static int cidx(int x, int y, int z) { return x + B * (y + B * z); } // x fastest inside a brick
    uint8_t *cells(int bi);
    const uint8_t *cells(int bi) const;
    int expand_index(int bi);
    void mark(int bi);
    inline int cell_at(int bi, int lx, int ly, int lz) const {
        const uint32_t r = ref_[bi];
        if (!(r & EXPANDED)) return (int)r;
        return pool_[(size_t)(r & ~EXPANDED) * CELLS + cidx(lx, ly, lz)];
    }
};

} // namespace godot
