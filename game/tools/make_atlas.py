"""Generate the placeholder 16x16-tile texture atlas (4x4 tiles = 64x64 px).

Tiles (index = row*4 + col):
 0 stone  1 dirt   2 grass_top  3 grass_side
 4 sand   5 water  6 wood       7 lava
 8 ash    9 oil   10 steam     11 smoke
12 fire  13 bedrock 14 snow 15 gravel 16-19 ores (patterned, ADR 0064) 20 leaves 21 sandstone 22 clay
23 tall_grass 24 flower 25 cactus 26 dead_bush 27 ice 28 flower_red
29-31 plant colours 32 crafting_table top 33 crafting_table side 34 furnace top
35 furnace front 36 furnace front lit 37 chest top 38 chest side 39 planks 40-50 detail, sludge, sulfur, obsidian
51 wool 52 farmland 53 farmland moist 54 ripe wheat 55 carrot
56 oak log top 57-59 spruce log side/top, needles 60-62 acacia 63-65 jungle
66-68 terracotta orange/red/cream 69 basalt top 70 basalt side 71 packed ice
72 fern frond; 73 birch bark 74 birch top 75 birch leaves 76 blueberries
77 pumpkin side 78 pumpkin top 79-81 petals blue / white / purple; 82 bone 83 raw meat; 84 leaf litter; 85 oak bark; 86 pebbles 87 flint
88 thatch top 89 thatch side 90 daub 91 wattle withy 92 torch ember (ADR 0030); 93 twigs 94-96 copper, tin, iron-bearing stones; 97 twig bark; 98 cobblestone 99 bricks (patterned, ADR 0039); 100 blueberry bush; 101 kelp 102 seagrass 103 seashell (ADR 0047);
104 crystal (geodes, ADR 0052), 105-108 reserved (world generation), 109-111 savanna grass, tall grass, red earth; 112-114 wheat stalks green / turning / golden, 115 wheat ear,
116 carrot leaves 117 carrot shoulder, 118 torch stick 119 torch wrap 120 torch flame (ADR 0053); 121-135 reserved (parts C, D)
136-170 rock kinds, cobbles, bricks, clutter, pyrite and ochres (tools/atlas_rocks.py, ADR 0057)
171 tnt side 172 tnt top (patterned: a 4-voxel period, one 0.5 m TNT block per period; ADR 0065)
173 monolith ashlar 174 monolith runes 175 monolith seam (patterned, 16-voxel period; runes and seam glow, ADR 0098)
176 spruce 177 acacia 178 jungle 179 birch planks (patterned like 39 oak planks); 180-191 other branches
192 dripstone 193 cave vine 194 fungus stem 195 rusted rail 204 glow berries 205 magma rock 206 fungus cap (cave dressing; 204..207 glow)
Run: python game/tools/make_atlas.py  -> game/assets/textures/atlas.png
"""
import math
import os
import random

from PIL import Image

import atlas_rocks

TILE = 16
COLS = 8
ROWS = 23  # 184 tiles; rows grow at the bottom so tile indices keep their meaning

random.seed(1337)


def noise_tile(base, spread, speckle=None, speckle_chance=0.0, stripes=False):
    img = Image.new("RGBA", (TILE, TILE))
    px = img.load()
    for y in range(TILE):
        for x in range(TILE):
            r, g, b = base
            d = random.randint(-spread, spread)
            if stripes:
                d += -spread if (y % 4 == 0) else spread // 3
            if speckle and random.random() < speckle_chance:
                r, g, b = speckle
            px[x, y] = (max(0, min(255, r + d)), max(0, min(255, g + d)), max(0, min(255, b + d)), 255)
    return img


