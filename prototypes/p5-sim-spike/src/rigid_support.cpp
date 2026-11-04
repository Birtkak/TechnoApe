// ADR 0142 (mines need shoring): the support rule for roofs the player dug, on RigidPieces' cells.
//
// Not a stress solver. A roof cell is a solid cell with open space under it. Walking the roof surface
// (the 8 columns around, stepping up or down at most STEP voxels), a step into rock that goes on down
// is a wall; a step into a rise of more than STEP is an edge (a shaft), which holds nothing. Every roof
// cell's distance to the nearest wall (Dijkstra over the surface, octile steps) must stay within its
// material's span, or a prop's radius must cover it. A prop carries the roof cells only it holds, up to
// its capacity; an overloaded prop fails and holds nothing. Only roof cells over a dug 0.5 m block can
// fail: caves and halls the generator made stand as generated. Scans are local (the dug box plus two
// reaches around it) and run when something was dug or a prop changed, never over the world.

#include "rigid_pieces.h"

#include <algorithm>
#include <chrono>
#include <cmath>

using namespace godot;

namespace {

double now_ms() {
    return std::chrono::duration<double, std::milli>(std::chrono::steady_clock::now().time_since_epoch()).count();
}

constexpr int BLOCK = 4;          // Settings.BLOCK: dug marks are kept per 0.5 m block
constexpr int STEP = 3;           // a roof may step this many voxels between neighbouring columns
constexpr int MAX_NODES = 80000;  // roof cells per scan (a 35 m square of open roof)
constexpr size_t MAX_DUG = 4000000;
constexpr uint16_t INF = 0xFFFF;
const int HX[8] = {1, -1, 0, 0, 1, 1, -1, -1}, HZ[8] = {0, 0, 1, -1, 1, -1, 1, -1};

// the same key as rigid_pieces.cpp (make_piece takes cells by it)
constexpr int64_t KOFF = 1 << 20;
inline int64_t key(int x, int y, int z) { return ((int64_t)(x + KOFF) << 42) | ((int64_t)(y + KOFF) << 21) | (int64_t)(z + KOFF); }
inline int fdiv(int a, int b) { return a >= 0 ? a / b : -((-a + b - 1) / b); }
inline int64_t bkey(int x, int y, int z) { return key(fdiv(x, BLOCK), fdiv(y, BLOCK), fdiv(z, BLOCK)); }

struct Node {
    int x, y, z;
    float span;
    uint16_t dw = INF, dp = INF;   // half voxels to the nearest wall / prop (prop: offset by its radius)
    int16_t lab = -1;              // nearest prop
    uint8_t dug = 0, wall = 0, bad = 0, seen = 0;
    int32_t nb[8];
};

}   // namespace

void RigidPieces::support_setup(const PackedFloat32Array &p_spans) {
    span_.assign(p_spans.size(), -1.0f);
    for (int64_t i = 0; i < p_spans.size(); i++) span_[i] = p_spans[i];
}

void RigidPieces::support_mark(const PackedInt32Array &p_cells) {
    std::lock_guard<std::mutex> lk(job_m_);
    Vector3i last(INT32_MIN, 0, 0);
    for (int64_t i = 0; i + 2 < p_cells.size(); i += 3) {
        const Vector3i b(fdiv(p_cells[i], BLOCK), fdiv(p_cells[i + 1], BLOCK), fdiv(p_cells[i + 2], BLOCK));
        if (b != last) dug_pending_.push_back(b);
        last = b;
    }
}

void RigidPieces::support_mark_box(Vector3i p_lo, Vector3i p_hi) {
    std::lock_guard<std::mutex> lk(job_m_);
    const Vector3i a(fdiv(p_lo.x, BLOCK), fdiv(p_lo.y, BLOCK), fdiv(p_lo.z, BLOCK));
    const Vector3i b(fdiv(p_hi.x, BLOCK), fdiv(p_hi.y, BLOCK), fdiv(p_hi.z, BLOCK));
    if ((int64_t)(b.x - a.x + 1) * (b.y - a.y + 1) * (b.z - a.z + 1) > 20000) return;   // a creative mass edit: not a mine
    for (int y = a.y; y <= b.y; y++)
        for (int z = a.z; z <= b.z; z++)
            for (int x = a.x; x <= b.x; x++) dug_pending_.push_back(Vector3i(x, y, z));
}

