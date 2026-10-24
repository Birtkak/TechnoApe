#include "rigid_pieces.h"

#include <godot_cpp/classes/mesh.hpp>
#include <godot_cpp/core/class_db.hpp>
#include <godot_cpp/core/math.hpp>
#include <godot_cpp/variant/packed_color_array.hpp>
#include <godot_cpp/variant/packed_vector2_array.hpp>

#include <algorithm>
#include <chrono>
#include <cmath>
#include <cstdlib>

using namespace godot;

namespace {

constexpr float GRAVITY = 9.8f / 0.125f;   // voxels / s^2
constexpr float WOOD_MASS = 1.0f, LEAF_MASS = 0.12f;
const int DX[6] = {1, -1, 0, 0, 0, 0}, DY[6] = {0, 0, 1, -1, 0, 0}, DZ[6] = {0, 0, 0, 0, 1, -1};

double now_ms() {
    return std::chrono::duration<double, std::milli>(std::chrono::steady_clock::now().time_since_epoch()).count();
}

// world voxel <-> 64-bit key (21 bits per axis, offset so negatives fit)
constexpr int64_t KOFF = 1 << 20;
inline int64_t key(int x, int y, int z) { return ((int64_t)(x + KOFF) << 42) | ((int64_t)(y + KOFF) << 21) | (int64_t)(z + KOFF); }
inline void unkey(int64_t k, int &x, int &y, int &z) {
    x = (int)((k >> 42) & 0x1FFFFF) - KOFF;
    y = (int)((k >> 21) & 0x1FFFFF) - KOFF;
    z = (int)(k & 0x1FFFFF) - KOFF;
}

// Open-addressing hash map from a key to a small int (a set when the value is unused).
struct FlatMap {
    static constexpr int64_t NONE = INT64_MIN;
    std::vector<int64_t> keys;
    std::vector<int32_t> vals;
    size_t n = 0, mask = 0;
    void reset(size_t cap = 1 << 12) {
        keys.assign(cap, NONE);
        vals.assign(cap, 0);
        n = 0;
        mask = cap - 1;
    }
    static inline uint64_t mix(uint64_t h) {
        h ^= h >> 33;
        h *= 0xff51afd7ed558ccdULL;
        h ^= h >> 33;
        return h;
    }
    int32_t *get(int64_t k) {
        for (size_t i = mix((uint64_t)k) & mask;; i = (i + 1) & mask) {
            if (keys[i] == k) return &vals[i];
            if (keys[i] == NONE) return nullptr;
        }
    }
    bool has(int64_t k) { return get(k) != nullptr; }
    // false if it was there already
    bool put(int64_t k, int32_t v = 0) {
        if ((n + 1) * 2 > keys.size()) grow();
        for (size_t i = mix((uint64_t)k) & mask;; i = (i + 1) & mask) {
            if (keys[i] == k) return false;
            if (keys[i] == NONE) {
                keys[i] = k;
                vals[i] = v;
                n++;
                return true;
            }
        }
    }
    void grow() {
        std::vector<int64_t> ok;
        std::vector<int32_t> ov;
        ok.swap(keys);
        ov.swap(vals);
        reset(ok.size() * 2);
        for (size_t i = 0; i < ok.size(); i++)
            if (ok[i] != NONE) put(ok[i], ov[i]);
    }
};

}   // namespace

RigidPieces::RigidPieces() {}

RigidPieces::~RigidPieces() {
    if (sim_.is_valid() && async_) {
        sim_->wait_idle();
        sim_->pre_tick_ = nullptr;
    }
}

void RigidPieces::_bind_methods() {
    ClassDB::bind_method(D_METHOD("setup", "sim", "defs", "atlas_tiles"), &RigidPieces::setup);
    ClassDB::bind_method(D_METHOD("set_origin", "world_cell"), &RigidPieces::set_origin);
    ClassDB::bind_method(D_METHOD("set_params", "params"), &RigidPieces::set_params);
    ClassDB::bind_method(D_METHOD("check", "cells", "push_from"), &RigidPieces::check);
    ClassDB::bind_method(D_METHOD("get_crumbs"), &RigidPieces::get_crumbs);
    ClassDB::bind_method(D_METHOD("take_spawned"), &RigidPieces::take_spawned);
    ClassDB::bind_method(D_METHOD("take_breaks"), &RigidPieces::take_breaks);
    ClassDB::bind_method(D_METHOD("take_dirty_meshes"), &RigidPieces::take_dirty_meshes);
    ClassDB::bind_method(D_METHOD("step", "dt"), &RigidPieces::step);
    ClassDB::bind_method(D_METHOD("get_ids"), &RigidPieces::get_ids);
    ClassDB::bind_method(D_METHOD("get_transform", "id", "alpha"), &RigidPieces::get_transform);
    ClassDB::bind_method(D_METHOD("get_hash_origin", "id"), &RigidPieces::get_hash_origin);
    ClassDB::bind_method(D_METHOD("build_mesh", "id"), &RigidPieces::build_mesh);
    ClassDB::bind_method(D_METHOD("get_info", "id"), &RigidPieces::get_info);
    ClassDB::bind_method(D_METHOD("get_stats"), &RigidPieces::get_stats);
    ClassDB::bind_method(D_METHOD("set_async", "on"), &RigidPieces::set_async);
    ClassDB::bind_method(D_METHOD("queue_check", "cells", "push_from"), &RigidPieces::queue_check);
    ClassDB::bind_method(D_METHOD("queue_step", "dt"), &RigidPieces::queue_step);
    ClassDB::bind_method(D_METHOD("queue_check_packed", "cells", "push_from"), &RigidPieces::queue_check_packed);
    ClassDB::bind_method(D_METHOD("collect"), &RigidPieces::collect);
    ClassDB::bind_method(D_METHOD("get_contact_info", "id"), &RigidPieces::get_contact_info);
}

void RigidPieces::setup(const Ref<SimWorld> &p_sim, const Array &p_defs, Vector2i p_atlas_tiles) {
    sim_ = p_sim;
    atlas_tiles_ = p_atlas_tiles;
    const int n = p_defs.size() + 1;
    cls_.assign(n, EMPTY);
    tiles_.assign(n, {0, 0, 0});
    transparent_.assign(n, 0);
    hardness_.assign(n, 1e9f);
    for (int i = 0; i < p_defs.size(); i++) {
        const Dictionary d = p_defs[i];
        const int m = i + 1;
        const String rigid = d.get("rigid", "");
        const String ct = d.get("cell_type", "solid");
        uint8_t c = EMPTY;
        if (rigid == "wood") c = WOOD;
        else if (rigid == "leaf") c = LEAF;
        else if (ct == "solid" || ct == "powder") c = (d.has("micro") || (bool)d.get("replaceable", false)) ? CRUSH : GROUND;
        cls_[m] = c;
        const Array t = d.get("tiles", Array());
        if (t.size() >= 3) tiles_[m] = {(int)t[0], (int)t[1], (int)t[2]};
        transparent_[m] = (bool)d.get("transparent", false) ? 1 : 0;
        // what a falling piece must hit it with to break it: "hardness", else "hp", else by class
        const float by_class = c == WOOD ? 4000.0f : c == LEAF ? 20.0f : c == CRUSH ? 5.0f : 1e9f;
        hardness_[m] = d.has("hardness") ? (float)d["hardness"] : (d.has("hp") && c != GROUND ? (float)d["hp"] : by_class);
    }
}

void RigidPieces::set_origin(Vector3i p) { origin_ = p; }