def thatch(top):
    # dry grass bundles: top = straws lying diagonally, side = overlapping rows with dark gaps
    img = noise_tile((200, 168, 78), 14, (168, 136, 56), 0.2)
    px = img.load()
    for y in range(TILE):
        for x in range(TILE):
            dark = ((x + y) % 4 == 0) if top else (y % 4 == 3 or (y % 4 == 2 and (x + y // 4) % 3 == 0))
            if dark:
                r, g, b, a = px[x, y]
                px[x, y] = (int(r * 0.72), int(g * 0.70), int(b * 0.62), 255)
    return img


def daub():
    # clay, straw and dirt plaster: a warm grey-tan with straw flecks and trowel marks
    img = noise_tile((178, 158, 128), 8, (206, 184, 112), 0.12)
    px = img.load()
    for x in range(TILE):
        y = (x * 3 // 4 + 5) % TILE
        r, g, b, a = px[x, y]
        px[x, y] = (r - 18, g - 18, b - 16, 255)
    return img


def grass_side():
    img = noise_tile((120, 86, 56), 14)
    px = img.load()
    for y in range(4):
        for x in range(TILE):
            if y < 3 or random.random() < 0.5:
                d = random.randint(-12, 12)
                px[x, y] = (max(0, 70 + d), max(0, 150 + d), max(0, 60 + d), 255)
    return img


def sandstone():
    img = noise_tile((214, 190, 130), 8)
    px = img.load()
    for y in range(TILE):
        if y % 5 == 0:
            for x in range(TILE):
                r, g, b, _ = px[x, y]
                px[x, y] = (r - 22, g - 22, b - 18, 255)
    return img


def cactus():
    img = noise_tile((60, 130, 50), 8)
    px = img.load()
    for y in range(TILE):
        for x in range(TILE):
            if x % 4 == 1:
                r, g, b, _ = px[x, y]
                px[x, y] = (r - 20, g - 25, b - 15, 255)
    return img


def cross_tile(base, blades=False, flower=None, sparse=False):
    """Cutout tile: dense 2-px grass blades (Minecraft-like), optional flower
    head. Transparent pixels keep the base colour so mipmaps do not darken."""
    img = Image.new("RGBA", (TILE, TILE), (base[0], base[1], base[2], 0))
    px = img.load()
    # Dense: at distance (no mipmaps) a tuft must still read as a green blob,
    # not as a few hairlines.
    nblades = 6 if sparse else 16
    for i in range(nblades):
        x = random.randint(0, TILE - 4)
        h = random.randint(9, 16) if blades else random.randint(6, 10)
        lean = random.choice([-1, 0, 0, 1])
        for k in range(h):
            y = TILE - 1 - k
            xx = max(0, min(TILE - 3, x + (lean if k > h * 2 // 3 else 0)))
            w = 3 if k < h - 4 else (2 if k < h - 2 else 1)
            for dx in range(w):
                d = random.randint(-18, 18)
                px[xx + dx, y] = (max(0, base[0] + d), max(0, base[1] + d), max(0, base[2] + d), 255)
    if flower:
        cx, cy = TILE // 2, 4
        for k in range(cy + 1, TILE):
            px[cx, k] = (base[0] - 15, base[1] - 10, base[2] - 15, 255)
            px[cx + 1, k] = (base[0] - 25, base[1] - 20, base[2] - 25, 255)
        for dy in range(-3, 4):
            for dx in range(-3, 4):
                if abs(dx) + abs(dy) <= 3 and 0 <= cx + dx < TILE and 0 <= cy + dy < TILE:
                    c = (255, 240, 120) if abs(dx) + abs(dy) <= 1 else flower
                    px[cx + dx, cy + dy] = (c[0], c[1], c[2], 255)
    return img


def planks(base=(176, 136, 82)):
    """Four horizontal boards with dark seams and staggered butt joints."""
    img = noise_tile(base, 7)
    px = img.load()
    for y in range(TILE):
        for x in range(TILE):
            r, g, b, _ = px[x, y]
            if y % 4 == 3:
                px[x, y] = (r - 45, g - 40, b - 30, 255)
            elif x == (5 if (y // 4) % 2 == 0 else 12):
                px[x, y] = (r - 35, g - 30, b - 22, 255)
            elif y % 4 == 0:   # lit top edge of each board (ADR 0129)
                px[x, y] = (r + 16, g + 13, b + 8, 255)
            elif y % 4 == 2 or x == (4 if (y // 4) % 2 == 0 else 11):   # shaded lower edge, end before the joint
                px[x, y] = (r - 12, g - 11, b - 8, 255)
    return img


def frame(img, colour, width=1):
    px = img.load()
    for y in range(TILE):
        for x in range(TILE):
            if x < width or y < width or x >= TILE - width or y >= TILE - width:
                px[x, y] = colour + (255,)
    return img


def crafting_table_top():
    img = planks((168, 120, 70))
    px = img.load()
    # 3x3 grid lines like the vanilla table top
    for i in range(TILE):
        for k in (1, 5, 10, 14):
            px[k, i] = (92, 60, 32, 255)
            px[i, k] = (92, 60, 32, 255)
    return frame(img, (110, 74, 40))


def crafting_table_side():
    img = planks((150, 104, 58))
    px = img.load()
    # top band and two hanging tools (saw blade + hammer head)
    for x in range(TILE):
        for y in range(3):
            px[x, y] = (120, 84, 46, 255)
    for y in range(5, 12):
        px[3, y] = (90, 60, 32, 255)
        px[4, y] = (180, 180, 186, 255)
        px[5, y] = (200, 200, 206, 255) if y % 2 else (150, 150, 156, 255)
    for x in range(9, 14):
        px[x, 5] = (110, 110, 116, 255)
        px[x, 6] = (90, 90, 96, 255)
    for y in range(7, 12):
        px[11, y] = (110, 74, 40, 255)
    return img


def furnace_top():
    img = noise_tile((118, 118, 122), 12, (90, 90, 94), 0.15)
    return frame(img, (80, 80, 84))


def furnace_front(lit):
    img = noise_tile((118, 118, 122), 12, (90, 90, 94), 0.15)
    px = img.load()
    for y in range(8, 14):
        for x in range(4, 12):
            if lit:
                hot = random.random()
                px[x, y] = (255, 120 + int(110 * hot) if y > 10 else 90 + int(80 * hot), 30, 255)
            else:
                px[x, y] = (24, 22, 22, 255)
    for x in range(3, 13):
        px[x, 7] = (70, 70, 74, 255)
        px[x, 14] = (70, 70, 74, 255)
    for x in range(4, 12, 2):  # vent slits
        px[x, 3] = (40, 40, 44, 255)
        px[x, 4] = (40, 40, 44, 255)
    return frame(img, (80, 80, 84))


def chest_tile(top):
    img = planks((160, 110, 52))
    frame(img, (88, 58, 26))
    if not top:
        px = img.load()
        for y in range(TILE):
            px[0, y] = px[TILE - 1, y] = (88, 58, 26, 255)
        for x in range(TILE):
            px[x, 5] = (88, 58, 26, 255)  # lid seam
        for y in range(4, 9):  # iron latch
            for x in range(7, 9):
                px[x, y] = (200, 200, 205, 255) if y < 8 else (120, 120, 126, 255)
    return img


def farmland(base, furrow):
    """Tilled soil seen from above: dark furrows every 4 px across the tile."""
    img = noise_tile(base, 10, furrow, 0.08)
    px = img.load()
    for y in range(TILE):
        if y % 4 == 3:
            for x in range(TILE):
                d = random.randint(-6, 6)
                px[x, y] = (max(0, furrow[0] + d), max(0, furrow[1] + d), max(0, furrow[2] + d), 255)
    return img


def wool():
    """White fleece: soft noise with curly darker loops."""
    img = noise_tile((234, 234, 228), 6, (212, 212, 204), 0.12)
    px = img.load()
    for y in range(0, TILE, 4):
        for x in range((y // 4) % 2 * 2, TILE, 4):
            for (dx, dy) in [(0, 0), (1, 0), (0, 1)]:
                r, g, b, _ = px[(x + dx) % TILE, (y + dy) % TILE]
                px[(x + dx) % TILE, (y + dy) % TILE] = (r - 18, g - 18, b - 20, 255)
    return img


def bark(base, groove):
    """Log side: vertical bark ridges with dark grooves that wander a little."""
    img = noise_tile(base, 10)
    px = img.load()
    for x0 in range(1, TILE, 4):
        x = x0
        for y in range(TILE):
            if random.random() < 0.2:
                x = max(0, min(TILE - 1, x + random.choice((-1, 1))))
            d = random.randint(-6, 6)
            px[x, y] = (max(0, groove[0] + d), max(0, groove[1] + d), max(0, groove[2] + d), 255)
    return img


def log_top(core, ring, rim):
    """Log end grain: growth rings around the pith, a bark rim."""
    img = noise_tile(core, 6)
    px = img.load()
    c = (TILE - 1) / 2.0
    for y in range(TILE):
        for x in range(TILE):
            d = max(abs(x - c), abs(y - c)) * 0.7 + ((x - c) ** 2 + (y - c) ** 2) ** 0.5 * 0.3
            if d >= 7.0:
                col = rim
            elif int(d) % 2 == 1:
                col = ring
            else:
                continue
            k = random.randint(-6, 6)
            px[x, y] = (max(0, col[0] + k), max(0, col[1] + k), max(0, col[2] + k), 255)
    return img


def terracotta(base, band):
    """Badlands strata: hardened clay with a faint lighter band."""
    img = noise_tile(base, 6, band, 0.08)
    px = img.load()
    for x in range(TILE):
        for y in (5, 6):
            r, g, b, _ = px[x, y]
            px[x, y] = (min(255, r + 12), min(255, g + 10), min(255, b + 8), 255)
    return img


def basalt_side():
    """Columnar basalt: dark grey with vertical joints."""
    img = noise_tile((62, 62, 68), 8, (44, 44, 50), 0.12)
    px = img.load()
    for x in (0, 5, 11):
        for y in range(TILE):
            px[x, y] = (34, 34, 38, 255)
    return img


def basalt_top():
    """Basalt column tops: polygon cells with dark joints."""
    img = noise_tile((70, 70, 76), 8, (52, 52, 58), 0.12)
    px = img.load()
    for i in range(TILE):
        px[i, 0] = px[i, 8] = (34, 34, 38, 255)
        px[5 if i < 8 else 11, i] = (34, 34, 38, 255)
    return img


def birch_bark():
    """White bark with short black horizontal marks (lenticels)."""
    img = noise_tile((222, 220, 210), 6, (200, 198, 188), 0.1)
    px = img.load()
    for _ in range(9):
        x0 = random.randint(0, TILE - 4)
        y = random.randint(0, TILE - 1)
        for x in range(x0, x0 + random.randint(2, 4)):
            px[x, y] = (40, 38, 36, 255)
    return img


def pumpkin_side():
    """Orange ribs with darker grooves."""
    img = noise_tile((222, 128, 30), 8, (200, 108, 22), 0.1)
    px = img.load()
    for x in (0, 4, 8, 12):
        for y in range(TILE):
            r, g, b, _ = px[x, y]
            px[x, y] = (r - 50, g - 40, b - 10, 255)
    return img


def pumpkin_top():
    """Ribs meeting at a green-brown stem."""
    img = pumpkin_side()
    px = img.load()
    for y in range(6, 10):
        for x in range(6, 10):
            px[x, y] = (96, 110, 40, 255) if (x + y) % 3 else (70, 80, 30, 255)
    return img


def oak_bark():
    """Grey-brown bark in vertical plates with dark grooves. Own RNG (see leaf_litter)."""
    noise_tile((128, 128, 128), 0)
    rng = random.Random(85)
    img = Image.new("RGBA", (TILE, TILE))
    px = img.load()
    plates = [(112, 92, 70), (98, 80, 60), (124, 104, 80), (88, 72, 56)]
    for x in range(TILE):
        groove = rng.random() < 0.25
        base = (58, 46, 36) if groove else rng.choice(plates)
        for y in range(TILE):
            v = rng.randint(-9, 9)
            c = base if not groove or rng.random() < 0.8 else rng.choice(plates)
            px[x, y] = (max(0, c[0] + v), max(0, c[1] + v), max(0, c[2] + v), 255)
    return img


def leaf_litter():
    """Fallen leaves: browns, rust and a little olive, in leaf-sized splotches. Own RNG, so adding
    it does not reshuffle the tiles after it (the free tile it replaced still draws its numbers)."""
    noise_tile((128, 128, 128), 0)
    rng = random.Random(84)
    cols = [(118, 80, 46), (150, 96, 44), (176, 112, 48), (92, 64, 38), (124, 112, 54), (190, 146, 70)]
    weights = [5, 4, 3, 3, 2, 1]
    img = Image.new("RGBA", (TILE, TILE))
    px = img.load()
    for y in range(TILE):
        for x in range(TILE):
            px[x, y] = (96, 70, 42, 255)
    for _ in range(60):
        c = rng.choices(cols, weights)[0]
        x0, y0 = rng.randrange(TILE), rng.randrange(TILE)
        for dx, dy in ((0, 0), (1, 0), (0, 1), (1, 1))[: rng.randint(1, 4)]:
            v = rng.randint(-10, 10)
            px[(x0 + dx) % TILE, (y0 + dy) % TILE] = (max(0, min(255, c[0] + v)), max(0, min(255, c[1] + v)), max(0, min(255, c[2] + v)), 255)
    return img


def palette(entries, seed):
    """A terrain palette (ADR 0036): every terrain voxel shows one texel of its material's tile,
    picked by a hash of its position, so a tile is a palette whose texel counts are the weights.
    entries = [("#rrggbb", weight), ...]; the 256 texels hold the colours in those proportions,
    shuffled with the tile's own RNG (no pattern: patterns mean nothing at one texel per voxel)."""
    rng = random.Random(seed)
    total = sum(w for _, w in entries)
    cols = []
    for hexc, w in entries:
        c = tuple(int(hexc[i:i + 2], 16) for i in (1, 3, 5))
        cols += [c] * round(256 * w / total)
    while len(cols) < 256:
        cols.append(cols[-1])
    cols = cols[:256]
    rng.shuffle(cols)
    img = Image.new("RGBA", (TILE, TILE))
    px = img.load()
    for i, c in enumerate(cols):
        px[i % TILE, i // TILE] = c + (255,)
    return img


# Terrain palettes: natural, a little warm and soft, a few close shades per material so ground,
# rock and foliage read as surfaces of many voxels (Lay of the Land) instead of noisy textures.
STONE = [("#8a8a87", 5), ("#7c7d7c", 4), ("#979590", 3), ("#6d6e6f", 2), ("#a5a198", 1)]
PAL = {
    "stone": STONE,
    "dirt": [("#7a5638", 5), ("#6b4a30", 4), ("#8a6442", 3), ("#5c3f28", 2), ("#96714b", 1)],
    "grass": [("#5f9a3c", 5), ("#6ea844", 4), ("#548c35", 4), ("#7fb24c", 2), ("#4a7d2f", 2), ("#8fbb57", 1)],
    "sand": [("#d8c48e", 5), ("#cdb67f", 4), ("#e2d09f", 3), ("#c2a970", 2)],
    "wood": [("#a8814f", 4), ("#9a7446", 4), ("#b58d5a", 3), ("#8a653d", 2)],
    "ash": [("#9a9790", 5), ("#8a8780", 4), ("#aaa79f", 3), ("#77746e", 2)],
    "bedrock": [("#3a3a3e", 5), ("#2e2e32", 4), ("#4a4a4f", 3), ("#25252a", 2)],
    "snow": [("#eef2f7", 6), ("#e2e8f0", 4), ("#f8fafc", 3), ("#d6dde8", 1)],
    "gravel": [("#8c8781", 4), ("#7a7570", 4), ("#9d9890", 3), ("#6b6661", 2), ("#a89f92", 1)],
    "coal": [("#2f2f31", 6), ("#3e3e41", 4), ("#4d4b48", 2), ("#8a8a87", 2)],
    "iron": [("#b9825e", 5), ("#a06a4a", 4), ("#c99a74", 3), ("#8a8a87", 2)],
    "copper": [("#c8743f", 5), ("#b0602f", 4), ("#d98c55", 2), ("#4f9a7e", 2), ("#8a8a87", 2)],
    "tin": [("#c9ccd2", 5), ("#b0b4bb", 4), ("#dde0e5", 2), ("#8f949c", 3)],
    "leaves": [("#3f7a2e", 5), ("#4c8a34", 4), ("#356a28", 4), ("#5a9a3c", 3), ("#2c5a22", 2), ("#6aa846", 1)],
    "sandstone": [("#d4bd85", 5), ("#c8ae74", 4), ("#ddc994", 3), ("#b89c64", 2)],
    "clay": [("#9aa2ad", 5), ("#8c95a1", 4), ("#a8afb8", 3), ("#7f8894", 2)],
    "tall_grass": [("#7cbc4f", 5), ("#8dc75a", 4), ("#6aa842", 4), ("#9ed06a", 2)],
    "plant_green": [("#5e9c44", 5), ("#6aa94c", 4), ("#4f8a3a", 3)],
    "dead_bush": [("#8d6a40", 5), ("#7a5a35", 4), ("#a07c4c", 3)],
    "moss": [("#4f7d32", 5), ("#5d8f3a", 4), ("#426b2a", 3), ("#6a9c44", 1)],
    "sulfur": [("#d6c03a", 5), ("#c4aa2c", 4), ("#e6d35a", 3), ("#a88f22", 2)],
    "obsidian": [("#221a30", 5), ("#2e2440", 4), ("#3a2e52", 2), ("#18121f", 2)],
    "spruce_needles": [("#2f5e45", 5), ("#264f3a", 4), ("#3a6d50", 3), ("#1f4230", 2)],
    "acacia_leaves": [("#7c8f3a", 5), ("#6c7f30", 4), ("#8ea046", 3), ("#5d6f28", 2)],
    "jungle_leaves": [("#3d9132", 5), ("#2f7a28", 4), ("#4ea33c", 3), ("#256622", 2)],
    "birch_leaves": [("#86ac4c", 5), ("#96bb58", 4), ("#75993f", 3), ("#a6c565", 2)],
    "fern": [("#4f9a3e", 5), ("#5daa48", 4), ("#3f8432", 3)],
    "berry_bush": [("#3d7430", 5), ("#4a8538", 4), ("#2f5f26", 3), ("#58943f", 2), ("#3b4fb4", 2), ("#2a3a8e", 1)],
    "basalt": [("#4a4a50", 5), ("#3e3e44", 4), ("#56565c", 3), ("#333338", 2)],
    "packed_ice": [("#8cb0de", 5), ("#9cbce4", 4), ("#b0ccec", 3)],
    "ice": [("#aacdf0", 5), ("#bcd8f4", 4), ("#9cc0e6", 3), ("#d2e6fa", 1)],
}


def grade(img, contrast=1.22, shift=14.0, sat=1.14):
    """ADR 0129: hue-shifted ramps for every generated tile. Each texel moves away from the tile's mean
    brightness, darker texels turn cooler (blue-violet) and lighter ones warmer (yellow), and saturation
    lifts a little, so close shades separate and materials of one family stop reading as one grey."""
    px = img.load()
    lum = lambda r, g, b: 0.299 * r + 0.587 * g + 0.114 * b
    ops = [px[x, y] for y in range(TILE) for x in range(TILE) if px[x, y][3]]
    if not ops:
        return img
    mean = sum(lum(r, g, b) for r, g, b, _ in ops) / len(ops)
    out = img.copy()
    po = out.load()
    for y in range(TILE):
        for x in range(TILE):
            r, g, b, a = px[x, y]
            if not a:
                continue
            l = lum(r, g, b)
            d = (mean + (l - mean) * contrast) - l
            t = max(-1.0, min(1.0, (l - mean) / 48.0))
            c = [l + (v - l) * sat + d for v in (r, g, b)]
            c = [c[0] + shift * t * 0.7, c[1] + shift * t * 0.25, c[2] - shift * t]
            po[x, y] = tuple(max(0, min(255, int(round(v)))) for v in c) + (a,)
    return out


def keep_rng(tile, *noise_args):
    """A tile that replaced a noise_tile: draws that tile's numbers from the shared RNG (same
    arguments as the old tile) after building, so the noise tiles after it keep their look."""
    noise_tile(*(noise_args or ((128, 128, 128), 0)))
    return tile


def on_free_tile(entries, seed):
    """A palette on a tile that used to be a free noise_tile: still draws that tile's numbers from
    the shared RNG, so the noise tiles after it keep their look."""
    noise_tile((128, 128, 128), 0)
    return palette(entries, seed)


# Ground pickups (ADR 0019): one voxel each on ground of nearly the same colour, so each gets a
# palette that stands out from stone, gravel, grass and leaf litter.
PICKUP = {
    "pebbles": [("#cfc8b8", 5), ("#bfb7a5", 4), ("#ddd7c9", 3), ("#aca390", 2)],   # pale river stones
    "flint": [("#2a2f3b", 5), ("#373d4b", 4), ("#1f232c", 3), ("#e6e0d0", 2)],     # blue-black, chalky rind
    "twigs": [("#c9a46c", 5), ("#b8925a", 4), ("#d8b781", 3), ("#9e7c4c", 2)],     # stick tops: dry, pale wood
    "twigs_bark": [("#6e4a2a", 5), ("#5a3b21", 4), ("#7d5631", 3), ("#4a301b", 2)],  # stick sides: dark bark
}
# Ore-bearing stones (Vintage Story's ore chunks): grey host rock, about a quarter of it saturated ore voxels.
# (A pale host read as quartz.)
HOST = [("#8e8a83", 5), ("#7c7872", 4), ("#9c978e", 3), ("#6a6661", 2)]
PICKUP["copper_bearing"] = HOST + [("#d4692a", 2), ("#b0471f", 1.5), ("#3f9a6a", 1)]   # native copper, malachite
PICKUP["tin_bearing"] = HOST + [("#3a2a1f", 2), ("#58402f", 1.5), ("#c8c2b0", 1)]  # cassiterite, a little quartz
PICKUP["iron_bearing"] = HOST + [("#a8402a", 2), ("#c9662f", 1.5), ("#6e2e1c", 1)]     # hematite and limonite


def _hexc(h):
    return tuple(int(h[i:i + 2], 16) for i in (1, 3, 5)) + (255,)


G = TILE   # pickup patterns: one texel per voxel ("pattern": true lays the tile over the world grid, ADR 0039)


def _cells_to_tile(cells):
    img = Image.new("RGBA", (TILE, TILE))
    px = img.load()
    for y in range(G):
        for x in range(G):
            px[x, y] = cells[y][x]
    return img


def stones_pattern(cols, seed, n=40):
    """Pickup stones (2026-10-25: "more defined, colours in patterns, more contrast", up to 8):
    rounded stones 2-4 cells across lit from the top-left (rim, body, shaded side), each stone one of
    two body tones, dark gaps between them, a rare bright speck. cols = 8 colours: gap, deep shade,
    shade, body A, body B, light, rim, speck. Wraps."""
    rng = random.Random(seed)
    seeds = [(rng.uniform(0, G), rng.uniform(0, G), rng.randint(0, 1)) for _ in range(n)]
    gap, deep, shade, body_a, body_b, light, rim, speck = [_hexc(c) for c in cols]
    cells = [[gap] * G for _ in range(G)]
    for y in range(G):
        for x in range(G):
            d = []
            for sx, sy, tone in seeds:
                dx = min(abs(x + 0.5 - sx), G - abs(x + 0.5 - sx))
                dy = min(abs(y + 0.5 - sy), G - abs(y + 0.5 - sy))
                d.append((dx * dx + dy * dy, sx, sy, tone))
            d.sort()
            (d0, sx, sy, tone), d1 = d[0], d[1][0]
            if d1 - d0 < 1.0:
                continue   # the gap between two stones
            ox, oy = (x + 0.5 - sx), (y + 0.5 - sy)
            ox -= G * round(ox / G)
            oy -= G * round(oy / G)
            lit = -(ox + oy)
            if lit > 1.2:
                c = rim
            elif lit > 0.4:
                c = light
            elif lit < -1.2:
                c = deep
            elif lit < -0.4:
                c = shade
            else:
                c = body_a if tone == 0 else body_b
            if rng.random() < 0.05:
                c = speck
            cells[y][x] = c
    return _cells_to_tile(cells)


def flint_pattern(seed):
    """Flint: blue-black glassy stone with conchoidal ripple bands, blue gloss flecks and only small
    chalky cortex patches, so a shard of a few voxels reads dark and glassy, never like the pale
    pebbles (2026-10-26). 8 colours, strong contrast. Wraps."""
    rng = random.Random(seed)
    rind, rind_dark, core, core_mid, band, gloss, shine, deep = [_hexc(c) for c in
        ("#e6dfcc", "#b3a88e", "#1b1f28", "#262b38", "#39425a", "#6f7fa6", "#a9b8dc", "#0d0f14")]
    # ripples: rings around a few strike points (conchoidal fracture)
    strikes = [(rng.uniform(0, G), rng.uniform(0, G), rng.uniform(1.6, 2.4)) for _ in range(5)]
    cortex = [(rng.uniform(0, G), rng.uniform(0, G), rng.uniform(1.0, 1.6)) for _ in range(3)]
    def wrap(d):
        return d - G * round(d / G)
    cells = [[core] * G for _ in range(G)]
    for y in range(G):
        for x in range(G):
            r = min(((wrap(x + 0.5 - cx) ** 2 + wrap(y + 0.5 - cy) ** 2) ** 0.5 / sp for cx, cy, sp in strikes))
            c = min(((wrap(x + 0.5 - cx) ** 2 + wrap(y + 0.5 - cy) ** 2) ** 0.5 / cr for cx, cy, cr in cortex))
            q = rng.random()
            if c < 0.8:
                cells[y][x] = rind if q < 0.7 else rind_dark
            elif c < 1.0:
                cells[y][x] = rind_dark
            elif q < 0.05:
                cells[y][x] = shine
            elif q < 0.13:
                cells[y][x] = gloss
            elif (r % 1.0) < 0.22:
                cells[y][x] = band
            elif q < 0.30:
                cells[y][x] = deep
            else:
                cells[y][x] = core if q < 0.75 else core_mid
    return _cells_to_tile(cells)


def wood_grain(cols, seed, across=False):
    """Twig wood: grain stripes along the stick (rows), a knot. cols: 8, dark first: [0] knot or
    furrow, [1..5] grain shades dark to light, [6] highlight, [7] bark: lichen / wood: dark fleck."""
    rng = random.Random(seed)
    c = [_hexc(h) for h in cols]
    # diagonal bands 1-2 voxels wide: a stick lying along x, z or a diagonal crosses several
    band = []
    while len(band) < 2 * G:
        band += [rng.choice([2, 3, 3, 4, 5])] * rng.randint(1, 2)
    cells = []
    for y in range(G):
        row = []
        for x in range(G):
            v = band[(x + y) % (2 * G)] if not across else band[(x + 2 * y) % (2 * G)]
            r = rng.random()
            if across and r < 0.16:
                v = 0 if rng.random() < 0.5 else 1    # a furrow
            elif r < 0.07:
                v = 7                                 # lichen on bark, a dark fleck on wood
            elif r < 0.14:
                v = 6                                 # a highlight
            row.append(c[v])
        cells.append(row)
    for _ in range(3):
        kx, ky = rng.randrange(G), rng.randrange(G)
        cells[ky][kx] = c[0]
        cells[ky][(kx + 1) % G] = c[1]
    return _cells_to_tile(cells)


def ore_stones_pattern(ore_cols, seed):
    """Ore-bearing stones: the stones pattern in grey host rock, some stones veined with the ore."""
    gap = "#3e3c38"
    img = stones_pattern([gap, "#57534d", "#6f6b64", "#8e8a83", "#9a958b", "#b2ada2", "#cfc9bd", "#e4dfd3"], seed)
    rng = random.Random(seed + 1)
    px = img.load()
    for _ in range(12):
        cx, cy = rng.randrange(G), rng.randrange(G)
        for k in range(rng.randint(2, 4)):
            x, y = (cx + rng.randint(-1, 1)) % G, (cy + rng.randint(-1, 1)) % G
            if px[x, y][:3] != _hexc(gap)[:3]:
                px[x, y] = _hexc(ore_cols[k % len(ore_cols)])
    return img


def ore_vein_pattern(cols, seed, n=9, veinlet=1):
    """Ore in a vein (ADR 0064, "pattern": true): one 16x16 picture laid over the world grid, so a lump of
    ore voxels shows one continuous texture instead of a texel per voxel picked at random. Crystals 4-8
    cells long, stretched along a diagonal like grains in a vein, each lit from the top-left (a bevel of
    light and shade round a flat body) with a thin dark matrix between them; `veinlet` thin wandering veins of the accent mineral cross the
    tile; a few glints. cols = 8 colours: matrix, deep shade, shade, body A, body B, light, rim, accent.
    Wraps."""
    rng = random.Random(seed)
    matrix, deep, shade, body_a, body_b, light, rim, accent = [_hexc(c) for c in cols]
    seeds = [(rng.uniform(0, G), rng.uniform(0, G), rng.randint(0, 2)) for _ in range(n)]
    ax, ay = 0.8, -0.6   # the grain's long axis (unit), a diagonal

    def dist(x, y, sx, sy):
        dx = (x + 0.5 - sx + G / 2) % G - G / 2
        dy = (y + 0.5 - sy + G / 2) % G - G / 2
        along = dx * ax + dy * ay
        across = -dx * ay + dy * ax
        return along * along * 0.35 + across * across, dx, dy

    cells = [[matrix] * G for _ in range(G)]
    for y in range(G):
        for x in range(G):
            d = sorted((dist(x, y, sx, sy) + (tone,)) for sx, sy, tone in seeds)
            (d0, dx, dy, tone), d1 = d[0], d[1][0]
            edge = d1 ** 0.5 - d0 ** 0.5
            if edge < 0.3:
                continue   # the thin matrix between crystals
            lit = -(dx + dy)   # toward the top-left: lit
            if edge < 1.2:
                c = (rim if lit > 2.5 else light) if lit > 0 else (deep if lit < -2.5 else shade)   # the crystal's bevel
            else:
                c = [body_a, body_b, body_a][tone]
            cells[y][x] = c
    for _ in range(veinlet):
        y0, ph, amp = rng.randrange(G), rng.uniform(0, 6.283), rng.choice([1.5, 2.5])
        for x in range(G):
            y = int(round(y0 + amp * math.sin(ph + x * 6.283 / G) + 0.6 * math.sin(2 * ph + x * 6.283 * 2 / G))) % G
            cells[y][x] = accent
            if rng.random() < 0.3:
                cells[(y + 1) % G][x] = accent
    for _ in range(3):
        cells[rng.randrange(G)][rng.randrange(G)] = rim
    return _cells_to_tile(cells)


def stripes(cols, seed, direction="rows", width=(1, 2), fleck=0.06):
    """Clutter in bands (2026-10-25, the pickups' treatment for every clutter kind): 8 colours, dark
    first; bands 1-2 voxels wide of shades 1..6 along rows (kelp segments), columns (stems, blades) or
    diagonals (shell ribs), with the darkest and the lightest as flecks. Laid over the world grid."""
    rng = random.Random(seed)
    c = [_hexc(h) for h in cols]
    band = []
    while len(band) < 2 * G:
        band += [rng.randint(1, 6)] * rng.randint(*width)
    cells = []
    for y in range(G):
        row = []
        for x in range(G):
            i = {"rows": y, "cols": x, "diag": x + y}[direction]
            v = band[i % (2 * G)]
            r = rng.random()
            if r < fleck:
                v = 0
            elif r < 2 * fleck:
                v = 7
            row.append(c[v])
        cells.append(row)
    return _cells_to_tile(cells)


def crystal(seed):
    """Geode crystal: violet facets, each a diagonal band of one shade with a bright edge, a few
    near-white glints; the terrain shader makes it glow."""
    rng = random.Random(seed)
    c = [_hexc(h) for h in ["#2a1248", "#43206e", "#5c2f94", "#7a44b8", "#9a62d6", "#b98bea", "#d7b6f6", "#f4e8ff"]]
    cells = []
    for y in range(G):
        row = []
        for x in range(G):
            facet = ((x + 2 * y) // 3 + (x - y) // 5) % 5
            v = 1 + facet
            if (x + 2 * y) % 3 == 0:
                v = min(7, v + 1)
            r = rng.random()
            if r < 0.04:
                v = 7
            elif r < 0.08:
                v = 0
            row.append(c[v])
        cells.append(row)
    return _cells_to_tile(cells)


def spotted_cap(cols, spot_cols, seed):
    """A red mushroom cap: three reds shaded in blotches, white spots 1-3 voxels across."""
    img = stones_pattern(cols, seed, n=30)
    rng = random.Random(seed + 7)
    px = img.load()
    for _ in range(9):
        cx, cy = rng.randrange(G), rng.randrange(G)
        for dx, dy in [(0, 0), (1, 0), (0, 1), (1, 1)][:rng.randint(1, 4)]:
            px[(cx + dx) % G, (cy + dy) % G] = _hexc(spot_cols[(dx + dy) % len(spot_cols)])
    return img


def cobblestone():
    """Rounded stones about 4 voxels across with dark gaps; wraps at the tile edge (patterned: the
    tile repeats every 16 voxels in the world)."""
    rng = random.Random(9801)
    seeds = []
    for gy in range(4):
        for gx in range(4):
            seeds.append((gx * 4 + rng.uniform(0.5, 3.5), gy * 4 + rng.uniform(0.5, 3.5), rng.randint(-22, 18)))
    img = Image.new("RGBA", (TILE, TILE))
    px = img.load()
    for y in range(TILE):
        for x in range(TILE):
            ds = []
            for sx, sy, shade in seeds:
                dx = min(abs(x + 0.5 - sx), TILE - abs(x + 0.5 - sx))
                dy = min(abs(y + 0.5 - sy), TILE - abs(y + 0.5 - sy))
                ds.append((dx * dx + dy * dy, shade, sx, sy))
            ds.sort()
            gap = ds[1][0] ** 0.5 - ds[0][0] ** 0.5 < 0.75
            v = 142 + ds[0][1] + rng.randint(-6, 6)
            sx, sy = ds[0][2], ds[0][3]   # lit from the top-left (ADR 0129): rim light, shaded far side
            ox, oy = x + 0.5 - sx, y + 0.5 - sy
            ox -= TILE * round(ox / TILE)
            oy -= TILE * round(oy / TILE)
            v += max(-20, min(18, int(-(ox + oy) * 7)))
            if gap:
                v = 88 + rng.randint(-6, 6)
            px[x, y] = (v + 4, v, v - 8, 255)
    return img


def bricks():
    """Fired-clay bricks in courses: 4 voxels tall (3 brick, 1 mortar), 8 long, every other course
    shifted by half a brick; each brick its own shade. Wraps (patterned)."""
    rng = random.Random(9902)
    img = Image.new("RGBA", (TILE, TILE))
    px = img.load()
    shades = {}
    for y in range(TILE):
        course = y // 4
        for x in range(TILE):
            xs = (x + (4 if course % 2 else 0)) % TILE
            if y % 4 == 3 or xs % 8 == 7:
                v = 178 + rng.randint(-8, 8)
                px[x, y] = (v, v - 6, v - 16, 255)
                continue
            key = (course, xs // 8)
            if key not in shades:
                shades[key] = rng.randint(-20, 16)
            d = shades[key] + rng.randint(-7, 7)
            if y % 4 == 0 or xs % 8 == 0:   # lit top and left edge of each brick (ADR 0129)
                d += 18
            elif y % 4 == 2 or xs % 8 == 6:
                d -= 16
            px[x, y] = (max(0, 162 + d), max(0, 74 + d // 2), max(0, 52 + d // 2), 255)
    return img


def spotted(base, spot):
    img = noise_tile(base, 12, None, 0)
    for (sx, sy) in [(3, 3), (10, 4), (6, 9), (12, 11), (2, 12)]:
        for dy in range(2):
            for dx in range(2):
                img.putpixel((sx + dx, sy + dy), (spot[0], spot[1], spot[2], 255))
    return img


tiles = [
    palette(PAL["stone"], 0),                                 # 0 stone
    palette(PAL["dirt"], 1),                                  # 1 dirt
    palette(PAL["grass"], 2),                                 # 2 grass top
    palette(PAL["grass"], 3),                                 # 3 grass side: a grass voxel is grass all round
    palette(PAL["sand"], 4),                                  # 4 sand
    noise_tile((44, 128, 156), 10, (78, 160, 182), 0.15),     # 5 water: teal (animated, scrolls)
    palette(PAL["wood"], 6),                                  # 6 wood (log heartwood, placed logs)
    noise_tile((240, 110, 30), 16, (255, 200, 60), 0.2),      # 7 lava
    palette(PAL["ash"], 8),                                   # 8 ash
    noise_tile((60, 50, 30), 8, (40, 35, 20), 0.1),           # 9 oil
    noise_tile((225, 228, 235), 8),                           # 10 steam
    noise_tile((80, 80, 84), 10),                             # 11 smoke
    noise_tile((255, 150, 40), 22, (255, 230, 90), 0.3),      # 12 fire
    palette(PAL["bedrock"], 13),                              # 13 bedrock
    palette(PAL["snow"], 14),                                 # 14 snow
    palette(PAL["gravel"], 15),                               # 15 gravel
    # ores (ADR 0064): patterned, one picture across a lump; crystals in a dark matrix, veinlets of a second mineral
    ore_vein_pattern(["#0b0b0d", "#17171b", "#222328", "#2e3036", "#3a3c44", "#5a6070", "#93a0b8", "#c8a94a"], 16),   # 16 coal: glossy black, brassy pyrite
    ore_vein_pattern(["#1e0a08", "#3a1410", "#5a1e16", "#74261b", "#86301f", "#a8482c", "#c8704a", "#9aa2b0"], 17),   # 17 iron: hematite, limonite, specular steel-grey
    ore_vein_pattern(["#2b170c", "#5c2a12", "#8a401c", "#b75a28", "#cf7638", "#e89a58", "#f8c88c", "#2e9a6c"], 18, veinlet=2),   # 18 copper: native copper, malachite green
    ore_vein_pattern(["#1a110b", "#24170f", "#3a2717", "#5a3b22", "#6a4628", "#94632f", "#d09a50", "#e8e2d4"], 19, veinlet=2),   # 19 tin: cassiterite, resin glints, quartz veins
    palette(PAL["leaves"], 20),                               # 20 leaves (oak)
    palette(PAL["sandstone"], 21),                            # 21 sandstone
    palette(PAL["clay"], 22),                                 # 22 clay
    palette(PAL["tall_grass"], 23),                           # 23 tall grass, lighter than the grass it stands on
    palette(PAL["tall_grass"], 24),                           # 24 flower stem
    cactus(),                                                 # 25 cactus
    palette(PAL["dead_bush"], 26),                            # 26 dead bush
    palette(PAL["ice"], 27),                                  # 27 ice
    palette(PAL["tall_grass"], 28),                           # 28 flower_red stem
    palette(PAL["plant_green"], 29),                          # 29 leaf green (stems)
    noise_tile((245, 215, 60), 12, (255, 240, 120), 0.2),     # 30 petal yellow
    noise_tile((220, 55, 55), 12, (250, 110, 110), 0.2),      # 31 petal red
    crafting_table_top(),                                     # 32 crafting table top
    crafting_table_side(),                                    # 33 crafting table side
    furnace_top(),                                            # 34 furnace top / bottom
    furnace_front(False),                                     # 35 furnace front (all four sides until blocks have facing)
    furnace_front(True),                                      # 36 furnace front, lit
    chest_tile(True),                                         # 37 chest top
    chest_tile(False),                                        # 38 chest side with latch
    planks(),                                                 # 39 oak planks
    keep_rng(stones_pattern(["#3a2414", "#5e3d22", "#7a5131", "#94633b", "#a8744a", "#c08a5c", "#d9a979", "#efd7b0"], 40, n=30), (150, 100, 60), 14, (120, 80, 45), 0.2),   # 40 mushroom cap, brown: mottled, a pale rim
    keep_rng(spotted_cap(["#4a0c0c", "#7d1414", "#a51c1c", "#c42828", "#d23232", "#e04848", "#f06a5a", "#ffd9c9"], ["#fbf6ea", "#e9e0cc"], 41), (200, 40, 40), 12, None, 0),   # 41 mushroom cap, red with white spots
    keep_rng(stripes(["#8d8068", "#b9ad94", "#c9bea6", "#d6ccb5", "#e1d8c3", "#ebe4d2", "#f5f0e3", "#a89a7e"], 42, "cols"), (225, 215, 190), 8, (200, 190, 165), 0.15),   # 42 mushroom stem: fibres
    noise_tile((110, 175, 70), 10, (150, 200, 90), 0.2),      # 43 lily pad, lighter than grass so it reads on a puddle
    noise_tile((150, 120, 70), 12, (120, 95, 55), 0.2),       # 44 reed head, brown
    spotted((70, 200, 210), (190, 250, 255)),                 # 45 glow mushroom cap (emissive in the shader)
    palette(PAL["moss"], 46),                                 # 46 moss
    noise_tile((150, 14, 20), 14, (95, 6, 12), 0.22),         # 47 blood (liquid; its stains come from scripts/visuals/stain_map.gd)
    noise_tile((150, 226, 30), 16, (226, 255, 120), 0.2),     # 48 toxic sludge: acid lime with bubbles, see-through like water
    palette(PAL["sulfur"], 49),                               # 49 sulfur crust
    palette(PAL["obsidian"], 50),                             # 50 obsidian
    wool(),                                                   # 51 wool (blocks, carpet, wild carrot flowers)
    farmland((104, 70, 44), (66, 42, 26)),                    # 52 farmland top, dry
    farmland((74, 48, 30), (44, 28, 16)),                     # 53 farmland top, moist (hydrated)
    noise_tile((214, 178, 70), 14, (176, 138, 44), 0.25),     # 54 ripe wheat (crop heads and stalks, wild wheat)
    noise_tile((236, 124, 30), 12, (200, 90, 20), 0.2),       # 55 carrot orange
    # content pass: tree kinds (biomes.json tree_kind), badlands strata, basalt, packed ice, ferns
    log_top((176, 136, 82), (140, 100, 56), (110, 76, 40)),   # 56 oak log top
    bark((70, 50, 32), (40, 28, 18)),                         # 57 spruce log side: dark bark
    log_top((150, 112, 66), (118, 84, 46), (70, 50, 32)),     # 58 spruce log top
    palette(PAL["spruce_needles"], 59),                       # 59 spruce needles: dark blue-green
    bark((112, 106, 96), (78, 72, 64)),                       # 60 acacia log side: grey bark
    log_top((200, 104, 52), (168, 82, 40), (112, 106, 96)),   # 61 acacia log top: orange heartwood
    palette(PAL["acacia_leaves"], 62),                        # 62 acacia leaves: olive
    bark((128, 98, 56), (88, 92, 40)),                        # 63 jungle log side: mossy streaks
    log_top((180, 140, 90), (150, 110, 66), (128, 98, 56)),   # 64 jungle log top
    palette(PAL["jungle_leaves"], 65),                        # 65 jungle leaves: bright
    terracotta((176, 96, 54), (196, 120, 70)),                # 66 terracotta orange
    terracotta((140, 70, 52), (160, 88, 64)),                 # 67 terracotta red-brown
    terracotta((208, 176, 140), (224, 196, 162)),             # 68 terracotta cream
    palette(PAL["basalt"], 69),                               # 69 basalt top
    palette(PAL["basalt"], 70),                               # 70 basalt side
    palette(PAL["packed_ice"], 71),                           # 71 packed ice: denser, bluer than ice
    palette(PAL["fern"], 72),                                 # 72 fern frond
    # content pass 2: birch, blueberries, pumpkins, flower petals
    birch_bark(),                                             # 73 birch log side
    log_top((214, 196, 150), (186, 166, 120), (222, 220, 210)),   # 74 birch log top
    palette(PAL["birch_leaves"], 75),                         # 75 birch leaves: light, yellowish
    noise_tile((70, 80, 170), 14, (110, 120, 210), 0.25),     # 76 blueberries
    pumpkin_side(),                                           # 77 pumpkin side
    pumpkin_top(),                                            # 78 pumpkin top
    noise_tile((70, 110, 230), 12, (120, 160, 250), 0.2),     # 79 petal blue (cornflower)
    noise_tile((240, 240, 236), 8, (255, 255, 255), 0.2),     # 80 petal white (oxeye daisy)
    noise_tile((160, 80, 200), 12, (200, 120, 230), 0.2),     # 81 petal purple (allium)
    noise_tile((230, 224, 206), 6, (206, 198, 176), 0.15),    # 82 bone (carcasses)
    noise_tile((150, 46, 46), 12, (110, 28, 30), 0.25),       # 83 raw meat (carcasses)
    leaf_litter(),                                            # 84 leaf litter (forest floor)
    oak_bark(),                                               # 85 oak bark: grey-brown, dark grooves (trunks in the world)
    keep_rng(stones_pattern(["#36332e", "#5e5a53", "#7f7a71", "#a39d91", "#b5ad9c", "#cdc6b6", "#ece6d8", "#8f6f52"], 86)),   # 86 pebbles: lit stones, dark gaps (ground pickup)
    keep_rng(flint_pattern(87)),                              # 87 flint stones: glassy blue-black, small chalky patches (ground pickup)
    thatch(True),                                             # 88 thatch top
    thatch(False),                                            # 89 thatch side
    daub(),                                                   # 90 daub (wattle and daub wall)
    noise_tile((176, 132, 76), 12, (140, 100, 54), 0.25),     # 91 wattle withy: peeled hazel, lighter than logs
    noise_tile((120, 30, 10), 18, (255, 120, 30), 0.35),      # 92 torch ember: charred wick glowing orange
    wood_grain(["#3a2414", "#6e4a2a", "#9e7c4c", "#b8925a", "#c9a46c", "#dcbc86", "#f0d9aa", "#4f331c"], 93),   # 93 twigs: pale wood, grain (ground pickup)
    ore_stones_pattern(["#d4692a", "#3f9a6a"], 94),          # 94 copper-bearing stones: native copper, malachite
    ore_stones_pattern(["#2a1d14", "#58402f"], 95),          # 95 tin-bearing stones: black cassiterite
    ore_stones_pattern(["#a8402a", "#c9662f"], 96),          # 96 iron-bearing stones: hematite, limonite
    wood_grain(["#1f1209", "#2e1c0f", "#432b17", "#573a20", "#6e4a2a", "#7f5733", "#9a6d42", "#9fae7a"], 97, across=True),   # 97 twigs sides: bark, lichen
    cobblestone(),                                            # 98 cobblestone (patterned)
    bricks(),                                                 # 99 bricks (patterned)
    palette(PAL["berry_bush"], 100),                          # 100 blueberry bush: leaves dotted with berries
    stripes(["#1f2a0e", "#3b4a1a", "#4c5d22", "#5d6e2a", "#6f7f33", "#84903f", "#9aa34e", "#b5a253"], 101, "rows"),   # 101 kelp: olive segments (ADR 0047)
    stripes(["#1f4a1a", "#2f6a27", "#3f8433", "#4f9a3e", "#62ad4b", "#78c05c", "#93d276", "#d6e59a"], 102, "cols"),   # 102 seagrass: blades
    stripes(["#8a5a4a", "#c98f7c", "#dba795", "#e8bdaa", "#f2d0c0", "#f8e2d6", "#fff3ea", "#b06f5b"], 103, "diag", (1, 1)),   # 103 seashell: pink ribs
    crystal(104),                                             # 104 crystal: geode lining (glows in the shader, ADR 0052)
    # conduit chambers (ADR 0110): spring stone, blue-grey with wet veins; magma heart, black with orange cracks
    palette([("#5a6e80", 5), ("#4c5e6e", 4), ("#6c8296", 3), ("#8fb0c4", 2), ("#3e4e5c", 2), ("#b8d8e8", 1), ("#2f3c48", 1)], 105),   # 105 spring stone
    palette([("#2a2420", 5), ("#1e1a18", 4), ("#3a2e28", 3), ("#d85a18", 2), ("#f08a2a", 1), ("#8a3410", 2), ("#ffc040", 1)], 106),   # 106 magma heart
] + [palette(PAL["stone"], 107 + i) for i in range(2)] + [   # 107-108 reserved for world generation
    # the dry savanna floor (bug report 2026-10-25 20-37-22)
    palette([("#b4a058", 5), ("#a48e4a", 4), ("#c4b068", 3), ("#958040", 2), ("#8c8c4c", 2), ("#d2bf7c", 1), ("#7a683a", 1)], 109),   # 109 savanna grass: straw, a hint of olive
    palette([("#d4ba68", 5), ("#c6a858", 4), ("#e0cb80", 3), ("#b49548", 2), ("#a9a25c", 1), ("#ead9a0", 1)], 110),   # 110 savanna tall grass: golden tussocks
    palette([("#8e4c30", 5), ("#7e432b", 4), ("#9c5a3a", 3), ("#6e3c26", 2), ("#a86a46", 1), ("#6a4c36", 2)], 111),   # 111 red earth: laterite
    # multi-voxel crops and torches (ADR 0053): 8 colours, laid over the world grid ("pattern": true)
    stripes(["#1c3a12", "#2f5a1c", "#3e7224", "#4d872c", "#5e9c36", "#73b043", "#8cc456", "#c8d97a"], 112, "cols"),   # 112 wheat, green blades
    stripes(["#3c4a14", "#5e6e1e", "#7a8a28", "#94a032", "#aab23e", "#c2c04c", "#d6cc62", "#e8dc8a"], 113, "cols"),   # 113 wheat, turning
    stripes(["#6a4a14", "#93691e", "#b08428", "#c49a34", "#d4ae44", "#e2c258", "#eed474", "#f8e8a8"], 114, "cols"),   # 114 wheat, golden stalks
    stripes(["#6a4410", "#b8861e", "#cc9c2a", "#dcb036", "#e8c248", "#f2d460", "#fae890", "#fff6c8"], 115, "rows", (1, 1), 0.12),   # 115 wheat ear: rows of kernels
    stripes(["#163a10", "#22561a", "#2f7024", "#3d882c", "#4e9e36", "#63b444", "#7fca58", "#b4e07e"], 116, "diag", (1, 1), 0.1),   # 116 carrot leaves: feathery
    stripes(["#7a2c08", "#a83e0c", "#c85412", "#dc6a18", "#ea7c20", "#f4922e", "#ffb050", "#5e2206"], 117, "rows", (1, 1)),   # 117 carrot shoulder: rings
    wood_grain(["#1e1208", "#33200f", "#4a3018", "#5e3e20", "#734e2a", "#8a6036", "#a07448", "#2a1a0c"], 118),   # 118 torch stick: dark wood
    stones_pattern(["#3a1a0a", "#5e2c12", "#7e3e18", "#a0521e", "#c06a24", "#e08a30", "#f8b048", "#ffe08a"], 119, n=48),   # 119 torch wrap: cloth glowing with embers
    stripes(["#c03408", "#e0580e", "#f07418", "#f89028", "#fcae3c", "#ffc850", "#ffe070", "#fff4c0"], 120, "rows", (1, 2), 0.1),   # 120 torch flame: red to a pale tip (glows, flame_tile)
]
# 104-135: other parts' rows (2026-10-25 split); empty here until the branches merge
tiles += [Image.new("RGBA", (TILE, TILE)) for _ in range(136 - len(tiles))]
tiles += atlas_rocks.tiles(palette, stones_pattern, stripes, spotted_cap)   # 136-170 (ADR 0057)


def tnt(top):
    """TNT, patterned (one texel per voxel, a 4-voxel period = one 0.5 m block): the side is red sticks
    with a paper band across the middle two rows, the top red with a grey fuse in each block's centre."""
    img = Image.new("RGBA", (TILE, TILE))
    px = img.load()
    rng = random.Random(171 + top)
    for y in range(TILE):
        for x in range(TILE):
            ix, iy = x % 4, y % 4
            if top:
                c = (70, 70, 74) if ix in (1, 2) and iy in (1, 2) else ((150, 30, 22) if ix in (0, 3) and iy in (0, 3) else (204, 52, 38))
            elif iy in (1, 2):
                c = (232, 226, 210) if (x + y) % 3 else (40, 34, 30)   # the paper band, dark letters
            else:
                c = (120, 20, 14) if ix == 0 else (212, 56, 40)
            d = rng.randint(-6, 6)
            px[x, y] = (max(0, min(255, c[0] + d)), max(0, min(255, c[1] + d)), max(0, min(255, c[2] + d)), 255)
    return img


assert len(tiles) == 171, len(tiles)
tiles += [tnt(False), tnt(True)]   # 171 tnt side, 172 tnt top (ADR 0065)


def _px(img, x, y, c, rng, d=6):
    v = rng.randint(-d, d)
    img.putpixel((x, y), tuple(max(0, min(255, k + v)) for k in c) + (255,))


def monolith_ashlar():
    """Gelvin monolith stone, patterned (16 voxels = 2 m): 2 m x 1 m blocks in offset courses, dark
    joints, a pale chipped top edge. Albedo near the rune tile's stone so bands blend."""
    img = Image.new("RGBA", (TILE, TILE))
    rng = random.Random(173)
    for y in range(TILE):
        for x in range(TILE):
            off = 0 if y < 8 else 8
            ix, iy = (x + off) % 16, y % 8
            if iy == 7 or ix == 0:
                c = (34, 32, 40)                       # joint
            elif iy == 0 and rng.random() < 0.6:
                c = (118, 114, 126)                    # worn top edge
            else:
                c = (78, 75, 88) if rng.random() > 0.08 else (58, 55, 66)
            _px(img, x, y, c, rng)
    return img


GLYPHS = ["0100010", "0110110", "0011100", "0001000", "0011100", "0110110", "0100010",
          "0111110", "0100010", "0101010", "0100010", "0111110", "0001000", "0001000",
          "0001000", "0011000", "0101000", "0001110", "0001010", "0001001", "0001000",
          "1000001", "0100010", "0010100", "0011100", "0001000", "0001000", "0001000"]


def monolith_runes():
    """A carved band: two glowing magenta glyphs per 2 m between thin glowing rules, on dark stone
    (the shader glows only the magenta texels, ADR 0098)."""
    img = Image.new("RGBA", (TILE, TILE))
    rng = random.Random(174)
    pink = (255, 60, 220)
    for y in range(TILE):
        for x in range(TILE):
            c = (38, 36, 44) if rng.random() > 0.1 else (28, 26, 34)
            if y in (1, 14):
                c = pink
            elif y in (0, 15):
                c = (24, 22, 30)
            _px(img, x, y, c, rng)
    for g, ox in ((0, 0), (2, 8)):
        for gy in range(7):
            for gx in range(7):
                rows = GLYPHS[g * 7:(g + 1) * 7]
                if rows[gy][gx] == "1":
                    _px(img, ox + gx, 4 + gy, pink, rng, 18)
    return img


def monolith_seam():
    """The seam between the two columns: raw magenta light, hot white veins."""
    img = Image.new("RGBA", (TILE, TILE))
    rng = random.Random(175)
    for y in range(TILE):
        for x in range(TILE):
            r = rng.random()
            c = (255, 210, 250) if r < 0.08 else (240, 40, 210) if r < 0.6 else (190, 20, 170)
            _px(img, x, y, c, rng, 10)
    return img


tiles += [monolith_ashlar(), monolith_runes(), monolith_seam()]   # 173..175 (ADR 0098)
assert len(tiles) == 176, len(tiles)
tiles += [planks(c) for c in [(118, 82, 50), (196, 110, 58), (166, 104, 82), (222, 204, 160)]]   # 176..179 spruce, acacia, jungle, birch planks


def cave_tiles():
    """Cave dressing (2026-11-02 overnight): 192 dripstone (tan, banded), 193 cave vine, 194 fungus stem;
    204 glow berries on a vine, 205 magma rock (dark crust, glowing cracks), 206 fungus cap (glowing spots).
    204..207 glow in the terrain shader, their bright texels only."""
    out = {}
    rng = random.Random(192)
    img = Image.new("RGBA", (TILE, TILE))
    for y in range(TILE):
        band = (y * 3 + (y // 5)) % 7
        for x in range(TILE):
            c = [(176, 142, 104), (160, 126, 90), (190, 158, 118), (146, 112, 80), (176, 142, 104), (168, 134, 96), (200, 170, 130)][band]
            _px(img, x, y, c, rng, 8)
    out[192] = img
    img = Image.new("RGBA", (TILE, TILE))
    for y in range(TILE):
        for x in range(TILE):
            leaf = (x + y * 3) % 5 < 2 or rng.random() < 0.25
            _px(img, x, y, (52, 120, 46) if leaf else (34, 82, 34), rng, 10)
    out[193] = img
    img = Image.new("RGBA", (TILE, TILE))
    for y in range(TILE):
        for x in range(TILE):
            _px(img, x, y, (214, 204, 186) if (x % 4) else (186, 174, 156), rng, 8)
    out[194] = img
    img = Image.new("RGBA", (TILE, TILE))
    for y in range(TILE):
        for x in range(TILE):
            _px(img, x, y, (40, 104, 40) if rng.random() > 0.3 else (28, 74, 30), rng, 8)
    for _ in range(9):
        bx, by = rng.randrange(1, 15), rng.randrange(1, 15)
        for dx, dy in ((0, 0), (1, 0), (0, 1), (1, 1)):
            _px(img, bx + dx - 1, by + dy - 1, (255, 176, 60) if (dx + dy) else (255, 226, 140), rng, 6)
    out[204] = img
    img = Image.new("RGBA", (TILE, TILE))
    for y in range(TILE):
        for x in range(TILE):
            _px(img, x, y, (44, 32, 30) if rng.random() > 0.15 else (64, 44, 38), rng, 6)
    for _ in range(4):
        x, y = rng.randrange(TILE), rng.randrange(TILE)
        for _ in range(12):
            _px(img, x % TILE, y % TILE, (255, 120, 30) if rng.random() > 0.3 else (255, 200, 80), rng, 10)
            x += rng.choice((-1, 0, 1, 1))
            y += rng.choice((-1, 0, 1))
    out[205] = img
    img = Image.new("RGBA", (TILE, TILE))
    for y in range(TILE):
        for x in range(TILE):
            _px(img, x, y, (98, 52, 138) if rng.random() > 0.2 else (80, 40, 116), rng, 8)
    for _ in range(7):
        bx, by = rng.randrange(1, 15), rng.randrange(1, 15)
        for dx, dy in ((0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)):
            _px(img, (bx + dx) % TILE, (by + dy) % TILE, (120, 255, 230) if (dx or dy) else (220, 255, 250), rng, 6)
    out[206] = img
    img = Image.new("RGBA", (TILE, TILE))   # 195 rusted rail: iron gone to rust, a worn bright top
    for y in range(TILE):
        for x in range(TILE):
            c = (150, 152, 158) if y < 3 else ((128, 66, 34) if rng.random() > 0.35 else (96, 52, 30))
            _px(img, x, y, c, rng, 10)
    out[195] = img
    # 196 dune grass (pale sea-green blades), 197 heather (purple bells on dark green), 198 tundra sedge (straw brown)
    for idx, base, tip, mix in ((196, (128, 156, 104), (196, 200, 150), 0.35), (197, (58, 84, 52), (168, 84, 160), 0.45), (198, (138, 112, 70), (186, 162, 110), 0.3)):
        img = Image.new("RGBA", (TILE, TILE))
        for y in range(TILE):
            for x in range(TILE):
                _px(img, x, y, tip if ((x * 5 + y * 2) % 7 < 2 or rng.random() < mix * 0.4) else base, rng, 10)
        out[idx] = img
    img = Image.new("RGBA", (TILE, TILE))   # 199 rope: twisted fibre, diagonal strands
    for y in range(TILE):
        for x in range(TILE):
            _px(img, x, y, (184, 156, 108) if (x + y) % 4 < 2 else (138, 112, 74), rng, 8)
    out[199] = img
    # each biome's flora (FineGen FLORA): leaves of the base colour, flecks of the bloom or berry colour
    for idx, base, tip in (
        (200, (112, 128, 104), (168, 176, 150)),
        (201, (112, 140, 80), (240, 200, 40)),
        (202, (70, 112, 60), (226, 120, 180)),
        (203, (80, 104, 50), (250, 96, 30)),
        (208, (40, 80, 44), (200, 30, 40)),
        (209, (60, 130, 60), (230, 60, 110)),
        (210, (96, 128, 64), (110, 70, 40)),
        (211, (96, 140, 110), (150, 186, 150)),
        (212, (50, 100, 50), (90, 100, 220)),
        (213, (150, 160, 130), (210, 220, 190)),
        (214, (70, 110, 70), (170, 120, 210)),
        (215, (70, 130, 50), (250, 220, 40)),
    ):
        img = Image.new("RGBA", (TILE, TILE))
        for y in range(TILE):
            for x in range(TILE):
                bloom = (x * 3 + y * 5) % 11 < 2 or rng.random() < 0.12
                _px(img, x, y, tip if bloom else base, rng, 10)
        out[idx] = img
    return out


ROWS = max(ROWS, 27)
blank = Image.new("RGBA", (TILE, TILE), (255, 0, 255, 255))
while len(tiles) < 216:
    tiles.append(blank)
for i, t in cave_tiles().items():
    tiles[i] = t
assert len(tiles) <= COLS * ROWS, len(tiles)

tiles = [grade(t) for t in tiles]   # one palette rule for every generated tile (ADR 0129)

# hand-painted overrides: game/art/tiles/<index>[_note].png (16x16) replaces tile <index> (game/art/README.md)
ART = os.path.join(os.path.dirname(__file__), "..", "art", "tiles")
if os.path.isdir(ART):
    for f in sorted(os.listdir(ART)):
        if not f.endswith(".png"):
            continue
        i = int(f.split("_")[0].split(".")[0])
        im = Image.open(os.path.join(ART, f)).convert("RGBA")
        if i >= len(tiles) or im.size != (TILE, TILE):
            raise SystemExit("game/art/tiles/%s: needs an existing tile index and %dx%d pixels" % (f, TILE, TILE))
        tiles[i] = im
        print("art override", f)

atlas = Image.new("RGBA", (COLS * TILE, ROWS * TILE))
for i, t in enumerate(tiles):
    atlas.paste(t, ((i % COLS) * TILE, (i // COLS) * TILE))

out = os.path.join(os.path.dirname(__file__), "..", "assets", "textures", "atlas.png")
os.makedirs(os.path.dirname(out), exist_ok=True)
atlas.save(out)
print("wrote", os.path.abspath(out), atlas.size)
