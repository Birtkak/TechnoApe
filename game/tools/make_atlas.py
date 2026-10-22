"""Generate the placeholder 16x16-tile texture atlas (4x4 tiles = 64x64 px).

Tiles (index = row*4 + col):
 0 stone  1 dirt   2 grass_top  3 grass_side
 4 sand   5 water  6 wood       7 lava
 8 ash    9 oil   10 steam     11 smoke
12 fire  13 bedrock 14 spare    15 spare
Run: python game/tools/make_atlas.py  -> game/assets/textures/atlas.png
"""
import os
import random

from PIL import Image

TILE = 16
COLS = 8
ROWS = 4

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


def grass_side():
    img = noise_tile((120, 86, 56), 14)
    px = img.load()
    for y in range(4):
        for x in range(TILE):
            if y < 3 or random.random() < 0.5:
                d = random.randint(-12, 12)
                px[x, y] = (max(0, 70 + d), max(0, 150 + d), max(0, 60 + d), 255)
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
] + [noise_tile((255, 0, 255), 0) for _ in range(12)]         # 20..31 spare (magenta = missing)

atlas = Image.new("RGBA", (COLS * TILE, ROWS * TILE))
for i, t in enumerate(tiles):
    atlas.paste(t, ((i % COLS) * TILE, (i // COLS) * TILE))

out = os.path.join(os.path.dirname(__file__), "..", "assets", "textures", "atlas.png")
os.makedirs(os.path.dirname(out), exist_ok=True)
atlas.save(out)
print("wrote", os.path.abspath(out), atlas.size)