void RigidPieces::set_params(const Dictionary &p) {
    max_wood_ = p.get("max_wood", max_wood_);
    leaf_reach_ = p.get("leaf_reach", leaf_reach_);
    substeps_ = std::max(1, (int)p.get("substeps", substeps_));
    friction_ = p.get("friction", friction_);
    restitution_ = p.get("restitution", restitution_);
    push_ = p.get("push", push_);
    sleep_ticks_ = p.get("sleep_ticks", sleep_ticks_);
    max_age_ = p.get("max_age_s", max_age_);
    max_points_ = p.get("max_points", max_points_);
    min_piece_ = p.get("min_piece", min_piece_);
    strength_ = p.get("strength", strength_);
    region_cap_ = p.get("region_cap", region_cap_);
    max_paths_ = p.get("max_paths", max_paths_);
    thick_ = p.get("thick", thick_);
    max_run_ = p.get("max_run", max_run_);
    hinge_deg_ = p.get("hinge_deg", hinge_deg_);
    limb_run_ = p.get("limb_run", limb_run_);
    heavy_mass_ = p.get("heavy_mass", heavy_mass_);
    hinge_s_ = p.get("hinge_s", hinge_s_);
    min_limb_ = p.get("min_limb", min_limb_);
    impact_k_ = p.get("impact_k", impact_k_);
    load_k_ = p.get("load_k", load_k_);
    max_leaf_points_ = p.get("max_leaf_points", max_leaf_points_);
    max_breaks_ = p.get("max_breaks", max_breaks_);
    recheck_every_ = std::max(1, (int)p.get("recheck_every", recheck_every_));
    mesh_every_ = std::max(1, (int)p.get("mesh_every", mesh_every_));
}

// ---- cells ----

inline bool RigidPieces::local_of(int wx, int wy, int wz, int &x, int &y, int &z) const {
    x = wx - origin_.x;
    y = wy - origin_.y;
    z = wz - origin_.z;
    return sim_->is_open(x, y, z);
}

uint8_t RigidPieces::mat_at(int wx, int wy, int wz, bool &open) const {
    int x, y, z;
    open = local_of(wx, wy, wz, x, y, z);
    return open ? sim_->cells_[sim_->idx(x, y, z)].mat : 0;
}

uint8_t RigidPieces::cls_at(int wx, int wy, int wz) const {
    bool open;
    const uint8_t m = mat_at(wx, wy, wz, open);
    return open ? cls_[m] : (uint8_t)GROUND;
}

void RigidPieces::set_at(int wx, int wy, int wz, uint8_t m) {
    int x, y, z;
    if (local_of(wx, wy, wz, x, y, z)) sim_->place_cell(x, y, z, m);
}

// ---- support ----

PackedInt32Array RigidPieces::check(const Array &p_cells, Vector3 p_push_from) {
    if (sim_.is_null()) return PackedInt32Array();
    sim_->wait_idle();
    crumbs_.clear();
    std::vector<Vector3i> cells(p_cells.size());
    for (int i = 0; i < p_cells.size(); i++) cells[i] = p_cells[i];
    return check_cells(cells, p_push_from, true);
}

// strength: also test whether supported wood above the cells hangs on too little (player cuts; the
// limbs a falling piece snaps only get the cheap free-or-held fill). Crumbs accumulate until check().
PackedInt32Array RigidPieces::check_cells(const std::vector<Vector3i> &p_cells, Vector3 p_push_from, bool strength) {
    PackedInt32Array out;
    anchors_.clear();
    fills_.clear();
    last_paths_ = last_region_ = 0;
    last_strength_ms_ = 0.0;
    if (sim_.is_null() || cls_.empty()) return out;
    const double t0 = now_ms();
    FlatMap supported, freed, visit;
    supported.reset();
    freed.reset();
    std::vector<std::vector<int64_t>> comps;
    std::vector<int64_t> queue;
    int filled = 0;
    for (const Vector3i &c : p_cells) {
        for (int d = 0; d < 6; d++) {
            const int sx = c.x + DX[d], sy = c.y + DY[d], sz = c.z + DZ[d];
            if (cls_at(sx, sy, sz) != WOOD) continue;
            const int64_t sk = key(sx, sy, sz);
            if (supported.has(sk) || freed.has(sk)) continue;
            // flood fill through wood until ground (or the cap) is found
            visit.reset();
            queue.clear();
            visit.put(sk);
            queue.push_back(sk);
            bool anchored = false;
            int why = 0;   // debug: 1 ground, 2 supported fill, 3 cap
            for (size_t h = 0; h < queue.size() && !anchored; h++) {
                int x, y, z;
                unkey(queue[h], x, y, z);
                for (int e = 0; e < 6; e++) {
                    const int nx = x + DX[e], ny = y + DY[e], nz = z + DZ[e];
                    const uint8_t cl = cls_at(nx, ny, nz);
                    if (cl == GROUND) {
                        anchored = true;
                        why = 1;
                        anchors_.push_back(Vector3i(nx, ny, nz));
                        break;
                    }
                    if (cl != WOOD) continue;
                    const int64_t nk = key(nx, ny, nz);
                    if (supported.has(nk)) { anchored = true; why = 2; break; }
                    if (visit.put(nk)) queue.push_back(nk);
                }
                if ((int)queue.size() > max_wood_) { anchored = true; why = 3; }
            }
            filled += (int)queue.size();
            fills_.push_back(Vector3i(sy, (int)queue.size(), why));
            if (anchored) {
                for (int64_t k : queue) supported.put(k);
            } else {
                for (int64_t k : queue) freed.put(k);
                comps.push_back(queue);
            }
        }
    }
    const double t_fill = now_ms();
    // Strength: the wood just above the hole may still hang on too little (a limb grown into the next
    // tree, the hinge of a chopped trunk). Test it, and break it off at the weakest place.
    {
        FlatMap seen;
        seen.reset();
        std::vector<int64_t> S;
        for (const Vector3i &c : p_cells) {
            const int64_t k = key(c.x, c.y + 1, c.z);
            if (cls_at(c.x, c.y + 1, c.z) == WOOD && !freed.has(k) && seen.put(k)) S.push_back(k);
        }
        std::vector<int64_t> piece;
        if (strength && !S.empty() && strength_split(S, piece)) {
            for (int64_t k : piece) freed.put(k);
            // the fracture: the wood left touching the piece across the split tears with it (else the
            // piece starts face to face with it and jams on the first tick). Checked with the snapped limbs.
            for (int64_t k : piece) {
                int x, y, z;
                unkey(k, x, y, z);
                for (int e = 0; e < 6; e++) {
                    const int nx = x + DX[e], ny = y + DY[e], nz = z + DZ[e];
                    if (cls_at(nx, ny, nz) != WOOD || freed.has(key(nx, ny, nz))) continue;
                    bool open;
                    const uint8_t m = mat_at(nx, ny, nz, open);
                    set_at(nx, ny, nz, 0);
                    note_break(nx, ny, nz, m);
                    snapped_.push_back(Vector3i(nx, ny, nz));
                    torn_++;
                }
            }
            comps.push_back(std::move(piece));
        }
    }
    const double t_strength = now_ms();
    double leaf_ms = 0.0, make_ms = 0.0;
    // leaves: those closer (through leaves) to the piece's wood than to any other wood. On a dense box
    // around the wood (no leaf further than leaf_reach can join): distances to this wood (da, OWN on the
    // wood itself) and to other wood (db); calloc'd, so untouched pages cost nothing.
    for (const std::vector<int64_t> &wood : comps) {
        const double tl = now_ms();
        int lo[3] = {INT32_MAX, INT32_MAX, INT32_MAX}, hi[3] = {INT32_MIN, INT32_MIN, INT32_MIN};
        for (int64_t k : wood) {
            int v[3];
            unkey(k, v[0], v[1], v[2]);
            for (int a = 0; a < 3; a++) {
                lo[a] = std::min(lo[a], v[a]);
                hi[a] = std::max(hi[a], v[a]);
            }
        }
        for (int a = 0; a < 3; a++) {
            lo[a] -= leaf_reach_ + 1;
            hi[a] += leaf_reach_ + 1;
        }
        const int bx = hi[0] - lo[0] + 1, by = hi[1] - lo[1] + 1, bz = hi[2] - lo[2] + 1;
        const size_t bn = (size_t)bx * by * bz;
        uint8_t *da = (uint8_t *)std::calloc(bn, 1), *db = (uint8_t *)std::calloc(bn, 1);
        constexpr uint8_t OWN = 255;
        const int64_t off[6] = {1, -1, bx, -bx, (int64_t)bx * by, -(int64_t)bx * by};
        auto at = [&](int x, int y, int z) { return (size_t)(x - lo[0]) + (size_t)bx * ((size_t)(y - lo[1]) + (size_t)by * (size_t)(z - lo[2])); };
        for (int64_t k : wood) {
            int x, y, z;
            unkey(k, x, y, z);
            da[at(x, y, z)] = OWN;
        }
        std::vector<uint32_t> front, bfront;   // box indices
        for (int64_t k : wood) {
            int x, y, z;
            unkey(k, x, y, z);
            for (int e = 0; e < 6; e++) {
                const int nx = x + DX[e], ny = y + DY[e], nz = z + DZ[e];
                const size_t i = at(nx, ny, nz);
                if (!da[i] && cls_at(nx, ny, nz) == LEAF) {
                    da[i] = 1;
                    front.push_back((uint32_t)i);
                }
            }
        }
        for (size_t h = 0; h < front.size(); h++) {
            const uint32_t i = front[h];
            const int dist = da[i];
            const int x = lo[0] + (int)(i % bx), y = lo[1] + (int)(i / bx % by), z = lo[2] + (int)(i / bx / by);
            for (int e = 0; e < 6; e++) {
                const uint32_t ni = (uint32_t)(i + off[e]);
                const uint8_t cl = cls_at(x + DX[e], y + DY[e], z + DZ[e]);
                if (cl == WOOD && da[ni] != OWN) {
                    if (!db[i]) {   // this leaf touches another tree's wood
                        db[i] = 1;
                        bfront.push_back(i);
                    }
                } else if (cl == LEAF && dist < leaf_reach_ && !da[ni]) {
                    da[ni] = (uint8_t)(dist + 1);
                    front.push_back(ni);
                }
            }
        }
        // the other trees' distances, inside the leaves reached
        for (size_t h = 0; h < bfront.size(); h++) {
            const uint32_t i = bfront[h];
            const int dist = db[i];
            if (dist >= leaf_reach_) continue;
            for (int e = 0; e < 6; e++) {
                const uint32_t ni = (uint32_t)(i + off[e]);
                if (da[ni] && da[ni] != OWN && !db[ni]) {
                    db[ni] = (uint8_t)(dist + 1);
                    bfront.push_back(ni);
                }
            }
        }
        std::vector<int64_t> mine;
        mine.reserve(front.size());
        for (uint32_t i : front)
            if (!db[i] || da[i] < db[i]) mine.push_back(key(lo[0] + (int)(i % bx), lo[1] + (int)(i / bx % by), lo[2] + (int)(i / bx / by)));
        std::free(da);
        std::free(db);
        // too small to fly: crumbs (Grimorium: < 8 voxels); snapped twigs too, below min_limb of wood (only
        // wood holds a piece up: a twig in a bunch of leaves rocked on its few wood voxels and never slept)
        if (strength ? (int)(wood.size() + mine.size()) < min_piece_ : (int)wood.size() < min_limb_) {
            const std::vector<int64_t> *lists[2] = {&wood, &mine};
            for (const std::vector<int64_t> *ks : lists)
                for (int64_t k : *ks) {
                    int x, y, z;
                    unkey(k, x, y, z);
                    bool open;
                    const uint8_t m = mat_at(x, y, z, open);
                    crumbs_.push_back(x);
                    crumbs_.push_back(y);
                    crumbs_.push_back(z);
                    crumbs_.push_back(m);
                    set_at(x, y, z, 0);
                }
            continue;
        }
        const double tm = now_ms();
        leaf_ms += tm - tl;
        out.push_back(next_id_);
        make_piece(wood, mine, p_push_from, strength);
        make_ms += now_ms() - tm;
    }
    last_filled_ = filled;
    split_ms_[0] = t_fill - t0;
    split_ms_[1] = t_strength - t_fill;
    split_ms_[2] = leaf_ms;
    split_ms_[3] = make_ms;
    last_check_ms_ = now_ms() - t0;
    max_check_ms_ = std::max(max_check_ms_, last_check_ms_);
    return out;
}

