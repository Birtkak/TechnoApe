#include "register_types.h"
#include "micro_mesher.h"
#include "rigid_pieces.h"
#include "sim_world.h"
#include "world_gen.h"
#include "fine_gen.h"

#include <gdextension_interface.h>
#include <godot_cpp/core/defs.hpp>
#include <godot_cpp/godot.hpp>

using namespace godot;

void initialize_technosim_module(ModuleInitializationLevel p_level) {
    if (p_level != MODULE_INITIALIZATION_LEVEL_SCENE) {
        return;
    }
    GDREGISTER_CLASS(SimWorld);
    GDREGISTER_CLASS(WorldGen);
    GDREGISTER_CLASS(FineGen);
    GDREGISTER_CLASS(MicroMesher);
    GDREGISTER_CLASS(RigidPieces);
}

void uninitialize_technosim_module(ModuleInitializationLevel p_level) {
    if (p_level != MODULE_INITIALIZATION_LEVEL_SCENE) {
        return;
    }
}

extern "C" {
GDExtensionBool GDE_EXPORT technosim_library_init(GDExtensionInterfaceGetProcAddress p_get_proc_address,
        const GDExtensionClassLibraryPtr p_library, GDExtensionInitialization *r_initialization) {
    GDExtensionBinding::InitObject init_obj(p_get_proc_address, p_library, r_initialization);
    init_obj.register_initializer(initialize_technosim_module);
    init_obj.register_terminator(uninitialize_technosim_module);
    init_obj.set_minimum_library_initialization_level(MODULE_INITIALIZATION_LEVEL_SCENE);
    return init_obj.init();
}
}
