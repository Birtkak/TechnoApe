#include "near_field_feed.h"

#include <godot_cpp/core/class_db.hpp>

#include <algorithm>

using namespace godot;

void NearFieldFeed::_bind_methods() {
    ClassDB::bind_method(D_METHOD("configure", "store", "n_cells", "max_chunks_per_upload"), &NearFieldFeed::configure);
    ClassDB::bind_method(D_METHOD("set_window", "origin_cells"), &NearFieldFeed::set_window);
    ClassDB::bind_method(D_METHOD("get_window_origin"), &NearFieldFeed::get_window_origin);
    ClassDB::bind_method(D_METHOD("get_n"), &NearFieldFeed::get_n);
    ClassDB::bind_method(D_METHOD("collect_dirty"), &NearFieldFeed::collect_dirty);
    ClassDB::bind_method(D_METHOD("pop_upload", "max_bytes"), &NearFieldFeed::pop_upload);
    ClassDB::bind_method(D_METHOD("get_valid_min"), &NearFieldFeed::get_valid_min);
    ClassDB::bind_method(D_METHOD("get_valid_max"), &NearFieldFeed::get_valid_max);
    ClassDB::bind_method(D_METHOD("pending_planes"), &NearFieldFeed::pending_planes);
    ClassDB::bind_method(D_METHOD("pending_chunks"), &NearFieldFeed::pending_chunks);
    ClassDB::bind_method(D_METHOD("stats"), &NearFieldFeed::stats);
}

void NearFieldFeed::configure(const Ref<BrickStore> &store, int n_cells, int max_chunks_per_upload) {
    store_ = store;
    n_ = std::max(CS, n_cells);
    w_ = n_ / CS;
    max_chunks_ = std::max(1, max_chunks_per_upload);
    bits_.assign((size_t)w_ * w_ * w_, 0);
    dirty_.clear();
    placed_ = false;
    for (int a = 0; a < 3; a++) { pend_lo_[a] = 0; pend_hi_[a] = 0; }
}

bool NearFieldFeed::set_window(Vector3i o) {
    if (!placed_) {
        placed_ = true;
        origin_ = o;
        reset_pending();
        return true;
    }
    if (o == origin_) return false;
    const int d[3] = { (o.x - origin_.x) / CS, (o.y - origin_.y) / CS, (o.z - origin_.z) / CS };
    bool full = false;
    for (int a = 0; a < 3; a++) {
        if (d[a] >= w_ || d[a] <= -w_) { full = true; continue; }
        if (d[a] > 0) { pend_hi_[a] = std::min(w_, pend_hi_[a] + d[a]); pend_lo_[a] = std::max(0, pend_lo_[a] - d[a]); }
        else if (d[a] < 0) { pend_lo_[a] = std::min(w_, pend_lo_[a] - d[a]); pend_hi_[a] = std::max(0, pend_hi_[a] + d[a]); }
        if (pend_lo_[a] + pend_hi_[a] >= w_) full = true;
    }
    if (full) reset_pending();
    origin_ = o;
    slides_++;
    return true;
}

// Nothing valid: every chunk must be uploaded, as the planes of ONE axis (the other
// axes' planes would upload the same chunks again).
void NearFieldFeed::reset_pending() {
    pend_lo_[0] = w_; pend_lo_[1] = 0; pend_lo_[2] = 0;
    pend_hi_[0] = 0; pend_hi_[1] = 0; pend_hi_[2] = 0;
}

int NearFieldFeed::collect_dirty() {
    if (store_.is_null() || !placed_) return 0;
    int queued = 0;
    for (int bi : store_->dirty_slots()) {
        const Vector3i b = store_->world_of(bi);
        const Vector3i c(b.x >> 1, b.y >> 1, b.z >> 1);   // 2 bricks per chunk edge; arithmetic shift floors negatives
        if (!inside(c)) continue;
        uint8_t &bit = bits_[slot(c)];
        if (bit) continue;
        bit = 1;
        dirty_.push_back(c);
        queued++;
    }
    store_->clear_dirty();
    return queued;
}

