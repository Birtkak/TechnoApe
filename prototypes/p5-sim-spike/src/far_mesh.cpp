#include "far_mesh.h"

#include <godot_cpp/classes/time.hpp>
#include <godot_cpp/core/class_db.hpp>
#include <godot_cpp/variant/packed_byte_array.hpp>
#include <godot_cpp/variant/packed_float32_array.hpp>
#include <godot_cpp/variant/packed_int32_array.hpp>
#include <godot_cpp/variant/packed_vector2_array.hpp>
#include <godot_cpp/variant/packed_vector3_array.hpp>

#include <algorithm>
#include <cmath>
#include <vector>

namespace godot {

namespace {

constexpr int FS = FineGen::FS;
// ground_f's amplitudes (fine_gen.cpp): swells, hummocks, roughness
constexpr float GROUND_AMP = 3.5f;
constexpr float HUMP_AMP = 1.5f;
constexpr float GROUND_AMP2 = 0.6f;
constexpr float CANOPY_M = 11.0f;   // a baked forest canopy stands about this tall
constexpr int SLOPE_M = 4;          // macro columns either side over which the slope is taken (2 m)

inline int floor_div(int a, int b) { return a >= 0 ? a / b : -((-a + b - 1) / b); }
inline float smoothstep(float a, float b, float x) {
    const float t = std::clamp((x - a) / (b - a), 0.0f, 1.0f);
    return t * t * (3.0f - 2.0f * t);
}
inline float bilerp(float a00, float a10, float a01, float a11, float tx, float tz) {
    return (a00 * (1.0f - tx) + a10 * tx) * (1.0f - tz) + (a01 * (1.0f - tx) + a11 * tx) * tz;
}
inline uint32_t h3(int a, int b, int c) { return WorldGen::hash3(a, b, c); }
inline float unit(uint32_t h) { return (float)(h & 1023) / 1023.0f; }

} // namespace

void FarMesh::_bind_methods() {
    ClassDB::bind_method(D_METHOD("configure", "gen"), &FarMesh::configure);
    ClassDB::bind_method(D_METHOD("set_material_colors", "colors"), &FarMesh::set_material_colors);
    ClassDB::bind_method(D_METHOD("get_material_colors"), &FarMesh::get_material_colors);
    ClassDB::bind_method(D_METHOD("tile_voxels", "level"), &FarMesh::tile_voxels);
    ClassDB::bind_method(D_METHOD("build_tile", "level", "tx", "tz", "overrides"), &FarMesh::build_tile, DEFVAL(Dictionary()));
    ClassDB::bind_method(D_METHOD("column_tops", "chunk", "y0", "count", "bytes"), &FarMesh::column_tops);
    ClassDB::bind_method(D_METHOD("build_nets_tile", "level", "tx", "tz"), &FarMesh::build_nets_tile);
    ClassDB::bind_integer_constant(get_class_static(), StringName(), "CELLS", CELLS);
    ClassDB::bind_integer_constant(get_class_static(), StringName(), "BASE_STEP", BASE_STEP);
    ClassDB::bind_integer_constant(get_class_static(), StringName(), "MAX_LEVEL", MAX_LEVEL);
    ClassDB::bind_integer_constant(get_class_static(), StringName(), "TREE_LEVEL", TREE_LEVEL);
    ClassDB::bind_integer_constant(get_class_static(), StringName(), "TREE_KINDS", TREE_KINDS);
}

void FarMesh::configure(Ref<FineGen> gen) { gen_ = gen; }

void FarMesh::set_material_colors(PackedColorArray colors) { colors_ = colors; }

Color FarMesh::color_of(uint16_t mat) const {
    if ((int)mat < colors_.size()) return colors_[mat];
    return Color(0.5f, 0.5f, 0.5f);
}

// ground_f (fine_gen.cpp) with the detail the level can show: level 0-1 keep the ledges and hummocks
// (25-50 cm cells), level 2 half of them, from level 3 only the 8 m swells, from level 5 the macro
// surface alone. A coarser level's parent heights are what its own tiles draw, so this is what a
// finer tile morphs into.
float FarMesh::ground(int level, float base, int x, int z, float w) const {
    if (w <= 0.0f) return base;
    const FineGen &f = *gen_.ptr();
    const float fx = (float)x, fz = (float)z;
    const float swell = level >= 5 ? 0.0f : (level == 4 ? 0.5f : 1.0f);
    const float hf = base + f.n_ground_.GetNoise(fx, fz) * GROUND_AMP * w * swell;
    const float d = level <= 1 ? 1.0f : (level == 2 ? 0.5f : 0.0f);
    if (d <= 0.0f) return hf;
    const float tn = f.n_terrace_.GetNoise(fx, fz);
    const float T = tn < -0.3f ? 3.0f : (tn < 0.05f ? 4.0f : (tn < 0.35f ? 5.0f : (tn < 0.6f ? 6.0f : 8.0f)));
    const float q = std::floor(hf / T + 0.5f) * T;
    const float small = f.n_hump_.GetNoise(fx, fz) * HUMP_AMP + f.n_ground2_.GetNoise(fx, fz) * GROUND_AMP2;
    return hf + (q - hf) * 0.85f * w * d + small * w * (1.0f - 0.6f * w) * d;
}

// The top material of a column, as FineGen::generate_block picks it (its lakebed / river / rock /
// snow / steep / scree / alpine / marsh / biome chain, without the per-voxel soil details).
uint16_t FarMesh::top_material(const FineGen::MacroPt &m, float slope, int h, uint32_t ph) const {
    using Mat = WorldGen::Mat;
    const WorldGen::Column &col = m.col;
    const WorldGen::Hydro &hy = m.hy;
    const int water_level = m.water_level * FS;
    const bool beach_sand = col.biome == WorldGen::BIOME_BEACH || (col.biome == WorldGen::BIOME_OCEAN && col.c > -0.4f);
    if (m.lakebed || m.riverbed || m.marshbed || h < water_level) {
        if (h < water_level && beach_sand) return Mat::SAND_STATIC;
        const uint32_t bh = ph & 7;
        return bh < 3 ? Mat::CLAY : (bh < 5 ? Mat::SAND_STATIC : Mat::GRAVEL_STATIC);
    }
    if (hy.river > 0.08f && h <= water_level + 2 * FS) return Mat::SAND_STATIC;
    if (col.rock) return col.rock;
    const bool steep = slope > 1.1f, scree = slope > 0.75f && slope <= 1.1f && col.alpine;
    if (col.snow && !steep) return Mat::SNOW;
    if (steep) return technosim_biome::SURFACE_UNDER[col.biome] == Mat::TERRACOTTA ? Mat::TERRACOTTA : Mat::STONE;
    if (scree) return Mat::GRAVEL_STATIC;
    if (col.alpine) {
        const uint32_t ah = ph & 7;
        return ah < 3 ? Mat::GRASS : (ah < 5 ? Mat::GRAVEL_STATIC : Mat::STONE);
    }
    if (hy.marsh) {
        const uint32_t mh = ph & 7;
        return mh < 2 ? Mat::CLAY : (mh < 5 ? Mat::DIRT : Mat::GRASS);
    }
    uint16_t top = (uint16_t)technosim_biome::SURFACE_TOP[col.biome];
    if (col.biome == WorldGen::BIOME_OCEAN && !beach_sand) top = Mat::GRAVEL_STATIC;
    if (technosim_biome::SURFACE_STEEP[col.biome] >= 0 && slope > 0.6f) top = (uint16_t)technosim_biome::SURFACE_STEEP[col.biome];
    if (m.spike > 0) top = (uint16_t)technosim_biome::SPIRE_MATERIAL[col.biome];
    return top;
}

// How much higher than the ground a baked canopy stands here (fine voxels): nothing below
// TREE_LEVEL + 1 (trees are instances there), a leaf mass that rises with the forest's density after.
float FarMesh::canopy(int level, const Node &n, int x, int z) const {
    if (level <= TREE_LEVEL || !n.canopy_ok || n.density < 0.08f) return 0.0f;
    const float cover = smoothstep(0.08f, 0.45f, n.density);
    const float lump = 0.75f + 0.5f * unit(h3(x >> 5, z >> 5, 913));   // 4 m lumps so a forest is not a slab
    return CANOPY_M / 0.125f * cover * lump;
}

Dictionary FarMesh::build_tile(int level, int tx, int tz, const Dictionary &overrides) const {
    Dictionary out;
    if (gen_.is_null() || level < 0 || level > MAX_LEVEL) return out;
    const uint64_t t0 = Time::get_singleton()->get_ticks_usec();
    const FineGen &f = *gen_.ptr();
    const WorldGen &g = *f.g_.ptr();
    using Mat = WorldGen::Mat;

    const int step = BASE_STEP << level;
    const int T = CELLS * step;
    const int x0 = tx * T, z0 = tz * T;
    const int N = CELLS + 3;   // samples: the tile's CELLS + 1 corners and a one-sample halo for normals

    // The macro lattice under the samples: stride ms macro columns (one macro column at level 0 and 1,
    // every second at level 2, ...), two nodes of margin for slopes and the halo.
    const int ms = std::max(1, step / FS);
    const int margin = std::max(2 * ms, SLOPE_M);   // nodes past the samples: the halo and the slope's reach
    const int mxa = floor_div(x0 - step, FS * ms) * ms - margin;
    const int mza = floor_div(z0 - step, FS * ms) * ms - margin;
    const int mxb = floor_div(x0 + T + step, FS * ms) * ms + margin;
    const int mzb = floor_div(z0 + T + step, FS * ms) * ms + margin;
    const int nx = (mxb - mxa) / ms + 1, nz = (mzb - mza) / ms + 1;
    WorldGen::Ruin ruins[16];
    const int nr = g.ruins_near(mxa, mza, mxb, mzb, ruins, 16);
    std::vector<Node> nodes((size_t)nx * nz);
    std::vector<FineGen::MacroPt> pts((size_t)nx * nz);
    for (int j = 0; j < nz; j++)
        for (int i = 0; i < nx; i++)
            pts[(size_t)j * nx + i] = f.macro_point(mxa + i * ms, mza + j * ms, ruins, nr);
    for (int j = 0; j < nz; j++)
        for (int i = 0; i < nx; i++) {
            const FineGen::MacroPt &m = pts[(size_t)j * nx + i];
            Node &n = nodes[(size_t)j * nx + i];
            n.hc = m.hc;
            n.gw = m.gw;
            n.water = m.water_level * FS;
            n.biome = m.col.biome;
            n.density = technosim_biome::TREE_DENSITY[m.col.biome];
            const int h = (int)std::floor(m.hc * FS + 0.5f);
            const int mx = mxa + i * ms, mz = mza + j * ms;
            // the slope over 2 x SLOPE_M macro columns of the base terrain, the same at every level (a tile's
            // parent values must be what the parent tile draws, so nothing here may depend on the lattice
            // stride): from the lattice where its stride divides SLOPE_M, from the base columns beyond
            float hl, hr, hd, hu;
            if (SLOPE_M % ms == 0) {
                const int k = SLOPE_M / ms;
                hl = (float)pts[(size_t)j * nx + std::max(i - k, 0)].col.height;
                hr = (float)pts[(size_t)j * nx + std::min(i + k, nx - 1)].col.height;
                hd = (float)pts[(size_t)std::max(j - k, 0) * nx + i].col.height;
                hu = (float)pts[(size_t)std::min(j + k, nz - 1) * nx + i].col.height;
            } else {
                hl = (float)g.column_base(mx - SLOPE_M, mz, false).height;
                hr = (float)g.column_base(mx + SLOPE_M, mz, false).height;
                hd = (float)g.column_base(mx, mz - SLOPE_M, false).height;
                hu = (float)g.column_base(mx, mz + SLOPE_M, false).height;
            }
            n.slope = std::max({std::fabs(hr - hl) / (float)(2 * SLOPE_M), std::fabs(hu - hd) / (float)(2 * SLOPE_M), m.col.slope * 0.75f});
            n.top = top_material(m, n.slope, h, h3(mx * FS >> 1, mz * FS >> 1, g.seed_ + 19));
            const bool dry = h > n.water + 2 && !m.lakebed && !m.riverbed && !m.marshbed && m.hy.river <= 0.05f;
            n.canopy_ok = dry && !m.col.snow && !m.col.alpine && n.slope < 0.9f && m.ruin < 0 && m.spike == 0;
            n.tree_ok = dry && m.ruin < 0;
        }

    // Samples: heights at this level and at the parent level (what the parent's tiles draw at the even
    // positions), top colours at both, water, canopy.
    const bool has_overrides = !overrides.is_empty();
    std::vector<float> h((size_t)N * N), hp((size_t)N * N);
    std::vector<Color> c((size_t)N * N), cp((size_t)N * N);
    std::vector<int> water((size_t)N * N);
    auto node_at = [&](int x, int z, int &a, int &b, float &ta, float &tb) {
        const float u = (float)x / FS, v = (float)z / FS;
        const float fa = (u - (float)mxa) / (float)ms, fb = (v - (float)mza) / (float)ms;
        a = std::clamp((int)std::floor(fa), 0, nx - 2);
        b = std::clamp((int)std::floor(fb), 0, nz - 2);
        ta = std::clamp(fa - (float)a, 0.0f, 1.0f);
        tb = std::clamp(fb - (float)b, 0.0f, 1.0f);
    };
    for (int j = 0; j < N; j++)
        for (int i = 0; i < N; i++) {
            const int x = x0 + (i - 1) * step, z = z0 + (j - 1) * step;
            int a, b;
            float ta, tb;
            node_at(x, z, a, b, ta, tb);
            const Node &n00 = nodes[(size_t)b * nx + a], &n10 = nodes[(size_t)b * nx + a + 1];
            const Node &n01 = nodes[(size_t)(b + 1) * nx + a], &n11 = nodes[(size_t)(b + 1) * nx + a + 1];
            const Node &nn = (ta < 0.5f ? (tb < 0.5f ? n00 : n01) : (tb < 0.5f ? n10 : n11));   // nearest node
            const float base = bilerp(n00.hc, n10.hc, n01.hc, n11.hc, ta, tb) * FS;
            const float w = bilerp(n00.gw, n10.gw, n01.gw, n11.gw, ta, tb);
            const size_t k = (size_t)j * N + i;
            float g0 = ground(level, base, x, z, w), g1 = ground(level + 1, base, x, z, w);
            if (has_overrides) {
                // an edited column: the real surface replaces the generated one at every level
                const Vector2i ck(floor_div(x, 16), floor_div(z, 16));
                if (overrides.has(ck)) {
                    const PackedInt32Array arr = overrides[ck];
                    const int ov = arr.size() == 256 ? arr[((z & 15) << 4) | (x & 15)] : -1;
                    if (ov >= 0) g0 = g1 = (float)ov;
                }
            }
            const float k0 = canopy(level, nn, x, z), k1 = canopy(level + 1, nn, x, z);
            h[k] = g0 + k0;
            hp[k] = g1 + k1;
            water[k] = nn.water;
            const Color leaf = color_of(WorldGen::tree_leaves(technosim_biome::TREE_KIND[nn.biome]));
            const Color top = color_of(nn.top);
            c[k] = k0 > 0.0f ? top.lerp(leaf, std::min(1.0f, smoothstep(0.08f, 0.45f, nn.density) * 1.5f)) : top;
            cp[k] = k1 > 0.0f ? top.lerp(leaf, std::min(1.0f, smoothstep(0.08f, 0.45f, nn.density) * 1.5f)) : top;
        }
    // Parent values at odd positions: the parent's triangles interpolate between its own (even) vertices
    // along rows, columns and the quad diagonal the tile is split along ((i, j) -> (i + 1, j + 1)).
    for (int j = 1; j < N - 1; j++)
        for (int i = 1; i < N - 1; i++) {
            const int ci = i - 1, cj = j - 1;
            if ((ci & 1) == 0 && (cj & 1) == 0) continue;
            const size_t k = (size_t)j * N + i;
            size_t k1, k2;
            if ((ci & 1) == 1 && (cj & 1) == 1) { k1 = (size_t)(j - 1) * N + i - 1; k2 = (size_t)(j + 1) * N + i + 1; }
            else if ((ci & 1) == 1) { k1 = k - 1; k2 = k + 1; }
            else { k1 = k - N; k2 = k + N; }
            hp[k] = 0.5f * (hp[k1] + hp[k2]);
            cp[k] = cp[k1].lerp(cp[k2], 0.5f);
        }

    // Ground surface: (CELLS + 1)^2 shared vertices, normals from the halo.
    const int V = CELLS + 1;
    PackedVector3Array verts, norms;
    PackedColorArray cols;
    PackedVector2Array uvs, uvs2;
    PackedByteArray custom;
    PackedInt32Array idx;
    verts.resize(V * V);
    norms.resize(V * V);
    cols.resize(V * V);
    uvs.resize(V * V);
    uvs2.resize(V * V);
    custom.resize(V * V * 4);
    idx.resize(CELLS * CELLS * 6);
    {
        Vector3 *vp = verts.ptrw();
        Vector3 *np = norms.ptrw();
        Color *cl = cols.ptrw();
        Vector2 *uv = uvs.ptrw();
        Vector2 *uv2 = uvs2.ptrw();
        uint8_t *cu = custom.ptrw();
        for (int cj = 0; cj < V; cj++)
            for (int ci = 0; ci < V; ci++) {
                const int i = ci + 1, j = cj + 1;
                const size_t k = (size_t)j * N + i, o = (size_t)cj * V + ci;
                const float x = (float)(x0 + ci * step), z = (float)(z0 + cj * step);
                vp[o] = Vector3(x, h[k], z);
                const float dx = (h[k + 1] - h[k - 1]) / (float)(2 * step), dz = (h[k + N] - h[k - N]) / (float)(2 * step);
                np[o] = Vector3(-dx, 1.0f, -dz).normalized();
                cl[o] = c[k];
                uv[o] = Vector2(hp[k], 0.0f);
                const float px = (hp[k + 1] - hp[k - 1]) / (float)(2 * step), pz = (hp[k + N] - hp[k - N]) / (float)(2 * step);
                const Vector3 pn = Vector3(-px, 1.0f, -pz).normalized();
                uv2[o] = Vector2(pn.x, pn.z);
                cu[o * 4 + 0] = (uint8_t)std::clamp((int)std::lround(cp[k].r * 255.0f), 0, 255);
                cu[o * 4 + 1] = (uint8_t)std::clamp((int)std::lround(cp[k].g * 255.0f), 0, 255);
                cu[o * 4 + 2] = (uint8_t)std::clamp((int)std::lround(cp[k].b * 255.0f), 0, 255);
                cu[o * 4 + 3] = 255;
            }
        int32_t *ip = idx.ptrw();
        size_t q = 0;
        for (int cj = 0; cj < CELLS; cj++)
            for (int ci = 0; ci < CELLS; ci++) {
                const int32_t a = cj * V + ci, b = a + 1, cc = a + V, d = cc + 1;
                // Godot front faces wind clockwise seen from outside (+y): a, d, cc / a, b, d, split along a-d
                ip[q++] = a; ip[q++] = d; ip[q++] = cc;
                ip[q++] = a; ip[q++] = b; ip[q++] = d;
            }
    }
    out["vertices"] = verts;
    out["normals"] = norms;
    out["colors"] = cols;
    out["uvs"] = uvs;
    out["uvs2"] = uvs2;
    out["custom0"] = custom;
    out["indices"] = idx;

    // Water: a flat sheet per cell with a wet corner, at the highest wet corner's level, unshared
    // vertices (lakes and the sea are flat, a river's sheet steps down cell by cell).
    {
        PackedVector3Array wv, wn;
        PackedColorArray wc;
        PackedInt32Array wi;
        const Color bed_default = color_of(Mat::GRAVEL_STATIC);
        for (int cj = 0; cj < CELLS; cj++)
            for (int ci = 0; ci < CELLS; ci++) {
                const int i = ci + 1, j = cj + 1;
                const size_t ks[4] = {(size_t)j * N + i, (size_t)j * N + i + 1, (size_t)(j + 1) * N + i, (size_t)(j + 1) * N + i + 1};
                int level_y = -1;
                for (size_t k : ks)
                    if (water[k] > h[k] + 0.25f) level_y = std::max(level_y, water[k]);
                if (level_y < 0) continue;
                const int32_t base = wv.size();
                for (int q = 0; q < 4; q++) {
                    const size_t k = ks[q];
                    const float x = (float)(x0 + (ci + (q & 1)) * step), z = (float)(z0 + (cj + (q >> 1)) * step);
                    wv.push_back(Vector3(x, (float)level_y, z));
                    wn.push_back(Vector3(0.0f, 1.0f, 0.0f));
                    const float depth_m = std::max(0.0f, ((float)level_y - h[k]) * 0.125f);
                    Color bc = c[k];
                    if (water[k] <= h[k]) bc = bed_default;
                    wc.push_back(Color(bc.r, bc.g, bc.b, std::min(1.0f, depth_m / 8.0f)));
                }
                wi.push_back(base); wi.push_back(base + 3); wi.push_back(base + 2);
                wi.push_back(base); wi.push_back(base + 1); wi.push_back(base + 3);
            }
        if (wv.size() > 0) {
            out["water_vertices"] = wv;
            out["water_normals"] = wn;
            out["water_colors"] = wc;
            out["water_indices"] = wi;
        }
    }

    // Trees as instances (level <= TREE_LEVEL): WorldGen's tree cells inside the tile, each with the
    // crown FineGen gives it at its age.
    if (level <= TREE_LEVEL) {
        const int mx_lo = floor_div(x0, FS), mz_lo = floor_div(z0, FS);
        const int mx_hi = floor_div(x0 + T - 1, FS), mz_hi = floor_div(z0 + T - 1, FS);
        const int cells = (T / FS) / WorldGen::TREE_CELL + 3;
        std::vector<WorldGen::Tree> trees((size_t)cells * cells);
        const int nt = g.trees_near(mx_lo, mz_lo, mx_hi, mz_hi, trees.data(), (int)trees.size());
        PackedFloat32Array tf, trunks;
        PackedFloat32Array crowns[TREE_KINDS];   // one buffer per kind: each kind has its own baked crown mesh
        // MultiMesh buffers (TRANSFORM_3D + colours): 12 floats of the transform's rows (scaled, turned by yaw
        // about y), then rgba
        auto push_instance = [](PackedFloat32Array &buf, const Vector3 &scale, const Vector3 &pos, const Color &c, float yaw) {
            const float cy = std::cos(yaw), sy = std::sin(yaw);
            const float row[12] = {scale.x * cy, 0.0f, scale.z * sy, pos.x, 0.0f, scale.y, 0.0f, pos.y, -scale.x * sy, 0.0f, scale.z * cy, pos.z};
            for (float v : row) buf.push_back(v);
            buf.push_back(c.r); buf.push_back(c.g); buf.push_back(c.b); buf.push_back(c.a);
        };
        for (int t = 0; t < nt; t++) {
            const WorldGen::Tree &tr = trees[t];
            const int fx = tr.x * FS + 2, fz = tr.z * FS + 2;
            if (fx < x0 || fx >= x0 + T || fz < z0 || fz >= z0 + T) continue;
            int a, b;
            float ta, tb;
            node_at(fx, fz, a, b, ta, tb);
            const Node &nn = nodes[(size_t)(tb < 0.5f ? b : b + 1) * nx + (ta < 0.5f ? a : a + 1)];
            if (!nn.tree_ok) continue;
            const float ground_y = f.surface_f(fx, fz);
            if (ground_y < (float)nn.water + 1.0f) continue;
            const uint32_t ts = h3(tr.x, tr.z, g.seed_ + 77);
            const float age = f.tree_age(tr, ts);
            const float H = (float)(tr.trunk_h * FS) * (0.55f + 0.65f * age);
            const float rc = tr.kind == WorldGen::TREE_SPRUCE ? (float)(tr.radius * FS) * (0.55f + 0.75f * age) + 2.0f
                                                              : 10.0f + 30.0f * age + 6.0f * (float)(tr.radius - 2);
            tf.push_back((float)fx + 0.5f);
            tf.push_back(ground_y);
            tf.push_back((float)fz + 0.5f);
            tf.push_back(rc);
            tf.push_back(H);
            tf.push_back((float)tr.kind);
            tf.push_back(age);
            // the crown as an ellipsoid (a unit sphere mesh scaled), spruces taller and narrower; the trunk a
            // box from the ground to the crown's middle (level <= 1 only, it is a pixel further out)
            const bool spruce = tr.kind == WorldGen::TREE_SPRUCE;
            const Vector3 pos((float)fx + 0.5f, ground_y, (float)fz + 0.5f);
            Color lc = color_of(WorldGen::tree_leaves(tr.kind));
            const float var = 0.92f + 0.16f * unit(ts >> 3);   // a little per-tree variation
            lc = Color(lc.r * var, lc.g * var, lc.b * var, 1.0f);
            const float rw = rc * (spruce ? 0.8f : 1.0f);
            const int kind = std::clamp((int)tr.kind, 0, TREE_KINDS - 1);
            const float yaw = unit(ts >> 13) * 6.2832f;
            push_instance(crowns[kind], Vector3(rw, H * (spruce ? 0.5f : 0.4f), rw), pos + Vector3(0.0f, H * (spruce ? 0.5f : 0.6f), 0.0f), lc, yaw);
            if (level <= 1) {
                const float tw = std::max(2.0f, rc * 0.12f), th = H * (spruce ? 0.5f : 0.3f);
                push_instance(trunks, Vector3(tw, th, tw), pos + Vector3(0.0f, th * 0.5f, 0.0f), color_of(WorldGen::tree_bark(tr.kind)), 0.0f);
            }
        }
        out["trees"] = tf;
        Array crown_arr;
        for (int k = 0; k < TREE_KINDS; k++) crown_arr.push_back(crowns[k]);
        out["crowns"] = crown_arr;
        out["trunks"] = trunks;
    }
    out["ms"] = (double)(Time::get_singleton()->get_ticks_usec() - t0) / 1000.0;
    return out;
}


// ---------- candidate (a): surface nets over the coarse voxel data ----------

namespace {
inline bool nets_liquid(uint16_t m) {
    return m == WorldGen::WATER || m == WorldGen::WATER_STATIC || m == WorldGen::LAVA || m == WorldGen::LAVA_STATIC || m == WorldGen::OIL;
}
inline bool nets_solid(uint16_t m) { return m != 0 && !nets_liquid(m) && m != WorldGen::STEAM && m != WorldGen::SMOKE && m != WorldGen::FIRE; }
} // namespace

Dictionary FarMesh::build_nets_tile(int level, int tx, int tz) const {
    Dictionary out;
    if (gen_.is_null() || level < 0 || level > MAX_LEVEL) return out;
    const uint64_t t0 = Time::get_singleton()->get_ticks_usec();
    const FineGen &f = *gen_.ptr();
    const WorldGen &g = *f.g_.ptr();
    const int lod = level + 1;   // BASE_STEP 2 voxels per cell at level 0
    const int step = 1 << lod;
    const int T = CELLS * step;
    const int x0 = tx * T, z0 = tz * T;

    // Height range from the macro columns under the tile (a margin for ledges, trees and water).
    const int ms = std::max(1, step / FS);
    float hmin = 1e9f, hmax = -1e9f;
    int wmax = 0;
    for (int mz = floor_div(z0, FS) - ms; mz <= floor_div(z0 + T, FS) + ms; mz += ms)
        for (int mx = floor_div(x0, FS) - ms; mx <= floor_div(x0 + T, FS) + ms; mx += ms) {
            const FineGen::MacroPt m = f.macro_point(mx, mz, nullptr, 0);
            hmin = std::min(hmin, m.hc);
            hmax = std::max(hmax, m.hc);
            wmax = std::max(wmax, m.water_level);
        }
    int y_lo = (int)std::floor(std::min(hmin * FS, (float)wmax * FS)) - 4 * step;
    int y_hi = (int)std::ceil(std::max(hmax * FS, (float)wmax * FS)) + (lod <= 3 ? 176 : 16) + 2 * step;
    y_lo = std::max(0, floor_div(y_lo, step) * step);
    y_hi = std::min(g.world_height_ * FS, floor_div(y_hi, step) * step + step);
    const int ny = (y_hi - y_lo) / step + 2;   // one cell of margin each side of the tile
    const int nxz = CELLS + 3;
    const Vector3i origin(x0 - step, y_lo - step, z0 - step);
    const PackedByteArray bytes = f.generate_block(origin, Vector3i(nxz, ny, nxz), lod, 2);
    std::vector<uint16_t> vox((size_t)nxz * ny * nxz, 0);
    if (bytes.size() == 2) {
        const uint16_t one = (uint16_t)(bytes[0] | (bytes[1] << 8));
        std::fill(vox.begin(), vox.end(), one);
    } else if (bytes.size() == (int64_t)vox.size() * 2) {
        const uint8_t *b = bytes.ptr();
        for (size_t i = 0; i < vox.size(); i++) vox[i] = (uint16_t)(b[2 * i] | (b[2 * i + 1] << 8));
    }
    auto at = [&](int x, int y, int z) -> uint16_t { return vox[(size_t)y + (size_t)ny * ((size_t)x + (size_t)nxz * z)]; };

    // Naive surface nets on a binary field: one vertex per cell whose 8 corners differ, at the mean of its
    // crossing edges' midpoints; one quad per crossing edge between the four cells around it.
    auto build = [&](bool liquid, PackedVector3Array &verts, PackedVector3Array &norms, PackedColorArray &cols, PackedInt32Array &idx) {
        auto in = [&](int x, int y, int z) -> bool {
            const uint16_t m = at(x, y, z);
            return liquid ? nets_liquid(m) : nets_solid(m);
        };
        const int cx = nxz - 1, cy = ny - 1, cz = nxz - 1;   // cells
        std::vector<int32_t> cell_vertex((size_t)cx * cy * cz, -1);
        std::vector<Vector3> acc_n;
        static const int E[12][2] = {{0, 1}, {2, 3}, {4, 5}, {6, 7}, {0, 2}, {1, 3}, {4, 6}, {5, 7}, {0, 4}, {1, 5}, {2, 6}, {3, 7}};
        for (int z = 0; z < cz; z++)
            for (int x = 0; x < cx; x++)
                for (int y = 0; y < cy; y++) {
                    int inside = 0;
                    bool c[8];
                    for (int k = 0; k < 8; k++) {
                        c[k] = in(x + (k & 1), y + ((k >> 1) & 1), z + (k >> 2));
                        inside += c[k];
                    }
                    if (inside == 0 || inside == 8) continue;
                    Vector3 sum;
                    int n = 0;
                    for (int e = 0; e < 12; e++) {
                        const int a = E[e][0], b = E[e][1];
                        if (c[a] == c[b]) continue;
                        const Vector3 pa((float)(a & 1), (float)((a >> 1) & 1), (float)(a >> 2)), pb((float)(b & 1), (float)((b >> 1) & 1), (float)(b >> 2));
                        sum += (pa + pb) * 0.5f;
                        n++;
                    }
                    Color col(0, 0, 0, 0);
                    int ncol = 0;
                    for (int k = 0; k < 8; k++) {
                        if (!c[k]) continue;
                        const uint16_t m = at(x + (k & 1), y + ((k >> 1) & 1), z + (k >> 2));
                        if (liquid && nets_solid(m)) continue;
                        col += color_of(m);
                        ncol++;
                    }
                    if (ncol == 0) { col = color_of(WorldGen::WATER); ncol = 1; }
                    col = col / (float)ncol;
                    col.a = 1.0f;
                    const Vector3 p = Vector3((float)origin.x, (float)origin.y, (float)origin.z) + (Vector3((float)x, (float)y, (float)z) + sum / (float)n) * (float)step;
                    cell_vertex[(size_t)y + (size_t)cy * ((size_t)x + (size_t)cx * z)] = verts.size();
                    verts.push_back(p);
                    cols.push_back(col);
                    acc_n.push_back(Vector3());
                }
        auto cv = [&](int x, int y, int z) -> int32_t {
            if (x < 0 || y < 0 || z < 0 || x >= cx || y >= cy || z >= cz) return -1;
            return cell_vertex[(size_t)y + (size_t)cy * ((size_t)x + (size_t)cx * z)];
        };
        // one quad per crossing edge; the four cells around an edge along axis a are the cells whose index
        // differs by 0 / -1 in the other two axes. Only edges that start inside the tile's own columns, so
        // neighbouring tiles share a seam instead of doubling it.
        for (int z = 1; z < nxz - 1; z++)
            for (int x = 1; x < nxz - 1; x++)
                for (int y = 1; y < ny - 1; y++) {
                    const bool here = in(x, y, z);
                    for (int a = 0; a < 3; a++) {
                        const int nx2 = x + (a == 0 ? 1 : 0), ny2 = y + (a == 1 ? 1 : 0), nz2 = z + (a == 2 ? 1 : 0);
                        if (nx2 >= nxz || ny2 >= ny || nz2 >= nxz) continue;
                        const bool there = in(nx2, ny2, nz2);
                        if (here == there) continue;
                        const int ex = origin.x + x * step, ez = origin.z + z * step;
                        if (ex < x0 || ex >= x0 + T || ez < z0 || ez >= z0 + T) continue;
                        int32_t q[4];
                        if (a == 0) { q[0] = cv(x, y - 1, z - 1); q[1] = cv(x, y, z - 1); q[2] = cv(x, y, z); q[3] = cv(x, y - 1, z); }
                        else if (a == 1) { q[0] = cv(x - 1, y, z - 1); q[1] = cv(x - 1, y, z); q[2] = cv(x, y, z); q[3] = cv(x, y, z - 1); }
                        else { q[0] = cv(x - 1, y - 1, z); q[1] = cv(x, y - 1, z); q[2] = cv(x, y, z); q[3] = cv(x - 1, y, z); }
                        if (q[0] < 0 || q[1] < 0 || q[2] < 0 || q[3] < 0) continue;
                        // the face normal points from the inside voxel to the empty one; Godot front faces wind
                        // clockwise seen from the front, so pick the order whose right-hand normal points inward
                        const Vector3 outward = Vector3(a == 0 ? 1.0f : 0.0f, a == 1 ? 1.0f : 0.0f, a == 2 ? 1.0f : 0.0f) * (here ? 1.0f : -1.0f);
                        const Vector3 rh = (verts[q[1]] - verts[q[0]]).cross(verts[q[2]] - verts[q[0]]);
                        const bool flip = rh.dot(outward) > 0.0f;   // right-hand normal outward = counter-clockwise from outside
                        if (!flip) { idx.push_back(q[0]); idx.push_back(q[1]); idx.push_back(q[2]); idx.push_back(q[0]); idx.push_back(q[2]); idx.push_back(q[3]); }
                        else { idx.push_back(q[0]); idx.push_back(q[2]); idx.push_back(q[1]); idx.push_back(q[0]); idx.push_back(q[3]); idx.push_back(q[2]); }
                        for (int k = 0; k < 4; k++) acc_n[q[k]] += outward;
                    }
                }
        norms.resize(verts.size());
        for (int64_t i = 0; i < verts.size(); i++) {
            const Vector3 n = acc_n[(size_t)i].length_squared() > 1e-12f ? acc_n[(size_t)i].normalized() : Vector3(0, 1, 0);
            norms.set(i, n);
        }
    };
    PackedVector3Array gv, gn;
    PackedColorArray gc;
    PackedInt32Array gi;
    build(false, gv, gn, gc, gi);
    if (gv.size() > 0) {
        PackedVector2Array uvs, uvs2;
        PackedByteArray custom;
        uvs.resize(gv.size());
        uvs2.resize(gv.size());
        custom.resize(gv.size() * 4);
        for (int64_t i = 0; i < gv.size(); i++) {
            uvs.set(i, Vector2(gv[i].y, 0.0f));
            uvs2.set(i, Vector2(gn[i].x, gn[i].z));
            const Color c = gc[i];
            custom.set(i * 4 + 0, (uint8_t)std::clamp((int)std::lround(c.r * 255.0f), 0, 255));
            custom.set(i * 4 + 1, (uint8_t)std::clamp((int)std::lround(c.g * 255.0f), 0, 255));
            custom.set(i * 4 + 2, (uint8_t)std::clamp((int)std::lround(c.b * 255.0f), 0, 255));
            custom.set(i * 4 + 3, 255);
        }
        out["vertices"] = gv;
        out["normals"] = gn;
        out["colors"] = gc;
        out["uvs"] = uvs;
        out["uvs2"] = uvs2;
        out["custom0"] = custom;
        out["indices"] = gi;
    }
    PackedVector3Array wv, wn;
    PackedColorArray wc;
    PackedInt32Array wi;
    build(true, wv, wn, wc, wi);
    if (wv.size() > 0) {
        for (int64_t i = 0; i < wc.size(); i++) { Color c = wc[i]; c.a = 0.6f; wc.set(i, c); }
        out["water_vertices"] = wv;
        out["water_normals"] = wn;
        out["water_colors"] = wc;
        out["water_indices"] = wi;
    }
    out["ms"] = (double)(Time::get_singleton()->get_ticks_usec() - t0) / 1000.0;
    return out;
}


// ---------- edits: the real surface of a chunk column against the generated one ----------

namespace {
inline bool tops_solid(uint16_t m) { return m != 0 && !nets_liquid(m) && m != WorldGen::STEAM && m != WorldGen::SMOKE && m != WorldGen::FIRE; }
}

PackedInt32Array FarMesh::column_tops(Vector2i chunk, int y0, int count, const PackedByteArray &bytes) const {
    PackedInt32Array out;
    out.resize(256);
    out.fill(-1);
    if (gen_.is_null() || count <= 0) return out;
    const int S = 16, H = S * count;
    if (bytes.size() != (int64_t)S * H * S * 2) return out;
    const FineGen &f = *gen_.ptr();
    const Vector3i origin(chunk.x * S, y0, chunk.y * S);
    const PackedByteArray gen = f.generate_block(origin, Vector3i(S, H, S), 0, 2);
    const bool gen_uniform = gen.size() == 2, gen_full = gen.size() == bytes.size();
    const uint8_t *a = bytes.ptr();
    const uint8_t *g = gen.ptr();
    auto at = [&](const uint8_t *p, int x, int y, int z) -> uint16_t {
        const size_t i = 2 * ((size_t)y + (size_t)H * ((size_t)x + (size_t)S * z));
        return (uint16_t)(p[i] | (p[i + 1] << 8));
    };
    const uint16_t gen_one = gen_uniform ? (uint16_t)(g[0] | (g[1] << 8)) : 0;
    for (int z = 0; z < S; z++)
        for (int x = 0; x < S; x++) {
            int ta = -1, tg = -1;
            for (int y = H - 1; y >= 0 && ta < 0; y--)
                if (tops_solid(at(a, x, y, z))) ta = y;
            if (gen_full) {
                for (int y = H - 1; y >= 0 && tg < 0; y--)
                    if (tops_solid(at(g, x, y, z))) tg = y;
            } else if (gen_uniform && tops_solid(gen_one)) {
                tg = H - 1;
            }
            if (ta < 0 || ta == tg) continue;   // emptied below the span (unknown) or as generated
            out.set(z * S + x, y0 + ta + 1);
        }
    return out;
}

} // namespace godot
