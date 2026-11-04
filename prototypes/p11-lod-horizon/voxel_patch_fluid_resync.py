# Re-applies the fluid mesher part of voxel_patch_fluid.py to a tree patched with an older copy of it (ADR 0138).
# The shipped build (2026-10-29) had the ADR 0069 height field but not ADR 0103's half (connected column tops,
# falling water as full boxes, no A3 face hiding on a falling column): voxel_patch_fluid.py grew those after the tree
# was patched, and running it again fails its asserts on the parts already in. So falling water drew as shards and
# glass slivers pulled toward whatever water lay in reach. This puts blocky_fluids_meshing_impl.h back to stock (git)
# and runs the script's part for that file; the PADDING and vertex-position parts stay as they are.
# python voxel_patch_fluid_resync.py ~/Tools/godot_voxel   (then rebuild, see game/README.md)
import os
import subprocess
import sys

root = sys.argv[1]
rel = 'meshers/blocky/blocky_fluids_meshing_impl.h'
stock = subprocess.run(['git', '-C', root, 'show', 'HEAD:' + rel], check=True, capture_output=True).stdout
with open(os.path.join(root, rel), 'wb') as f:
    f.write(stock.replace(b'\r\n', b'\n'))
src = open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'voxel_patch_fluid.py'), encoding='utf-8').read()
part = src[src.index("p = root + '/meshers/blocky/blocky_fluids_meshing_impl.h'"):]
exec(compile(part, 'voxel_patch_fluid.py (fluid mesher part)', 'exec'), {'root': root})
s = open(os.path.join(root, rel), encoding='utf-8').read()
assert 'fluid_falls' in s and 'fluid_column_top' in s
