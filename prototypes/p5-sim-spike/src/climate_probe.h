#pragma once
// ADR 0040: what the player's surroundings do to their temperature. One call flood-fills the open
// cells (air, gases, plants, leaves, liquids) from the player's head inside a box: if the fill closes
// before it reaches the box edge, the window edge or the cell cap, the player is in a room (shelter).
// The same call adds up the heat of fire and lava cells nearby.

#include "sim_world.h"

#include <godot_cpp/classes/ref_counted.hpp>
#include <godot_cpp/variant/dictionary.hpp>
#include <godot_cpp/variant/packed_byte_array.hpp>
#include <godot_cpp/variant/packed_float32_array.hpp>
#include <godot_cpp/variant/vector3i.hpp>

#include <vector>

namespace godot {

class ClimateProbe : public RefCounted {
    GDCLASS(ClimateProbe, RefCounted)

public:
    // start: the SimWorld's local cell to fill from. half: half size of the box (cells) the room must
    // fit in. cap: most cells filled. open: per material, 1 = the fill passes through it. heat: per
    // material, heat of one cell (0 for most). heat_radius (cells): how far heat counts, falling off
    // as 1 / (1 + (d / heat_falloff)^2).
    // Returns {enclosed: bool, cells: int (filled), heat: float, heat_cells: int}.
    Dictionary probe(const Ref<SimWorld> &p_sim, Vector3i p_start, Vector3i p_half, int p_cap, const PackedByteArray &p_open,
            const PackedFloat32Array &p_heat, int p_heat_radius, float p_heat_falloff);

protected:
    static void _bind_methods();

private:
    std::vector<uint32_t> stamp_;
    uint32_t gen_ = 0;
    std::vector<int32_t> queue_;
};

} // namespace godot