void RigidPieces::support_apply_marks() {
    std::vector<Vector3i> todo;
    {
        std::lock_guard<std::mutex> lk(job_m_);
        todo.swap(dug_pending_);
    }
    // shortcut: past MAX_DUG blocks (a 500 m tunnel network) the marks start over; a ring buffer if that matters
    if (dug_.size() + todo.size() > MAX_DUG) dug_.clear();
    for (const Vector3i &b : todo) dug_.insert(key(b.x, b.y, b.z));
}

RigidPieces::ScanJob RigidPieces::scan_job_of(const Dictionary &d) {
    ScanJob j;
    j.lo = d.get("lo", Vector3i());
    j.hi = d.get("hi", j.lo);
    j.reach = d.get("reach", 32);
    j.tag = d.get("tag", 0);
    j.has_probe = d.has("probe");
    if (j.has_probe) j.probe = d["probe"];
    const PackedInt32Array pr = d.get("props", PackedInt32Array());
    j.props.assign(pr.ptr(), pr.ptr() + pr.size());
    const PackedInt32Array ck = d.get("check", PackedInt32Array());
    j.check.assign(ck.ptr(), ck.ptr() + ck.size());
    return j;
}

Dictionary RigidPieces::support_scan(const Dictionary &p_job) {
    if (sim_.is_null()) return Dictionary();
    sim_->wait_idle();
    support_apply_marks();
    return support_scan_impl(scan_job_of(p_job));
}

int RigidPieces::support_drop(const PackedInt32Array &p_roof, int p_thick, Vector3 p_from) {
    if (sim_.is_null()) return 0;
    sim_->wait_idle();
    DropJob j;
    for (int64_t i = 0; i + 2 < p_roof.size(); i += 3) j.roof.push_back(Vector3i(p_roof[i], p_roof[i + 1], p_roof[i + 2]));
    j.thick = p_thick;
    j.from = p_from;
    return support_drop_impl(j);
}

void RigidPieces::queue_support_scan(const Dictionary &p_job) {
    ScanJob j = scan_job_of(p_job);
    std::lock_guard<std::mutex> lk(job_m_);
    scan_jobs_.push_back(std::move(j));
}

void RigidPieces::queue_support_drop(const PackedInt32Array &p_roof, int p_thick, Vector3 p_from) {
    DropJob j;
    for (int64_t i = 0; i + 2 < p_roof.size(); i += 3) j.roof.push_back(Vector3i(p_roof[i], p_roof[i + 1], p_roof[i + 2]));
    j.thick = p_thick;
    j.from = p_from;
    std::lock_guard<std::mutex> lk(job_m_);
    drop_jobs_.push_back(std::move(j));
}

void RigidPieces::support_run_jobs() {
    std::vector<ScanJob> scans;
    std::vector<DropJob> drops;
    {
        std::lock_guard<std::mutex> lk(job_m_);
        if (dug_pending_.empty() && scan_jobs_.empty() && drop_jobs_.empty()) return;
        scans.swap(scan_jobs_);
        drops.swap(drop_jobs_);
    }
    support_apply_marks();
    for (const DropJob &d : drops) {
        Dictionary r;
        r["drop"] = support_drop_impl(d);
        support_out_.push_back(r);
    }
    for (const ScanJob &s : scans) support_out_.push_back(support_scan_impl(s));
}

Dictionary RigidPieces::support_stats() const {
    Dictionary d;
    d["dug_blocks"] = (int64_t)dug_.size();
    d["scans"] = support_scans_;
    d["drops"] = support_drops_;
    d["last_ms"] = support_last_ms_;
    d["max_ms"] = support_max_ms_;
    d["last_nodes"] = support_last_nodes_;
    return d;
}

void RigidPieces::support_clear() {
    std::lock_guard<std::mutex> lk(job_m_);
    dug_pending_.clear();
    dug_.clear();
}

