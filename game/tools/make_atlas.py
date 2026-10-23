"""Generate the placeholder 16x16-tile texture atlas (4x4 tiles = 64x64 px).

Tiles (index = row*4 + col):
 0 stone  1 dirt   2 grass_top  3 grass_side
 4 sand   5 water  6 wood       7 lava
 8 ash    9 oil   10 steam     11 smoke
12 fire  13 bedrock 14 snow 15 gravel 16-19 ores 20 leaves 21 sandstone 22 clay
23 tall_grass 24 flower 25 cactus 26 dead_bush 27 ice 28 flower_red
29-31 plant colours 32 crafting_table top 33 crafting_table side 34 furnace top
35 furnace front 36 furnace front lit 37 chest top 38 chest side 39 planks 40-50 detail, sludge, sulfur, obsidian
51 wool 52 farmland 53 farmland moist 54 ripe wheat 55 carrot
56 oak log top 57-59 spruce log side/top, needles 60-62 acacia 63-65 jungle
66-68 terracotta orange/red/cream 69 basalt top 70 basalt side 71 packed ice
72 fern frond; 73 birch bark 74 birch top 75 birch leaves 76 blueberries
77 pumpkin side 78 pumpkin top 79-81 petals blue / white / purple; 82 bone 83 raw meat; 84-87 free
88 thatch top 89 thatch side 90 daub 91 wattle withy 92 torch ember (ADR 0030); 93-95 free (8x12)
Run: python game/tools/make_atlas.py  -> game/assets/textures/atlas.png
"""
import os
import random

from PIL import Image

TILE = 16
COLS = 8
ROWS = 12  # 96 tiles; rows grow at the bottom so tile indices keep their meaning

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


def spotted(base, spot):
    img = noise_tile(base, 12, None, 0)
    for (sx, sy) in [(3, 3), (10, 4), (6, 9), (12, 11), (2, 12)]:
        for dy in range(2):
            for dx in range(2):
                img.putpixel((sx + dx, sy + dy), (spot[0], spot[1], spot[2], 255))
    return img


