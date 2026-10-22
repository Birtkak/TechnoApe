#include "brick_store.h"

#include <godot_cpp/core/class_db.hpp>

#include <algorithm>
#include <cstring>

using namespace godot;

void BrickStore::_bind_methods() {
    ClassDB::bind_method(D_METHOD("configure", "size_bricks"), &BrickStore::configure);
    ClassDB::bind_method(D_METHOD("get_size_bricks"), &BrickStore::get_size_bricks);
    ClassDB::bind_method(D_METHOD("load_voxels", "origin_brick", "size", "ids16"), &BrickStore::load_voxels);
    ClassDB::bind_method(D_METHOD("fill_uniform", "min_brick", "max_brick", "material"), &BrickStore::fill_uniform);
    ClassDB::bind_method(D_METHOD("brick_material", "brick"), &BrickStore::brick_material);
    ClassDB::bind_method(D_METHOD("is_expanded", "brick"), &BrickStore::is_expanded);
    ClassDB::bind_method(D_METHOD("expand", "brick"), &BrickStore::expand);
    ClassDB::bind_method(D_METHOD("collapse", "brick"), &BrickStore::collapse);
    ClassDB::bind_method(D_METHOD("collapse_all"), &BrickStore::collapse_all);
    ClassDB::bind_method(D_METHOD("collapse_dirty"), &BrickStore::collapse_dirty);
    ClassDB::bind_method(D_METHOD("get_cell", "cell"), &BrickStore::get_cell);
    ClassDB::bind_method(D_METHOD("set_cell", "cell", "material"), &BrickStore::set_cell);
    ClassDB::bind_method(D_METHOD("fill_cells", "min_cell", "max_cell", "material"), &BrickStore::fill_cells);
    ClassDB::bind_method(D_METHOD("carve_sphere", "centre_cell", "radius_cells", "material"), &BrickStore::carve_sphere);
    ClassDB::bind_method(D_METHOD("expand_surfaces"), &BrickStore::expand_surfaces);
    ClassDB::bind_method(D_METHOD("get_chunk_bytes", "chunk"), &BrickStore::get_chunk_bytes);
    ClassDB::bind_method(D_METHOD("get_dirty_bricks"), &BrickStore::get_dirty_bricks);
    ClassDB::bind_method(D_METHOD("clear_dirty"), &BrickStore::clear_dirty);
    ClassDB::bind_method(D_METHOD("stats"), &BrickStore::stats);
}

void BrickStore::configure(Vector3i s) {
    bx_ = std::max(0, s.x); by_ = std::max(0, s.y); bz_ = std::max(0, s.z);
    ref_.assign((size_t)bx_ * by_ * bz_, 0);
    dirty_.assign(ref_.size(), 0);
    dirty_list_.clear();
    pool_.clear();
    free_.clear();
}

uint8_t *BrickStore::cells(int bi) { return pool_.data() + (size_t)(ref_[bi] & ~EXPANDED) * CELLS; }
const uint8_t *BrickStore::cells(int bi) const { return pool_.data() + (size_t)(ref_[bi] & ~EXPANDED) * CELLS; }

void BrickStore::mark(int bi) {
    if (!dirty_[bi]) { dirty_[bi] = 1; dirty_list_.push_back(bi); }
}

int BrickStore::expand_index(int bi) {
    const uint32_t r = ref_[bi];
    if (r & EXPANDED) return (int)(r & ~EXPANDED);
    uint32_t slot;
    if (!free_.empty()) { slot = free_.back(); free_.pop_back(); }
    else { slot = (uint32_t)(pool_.size() / CELLS); pool_.resize(pool_.size() + CELLS); }
    std::memset(pool_.data() + (size_t)slot * CELLS, (int)(r & 0xFF), CELLS);
    ref_[bi] = EXPANDED | slot;
    mark(bi);
    return (int)slot;
}