Dictionary RigidPieces::support_scan_impl(const ScanJob &j) {
    const double t0 = now_ms();
    Dictionary out;
    out["tag"] = j.tag;
    if (sim_.is_null() || span_.empty()) return out;
    // the cell's span: < 0 open, else solid; outside the window or unloaded it is solid and holds (0)
    auto span_at = [&](int wx, int wy, int wz) -> float {
        const int x = wx - origin_.x, y = wy - origin_.y, z = wz - origin_.z;
        if (!sim_->is_open(x, y, z)) return 0.0f;
        const uint8_t m = sim_->cells_[sim_->idx(x, y, z)].mat;
        return m < span_.size() ? span_[m] : -1.0f;
    };
    auto solid = [&](int x, int y, int z) { return span_at(x, y, z) >= 0.0f; };
    const int R = std::max(4, j.reach);
    // the dug box, at most 6 m across and 5 m high (a long cut is scanned around its middle)
    Vector3i dlo = j.lo, dhi = j.hi;
    for (int a : {0, 2})
        if (dhi[a] - dlo[a] > 48) {
            const int c = (dlo[a] + dhi[a]) / 2;
            dlo[a] = c - 24;
            dhi[a] = c + 24;
        }
    if (dhi.y - dlo.y > 40) dlo.y = dhi.y - 40;
    const Vector3i lo(dlo.x - 2 * R, dlo.y - 8, dlo.z - 2 * R), hi(dhi.x + 2 * R, dhi.y + 24, dhi.z + 2 * R);
    const int bx = hi.x - lo.x + 1, by = hi.y - lo.y + 1, bz = hi.z - lo.z + 1;
    const size_t bn = (size_t)bx * by * bz;
    if (s_node_.size() < bn) {
        s_node_.resize(bn);
        s_stamp_.assign(bn, 0);
        s_gen_ = 0;
    }
    if (++s_gen_ == 0) {
        std::fill(s_stamp_.begin(), s_stamp_.end(), 0);
        s_gen_ = 1;
    }
    std::vector<Node> nodes;
    nodes.reserve(4096);
    auto add = [&](int x, int y, int z) -> int {
        if (x < lo.x || y < lo.y || z < lo.z || x > hi.x || y > hi.y || z > hi.z) return -1;
        const size_t b = (size_t)(x - lo.x) + (size_t)bx * ((size_t)(z - lo.z) + (size_t)bz * (size_t)(y - lo.y));
        if (s_stamp_[b] == s_gen_) return s_node_[b];
        if ((int)nodes.size() >= MAX_NODES) return -1;
        s_stamp_[b] = s_gen_;
        s_node_[b] = (int32_t)nodes.size();
        Node n;
        n.x = x;
        n.y = y;
        n.z = z;
        n.span = span_at(x, y, z);
        n.dug = dug_.count(bkey(x, y - 1, z)) ? 1 : 0;   // the open cell under it was dug
        for (int k = 0; k < 8; k++) n.nb[k] = -1;
        nodes.push_back(n);
        return (int)nodes.size() - 1;
    };
    auto find = [&](int x, int y, int z) -> int {
        if (x < lo.x || y < lo.y || z < lo.z || x > hi.x || y > hi.y || z > hi.z) return -1;
        const size_t b = (size_t)(x - lo.x) + (size_t)bx * ((size_t)(z - lo.z) + (size_t)bz * (size_t)(y - lo.y));
        return s_stamp_[b] == s_gen_ ? s_node_[b] : -1;
    };
    // seeds: the roof over the dug box
    for (int z = dlo.z - 1; z <= dhi.z + 1; z++)
        for (int x = dlo.x - 1; x <= dhi.x + 1; x++) {
            bool below = solid(x, dlo.y - 2, z);
            for (int y = dlo.y - 1; y <= dhi.y + 2; y++) {
                const bool s = solid(x, y, z);
                if (s && !below) add(x, y, z);
                below = s;
            }
        }
    // the roof surface around them
    for (size_t i = 0; i < nodes.size(); i++) {
        const int x = nodes[i].x, y = nodes[i].y, z = nodes[i].z;
        for (int k = 0; k < 8; k++) {
            const int nx = x + HX[k], nz = z + HZ[k];
            int ny = INT32_MIN;
            if (solid(nx, y, nz)) {
                for (int s = 1; s <= STEP; s++)
                    if (!solid(nx, y - s, nz)) { ny = y - s + 1; break; }
                if (ny == INT32_MIN) {   // rock that goes on down: a wall (half a voxel off, or a diagonal's)
                    const uint8_t w = k < 4 ? 1 : 2;
                    if (!nodes[i].wall || w < nodes[i].wall) nodes[i].wall = w;
                    continue;
                }
            } else {
                for (int s = 1; s <= STEP; s++)
                    if (solid(nx, y + s, nz)) { ny = y + s; break; }
                if (ny == INT32_MIN) continue;   // the roof rises away (a shaft): no support there
            }
            const int m = add(nx, ny, nz);
            nodes[i].nb[k] = m;
        }
    }
    const int n = (int)nodes.size();
    int dug_nodes = 0;
    for (const Node &nd : nodes) dug_nodes += nd.dug;
    out["nodes"] = n;
    out["dug_nodes"] = dug_nodes;
    // distances to walls (Dial's buckets, half voxels: 2 straight, 3 diagonal)
    auto dial = [&](bool props, int cap, const std::vector<int> &src, const std::vector<int> &d0, const std::vector<int> &lab0) {
        std::vector<std::vector<int>> bk(cap + 1);
        for (Node &nd : nodes) {
            if (props) nd.dp = INF, nd.lab = -1;
            else nd.dw = INF;
        }
        for (size_t s = 0; s < src.size(); s++) {
            Node &nd = nodes[src[s]];
            uint16_t &d = props ? nd.dp : nd.dw;
            if (d0[s] < d && d0[s] <= cap) {
                d = (uint16_t)d0[s];
                if (props) nd.lab = (int16_t)lab0[s];
                bk[d0[s]].push_back(src[s]);
            }
        }
        for (int d = 0; d <= cap; d++)
            for (size_t h = 0; h < bk[d].size(); h++) {
                const int i = bk[d][h];
                if ((props ? nodes[i].dp : nodes[i].dw) != d) continue;
                for (int k = 0; k < 8; k++) {
                    const int m = nodes[i].nb[k];
                    if (m < 0) continue;
                    const int nd = d + (k < 4 ? 2 : 3);
                    uint16_t &dm = props ? nodes[m].dp : nodes[m].dw;
                    if (nd <= cap && nd < dm) {
                        dm = (uint16_t)nd;
                        if (props) nodes[m].lab = nodes[i].lab;
                        bk[nd].push_back(m);
                    }
                }
            }
    };
    {
        std::vector<int> src, d0, lab0;
        for (int i = 0; i < n; i++)
            if (nodes[i].wall) {
                src.push_back(i);
                d0.push_back(nodes[i].wall);
            }
        dial(false, 4 * R + 4, src, d0, lab0);
    }
    // props: each covers the roof within its radius; it carries the dug cells only it holds, up to its capacity
    const int np = (int)j.props.size() / 5;
    std::vector<int> pnode(np, -1), load(np, 0);
    std::vector<uint8_t> alive(np, 1);
    int rmax = 0;
    for (int p = 0; p < np; p++) {
        const int *q = &j.props[p * 5];
        for (int s = 0; s <= STEP && pnode[p] < 0; s++) pnode[p] = find(q[0], q[1] + s, q[2]);
        if (pnode[p] < 0) alive[p] = 0;   // it does not reach the roof
        rmax = std::max(rmax, q[3]);
    }
    const int pcap = 2 * rmax;
    auto held_by_wall = [&](const Node &nd) { return nd.span <= 0.0f || nd.dw <= 2.0f * nd.span; };
    for (int round = 0; round < 4 && np > 0; round++) {
        std::vector<int> src, d0, lab0;
        for (int p = 0; p < np; p++)
            if (alive[p]) {
                src.push_back(pnode[p]);
                d0.push_back(2 * (rmax - j.props[p * 5 + 3]));
                lab0.push_back(p);
            }
        dial(true, pcap, src, d0, lab0);
        std::fill(load.begin(), load.end(), 0);
        for (const Node &nd : nodes)
            if (nd.dug && !held_by_wall(nd) && nd.lab >= 0 && nd.dp <= pcap) load[nd.lab]++;
        bool failed = false;
        for (int p = 0; p < np; p++)
            if (alive[p] && load[p] > j.props[p * 5 + 4]) {
                alive[p] = 0;
                failed = true;
            }
        if (!failed) break;
        if (round == 3)   // still overloaded after the last pass: nothing holds by props any more
            for (Node &nd : nodes) nd.dp = INF;
    }
    Array props_out;
    for (int p = 0; p < np; p++) {
        Dictionary pd;
        pd["load"] = load[p];
        pd["capacity"] = j.props[p * 5 + 4];
        pd["touches"] = pnode[p] >= 0;
        pd["failed"] = pnode[p] >= 0 && !alive[p];
        props_out.push_back(pd);
    }
    out["props"] = props_out;
    // what fails: dug roof cells nothing holds, inside the zone where every support within reach was seen
    const int zx0 = dlo.x - R, zx1 = dhi.x + R, zz0 = dlo.z - R, zz1 = dhi.z + R;
    for (Node &nd : nodes)
        nd.bad = nd.dug && !held_by_wall(nd) && !(nd.dp <= pcap) && nd.x >= zx0 && nd.x <= zx1 && nd.z >= zz0 && nd.z <= zz1;
    Array clusters;
    std::vector<int> q;
    for (int i = 0; i < n; i++) {
        if (!nodes[i].bad || nodes[i].seen) continue;
        q.clear();
        q.push_back(i);
        nodes[i].seen = 1;
        for (size_t h = 0; h < q.size(); h++)
            for (int k = 0; k < 8; k++) {
                const int m = nodes[q[h]].nb[k];
                if (m >= 0 && nodes[m].bad && !nodes[m].seen) {
                    nodes[m].seen = 1;
                    q.push_back(m);
                }
            }
        PackedInt32Array cells;
        cells.resize(q.size() * 3);
        Vector3 c;
        float ratio = 0.0f;
        for (size_t h = 0; h < q.size(); h++) {
            const Node &nd = nodes[q[h]];
            cells.set(h * 3, nd.x);
            cells.set(h * 3 + 1, nd.y);
            cells.set(h * 3 + 2, nd.z);
            c += Vector3(nd.x, nd.y, nd.z);
            ratio = std::max(ratio, nd.dw == INF ? 9.9f : nd.dw / (2.0f * nd.span));
        }
        Dictionary cd;
        cd["cells"] = cells;
        cd["count"] = (int)q.size();
        cd["ratio"] = ratio;
        cd["center"] = c / (float)q.size() + Vector3(0.5f, 0.0f, 0.5f);
        clusters.push_back(cd);
    }
    out["clusters"] = clusters;
    out["zone_lo"] = Vector3i(zx0, lo.y, zz0);
    out["zone_hi"] = Vector3i(zx1, hi.y, zz1);
    // the roofs the caller already warns about, x y z state: 1 still failing, 0 seen and held now, -1 not reached
    PackedInt32Array checked;
    for (size_t c = 0; c + 2 < j.check.size(); c += 3) {
        const int i = find(j.check[c], j.check[c + 1], j.check[c + 2]);
        for (int a = 0; a < 3; a++) checked.push_back(j.check[c + a]);
        checked.push_back(i < 0 ? -1 : nodes[i].bad ? 1 : 0);
    }
    out["checked"] = checked;
    if (j.has_probe) {
        Dictionary pr;
        int i = -1;
        for (int s = 0; s <= STEP && i < 0; s++) {
            i = find(j.probe.x, j.probe.y + s, j.probe.z);
            if (i < 0) i = find(j.probe.x, j.probe.y - s, j.probe.z);
        }
        pr["found"] = i >= 0;
        if (i >= 0) {
            const Node &nd = nodes[i];
            pr["wall"] = nd.dw == INF ? -1.0f : nd.dw * 0.5f;   // voxels
            pr["span"] = nd.span;
            pr["prop"] = nd.dp <= pcap ? (int)nd.lab : -1;
            pr["dug"] = nd.dug != 0;
            pr["held"] = !nd.dug || held_by_wall(nd) || nd.dp <= pcap;
        }
        out["probe"] = pr;
    }
    support_last_nodes_ = n;
    support_scans_++;
    support_last_ms_ = now_ms() - t0;
    support_max_ms_ = std::max(support_max_ms_, support_last_ms_);
    out["ms"] = support_last_ms_;
    return out;
}

