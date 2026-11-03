#pragma once
// ADR 0113 / 0122 / 0123: networks over block entities. Entities (machines, cables, pipes) are boxes on the 0.5 m
// block grid; boxes that touch face to face are connected.
//  - solve(): the flow model for steam and water (ADR 0122): per component min(supply, demand, weakest
//    conductor), sources share in proportion, sinks fill in node order, low sinks take what is left.
//  - solve_dc(): the electrical model (ADR 0123): a DC circuit in real units. Sources are an EMF with an internal
//    resistance, conductors a series resistance, loads a conductance (P / V^2). Runs of plain conductors collapse
//    into one resistor, the reduced network is solved by nodal analysis (conjugate gradient, Jacobi
//    preconditioned), and every node gets its voltage, the current through it and its power.

#include <godot_cpp/classes/ref_counted.hpp>
#include <godot_cpp/variant/dictionary.hpp>
#include <godot_cpp/variant/packed_float32_array.hpp>
#include <godot_cpp/variant/packed_int32_array.hpp>

#include <vector>

namespace godot {

class NetworkSolver : public RefCounted {
    GDCLASS(NetworkSolver, RefCounted)

public:
    enum Role { CONDUCTOR = 0, SOURCE = 1, SINK = 2, SINK_LOW = 3 };
    enum DcKind { DC_CONDUCTOR = 0, DC_SOURCE = 1, DC_LOAD = 2 };
    // boxes: 6 ints per node (min x, y, z in world voxels, size x, y, z in voxels); roles: one per node;
    // amounts: a conductor's capacity per tick, a source's offer, a sink's want. Returns per node
    // [component, flow]: flow = what a source gave or a sink got this tick (0 for conductors).
    PackedFloat32Array solve(const PackedInt32Array &p_boxes, const PackedInt32Array &p_roles, const PackedFloat32Array &p_amounts);
    // The last solve's components: {count, supply, demand, flow} summed over all.
    Dictionary stats() const { return stats_; }
    // boxes as above; kinds: DC_CONDUCTOR (a = its resistance in ohms), DC_SOURCE (a = EMF in volts, b = internal
    // resistance in ohms), DC_LOAD (a = conductance in siemens, i.e. rated watts / rated volts^2). Returns per node
    // [component, volts at it, amps through it (a source: positive while it delivers, negative while it takes),
    // watts (a load: taken; a source: delivered)].
    PackedFloat32Array solve_dc(const PackedInt32Array &p_boxes, const PackedInt32Array &p_kinds, const PackedFloat32Array &p_a, const PackedFloat32Array &p_b);
    // The last solve_dc: {count, reduced_nodes, iterations, delivered_watts}.
    Dictionary dc_stats() const { return dc_stats_; }

protected:
    static void _bind_methods();

private:
    // components and the touching pairs of the boxes: parent[] after union-find, edges as (i, j) pairs
    void connect(const PackedInt32Array &p_boxes, int n, std::vector<int> &parent, std::vector<std::pair<int, int>> &edges);
    Dictionary stats_;
    Dictionary dc_stats_;
};

} // namespace godot

VARIANT_ENUM_CAST(godot::NetworkSolver::Role);
VARIANT_ENUM_CAST(godot::NetworkSolver::DcKind);