// Max flow from the sources S (the wood just above a hole) to the ground through wood, one unit per
// voxel face: the number of separate paths is the cross-section that holds S up. Each path carries
// `strength` voxels. The region grows as the searches reach it (a dig by the roots meets the ground
// at once and never walks the whole forest); wood beyond region_cap counts as held (the rest of a big
// structure). If the paths found carry less than the wood on S's side of the narrowest cut, that side
// is `piece`.
bool RigidPieces::strength_split(const std::vector<int64_t> &S, std::vector<int64_t> &piece) {
    const double t0 = now_ms();
    FlatMap id;
    id.reset();
    std::vector<int64_t> nodes;
    std::vector<int32_t> nbr, stamp, prev, q;
    std::vector<int8_t> sinkcap, sinkflow, flow, pdir;
    std::vector<uint8_t> expanded;
    auto add = [&](int64_t k) -> int32_t {
        const int32_t i = (int32_t)nodes.size();
        nodes.push_back(k);
        id.put(k, i);
        nbr.insert(nbr.end(), 6, -1);
        flow.insert(flow.end(), 6, 0);
        sinkcap.push_back(0);
        sinkflow.push_back(0);
        expanded.push_back(0);
        stamp.push_back(0);
        prev.push_back(-1);
        pdir.push_back(0);
        return i;
    };
    // a node's faces, the first time a search reaches it
    auto expand = [&](int32_t i) {
        expanded[i] = 1;
        int x, y, z;
        unkey(nodes[i], x, y, z);
        for (int e = 0; e < 6; e++) {
            const int nx = x + DX[e], ny = y + DY[e], nz = z + DZ[e];
            const uint8_t cl = cls_at(nx, ny, nz);
            if (cl == GROUND) {
                sinkcap[i]++;
            } else if (cl == WOOD) {
                const int64_t nk = key(nx, ny, nz);
                const int32_t *j = id.get(nk);
                if (j) nbr[(size_t)i * 6 + e] = *j;
                else if ((int)nodes.size() >= region_cap_) sinkcap[i]++;   // past the region: held by the rest
                else nbr[(size_t)i * 6 + e] = add(nk);
            }
        }
    };
    for (int64_t k : S)
        if (!id.has(k)) add(k);
    const size_t nsrc = nodes.size();
    const int enough = max_paths_;
    int f = 0, it = 0;
    bool held = false;
    for (;;) {
        it++;
        q.clear();
        for (size_t s = 0; s < nsrc; s++) {
            stamp[s] = it;
            prev[s] = -1;
            q.push_back((int32_t)s);
        }
        int hit = -1;
        for (size_t h = 0; h < q.size() && hit < 0; h++) {
            const int i = q[h];
            if (!expanded[i]) expand(i);
            if (sinkflow[i] < sinkcap[i]) { hit = i; break; }
            for (int e = 0; e < 6; e++) {
                const int j = nbr[(size_t)i * 6 + e];
                if (j < 0 || stamp[j] == it || flow[(size_t)i * 6 + e] >= 1) continue;
                stamp[j] = it;
                prev[j] = i;
                pdir[j] = (int8_t)e;
                q.push_back(j);
            }
        }
        if (hit < 0) break;
        sinkflow[hit]++;
        for (int j = hit; prev[j] >= 0; j = prev[j]) {
            const int i = prev[j], e = pdir[j];
            flow[(size_t)i * 6 + e]++;
            flow[(size_t)j * 6 + (e ^ 1)]--;
        }
        if (++f >= enough) { held = true; break; }
    }
    last_paths_ = f;
    last_region_ = (int)nodes.size();
    last_side_ = held ? -1 : (int)q.size();
    // the last, failing search reached exactly S's side of the narrowest cut
    if (!held && (double)f * strength_ < q.size() * 1.3) {
        piece.reserve(q.size());
        for (int32_t i : q) piece.push_back(nodes[i]);
    }
    last_strength_ms_ = now_ms() - t0;
    return !piece.empty();
}

