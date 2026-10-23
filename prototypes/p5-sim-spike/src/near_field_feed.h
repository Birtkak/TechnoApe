#pragma once
// NearFieldFeed (ADR 0012 step 3): the CPU side of the ray-cast near field.
// A toroidal window of n^3 cells (n = 256 or 512, 16 or 32 m) around the player
// lives in a GPU texture; this class decides which 16^3-cell chunks of a
// wrapped BrickStore must be (re)uploaded each frame and packs their bytes:
//  - slabs that entered the window when it slid (one plane of chunks per metre
//    of travel per axis), uploaded innermost first so the valid box grows back;
//  - chunks whose bricks the store reports dirty (simulation ticks, bubble
//    loads, edits).
// Everything is budgeted in bytes per call; what does not fit stays pending.
// The valid box (world cells) is the part of the window whose texels hold the
// current world: the shader and the mesh-hiding box use exactly it, so the
// stale slabs of a slide are never shown.

#include "brick_store.h"

#include <godot_cpp/classes/ref.hpp>
#include <godot_cpp/classes/ref_counted.hpp>
#include <godot_cpp/variant/dictionary.hpp>
#include <godot_cpp/variant/packed_byte_array.hpp>
#include <godot_cpp/variant/packed_int32_array.hpp>
#include <godot_cpp/variant/vector3i.hpp>

#include <cstdint>
#include <vector>

namespace godot {

class NearFieldFeed : public RefCounted {
    GDCLASS(NearFieldFeed, RefCounted)

public:
    static constexpr int CS = 16;              // cells per chunk edge (the GPU scatter unit)
    static constexpr int CHUNK_BYTES = CS * CS * CS;

    void configure(const Ref<BrickStore> &store, int n_cells, int max_chunks_per_upload);
    // Window origin in world cells, a multiple of 16; returns true when it moved.
    bool set_window(Vector3i origin_cells);
    Vector3i get_window_origin() const { return origin_; }
    int get_n() const { return n_; }
    // Store dirty bricks -> pending chunks inside the window; clears the store's dirty list.
    int collect_dirty();
    // Packs up to max_bytes of pending chunks: {"bytes": PackedByteArray, "coords": PackedInt32Array (x,y,z,0 per chunk,
    // world chunk coords), "count": int}. Pending slabs come first; at least one slab goes out when any is pending.
    Dictionary pop_upload(int max_bytes);
    Vector3i get_valid_min() const;   // world cells, inclusive
    Vector3i get_valid_max() const;   // exclusive; equal to min when nothing is valid
    int pending_planes() const;
    int pending_chunks() const { return (int)dirty_.size(); }
    Dictionary stats() const;

protected:
    static void _bind_methods();

private:
    Ref<BrickStore> store_;
    int n_ = 0, w_ = 0;                 // cells and chunks per window edge
    int max_chunks_ = 4096;
    bool placed_ = false;
    Vector3i origin_;                   // world cells
    int pend_lo_[3] = {0, 0, 0};        // chunk planes not yet uploaded at the low / high side of each axis
    int pend_hi_[3] = {0, 0, 0};
    std::vector<uint8_t> bits_;         // per window chunk slot: queued as dirty
    std::vector<Vector3i> dirty_;       // world chunk coords queued
    // stats
    int64_t uploads_ = 0, chunks_uploaded_ = 0, bytes_uploaded_ = 0, slides_ = 0, planes_uploaded_ = 0;
    int max_chunks_in_upload_ = 0;

    inline int slot(Vector3i c) const { return (c.x & (w_ - 1)) + w_ * ((c.y & (w_ - 1)) + w_ * (c.z & (w_ - 1))); }
    inline Vector3i chunk0() const { return Vector3i(origin_.x / CS, origin_.y / CS, origin_.z / CS); }
    inline bool inside(Vector3i c) const {
        const Vector3i c0 = chunk0();
        return c.x >= c0.x && c.y >= c0.y && c.z >= c0.z && c.x < c0.x + w_ && c.y < c0.y + w_ && c.z < c0.z + w_;
    }
    void append_plane(int axis, bool low, std::vector<Vector3i> &out);
    void reset_pending();
};

} // namespace godot
