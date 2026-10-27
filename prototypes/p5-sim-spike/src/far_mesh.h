#pragma once
// FarMesh (ADR 0074): the far layer beyond the full-detail box, the way Lay of the Land draws
// distance (docs/research/2026-10-26-lay-of-the-land-distance.md): the silhouette of the terrain,
// flat water at its level, trees as blobs to the horizon, colours continuous with the near voxels.
//
// A tile is CELLS x CELLS cells of a column height field sampled from FineGen's macro lattice (the
// same hc / gw / hydrology the voxels come from, so the skin and the voxels agree); level L cells are
// BASE_STEP << L voxels (25 cm, 50 cm, 1 m, 2 m, 4 m, 8 m). Every vertex also carries the height and
// colour it has at the parent level (CDLOD): the shader morphs a tile into its parent over the outer
// part of its ring, so a ring change never pops and tiles of two levels meet without cracks. Water
// is a flat sheet per wet cell at the cell's level. Trees are instances (level <= TREE_LEVEL) from
// WorldGen's tree cells; beyond that the canopy is baked into the skin as a raised, leaf-coloured
// surface, which is what a forest is from a kilometre. Built on worker threads: no voxel volume is
// generated for the horizon, only columns.
//
// Units: positions in world voxels (12.5 cm), absolute (the mesh instance sits at the origin).

#include "fine_gen.h"

#include <godot_cpp/classes/ref_counted.hpp>
#include <godot_cpp/variant/dictionary.hpp>
#include <godot_cpp/variant/packed_color_array.hpp>

namespace godot {

class FarMesh : public RefCounted {
    GDCLASS(FarMesh, RefCounted)

public:
    static constexpr int CELLS = 64;       // cells per tile edge
    static constexpr int BASE_STEP = 2;    // voxels per cell at level 0 (25 cm)
    static constexpr int MAX_LEVEL = 6;
    static constexpr int TREE_LEVEL = 3;   // trees are instances up to this level (384 m with R0 48 m), baked canopy after
    static constexpr int TREE_KINDS = 5;   // WorldGen::TreeKind: oak, spruce, acacia, jungle, birch

    void configure(Ref<FineGen> gen);
    // index = material id, the colour a top face of that material averages to (the atlas tile's mean)
    void set_material_colors(PackedColorArray colors);
    PackedColorArray get_material_colors() const { return colors_; }
    int tile_voxels(int level) const { return CELLS * (BASE_STEP << level); }

    // One tile. Keys: "vertices", "normals", "colors", "uvs" (x = parent height, y = 0), "custom0"
    // (parent colour, RGBA8), "indices" for the ground; "water_vertices", "water_normals", "water_colors"
    // (rgb = bed colour, a = depth in metres / 8, clamped), "water_indices"; "trees": PackedFloat32Array of
    // 7 floats per tree (x, ground y, z, crown radius, crown height, kind, age); "crowns": Array of TREE_KINDS
    // MultiMesh buffers (16 floats per instance: the transform's rows, scaled and turned about y, then rgba),
    // "trunks": one such buffer (level <= 1); "ms": build time.
    // `overrides` (edits, ADR 0074): Vector2i chunk column (16 x 16 voxels) -> PackedInt32Array of 256 surface
    // heights (world y of the top solid voxel + 1, x fastest), -1 where the column is as generated.
    Dictionary build_tile(int level, int tx, int tz, const Dictionary &overrides = Dictionary()) const;
    // The surface heights of one chunk column of the real voxel data where they differ from the generated
    // ones: `bytes` is the 16 x (16 * count) x 16 block of 16-bit materials at (cx * 16, y0, cz * 16) in
    // VoxelBuffer order (World.read_column). 256 entries, -1 = as generated or no surface in the span.
    PackedInt32Array column_tops(Vector2i chunk, int y0, int count, const PackedByteArray &bytes) const;
    // Candidate (a) of P13: the same tile as a smoothed surface (naive surface nets) over FineGen's
    // coarse voxel data at lod = level + 1, vertex colours from the solid corners' materials; water as a
    // second surface. Same keys as build_tile (parent height = own height, parent colour = own colour:
    // no morphing), no trees (they are in the voxel data up to lod 3).
    Dictionary build_nets_tile(int level, int tx, int tz) const;

protected:
    static void _bind_methods();

private:
    struct Node {
        float hc = 0.0f;       // macro surface (macro units)
        float gw = 1.0f;       // fine ground weight
        int water = 0;         // water surface (fine voxels), columns below it are wet
        uint16_t top = 1;      // top material
        int biome = 0;
        float density = 0.0f;  // tree density of the biome
        bool canopy_ok = false;   // a forest canopy may be baked here (above water, not steep, not snow)
        bool tree_ok = false;     // a tree instance may stand here
        float slope = 0.0f;
    };
    Ref<FineGen> gen_;
    PackedColorArray colors_;

    Color color_of(uint16_t mat) const;
    float ground(int level, float base, int x, int z, float w) const;
    uint16_t top_material(const FineGen::MacroPt &m, float slope, int h, uint32_t ph) const;
    float canopy(int level, const Node &n, int x, int z) const;
};

} // namespace godot