void RigidPieces::make_piece(const std::vector<int64_t> &wood, const std::vector<int64_t> &leaves, Vector3 push_from, bool hinged) {
    Piece p;
    p.id = next_id_++;
    int lo[3] = {INT32_MAX, INT32_MAX, INT32_MAX}, hi[3] = {INT32_MIN, INT32_MIN, INT32_MIN};
    auto grow = [&](const std::vector<int64_t> &ks) {
        for (int64_t k : ks) {
            int v[3];
            unkey(k, v[0], v[1], v[2]);
            for (int a = 0; a < 3; a++) {
                lo[a] = std::min(lo[a], v[a]);
                hi[a] = std::max(hi[a], v[a]);
            }
        }
    };
    grow(wood);
    grow(leaves);
    p.size = Vector3i(hi[0] - lo[0] + 1, hi[1] - lo[1] + 1, hi[2] - lo[2] + 1);
    p.grid.assign((size_t)p.size.x * p.size.y * p.size.z, 0);
    p.hash_origin = Vector3(lo[0], lo[1], lo[2]);
    Vector3 msum;
    double mass = 0.0;
    auto take = [&](const std::vector<int64_t> &ks, float m) {
        for (int64_t k : ks) {
            int x, y, z;
            unkey(k, x, y, z);
            bool open;
            const uint8_t mat = mat_at(x, y, z, open);
            const int lx = x - lo[0], ly = y - lo[1], lz = z - lo[2];
            p.grid[lx + p.size.x * (ly + p.size.y * lz)] = mat;
            msum += Vector3(lx + 0.5f, ly + 0.5f, lz + 0.5f) * m;
            mass += m;
            set_at(x, y, z, 0);
        }
    };
    take(wood, WOOD_MASS);
    take(leaves, LEAF_MASS);
    p.wood = (int)wood.size();
    p.leaves = (int)leaves.size();
    p.mass = (float)mass;
    p.inv_mass = 1.0f / p.mass;
    p.com = msum / p.mass;
    // inertia about the centre of mass (each voxel a unit cube) and the wood surface as contact points
    double I[3][3] = {};
    std::vector<Vector3> pts;
    std::vector<LeafPoint> lpts;
    for (int z = 0; z < p.size.z; z++)
        for (int y = 0; y < p.size.y; y++)
            for (int x = 0; x < p.size.x; x++) {
                const uint8_t m = p.grid[x + p.size.x * (y + p.size.y * z)];
                if (!m) continue;
                const float w = cls_[m] == WOOD ? WOOD_MASS : LEAF_MASS;
                const Vector3 r = Vector3(x + 0.5f, y + 0.5f, z + 0.5f) - p.com;
                const double rr = r.dot(r);
                const double rv[3] = {r.x, r.y, r.z};
                for (int a = 0; a < 3; a++)
                    for (int b = 0; b < 3; b++) I[a][b] += w * ((a == b ? rr + 1.0 / 6.0 : 0.0) - rv[a] * rv[b]);
                if (cls_[m] != WOOD) {
                    bool open_face = false;
                    for (int e = 0; e < 6 && !open_face; e++) {
                        const int nx = x + DX[e], ny = y + DY[e], nz = z + DZ[e];
                        open_face = nx < 0 || ny < 0 || nz < 0 || nx >= p.size.x || ny >= p.size.y || nz >= p.size.z ||
                                !p.grid[nx + p.size.x * (ny + p.size.y * nz)];
                    }
                    if (open_face) lpts.push_back({r, x + p.size.x * (y + p.size.y * z)});
                    continue;
                }
                bool surface = false;
                for (int e = 0; e < 6 && !surface; e++) {
                    const int nx = x + DX[e], ny = y + DY[e], nz = z + DZ[e];
                    surface = nx < 0 || ny < 0 || nz < 0 || nx >= p.size.x || ny >= p.size.y || nz >= p.size.z ||
                            cls_[p.grid[nx + p.size.x * (ny + p.size.y * nz)]] != WOOD;
                }
                if (surface) pts.push_back(r);
            }
    const Basis inertia(Vector3(I[0][0], I[0][1], I[0][2]), Vector3(I[1][0], I[1][1], I[1][2]), Vector3(I[2][0], I[2][1], I[2][2]));
    p.inv_inertia = inertia.inverse();
    const size_t stride = std::max<size_t>(1, (pts.size() + max_points_ - 1) / max_points_);
    for (size_t i = 0; i < pts.size(); i += stride) p.points.push_back(pts[i]);
    const size_t lstride = std::max<size_t>(1, (lpts.size() + max_leaf_points_ - 1) / max_leaf_points_);
    for (size_t i = 0; i < lpts.size(); i += lstride) p.leaf_points.push_back(lpts[i]);
    p.blocks = Vector3i((p.size.x + MESH_BLOCK - 1) / MESH_BLOCK, (p.size.y + MESH_BLOCK - 1) / MESH_BLOCK, (p.size.z + MESH_BLOCK - 1) / MESH_BLOCK);
    p.block_dirty.assign((size_t)p.blocks.x * p.blocks.y * p.blocks.z, 0);
    p.pos = p.prev_pos = p.hash_origin + p.com;
    p.rot = Quaternion();
    p.prev_rot = p.rot;
    // topple away from the cut: spin about the base so the bottom stays put
    Vector3 dir = p.pos - push_from;
    dir.y = 0.0f;
    if (dir.length_squared() < 1e-4f) dir = Vector3(1, 0, 0);
    dir.normalize();
    p.ang = Vector3(0, 1, 0).cross(dir) * push_;
    Vector3 base = Vector3(p.com.x, 0.0f, p.com.z) - p.com;
    if (hinged) {
        // the pivot: the bottom wood voxel furthest along the fall, at its outer bottom edge (like the
        // hinge wood a feller leaves), so the trunk rotates off its stump instead of sliding about on it
        int ymin = INT32_MAX;
        for (int64_t k : wood) {
            int x, y, z;
            unkey(k, x, y, z);
            ymin = std::min(ymin, y);
        }
        float best = -1e9f;
        for (int64_t k : wood) {
            int x, y, z;
            unkey(k, x, y, z);
            if (y != ymin) continue;
            const Vector3 c(x - lo[0] + 0.5f, 0.0f, z - lo[2] + 0.5f);
            const float d = (c - Vector3(p.com.x, 0.0f, p.com.z)).dot(dir);
            if (d > best) {
                best = d;
                base = Vector3(c.x + dir.x * 0.5f, (float)(ymin - lo[1]), c.z + dir.z * 0.5f) - p.com;
            }
        }
        p.hinged = true;
        p.hinge_r = base;
        p.hinge_world = p.pos + base;
    }
    p.vel = -p.ang.cross(base);
    spawned_ids_.push_back(p.id);
    if (made_.size() < 16) made_.push_back(Vector3i(p.id, p.wood, p.leaves));
    pieces_.push_back(std::move(p));
    spawned_++;
}

// ---- physics ----

// The surface normal around a cell from the solid cells in the 3x3x3 block (Grimorium takes it from a
// marching-cubes fit): a log on a stair-stepped slope rolls down it instead of skidding on flat steps.
// Falls back to the face normal when the block is too symmetric to say.
Vector3 RigidPieces::smooth_normal(int cx, int cy, int cz, const Vector3 &face) const {
    Vector3 g;
    for (int z = -1; z <= 1; z++)
        for (int y = -1; y <= 1; y++)
            for (int x = -1; x <= 1; x++) {
                if (!x && !y && !z) continue;
                const uint8_t c = cls_at(cx + x, cy + y, cz + z);
                if (c == GROUND || c == WOOD) g -= Vector3(x, y, z);
            }
    if (g.length_squared() < 1e-6f) return face;
    g.normalize();
    return g.dot(face) > 0.3f ? g : face;
}

// A falling piece hitting a voxel at `speed` (voxels/s) breaks it when impact_k * speed^2 reaches the
// voxel's hardness: leaves and plants give way at once, wood by how thick it is (wood_factor: a limb
// twice as thick needs twice the speed), so a tree tipping hard smashes through its neighbours' limbs
// while their trunks hold; ground never breaks. Broken wood is checked after the tick, so a
// snapped-off limb falls by itself.
// A heavy piece (a felled tree, heavy_mass or more) also breaks any wood thinner than limb_run it
// touches, however slowly: old-growth crowns interlock a voxel apart, and a tree that only breaks what it
// hits fast enough stood propped in its neighbours' limbs. Their trunks still hold.
bool RigidPieces::breaks(int x, int y, int z, float speed) {
    bool open;
    const uint8_t m = mat_at(x, y, z, open);
    if (!open || !m) return false;
    const bool limb_crush = cls_[m] == WOOD && cur_mass_ >= heavy_mass_ && wood_run(x, y, z, limb_run_) < limb_run_;
    if (!limb_crush && impact_k_ * speed * speed < hardness_[m]) return false;
    if (breaks_tick_ >= max_breaks_) return false;   // over this tick's budget: it holds (wood) or waits (leaves)
    if (cls_[m] == WOOD) {
        if (!limb_crush && impact_k_ * speed * speed < hardness_[m] * wood_factor(x, y, z)) return false;
        snapped_.push_back(Vector3i(x, y, z));
    }
    set_at(x, y, z, 0);
    note_break(x, y, z, m);
    crushed_++;
    breaks_tick_++;
    return true;
}

