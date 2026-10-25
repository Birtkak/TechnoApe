"""Atlas tiles 136-167 (ADR 0057): rock kinds, their cobbles and bricks, and the new clutter pieces.

Called by make_atlas.py with its helpers, so this file holds only the colours and the brick
course layout. Every tile draws from its own RNG (never the shared one), so the tiles before
it keep their look whatever the merge order of the atlas rows.
"""
import random

from PIL import Image

TILE = 16

# Natural rock (terrain palettes, one texel per voxel picked by position: weights, no pattern).
# Stone (tile 0) is the common grey rock (andesite in all but name) and basalt has its own tiles
# (69, 70): with these six that makes the eight rock kinds.
ROCK = {
    "granite": [("#b8958a", 5), ("#a68478", 4), ("#c9aea2", 3), ("#8d8a88", 3), ("#d9c6bc", 1.5), ("#3a3331", 1.2)],
    "diorite": [("#d6d4cf", 5), ("#c4c2bc", 4), ("#e8e6e1", 3), ("#a3a29d", 2), ("#45464a", 1.6), ("#2a2a2d", 0.8)],
    "limestone": [("#cfc4a8", 5), ("#bfb396", 4), ("#ddd3ba", 3), ("#a99d80", 2), ("#e8dfca", 1)],
    "slate": [("#525a65", 5), ("#5f6772", 4), ("#464c56", 3), ("#6d7581", 2), ("#3a3f48", 1.5)],
    "marble": [("#eceae5", 6), ("#dedcd5", 4), ("#f7f6f2", 3), ("#bdbab4", 1.4), ("#9a9792", 0.5)],
    "chalk": [("#f0ece0", 6), ("#e4dfcf", 4), ("#faf7ee", 3), ("#d3cdb9", 2), ("#bfb7a0", 0.6)],
}

# 8 colours per rock for its stones (cobble, the loose stones on the ground) and bricks:
# gap / mortar, deep shade, shade, body A, body B, light, rim, speck.
STONES = {
    "granite": ["#3b2f2b", "#6e5850", "#8f7469", "#ad8b7e", "#9c8f8a", "#c7a99c", "#e0cbc1", "#2a2422"],
    "diorite": ["#3e3d3a", "#77766f", "#9e9d96", "#c7c5be", "#b7b6b0", "#dcdad4", "#f1f0eb", "#26262a"],
    "limestone": ["#4e4636", "#7d7259", "#9d9174", "#bfb294", "#b0a385", "#d3c8ab", "#e9e0c9", "#8a6f4c"],
    "slate": ["#1f2228", "#353a42", "#454b55", "#59606b", "#4e5560", "#6c7480", "#858d99", "#9aa3a0"],
    "marble": ["#6d6a65", "#a8a59f", "#c4c1ba", "#e3e0d9", "#d6d3cc", "#efede8", "#fbfaf7", "#8d8a86"],
    "chalk": ["#7d7663", "#b3ac97", "#cbc5b1", "#e6e1d1", "#ddd7c6", "#f2eee3", "#fdfbf5", "#5d6772"],
    "basalt": ["#141417", "#26262b", "#323238", "#43434a", "#3b3b41", "#55555d", "#6b6b74", "#7a4f3a"],
    "stone": ["#36332e", "#5e5a53", "#7f7a71", "#9a958b", "#8e8a83", "#b2ada2", "#cfc9bd", "#5a554d"],
}


def _hexc(h):
    return tuple(int(h[i:i + 2], 16) for i in (1, 3, 5)) + (255,)


