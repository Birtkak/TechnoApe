#pragma once
// P12 (ADR 0037): rigid voxel pieces. A dig that cuts a group of voxels loose from the ground (a
// felled trunk with its crown, a cut pillar) turns that group into one rigid body: it falls, tumbles,
// crushes leaves and plants in its way, and when it comes to rest its voxels are written back into the
// grid. Works on a SimWorld's cells (the bubble), so the Noita kernel and the terrain paste see the
// result like any other edit.
//
// Support: every material has a rigid class (see setup): WOOD carries support, LEAF hangs on wood
// and carries nothing, GROUND anchors, EMPTY (air, liquids, gases, fire) and CRUSH (plants) are open.
// After an edit, a capped flood fill through WOOD from each wood face of the hole decides: touching
// ground, an unloaded chunk or the window edge, or growing past the cap, means supported. Supported
// wood above the hole must also be held strongly enough: a max flow to the ground counts the
// cross-section holding it, and if that carries less than the wood above the narrowest cut, the part
// above breaks off there (a chopped trunk falls on its hinge; a limb grown into the next tree snaps).
//
// Coordinates: pieces live in world voxels (1 unit = 12.5 cm); set_origin gives the world voxel of the
// window's local cell (0,0,0), so the wrapped window may move while a piece flies.

#include "sim_world.h"

#include <godot_cpp/classes/ref_counted.hpp>
#include <godot_cpp/variant/array.hpp>
#include <godot_cpp/variant/dictionary.hpp>
#include <godot_cpp/variant/packed_int32_array.hpp>
#include <godot_cpp/variant/packed_vector3_array.hpp>
#include <godot_cpp/variant/basis.hpp>
#include <godot_cpp/variant/quaternion.hpp>
#include <godot_cpp/variant/transform3d.hpp>
#include <godot_cpp/variant/vector2i.hpp>
#include <godot_cpp/variant/vector3.hpp>
#include <godot_cpp/variant/vector3i.hpp>
#include <godot_cpp/variant/vector4.hpp>
#include <godot_cpp/variant/vector4i.hpp>

#include <array>
#include <cstdint>
#include <mutex>
#include <unordered_set>
#include <vector>
#include <godot_cpp/variant/packed_float32_array.hpp>

namespace godot {

class RigidPieces : public RefCounted {
    GDCLASS(RigidPieces, RefCounted)

public:
    enum Class : uint8_t { EMPTY = 0, GROUND = 1, WOOD = 2, LEAF = 3, CRUSH = 4 };
    static constexpr int MESH_BLOCK = 16;   // async meshes come in blocks: a crown that sheds leaves rebuilds only where it did

    RigidPieces();
    ~RigidPieces();

    // defs: the same Array as SimWorld.set_materials (index = material id - 1). Reads "rigid"
    // ("wood" / "leaf"), "cell_type", "micro" and "replaceable" to pick each material's class, and
    // "tiles" ([top, side, bottom]) and atlas_tiles for the mesh UVs.
    void setup(const Ref<SimWorld> &p_sim, const Array &p_defs, Vector2i p_atlas_tiles);
    void set_origin(Vector3i p_world_cell);
    // Tunables: max_wood (flood-fill cap), leaf_reach (voxels), substeps, friction, restitution,
    // push (topple speed, rad/s), sleep_ticks, max_age_s, max_points, min_piece, strength (voxels held
    // per voxel of cross-section), region_cap, max_paths, thick (wood thinner than this breaks at its hardness, thicker needs (run / thick)^2 times), max_run, load_k,
    // impact_k, max_leaf_points. Hardness per material: "hardness", else "hp", else wood 4000, leaves 20,
    // plants 5; ground never breaks.
    void set_params(const Dictionary &p_params);

    // After cells were emptied (world voxels): finds what they cut loose and turns it into pieces.
    // push_from: world position the cut came from (the player); pieces topple away from it.
    // Returns the new piece ids.
    PackedInt32Array check(const Array &p_cells, Vector3 p_push_from);
    // What was cut loose too small for a piece (min_piece voxels; min_limb for limbs a piece snapped)
    // since the last check: 4 ints each (world x, y, z, material), already removed from the grid; the
    // caller drops them as items.
    PackedInt32Array get_crumbs() const;
    // Voxels pieces broke since the last call (world x, y, z, material: 4 ints each), for break effects.
    PackedInt32Array take_breaks();
    // Pieces that lost voxels (shredded leaves) since the last call: rebuild their meshes.
    PackedInt32Array take_dirty_meshes();
    // Every piece made since the last call, by check or by step (limbs a falling piece snapped).
    PackedInt32Array take_spawned();
    // One simulation tick of dt seconds (call while the SimWorld is idle: it crushes and settles).
    // Returns the ids that settled this tick (their voxels are back in the grid; free their meshes).
    PackedInt32Array step(float p_dt);