void BrickStore::load_voxels(Vector3i o, Vector3i size, const PackedByteArray &ids16) {
    if (ids16.size() < size.x * size.y * size.z * 2) return;
    const uint8_t *p = ids16.ptr();
    for (int z = 0; z < size.z; z++)
        for (int x = 0; x < size.x; x++)
            for (int y = 0; y < size.y; y++) {
                const int bxx = o.x + x, byy = o.y + y, bzz = o.z + z;
                if (!in_bricks(bxx, byy, bzz)) continue;
                const int i = (y + size.y * (x + size.x * z)) * 2;
                const uint32_t id = std::min<uint32_t>(p[i] | (p[i + 1] << 8), 255);
                const int bi = bidx(bxx, byy, bzz);
                if (ref_[bi] & EXPANDED) free_.push_back(ref_[bi] & ~EXPANDED);
                ref_[bi] = id;
                mark(bi);
            }
}

void BrickStore::fill_uniform(Vector3i mn, Vector3i mx, int material) {
    const uint32_t m = (uint32_t)std::clamp(material, 0, 255);
    for (int z = std::max(0, mn.z); z < std::min(bz_, mx.z); z++)
        for (int y = std::max(0, mn.y); y < std::min(by_, mx.y); y++)
            for (int x = std::max(0, mn.x); x < std::min(bx_, mx.x); x++) {
                const int bi = bidx(x, y, z);
                if (ref_[bi] & EXPANDED) free_.push_back(ref_[bi] & ~EXPANDED);
                if (ref_[bi] != m) { ref_[bi] = m; mark(bi); }
            }
}

int BrickStore::brick_material(Vector3i b) const {
    if (!in_bricks(b.x, b.y, b.z)) return 0;
    const uint32_t r = ref_[bidx(b.x, b.y, b.z)];
    return (r & EXPANDED) ? -1 : (int)r;
}

bool BrickStore::is_expanded(Vector3i b) const {
    return in_bricks(b.x, b.y, b.z) && (ref_[bidx(b.x, b.y, b.z)] & EXPANDED);
}

bool BrickStore::expand(Vector3i b) {
    if (!in_bricks(b.x, b.y, b.z)) return false;
    const int bi = bidx(b.x, b.y, b.z);
    if (ref_[bi] & EXPANDED) return false;
    expand_index(bi);
    return true;
}

bool BrickStore::collapse(Vector3i b) {
    if (!in_bricks(b.x, b.y, b.z)) return false;
    const int bi = bidx(b.x, b.y, b.z);
    if (!(ref_[bi] & EXPANDED)) return false;
    const uint8_t *c = cells(bi);
    const uint8_t m = c[0];
    for (int i = 1; i < CELLS; i++) if (c[i] != m) return false;
    free_.push_back(ref_[bi] & ~EXPANDED);
    ref_[bi] = m;
    mark(bi);
    return true;
}

int BrickStore::collapse_all() {
    int n = 0;
    for (int z = 0; z < bz_; z++)
        for (int y = 0; y < by_; y++)
            for (int x = 0; x < bx_; x++)
                if (collapse(Vector3i(x, y, z))) n++;
    return n;
}

int BrickStore::collapse_dirty() {
    int n = 0;
    const std::vector<int> todo = dirty_list_; // collapse marks bricks again; iterate a copy
    for (int bi : todo) {
        if (!(ref_[bi] & EXPANDED)) continue;
        const uint8_t *c = cells(bi);
        const uint8_t m = c[0];
        bool same = true;
        for (int i = 1; i < CELLS && same; i++) same = c[i] == m;
        if (!same) continue;
        free_.push_back(ref_[bi] & ~EXPANDED);
        ref_[bi] = m;
        n++;
    }
    return n;
}

int BrickStore::get_cell(Vector3i c) const {
    const int bxx = c.x >> 3, byy = c.y >> 3, bzz = c.z >> 3;
    if (!in_bricks(bxx, byy, bzz)) return 0;
    return cell_at(bidx(bxx, byy, bzz), c.x & 7, c.y & 7, c.z & 7);
}

void BrickStore::set_cell(Vector3i c, int material) {
    const int bxx = c.x >> 3, byy = c.y >> 3, bzz = c.z >> 3;
    if (!in_bricks(bxx, byy, bzz)) return;
    const int bi = bidx(bxx, byy, bzz);
    const uint8_t m = (uint8_t)std::clamp(material, 0, 255);
    if (!(ref_[bi] & EXPANDED) && ref_[bi] == m) return;
    const int slot = expand_index(bi);
    pool_[(size_t)slot * CELLS + cidx(c.x & 7, c.y & 7, c.z & 7)] = m;
    mark(bi);
}

