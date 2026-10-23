#pragma once
// BrickStore (P9, ADR 0012 option B): the world as bricks of 8x8x8 cells of
// 6.25 cm, where a brick is exactly one 0.5 m voxel of today. A brick is
// uniform (one material, one reference word) or expanded (512 cells from a
// pool). Uniform bricks are what the generator emits and what the far field
// meshes; expanded bricks hold chisel detail and simulated cells.
//
// Cell coordinates are in cells; brick coordinates = cell >> 3. Materials are
// 8-bit here (the sim is 8-bit too); 16-bit ids above 255 are clamped.
//
// Wrapped mode (ADR 0012 step 3, the game): set_wrap_origin() turns the store
// into a toroidal window of power-of-two size addressed by WORLD brick
// coordinates (brick -> slot = brick & (size - 1)). Sliding the window costs
// nothing: the owner moves the origin and rewrites the slabs that entered.
// Dirty bricks are reported in world coordinates relative to that origin.
// Micro patterns (set_micro_patterns) make a uniform brick of a vegetation /
// detail material expand into its 8^3 cell pattern on write, so the near field
// shows a tuft of grass, not a green cube.

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
    // Wrapped (toroidal) addressing by world brick coordinates; origin = world brick of the window's min corner.
    // Sizes must be powers of two. Call again on every slide of the window.
    void set_wrap_origin(Vector3i origin_brick);
    bool is_wrapped() const { return wrap_; }
    Vector3i get_wrap_origin() const { return origin_; }
    // material id -> 512 bytes (x fastest, y, z; 0 = air, else the material): bricks of that material are written expanded
    void set_micro_patterns(const Dictionary &patterns);
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
    void read_chunk_cells(Vector3i chunk, uint8_t *out) const; // same, into 4096 bytes the caller owns (NearFieldFeed)
    const std::vector<int> &dirty_slots() const { return dirty_list_; } // slot indices; world_of() maps them
    Vector3i world_of(int bi) const; // slot index -> brick coordinates (world when wrapped)

    // ---- native access for the simulation (not bound to GDScript) ----
    // Cells of a brick (expanded in place if needed), 512 bytes, x fastest then y then z; marks it dirty.
    uint8_t *raw_cells_mut(Vector3i b) { if (!in_bricks(b.x, b.y, b.z)) return nullptr; const int bi = bidx(b.x, b.y, b.z); expand_index(bi); mark(bi); return cells(bi); }
    const uint8_t *raw_cells(Vector3i b) const { if (!in_bricks(b.x, b.y, b.z)) return nullptr; const int bi = bidx(b.x, b.y, b.z); return (ref_[bi] & EXPANDED) ? cells(bi) : nullptr; }
    uint32_t raw_ref(Vector3i b) const { return in_bricks(b.x, b.y, b.z) ? ref_[bidx(b.x, b.y, b.z)] : 0; } // material, or EXPANDED | slot
    void mark_dirty(Vector3i b) { if (in_bricks(b.x, b.y, b.z)) mark(bidx(b.x, b.y, b.z)); }
    // One brick from the simulation: uniform (or its micro pattern) when eighths is 0 or 8, else expanded with the
    // bottom `eighths` cell layers of `material` over air (a partly full liquid, godot_voxel fluid levels).
    void set_brick_filled(Vector3i b, int material, int eighths);
    Array get_dirty_bricks() const;
    void clear_dirty();
    Dictionary stats() const;

protected:
    static void _bind_methods();

private:
    int bx_ = 0, by_ = 0, bz_ = 0;
    bool wrap_ = false;
    Vector3i origin_;                        // wrapped mode: world brick of the window's min corner
    std::vector<std::vector<uint8_t>> micro_; // per material id: 512 cells or empty
    std::vector<uint32_t> ref_;              // per brick: material (uniform) or EXPANDED | pool index
    std::vector<uint8_t> pool_;              // CELLS bytes per expanded brick
    std::vector<uint32_t> free_;             // free pool slots
    std::vector<uint8_t> dirty_;             // per brick flag
    std::vector<int> dirty_list_;

    inline bool in_bricks(int x, int y, int z) const { return wrap_ || (x >= 0 && y >= 0 && z >= 0 && x < bx_ && y < by_ && z < bz_); }
    inline int bidx(int x, int y, int z) const {
        if (wrap_) return (x & (bx_ - 1)) + bx_ * ((y & (by_ - 1)) + by_ * (z & (bz_ - 1)));
        return x + bx_ * (y + by_ * z);
    }
    void set_uniform(int bi, uint32_t m);   // uniform brick, or its micro pattern when the material has one
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
