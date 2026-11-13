# 0084 – Every block painted: calm, readable textures from a script

- **Status:** accepted (tool tier: data and a script; drafts until the developer paints over them)
- **Date:** 2026-11-12

## Context
Most blocks still showed their shader pattern (ADR 0039), which reads as busy checker noise up close (bug
2026-11-12_20-49-17: the grass, the blue lamp, the log ends). The developer chose the style for painted
textures: **calm and readable** – three or four colours from one ramp of the palette, low contrast between
neighbours, and a material shape instead of noise; the rocks of `data/rocks.toml` told apart at a glance, each
ore of `data/minerals.toml` visible on its host rock, tiling without a repeat grid, readable from 100 m.
The rules they live under: 16 x 16 PNGs, up to four variants side by side, per-face and season files, every
pixel a palette colour (ADR 0044, 0045, 0061, 0064, 0069, 0080). `gravel.png` is the developer's own (untracked
in the main checkout) and water's look belongs to another session.

## Options considered
1. **Paint by hand in the Studio, block by block** – the Studio's way, but 61 blocks and 27 season looks in
   one night with no one to judge the eye; and a style change would mean repainting all of it.
2. **The Studio's pattern panel** (ADR 0064) – the eleven shader recipes snapped to the palette. They are the
   noise the developer wants gone; they have no strata, crystals, blades or rings.
3. **A small generator under `tools/` that paints from palette names** – one recipe per material, the colour
   steps named, deterministic from the block's name. A style change is one edit and a run; the result is
   plain PNGs the developer can repaint in the Studio.

## Decision
Option 3: `tools/paint_textures.py` (`python -I tools/paint_textures.py [block ...] [--preview out/x.png]`).