// The shortest run of wood through a voxel along x, y or z, up to `cap`: how thick that wood is (a
// trunk is thick across both horizontal axes, a limb across two of the three).
int RigidPieces::wood_run(int x, int y, int z, int cap) const {
    int best = cap;
    for (int a = 0; a < 3; a++) {
        int run = 1;
        for (int sgn = -1; sgn <= 1; sgn += 2)
            for (int k = 1; k < cap && run < best; k++) {
                const int qx = x + (a == 0 ? sgn * k : 0), qy = y + (a == 1 ? sgn * k : 0), qz = z + (a == 2 ? sgn * k : 0);
                if (cls_at(qx, qy, qz) != WOOD) break;
                run++;
            }
        best = std::min(best, run);
    }
    return best;
}

// How much harder than its hardness wood is to break: 1 under `thick` voxels, then (run / thick)^2, so
// the speed (or load) that breaks it grows with its thickness. Runs past max_run count as max_run.
float RigidPieces::wood_factor(int x, int y, int z) const {
    const float f = std::max(1.0f, (float)wood_run(x, y, z, max_run_) / (float)thick_);
    return f * f;
}

void RigidPieces::note_break(int x, int y, int z, uint8_t m) {
    if (breaks_.size() >= 4 * 4096) return;
    breaks_.push_back(x);
    breaks_.push_back(y);
    breaks_.push_back(z);
    breaks_.push_back(m);
}

void RigidPieces::collide(Piece &p, float dt, std::vector<Contact> &cs) {
    cs.clear();
    cur_mass_ = p.mass;
    const Basis R(p.rot);
    for (const Vector3 &pl : p.points) {
        const Vector3 r = R.xform(pl);
        const Vector3 w = p.pos + r;
        const int cx = (int)std::floor(w.x), cy = (int)std::floor(w.y), cz = (int)std::floor(w.z);
        const uint8_t cl = cls_at(cx, cy, cz);
        const float f[3] = {w.x - cx, w.y - cy, w.z - cz};
        const float speed = (p.vel + p.ang.cross(r)).length();
        if (cl == GROUND || (cl == WOOD && !breaks(cx, cy, cz, speed))) {
            (cl == WOOD ? hit_wood_ : hit_ground_)++;
            if (cl == WOOD) hit_at_ = Vector3i(cx, cy, cz);
            // inside a solid cell: push out through the nearest open face
            float best = 1e9f;
            Vector3 bn(0, 1, 0);
            for (int e = 0; e < 6; e++) {
                const uint8_t nc = cls_at(cx + DX[e], cy + DY[e], cz + DZ[e]);
                if (nc == GROUND || nc == WOOD) continue;
                const int a = DX[e] ? 0 : (DY[e] ? 1 : 2);
                const float d = (DX[e] + DY[e] + DZ[e]) > 0 ? 1.0f - f[a] : f[a];
                if (d < best) {
                    best = d;
                    bn = Vector3(DX[e], DY[e], DZ[e]);
                }
            }
            if (best > 1e8f) best = 1.0f;
            Contact c{};
            c.r = r;
            c.n = smooth_normal(cx + (int)bn.x, cy + (int)bn.y, cz + (int)bn.z, bn);
            c.depth = best + 0.5f;
            if (cl == WOOD) c.cell = Vector3i(cx, cy, cz), c.factor = wood_factor(cx, cy, cz);
            cs.push_back(c);
            if (contact_log_.size() < 64) contact_log_.push_back(Vector4i(cx, cy, cz, cl * 10 + 1));   // debug: 1 = inside
            continue;
        }
        if (cl == LEAF || cl == CRUSH) breaks(cx, cy, cz, speed);   // wood goes through leaves and plants
        // a half-voxel sphere against the faces of the cells around it
        for (int a = 0; a < 3; a++) {
            const int s = f[a] < 0.5f ? -1 : 1;
            const float gap = s < 0 ? f[a] : 1.0f - f[a];
            if (gap >= 0.5f) continue;
            const uint8_t nc = cls_at(cx + (a == 0 ? s : 0), cy + (a == 1 ? s : 0), cz + (a == 2 ? s : 0));
            if (nc != GROUND && nc != WOOD) continue;
            if (nc == WOOD && breaks(cx + (a == 0 ? s : 0), cy + (a == 1 ? s : 0), cz + (a == 2 ? s : 0), speed)) continue;
            (nc == WOOD ? hit_wood_ : hit_ground_)++;
            if (nc == WOOD) hit_at_ = Vector3i(cx + (a == 0 ? s : 0), cy + (a == 1 ? s : 0), cz + (a == 2 ? s : 0));
            Contact c{};
            c.r = r;
            c.n = smooth_normal(cx, cy, cz, Vector3(a == 0 ? -s : 0, a == 1 ? -s : 0, a == 2 ? -s : 0));
            c.depth = 0.5f - gap;
            if (nc == WOOD) {
                c.cell = Vector3i(cx + (a == 0 ? s : 0), cy + (a == 1 ? s : 0), cz + (a == 2 ? s : 0));
                c.factor = wood_factor(c.cell.x, c.cell.y, c.cell.z);
            }
            cs.push_back(c);
            if (contact_log_.size() < 64)   // debug: 2 = touching a face
                contact_log_.push_back(Vector4i(cx + (a == 0 ? s : 0), cy + (a == 1 ? s : 0), cz + (a == 2 ? s : 0), nc * 10 + 2));
        }
    }
    // The piece's own leaves do not hold it up; they shred against what they hit at speed, and leaves
    // meeting leaves both go (a crown crashing through a crown).
    for (size_t i = 0; i < p.leaf_points.size();) {
        const LeafPoint &lp = p.leaf_points[i];
        const Vector3 r = R.xform(lp.r);
        const Vector3 w = p.pos + r;
        const int cx = (int)std::floor(w.x), cy = (int)std::floor(w.y), cz = (int)std::floor(w.z);
        const uint8_t cl = cls_at(cx, cy, cz);
        const float speed = (p.vel + p.ang.cross(r)).length();
        bool shred = false;
        if (cl == LEAF || cl == CRUSH) shred = breaks(cx, cy, cz, speed);
        else if (cl == GROUND || cl == WOOD) shred = true;
        const uint8_t own = p.grid[lp.cell];
        if (shred && impact_k_ * speed * speed >= hardness_[own]) {
            note_break(cx, cy, cz, own);
            p.grid[lp.cell] = 0;
            p.mesh_dirty = true;
            mark_block(p, lp.cell);
            p.leaf_points[i] = p.leaf_points.back();
            p.leaf_points.pop_back();
            shed_++;
            continue;
        }
        i++;
    }
    (void)dt;
}

// Keeps a felled trunk's hinge point where it was (a ball joint, three axes of impulse with position
// correction), so gravity tips it over the edge of its cut; lets go at hinge_deg of tilt or hinge_s.
void RigidPieces::hold_hinge(Piece &p, float dt) {
    const Basis R(p.rot);
    if (std::acos(std::clamp(R.get_column(1).y, -1.0f, 1.0f)) >= Math::deg_to_rad(hinge_deg_) || p.age >= hinge_s_) {
        p.hinged = false;
        return;
    }
    const Basis Iw = R * p.inv_inertia * R.transposed();
    const Vector3 r = R.xform(p.hinge_r);
    const Vector3 err = p.hinge_world - (p.pos + r);
    for (int it = 0; it < 4; it++)
        for (int a = 0; a < 3; a++) {
            const Vector3 e(a == 0, a == 1, a == 2);
            const float k = 1.0f / (p.inv_mass + e.dot((Iw.xform(r.cross(e))).cross(r)));
            const float vr = (p.vel + p.ang.cross(r)).dot(e);
            const float dj = k * (err[a] * 0.3f / dt - vr);
            const Vector3 P = e * dj;
            p.vel += P * p.inv_mass;
            p.ang += Iw.xform(r.cross(P));
        }
}

