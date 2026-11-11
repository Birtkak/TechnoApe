# 0044 – The Studio: our own art tool, and hand-made looks

- **Status:** proposed
- **Date:** 2026-11-11

## Context
The game's looks so far are made by Claude: block faces are shader recipes (ADR 0039), and there are no
models yet. The developer wants their own hand in every texture, model and effect, since art an AI made
whole is disliked, and proposed a stand-alone program of our own to make them in, fitted exactly to the
engine. Constraints: one pixel grid of 16 px per 1 m block (vision, point 4); content is data; the frame
budget of ADR 0004; Claude must be able to check the tool and its files without eyes.

## Options considered
1. Existing tools (Aseprite for textures, MagicaVoxel or Blockbench for models) and importers – nothing to
   build but importers; but their previews are not the game's light, their formats carry what we do not want
   and lack what we do (our parts and clips, our grid), and it is not the developer's own tool.
2. An editor mode inside the game – the preview is the game by construction; but the game's small menu
   toolkit would have to grow into an editor's, and editor code would sit in the client.
3. Our own second program in the workspace, drawing with the game's renderer and writing the game's own data
   files – the preview is the game's, the formats are exactly ours, the client stays a game; it is the
   largest to build.

## Decision
Option 3, as designed in `docs/design/studio.md`. The developer's calls of 2026-11-11:

- Block textures and voxel models first; particles, then trees and structures, later.
- Model voxels are 1/16 m, the texture pixel's size.
- Models have rigid moving parts with keyframed clips from the first version.
- Claude may make drafts; they are listed (`data/drafts.txt`) until the developer has redone them.
- One fixed palette (`data/palette.toml`): every texture and model is painted from it, and the file check
  refuses a colour outside it.

Routine defaults, recorded: textures are PNG files that replace a block's pattern as a recording replaces a
code-made sound (ADR 0032), so any paint program also works and no block must be painted before the rest
runs; models are text files; the Studio has a headless mode that validates files and writes filmstrips.

Still to decide, each in its own ADR when its step is built: whether blocks draw from a texture array (after
the spike of step 1 has measured it against ADR 0039's patterns), how the Studio shares the renderer's code,
its interface library, the model file's exact layout and how models are drawn within the budget.

## Consequences
- The developer can paint a block the day step 2 lands, before the Studio exists.
- Model drawing is new engine work with its own benchmark; the Studio's model page waits for it.
- Two programs share one renderer: a change to the renderer must keep both building, and the gate grows by
  the Studio's file check.
- The shader patterns stay as the look of every block not yet painted, and as a start for a painting.
- The fixed palette makes a later change of mood one edit (change a colour, everything follows), and means
  the palette must exist before the first texture is painted.
- Revisit: the program's name and per-voxel glow are open questions in the design doc.