tiles = [
    noise_tile((128, 128, 132), 14, (100, 100, 104), 0.08),   # 0 stone
    noise_tile((120, 86, 56), 14, (90, 60, 40), 0.1),         # 1 dirt
    noise_tile((70, 150, 60), 14, (50, 120, 45), 0.12),       # 2 grass top
    grass_side(),                                             # 3 grass side
    noise_tile((218, 196, 120), 10, (200, 180, 110), 0.1),    # 4 sand
    noise_tile((50, 100, 220), 10, (80, 130, 240), 0.15),     # 5 water
    noise_tile((140, 95, 50), 8, None, 0, stripes=True),      # 6 wood
    noise_tile((240, 110, 30), 16, (255, 200, 60), 0.2),      # 7 lava
    noise_tile((150, 148, 140), 10, (120, 118, 112), 0.15),   # 8 ash
    noise_tile((60, 50, 30), 8, (40, 35, 20), 0.1),           # 9 oil
    noise_tile((225, 228, 235), 8),                           # 10 steam
    noise_tile((80, 80, 84), 10),                             # 11 smoke
    noise_tile((255, 150, 40), 22, (255, 230, 90), 0.3),      # 12 fire
    noise_tile((40, 40, 44), 10, (60, 60, 64), 0.15),         # 13 bedrock
    noise_tile((240, 243, 250), 6),                           # 14 snow
    noise_tile((140, 135, 128), 18, (100, 96, 92), 0.2),      # 15 gravel
    noise_tile((128, 128, 132), 14, (30, 30, 32), 0.22),      # 16 coal ore
    noise_tile((128, 128, 132), 14, (200, 150, 110), 0.22),   # 17 iron ore
    noise_tile((128, 128, 132), 14, (210, 120, 70), 0.22),    # 18 copper ore
    noise_tile((128, 128, 132), 14, (200, 205, 215), 0.22),   # 19 tin ore
    noise_tile((45, 120, 40), 22, (25, 80, 30), 0.3),         # 20 leaves
    sandstone(),                                              # 21 sandstone
    noise_tile((150, 158, 170), 10, (120, 128, 140), 0.12),   # 22 clay
    noise_tile((96, 182, 74), 14, (70, 150, 55), 0.15),       # 23 tall grass body (micro-voxel model), lighter than grass top
    noise_tile((96, 182, 74), 14, (70, 150, 55), 0.15),       # 24 flower body (micro-voxel model)
    cactus(),                                                 # 25 cactus
    noise_tile((135, 95, 50), 14, (100, 70, 35), 0.2),        # 26 dead bush (micro-voxel model)
    noise_tile((170, 205, 240), 8, (210, 230, 250), 0.15),    # 27 ice
    noise_tile((96, 182, 74), 14, (70, 150, 55), 0.15),       # 28 flower_red body (micro-voxel model)
    noise_tile((60, 140, 55), 12),                            # 29 leaf green (stems)
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
    noise_tile((150, 100, 60), 14, (120, 80, 45), 0.2),       # 40 mushroom cap, brown (micro-voxel detail)
    spotted((200, 40, 40), (240, 235, 220)),                  # 41 mushroom cap, red with white spots
    noise_tile((225, 215, 190), 8, (200, 190, 165), 0.15),    # 42 mushroom stem
    noise_tile((110, 175, 70), 10, (150, 200, 90), 0.2),      # 43 lily pad, lighter than grass so it reads on a puddle
    noise_tile((150, 120, 70), 12, (120, 95, 55), 0.2),       # 44 reed head, brown
    spotted((70, 200, 210), (190, 250, 255)),                 # 45 glow mushroom cap (emissive in the shader)
    noise_tile((48, 92, 40), 10, (70, 120, 55), 0.25),        # 46 moss carpet
    noise_tile((150, 14, 20), 14, (95, 6, 12), 0.22),         # 47 blood (liquid; its stains come from scripts/visuals/stain_map.gd)
    noise_tile((150, 226, 30), 16, (226, 255, 120), 0.2),     # 48 toxic sludge: acid lime with bubbles, see-through like water
    noise_tile((214, 192, 58), 12, (168, 138, 30), 0.22),     # 49 sulfur crust
    noise_tile((30, 20, 42), 6, (76, 54, 110), 0.10),         # 50 obsidian
    wool(),                                                   # 51 wool (blocks, carpet, wild carrot flowers)
    farmland((104, 70, 44), (66, 42, 26)),                    # 52 farmland top, dry
    farmland((74, 48, 30), (44, 28, 16)),                     # 53 farmland top, moist (hydrated)
    noise_tile((214, 178, 70), 14, (176, 138, 44), 0.25),     # 54 ripe wheat (crop heads and stalks, wild wheat)
    noise_tile((236, 124, 30), 12, (200, 90, 20), 0.2),       # 55 carrot orange
    # content pass: tree kinds (biomes.json tree_kind), badlands strata, basalt, packed ice, ferns
    log_top((176, 136, 82), (140, 100, 56), (110, 76, 40)),   # 56 oak log top
    bark((70, 50, 32), (40, 28, 18)),                         # 57 spruce log side: dark bark
    log_top((150, 112, 66), (118, 84, 46), (70, 50, 32)),     # 58 spruce log top
    noise_tile((38, 78, 52), 12, (24, 58, 40), 0.3),          # 59 spruce needles: dark blue-green
    bark((112, 106, 96), (78, 72, 64)),                       # 60 acacia log side: grey bark
    log_top((200, 104, 52), (168, 82, 40), (112, 106, 96)),   # 61 acacia log top: orange heartwood
    noise_tile((112, 136, 44), 16, (86, 110, 30), 0.3),       # 62 acacia leaves: olive
    bark((128, 98, 56), (88, 92, 40)),                        # 63 jungle log side: mossy streaks
    log_top((180, 140, 90), (150, 110, 66), (128, 98, 56)),   # 64 jungle log top
    noise_tile((40, 150, 36), 20, (24, 110, 24), 0.35),       # 65 jungle leaves: bright, broad-leaved
    terracotta((176, 96, 54), (196, 120, 70)),                # 66 terracotta orange
    terracotta((140, 70, 52), (160, 88, 64)),                 # 67 terracotta red-brown
    terracotta((208, 176, 140), (224, 196, 162)),             # 68 terracotta cream
    basalt_top(),                                             # 69 basalt top
    basalt_side(),                                            # 70 basalt side
    noise_tile((140, 176, 222), 6, (176, 204, 238), 0.12),    # 71 packed ice: denser, bluer than ice
    noise_tile((70, 146, 58), 12, (50, 120, 44), 0.2),        # 72 fern frond (micro-voxel model)
    # content pass 2: birch, blueberries, pumpkins, flower petals
    birch_bark(),                                             # 73 birch log side
    log_top((214, 196, 150), (186, 166, 120), (222, 220, 210)),   # 74 birch log top
    noise_tile((120, 168, 70), 16, (96, 140, 52), 0.3),       # 75 birch leaves: light, yellowish
    noise_tile((70, 80, 170), 14, (110, 120, 210), 0.25),     # 76 blueberries
    pumpkin_side(),                                           # 77 pumpkin side
    pumpkin_top(),                                            # 78 pumpkin top
    noise_tile((70, 110, 230), 12, (120, 160, 250), 0.2),     # 79 petal blue (cornflower)
    noise_tile((240, 240, 236), 8, (255, 255, 255), 0.2),     # 80 petal white (oxeye daisy)
    noise_tile((160, 80, 200), 12, (200, 120, 230), 0.2),     # 81 petal purple (allium)
    noise_tile((230, 224, 206), 6, (206, 198, 176), 0.15),    # 82 bone (carcasses)
    noise_tile((150, 46, 46), 12, (110, 28, 30), 0.25),       # 83 raw meat (carcasses)
    noise_tile((128, 128, 128), 0),                           # 84 free
    noise_tile((128, 128, 128), 0),                           # 85 free
    noise_tile((128, 128, 128), 0),                           # 86 free
    noise_tile((128, 128, 128), 0),                           # 87 free
    thatch(True),                                             # 88 thatch top
    thatch(False),                                            # 89 thatch side
    daub(),                                                   # 90 daub (wattle and daub wall)
    noise_tile((176, 132, 76), 12, (140, 100, 54), 0.25),     # 91 wattle withy: peeled hazel, lighter than logs
    noise_tile((120, 30, 10), 18, (255, 120, 30), 0.35),      # 92 torch ember: charred wick glowing orange
]
assert len(tiles) <= COLS * ROWS, len(tiles)

atlas = Image.new("RGBA", (COLS * TILE, ROWS * TILE))
for i, t in enumerate(tiles):
    atlas.paste(t, ((i % COLS) * TILE, (i // COLS) * TILE))

out = os.path.join(os.path.dirname(__file__), "..", "assets", "textures", "atlas.png")
os.makedirs(os.path.dirname(out), exist_ok=True)
atlas.save(out)
print("wrote", os.path.abspath(out), atlas.size)
