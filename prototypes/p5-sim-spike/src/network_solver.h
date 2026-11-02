#pragma once
// ADR 0113: power and fluid networks. Block entities (machines, cables, pipes) are boxes on the 0.5 m block
// grid; boxes that touch face to face are connected. Each connected component moves min(supply, demand,
// weakest conductor) per tick: sources give in proportion to what they offer, sinks are filled in node order
// (a machine runs whole or waits), low sinks share what is left. One solve per channel (energy, steam, water...) once a second, native because the cable
// count is what grows (a factory: thousands).

#include <godot_cpp/classes/ref_counted.hpp>
#include <godot_cpp/variant/dictionary.hpp>
#include <godot_cpp/variant/packed_float32_array.hpp>
#include <godot_cpp/variant/packed_int32_array.hpp>

namespace godot {

class NetworkSolver : public RefCounted {
    GDCLASS(NetworkSolver, RefCounted)

public:
    enum Role { CONDUCTOR = 0, SOURCE = 1, SINK = 2, SINK_LOW = 3 };   // SINK_LOW: fed only from what the sinks leave (batteries)
    // boxes: 6 ints per node (min x, y, z in world voxels, size x, y, z in voxels); roles: one per node;
    // amounts: a conductor's capacity per tick, a source's offer, a sink's want. Returns per node
    // [component, flow]: flow = what a source gave or a sink got this tick (0 for conductors).
    PackedFloat32Array solve(const PackedInt32Array &p_boxes, const PackedInt32Array &p_roles, const PackedFloat32Array &p_amounts);
    // The last solve's components: {count, supply, demand, flow} summed over all.
    Dictionary stats() const { return stats_; }

protected:
    static void _bind_methods();

private:
    Dictionary stats_;
};

} // namespace godot

VARIANT_ENUM_CAST(godot::NetworkSolver::Role);