void BrickStore::fill_cells(Vector3i mn, Vector3i mx, int material) {
    const uint8_t m = (uint8_t)std::clamp(material, 0, 255);
    const int cx0 = std::max(0, mn.x), cy0 = std::max(0, mn.y), cz0 = std::max(0, mn.z);
    const int cx1 = std::min(bx_ * B, mx.x), cy1 = std::min(by_ * B, mx.y), cz1 = std::min(bz_ * B, mx.z);
    if (cx0 >= cx1 || cy0 >= cy1 || cz0 >= cz1) return;
    for (int bzz = cz0 >> 3; bzz <= (cz1 - 1) >> 3; bzz++)
        for (int byy = cy0 >> 3; byy <= (cy1 - 1) >> 3; byy++)
            for (int bxx = cx0 >> 3; bxx <= (cx1 - 1) >> 3; bxx++) {
                const int bi = bidx(bxx, byy, bzz);
                const int lx0 = std::max(cx0 - bxx * B, 0), lx1 = std::min(cx1 - bxx * B, B);
                const int ly0 = std::max(cy0 - byy * B, 0), ly1 = std::min(cy1 - byy * B, B);
                const int lz0 = std::max(cz0 - bzz * B, 0), lz1 = std::min(cz1 - bzz * B, B);
                const bool whole = lx0 == 0 && ly0 == 0 && lz0 == 0 && lx1 == B && ly1 == B && lz1 == B;
                if (whole) {
                    if (ref_[bi] & EXPANDED) free_.push_back(ref_[bi] & ~EXPANDED);
                    if (ref_[bi] != m || (ref_[bi] & EXPANDED)) { ref_[bi] = m; mark(bi); }
                    continue;
                }
                if (!(ref_[bi] & EXPANDED) && ref_[bi] == m) continue;
                const int slot = expand_index(bi);
                uint8_t *c = pool_.data() + (size_t)slot * CELLS;
                for (int lz = lz0; lz < lz1; lz++)
                    for (int ly = ly0; ly < ly1; ly++)
                        std::memset(c + cidx(lx0, ly, lz), m, lx1 - lx0);
                mark(bi);
            }
}

int BrickStore::carve_sphere(Vector3i cc, int r, int material) {
    const uint8_t m = (uint8_t)std::clamp(material, 0, 255);
    const long r2 = (long)r * r;
    int touched = 0;
    const int b0x = std::max(0, (cc.x - r) >> 3), b1x = std::min(bx_ - 1, (cc.x + r) >> 3);
    const int b0y = std::max(0, (cc.y - r) >> 3), b1y = std::min(by_ - 1, (cc.y + r) >> 3);
    const int b0z = std::max(0, (cc.z - r) >> 3), b1z = std::min(bz_ - 1, (cc.z + r) >> 3);
    for (int bzz = b0z; bzz <= b1z; bzz++)
        for (int byy = b0y; byy <= b1y; byy++)
            for (int bxx = b0x; bxx <= b1x; bxx++) {
                // classify the brick against the sphere by its corners: fully inside -> uniform, partly -> cells
                const int x0 = bxx * B, y0 = byy * B, z0 = bzz * B;
                int inside = 0;
                for (int k = 0; k < 8; k++) {
                    const long dx = (k & 1 ? x0 + B : x0) - cc.x, dy = (k & 2 ? y0 + B : y0) - cc.y, dz = (k & 4 ? z0 + B : z0) - cc.z;
                    if (dx * dx + dy * dy + dz * dz <= r2) inside++;
                }
                const int bi = bidx(bxx, byy, bzz);
                if (inside == 8) {
                    if (ref_[bi] & EXPANDED) free_.push_back(ref_[bi] & ~EXPANDED);
                    if (ref_[bi] != m || (ref_[bi] & EXPANDED)) { ref_[bi] = m; mark(bi); touched++; }
                    continue;
                }
                // nearest corner distance > r -> brick entirely outside
                long nx = std::max<long>(0, std::max<long>(x0 - cc.x, cc.x - (x0 + B)));
                long ny = std::max<long>(0, std::max<long>(y0 - cc.y, cc.y - (y0 + B)));
                long nz = std::max<long>(0, std::max<long>(z0 - cc.z, cc.z - (z0 + B)));
                if (nx * nx + ny * ny + nz * nz > r2) continue;
                if (!(ref_[bi] & EXPANDED) && ref_[bi] == m) continue;
                const int slot = expand_index(bi);
                uint8_t *c = pool_.data() + (size_t)slot * CELLS;
                bool any = false;
                for (int lz = 0; lz < B; lz++)
                    for (int ly = 0; ly < B; ly++)
                        for (int lx = 0; lx < B; lx++) {
                            const long dx = x0 + lx - cc.x, dy = y0 + ly - cc.y, dz = z0 + lz - cc.z;
                            if (dx * dx + dy * dy + dz * dz <= r2) { c[cidx(lx, ly, lz)] = m; any = true; }
                        }
                if (any) { mark(bi); touched++; }
            }
    return touched;
}