    PackedInt32Array get_ids() const;
    // World transform of the piece's mesh (mesh origin = the piece's local cell (0,0,0)), blended
    // between the last two ticks by alpha 0..1.
    Transform3D get_transform(int p_id, float p_alpha) const;
    // World voxel of local (0,0,0) when it broke off: the colour hash origin (terrain_piece.gdshader).
    Vector3 get_hash_origin(int p_id) const;
    // Mesh arrays (Mesh.ARRAY_MAX entries: vertex, normal, uv, color, index), local cell units.
    Array build_mesh(int p_id) const;
    Dictionary get_info(int p_id) const;
    Dictionary get_stats() const;

    // In the game the work runs on the SimWorld's driver thread, just before each sim tick, never on
    // the main thread. set_async(true) hooks it in; then queue_check / queue_step only queue, and
    // collect() (main thread, between ticks) hands over what the last jobs did: {spawned, settled,
    // meshes (id -> {block index -> mesh arrays}, built on the driver), breaks, crumbs}. get_transform, get_hash_origin
    // and get_stats then read the state as of that collect. check() and step() stay synchronous
    // (tests, the bench).
    void set_async(bool p_on);
    void queue_check(const Array &p_cells, Vector3 p_push_from);
    void queue_check_packed(const PackedInt32Array &p_cells, Vector3 p_push_from);   // x, y, z triples (a box dig)
    void queue_step(float p_dt);
    Dictionary collect();
    // Debug, as of collect(): {load (highest wood contact load over its snapping load, last tick), wood, ground (contacts)}.
    Dictionary get_contact_info(int p_id) const;
    // Bug report 2026-10-25 02-51-52: pieces in flight are cut like voxels. queue_carve empties the piece cells in a
    // sphere (world voxels) before the next step (a piece cut in two flies on as two); raycast finds the nearest
    // piece cell along a ray: {id, position, distance} or {}.
    void queue_carve(Vector3 p_center, float p_radius);
    Dictionary raycast(Vector3 p_from, Vector3 p_dir, float p_max) const;
    // ADR 0160: contraptions. Call between ticks (the driver thread idle), like collect().
    Dictionary lift(Vector3i p_seed, Vector3i p_lo, Vector3i p_hi, int p_max_cells, bool p_wood_only);
    bool drive(int p_id, const Transform3D &p_local_to_world);
    bool release(int p_id, bool p_settle);

    // ADR 0142 (mines need shoring), rigid_support.cpp: the support rule for roofs the player dug.
    // spans: per material id, < 0 open, 0 solid that holds but never fails, > 0 the farthest a roof cell of it
    // may be from a support (voxels). mark / mark_box record dug 0.5 m blocks (applied before the next scan).
    // scan: {lo, hi (the dug box, world voxels), reach (voxels), props (x, y_top, z, radius, capacity) x n,
    // probe?, tag} -> {clusters [{cells, count, ratio, center}], props [{load, capacity, failed, touches}], probe, ...}.
    // drop: the roof cells and the rock thick voxels above them fall as one piece (its id, 0 for crumbs).
    // queue_* run on the driver thread; collect() hands their results over as "support".
    void support_setup(const PackedFloat32Array &p_spans);
    void support_mark(const PackedInt32Array &p_cells);
    void support_mark_box(Vector3i p_lo, Vector3i p_hi);
    Dictionary support_scan(const Dictionary &p_job);
    int support_drop(const PackedInt32Array &p_roof, int p_thick, Vector3 p_from);
    void queue_support_scan(const Dictionary &p_job);
    void queue_support_drop(const PackedInt32Array &p_roof, int p_thick, Vector3 p_from);
    Dictionary support_stats() const;
    void support_clear();
    // the dug marks for the save, per 16-voxel chunk: (cx, cy, cz, mask lo, mask hi) x n, one bit per 0.5 m block
    PackedInt32Array support_dug_chunks();
    void support_load_chunks(const PackedInt32Array &p_chunks);

protected:
    static void _bind_methods();

private:
    struct LeafPoint { Vector3 r; int cell; };   // relative to com; index into grid
    struct View { int id; Vector3 prev_pos, pos, com, hash_origin; Quaternion prev_rot, rot; float load; int wood_touch, ground_touch, inside; bool hinged; float spin; };   // async: as of collect()
    struct Piece {
        int id = 0;
        Vector3i size;                    // local grid size
        std::vector<uint8_t> grid;        // material per local cell (0 = none), x + sx*(y + sy*z)
        Vector3 hash_origin;              // world voxel of local (0,0,0) at detach
        Vector3 com;                      // centre of mass, local units (cell centres at +0.5)
        float mass = 0.0f, inv_mass = 0.0f;
        Basis inv_inertia;                // local, about com
        Vector3 pos, prev_pos;            // world position of com
        Quaternion rot, prev_rot;
        Vector3 vel, ang;                 // world, voxels/s and rad/s
        std::vector<Vector3> points;      // contact points (wood surface), local, relative to com
        std::vector<LeafPoint> leaf_points;   // leaf surface: shreds on impact, never holds the piece up
        bool mesh_dirty = false;
        bool mesh_new = true;             // async: no mesh handed over yet
        int mesh_wait = 0;                // ticks since its mesh was last built (async rebuilds are spaced)
        Vector3i blocks;                  // mesh blocks of MESH_BLOCK^3 cells along each axis
        bool hinged = false;              // a felled trunk pivots on the edge of its cut (its hinge wood) until it tips hinge_deg
        Vector3 hinge_r, hinge_world;     // that pivot: local, relative to com; and where it stays in the world
        float load = 0.0f;                // debug, last tick: highest wood contact load / its snapping load
        int wood_touch = 0, ground_touch = 0, inside = 0;   // debug, last tick: most contacts on wood / ground / inside a cell in a substep
        std::vector<uint8_t> block_dirty; // async: blocks that lost voxels since their last mesh
        int still = 0;
        float age = 0.0f;
        int wood = 0, leaves = 0;
        bool driven = false;              // ADR 0160: a contraption part; script sets its pose, no physics
        Vector3 drive_pos;                // the pose for the next step: com in the world
        Quaternion drive_rot;
    };

