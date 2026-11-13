# Flora: trees, shrubs and grass that look alive

- **Status:** design, 2026-11-13 (the developer's calls of that day; ADR 0089). Nothing here is built.
- **Reference:** Vintage Story (the developer's five screenshots): soft ragged crowns with branches showing
  inside, layered conifers, many species, meadows of tall grass and flowers as crossed planes, berry bushes,
  yellow-green dry land against deep green wet land.
- **From:** ADR 0069 (cutout blocks, fast and fancy leaves), ADR 0088 (fast leaves with open holes: built on
  branch `claude/open-leaf-holes`, not merged), ADR 0082 (clutter: still models in the chunk mesh, computed
  from the seed), ADR 0084 (calm, readable textures), ADR 0076 (the weather's wind), `seasonal-looks.md`,
  `biomes.toml`'s plants (`cone`, `round`, `flat`, `bush`, `column`).

## The developer's calls
1. **Fluffy crowns:** leaf blocks carry leaf planes that stick out past the block, so a crown is a soft mass,
   not cubes; branch blocks show inside.
2. **Small plants are crossed flat planes:** grass, ferns, flowers, meadow plants, berry bushes.
3. **Trees get branches and more species,** in small, medium and large sizes.
4. **Wind sway** for grass, leaves and small plants.
5. **Colour by climate:** yellow-green where it is dry and warm, deep green where it is wet, with the seasons.
6. **Dense meadows,** with a Vegetation setting (few, normal, many).
7. **Flowering and fruit:** blossom in spring, berries and fruit in late summer.

## The cost comes first
Measured on 2026-11-13 (forest fly-through, three quick runs each): drawing today's leaf shells through the
discarding pipeline (ADR 0088) moves GPU p50 from 2.9 to 3.8 ms and p99 from 6.3 to 7.4 ms; the budget is
8.3 ms (ADR 0004). Fluffy crowns and dense meadows add more discarded pixels on top. So:
- every part starts with a measured spike, and none is merged that takes fly's p99 over the budget at the
  default settings;
- the Vegetation and Leaves settings are the valve: the defaults must hold the budget on the dev PC;
- to rays, plants are whole blocks or nothing (ADR 0069's finding: non-opaque BLAS geometry is what costs);
- far levels stay one colour.

## Four parts
### A. Crowns (client, shader, mesher)
Extra leaf planes on a crown's outer faces, pushed out and turned by a hash of the place; holes open
(ADR 0088 is the base, to be made cheap enough or replaced); leaves sway. Seasons as today (`seasonal-looks.md`)
plus a blossom look in spring and fruit in late summer for the species that have them.

### B. Small plants (client, clutter)
A plant made of two or more painted planes (a new kind beside ADR 0082's voxel models, same computed
placement, no storage): short and tall grass, fern, reed, flowers, wheat-like meadow grass, berry bush.
Density per biome in `biomes.toml`; the Vegetation setting scales it. Sway in the vertex shader from the
weather's wind. A tint from the climate at the place (the bake's heat and wetness) and the season.
Voxel models stay for what is picked up.

### C. Trees (engine worldgen, data)
Branches as blocks inside the crown; layered conifers; sizes small, medium, large per species. New species
by biome, each a data row: birch, oak, pine, willow, palm, bamboo, beside spruce, broadleaf, jungle and
acacia. Deterministic from the seed as today; saves and the fixed views change (a rule change, like rocks).

### D. Paintings (tool tier)
Leaf, bark and plant paintings per species and season in the calm style, blossom and fruit looks, by
`tools/paint_textures.py`.

## Order
A and C and D side by side; B beside them or after A (both touch the client's terrain draw). Each merges alone,
gated on a quiet PC, with a filmstrip the developer can judge.