void NearFieldFeed::append_plane(int axis, bool low, std::vector<Vector3i> &out) {
    const Vector3i c0 = chunk0();
    const int idx = low ? pend_lo_[axis] - 1 : w_ - pend_hi_[axis];
    for (int v = 0; v < w_; v++)
        for (int u = 0; u < w_; u++) {
            Vector3i c = c0;
            if (axis == 0) c += Vector3i(idx, u, v);
            else if (axis == 1) c += Vector3i(u, idx, v);
            else c += Vector3i(u, v, idx);
            bits_[slot(c)] = 0;   // a queued dirty chunk in this plane is covered by it
            out.push_back(c);
        }
    if (low) pend_lo_[axis]--; else pend_hi_[axis]--;
    planes_uploaded_++;
}

Dictionary NearFieldFeed::pop_upload(int max_bytes) {
    Dictionary d;
    std::vector<Vector3i> chunks;
    if (store_.is_null() || !placed_) { d["count"] = 0; return d; }
    const int budget = std::min(max_chunks_, std::max(0, max_bytes / CHUNK_BYTES));
    const int plane = w_ * w_;
    // slabs first: the one that lets the valid box grow most (largest pending count), innermost plane of it
    while (true) {
        int best = -1; bool low = true; int most = 0;
        for (int a = 0; a < 3; a++) {
            if (pend_lo_[a] > most) { most = pend_lo_[a]; best = a; low = true; }
            if (pend_hi_[a] > most) { most = pend_hi_[a]; best = a; low = false; }
        }
        if (best < 0) break;
        if (!chunks.empty() && (int)chunks.size() + plane > budget) break;
        if (plane > max_chunks_) break;   // window too large for the upload buffer: misconfigured
        append_plane(best, low, chunks);
    }
    // then dirty chunks, oldest first
    size_t i = 0;
    for (; i < dirty_.size(); i++) {
        const Vector3i c = dirty_[i];
        uint8_t &bit = bits_[slot(c)];
        if (!bit) continue;                 // uploaded by a plane meanwhile, or duplicate
        if (!inside(c)) { bit = 0; continue; }
        if ((int)chunks.size() >= budget) break;
        bit = 0;
        chunks.push_back(c);
    }
    if (i > 0) dirty_.erase(dirty_.begin(), dirty_.begin() + i);
    const int count = (int)chunks.size();
    d["count"] = count;
    if (count == 0) return d;
    PackedByteArray bytes;
    bytes.resize((int64_t)count * CHUNK_BYTES);
    PackedInt32Array coords;
    coords.resize(count * 4);
    uint8_t *bp = bytes.ptrw();
    int32_t *cp = coords.ptrw();
    for (int k = 0; k < count; k++) {
        store_->read_chunk_cells(chunks[k], bp + (size_t)k * CHUNK_BYTES);
        cp[k * 4] = chunks[k].x; cp[k * 4 + 1] = chunks[k].y; cp[k * 4 + 2] = chunks[k].z; cp[k * 4 + 3] = 0;
    }
    d["bytes"] = bytes;
    d["coords"] = coords;
    uploads_++;
    chunks_uploaded_ += count;
    bytes_uploaded_ += (int64_t)count * CHUNK_BYTES;
    max_chunks_in_upload_ = std::max(max_chunks_in_upload_, count);
    return d;
}

Vector3i NearFieldFeed::get_valid_min() const {
    if (!placed_) return Vector3i();
    return origin_ + Vector3i(pend_lo_[0], pend_lo_[1], pend_lo_[2]) * CS;
}

Vector3i NearFieldFeed::get_valid_max() const {
    if (!placed_) return Vector3i();
    const Vector3i lo = get_valid_min();
    Vector3i hi = origin_ + Vector3i(w_ - pend_hi_[0], w_ - pend_hi_[1], w_ - pend_hi_[2]) * CS;
    if (hi.x <= lo.x || hi.y <= lo.y || hi.z <= lo.z) return lo;
    return hi;
}

int NearFieldFeed::pending_planes() const {
    int n = 0;
    for (int a = 0; a < 3; a++) n += pend_lo_[a] + pend_hi_[a];
    return n;
}

Dictionary NearFieldFeed::stats() const {
    Dictionary d;
    d["n"] = n_;
    d["uploads"] = uploads_;
    d["chunks_uploaded"] = chunks_uploaded_;
    d["mb_uploaded"] = (double)bytes_uploaded_ / 1048576.0;
    d["max_chunks_in_upload"] = max_chunks_in_upload_;
    d["slides"] = slides_;
    d["planes_uploaded"] = planes_uploaded_;
    d["pending_planes"] = pending_planes();
    d["pending_chunks"] = pending_chunks();
    return d;
}