- **Ramps of five steps by key:** `d` dark, `s` shade, `m` mid, `l` lit, `L` light. A palette ramp with
  `X dark`, `X`, `X light` gains `X shade` (55 % of the way from dark to mid) and `X lit` (a third of the way
  from mid to light) the first time a texture asks for them – the palette's own steps are too far apart for
  calm neighbours (grass to grass light is a jump of 0.36 in value). A block with no ramp gets one from its
  `blocks.toml` colour (darker steps lean cool, lighter ones warm, as the Studio's "Add shades"); a season look
  from that season's row (`grass autumn`, `shrub winter`, ...). Only colours a texture uses are added.
- **Most texels are `s`, `m`, `l`; `d` draws the shapes** (cracks, joints, laminae, rims), `L` the glints.
  A field of shares (`levels`) sets exactly how much of each, so a texture's mean and calm do not drift.
- **Shapes per material** (top/bottom in `<block>.png`, the sides in `_side` where strata or bark show):
  - grass of every biome and season: fine two-tone ground with short dark blades and a few lit tips; the side is
    dirt with the grass hanging over the top edge in tufts of 2 to 5 rows, lighter at the edge, a shadow under it;
  - the rocks: stone soft beds and a crack; granite and gabbro coarse two-texel speckle (gabbro dark and green);
    gneiss broad wavy bands, schist fine wavy bands with mica glints; sandstone fine beds with two bedding lines;
    shale thin flaky laminae; limestone thick beds, bedding lines, a shell on top; slate two long cleavage
    partings; marble a grey vein; quartzite pale with white healed cracks; tuff clasts and pores; andesite pale
    phenocrysts; basalt columns (two joints on the side, a net on top);
  - the ores on their hosts: chalcopyrite brass crystals on andesite, cassiterite dark crystals on granite, galena
    cubes on limestone, pitchblende black lumps with a yellow-green halo on granite, malachite banded green crusts
    on stone, gold flecks in white quartz, coal with glossy streaks, hematite red and grey bands (banded iron),
    bauxite nodules, bog iron rusty lumps in mud, gossan rust with cracks; the three gravels are the developer's
    gravel mix (its exact colours and weights) with copper, tin or gold grains;
  - log: rings with pith, a radial check and a bark edge on the ends; bark of vertical plates and dark furrows;
  - leaves as cutouts (soft clumps, the lit tone on their tops, few holes), spruce as slanted needles, reed as
    blades; autumn broadleaf takes the palette's autumn ramp; winter broadleaf and shrub are bare branches;
  - lamps: a two-texel frame round an even glow, four variants (ADR 0085 joins them); copper a bevelled brushed plate; sand fine grains with wind
    ripples on top; snow and ice near flat (ice one long crack); clay and mud smooth; Luna's dusts with small
    rocks, anorthosite with feldspar laths, iron-nickel with Widmanstätten bands.
- **Tiling:** all noise is periodic on 16 texels; strata, bark plates and grass overhang use the block's seed,
  not the variant's, so beds and tufts run on across neighbouring blocks; a vein or crack that crosses an edge
  runs corner to corner so it meets itself. Features no larger than four texels, so no repeat lattice shows on a
  plain (the first grass, with eight-texel patches, did show one and was changed).
- Every written file goes on `data/drafts.txt`; `gravel.png` and water are never written.

## Proof
- `technoape-studio check=1`: `{"check":"ok","palette":349,"textures":125,"refused":[]}`.
- Contact sheet of every texture, tiled 4 x 4 with variants mixed: `out/tex-preview.png` (read); Studio render
  sheets: `out/studio-sheets/<file>.png` (every file, 3 x 3), gathered with names in `out/all-textures.png` and `out/season-textures.png` (read).
- Filmstrips at the forest of the season views (noon of each season), the coast view at noon and dusk, walls of
  every rock, ore and other block placed with `place=` (noon), a stacked cliff of limestone, shale, sandstone and
  granite (noon and dusk), and a closed cave of granite with ores, a lamp and a blue lamp. Read; what changed
  from them: the first grass (eight-texel patches) showed a repeat lattice on a plain and was changed to fine ground with blades; chalcopyrite's crystals read as crosses and became small cubes; galena's cubes were too small; gold quartz lost a stray slate sliver; snow and the regoliths had busy grey flecks and became near flat; granite (and the ores on it) had too many dark specks for a cave wall seen close under a lamp and was calmed twice. The coast view is open sea on today's bake, so the shore was seen from the forest spot, from above.
- `cargo test -p studio -p engine`: engine 217 and 115 passed; studio's `drive` tests needed three changes, all in the test files: the two clay tests and the palette test assumed their block had no painting (`scratch_unpainted` removes it in the scratch copy), and the palette test scrolled the swatches by a fixed amount that no longer reached the end of 349 colours (`wheel -400`). 28 passed. `tools/views.sh`: every view with a painted block changed, by
  design; looked at and accepted.

## Consequences
- The palette grows from 109 to 349 colours, almost all of them `shade`/`lit` steps and the ramps of
  blocks that had none. Changing a colour in the Studio repaints every texture that uses it, as before; running
  the script again repaints from the recipes (and overwrites hand edits to those files: once the developer has
  painted a file, take it out of the script's reach by removing it from `data/drafts.txt` first – the script
  does not check that yet).
- A block added later has no texture until a recipe is written; the script names such blocks on stderr.
- The shader patterns stay as the look of a block without a file.

## Calls for the developer to confirm
- **The style itself**: the shares (mostly mid, a quarter shade, a quarter lit) and how strong the shapes are
  (cracks, laminae, crystals). One edit in the script moves all of them.
- **Grass blades**: dark strokes with a lit tip on every third, against a fine two-tone ground; the tufts on the
  side hang 2 to 5 texels.
- **Grass's new steps sit in a ramp of their own** (`grass steps`): the engine test `the_data_file_loads` holds the
  `grass` ramp to three colours. Join them when that test allows more (an engine change, left for the developer).
- **Palette size**: 349 colours; the generated ramps for each biome grass in each season are the bulk.
  Fewer is possible by letting the biome grasses share ramps (they would lose their exact `blocks.toml` tints).
- **Hosts for ores with none in `minerals.toml`**: chalcopyrite on andesite, malachite on stone, gold quartz as
  a white quartz vein, bog iron in mud.
- ~~Lamps tile as framed panels~~ – decided, below.
- ~~Winter leaves keep a third~~ – decided, below.
- **Bare crowns under fast leaves** are dark cubes (fast fills an empty texel with the block's colour,
  darker). Options: winter crowns always through the cutout pipeline (fancy's cost, on fewer blocks), or fast
  fills a mostly empty look's holes with the sky's colour; neither is a texture change.

## Review
Tool tier (CLAUDE.md principle 13): the diff is a script, data and this ADR; read through by the author against the
shots above. A fresh reviewer read the script (it is over 300 lines). Fixed: the colour nudge could loop forever near blue 255 (now searches outward and fails loudly); sand's dark grains used the wrong colour; the brightness order now uses the Studio's `luma` weights and every ramp the script grows is asserted to run dark to light (all do; `gold gravel` and `gold quartz`, the developer's own, do not by their names); `data/palette.toml` goes on the drafts list when colours are added; dead code went; shale's dashes wrapped 2.5 times; `--preview` without a path. Answered: grass bottoms are not unpainted, the shader takes a grass block's foot from its side block, `dirt.png`. Documented: a run for some blocks still builds every recipe, so the palette gains every step the set needs.

## The developer's calls (2026-11-13)
- **Calmer leaves.** In the forest view (`tests/views/seam.png`, autumn there) the canopy read as a busy
  yellow-and-black checker: a quarter to a third of every leaf texture was holes, scattered, and under the
  default fast leaves (ADR 0069) a hole is drawn as the leaf colour at 60 %, so gold leaves came out
  dotted with near black; autumn also jumped from rust to gold. Now: two close tones in soft clumps (cell 4)
  with the lit one on 15 % (clump tops), the texel under a hole in shade, no dark rims; the holes are 4 %
  (jungle) to 10 % (acacia) and gather in one or two blobs of a smooth field; autumn broadleaf is the orange
  with a shade and a lit step of its own (two palette colours added), rust and gold only at the ends; spruce's
  needles keep 6 % holes and lost their dark specks.
- **Bare winter branches.** The winter look of the deciduous crowns (`broadleaf leaves@winter`,
  `shrub@winter`) is branches in the log's bark colours: limbs that lean out as they rise through the block
  with twigs forking off (two limbs and five twigs a broadleaf texture, one and three a shrub), everything else
  open. Spruce, jungle and acacia have no winter look and stay green. **Seen in fancy leaves only:** with the
  default fast leaves an empty texel is the block's colour at 60 %, so a bare crown is a dark brown cube with
  lighter twigs (filmed: `out/winter2.png` fast, `out/winter3.png` fancy). Fixing that is engine work, for the
  developer: a call below.
- **Lamps join** into one glowing surface with the frame only on the outer edge: engine and shader code, its own
  ADR 0085 on its own branch.
- Views that change: `seam` (the canopy) and the season views of the forest (winter: bare crowns; spring,
  summer and autumn: the calmer leaves).

## Flora paintings (2026-11-13, `docs/design/flora.md` part D, ADR 0089)
The richness of the developer's Vintage Story references, in the calm style above: **each species is told apart
by its shape first, its colour second.** The colours are picked a step of hue apart around the mid green that a
climate tint (part B) will multiply, so the tint moves them all and keeps them apart.

- **Leaves** (four variants each, cutouts):
  - broadleaf: soft round clumps (as before);
  - oak: big lobed clumps, deep green, with notched rims round its holes;
  - birch: small light leaves, more open;
  - willow: hanging strands from the top of the block with long open slits between them, a curtain;
  - spruce: drooping tiers every five rows;
  - pine: needle clumps, rosettes lit at the tips, open between;
  - jungle: big broad ovals with a midrib;
  - acacia: flat layers of fine leaflets, open slits between;
  - palm: a rib on the slant with long leaflets hanging from it;
  - bamboo: slanted lance leaves.

  Holes are few and ragged. Each variant opens from none to its species' share, in clusters that start on the
  block's edge and wander in. Oak has one big notch, birch five small ones, and spruce and bamboo stretched ones.
  A cube of leaves stays at most 15 % open, and the script asserts it; the crowns' fluffy planes (part A) give
  the ragged outline. Two earlier tries failed: holes from a smooth noise field could fall along a whole row,
  cutting a slit across the block, and round blobs in the middle read as polka dots on a regular grid. Pine and
  bamboo now have a ground of dark needles or shaded leaves under their clumps and lances; they were 40 to 65 %
  open. Palm's fronds and willow's curtain stay open as continuous shapes that reach the block's edge: the
  willow's strands hang from the top edge, so they run on from the block above.
- **Bark** (`<log>_side`, three variants) and **ends** (`<log>`):
  - birch: white with dark lenticel dashes;
  - oak: grey ridges broken across;
  - pine: big one-tone plates parted by furrows (spruce: small plates);
  - willow: slanting furrows into diamonds;
  - jungle: pale with moss;
  - acacia: grey with orange under-bark;
  - palm: rings of leaf scars;
  - bamboo: a jointed green culm, its end a hollow ring.
- **Seasons:**
  - The deciduous species (broadleaf, oak, birch, willow) take a spring, an autumn and a bare winter look. Winter
    branches are in the species' bark colours; birch's twigs are in the log's dark brown.
  - Blossom is the spring look where it fits: broadleaf with pink-white flowers, birch and willow with catkins.
  - Fruit is a late-summer look `@fruit`: broadleaf apples, oak acorns, jungle fruit, palm coconuts, ripe berries.
    No season file holds it yet; part A decides how it is shown. Until then it waits in `out/flora/pending/`.
- **Small plants as planes** (ADR 0091's format: one upright plane a file in `data/plants/<name>.png`). One
  pixel is 1/16 m, the block's texel (the overseer's call). Sizes are in `PLANT_SIZES`, width x height in
  pixels, and were sent to the plants session to read its planes by:
  - short grass 16x7 and 16x6, tall grass 16x16 and 16x14, wild wheat 16x20, reeds 16x32;
  - fern 24x11, berry bush 24x20, flowers 16x8.

  A tuft fills its width edge to edge: a blade on every column and more besides, wrapping at the sides, so
  neighbouring tufts merge into a field as in the references. Each tuft has four tones (shade at the foot, each
  blade's own tone, a lit or light tip), and tall light blades stand in front of short dark ones.
  - short grass (and `.1`) a low dense fringe, tall grass (and `.1`), wild wheat (straw ears over a full tuft),
    fern (six fronds arching out of the middle), reeds two metres tall with cattails;
  - `flowers` in red and white, `.1` yellow, `.2` blue and white;
  - the berry bush with its looks `.bloom` (pink-white blossom) and `.fruit` (red berries), all three from the
    same leaves.

  Greens are the grass block's own ramp, so a meadow and its ground read as one field. The game
  tints only green pixels (g above r and b), so the greens take the climate's tint and the straw, petals,
  cattails and fruit keep their colour. On the grass block's side, the top row is mid with a few lit tips; it
  was all lit and drew a light line under standing blades.
- **Names** come from the other sessions' lists: the trees session's `out/flora-tree-blocks.txt` is read, and
  bamboo's stalk is the block `bamboo`. The plants session's `out/flora-plant-paintings.txt` gives the plants'
  names and sizes. A painting whose block does not exist on the branch, or whose look has no season file, is
  written to `out/flora/pending/`; so are the plants until `data/plants/` exists, and then they replace the plants
  session's placeholders. Its colours join the palette only when a file in `data/textures` uses them, so the
  palette grows with the blocks: +2 now (`blossom`, `catkin`). Rerun the script once the trees and plants
  branches are on main.
- **Proof:**
  - Look sheets, read: `out/flora/species.png` (every species, by season, bark and end, over sky so holes show),
    `out/flora/plants.png` (each plane at its size, standing on a grass block), and `out/flora/meadow.png` (a
    field of mixed planes on grass tops, near to far).
  - `technoape-studio check=1` ok (353 colours, 125 textures).
  - On a copy of `data/` with the trees session's twelve blocks added, the script writes 152 textures and the
    check passes (442 colours; +89 for the six new species and their seasons).
  - The spruce, jungle and acacia barks wait: those trees still share `log`.
- **Views that change:**
  - `seam` and the season views: broadleaf leaves now have four variants with round holes, and spruce has tiers;
  - acacia and jungle canopies;
  - every view with a grass block's side (its top row).
- **Winter twigs grow out of the wood** (the developer, 2026-11-13: "not just a block with random sticks
  flying"):
  - The bare winter looks (`broadleaf/oak/birch/willow leaves@winter`, `shrub@winter`) are four roles, in file
    order. Each one's wood is the painting's **bottom edge**:
    - `root`: one stem from the middle that fans out;
    - `root off-centre`: the same, leaning across;
    - `through`: a limb from the bottom edge to the top edge at x 8 with side twigs, so stacked blocks chain;
    - `tips`: a few fine twigs for the crown's outer blocks.
  - Limbs taper from three texels to one. They are bark-mid with a dark side, and a thin twig ends in a lit tip.
  - The script asserts that every texel reaches the bottom edge through its neighbours, so nothing floats.
    Before the change, the old floating sticks would have failed it, and twigs started mid-air fail it now.
  - ADR 0090 (crowns, part A) confirms this layout:
    - a leaf face beside the wood takes `root` or `root off-centre`, two steps away `through`, three steps away
      `tips`, and none further out;
    - the face is turned so the bottom edge meets the wood;
    - where the wood is straight behind the face, the lower half is folded about the middle.
  - So the root variants fan out low: two limbs leave the stem at rows 12 and 10, and the lower half reads on its
    own (`out/flora/winter-folded.png`).
  - A limb now stops where it leaves the painting. A fork from off the canvas could lean back in mid-air, and the
    no-floating assert caught it.
  - `out/flora/winter.png` shows each role on its log, and a small crown laid out as the shader turns it.
- **Fruit** is `@fruit` (late summer, ADR 0090). The script adds `fruit` to its season tags once the engine's
  season list names it; until then the fruit looks wait.
- **32 px a metre, rich colour** (the developer, 2026-11-13: "still too low quality of details and colours"):
  - All flora is painted again at 32 px a metre. Leaves and bark are 32x32 a face, in `data/textures/hd/<block>.png`,
    named as before (variants side by side, the season tags). The 16x16 files stay where they were. The Studio's
    check reads only the top folder, so `hd/` does not disturb it; the engine reads it once the crowns session's
    renderer takes 32 px faces.
  - Plants are planes at 32 px a metre, width x height in pixels: short grass 32x13, tall grass 32x32, wild wheat
    32x40, fern 48x22, reeds 32x64, flowers 32x16, berry bush 40x40. Each plant has `.1` and `.2` besides itself:
    another shape and a hue a few degrees off. The berry bush adds `.bloom` and `.fruit`.
  - Colour comes from **rich ramps** (`<name> rich` in the palette, `r1`..`r8`), eight tones from very dark to
    light. The lights are pulled toward warm yellow-green and the shadows pushed toward cool blue-green, sampled
    from the five pictures in `out/flora-refs`. Non-flora blocks stay calm at 16 px.
  - Leaves are single leaf shapes (oval, lobed, small, broad, leaflet, lance) stamped in clumps. The clumps are
    drawn back to front, each lit as a ball from the top left, over a dark accent that shows only in the gaps
    between them. The clumps are drawn until about 92 % of the mass is covered.
    - Willow is a curtain of strands, its leaves in pairs.
    - Spruce is drooping sprays in tiers.
    - Pine is needle rosettes over a mid-dark ground.
    - Palm is two or three fronds at their own slant.
  - Grass blades run from a dark root to a bright tip. Wild wheat is a bushy tuft: dry straw blades and arching
    stems with nodding, awned ears. Flowers are six heads, big and small, at their own heights. The berry bush
    is lumpy, with red berries.
  - Barks:
    - ridged (oak, broadleaf);
    - diamond (willow);
    - plates for pine and spruce: wrapping cells stretched along the trunk, deep furrows between them (a grid of
      columns read as brick);
    - birch;
    - palm rings;
    - culm (bamboo);
    - smooth with long fibres (jungle, acacia).
  - Winter twigs follow the same four roles at 32 px: the `through` limb is at x 16, and the root variants' low
    limbs leave at rows 25, 21 and 17.
  - A rich ramp that the script changes is written over in the palette, so a new hue takes effect.
  - The palette has 430 colours, 77 of them rich ramps for the blocks on this branch; the pending species add
    theirs when their blocks arrive.
  - Proof:
    - look sheets, read beside the references: `out/flora/species-hd.png`, `walls-a.png`, `walls-b.png`,
      `walls-bark.png`, `plants-hd.png`, `meadow-vs-ref.png`;
    - `technoape-studio check=1` ok (430 colours, 125 textures).
- **Deeper greens, painted for light** (the overseer and the developer, 2026-11-13):
  - The meadow's greens are now a fixed ramp, `MEADOW` in `tools/paint_textures.py`, sampled from reference 4:
    a deep, saturated green, yellow only in the two lightest tones. Blades stay mid green most of their length
    and are lit only at the tip (tone follows height^1.7). The variants turn its hue by -7 and +6 degrees. The
    berry bush and the fern sit a step deeper too.
  - Plants must take real light and shadow, so their painted shadow stays light:
    - no dark outlines: where the berry bush's mass meets the air the darkest tones are lifted to mid, and the
      fern's rib is dark only at its root;
    - dark at the root and bright at the tips.
  - The clutter models (ADR 0082), painted by `tools/paint_clutter.py`, take the same rich ramps. That tool
    imports `MEADOW`, `rich_rgb` and the palette from `paint_textures.py`, and adds the twelve ramps it uses to
    the palette:
    - stones, sticks and flint: the top lit, the sides from shade at the ground to light, soft noise, glints
      and pits;
    - the mushroom: a red cap, lit at the middle, with pale spots and pale gills;
    - grass, fern, flowers, fallen leaves and the berry bush: as the plants.
  - The palette now has 526 colours. The model files' boxes are unchanged; only their paintings change.
  - Proof: the sheets `out/flora/plants-hd.png`, `meadow-vs-ref.png` and `clutter.png` (the models' paintings,
    8x), read; `technoape-studio check=1` ok.
  - **Painted for the plants' shader** (claude/flora-plants, `shader.wgsl`):
    - The shader tints green pixels (g above r and b) with a per-channel multiply. It is about x(0.75, 0.86, 0.5)
      in a middling climate; the dry end raises red and cuts blue.
    - It then darkens every plant to x0.45 at the root.
    - So a blade's root is now only two tones down (tone = 2 + t^1.7 x 5), and the fern's rib and the stems start
      at tone 2.
    - `MEADOW`'s middle tones carry little red, so they stay green and do not turn olive; yellow is in the
      lightest tone only.
    - Proof: `out/flora/meadow-lit-vs-ref.png` is the meadow with that tint and depth shading applied, read beside
      reference 4.
  - `paint_textures.py` now keeps any colour that was in the palette before its run (`Palette.before`), so the
    clutter's ramps that `paint_clutter.py` added are not dropped.
- **Calm bark** (the overseer, from the crowns session's in-game still): at 9 m the oak's bark read as black and
  cream zebra stripes.
  - Every bark's tones are now squeezed into a narrow band of its ramp (`calm`): ridged and plates 1 to 4 or 1 to
    5, willow, smooth and palm 2 to 5. That gives two or three close browns or greys, with furrows a little darker.
  - The ridged bark (oak, broadleaf) is no longer a regular sine wave. Noise stretched along the trunk now carries
    more of its shape than the waves.
  - Birch's lenticels are dark grey and fewer.
  - Bamboo's nodes are a single step, no longer a stripe.
  - Proof: `out/flora/trunks-9m.png`, every bark as a two-face trunk at the still's scale (about 28 px a metre),
    beside the still's oak, read.
- **Outline, depth and distance for plants** (the developer, 2026-11-13: plants as the crowns, target
  `out/flora-refs/6-leaf-block-target.png` with references 3 and 4):
  - Outline:
    - A tuft's blade heights follow its own outline (`envelope`): two or three bulges with dents between them,
      falling to the sides. No tuft has a flat top or straight sides.
    - Roots sit in the middle 80 % of the plane, and the outer blades lean out.
    - Each variant draws its own outline.
    - Bushes are two or three lobes with dents between them, not a round blob.
  - Depth:
    - A tuft draws a back layer, darker, then a lighter front layer over it.
    - A bush keeps the lit upper left of each leaf clump, and loses up to two tones in its lower third, darker
      underneath and inside.
  - Distance:
    - A far painting per plant kind: `data/plants/<name>.far.png`, at 8 px a metre. Each 4x4 cell takes its
      most common colour where a quarter of it is filled, so the outline and the main colours stay.
    - `data/plants/meadow ground.png`: three 32 px tiles side by side, a mottled grass top in the meadow's
      greens with lit blade tips and a few petals and seed heads. It is for where the plants session thins
      plants out.
    - These names and sizes were proposed to the plants session (glass-clutter); its answer may move them.
  - Proof: the sheets `out/flora/plants-hd.png` and `meadow-vs-ref.png` (outline and depth, beside reference 4),
    and `far-vs-ref.png` and `far-plants.png` (distance), all read.