void RigidPieces::solve(Piece &p, std::vector<Contact> &cs, float dt) {
    if (cs.empty()) return;
    const Basis R(p.rot);
    const Basis Iw = R * p.inv_inertia * R.transposed();
    auto k_of = [&](const Vector3 &r, const Vector3 &d) {
        const Vector3 rd = r.cross(d);
        return 1.0f / (p.inv_mass + d.dot((Iw.xform(rd)).cross(r)));
    };
    std::vector<float> target(cs.size());
    for (size_t i = 0; i < cs.size(); i++) {
        Contact &c = cs[i];
        c.mass_n = k_of(c.r, c.n);
        c.t1 = std::fabs(c.n.x) > 0.5f ? Vector3(0, 1, 0).cross(c.n).normalized() : Vector3(1, 0, 0).cross(c.n).normalized();
        c.t2 = c.n.cross(c.t1);
        c.mass_t1 = k_of(c.r, c.t1);
        c.mass_t2 = k_of(c.r, c.t2);
        const float vn = (p.vel + p.ang.cross(c.r)).dot(c.n);
        float t = std::min(std::max(c.depth - 0.05f, 0.0f) * 0.25f / dt, 12.0f);   // Baumgarte, capped
        if (vn < -8.0f) t = std::max(t, -restitution_ * vn);
        target[i] = t;
    }
    // split the impulse over the contacts: many points touch at once on a resting log
    for (int it = 0; it < 8; it++) {
        for (size_t i = 0; i < cs.size(); i++) {
            Contact &c = cs[i];
            Vector3 vr = p.vel + p.ang.cross(c.r);
            float dj = c.mass_n * (target[i] - vr.dot(c.n));
            const float jn = std::max(c.jn + dj, 0.0f);
            dj = jn - c.jn;
            c.jn = jn;
            Vector3 P = c.n * dj;
            p.vel += P * p.inv_mass;
            p.ang += Iw.xform(c.r.cross(P));
            vr = p.vel + p.ang.cross(c.r);
            const float lim = friction_ * c.jn;
            float d1 = -c.mass_t1 * vr.dot(c.t1);
            const float j1 = std::clamp(c.jt1 + d1, -lim, lim);
            d1 = j1 - c.jt1;
            c.jt1 = j1;
            float d2 = -c.mass_t2 * vr.dot(c.t2);
            const float j2 = std::clamp(c.jt2 + d2, -lim, lim);
            d2 = j2 - c.jt2;
            c.jt2 = j2;
            P = c.t1 * d1 + c.t2 * d2;
            p.vel += P * p.inv_mass;
            p.ang += Iw.xform(c.r.cross(P));
        }
    }
    // wood carrying more of the piece's weight than its hardness (times its thickness factor) allows
    // snaps: a leaning tree breaks the branches it leans on even when it moves slowly
    for (const Contact &c : cs) {
        if (c.factor <= 0.0f) continue;
        bool open;
        const uint8_t m = mat_at(c.cell.x, c.cell.y, c.cell.z, open);
        if (open && cls_[m] == WOOD) p.load = std::max(p.load, c.jn / dt / (hardness_[m] * load_k_ * c.factor));
        if (open && cls_[m] == WOOD && c.jn / dt >= hardness_[m] * load_k_ * c.factor && breaks_tick_ < max_breaks_) {
            set_at(c.cell.x, c.cell.y, c.cell.z, 0);
            note_break(c.cell.x, c.cell.y, c.cell.z, m);
            snapped_.push_back(c.cell);
            crushed_++;
            breaks_tick_++;
        }
    }
}

PackedInt32Array RigidPieces::step(float p_dt) {
    PackedInt32Array done;
    if (pieces_.empty() && snapped_.empty()) return done;
    sim_->wait_idle();
    step_impl(p_dt, done);
    return done;
}

void RigidPieces::step_impl(float p_dt, PackedInt32Array &done) {
    if (pieces_.empty() && snapped_.empty()) return;
    const double t0 = now_ms();
    breaks_tick_ = 0;
    const float h = p_dt / (float)substeps_;
    std::vector<Contact> cs;
    int contacts = 0;
    for (size_t i = 0; i < pieces_.size();) {
        Piece &p = pieces_[i];
        p.prev_pos = p.pos;
        p.prev_rot = p.rot;
        int touching = 0;
        p.load = 0.0f;
        for (int s = 0; s < substeps_; s++) {
            p.vel.y -= GRAVITY * h;
            collide(p, h, cs);
            solve(p, cs, h);
            if (p.hinged) hold_hinge(p, h);
            {   // debug: the most contacts of each kind in any substep of this tick
                int w = 0, g = 0, in = 0;
                for (const Contact &c : cs) {
                    (c.factor > 0.0f ? w : g)++;
                    in += c.depth > 0.5f;   // a point inside a solid cell, not just touching a face
                }
                if (s == 0) p.wood_touch = p.ground_touch = p.inside = 0;
                p.wood_touch = std::max(p.wood_touch, w);
                p.ground_touch = std::max(p.ground_touch, g);
                p.inside = std::max(p.inside, in);
            }
            touching = std::max(touching, (int)cs.size());
            p.pos += p.vel * h;
            const Quaternion dq(p.ang.x * 0.5f * h, p.ang.y * 0.5f * h, p.ang.z * 0.5f * h, 0.0f);
            p.rot = (p.rot + dq * p.rot).normalized();
            p.ang *= 1.0f - 0.3f * h;
        }
        contacts += touching;
        p.age += p_dt;
        const bool slow = touching > 0 && p.vel.length() < 1.5f && p.ang.length() < 0.1f;
        p.still = slow ? p.still + 1 : 0;
        const bool lost = p.pos.y < origin_.y - 64;   // fell out of the window
        if (p.still >= sleep_ticks_ || p.age >= max_age_ || lost) {
            if (rest_.size() < 16) {   // debug: what it came to rest on (the last substep's contacts)
                int ng = 0, nw = 0;
                float fmax = 0.0f, fsum = 0.0f, hsum = 0.0f;
                for (const Contact &c : cs) {
                    if (c.factor > 0.0f) {
                        nw++;
                        fmax = std::max(fmax, c.factor);
                        fsum += c.factor;
                    } else {
                        ng++;
                    }
                    hsum += c.r.y;
                }
                Dictionary r;
                r["id"] = p.id;
                r["tilt_deg"] = Math::rad_to_deg(std::acos(std::clamp(Basis(p.rot).get_column(1).y, -1.0f, 1.0f)));
                r["ground"] = ng;
                r["wood"] = nw;
                r["wood_factor_max"] = fmax;
                r["wood_factor_mean"] = nw ? fsum / nw : 0.0f;
                r["contact_dy_m"] = cs.empty() ? 0.0f : hsum / cs.size() * 0.125f;   // mean height of contacts over the centre of mass
                r["age_s"] = p.age;
                rest_.push_back(r);
            }
            if (!lost) settle(p);
            done.push_back(p.id);
            pieces_.erase(pieces_.begin() + i);
            settled_++;
            continue;
        }
        i++;
    }
    last_contacts_ = contacts;
    // limbs the pieces broke through: do they hang on anything? Checked together every few ticks
    if (!snapped_.empty() && (++recheck_wait_ >= recheck_every_ || pieces_.empty())) {
        recheck_wait_ = 0;
        std::vector<Vector3i> cells;
        cells.swap(snapped_);
        snapped_total_ += (int)cells.size();
        const Vector3 from = pieces_.empty() ? Vector3(cells[0]) : pieces_[0].pos;
        check_cells(cells, from, false);
    }
    last_step_ms_ = now_ms() - t0;
    max_step_ms_ = std::max(max_step_ms_, last_step_ms_);
}

