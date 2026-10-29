# ADR 0107: Corner occlusion is computed in the terrain shader, so every cube face merges; 72 m is the default

Status: accepted (2026-10-29). Follows ADR 0106 (merged cube faces).

## Context
ADR 0106 merges only faces whose baked corner occlusion (vertex colours) a rectangle can carry. A run with
occlusion off showed what the rest costs: 72 m flight 8.3 M -> 4.8 M primitives, GPU p99 6.1 -> 4.1 ms; 100 m
GPU p99 9.3 -> 5.7 ms. The developer asked for that cut and for 72 m as the default (2026-10-29).

Tried first: per-rectangle occlusion texels packed into a 256x256 page per mesh block (UV2 into a Texture2DArray).
It overflowed: on a 12.5 cm hillside or a forest canopy nearly every face is shaded, ~1,400 rectangles per page did
not fit; 512x512 pages still overflowed (~60 per page) at +550 MB VRAM. Dropped.

## Decision
`voxel_patch_ao_pages.py` (godot_voxel), `VoxelMesherBlocky.set_ao_pages(true)` (`Settings.ao_pages`, arg
`ao_pages=0` for A/B):
- every face of a plain cube merges with its neighbours of the same model whatever its occlusion; merged faces carry
  UV2 = (1, 0) and the model colour, every other face keeps its baked vertex occlusion and UV2 = (0, 0);
- each 64-voxel mesh block also outputs one bit per voxel of the block and one voxel around it (66^3 bits, an R8
  image 66 x 594, 39 KB), set where the voxel darkens corners (`contributes_to_ao`);
- `VoxelLodTerrain.set_ao_pages(textures, layers_per_texture, layers)` gives each mesh block a layer of a
  Texture2DArray, uploads the image with the mesh and sets the block's `ao_layer` instance uniform; layers of
  blocks that are gone return to a free list about once a second. GPUs allow 2048 layers per array, so
  `world.gd` makes two (sized for the largest render distance: 2,960 layers, ~115 MB);
- `terrain.gdshader` reads the 8 voxels around the air cell the face looks into and computes godot_voxel's corner
  occlusion (two side neighbours = 3 levels, else sides + corner; `occlusion_darkness / 3` per level), interpolated
  bilinearly across the face (the baked version interpolated per triangle).

`view_distance_m` defaults to 72 m, the high preset too (medium 40 m, low 24 m).

## Measurements
Spectator autopilot flights (seed 272905368, 20 s, vsync off), `ao_pages=0` (ADR 0106 only) / `=1`:

| box | p99 frame | primitives | GPU p99 | VRAM |
|---|---|---|---|---|
| 40 m, shader occlusion | 3.6 ms | 5.2 M | 2.8 ms | 0.9 GB |
| 72 m, baked -> shader | 8.2 -> 5.9 ms | 9.0 M -> 5.0 M | 6.8 -> 4.8 ms | 1.7 -> 1.5 GB |
| 100 m, baked -> shader | 30.1 -> 7.5 ms | 13.9 M -> 7.9 M | 6.9 -> 5.2 ms | 3.2 -> 2.5 GB |

100 m's p99 moves between 7 and 30 ms from run to run with either setting: main-thread stalls while the box
streams in (next job), not rendering. A still frame at 72 m differs from the baked occlusion only where animals
moved and in faint noise. `test_merge_faces.gd` checks the mode's mesh (fully merged step, UV2 marks, no baked
occlusion in the colours).

## Consequences
- One layer per mesh block with a mesh (100 m used ~2,650). If the layers run out, the terrain warns once and
  merged faces of the blocks without one draw unshaded.
- Only cubic blocks get a volume (the material warm-up's buffers do not); props and item cubes use other shaders.
- The occlusion costs 8 texel reads per terrain pixel.