int BrickStore::expand_surfaces() {
    int n = 0;
    static const int D[6][3] = {{1, 0, 0}, {-1, 0, 0}, {0, 1, 0}, {0, -1, 0}, {0, 0, 1}, {0, 0, -1}};
    std::vector<int> todo;
    for (int z = 0; z < bz_; z++)
        for (int y = 0; y < by_; y++)
            for (int x = 0; x < bx_; x++) {
                const int bi = bidx(x, y, z);
                if (ref_[bi] & EXPANDED) continue;
                const uint32_t m = ref_[bi];
                if (m == 0) continue; // air stays uniform
                for (int d = 0; d < 6; d++) {
                    const int nx = x + D[d][0], ny = y + D[d][1], nz = z + D[d][2];
                    if (!in_bricks(nx, ny, nz)) continue;
                    const uint32_t nm = ref_[bidx(nx, ny, nz)];
                    if (nm == 0) { todo.push_back(bi); break; }
                }
            }
    for (int bi : todo) { expand_index(bi); n++; }
    return n;
}

PackedByteArray BrickStore::get_chunk_bytes(Vector3i ch) const {
    PackedByteArray out;
    out.resize(16 * 16 * 16);
    uint8_t *w = out.ptrw();
    const int c0x = ch.x * 16, c0y = ch.y * 16, c0z = ch.z * 16;
    for (int z = 0; z < 16; z++)
        for (int x = 0; x < 16; x++) {
            const int bxx = (c0x + x) >> 3, bzz = (c0z + z) >> 3;
            const int lx = (c0x + x) & 7, lz = (c0z + z) & 7;
            uint8_t *col = w + 16 * (x + 16 * z);
            for (int half = 0; half < 2; half++) {
                const int byy = (c0y >> 3) + half;
                if (!in_bricks(bxx, byy, bzz)) { std::memset(col + half * 8, 0, 8); continue; }
                const int bi = bidx(bxx, byy, bzz);
                const uint32_t r = ref_[bi];
                if (!(r & EXPANDED)) { std::memset(col + half * 8, (int)(r & 0xFF), 8); continue; }
                const uint8_t *cc = pool_.data() + (size_t)(r & ~EXPANDED) * CELLS;
                for (int ly = 0; ly < 8; ly++) col[half * 8 + ly] = cc[cidx(lx, ly, lz)];
            }
        }
    return out;
}

Array BrickStore::get_dirty_bricks() const {
    Array a;
    for (int bi : dirty_list_) a.push_back(Vector3i(bi % bx_, (bi / bx_) % by_, bi / (bx_ * by_)));
    return a;
}

void BrickStore::clear_dirty() {
    for (int bi : dirty_list_) dirty_[bi] = 0;
    dirty_list_.clear();
}

Dictionary BrickStore::stats() const {
    Dictionary d;
    const size_t bricks = ref_.size();
    size_t expanded = 0;
    for (uint32_t r : ref_) if (r & EXPANDED) expanded++;
    d["bricks"] = (int64_t)bricks;
    d["expanded"] = (int64_t)expanded;
    d["free_slots"] = (int64_t)free_.size();
    d["ref_bytes"] = (int64_t)(bricks * sizeof(uint32_t));
    d["pool_bytes"] = (int64_t)pool_.size();
    d["total_mb"] = (double)(bricks * sizeof(uint32_t) + pool_.size() + dirty_.size()) / 1048576.0;
    d["expanded_ratio"] = bricks ? (double)expanded / (double)bricks : 0.0;
    d["dirty"] = (int64_t)dirty_list_.size();
    return d;
}