// Every grid cell the piece covers takes the material of the piece voxel its centre falls in (inverse
// map, so a rotated piece leaves no holes); only empty cells, plants and leaves are overwritten.
bool RigidPieces::settle(Piece &p) {
    const double t0 = now_ms();
    const Basis R(p.rot);
    const Basis Ri = R.transposed();
    int total = 0;
    for (uint8_t m : p.grid) total += m != 0;
    // world box of the rotated local box; each world row maps linearly into local space, clipped to it
    Vector3 wlo(1e9f, 1e9f, 1e9f), whi(-1e9f, -1e9f, -1e9f);
    for (int c = 0; c < 8; c++) {
        const Vector3 corner((c & 1) ? p.size.x : 0, (c & 2) ? p.size.y : 0, (c & 4) ? p.size.z : 0);
        const Vector3 w = p.pos + R.xform(corner - p.com);
        wlo = wlo.min(w);
        whi = whi.max(w);
    }
    const Vector3 dl = Ri.xform(Vector3(1, 0, 0));   // local step per world x
    const float S[3] = {(float)p.size.x, (float)p.size.y, (float)p.size.z};
    int written = 0;
    const int x0 = (int)std::floor(wlo.x), x1 = (int)std::floor(whi.x);
    for (int wz = (int)std::floor(wlo.z); wz <= (int)std::floor(whi.z); wz++)
        for (int wy = (int)std::floor(wlo.y); wy <= (int)std::floor(whi.y); wy++) {
            const Vector3 l0 = Ri.xform(Vector3(x0 + 0.5f, wy + 0.5f, wz + 0.5f) - p.pos) + p.com;
            // the steps t where l0 + t * dl lies inside [0, size) on every axis
            float t0 = 0.0f, t1 = (float)(x1 - x0);
            for (int a = 0; a < 3 && t0 <= t1; a++) {
                if (std::fabs(dl[a]) < 1e-6f) {
                    if (l0[a] < 0.0f || l0[a] >= S[a]) t1 = -1.0f;
                    continue;
                }
                float ta = (0.0f - l0[a]) / dl[a], tb = (S[a] - l0[a]) / dl[a];
                if (ta > tb) std::swap(ta, tb);
                t0 = std::max(t0, ta);
                t1 = std::min(t1, tb);
            }
            if (t0 > t1) continue;
            for (int t = std::max(0, (int)std::floor(t0)); t <= std::min(x1 - x0, (int)std::ceil(t1)); t++) {
                const Vector3 l = l0 + dl * (float)t;
                const int lx = (int)std::floor(l.x), ly = (int)std::floor(l.y), lz = (int)std::floor(l.z);
                if (lx < 0 || ly < 0 || lz < 0 || lx >= p.size.x || ly >= p.size.y || lz >= p.size.z) continue;
                const uint8_t m = p.grid[lx + p.size.x * (ly + p.size.y * lz)];
                if (!m) continue;
                const int wx = x0 + t;
                bool open;
                const uint8_t have = mat_at(wx, wy, wz, open);
                if (!open) continue;
                const uint8_t c = cls_[have];
                if (c == GROUND || c == WOOD) continue;
                set_at(wx, wy, wz, m);
                written++;
            }
        }
    last_written_ = written;
    last_lost_ = total - written;
    last_settle_ms_ = now_ms() - t0;
    max_settle_ms_ = std::max(max_settle_ms_, last_settle_ms_);
    return written > 0;
}

// ---- queries ----

const RigidPieces::Piece *RigidPieces::find(int id) const {
    for (const Piece &p : pieces_)
        if (p.id == id) return &p;
    return nullptr;
}

PackedInt32Array RigidPieces::get_crumbs() const { return crumbs_; }

PackedInt32Array RigidPieces::take_breaks() {
    PackedInt32Array a = breaks_;
    breaks_.clear();
    return a;
}

PackedInt32Array RigidPieces::take_dirty_meshes() {
    PackedInt32Array a;
    for (Piece &p : pieces_)
        if (p.mesh_dirty) {
            p.mesh_dirty = false;
            a.push_back(p.id);
        }
    return a;
}

PackedInt32Array RigidPieces::take_spawned() {
    PackedInt32Array a = spawned_ids_;
    spawned_ids_.clear();
    return a;
}

PackedInt32Array RigidPieces::get_ids() const {
    PackedInt32Array a;
    for (const Piece &p : pieces_) a.push_back(p.id);
    return a;
}

const RigidPieces::View *RigidPieces::find_view(int id) const {
    for (const View &v : view_)
        if (v.id == id) return &v;
    return nullptr;
}

Transform3D RigidPieces::get_transform(int id, float alpha) const {
    if (async_) {
        const View *v = find_view(id);
        if (!v) return Transform3D();
        alpha = std::clamp(alpha, 0.0f, 1.0f);
        const Basis b(v->prev_rot.slerp(v->rot, alpha));
        return Transform3D(b, v->prev_pos.lerp(v->pos, alpha) - b.xform(v->com));
    }
    const Piece *p = find(id);
    if (!p) return Transform3D();
    alpha = std::clamp(alpha, 0.0f, 1.0f);
    const Basis b(p->prev_rot.slerp(p->rot, alpha));
    const Vector3 pos = p->prev_pos.lerp(p->pos, alpha);
    return Transform3D(b, pos - b.xform(p->com));
}

Dictionary RigidPieces::get_contact_info(int id) const {
    Dictionary d;
    const View *v = find_view(id);
    if (!v) return d;
    d["load"] = v->load;
    d["wood"] = v->wood_touch;
    d["ground"] = v->ground_touch;
    d["inside"] = v->inside;
    d["hinged"] = v->hinged;
    d["spin"] = v->spin;
    return d;
}

Vector3 RigidPieces::get_hash_origin(int id) const {
    if (async_) {
        const View *v = find_view(id);
        return v ? v->hash_origin : Vector3();
    }
    const Piece *p = find(id);
    return p ? p->hash_origin : Vector3();
}

Array RigidPieces::build_mesh(int id) const {
    const Piece *p = find(id);
    return p ? mesh_arrays(*p) : Array();
}

// A removed cell changes its own block's mesh and, through the faces it uncovers, its neighbours'.
void RigidPieces::mark_block(Piece &p, int cell) {
    const int x = cell % p.size.x, y = cell / p.size.x % p.size.y, z = cell / p.size.x / p.size.y;
    for (int e = -1; e < 6; e++) {
        const int nx = x + (e < 0 ? 0 : DX[e]), ny = y + (e < 0 ? 0 : DY[e]), nz = z + (e < 0 ? 0 : DZ[e]);
        if (nx < 0 || ny < 0 || nz < 0 || nx >= p.size.x || ny >= p.size.y || nz >= p.size.z) continue;
        p.block_dirty[nx / MESH_BLOCK + p.blocks.x * (ny / MESH_BLOCK + p.blocks.y * (nz / MESH_BLOCK))] = 1;
    }
}

Array RigidPieces::mesh_arrays(const Piece &piece, int block) const {
    Array arrays;
    const Piece *p = &piece;
    // corners of each face, wound clockwise seen from outside (Godot's front faces)
    static const int C[6][4][3] = {
            {{1, 0, 0}, {1, 0, 1}, {1, 1, 1}, {1, 1, 0}},
            {{0, 0, 0}, {0, 1, 0}, {0, 1, 1}, {0, 0, 1}},
            {{0, 1, 0}, {1, 1, 0}, {1, 1, 1}, {0, 1, 1}},
            {{0, 0, 0}, {0, 0, 1}, {1, 0, 1}, {1, 0, 0}},
            {{0, 0, 1}, {0, 1, 1}, {1, 1, 1}, {1, 0, 1}},
            {{0, 0, 0}, {1, 0, 0}, {1, 1, 0}, {0, 1, 0}},
    };
    PackedVector3Array verts, normals;
    PackedVector2Array uvs;
    PackedColorArray cols;
    PackedInt32Array idx;
    const Vector3i S = p->size;
    int lo[3] = {0, 0, 0}, hi[3] = {S.x, S.y, S.z};
    if (block >= 0) {
        const int b[3] = {block % p->blocks.x, block / p->blocks.x % p->blocks.y, block / p->blocks.x / p->blocks.y};
        for (int a = 0; a < 3; a++) {
            lo[a] = b[a] * MESH_BLOCK;
            hi[a] = std::min(lo[a] + MESH_BLOCK, a == 0 ? S.x : (a == 1 ? S.y : S.z));
        }
    }
    for (int z = lo[2]; z < hi[2]; z++)
        for (int y = lo[1]; y < hi[1]; y++)
            for (int x = lo[0]; x < hi[0]; x++) {
                const uint8_t m = p->grid[x + S.x * (y + S.y * z)];
                if (!m) continue;
                for (int f = 0; f < 6; f++) {
                    const int nx = x + DX[f], ny = y + DY[f], nz = z + DZ[f];
                    const uint8_t nm = (nx < 0 || ny < 0 || nz < 0 || nx >= S.x || ny >= S.y || nz >= S.z) ? 0 : p->grid[nx + S.x * (ny + S.y * nz)];
                    if (nm && !(transparent_[nm] && nm != m)) continue;
                    const int tile = tiles_[m][f == 2 ? 0 : (f == 3 ? 2 : 1)];
                    const Vector2 uv((tile % atlas_tiles_.x + 0.5f) / atlas_tiles_.x, (tile / atlas_tiles_.x + 0.5f) / atlas_tiles_.y);
                    const int base = verts.size();
                    const Vector3 n(DX[f], DY[f], DZ[f]);
                    for (int k = 0; k < 4; k++) {
                        verts.push_back(Vector3(x + C[f][k][0], y + C[f][k][1], z + C[f][k][2]));
                        normals.push_back(n);
                        uvs.push_back(uv);
                        cols.push_back(Color(1, 1, 1));
                    }
                    idx.push_back(base);
                    idx.push_back(base + 1);
                    idx.push_back(base + 2);
                    idx.push_back(base);
                    idx.push_back(base + 2);
                    idx.push_back(base + 3);
                }
            }
    arrays.resize(Mesh::ARRAY_MAX);
    arrays[Mesh::ARRAY_VERTEX] = verts;
    arrays[Mesh::ARRAY_NORMAL] = normals;
    arrays[Mesh::ARRAY_TEX_UV] = uvs;
    arrays[Mesh::ARRAY_COLOR] = cols;
    arrays[Mesh::ARRAY_INDEX] = idx;
    return arrays;
}

