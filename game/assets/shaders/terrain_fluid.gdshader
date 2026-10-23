shader_type spatial;
render_mode cull_back, depth_draw_always;

// See-through liquid surfaces (water, oil, blood, toxic sludge) as
// Minecraft-style fluids with levels; body in terrain_fluid.gdshaderinc.

#define FLUID_SEE_THROUGH
#include "res://assets/shaders/terrain_fluid.gdshaderinc"
