# Block look spike

**Question (roadmap, ADR 0002):** patterned flat-colour blocks or pixel textures, and if textures, how many
texels per 0.5 m block?

**How:** four looks in the client's fragment shader, picked with `look=0..3` (`crates/client/src/shader.wgsl`),
rendered from the same two standing spots at 1280 px: a terrace 1-6 m away (`at=1,0.2,0.4 yaw=200 pitch=-25`)
and a valley with hills 20-200 m away (`at=0.3,1,0.2 yaw=0 pitch=-10`). The sheet is `sheet.png`.

| look | what it is |
|---|---|
| 0 | flat colour per block, slight per-block tint, dark edge line (the engine spike's look) |
| 1 | flat colour, one-texel rim, faint 4x4 checker inside each face |
| 2 | pixel texture, 16 texels per block (3.125 cm): 3-tone noise, grass fringe over dirt, stone cracks |
| 3 | the same at 32 texels per block (1.56 cm) |

In every look a grass block's top is grass and its sides are dirt, so only the surface style differs.
The textures are made in the shader from a hash on the texel grid, not loaded from images: the question is
what the grid size looks like, and that reads the same either way. Real 16x16 images come with content.

**Cost:** none measurable; every look is a few ALU ops per pixel on top of the same geometry.

**Answer (2026-11-10):** look 2, **16 texels per block**. 16 px reads as deliberate pixel art up close and
still shows texture on far hills; 32 px turns to noise near and mush far; the flat looks make the far hills
bands of plain colour. Recorded in ADR 0002 and the roadmap; the shader keeps only this look.