Dictionary RigidPieces::get_info(int id) const {
    Dictionary d;
    const Piece *p = find(id);
    if (!p) return d;
    d["id"] = p->id;
    d["wood"] = p->wood;
    d["leaves"] = p->leaves;
    d["size"] = p->size;
    d["mass"] = p->mass;
    d["points"] = (int)p->points.size();
    d["leaf_points"] = (int)p->leaf_points.size();
    d["pos"] = p->pos;
    d["vel"] = p->vel;
    d["ang"] = p->ang;
    d["hinged"] = p->hinged;
    d["hinge_r"] = p->hinge_r;   // local, from the centre of mass
    d["com"] = p->com;
    d["age"] = p->age;
    d["tilt_deg"] = Math::rad_to_deg(std::acos(std::clamp(Basis(p->rot).get_column(1).y, -1.0f, 1.0f)));
    return d;
}

Dictionary RigidPieces::get_stats() const {
    return async_ ? stats_view_ : live_stats();
}

Dictionary RigidPieces::live_stats() const {
    Dictionary d;
    d["last_job_ms"] = last_job_ms_;
    Array made;
    for (const Vector3i &m : made_) made.push_back(m);
    d["made"] = made;   // (id, wood, leaves) of the first 16 pieces
    d["rest"] = rest_.duplicate();
    d["max_job_ms"] = max_job_ms_;
    d["pieces"] = (int)pieces_.size();
    d["spawned"] = spawned_;
    d["settled"] = settled_;
    d["crushed"] = crushed_;
    d["last_check_ms"] = last_check_ms_;
    d["max_check_ms"] = max_check_ms_;
    d["last_filled"] = last_filled_;
    d["last_step_ms"] = last_step_ms_;
    d["max_step_ms"] = max_step_ms_;
    d["last_contacts"] = last_contacts_;
    d["hits_ground"] = hit_ground_;
    d["hits_wood"] = hit_wood_;
    d["snapped"] = snapped_total_;
    d["torn"] = torn_;   // wood torn along strength splits
    Array cl;
    for (const Vector4i &c : contact_log_) cl.push_back(c);
    d["contact_log"] = cl;   // the first 64 contacts: world cell, class * 10 + (1 inside, 2 at a face)
    d["shed"] = shed_;
    d["last_wood_hit"] = hit_at_;
    d["last_settle_ms"] = last_settle_ms_;
    d["max_settle_ms"] = max_settle_ms_;
    d["last_written"] = last_written_;
    d["last_lost"] = last_lost_;
    d["last_paths"] = last_paths_;
    d["last_check_split_ms"] = Vector4(split_ms_[0], split_ms_[1], split_ms_[2], split_ms_[3]);   // fill, strength, leaves, make
    d["last_region"] = last_region_;
    d["last_side"] = last_side_;   // wood on the sources' side of the narrowest cut (-1: held before it was found)
    d["last_strength_ms"] = last_strength_ms_;
    Array an;
    for (const Vector3i &a : anchors_) an.push_back(a);
    d["last_anchors"] = an;
    Array fl;
    for (const Vector3i &f : fills_) fl.push_back(f);
    d["last_fills"] = fl;   // (seed y, voxels, 0 free / 1 ground / 2 joined a supported fill / 3 cap)
    return d;
}

// ---- async: the work runs on the sim's driver thread ----

void RigidPieces::set_async(bool p_on) {
    if (sim_.is_null() || p_on == async_) return;
    sim_->wait_idle();
    async_ = p_on;
    if (p_on) sim_->pre_tick_ = [this]() { run_job(); };
    else sim_->pre_tick_ = nullptr;
}

void RigidPieces::queue_check(const Array &p_cells, Vector3 p_push_from) {
    PendingCheck c;
    c.cells.resize(p_cells.size());
    for (int i = 0; i < p_cells.size(); i++) c.cells[i] = p_cells[i];
    c.from = p_push_from;
    std::lock_guard<std::mutex> lk(job_m_);
    pending_.push_back(std::move(c));
}

void RigidPieces::queue_check_packed(const PackedInt32Array &p_cells, Vector3 p_push_from) {
    PendingCheck c;
    c.cells.reserve(p_cells.size() / 3);
    for (int64_t i = 0; i + 2 < p_cells.size(); i += 3) c.cells.push_back(Vector3i(p_cells[i], p_cells[i + 1], p_cells[i + 2]));
    c.from = p_push_from;
    std::lock_guard<std::mutex> lk(job_m_);
    pending_.push_back(std::move(c));
}

void RigidPieces::queue_step(float p_dt) {
    std::lock_guard<std::mutex> lk(job_m_);
    pending_dt_ += p_dt;
}

void RigidPieces::run_job() {
    std::vector<PendingCheck> todo;
    float dt;
    {
        std::lock_guard<std::mutex> lk(job_m_);
        todo.swap(pending_);
        dt = pending_dt_;
        pending_dt_ = 0.0f;
    }
    if (todo.empty() && (dt <= 0.0f || (pieces_.empty() && snapped_.empty()))) return;
    const double t0 = now_ms();
    for (const PendingCheck &c : todo) check_cells(c.cells, c.from, true);
    if (dt > 0.0f) step_impl(std::min(dt, 0.1f), settled_out_);
    // meshes for new pieces (every block), and the blocks of pieces that shed leaves (spaced)
    for (Piece &p : pieces_) {
        p.mesh_wait++;
        if (!p.mesh_new && !(p.mesh_dirty && p.mesh_wait >= mesh_every_)) continue;
        Dictionary blocks = meshes_out_.get(p.id, Dictionary());
        for (int b = 0; b < (int)p.block_dirty.size(); b++) {
            if (!p.mesh_new && !p.block_dirty[b]) continue;
            Array arr = mesh_arrays(p, b);
            if (p.mesh_new && ((PackedInt32Array)arr[Mesh::ARRAY_INDEX]).is_empty()) continue;   // nothing there
            blocks[b] = arr;
            p.block_dirty[b] = 0;
        }
        meshes_out_[p.id] = blocks;
        p.mesh_new = p.mesh_dirty = false;
        p.mesh_wait = 0;
    }
    last_job_ms_ = now_ms() - t0;
    max_job_ms_ = std::max(max_job_ms_, last_job_ms_);
}

Dictionary RigidPieces::collect() {
    Dictionary d;
    if (sim_.is_null()) return d;
    sim_->wait_idle();   // a no-op between ticks, where the game calls it
    d["spawned"] = take_spawned();
    d["settled"] = settled_out_;
    settled_out_.clear();
    d["meshes"] = meshes_out_;
    meshes_out_ = Dictionary();
    d["breaks"] = take_breaks();
    d["crumbs"] = crumbs_;
    crumbs_.clear();
    view_.clear();
    for (const Piece &p : pieces_)
        view_.push_back({p.id, p.prev_pos, p.pos, p.com, p.hash_origin, p.prev_rot, p.rot, p.load, p.wood_touch, p.ground_touch, p.inside, p.hinged, p.ang.length()});
    stats_view_ = live_stats();
    return d;
}