    struct Contact {
        Vector3 r, n;
        float depth, jn, jt1, jt2, mass_n;
        Vector3 t1, t2;
        float mass_t1, mass_t2;
        Vector3i cell;      // the wood cell touched (other trees), for snapping under load
        float factor;       // that wood's wood_factor (0: not wood)
    };

    Ref<SimWorld> sim_;
    std::vector<uint8_t> cls_;                 // per material
    std::vector<std::array<int, 3>> tiles_;    // per material: top, side, bottom
    std::vector<uint8_t> transparent_;
    std::vector<float> hardness_;              // per material (setup)
    Vector2i atlas_tiles_ = Vector2i(8, 13);
    Vector3i origin_;
    std::vector<Piece> pieces_;
    int next_id_ = 1;

    int max_wood_ = 200000;
    int leaf_reach_ = 24;
    int substeps_ = 6;
    float friction_ = 0.7f;
    float restitution_ = 0.05f;
    float push_ = 0.35f;
    int sleep_ticks_ = 12;
    float max_age_ = 20.0f;
    int max_points_ = 3000;
    int min_piece_ = 8;
    int min_limb_ = 64;          // a snapped-off bit smaller than this drops as crumbs instead of flying
    float strength_ = 600.0f;   // voxels one path (one voxel of cross-section) holds up
    int region_cap_ = 96000;
    int max_paths_ = 160;
    int last_paths_ = 0, last_region_ = 0, last_side_ = 0;
    double last_strength_ms_ = 0.0;
    double split_ms_[4] = {};
    int hit_ground_ = 0, hit_wood_ = 0, snapped_total_ = 0, shed_ = 0, torn_ = 0, decayed_ = 0;
    double max_decay_ms_ = 0.0;
    float impact_k_ = 20.0f;              // impact_k * speed^2 (voxels/s) breaks a voxel of that hardness
    int max_leaf_points_ = 2000;
    PackedInt32Array breaks_;
    int thick_ = 6;                      // wood thinner than this (shortest run, voxels) breaks at its plain hardness
    int max_run_ = 48;                   // thicker wood counts as this thick (6 m: holds against ~14 m/s)
    int limb_run_ = 12;                  // a heavy piece breaks wood thinner than this (1.5 m) on any contact
    float heavy_mass_ = 1000.0f;         // ...when it weighs this much (wood voxels; a leaf is 0.12)
    float cur_mass_ = 0.0f;              // the piece collide() is moving
    float hinge_deg_ = 45.0f;            // a felled trunk leaves its hinge at this tilt (or after hinge_s)
    float hinge_s_ = 4.0f;
    float load_k_ = 40.0f;                // a limb snaps under a contact force of hardness * load_k (mass voxels * voxels/s^2)
    std::vector<Vector3i> snapped_;
    PackedInt32Array spawned_ids_;
    Vector3i hit_at_;
    PackedInt32Array crumbs_;
    std::vector<Vector3i> anchors_, fills_;   // debug: where the last check's fills touched ground
    int max_breaks_ = 800;       // world voxels pieces may break per tick (each is a terrain edit and a remesh)
    int breaks_tick_ = 0;
    int recheck_every_ = 3;      // snapped limbs are checked together every this many ticks
    int recheck_wait_ = 0;
    int mesh_every_ = 2;         // async: a piece that lost leaves gets its changed blocks at most every this many ticks