def ashlar(cols, seed):
    """Cut stone bricks: courses 4 voxels tall (3 stone, 1 mortar), blocks 8 long, every other course
    shifted by half; each block one of the body tones, lit along its top edge, shaded along its
    bottom, a few flecks. Wraps (patterned, ADR 0039)."""
    rng = random.Random(seed)
    mortar, deep, shade, body_a, body_b, light, rim, speck = [_hexc(c) for c in cols]
    img = Image.new("RGBA", (TILE, TILE))
    px = img.load()
    tone = {}
    for y in range(TILE):
        course, row = y // 4, y % 4
        for x in range(TILE):
            xs = (x + (4 if course % 2 else 0)) % TILE
            if row == 3 or xs % 8 == 7:
                px[x, y] = mortar if rng.random() < 0.8 else deep
                continue
            key = (course, xs // 8)
            if key not in tone:
                tone[key] = rng.choice([body_a, body_b, body_a, light])
            c = tone[key]
            if row == 0:
                c = rim if xs % 8 < 6 else light
            elif row == 2 and rng.random() < 0.5:
                c = shade
            r = rng.random()
            if r < 0.05:
                c = speck
            elif r < 0.1:
                c = shade
            px[x, y] = c
    return img


def cobble(cols, seed):
    """Rounded stones about 4 voxels across in a dark bed (the cobblestone tile's layout, ADR 0039),
    each stone lit from the top-left: rim, light, body, shade, deep. Wraps."""
    rng = random.Random(seed)
    gap, deep, shade, body_a, body_b, light, rim, speck = [_hexc(c) for c in cols]
    seeds = [(gx * 4 + rng.uniform(0.5, 3.5), gy * 4 + rng.uniform(0.5, 3.5), rng.randint(0, 1)) for gy in range(4) for gx in range(4)]
    img = Image.new("RGBA", (TILE, TILE))
    px = img.load()
    for y in range(TILE):
        for x in range(TILE):
            ds = []
            for sx, sy, tone in seeds:
                dx = x + 0.5 - sx
                dy = y + 0.5 - sy
                dx -= TILE * round(dx / TILE)
                dy -= TILE * round(dy / TILE)
                ds.append(((dx * dx + dy * dy) ** 0.5, dx, dy, tone))
            ds.sort()
            d0, dx, dy, tone = ds[0]
            if ds[1][0] - d0 < 0.8:
                px[x, y] = gap if rng.random() < 0.85 else deep
                continue
            lit = -(dx + dy)
            c = rim if lit > 1.6 else light if lit > 0.6 else deep if lit < -1.6 else shade if lit < -0.6 else (body_a if tone == 0 else body_b)
            if rng.random() < 0.05:
                c = speck
            px[x, y] = c
    return img


def cracked(cols, seed):
    """An old pot: the clay rings darkened, crossed by a few dark cracks that wander and branch."""
    rng = random.Random(seed)
    c = [_hexc(h) for h in cols]
    img = Image.new("RGBA", (TILE, TILE))
    px = img.load()
    for y in range(TILE):
        for x in range(TILE):
            px[x, y] = c[1 + (y // 2 + rng.randint(0, 2)) % 6]
    for _ in range(4):
        x, y = rng.randrange(TILE), rng.randrange(TILE)
        for _ in range(rng.randint(5, 10)):
            px[x % TILE, y % TILE] = c[0] if rng.random() < 0.7 else c[7]
            x += rng.choice([-1, 0, 1])
            y += 1
    return img


def tiles(palette, stones_pattern, stripes, spotted_cap):
    """The tiles 136..170 in order (168-170: pyrite and ochres, from part F's range, ADR 0057); make_atlas.py appends them after the placeholders."""
    t = []
    for i, k in enumerate(["granite", "diorite", "limestone", "slate", "marble", "chalk"]):
        t.append(palette(ROCK[k], 136 + i))                                   # 136-141 rock
    for i, k in enumerate(["granite", "diorite", "limestone", "slate", "marble", "chalk", "basalt"]):
        t.append(cobble(STONES[k], 142 + i))                   # 142-148 cobble and loose stones
    for i, k in enumerate(["stone", "granite", "diorite", "limestone", "slate", "marble", "chalk", "basalt"]):
        t.append(ashlar(STONES[k], 149 + i))                                 # 149-156 bricks
    t += [
        spotted_cap(["#4a0a10", "#7a1018", "#a3161f", "#c41e26", "#d92b2c", "#e8473c", "#f26b55", "#ffd0b8"], ["#f6e27a", "#e9c84a"], 157),   # 157 wild strawberries: red, yellow seeds
        stones_pattern(["#2e1d10", "#523519", "#6e4823", "#8a5c2e", "#9a6a36", "#b98a4e", "#d6ae72", "#e8d6a8"], 158, n=26),   # 158 hazelnuts: glossy brown shells
        stripes(["#2c1f10", "#4b3519", "#6a4c25", "#8a6a34", "#a07e3e", "#7c8a3a", "#c2a466", "#d9c58c"], 159, "rows", (2, 3)),   # 159 acorns: cap bands over the nut
        stripes(["#8b8170", "#b9b09b", "#cdc5b1", "#ddd6c4", "#e7e1d1", "#f1ecdf", "#fbf8ef", "#a39782"], 160, "cols", (1, 2)),   # 160 bones: ivory, fibres along
        stripes(["#3a3a40", "#5e5e66", "#8b8b93", "#b5b5ba", "#d5d5d8", "#ececee", "#ffffff", "#8a6a4c"], 161, "diag", (1, 1)),   # 161 feathers: barbs, grey to white
        stones_pattern(["#24160b", "#3e2713", "#563619", "#6f4721", "#7f5429", "#9a6a37", "#b8874c", "#cfa86c"], 162, n=48),   # 162 pine cones: scales
        stripes(["#6e665a", "#8f877a", "#a39b8c", "#b5ad9d", "#c4bcac", "#d3ccbd", "#e2dccf", "#5b5348"], 163, "rows", (1, 3)),   # 163 driftwood: bleached grey grain
        stripes(["#5a4630", "#7d6548", "#a08a6a", "#bba786", "#cfbe9f", "#e0d3b8", "#efe6d2", "#3e2f1f"], 164, "cols", (1, 2)),   # 164 antlers: tan to tip-white
        stripes(["#4a2412", "#7a3c1e", "#94502a", "#a65f33", "#b86e3c", "#c98250", "#dda070", "#3a1c0e"], 165, "rows", (1, 3), 0.03),   # 165 clay pots: fired terracotta in throwing rings
        cracked(["#3a1c0e", "#6a3418", "#80452a", "#8f5232", "#9e6040", "#b07656", "#c79070", "#1e0f08"], 166),   # 166 cracked pots: older, darker, crazed
        stones_pattern(["#2a0f3e", "#4b1d6b", "#6a2c93", "#8a44b8", "#7a5ad0", "#a979e0", "#d5b6f6", "#f5ecff"], 167, n=20),   # 167 cave crystals: violet facets
    ]
    t += [
        stones_pattern(["#3b3220", "#5e5030", "#8a7a44", "#b8a258", "#c9b467", "#e2cf7c", "#f6ebae", "#6e6f6c"], 168, n=36),   # 168 pyrite: brassy cubes in grey
        palette([("#9e3a24", 5), ("#8a2f1c", 4), ("#b4482c", 3), ("#6e2414", 2), ("#c46a48", 1)], 169),   # 169 red ochre: earthy haematite red
        palette([("#c4902e", 5), ("#b07e26", 4), ("#d6a440", 3), ("#946620", 2), ("#e2bc62", 1)], 170),   # 170 yellow ochre: goethite yellow
    ]
    assert len(t) == 35, len(t)
    return t
