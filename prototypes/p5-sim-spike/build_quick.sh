#!/bin/bash
# Builds the technosim DLL without SCons re-archiving godot-cpp (on Windows its archive step appends
# ~1 000 objects one `ar q` at a time: 20-30 min on a busy machine). Run SCons once to compile, then this.
# Compile changed sources first: bash build_quick.sh src/fine_gen.cpp ...   (flags as SCons uses them)
set -e
cd "$(dirname "$0")"
export PATH=~/Tools/mingw-dl/mingw64/bin:$PATH
CPP=${GODOT_CPP_PATH:-$HOME/Tools/godot-cpp}
for f in "$@"; do
  g++ -o "${f%.cpp}.o" -c -std=c++17 -fno-exceptions -Wwrite-strings -fvisibility=hidden -O2 -msse4.1 ${PROF:+-g} \
    -DWINDOWS_ENABLED -DTHREADS_ENABLED -DHOT_RELOAD_ENABLED -DDEBUG_ENABLED -DNDEBUG -DGDEXTENSION \
    -I"$CPP/include" -I"$CPP/gen/include" -I"$CPP/gdextension" -Isrc "$f" &
done
wait
g++ -Wl,--no-undefined -static -static-libgcc -static-libstdc++ -fvisibility=hidden -shared -o bin/libtechnosim.windows.template_debug.x86_64.dll \
  src/*.o "$CPP/bin/libgodot-cpp.windows.template_debug.x86_64.a"
ls -la bin/libtechnosim.windows.template_debug.x86_64.dll