    // ADR 0142 support rule (rigid_support.cpp)
    struct ScanJob { Vector3i lo, hi, probe; int reach = 32, tag = 0; bool has_probe = false; std::vector<int> props, check; };
    struct DropJob { std::vector<Vector3i> roof; int thick = 4; Vector3 from; };
    std::vector<float> span_;                 // per material id
    std::unordered_set<int64_t> dug_;         // dug 0.5 m blocks (driver thread, or a sync call), under dug_m_
    mutable std::mutex dug_m_;                // so the save reads dug_ without waiting for the driver to idle
    std::vector<Vector3i> dug_pending_;       // under job_m_: blocks to add
    std::vector<ScanJob> scan_jobs_;          // under job_m_
    std::vector<DropJob> drop_jobs_;          // under job_m_
    Array support_out_;
    std::vector<int32_t> s_node_;             // scan box -> node index, valid where s_stamp_ == s_gen_
    std::vector<uint32_t> s_stamp_;
    uint32_t s_gen_ = 0;
    double support_last_ms_ = 0.0, support_max_ms_ = 0.0;
    int support_scans_ = 0, support_drops_ = 0, support_last_nodes_ = 0;
    void support_apply_marks();
    static ScanJob scan_job_of(const Dictionary &d);
    Dictionary support_scan_impl(const ScanJob &j);
    int support_drop_impl(const DropJob &j);
    void support_run_jobs();   // driver thread, from run_job

    // async (set_async)
    struct PendingCheck { std::vector<Vector3i> cells; Vector3 from; };
    bool async_ = false;
    std::mutex job_m_;
    std::vector<PendingCheck> pending_;
    std::vector<Vector4> carves_;   // queued carves: centre xyz, radius
    int carved_ = 0;
    void carve_now(const Vector3 &c, float r);
    bool finish_body(Piece &p);
    float pending_dt_ = 0.0f;
    PackedInt32Array settled_out_;
    Dictionary meshes_out_;
    std::vector<View> view_;
    Dictionary stats_view_;
    double last_job_ms_ = 0.0, max_job_ms_ = 0.0;
    std::vector<Vector3i> made_;   // debug: (id, wood, leaves) of the first pieces made
    Array rest_;                   // debug: what the first pieces came to rest on
    std::vector<Vector4i> contact_log_;   // debug: the first contacts made

    // stats
    double last_check_ms_ = 0.0, max_check_ms_ = 0.0;
    double last_step_ms_ = 0.0, max_step_ms_ = 0.0;
    double last_settle_ms_ = 0.0, max_settle_ms_ = 0.0;
    int last_filled_ = 0, last_written_ = 0, last_lost_ = 0, crushed_ = 0, spawned_ = 0, settled_ = 0;
    int last_contacts_ = 0;

    // cell access in world voxels
    inline bool local_of(int wx, int wy, int wz, int &x, int &y, int &z) const;
    uint8_t mat_at(int wx, int wy, int wz, bool &open) const;   // open = loaded and inside the window
    uint8_t cls_at(int wx, int wy, int wz) const;               // GROUND outside the window / unloaded
    void set_at(int wx, int wy, int wz, uint8_t m);

    PackedInt32Array check_cells(const std::vector<Vector3i> &cells, Vector3 p_push_from, bool strength);
    void step_impl(float dt, PackedInt32Array &done);
    void run_job();   // driver thread, before a sim tick
    // block < 0: the whole piece; else one MESH_BLOCK^3 block (index into Piece::blocks)
    Array mesh_arrays(const Piece &p, int block = -1) const;
    void mark_block(Piece &p, int cell);
    Dictionary live_stats() const;
    const View *find_view(int id) const;
    bool strength_split(const std::vector<int64_t> &S, std::vector<int64_t> &piece);
    void decay_orphan_leaves(const std::vector<Vector3i> &cells);
    // hinged: a cut (not a snapped limb) pivots on the edge of its base that faces the fall
    void make_piece(const std::vector<int64_t> &wood, const std::vector<int64_t> &leaves, Vector3 push_from, bool hinged);
    void hold_hinge(Piece &p, float dt);
    Vector3 smooth_normal(int cx, int cy, int cz, const Vector3 &face) const;
    bool breaks(int x, int y, int z, float speed);
    int wood_run(int x, int y, int z, int cap) const;
    float wood_factor(int x, int y, int z) const;
    void note_break(int x, int y, int z, uint8_t m);
    void collide(Piece &p, float dt, std::vector<Contact> &cs);
    void solve(Piece &p, std::vector<Contact> &cs, float dt);
    bool settle(Piece &p);
    const Piece *find(int id) const;
};

} // namespace godot