int RigidPieces::support_drop_impl(const DropJob &j) {
    std::vector<int64_t> keys;
    for (const Vector3i &c : j.roof)
        for (int t = 0; t < j.thick; t++) {
            bool open;
            const uint8_t m = mat_at(c.x, c.y + t, c.z, open);
            if (!open || m >= span_.size() || span_[m] <= 0.0f) break;   // only the rock that fails comes down
            keys.push_back(key(c.x, c.y + t, c.z));
        }
    std::sort(keys.begin(), keys.end());
    keys.erase(std::unique(keys.begin(), keys.end()), keys.end());
    if (keys.empty()) return 0;
    if ((int)keys.size() < min_piece_) {
        for (int64_t k : keys) {
            const int x = (int)((k >> 42) & 0x1FFFFF) - KOFF, y = (int)((k >> 21) & 0x1FFFFF) - KOFF, z = (int)(k & 0x1FFFFF) - KOFF;
            bool open;
            crumbs_.push_back(x);
            crumbs_.push_back(y);
            crumbs_.push_back(z);
            crumbs_.push_back(mat_at(x, y, z, open));
            set_at(x, y, z, 0);
        }
        return 0;
    }
    const int id = next_id_;
    make_piece(keys, std::vector<int64_t>(), j.from, false);
    Piece &p = pieces_.back();
    p.ang = Vector3();   // a roof drops, it does not topple
    p.vel = Vector3();
    support_drops_++;
    return id;
}
