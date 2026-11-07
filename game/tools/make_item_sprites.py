"""Generates game/data/item_sprites.json: 16x16 pixel-art item sprites as
shared shapes (character grids), material palettes and per-item layers, so a
new metal or tool is one line. Shapes are drawn with the primitives below
(lines, ellipses, polygons, auto shading lit from the top-left, outlines).

Run: python game/tools/make_item_sprites.py [sheet.png]
  -> overwrites game/data/item_sprites.json; with a path, also writes a
     contact sheet of every sprite at 4x. Hand edits to the JSON are lost on
     the next run: change the shapes here instead.
"""
import json
import math
import os
import random
import sys


N = 16


def new():
    return [["."] * N for _ in range(N)]


def put(g, x, y, c):
    if 0 <= x < N and 0 <= y < N:
        g[y][x] = c


def get(g, x, y):
    if 0 <= x < N and 0 <= y < N:
        return g[y][x]
    return "."


def art(rows, g=None):
    g = g or new()
    for y, r in enumerate(rows):
        for x, c in enumerate(r):
            if c not in ". ":
                put(g, x, y, c)
    return g


def line(g, x0, y0, x1, y1, c="#", w=1):
    n = max(abs(x1 - x0), abs(y1 - y0), 1)
    for i in range(n + 1):
        x = round(x0 + (x1 - x0) * i / n)
        y = round(y0 + (y1 - y0) * i / n)
        for k in range(w):
            put(g, x + k, y, c)
    return g


def ellipse(g, cx, cy, rx, ry, c="#", ang=0.0):
    ca, sa = math.cos(math.radians(ang)), math.sin(math.radians(ang))
    for y in range(N):
        for x in range(N):
            dx, dy = x + 0.5 - cx, y + 0.5 - cy
            u = dx * ca + dy * sa
            v = -dx * sa + dy * ca
            if (u / rx) ** 2 + (v / ry) ** 2 <= 1.0:
                put(g, x, y, c)
    return g


def rect(g, x0, y0, x1, y1, c="#"):
    for y in range(y0, y1 + 1):
        for x in range(x0, x1 + 1):
            put(g, x, y, c)
    return g


def poly(g, pts, c="#"):
    for y in range(N):
        for x in range(N):
            px, py = x + 0.5, y + 0.5
            inside = False
            j = len(pts) - 1
            for i in range(len(pts)):
                xi, yi = pts[i]
                xj, yj = pts[j]
                if (yi > py) != (yj > py) and px < (xj - xi) * (py - yi) / (yj - yi) + xi:
                    inside = not inside
                j = i
            if inside:
                put(g, x, y, c)
    return g


def mirror_diag(g, c=None):
    """Union with the reflection across the anti-diagonal (x, y) -> (15 - y, 15 - x)."""
    out = [r[:] for r in g]
    for y in range(N):
        for x in range(N):
            if g[y][x] != ".":
                put(out, N - 1 - y, N - 1 - x, c or g[y][x])
    return out


def shade(g, body="#", light="l", mid="m", dark="d", hi="h"):
    """Body pixels lit from the top-left: light where the top or left neighbour
    is outside the body, dark where the bottom or right one is, highlight
    where both top and left are outside."""
    out = [r[:] for r in g]
    for y in range(N):
        for x in range(N):
            if g[y][x] != body:
                continue
            up = get(g, x, y - 1) != body
            lf = get(g, x - 1, y) != body
            dn = get(g, x, y + 1) != body
            rt = get(g, x + 1, y) != body
            if up and lf:
                c = hi
            elif up or lf:
                c = light
            elif dn or rt:
                c = dark
            else:
                c = mid
            out[y][x] = c
    return out


def outline(g, c="o"):
    out = [r[:] for r in g]
    for y in range(N):
        for x in range(N):
            if g[y][x] != ".":
                continue
            if any(get(g, x + dx, y + dy) != "." for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1))):
                out[y][x] = c
    return out


def rows(g):
    return ["".join(r) for r in g]



REPO = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..")
SHEET = sys.argv[1] if len(sys.argv) > 1 else ""


def tint(h, f):
    h = h.lstrip("#")
    c = [int(h[i:i + 2], 16) for i in (0, 2, 4)]
    c = [max(0, min(255, round(v * f))) for v in c]
    return "#%02x%02x%02x" % tuple(c)


def hue_shift(h, t):
    """ADR 0129: t < 0 turns a shade cooler (toward blue-violet), t > 0 warmer (toward yellow)."""
    h = h.lstrip("#")
    c = [int(h[i:i + 2], 16) for i in (0, 2, 4)]
    c = [c[0] + 20 * t, c[1] + 6 * t, c[2] - 20 * t]
    return "#%02x%02x%02x" % tuple(max(0, min(255, round(v))) for v in c)


def ramp(o, d, m, l, h):
    # the outline goes darker than the ramp's darkest tone, so every edge reads as a line; shades turn
    # cool and lights warm (ADR 0129), so close palettes (iron, tin, steel; the woods) separate
    return {"o": hue_shift(tint(o, 0.62), -0.6), "d": hue_shift(tint(d, 0.92), -0.8), "m": m,
            "l": hue_shift(l, 0.4), "h": hue_shift(h, 0.8)}


PALETTES = {
    "copper": ramp("#4a2414", "#8a4424", "#c0663a", "#e08a55", "#f6b98a"),
    "tin": ramp("#3a4044", "#79838a", "#a6b0b6", "#c8d0d4", "#eef2f4"),
    "bronze": ramp("#4a3010", "#8a5e22", "#b8863a", "#d8a850", "#f2d488"),
    "iron": ramp("#2e3034", "#6a6e74", "#9ea2a8", "#c4c8cc", "#eceff2"),
    "steel": ramp("#1e232a", "#4a525e", "#737d8a", "#9ca6b2", "#d0d8e2"),
    "stone": ramp("#2c2c2c", "#5a5a5a", "#808080", "#a0a0a0", "#c2c2c2"),
    "flint": ramp("#121216", "#2c2e36", "#474b57", "#6a707e", "#9ea6b4"),
    "wood": ramp("#3e2a14", "#7a5428", "#a07038", "#c08c4c", "#dcae70"),
    "handle": ramp("#2e1d0c", "#5a3a1a", "#7a5226", "#9a6a34", "#b8864a"),
    "coal": ramp("#0c0c0e", "#1c1c20", "#2c2c32", "#45454d", "#7a7a86"),
    "charcoal": ramp("#120c08", "#241810", "#38261a", "#523a28", "#7a6048"),
    "clay": ramp("#474c55", "#7a808c", "#979eaa", "#b0b6c0", "#cdd2da"),
    "rot": ramp("#1c1a10", "#3a3a1c", "#55532a", "#6e6a38", "#8a8448"),
    "terracotta": ramp("#4a1e10", "#8a3e22", "#b05a34", "#c8744a", "#e09468"),
    "fire_clay": ramp("#5a4e3a", "#968668", "#b8aa88", "#cec2a2", "#e6dec4"),
    "fire_brick": ramp("#5a3a22", "#9c6c3e", "#c4965e", "#d8b07a", "#ecd0a0"),
    "raw_meat": ramp("#4a1012", "#8a2224", "#bc3a3c", "#dc5e5e", "#f09a9a"),
    "cooked_meat": ramp("#32180a", "#62321a", "#874a24", "#a8663a", "#c88a56"),
    "blueberry": ramp("#131834", "#242e66", "#3a4c9e", "#5a72c6", "#a0b4f0"),
    "cranberry": ramp("#360a10", "#6a1422", "#9c2234", "#c43a4c", "#f07a88"),
    "leaf": ramp("#14300e", "#2c5a1e", "#3e7e2a", "#56a03a", "#86c860"),
    "bark": ramp("#24170c", "#4a3018", "#664226", "#825632", "#a07248"),
    "fiber": ramp("#1e3412", "#3e6424", "#5a8834", "#78a848", "#a8cc72"),
    "twine": ramp("#4a3a1c", "#8a7040", "#ae9058", "#c8aa70", "#e2c890"),
    "straw": ramp("#5a4614", "#a08430", "#c8a840", "#dcc058", "#f0dc86"),
    "mana": ramp("#1e0e3a", "#4a2690", "#6e44c8", "#9a74e8", "#d4c0ff"),
    "fire": ramp("#5a1004", "#c0300c", "#ec6418", "#fca030", "#ffe27a"),
    "glass": ramp("#3a6a78", "#7eb4c4", "#a6d6e2", "#c8eaf2", "#ffffff"),
    "gem": ramp("#2a0a3a", "#5c1a86", "#8a32c0", "#b45ae6", "#ecc2ff"),
    "bomb": ramp("#0e0e12", "#24242a", "#36363e", "#50505a", "#8a8a96"),
    "sulfur": ramp("#5a4a08", "#a8901c", "#d6bc32", "#ead25a", "#fff09a"),
    "water": ramp("#0e2a5a", "#2050a8", "#3a78d8", "#6aa4f0", "#c0e0ff"),
    "blood": ramp("#2a0206", "#5e0a10", "#8e141c", "#b8242c", "#e86a70"),
    "oil": ramp("#050403", "#16120c", "#2a2218", "#463a2a", "#8a7a60"),
    "toxic": ramp("#163a06", "#2e7a10", "#4eaa1e", "#7ed43a", "#d0ff90"),
    "blanket": ramp("#3a0a0e", "#7a161c", "#a82228", "#c83a3e", "#ec7a7a"),
    "carrot": ramp("#5a2004", "#a8460c", "#e0701a", "#f49434", "#ffc47a"),
    "bread": ramp("#4a2408", "#8a5018", "#b87a30", "#d49a4a", "#f0c880"),
    "egg": ramp("#6a5a44", "#b8a07c", "#dcc8a2", "#efe2c4", "#fcf6e4"),
    "feather": ramp("#5a5a5e", "#b4b4b8", "#dcdce0", "#efeff2", "#ffffff"),
    "bone": ramp("#6a604c", "#b8ac8c", "#d8ceb0", "#ebe4cc", "#f8f4e6"),
    "hide": ramp("#2a1a10", "#4e3220", "#6e4a30", "#8a6040", "#a87a54"),
    "scraped_hide": ramp("#5a4a36", "#a08a6a", "#c8b08a", "#dcc8a4", "#f0e4c8"),
    "leather": ramp("#2e1808", "#6a3a18", "#945428", "#b06c38", "#cc8c54"),
    "daub": ramp("#4a3e30", "#8a7a62", "#b2a084", "#c8b89c", "#e0d4bc"),
    "seed": ramp("#3a2c10", "#6e5824", "#9a8034", "#bca048", "#e0cc7a"),
    "sand": ramp("#6a5430", "#b8985a", "#dcc084", "#ead49c", "#f8ecc6"),
    "lava": ramp("#3a0802", "#8a1c04", "#d04a0a", "#f48a1c", "#ffd25a"),
    "hazard": ramp("#4a3a04", "#a88a0c", "#e0c020", "#f2d84a", "#fff090"),
}


def compose(*gs):
    out = new()
    for g in gs:
        for y in range(N):
            for x in range(N):
                if g[y][x] != ".":
                    out[y][x] = g[y][x]
    return out


def body(g):
    """shade + outline: the standard finish for an auto-shaded '#' mask."""
    return outline(shade(g))


U = (math.sqrt(0.5), -math.sqrt(0.5))     # along a tool handle, toward its tip
NRM = (-math.sqrt(0.5), -math.sqrt(0.5))  # perpendicular, toward the upper left
TIP = (12.0, 4.0)


def tool_pts(st, base=TIP):
    return [(base[0] + s * NRM[0] + t * U[0], base[1] + s * NRM[1] + t * U[1]) for s, t in st]


def diag(x0, y0, length, w=2):
    g = new()
    for i in range(length):
        for k in range(w):
            put(g, x0 + i + k, y0 - i, "#")
    return g


# ---------- shapes ----------
S = {}


def shape(name, g, colors=None):
    S[name] = {"rows": rows(g)}
    if colors:
        S[name]["colors"] = colors


# handles
def mc_handle(x0, y0, length):
    """A Minecraft tool handle: a one-pixel diagonal core with a wood grain, outlined (3 px across)."""
    g = new()
    for i in range(length):
        put(g, x0 + i, y0 - i, "l" if i % 3 == 0 else ("d" if i % 3 == 2 else "m"))
    return outline(g)


def mask(rows_spec):
    """'#' mask from {y: [(x0, x1), ...]} runs."""
    g = new()
    for y, runs in rows_spec.items():
        for a, b in runs:
            rect(g, a, y, b, y)
    return g


shape("handle", mc_handle(2, 13, 10))
shape("short_handle", mc_handle(2, 13, 5))
shape("stick", body(diag(2, 13, 11)))

# Minecraft-style heads (developer, 2026-10-27): big and two pixels thick, over the handle's top end
# pickaxe head: a thick crescent, the top arm mirrored across the anti-diagonal
# (2026-10-27: the developer's reference, Minecraft's pickaxe head pixel for pixel; the handle is ours)
shape("pickaxe_head", art([
    "................", "................", "......ooooo.....", ".....ohlmmlo....",
    "......oooomm....", "...........lmo..", "............mlo.", "............omo.",
    "............omo.", "............olo.", "............oho.", ".............o..",
    "................", "................", "................", "................"]))

# axe head: a broad blade to the upper left of the handle's end, honed along its left edge
g = shade(mask({1: [(5, 8)], 2: [(4, 10)], 3: [(4, 11)], 4: [(4, 12)], 5: [(4, 6), (9, 12)], 6: [(4, 5), (10, 11)]}))
for y in range(N):
    xs = [x for x in range(N) if g[y][x] != "."]
    if xs:
        g[y][xs[0]] = "h"   # honed edge
shape("axe_head", outline(g))

# shovel: a wide spade beyond the handle, rounded at the tip
shape("shovel_head", body(poly(new(), tool_pts([(-2.7, 0.0), (2.7, 0.0), (2.7, 4.4), (1.3, 6.3), (-1.3, 6.3), (-2.7, 4.4)], (8.8, 7.2)))))

# hammer: a thick block across the tip (5 px deep, 9 long)
shape("hammer_head", body(poly(new(), tool_pts([(-4.4, -2.4), (-4.4, 2.6), (4.4, 2.6), (4.4, -2.4)], (11.0, 5.0)))))

# saw (developer, 2026-10-27): a hand saw rising one pixel per two columns, so the lower edge is flat
# two-pixel steps and a lit tooth hangs crisply under each step (the dark outline cuts the notch between);
# the blade tapers from 4 pixels deep at the grip to 2 at the tip; the D grip is its own wooden shape
g = new()
for x in range(5, 15):
    ye = 12 - (x - 5) // 2
    rect(g, x, ye - (4 - (x - 5) * 2 // 10) + 1, x, ye)
g = shade(g)
for x in range(5, 15):
    put(g, x, min(y for y in range(N) if g[y][x] != "."), "l")   # an even spine, so only the teeth look toothy
for x in range(6, 14, 2):
    put(g, x, 12 - (x - 5) // 2 + 1, "h")   # a tooth under the left pixel of each step
shape("saw_blade", outline(g))
g = rect(new(), 1, 8, 5, 13)
g = shade(g)
rect(g, 2, 10, 3, 11, ".")   # the hand hole
shape("saw_handle", outline(g))

# knife: tapered blade, spine on the upper-left side
shape("knife_blade", body(poly(new(), tool_pts([(-1.6, 0.0), (1.6, 0.0), (1.6, 3.0), (0.4, 7.8), (-0.4, 8.6), (-1.6, 6.4)], (6.2, 9.8)))))
# chisel: a straight bar with a flat edge
shape("chisel_blade", body(poly(new(), tool_pts([(-1.3, 0.0), (1.3, 0.0), (1.3, 6.8), (-1.3, 6.8)], (6.2, 9.8)))))

# prospecting pick: pointed arm one side, small hammer face the other
g = new()
for x in range(4, 11):
    put(g, x, 1, "#")
for x in range(1, 13):
    put(g, x, 2, "#")
for x in range(8, 14):
    put(g, x, 3, "#")
rect(g, 11, 3, 14, 8)
shape("prospecting_head", body(g))

# ingot and brick
# ingot (2026-10-27 retry): Minecraft's bar lying on the diagonal, a lit top and two shaded sides
g = poly(new(), [(1, 9.5), (6, 12.5), (6, 15), (1, 12)], "m")                 # front-left end
poly(g, [(6, 12.5), (15, 7), (15, 9.5), (6, 15)], "d")                         # long front side
poly(g, [(1, 9.5), (10, 4), (15, 7), (6, 12.5)], "l")                          # top
line(g, 2, 9, 9, 5, "h")                                                       # lit back edge of the top
shape("ingot", outline(g))
shape("brick", outline(art([
    "................", "................", "................", "................",
    "................", "....hlllllll....", "...lllllllll....", "..llllllllllm...",
    "..lmmmmmmmmmmd..", "..lmmmmmmmmmmd..", "..mmmmmmmmmmmd..", "..mmmmmmmmmmmd..",
    "..dddddddddddd..", "................", "................", "................"])))

# a half-smithed work piece (ADR 0042): a flattened, hammer-dented bar
shape("work_piece", outline(art([
    "................", "................", "................", "................",
    "................", "...hhhh.........", "..lllllhhhh.....", ".llmmmlllllhhh..",
    ".lmmdmmmmmllll..", "..mmmmmdmmmmmd..", "...ddmmmmmmdd...", ".....ddddd......",
    "................", "................", "................", "................"])))

# nugget cluster
shape("nugget", body(art([
    "................", "................", "................", "................",
    "................", "......##........", ".....####.......", ".....####.##....",
    "......##.####...", "...##....####...", "..####....##....", "..####..##......",
    "...##..####.....", "........##......", "................", "................"])))

# rocks and lumps
rock = ellipse(new(), 8, 10, 5.8, 4.2)
shape("rock", body(rock))
g = shade(rock)
for (x, y, ch) in [(5, 9, "s"), (6, 9, "S"), (9, 11, "s"), (10, 8, "S"), (10, 9, "s"), (7, 12, "s"), (11, 11, "s"), (4, 11, "S")]:
    put(g, x, y, ch)
shape("ore_rock", outline(g))
g = shade(poly(new(), [(3.5, 12.5), (5.5, 5.5), (10, 2.5), (12.5, 6.5), (11.5, 11.5), (7, 13.5)]))
for (x, y) in [(7, 5), (8, 6), (8, 7), (6, 9), (7, 10)]:
    put(g, x, y, "h")
shape("flint_shard", outline(g))
g = shade(poly(new(), [(3, 11), (4.5, 6), (8, 4), (12, 5.5), (13, 10), (10, 13), (5, 13)]))
for (x, y) in [(6, 7), (9, 6), (10, 9), (7, 10)]:
    put(g, x, y, "h")
shape("lump", outline(g))
g = shade(rect(new(), 3, 5, 12, 12))
for y in range(5, 13):
    for x in (5, 8, 11):
        if get(g, x, y) == "m":
            put(g, x, y, "d")
for x in range(4, 12):
    put(g, x, 5, "l")
put(g, 6, 7, "h")
put(g, 9, 10, "h")
shape("charcoal_lump", outline(g))
shape("ball", body(ellipse(new(), 8, 9, 5, 5)))
g = shade(poly(new(), [(3, 11), (4, 6), (8, 3.5), (12.5, 5.5), (13, 11), (9, 13.5), (5, 13)]))
for (x, y) in [(6, 7), (9, 6), (10, 10), (7, 10), (11, 8), (5, 10)]:
    put(g, x, y, "x")
shape("bloom", outline(g), {"x": "#3a1a10"})

# dust, crystal, essence
g = shade(poly(new(), [(1.5, 13.5), (4, 10), (7, 7.5), (9.5, 7.5), (12, 10), (14.5, 13.5)]))
for (x, y) in [(5, 10), (8, 9), (11, 11), (7, 12), (3, 12)]:
    put(g, x, y, "h")
shape("dust_pile", outline(g))
g = poly(new(), [(8, 1), (11.5, 4), (11.5, 11), (8, 14.5), (4.5, 11), (4.5, 4)])
for y in range(N):
    for x in range(N):
        if g[y][x] == "#":
            g[y][x] = "l" if x < 7 else ("h" if x == 7 else ("m" if x < 10 else "d"))
shape("crystal", outline(g))
g = compose(poly(new(), [(8, 1), (12, 8), (4, 8)]), ellipse(new(), 8, 10, 4.5, 4.5))
for y in range(N):
    for x in range(N):
        if g[y][x] == "#":
            g[y][x] = "m"
inner = compose(poly(new(), [(8, 4.5), (10.5, 9), (5.5, 9)]), ellipse(new(), 8, 10.5, 2.8, 2.8))
for y in range(N):
    for x in range(N):
        if inner[y][x] == "#":
            g[y][x] = "l"
for (x, y) in [(7, 10), (8, 10), (7, 11), (8, 9)]:
    g[y][x] = "h"
shape("flame_drop", outline(g))


# berries: three outlined spheres and a leaf
def berry(cx, cy, r):
    return outline(shade(ellipse(new(), cx, cy, r, r)))


g = compose(berry(5.5, 10.5, 3), berry(10.5, 10.5, 3), berry(8, 6.5, 3))
for (x, y) in [(9, 2), (10, 2), (10, 3), (11, 3), (11, 2)]:
    put(g, x, y, "g")
put(g, 8, 3, "G")
shape("berries", g, {"g": "#3e7e2a", "G": "#2c5a1e"})

# steak with a fat streak and a bone
g = shade(ellipse(new(), 7.5, 9.5, 6, 4.2, ang=-25))
for (x, y) in [(4, 11), (5, 11), (6, 10), (7, 10), (8, 9), (9, 9), (10, 8)]:
    if g[y][x] != ".":
        g[y][x] = "f"
g = outline(g)
bone = outline(compose(ellipse(new(), 13, 4.5, 1.7, 1.7, "b"), line(new(), 11, 6, 12, 5, "b")))
shape("steak", compose(g, bone), {"f": "#f0e0d0", "b": "#f4efe4"})

# bowl and its contents
g = new()
for y in range(N):
    for x in range(N):
        dx, dy = x + 0.5 - 8, y + 0.5 - 8
        if dy >= -1 and (dx / 6.6) ** 2 + (dy / 5.6) ** 2 <= 1:
            put(g, x, y, "#")
g = shade(g)
ellipse(g, 8, 8, 5.2, 1.6, "i")
shape("bowl", outline(g), {"i": "#2a1408"})
g = ellipse(new(), 8, 8, 5.0, 1.5, "q")
for (x, y) in [(5, 8), (8, 7), (10, 8)]:
    put(g, x, y, "k")
shape("bowl_fill", g)

# bucket and its contents
g = shade(poly(new(), [(2.5, 4.5), (13.5, 4.5), (12.2, 14.5), (3.8, 14.5)]))
ellipse(g, 8, 5, 4.6, 1.4, "i")
g = outline(g)
for (x, y) in [(3, 3), (4, 2), (5, 1), (6, 1), (7, 1), (8, 1), (9, 1), (10, 1), (11, 2), (12, 3)]:
    put(g, x, y, "r")
shape("bucket", g, {"i": "#565a60", "r": "#46494e"})   # inside: dark iron, so an empty bucket reads as empty
g = ellipse(new(), 8, 5, 4.6, 1.4, "q")
put(g, 6, 4, "Q")
put(g, 7, 4, "Q")
shape("bucket_fill", g)

# flask
g = shade(compose(ellipse(new(), 8, 10.5, 5, 4.4), rect(new(), 7, 3, 8, 6)))
rect(g, 6, 2, 9, 2, "l")
for (x, y) in [(5, 9), (5, 10), (6, 8)]:
    put(g, x, y, "h")
shape("flask", outline(g))

# pottery
g = shade(poly(new(), [(3.5, 3), (12.5, 3), (11.2, 14.5), (4.8, 14.5)]))
ellipse(g, 8, 3.6, 4.0, 1.2, "i")
shape("crucible", outline(g), {"i": "#24140c"})
g = shade(rect(new(), 1, 7, 14, 12))
rect(g, 4, 8, 11, 10, "i")
rect(g, 4, 8, 11, 8, "I")
shape("ingot_mold", outline(g), {"i": "#3a1a0e", "I": "#24100a"})


def mold_of(head):
    """A clay slab with the head's silhouette pressed into it (tool molds, ADR 0023)."""
    src = S[head]["rows"]
    pts = [(x, y) for y in range(N) for x in range(N) if src[y][x] != "."]
    x0, x1 = min(p[0] for p in pts), max(p[0] for p in pts)
    y0, y1 = min(p[1] for p in pts), max(p[1] for p in pts)
    g = shade(rect(new(), 0, 4, 15, 13))
    for y in range(6, 12):
        for x in range(2, 14):
            sx = x0 + (x - 2) * (x1 - x0 + 1) // 12
            sy = y0 + (y - 6) * (y1 - y0 + 1) // 6
            if src[sy][sx] not in ".o":
                put(g, x, y, "i" if get(g, x, y - 1) != "I" else "i")
    for x in range(2, 14):
        for y in range(6, 12):
            if g[y][x] == "i" and g[y - 1][x] not in ("i", "I"):
                put(g, x, y, "I")
    return outline(g)


g = new()
line(g, 3, 13, 12, 4, "#", 2)     # the spindle
g = body(g)
line(g, 1, 9, 13, 9, "s")          # the bow string over it
line(g, 1, 9, 3, 7, "b", 1)
line(g, 13, 9, 11, 7, "b", 1)
line(g, 3, 7, 11, 7, "b", 1)
shape("firestarter", g, {"s": "#d8c8a0", "b": "#7a5226"})
g = shade(compose(ellipse(new(), 8, 10, 6.2, 4.6), rect(new(), 2, 6, 13, 10)))
rect(g, 2, 6, 13, 6, "h")
rect(g, 6, 4, 9, 5, "l")
g = outline(g)
for (x, y) in [(0, 8), (0, 9), (15, 8), (15, 9)]:
    put(g, x, y, "o")
shape("cooking_pot", g)

# stations without a block material
shape("anvil", body(art([
    "................", "................", "................", "................",
    "..############..", "...###########..", ".....#########..", "......#######...",
    ".......#####....", ".......#####....", "......#######...", ".....#########..",
    "....###########.", "................", "................", "................"])))
shape("stone_anvil", outline(art([
    "................", "................", "................", "................",
    "................", "...hhhhhhhhhh...", "...llllllllllm..", "...lmmmmmmmmmd..",
    "...lmmdmmmmmmd..", "...lmmmmmmdmmd..", "...lmmmmmmmmmd..", "...lmdmmmmmmmd..",
    "...lmmmmmmmmmd..", "...dddddddddddd.", "................", "................"])))


def bricks(g, x0, y0, x1, y1):
    for y in range(y0, y1 + 1):
        for x in range(x0, x1 + 1):
            row = (y - y0) // 3
            if (y - y0) % 3 == 2:
                c = "j"
            elif (x - x0 + (2 if row % 2 else 0)) % 5 == 4:
                c = "j"
            else:
                c = "B" if (y - y0) % 3 == 0 else "b"
            put(g, x, y, c)
    return g


BRICK = {"b": "#8e3e2a", "B": "#ac5640", "j": "#5a4a40", "o": "#2a1610"}
g = bricks(new(), 4, 1, 11, 14)
rect(g, 6, 10, 9, 14, "x")
rect(g, 7, 12, 8, 14, "y")
shape("blast_furnace", outline(g), dict(BRICK, x="#ff7a1a", y="#ffe066"))
g = shade(poly(new(), [(2.5, 14.5), (5, 5), (7, 2), (9, 2), (11, 5), (13.5, 14.5)]))
rect(g, 6, 11, 9, 14, "x")
rect(g, 7, 12, 8, 14, "y")
rect(g, 7, 2, 8, 2, "i")
shape("bloomery", outline(g), {"x": "#ff7a1a", "y": "#ffe066", "i": "#1a0c06"})
g = bricks(new(), 2, 7, 13, 14)
for x in range(3, 13):
    put(g, x, 6, "x" if x % 3 else "y")
    if x % 2:
        put(g, x, 5, "y" if x % 4 == 1 else "x")
shape("forge", outline(g), dict(BRICK, x="#2a1a14", y="#ff8a24"))
g = compose(line(new(), 2, 12, 13, 9), line(new(), 2, 13, 13, 10), line(new(), 2, 9, 13, 12, "%"), line(new(), 2, 10, 13, 13, "%"))
g = shade(g, "#")
g = shade(g, "%")
flame = poly(new(), [(5, 10), (6, 5), (8, 1.5), (10, 5), (11, 10)], "q")
ellipse(flame, 8, 8.3, 1.8, 2.6, "Q")
put(flame, 8, 9, "Y")
put(flame, 8, 8, "Y")
shape("campfire", outline(compose(g, flame)), {"q": "#e8541a", "Q": "#ffa030", "Y": "#ffe27a"})
g = shade(ellipse(new(), 8, 9, 7, 4))
for y in range(N):
    for x in range(N):
        dx, dy = x + 0.5 - 8, y + 0.5 - 9
        if (dx / 5.2) ** 2 + (dy / 2.4) ** 2 <= 1:
            g[y][x] = "n" if (x + y) % 2 else "N"
shape("sieve", outline(g), {"n": "#c8b088", "N": "#3a2a18"})
g = shade(rect(new(), 1, 8, 14, 11))
for x in range(1, 15):
    if x % 3 == 0:
        put(g, x, 9, "d")
        put(g, x + 1, 10, "d")
rect(g, 2, 6, 5, 8, "p")
rect(g, 2, 6, 5, 6, "P")
g = outline(g)
rect(g, 1, 12, 1, 13, "o")
rect(g, 14, 12, 14, 13, "o")
shape("bed", g, {"p": "#e8e0d0", "P": "#ffffff"})
g = shade(rect(new(), 7, 6, 8, 14))
flame = ellipse(new(), 8, 4, 1.9, 2.6, "q")
ellipse(flame, 8, 4.4, 1.0, 1.4, "Y")
shape("torch", outline(compose(g, flame)), {"q": "#ff8a24", "Y": "#ffe27a"})
stem = line(new(), 8, 14, 8, 8, "t")
put(stem, 7, 14, "t")
leaves = compose(body(ellipse(new(), 5.5, 7, 3, 2.4)), body(ellipse(new(), 10.5, 7, 3, 2.4)), body(ellipse(new(), 8, 4, 3, 2.6)))
shape("sapling", compose(outline(stem), leaves), {"t": "#6a4a24"})
# flowers: a stem with a leaf, and a head of petals around a centre (or a round allium ball)
stem = line(new(), 8, 15, 8, 7, "t")
line(stem, 8, 12, 11, 10, "u")
line(stem, 8, 13, 5, 11, "u")
shape("flower_stem", outline(stem), {"t": "#3e7e2a", "u": "#56a03a"})
g = new()
for (px, py) in [(8, 2.6), (10.8, 4.4), (10.2, 7.4), (5.8, 7.4), (5.2, 4.4)]:
    ellipse(g, px, py, 1.7, 1.7, "p")
ellipse(g, 8, 5.3, 1.4, 1.4, "c")
shape("flower_head", outline(g), {"p": "#f0d040", "c": "#c07a18", "o": "#2a2418"})
g = shade(ellipse(new(), 8, 5, 3.6, 3.6))
for (x, y) in [(6, 3), (9, 4), (7, 6), (10, 6), (8, 2)]:
    put(g, x, y, "h")
shape("allium_head", outline(g))

# bark, fiber, twine
g = shade(poly(new(), [(3, 13), (3.5, 5), (6.5, 2.5), (8, 3.5), (6, 6), (6, 13)]))
g2 = shade(poly(new(), [(8, 14), (9, 7), (12, 4.5), (13, 6), (11, 8), (11, 14)]))
for y in range(4, 13, 2):
    if g[y][4] != ".":
        put(g, 4, y, "d")
    if g2[y + 1][9] != ".":
        put(g2, 9, y + 1, "d")
shape("bark", compose(outline(g), outline(g2)))
g = compose(line(new(), 4, 14, 6, 2, "l"), line(new(), 7, 14, 9, 1, "m"), line(new(), 10, 14, 12, 3, "l"), line(new(), 5, 14, 11, 4, "d"))
shape("fiber", outline(g))
g = new()
for y in range(N):
    for x in range(N):
        dx, dy = x + 0.5 - 8, y + 0.5 - 8
        if (dx / 6.2) ** 2 + (dy / 5.2) ** 2 <= 1 and (dx / 3.4) ** 2 + (dy / 2.6) ** 2 > 1:
            put(g, x, y, "#")
g = shade(g)
for y in range(N):
    for x in range(N):
        if g[y][x] != "." and (x + y) % 3 == 0:
            g[y][x] = "d"
g = outline(g)
line(g, 12, 12, 14, 15, "l")
shape("twine", g)

# Flint Age depth (ADR 0030): spear, leather goods, pan, jar, stew, firewood
g = shade(poly(new(), [(12.6, 0.6), (15.0, 0.6), (15.0, 3.0), (12.0, 5.6), (10.4, 4.0)]))
shape("spear_tip", outline(g))
g = new()
line(g, 1, 14, 11, 4, "#", 2)
shape("spear_shaft", body(g))
g = shade(poly(new(), [(8, 1), (11, 5), (10.5, 10), (8, 15), (5.5, 10), (5, 5)]))
for y in range(4, 13):
    put(g, 8, y, "d")
shape("spear_head", outline(g))
g = new()
for k, (x0, y0) in enumerate([(2, 4), (3, 8), (2, 12)]):
    line(g, x0, y0, x0 + 11, y0 - 2, "#", 1)
    line(g, x0, y0 + 1, x0 + 11, y0 - 1, "#", 1)
shape("strips", body(g))
g = shade(compose(poly(new(), [(3, 6), (13, 6), (14, 14.5), (2, 14.5)]), ellipse(new(), 8, 6, 5, 1.6)))
rect(g, 3, 5, 12, 5, "f")
line(g, 4, 6, 6, 2, "s")
line(g, 12, 6, 10, 2, "s")
line(g, 6, 2, 10, 2, "s")
for x in (5, 8, 11):
    put(g, x, 10, "k")
shape("bag", outline(g), {"f": "#3a2010", "s": "#ae9058", "k": "#ae9058"})
g = shade(ellipse(new(), 8, 9, 7.2, 3.8))
ellipse(g, 8, 8.4, 5.6, 2.4, "i")
for (x, y) in [(5, 8), (7, 9), (10, 8)]:
    put(g, x, y, "j")
shape("pan", outline(g), {"i": "#3a1a0e", "j": "#c8a050"})
g = new()
for y in range(N):
    w = [0, 0, 2, 2, 3, 4, 5, 6, 6, 6, 6, 5, 5, 4, 3, 0][y]
    for x in range(8 - w, 8 + w):
        put(g, x, y, "#")
g = shade(g)
rect(g, 6, 2, 9, 2, "i")
g = outline(g)
rect(g, 5, 1, 10, 1, "o")
shape("jar", g, {"i": "#24140c"})
g = ellipse(new(), 8, 6.2, 4.6, 1.0, "q")
for (x, y) in [(6, 6), (9, 6)]:
    put(g, x, y, "k")
shape("pot_fill", g)
g = shade(poly(new(), [(1, 9), (11, 3), (15, 6), (5, 13)]))
ellipse(g, 13, 4.6, 2.0, 2.2, "e")
ellipse(g, 13, 4.6, 1.0, 1.1, "E")
for (x, y) in [(4, 10), (7, 8), (9, 7)]:
    put(g, x, y, "d")
shape("firewood", outline(g), {"e": "#c89a5a", "E": "#8a6232"})

# wand (stick + gem layers), fire bomb
g = shade(diag(2, 13, 8))
put(g, 8, 6, "r")
put(g, 9, 6, "r")
put(g, 9, 5, "r")
shape("wand_stick", outline(g), {"r": "#e0b040"})
shape("wand_gem", body(ellipse(new(), 11.5, 4.5, 2.7, 2.7)))
g = shade(ellipse(new(), 7.5, 10, 5, 5))
rect(g, 3, 9, 12, 9, "q")
g = compose(outline(g))
for y in range(N):
    for x in range(N):
        if g[y][x] == "q" and get(g, x, y - 1) == ".":
            g[y][x] = "o"
line(g, 9, 5, 11, 3, "r")
put(g, 12, 2, "Y")
put(g, 12, 1, "q")
put(g, 13, 2, "q")
shape("fire_bomb", g, {"q": "#ff7a1a", "r": "#8a6a40", "Y": "#ffe27a"})

# playtest gadgets (PlaytestTools): laser cutter, tape measure, sparkle over the instant tree
g = new()
line(g, 4, 10, 11, 3, "#", 3)      # the barrel
rect(g, 3, 10, 5, 14, "#")         # the grip
g = shade(g)
for (x, y) in [(12, 2), (13, 2), (12, 3), (13, 1)]:
    put(g, x, y, "r")
put(g, 14, 1, "R")
put(g, 15, 0, "R")
for (x, y) in [(7, 8), (8, 7), (9, 6)]:
    put(g, x, y, "r")              # the glowing coil along the barrel
shape("laser_cutter", outline(g), {"r": "#ff3a20", "R": "#ffd8a0"})
g = shade(rect(new(), 2, 4, 11, 13))
ellipse(g, 6.5, 8.5, 2.4, 2.4, "c")
put(g, 6, 8, "k")
g = outline(g)
line(g, 12, 12, 15, 12, "t")
line(g, 12, 13, 15, 13, "T")
shape("tape_measure", g, {"c": "#3a3a3a", "k": "#d0d0d0", "t": "#f4e870", "T": "#b8a830"})
shape("sparkle", art([
    "..y.............", ".yYy............", "..y..........y..", "............yYy.",
    ".............y..", "................", "................", "................",
    "................", "................", "................", "................",
    "................", "................", "................", "................"]), {"y": "#fff2a0", "Y": "#ffffff"})

# cube faces (drawn as an isometric cube in icons, a cube mesh in the world)
shape("bricks_face", bricks(new(), 0, 0, 15, 15), BRICK)
rnd = random.Random(7)
g = new()
for y in range(N):
    for x in range(N):
        v = rnd.random()
        g[y][x] = "h" if v < 0.04 else ("l" if v < 0.18 else ("d" if v > 0.7 else "m"))
shape("coal_block_face", g)
g = new()
for i in range(N):
    put(g, i, 0, "l")
    put(g, i, 15, "d")
    put(g, 0, i, "l")
    put(g, 15, i, "d")
for (x, y) in [(3, 2), (2, 3), (4, 2), (2, 4), (6, 3), (5, 4), (4, 5), (3, 6)]:
    put(g, x, y, "h")
shape("glass_face", g)

# spells: a card and one glyph per spell
g = rect(new(), 1, 1, 14, 14, "f")
rect(g, 2, 2, 13, 13, "g")
for i in range(2, 14):
    put(g, i, 13, "k")
    put(g, 13, i, "k")
    put(g, i, 2, "G")
    put(g, 2, i, "G")
shape("spell_card", outline(g), {"f": "#1c1410", "o": "#0a0806"})
gl = {}
g = new()
line(g, 4, 12, 8, 8, "b")
line(g, 5, 12, 8, 9, "b")
for (x, y) in [(10, 3), (10, 4), (10, 7), (10, 8), (7, 6), (8, 6), (11, 6), (12, 6), (9, 5), (11, 5), (9, 7), (11, 7)]:
    put(g, x, y, "a")
put(g, 10, 5, "c")
put(g, 10, 6, "c")
put(g, 9, 6, "c")
put(g, 11, 6, "c")
gl["spark"] = (g, {"a": "#ffc8ff", "b": "#d040e0", "c": "#ffffff"})
g = poly(new(), [(4, 11.5), (8, 5), (11, 8)], "c")
ellipse(g, 9.5, 6.5, 2.6, 2.6, "b")
ellipse(g, 9.5, 6.5, 1.3, 1.3, "a")
gl["fire"] = (g, {"a": "#ffe27a", "b": "#ff8a24", "c": "#c03010"})
g = new()
for ox in (4, 8):
    for i in range(3):
        put(g, ox + i, 5 + i, "a")
        put(g, ox + i, 11 - i, "a")
        put(g, ox + i + 1, 5 + i, "b")
        put(g, ox + i + 1, 11 - i, "b")
    put(g, ox + 3, 8, "a")
gl["speed"] = (g, {"a": "#e8ffff", "b": "#60c8e8"})
g = ellipse(new(), 7.5, 9, 3.2, 3.2, "b")
put(g, 6, 7, "a")
put(g, 6, 8, "a")
line(g, 9, 6, 11, 4, "c")
put(g, 12, 3, "d")
gl["bomb"] = (g, {"a": "#8a8a96", "b": "#1c1c22", "c": "#a08050", "d": "#ffe27a"})
g = poly(new(), [(4, 12), (6.5, 8), (11.5, 3.5), (8, 9.5)], "b")
line(g, 5, 11, 10, 5, "a")
gl["dig"] = (g, {"a": "#e0c890", "b": "#8a6a40"})
g = new()
for oy in (4, 9):
    line(g, 4, oy + 2, 9, oy + 2, "a")
    put(g, 10, oy + 2, "b")
    put(g, 9, oy + 1, "b")
    put(g, 9, oy + 3, "b")
gl["double"] = (g, {"a": "#d8ffd0", "b": "#70d860"})
g = new()
line(g, 5, 5, 11, 11, "b")
line(g, 5, 11, 11, 5, "b")
line(g, 8, 3, 8, 12, "a")
line(g, 4, 8, 12, 8, "a")
put(g, 8, 8, "c")
gl["freeze"] = (g, {"a": "#d8f6ff", "b": "#70c8f0", "c": "#ffffff"})
g = new()
for (x, y) in [(8, 3), (8, 13), (3, 8), (13, 8), (5, 5), (11, 5), (5, 11), (11, 11)]:
    put(g, x, y, "b")
ellipse(g, 8.5, 8.5, 3, 3, "a")
ellipse(g, 8.5, 8.5, 1.5, 1.5, "c")
gl["light"] = (g, {"a": "#ffd84a", "b": "#ffb020", "c": "#fffbe0"})
g = new()
for oy, ch in ((6, "a"), (10, "b")):
    for x in range(3, 13):
        put(g, x, oy + round(math.sin(x * 0.9) * 1.2), ch)
gl["water"] = (g, {"a": "#a8dcff", "b": "#3a86e0"})
for k, (g, cols) in gl.items():
    shape("glyph_" + k, g, cols)

# farming and animals (ADR 0022)
g = new()
for (cx, cy) in [(5, 6), (9, 4), (11, 8), (6, 10), (9, 11), (12, 12), (4, 13)]:
    ellipse(g, cx + 0.5, cy + 0.5, 1.4, 1.0, ang=-35)
shape("seeds", body(g))
g = new()
for (x0, x1) in [(6, 4), (7, 6), (8, 8), (9, 10), (10, 12)]:
    line(g, 8, 15, x1, 5, "s")
    put(g, x0, 15, "s")
for (hx, hy) in [(4, 5), (6, 4), (8, 3), (10, 4), (12, 5)]:
    ellipse(g, hx + 0.5, hy - 0.5, 1.1, 2.4, "#")
g = shade(g)
rect(g, 6, 10, 10, 11, "t")
shape("sheaf", outline(g), {"s": "#c8a840", "t": "#6a4a24"})
g = shade(poly(new(), [(3, 13.5), (9.5, 5.5), (12, 8), (4.5, 14.5)]))
for (x, y) in [(6, 11), (8, 9), (9, 8)]:
    put(g, x, y, "d")
leaves = compose(line(new(), 10, 6, 11, 1, "q"), line(new(), 11, 6, 14, 2, "q"), line(new(), 11, 7, 15, 5, "Q"))
shape("carrot", compose(outline(g), outline(leaves)), {"q": "#4e9a2c", "Q": "#2e6a1a"})
g = shade(compose(ellipse(new(), 8, 9.5, 6.6, 3.8), rect(new(), 2, 9, 13, 12)))
for (x, y) in [(5, 7), (6, 8), (8, 7), (9, 8), (11, 7), (12, 8)]:
    put(g, x, y, "d")
shape("bread", outline(g))
# pumpkin pie: a round crust seen from above at an angle, orange filling, a lattice
g = shade(ellipse(new(), 8, 9.5, 6.6, 4.2))
f = ellipse(new(), 8, 9.0, 5.0, 3.0, "f")
for (x, y) in [(5, 8), (8, 7), (11, 8), (6, 10), (9, 10)]:
    put(f, x, y, "F")
shape("pie", outline(compose(g, f)), {"f": "#e08a2a", "F": "#c06a18"})
# wildlife (ADR 0027): egg, fried egg, feather, bone, hide
shape("egg", outline(shade(ellipse(new(), 8, 9, 4.2, 5.4))))
g = ellipse(new(), 8, 9, 6.4, 5.0, "w")
for (x, y) in [(4, 6), (12, 7), (5, 12), (11, 12)]:
    put(g, x, y, ".")
y = shade(ellipse(new(), 8.5, 8.5, 2.4, 2.4))
shape("fried_egg", outline(compose(g, y)), {"w": "#f6f4ec"})
g = new()
line(g, 3, 13, 12, 3, "s")
for k in range(1, 8):
    line(g, 4 + k, 12 - k, 4 + k - 2, 12 - k - 3, "#")
    line(g, 4 + k, 12 - k, 4 + k + 3, 12 - k + 1, "#")
shape("feather", outline(shade(g)), {"s": "#8a8070"})
g = compose(line(new(), 4, 12, 11, 4, "#", 2), ellipse(new(), 3.5, 12.5, 1.8, 1.8), ellipse(new(), 5, 13.5, 1.6, 1.6),
            ellipse(new(), 11.5, 3.5, 1.8, 1.8), ellipse(new(), 12.5, 5, 1.6, 1.6))
shape("bone", outline(shade(g)))
g = shade(poly(new(), [(2, 5), (6, 3), (10, 3), (14, 5), (13, 10), (14, 14), (8, 12.5), (2, 14), (3, 10)]))
shape("hide", outline(g))
shape("hoe_head", body(mask({2: [(6, 13)], 3: [(6, 13)], 4: [(6, 7)], 5: [(6, 7)]})))   # Minecraft hoe: a bar over the handle end, bent down at the far side
g = compose(line(new(), 3, 3, 10, 10, "#", 2), line(new(), 12, 3, 5, 10, "#", 2))
g = shade(g)
ring = compose(ellipse(new(), 4, 12.5, 2.4, 2.4, "r"), ellipse(new(), 11.5, 12.5, 2.4, 2.4, "r"))
for (cx, cy) in [(4, 12.5), (11.5, 12.5)]:
    ellipse(ring, cx, cy, 1.1, 1.1, ".")
shape("shears", outline(compose(ring, g)), {"r": "#8a3a2a"})
# tongs (ADR 0087): two green-wood arms, spread at the grip, crossing at a cord lashing, jaws closed at the top right
g = shade(compose(line(new(), 1, 11, 13, 2, "#", 2), line(new(), 4, 14, 13, 3, "#", 2)))
lash = new()
rect(lash, 9, 4, 10, 6, "r")
shape("tongs", outline(compose(g, lash)), {"r": "#c8b078"})
shape("chair", outline(art([
    "................", "...d............", "...dl...........", "...dl...........",
    "...dl...........", "...dlll.........", "...dl...........", "...dl...........",
    "...dlwwwwwww....", "...dlmmmmmmmm...", "...dddddddddd...", "...d.......d....",
    "...d.......d....", "...d.......d....", "...d.......d....", "................"])), {"w": "#ecece4"})
# base building and decoration (ADR 0094): wood-palette shapes, iron (i), glass (g), flame (f), paint
shape("door", outline(art([
    "................", "....dddddddd....", "....dlmlmlmd....", "....dlmlmlmd....",
    "....dlmlmlmd....", "....dddddddd....", "....dlmlmlmd....", "....dlmlmiid....",
    "....dlmlmlmd....", "....dddddddd....", "....dlmlmlmd....", "....dlmlmlmd....",
    "....dlmlmlmd....", "....dlmlmlmd....", "....dddddddd....", "................"])), {"i": "#3a3c42"})
shape("trapdoor", outline(art([
    "................", "................", "................", "................",
    "..dddddddddddd..", "..dlllllllllld..", "..dmmmmmmmmmmd..", "..dddddddddddd..",
    "..dlllllllllld..", "..dmmmmmiimmmd..", "..dddddddddddd..", "................",
    "................", "................", "................", "................"])), {"i": "#3a3c42"})
shape("fence", outline(art([
    "................", "................", "..dd........dd..", "..dl........dl..",
    "..dlmmmmmmmmdl..", "..dllllllllldl..", "..dl........dl..", "..dl........dl..",
    "..dlmmmmmmmmdl..", "..dllllllllldl..", "..dl........dl..", "..dl........dl..",
    "..dl........dl..", "..dl........dl..", "..dd........dd..", "................"])))
shape("fence_gate", outline(art([
    "................", "................", "................", "..dd........dd..",
    "..dlmmmmmmmmdl..", "..dllllllllldl..", "..dl.....ll.dl..", "..dl...ll...dl..",
    "..dl.ll.....dl..", "..dlmmmmmmmmdl..", "..dllllllllldl..", "..dl........dl..",
    "..dl........dl..", "..dl........dl..", "..dd........dd..", "................"])))
shape("ladder", outline(art([
    "................", "....d......d....", "....dmmmmmmd....", "....dllllllld...",
    "....d......d....", "....d......d....", "....dmmmmmmd....", "....dllllllld...",
    "....d......d....", "....d......d....", "....dmmmmmmd....", "....dllllllld...",
    "....d......d....", "....d......d....", "....d......d....", "................"])))
shape("window", outline(art([
    "................", "................", "..dddddddddddd..", "..dgggghdggggd..",
    "..dghggggdgghgd..", "..dgggggdggggd..", "..dgggggdggggd..", "..dgggggdggggd..",
    "..dddddddddddd..", "..dgggghdggggd..", "..dghggggdgghgd.", "..dgggggdggggd..",
    "..dgggggdggggd..", "..dddddddddddd..", "..mmmmmmmmmmmm..", "................"])),
    {"g": "#a6d6e2", "h": "#eef8fc"})
shape("table", outline(art([
    "................", "................", "................", "................",
    "................", "..hlllllllllll..", "..mmmmmmmmmmmm..", "..dddddddddddd..",
    "...dd......dd...", "...dd......dd...", "...dd......dd...", "...dd......dd...",
    "...dd......dd...", "...dd......dd...", "................", "................"])))
shape("shelf", outline(art([
    "................", "..dddddddddddd..", "..drrbbgyrbbgd..", "..drrbbgyrbbgd..",
    "..drrbbgyrbbgd..", "..dllllllllllld.", "..dbgyyrrbgrrd..", "..dbgyyrrbgrrd..",
    "..dbgyyrrbgrrd..", "..dllllllllllld.", "..dyrrbgbbyrgd..", "..dyrrbgbbyrgd..",
    "..dyrrbgbbyrgd..", "..dllllllllllld.", "..dd........dd..", "................"])),
    {"r": "#a82228", "b": "#34489a", "g": "#46843a", "y": "#c8a840"})
shape("lantern", outline(art([
    "................", "......iiii......", "......i..i......", ".....iiiiii.....",
    "....iiiiiiii....", "....i.ffff.i....", "....i.fyyf.i....", "....i.fyyf.i....",
    "....i.fyyf.i....", "....i.ffff.i....", "....iiiiiiii....", ".....iiiiii.....",
    "................", "................", "................", "................"])),
    {"i": "#45474e", "f": "#ffb050", "y": "#fff0a0"})
shape("flower_pot", outline(art([
    "................", "......yry.......", ".....yrrry......", "......yry.......",
    ".......g........", "...gg..g..gg....", "....ggggggg.....", ".......g........",
    "...tttttttttt...", "...TTTTTTTTTT...", "....tttttttt....", "....tttttttt....",
    "....tttttttt....", ".....TTTTTT.....", "................", "................"])),
    {"y": "#e2c440", "r": "#c43a2a", "g": "#46843a", "t": "#b05a34", "T": "#8a3e22"})
shape("painting", outline(art([
    "................", "................", ".dddddddddddddd.", ".dsssssssssyysd.",
    ".dsssssssssyysd.", ".dssssssssssssd.", ".dsssgggsssssd..", ".dssgggggssgggd.",
    ".dgggggggggggggd", ".dgggggggggggggd", ".dwwwwwwwwwwwwd.", ".dwwwwwwwwwwwwd.",
    ".dddddddddddddd.", "................", "................", "................"])),
    {"s": "#80ace0", "y": "#e2c440", "g": "#46843a", "w": "#34489a"})

# ADR 0164: a page of paper with lines of writing, and a market stall
shape("page", outline(art([
    "................", "...pppppppppp...", "..pppppppppppp..", "..pkkkkkkkkppp..",
    "..pppppppppppp..", "..pkkkkkkkkkkp..", "..pppppppppppp..", "..pkkkkkkkpppp..",
    "..pppppppppppp..", "..pkkkkkkkkkpp..", "..pppppppppppp..", "..pkkkkkpppppp..",
    "..qqqqqqqqqqqq..", "................", "................", "................"])),
    {"o": "#4a3a2a", "p": "#f2e8cc", "q": "#c8b48a", "k": "#3a2a1a"})
shape("note_page", outline(art([
    "................", "...pppppppppp...", "..pppppppppppp..", "..pkkkkkkkkppp..",
    "..pppppppppppp..", "..pkkkkkkkkkkp..", "..pppppprrpppp..", "..pkkkkkrrkkpp..",
    "..pppppppppppp..", "..pkkkkkkkkkpp..", "..pppppppppppp..", "..pkkkkkpppppp..",
    "..qqqqqqqqqqqq..", "................", "................", "................"])),
    {"o": "#4a3a2a", "p": "#f2e8cc", "q": "#c8b48a", "k": "#3a2a1a", "r": "#b02a2a"})
shape("stall", outline(art([
    "................", "..rwrwrwrwrwrw..", ".rwrwrwrwrwrwr..", ".rrrrrrrrrrrrr..",
    "..d..........d..", "..d..gg..yy..d..", "..d..gg..yy..d..", "..d..........d..",
    "..tttttttttttt..", "..tllllllllllt..", "..tllllllllllt..", "..tttttttttttt..",
    "..d..........d..", "................", "................", "................"])),
    {"o": "#3a2a1a", "r": "#b83a2a", "w": "#f0e4c8", "d": "#6a4a2a", "g": "#d8a030", "y": "#7ac8f0", "t": "#8a5a30", "l": "#b8864a"})

# ---------- sprites ----------
SP = {}


def sp(item, *layers, cube=False):
    ls = []
    for l in layers:
        if isinstance(l, str):
            l = (l,)
        e = {"shape": l[0]}
        if len(l) > 1 and l[1]:
            e["palette"] = l[1]
        if len(l) > 2:
            e["colors"] = l[2]
        ls.append(e)
    SP[item] = {"layers": ls}
    if cube:
        SP[item]["cube"] = True


H = ("handle", "handle")
SH = ("short_handle", "handle")
for m in ("copper", "tin", "bronze", "iron", "steel"):
    sp(m + "_ingot", ("ingot", m))
for m in ("copper", "tin", "iron"):
    sp(m + "_nugget", ("nugget", m))
for m in ("wood", "stone", "flint", "copper", "bronze", "iron", "steel"):
    sp({"wood": "wooden"}.get(m, m) + "_pickaxe", H, ("pickaxe_head", m))
for m in ("copper", "bronze", "iron", "steel"):
    sp(m + "_pickaxe_head", ("pickaxe_head", m))
sp("flint_hatchet", H, ("axe_head", "flint"))
for m in ("copper", "bronze", "iron"):
    sp(m + "_axe", H, ("axe_head", m))
    sp(m + "_axe_head", ("axe_head", m))
    sp(m + "_shovel", H, ("shovel_head", m))
    sp(m + "_shovel_head", ("shovel_head", m))
    sp(m + "_saw", ("saw_handle", "handle"), ("saw_blade", m))
    sp(m + "_saw_head", ("saw_blade", m))
    sp(m + "_hammer", H, ("hammer_head", m))
    sp(m + "_hammer_head", ("hammer_head", m))
sp("flint_shovel", H, ("shovel_head", "flint"))
sp("stone_hammer", H, ("hammer_head", "stone"))
sp("deconstruction_hammer", H, ("hammer_head", "stone"))
for m in ("flint", "copper", "iron"):
    sp(m + "_knife", SH, ("knife_blade", m))
for m in ("copper", "iron"):
    sp(m + "_knife_head", ("knife_blade", m))
sp("copper_saw_blade", ("saw_blade", "copper"))   # smithed (ADR 0042)
# the steel set (ADR 0121): tools and their smithed heads
for kind, head in (("axe", "axe_head"), ("shovel", "shovel_head"), ("hammer", "hammer_head"), ("hoe", "hoe_head")):
    sp("steel_" + kind, H, (head, "steel"))
    sp("steel_" + head, (head, "steel"))
sp("steel_saw", ("saw_handle", "handle"), ("saw_blade", "steel"))
sp("steel_saw_blade", ("saw_blade", "steel"))
sp("steel_knife", SH, ("knife_blade", "steel"))
sp("steel_knife_head", ("knife_blade", "steel"))
sp("bronze_spear_head", ("spear_head", "bronze"))
sp("iron_spear_head", ("spear_head", "iron"))
sp("steel_spear_head", ("spear_head", "steel"))
sp("copper_work_piece", ("work_piece", "copper"))
sp("copper_chisel", SH, ("chisel_blade", "copper"))
sp("copper_chisel_head", ("chisel_blade", "copper"))
sp("copper_prospecting_pick", H, ("prospecting_head", "copper"))
sp("copper_prospecting_pick_head", ("prospecting_head", "copper"))

# Vintage Story early game (ADR 0023): flint heads, hoes, molds, nuggets in the crucible
for k, sh in (("knife_blade", "knife_blade"), ("axe_head", "axe_head"), ("shovel_head", "shovel_head"),
              ("hoe_head", "hoe_head"), ("pickaxe_head", "pickaxe_head")):
    sp("flint_" + k, (sh, "flint"))
sp("flint_axe", H, ("axe_head", "flint"))
sp("flint_hoe", H, ("hoe_head", "flint"))
sp("copper_hoe", H, ("hoe_head", "copper"))
sp("copper_hoe_head", ("hoe_head", "copper"))
for t, head in (("axe", "axe_head"), ("pickaxe", "pickaxe_head"), ("shovel", "shovel_head"),
                ("knife", "knife_blade"), ("hoe", "hoe_head"), ("hammer", "hammer_head")):
    shape(t + "_mold", mold_of(head), {"i": "#3a1a0e", "I": "#24100a"})
    sp(t + "_mold", (t + "_mold", "terracotta"))
    sp("raw_" + t + "_mold", (t + "_mold", "clay", {"i": "#5a606a", "I": "#3a3e46"}))
sp("crucible_copper", ("crucible", "terracotta", {"i": "#ffa030"}))
sp("dry_grass", ("fiber", "straw"))
sp("firestarter", ("firestarter", "handle"))
sp("firepit", ("campfire", "handle"))
sp("stick", ("stick", "handle"))
sp("bark", ("bark", "bark"))
sp("plant_fiber", ("fiber", "fiber"))
sp("twine", ("twine", "twine"))
sp("flint", ("flint_shard", "flint"))
sp("loose_stone", ("rock", "stone"))
sp("loose_copper_ore", ("ore_rock", "stone", {"s": "#3fa060", "S": "#e08a55"}))
sp("loose_tin_ore", ("ore_rock", "stone", {"s": "#2a2018", "S": "#b0a090"}))
sp("loose_iron_ore", ("ore_rock", "stone", {"s": "#9a4a2a", "S": "#c8784a"}))
sp("coal", ("lump", "coal"))
sp("charcoal", ("charcoal_lump", "charcoal"))
sp("clay_ball", ("ball", "clay"))
sp("fire_clay", ("ball", "fire_clay"))
sp("clay_brick", ("brick", "terracotta"))
sp("raw_clay_brick", ("brick", "clay"))
sp("rot", ("ball", "rot"))
sp("fire_brick", ("brick", "fire_brick"))
sp("iron_bloom", ("bloom", "iron"))
sp("mana_dust", ("dust_pile", "mana"))
sp("mana_crystal", ("crystal", "mana"))
sp("fire_essence", ("flame_drop", "fire"))
sp("blueberry", ("berries", "blueberry"))
sp("cranberry", ("berries", "cranberry"))
sp("raw_meat", ("steak", "raw_meat"))
sp("cooked_meat", ("steak", "cooked_meat", {"f": "#d8b080", "b": "#e8dcc4"}))
sp("clay_bowl", ("bowl", "terracotta"))
sp("raw_clay_bowl", ("bowl", "clay", {"i": "#3a3e46"}))
sp("meat_stew", ("bowl", "terracotta"), ("bowl_fill", None, {"q": "#7a4a22", "k": "#c87a4a"}))
sp("bucket", ("bucket", "iron"))
sp("water_bucket", ("bucket", "iron"), ("bucket_fill", None, {"q": "#2e64c8", "Q": "#7ab0f0"}))
sp("lava_bucket", ("bucket", "iron"), ("bucket_fill", None, {"q": "#e8541a", "Q": "#ffd060"}))
sp("oil_bucket", ("bucket", "iron"), ("bucket_fill", None, {"q": "#5a3a12", "Q": "#e0b040"}))   # brown oil with an amber sheen (black read as obsidian)
sp("toxic_sludge_bucket", ("bucket", "iron"), ("bucket_fill", None, {"q": "#60c42c", "Q": "#b0fa60"}))
sp("sulfur", ("dust_pile", "sulfur"))
# HUD status icons for the player's stains (PlayerStains), not items
sp("stain_wet", ("flame_drop", "water"))
sp("stain_bloody", ("flame_drop", "blood"))
sp("stain_oily", ("flame_drop", "oil"))
sp("stain_toxic", ("flame_drop", "toxic"))
sp("stain_burning", ("flame_drop", "fire"))
sp("glass_flask", ("flask", "glass"))
sp("crucible", ("crucible", "terracotta"))
sp("raw_crucible", ("crucible", "clay", {"i": "#3a3e46"}))
sp("ingot_mold", ("ingot_mold", "terracotta"))
sp("raw_ingot_mold", ("ingot_mold", "clay", {"i": "#5a606a", "I": "#3a3e46"}))
sp("cooking_pot", ("cooking_pot", "terracotta"))
sp("raw_cooking_pot", ("cooking_pot", "clay"))
sp("anvil", ("anvil", "bronze"))
sp("stone_anvil", ("stone_anvil", "stone"))
sp("blast_furnace", "blast_furnace")
sp("bloomery", ("bloomery", "terracotta"))
sp("forge", "forge")
sp("campfire", ("campfire", "handle"))
sp("sieve", ("sieve", "wood"))
sp("straw_bed", ("bed", "straw"))
sp("bed", ("bed", "blanket"))
sp("chair", ("chair", "wood"))
for k in ("door", "trapdoor", "fence", "fence_gate", "ladder", "window", "table", "shelf", "lantern", "flower_pot", "painting"):
    sp(k, (k, "wood"))
# ADR 0142: mine supports, a post with a cap under the roof
shape("mine_post", outline(art([
    "................", ".hhhhhhhhhhhhhh.", ".mmmmmmmmmmmmmm.", ".....dlmmd......",
    ".....dlmmd......", ".....dmmmd......", ".....dlmmd......", ".....dlmmd......",
    ".....dmmmd......", ".....dlmmd......", ".....dlmmd......", ".....dmmmd......",
    ".....dlmmd......", "....dlllmmd.....", "....dddddddd....", "................"])))
shape("mine_pillar", outline(art([
    "................", "...dddddddddd...", "...mhhhhhhhhm...", "....mllllllm....",
    "....mlmmmmlm....", "....dddddddd....", "....mllllllm....", "....mlmmmmlm....",
    "....dddddddd....", "....mllllllm....", "....mlmmmmlm....", "....dddddddd....",
    "....mllllllm....", "...mhhhhhhhhm...", "...dddddddddd...", "................"])))
sp("timber_prop", ("mine_post", "wood"))
sp("stone_pillar", ("mine_pillar", "stone"))
sp("iron_beam", ("mine_post", "iron"))
# ADR 0121: a bench, a candle, and the plank furniture in every wood. Wood ramps per ADR 0129: shadows
# cooler and more saturated, highlights warmer; each matches its planks' tile (176..179).
shape("bench", outline(art([
    "................", "................", "................", "................",
    "................", "................", "hlllllllllllllll", "lmmmmdmmmmmdmmmm",
    "dddddddddddddddd", ".dd..........dd.", ".dm..........dm.", ".dmddddddddddmd.",
    ".dm..........dm.", ".dm..........dm.", ".dd..........dd.", "................"])))
shape("candle", outline(art([
    "................", "................", ".......f........", "......fYf.......",
    "......fYf.......", ".......f........", ".......t........", "......hll.......",
    "......lmm.......", "......lmm.......", "......lmd.......", "......lmd.......",
    "......mmd.......", "...iiiiiiiii....", "....iiiiiii.....", "................"])),
    {"f": "#ff8a24", "Y": "#ffe27a", "t": "#2a2018", "i": "#4a4c54"})
PALETTES["wax"] = ramp("#7a6040", "#c8b48a", "#e4d4ac", "#f2e8cc", "#fff8e6")
PALETTES["spruce_wood"] = ramp("#22161a", "#4a3226", "#6e4c32", "#8a6440", "#a88252")
PALETTES["acacia_wood"] = ramp("#4a1a1e", "#8a3e26", "#b85e34", "#d27c44", "#ec9e5c")
PALETTES["jungle_wood"] = ramp("#3a1a26", "#7a4040", "#9e6252", "#b87c62", "#d29c7a")
PALETTES["birch_wood"] = ramp("#5a5040", "#b0a07c", "#d0c09a", "#e4d6b2", "#f6ecce")
sp("bench", ("bench", "wood"))
sp("candle", ("candle", "wax"))
shape("sign", outline(art([
    "................", "................", "................", "................",
    ".dddddddddddddd.", ".dllllllllllllm.", ".dlmmdmmddmdmlm.", ".dllllllllllllm.",
    ".dddddddddddddd.", ".dllllllllllllm.", ".dlmddmdmmddmlm.", ".dllllllllllllm.",
    ".dmmmmmmmmmmmmm.", "................", "................", "................"])))
sp("sign", ("sign", "wood"))
sp("paper", ("page", None))
sp("note", ("note_page", None))
sp("trading_post", ("stall", None))
# rugs (ADR 0121): a rug seen from above at a slant, fringe at both ends (ADR 0129 ramps: cool shadows, warm lights)
shape("rug", outline(art([
    "................", "................", "................", "...h.h.h.h.h.h..",
    "..dddddddddddd..", "..dmmmmmmmmmmd..", "..dmmmmlmmmmmd..", "..dmmmlhlmmmmd..",
    "..dmmlhhhlmmmd..", "..dmmmlhlmmmmd..", "..dmmmmlmmmmmd..", "..dmmmmmmmmmmd..",
    "..dddddddddddd..", "...h.h.h.h.h.h..", "................", "................"])))
shape("mat", outline(art([
    "................", "................", "................", "................",
    "..dlmlmlmlmlmd..", "..lmlmlmlmlmld..", "..mlmlmlmlmlmd..", "..lmlmlmlmlmld..",
    "..mlmlmlmlmlmd..", "..lmlmlmlmlmld..", "..mlmlmlmlmlmd..", "..dddddddddddd..",
    "................", "................", "................", "................"])))
PALETTES["red_rug"] = ramp("#3a0e22", "#7a1a2a", "#b82c30", "#d85a40", "#f2e0c4")
PALETTES["blue_rug"] = ramp("#0e1430", "#1e2c66", "#34489a", "#5a74c0", "#f0eadc")
sp("red_rug", ("rug", "red_rug"))
sp("blue_rug", ("rug", "blue_rug"))
sp("straw_mat", ("mat", "straw"))
for w in ("spruce", "acacia", "jungle", "birch"):
    for k in ("door", "trapdoor", "fence", "fence_gate", "table", "bench", "shelf", "sign", "window"):
        sp(w + "_" + k, (k, w + "_wood"))
sp("wheat_seeds", ("seeds", "seed"))
sp("wheat", ("sheaf", "straw"))
sp("carrot", ("carrot", "carrot"))
# more crops (ADR 0121, ADR 0129 ramps: shadows cooler, highlights warmer)
shape("potato", outline(art([
    "................", "................", "................", "................",
    ".....mmmmm......", "...mmlllhlmm....", "..mllhhlllmmm...", "..mlhlldllmmmd..",
    ".mmlllllmmmdmd..", ".mmlmdmmmmmmdd..", ".dmmmmmmmmmdd...", "..ddmmmmdddd....",
    "....dddddd......", "................", "................", "................"])))
shape("onion", outline(art([
    "........q.......", ".......qQ.......", ".......Qq.......", "........m.......",
    ".......mlm......", "......mlhlm.....", ".....mllhllm....", "....mllhlllmd...",
    "....mlhllllmd...", "....mllllmmmd...", ".....mmmmmmd....", "......dmmdd.....",
    ".......hh.......", "......h..h......", "................", "................"])), {"q": "#5a9a30", "Q": "#2e5e1e"})
shape("beans", outline(art([
    "................", "................", "................", "................",
    "....mmm.........", "...mlhlm..mmm...", "...mllmd.mlhlm..", "....mdd..mllmd..",
    "..........mdd...", ".....mmm........", "....mlhlm.......", "....mllmd.......",
    ".....mdd........", "................", "................", "................"])))
PALETTES["potato"] = ramp("#3a2630", "#7a5a40", "#b08a5a", "#d2ac74", "#f0d498")
PALETTES["baked_potato"] = ramp("#2a1820", "#5e3a26", "#8a5a34", "#b07c44", "#f2d27a")
PALETTES["onion"] = ramp("#3a1a24", "#7a3a2a", "#b46a3a", "#d6964e", "#f4d08a")
PALETTES["beans"] = ramp("#2a0e1a", "#5e1c22", "#94322e", "#bc5a40", "#e89a6a")
sp("potato", ("potato", "potato"))
sp("baked_potato", ("potato", "baked_potato"))
sp("onion", ("onion", "onion"))
sp("beans", ("beans", "beans"))
sp("bread", ("bread", "bread"))
sp("pumpkin_pie", ("pie", "bread"))
sp("egg", ("egg", "egg"))
sp("fried_egg", ("fried_egg", "carrot"))
sp("feather", ("feather", "feather"))
sp("bone", ("bone", "bone"))
sp("bear_hide", ("hide", "hide"))
sp("raw_hide", ("hide", "hide"))
sp("scraped_hide", ("hide", "scraped_hide"))
sp("leather", ("hide", "leather"))
sp("leather_strip", ("strips", "leather"))
sp("leather_bag", ("bag", "leather"))
sp("cordage", ("twine", "twine"))
sp("flint_spear_head", ("spear_head", "flint"))
sp("flint_spear", ("spear_shaft", "handle"), ("spear_tip", "flint"))
for m in ("bronze", "iron", "steel"):   # ADR 0121
    sp(m + "_spear", ("spear_shaft", "handle"), ("spear_tip", m))
sp("pan", ("pan", "terracotta"))
sp("raw_pan", ("pan", "clay", {"i": "#3a3e46", "j": "#5a606a"}))
sp("storage_pot", ("jar", "terracotta"))
sp("raw_storage_pot", ("jar", "clay", {"i": "#3a3e46"}))
sp("pot_of_stew", ("cooking_pot", "terracotta"), ("pot_fill", None, {"q": "#7a4a22", "k": "#d07a30"}))
sp("stew", ("bowl", "terracotta"), ("bowl_fill", None, {"q": "#7a4a22", "k": "#d07a30"}))
# cooking-pot meals (ADR 0121): the pot and the bowl, filled with the meal's colours
for meal, q, k in (("porridge", "#c8a874", "#efdcae"), ("fish_soup", "#a8a070", "#e8d8a0"),
                   ("mushroom_soup", "#6a4a30", "#b08a62"), ("berry_compote", "#6a1e48", "#c03c5a"), ("vegetable_soup", "#7a5a1e", "#c89a3a")):
    sp("pot_of_" + meal, ("cooking_pot", "terracotta"), ("pot_fill", None, {"q": q, "k": k}))
    sp(meal, ("bowl", "terracotta"), ("bowl_fill", None, {"q": q, "k": k}))
sp("bowl_of_milk", ("bowl", "terracotta"), ("bowl_fill", None, {"q": "#d4d0c6", "k": "#fbf8f0"}))   # goats (ADR 0120)
sp("milk_porridge", ("bowl", "terracotta"), ("bowl_fill", None, {"q": "#d2bc8e", "k": "#f6ecd2"}))
sp("daub", ("ball", "daub"))
sp("firewood", ("firewood", "wood"))
for m, item in (("wood", "wooden_hoe"), ("stone", "stone_hoe"), ("iron", "iron_hoe")):
    sp(item, H, ("hoe_head", m))
sp("shears", ("shears", "iron"))
sp("tongs", ("tongs", "wood"))
sp("torch", ("torch", "handle"))
sp("oak_sapling", ("sapling", "leaf"))
# the other woods' saplings (ADR 0112): their leaf colour, their bark on the stem
PALETTES["spruce_leaf"] = ramp("#0c2014", "#1e4028", "#2c5a36", "#3e7448", "#5e9468")
PALETTES["acacia_leaf"] = ramp("#262a0c", "#4a5420", "#68742e", "#86923e", "#acb85e")
PALETTES["jungle_leaf"] = ramp("#0e3010", "#226a20", "#2e8a2a", "#44a83a", "#78d060")
PALETTES["birch_leaf"] = ramp("#22340e", "#4a6a22", "#68903a", "#86ae4e", "#b0d478")
# blocks that showed as a plain cube of their tile: wattle (woven withies), carpet (a rug), the ruin pot
g = new()
for x in (3, 7, 11):
    rect(g, x, 1, x + 1, 14)
for y in (4, 8, 12):
    rect(g, 1, y, 14, y + 1)
shape("wattle", outline(shade(g)))
sp("wattle", ("wattle", "handle"))
g = shade(rect(new(), 1, 6, 14, 11))
for x in range(1, 15, 2):
    put(g, x, 5, "f")
    put(g, x, 12, "f")
shape("carpet", outline(g), {"f": "#d8d2c0"})
PALETTES["wool"] = ramp("#6a665e", "#c4beb0", "#dcd6c8", "#ece8dc", "#fbf9f2")
sp("carpet", ("carpet", "wool"))
sp("clay_pot", ("jar", "fire_brick", {"i": "#3a2a1a"}))
for k, stem in (("spruce", "#3a2614"), ("acacia", "#76665a"), ("jungle", "#5a4420"), ("birch", "#d8d4c6")):
    sp(k + "_sapling", ("sapling", k + "_leaf", {"t": stem}))
sp("dandelion", ("flower_stem", "leaf"), ("flower_head", None, {"p": "#f4d23c", "c": "#d08a1c"}))
sp("poppy", ("flower_stem", "leaf"), ("flower_head", None, {"p": "#d8302a", "c": "#2a1a14"}))
sp("cornflower", ("flower_stem", "leaf"), ("flower_head", None, {"p": "#4a72e0", "c": "#24306a"}))
sp("oxeye_daisy", ("flower_stem", "leaf"), ("flower_head", None, {"p": "#f4f2ea", "c": "#e8b820"}))
sp("allium", ("flower_stem", "leaf"), ("allium_head", None, ramp("#3a1450", "#6a2a8a", "#9446b8", "#b46ad4", "#dca8f0")))
sp("wand_basic", ("wand_stick", "handle"), ("wand_gem", "gem"))
sp("fire_bomb", ("fire_bomb", "bomb"))
sp("laser_cutter", ("laser_cutter", "steel"))
sp("crater_rod", ("wand_stick", "iron"), ("wand_gem", "lava"))
sp("sounding_rod", ("wand_stick", "iron"), ("wand_gem", "steel"))
sp("water_balloon", ("fire_bomb", "water", {"q": "#c0e0ff"}))
sp("sand_bomb", ("fire_bomb", "sand", {"q": "#8a6a3a"}))
sp("lava_bomb", ("fire_bomb", "lava", {"q": "#ffe27a"}))
sp("instant_tree", ("sapling", "leaf"), "sparkle")
sp("tape_measure", ("tape_measure", "hazard"))
sp("bricks", "bricks_face", cube=True)
sp("coal_block", ("coal_block_face", "coal"), cube=True)
sp("glass", ("glass_face", "glass"), cube=True)

# ---------- part C of the 2026-10-25 playtest (ADR 0053) ----------
# Ore blocks like Minecraft's 1.17+ ores (bug report 02-21-05): grey stone with cracks, and chunky ore
# clusters that each have a dark rim, a body, a lit top-left and a bright glint. The cube shows in the
# inventory, in the hand and on the ground; the nuggets in the rock keep their terrain tile.
STONE_FACE = {"s": "#6f6f6c", "t": "#7f7f7c", "u": "#8e8e8a", "v": "#9d9d98", "w": "#5c5c5a", "x": "#adada7"}


def ore_face(seed, clusters, accent=None):
    rnd = random.Random(seed)
    g = new()
    for y in range(N):
        for x in range(N):
            v = rnd.random()
            g[y][x] = "x" if v < 0.05 else ("v" if v < 0.25 else ("u" if v < 0.6 else ("t" if v < 0.85 else "s")))
    # a few dark cracks, two or three pixels long
    for _ in range(5):
        x, y = rnd.randrange(N), rnd.randrange(N)
        for k in range(rnd.randint(2, 3)):
            put(g, x + k, y + (k if rnd.random() < 0.5 else 0), "w")
    for (cx, cy, kind) in clusters:
        blob = {(0, 0), (1, 0), (0, 1), (1, 1)}
        blob |= set([[(2, 0), (2, 1)], [(0, 2), (1, 2)], [(-1, 1), (2, 0)], [(1, -1), (2, 1)]][kind % 4])
        for (dx, dy) in blob:
            put(g, cx + dx, cy + dy, "m")
        # rim: right and bottom neighbours outside the blob get the dark ore edge
        for (dx, dy) in blob:
            for (ex, ey) in ((1, 0), (0, 1), (1, 1)):
                if (dx + ex, dy + ey) not in blob:
                    put(g, cx + dx + ex, cy + dy + ey, "o")
        top = min(blob, key=lambda q: (q[1], q[0]))
        put(g, cx + top[0], cy + top[1], "h")
        for (dx, dy) in blob:
            if (dx - 1, dy) not in blob and (dx, dy) != top:
                put(g, cx + dx, cy + dy, "l")
                break
        for (dx, dy) in blob:
            if (dx + 1, dy) not in blob and (dx, dy + 1) not in blob:
                put(g, cx + dx, cy + dy, "d")
        if accent:
            put(g, cx + 1, cy + 1, accent)
    return g


ORE_SPOTS = [(1, 1, 0), (9, 2, 1), (5, 6, 2), (12, 7, 3), (2, 10, 1), (8, 11, 0), (12, 13, 2)]
for item, seed in (("iron_ore", 171), ("copper_ore", 181), ("tin_ore", 191)):
    spots = [(x, y, k) for (x, y, k) in ORE_SPOTS]
    random.Random(seed).shuffle(spots)
    shape(item + "_face", ore_face(seed, spots[:6], "g" if item == "copper_ore" else None),
          dict(STONE_FACE, **({"g": "#3fa88a"} if item == "copper_ore" else {})))
PALETTES["iron_ore"] = ramp("#7a4a34", "#b8805e", "#d8a684", "#ecc4a4", "#fff0e2")       # raw iron: peach
PALETTES["copper_ore"] = ramp("#6a2a10", "#b0501e", "#e07434", "#f49a58", "#ffd2a8")     # copper: orange, teal spots
PALETTES["tin_ore"] = ramp("#0e0a08", "#24180f", "#3e2c20", "#5e4634", "#d8d2c0")        # cassiterite: black-brown, a pale glint
for item in ("iron_ore", "copper_ore", "tin_ore"):
    sp(item, (item + "_face", item), cube=True)

# clutter in the hand and on the ground (bug report 02-28-54: a mushroom was a cube)
shape("mushroom", outline(compose(
    shade(ellipse(new(), 8.0, 7.0, 6.2, 3.6), "#", "l", "m", "d", "h"),
    art(["................", "................", "................", "................",
         "................", "................", "................", "................",
         "................", "......SsS.......", "......SsS.......", "......SsT.......",
         "......SsT.......", ".....SSsTT......", "................", "................"]))),
    {"S": "#e8e0cc", "s": "#d4cab2", "T": "#b4a88e"})
g = shade(ellipse(new(), 8.0, 6.5, 6.6, 4.4), "#", "l", "m", "d", "h")
for (x, y) in [(5, 4), (6, 4), (10, 5), (11, 5), (7, 7), (3, 7), (12, 8), (9, 3)]:
    put(g, x, y, "w")
rect(g, 1, 9, 14, 10, ".")
for y in range(9, 14):
    for x in (7, 8):
        put(g, x, y, "S")
put(g, 6, 13, "S")
put(g, 9, 13, "T")
put(g, 9, 11, "T")
put(g, 9, 12, "T")
shape("red_mushroom", outline(g), {"S": "#ece4d0", "T": "#b8ac92", "w": "#fbf6ea"})
g = new()
for y in range(1, 15):
    x = 7 + round(1.4 * math.sin(y * 0.8))
    put(g, x, y, "m")
    put(g, x + 1, y, "d")
    if y % 4 == 2:
        put(g, x - 1, y, "l")
        put(g, x - 2, y - 1, "l")
    if y % 4 == 0:
        put(g, x + 2, y, "m")
        put(g, x + 3, y - 1, "l")
for (x, y) in [(6, 4), (10, 8), (5, 12)]:
    put(g, x, y, "b")
shape("kelp", outline(g), {"b": "#b5a253"})
g = new()
for y in range(4, 14):
    half = max(1, min(6, (y - 3)))
    for x in range(8 - half, 8 + half):
        put(g, x, y, "d" if x % 2 else "m")   # ribs
for x in range(5, 11):
    put(g, x, 14, "d")
for y in range(4, 14):
    put(g, 8 - max(1, min(6, y - 3)), y, "l")
put(g, 8, 4, "h")
put(g, 7, 4, "h")
shape("seashell", outline(g))
PALETTES["mushroom_brown"] = ramp("#3a2414", "#7a5131", "#a8744a", "#c08a5c", "#efd7b0")
PALETTES["mushroom_red"] = ramp("#4a0c0c", "#a51c1c", "#c42828", "#e04848", "#f06a5a")
PALETTES["kelp"] = ramp("#1f2a0e", "#4c5d22", "#6f7f33", "#9aa34e", "#c8c070")
PALETTES["shell"] = ramp("#8a5a4a", "#c98f7c", "#e8bdaa", "#f8e2d6", "#fff3ea")
sp("brown_mushroom", ("mushroom", "mushroom_brown"))
sp("red_mushroom", ("red_mushroom", "mushroom_red"))
sp("kelp", ("kelp", "kelp"))
sp("seashell", ("seashell", "shell"))
CARD = {"projectile": "#5a3428", "modifier": "#24406e", "multicast": "#2a5a30", "utility": "#6a5620"}
for item, glyph, kind in [("spell_spark_bolt", "spark", "projectile"), ("spell_fire_bolt", "fire", "projectile"),
                          ("spell_bomb", "bomb", "projectile"), ("spell_digging_bolt", "dig", "projectile"),
                          ("spell_speed_up", "speed", "modifier"), ("spell_freeze_charge", "freeze", "modifier"),
                          ("spell_water_trail", "water", "modifier"), ("spell_double_cast", "double", "multicast"),
                          ("spell_light", "light", "utility")]:
    bg = CARD[kind]
    sp(item, ("spell_card", None, {"g": bg, "G": tint(bg, 1.35), "k": tint(bg, 0.7)}), "glyph_" + glyph)

# part G (ADR 0061): the flint prospecting pick and the loose heads (the copper pick is above)
sp("flint_prospecting_pick", H, ("prospecting_head", "flint"))
for m in ("flint", "copper"):
    sp(m + "_prospecting_head", ("prospecting_head", m))


# part D (bug reports 2026-10-25, ADR 0056): flint and steel, the world vacuum
shape("flint_and_steel", outline(art([
    "................", "................", "...hlll.........", "..lmmmmd........",
    "..lm...md.......", "..lm....d.......", "..lm....d.......", "..lmd...........",
    "...dd......ff...", "..........fFFf..", ".........fFkFFf.", "........fFkkFFf.",
    "........fFkkkFf.", ".........ffffff.", "................", "................"])),
    {"f": "#2a2a34", "F": "#5a5a6e", "k": "#8e8ea8"})
shape("world_vacuum", outline(art([
    "................", "...........tT...", "..........tTd...", ".........tTd....",
    "........tTd.....", "..lll..tTd......", ".lmmmltTd.......", "lmmmmmmd........",
    "lmyymmmd........", "lmyymmmd........", "lmmmmmmd........", "lmmmmmmd........",
    ".dddddd.........", "..g...g.........", "................", "................"])),
    {"t": "#9ca6b2", "T": "#d0d8e2", "y": "#f2d84a", "g": "#1e1e22"})
sp("flint_and_steel", ("flint_and_steel", "steel"))
sp("world_vacuum", ("world_vacuum", "hazard"))

# TNT and the blasting igniter (bug report 2026-10-25_21-10-33): a block face (red sticks, a paper
# band with "TNT") and a plunger box with a fuse wire
shape("tnt_face", art([
    "dRRdRRRdRRRdRRRd", "dRrdRRrdRRrdRRrd", "dRrdRRrdRRrdRRrd", "dRrdRRrdRRrdRRrd",
    "wwwwwwwwwwwwwwww", "wkkkwkwwkwkkkwww", "wwkwwkkwkwwkwwww", "wwkwwkwkkwwkwwww",
    "wwkwwkwwkwwkwwww", "wwwwwwwwwwwwwwww", "dRrdRRrdRRrdRRrd", "dRrdRRrdRRrdRRrd",
    "dRrdRRrdRRrdRRrd", "dRrdRRrdRRrdRRrd", "dRrdRRrdRRrdRRrd", "dddddddddddddddd"]),
    {"R": "#d83a2a", "r": "#b02a1e", "d": "#6e140e", "w": "#ece6d6", "k": "#26201c"})
shape("igniter", outline(art([
    "................", "....hhhhhhh.....", "......lm........", "......lm........",
    "......lm........", "......lm........", "..llllllllll....", "..lmmmmmmmmd....",
    "..lmRmmmmmmd.ww.", "..lmmmmmmmmdw..w", "..lmmmmmmmmd...w", "..lmmmmmmmmd..w.",
    "..dddddddddd.w..", "..............w.", "...............w", "................"])),
    {"R": "#e03a2a", "w": "#c88a3a"})
sp("tnt", ("tnt_face", None), cube=True)
sp("igniter", ("igniter", "iron"))

import item_sprites_e  # noqa: E402  rock kinds, clutter, fish (ADR 0057, 0058)
item_sprites_e.add(globals())

# the four Stone Age stages (ADR 0059): their own file, so the parts merge cleanly
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import stone_age_sprites  # noqa: E402
stone_age_sprites.add(globals())

import metal_age_sprites  # noqa: E402  the bronze and iron ages (ADR 0070)
metal_age_sprites.add(globals())


# loose heads and blades (crafting parts): the same shape as on the tool, moved to the middle of the icon
_heads = {s["layers"][1]["shape"] for s in SP.values()
          if len(s["layers"]) == 2 and s["layers"][0]["shape"] in ("handle", "short_handle", "saw_handle")}
for _id, _s in SP.items():
    _l = _s["layers"]
    if len(_l) != 1 or _l[0]["shape"] not in _heads:
        continue
    _name = _l[0]["shape"] + "_loose"
    if _name not in S:
        _r = S[_l[0]["shape"]]["rows"]
        _pts = [(x, y) for y in range(N) for x in range(N) if _r[y][x] != "."]
        _dx = (N - 1 - max(p[0] for p in _pts) - min(p[0] for p in _pts)) // 2
        _dy = (N - 1 - max(p[1] for p in _pts) - min(p[1] for p in _pts)) // 2
        _g = new()
        for x, y in _pts:
            put(_g, x + _dx, y + _dy, _r[y][x])
        S[_name] = dict(S[_l[0]["shape"]], rows=rows(_g))
    _l[0]["shape"] = _name

# ---------- validate + write ----------




def resolve(layer):
    s = S[layer["shape"]]
    cols = {}
    if "palette" in layer:
        cols.update(PALETTES[layer["palette"]])
    cols.update(s.get("colors", {}))
    cols.update(layer.get("colors", {}))
    return s["rows"], cols


def to_rgba(h):
    h = h.lstrip("#")
    if len(h) == 6:
        h += "ff"
    return tuple(int(h[i:i + 2], 16) for i in (0, 2, 4, 6))


def image(item):
    px = [[(0, 0, 0, 0)] * N for _ in range(N)]
    for layer in SP[item]["layers"]:
        rws, cols = resolve(layer)
        for y, r in enumerate(rws):
            for x, ch in enumerate(r):
                if ch == ".":
                    continue
                if ch not in cols:
                    raise SystemExit("%s: colour %r missing in shape %s" % (item, ch, layer["shape"]))
                px[y][x] = to_rgba(cols[ch])
    return px


for k in SP:
    image(k)

COMMENT = ("Item sprites, 16x16 pixel art. A shape is 16 rows of 16 characters: '.' is transparent, any other "
           "character is a colour key. A sprite stacks layers; a layer is a shape coloured by the named palette, "
           "then the shape's own colours, then the layer's colours. cube = the sprite is a block face, drawn as an "
           "isometric cube in icons and a cube in the world. Items with a block material and no sprite show their "
           "material's atlas tiles. Authored with shared shapes so a new tool or metal is one line here.")


def dump():
    L = ["{", '  "_comment": %s,' % json.dumps(COMMENT), '  "palettes": {']
    pk = list(PALETTES)
    for i, k in enumerate(pk):
        L.append("    %s: %s%s" % (json.dumps(k), json.dumps(PALETTES[k]), "," if i < len(pk) - 1 else ""))
    L += ["  },", '  "shapes": {']
    sk = list(S)
    for i, k in enumerate(sk):
        s = S[k]
        L.append("    %s: {" % json.dumps(k))
        if "colors" in s:
            L.append('      "colors": %s,' % json.dumps(s["colors"]))
        L.append('      "rows": [')
        for j, r in enumerate(s["rows"]):
            L.append("        %s%s" % (json.dumps(r), "," if j < 15 else ""))
        L.append("      ]")
        L.append("    }%s" % ("," if i < len(sk) - 1 else ""))
    L += ["  },", '  "sprites": {']
    ik = list(SP)
    for i, k in enumerate(ik):
        L.append("    %s: %s%s" % (json.dumps(k), json.dumps(SP[k]), "," if i < len(ik) - 1 else ""))
    L += ["  }", "}"]
    return "\n".join(L) + "\n"


# the steam age (ADR 0097)
g = shade(poly(new(), [(1, 8), (9, 3), (15, 7), (7, 12)]))                       # a flat sheet on the diagonal
shape("plate", outline(g))
g = shade(poly(new(), [(2, 11), (3, 13), (14, 5), (13, 3)]))                     # a tube: a thick bar
rect(g, 13, 3, 14, 5, "i")                                                      # its dark open end
shape("pipe", outline(g), {"i": "#1a1410"})
g = bricks(new(), 1, 10, 14, 14)
rect(g, 6, 11, 9, 14, "x")
rect(g, 7, 12, 8, 14, "y")
d = shade(ellipse(new(), 8, 6, 7, 3.5))                                         # the drum
for x in (3, 7, 11):
    put(d, x, 3, "r"); put(d, x, 9, "r")
rect(d, 12, 1, 13, 2, "c")                                                      # chimney
put(d, 2, 6, "g")                                                               # the gauge
shape("boiler", outline(compose(g, d)), dict(BRICK, x="#2a1a14", y="#ff8a24", r="#9ea2a8", c="#1a1410", g="#ffe066",
    h="#8a8e94", l="#6a6e74", m="#46494e", d="#2e3034"))   # the iron drum, dark
g = shade(compose(rect(new(), 2, 3, 4, 14), rect(new(), 11, 3, 13, 14), rect(new(), 2, 2, 13, 3)))   # the frame
h = shade(rect(new(), 6, 4, 9, 8), "#", "L", "M", "D", "H")                                         # the head
a = shade(art([
    "................", "................", "................", "................", "................",
    "................", "................", "................", "................", "....######......",
    ".....#####......", "......###.......", "......###.......", ".....#####......", "................", "................"]), "#", "e", "f", "k", "e")
shape("steam_hammer", outline(compose(g, h, a)), {"L": "#c4c8cc", "M": "#9ea2a8", "D": "#6a6e74", "H": "#eceff2", "e": "#d8a850", "f": "#b8863a", "k": "#8a5e22"})
g = shade(ellipse(new(), 8, 8, 7, 4))
for y in range(N):
    for x in range(N):
        dx, dy = x + 0.5 - 8, y + 0.5 - 8
        if (dx / 5.2) ** 2 + (dy / 2.4) ** 2 <= 1:
            g[y][x] = "n" if (x + y) % 2 else "N"
f = shade(compose(rect(new(), 2, 11, 3, 14), rect(new(), 12, 11, 13, 14), rect(new(), 7, 12, 8, 14)), "#", "L", "M", "D", "H")
shape("steam_sieve", outline(compose(f, g)), {"n": "#c8b088", "N": "#3a2a18", "L": "#c4c8cc", "M": "#9ea2a8", "D": "#6a6e74", "H": "#eceff2"})
g = shade(compose(ellipse(new(), 4.5, 8, 3.5, 3), ellipse(new(), 11.5, 8, 3.5, 3), rect(new(), 7, 7, 8, 8)))
ellipse(g, 4.5, 8, 2, 1.8, "w")
ellipse(g, 11.5, 8, 2, 1.8, "w")
line(g, 1, 7, 0, 5, "s"); line(g, 14, 7, 15, 5, "s")
shape("goggles", outline(g), {"w": "#a6d6e2", "s": "#6a3a18"})
g = shade(compose(rect(new(), 5, 2, 10, 4), rect(new(), 3, 5, 12, 14)))
line(g, 3, 5, 1, 2, "s"); line(g, 12, 5, 14, 2, "s")
put(g, 5, 9, "d"); put(g, 10, 9, "d")                                            # pocket seams
shape("apron", outline(g), {"s": "#2e1808"})
g = shade(diag(2, 13, 9, 2))                                                     # a wrench: a diagonal shaft with an open jaw
poly(g, [(9, 6), (13, 2), (15, 4), (13, 6), (11, 8)], "#")
rect(g, 12, 3, 13, 4, ".")
shape("wrench", outline(shade(g)))
sp("wrench", ("wrench", "iron"))
sp("iron_plate", ("plate", "iron"))
sp("iron_rod", ("stick", "iron"))
sp("bronze_pipe", ("pipe", "bronze"))
sp("copper_pipe", ("pipe", "copper"))
sp("small_boiler", "boiler")
sp("steam_hammer", ("steam_hammer", "iron"))
sp("steam_sieve", ("steam_sieve", "wood"))
sp("goggles", ("goggles", "bronze"))
# ADR 0131, second LV pass: things made from the machines' outputs (shapes from the files above)
PALETTES["rubber"] = ramp("#0e0e16", "#22222c", "#34343e", "#4a4a52", "#6e6a62")
sp("rubber_boots", ("boots", "rubber"))
sp("diving_mask", ("goggles", "rubber"))
sp("spyglass", ("pipe", "copper"), ("ball", "tin"))
sp("hand_drill", H, ("pickaxe_head", "steel"), ("flower_head", "iron"))
sp("heavy_copper_cable", ("pipe", "copper", {"e": "#0e0e16", "f": "#22222c"}))
sp("leather_apron", ("apron", "leather"))
# ADR 0155: a cloth smoke mask: a wool pad over mouth and nose with ties at the sides and a charcoal pad sewn in
g = shade(compose(rect(new(), 3, 6, 12, 11), rect(new(), 4, 12, 11, 12)))
line(g, 3, 7, 0, 5, "s"); line(g, 12, 7, 15, 5, "s")
rect(g, 6, 8, 9, 9, "c")
shape("cloth_mask", outline(g), {"s": "#8a7a5a", "c": "#2a2626"})
sp("cloth_mask", ("cloth_mask", "wool"))

# stage 2 and the turbine (ADR 0099)
PALETTES["fire_brick"] = ramp("#6a5a3a", "#b09a62", "#d4bc84", "#e8d4a4", "#f8ecc8")   # fire clay: pale buff
sp("fire_clay", ("ball", "fire_brick"))
sp("fire_brick", ("brick", "fire_brick"))
sp("steel_plate", ("plate", "steel"))
sp("steel_work_piece", ("work_piece", "steel"))
sp("blast_furnace", ("blast_furnace", "fire_brick"))
sp("steel_boiler", ("boiler", "", {"h": "#d0d8e2", "l": "#9ca6b2", "m": "#737d8a", "d": "#4a525e"}))
sp("ore_crusher", ("steam_sieve", "steel", {"n": "#9ea2a8", "N": "#2e3034"}))
sp("water_tank", ("bucket", "steel"))
sp("crushed_iron_ore", ("dust_pile", "iron_ore"))
sp("crushed_copper_ore", ("dust_pile", "copper_ore"))
sp("crushed_tin_ore", ("dust_pile", "tin_ore"))
sp("copper_wire", ("twine", "copper"))
sp("copper_cable", ("pipe", "copper", {"e": "#1c1410", "f": "#2a2018"}))   # ADR 0113
sp("electric_furnace", ("machine", "iron", {"e": "#fca030", "f": "#c0300c", "k": "#b05a34", "K": "#8a3e22"}))
# the industrial hull (ADR 0129), like MachineEntity.box_model: seen from the front, engine-green panels (q lit,
# p, r shaded) in a dark iron frame (palette), a steel-framed window (s) glowing with the kind's colours (e over f),
# vent slits (v), a hazard-striped plinth (y, b) and the kind's fitting on top (k, K)
g = new()
rect(g, 1, 3, 14, 13, "m")                   # the frame
rect(g, 2, 4, 13, 11, "p")                   # the panels
rect(g, 2, 4, 13, 4, "q"); rect(g, 2, 4, 2, 11, "q"); rect(g, 13, 5, 13, 11, "r"); rect(g, 3, 11, 12, 11, "r")
rect(g, 3, 5, 9, 10, "s")                    # the window frame
rect(g, 4, 6, 8, 9, "w")
rect(g, 4, 8, 8, 8, "e"); rect(g, 4, 9, 8, 9, "f")
for y in (6, 8, 10):
    rect(g, 11, y, 12, y, "v")               # the vents
rect(g, 1, 3, 14, 3, "l"); rect(g, 1, 3, 1, 13, "l"); rect(g, 14, 4, 14, 13, "d")
for x in range(1, 15):
    put(g, x, 13, "y" if (x // 2) % 2 == 0 else "b")
for p in ((3, 5), (9, 5), (3, 10), (9, 10)):
    put(g, p[0], p[1], "h")                  # rivets
rect(g, 5, 1, 10, 2, "k"); rect(g, 9, 1, 10, 2, "K")
shape("machine", outline(g), {"p": "#3a6852", "q": "#5a8e70", "r": "#24453a", "s": "#a6b0bc", "w": "#16181e",
                              "v": "#16181e", "y": "#e8b830", "b": "#24232a", "k": "#9ea6b2", "K": "#5c6470",
                              "e": "#fca030", "f": "#c0300c"})
# its cube face (a hull block, the battery box): the frame round a riveted green panel
g = new()
rect(g, 0, 0, 15, 15, "m")
rect(g, 2, 2, 13, 13, "p")
rect(g, 2, 2, 13, 2, "q"); rect(g, 2, 2, 2, 13, "q"); rect(g, 13, 3, 13, 13, "r"); rect(g, 3, 13, 12, 13, "r")
rect(g, 0, 0, 15, 0, "l"); rect(g, 0, 0, 0, 15, "l"); rect(g, 15, 1, 15, 15, "d"); rect(g, 1, 15, 15, 15, "d")
for p in ((1, 1), (14, 1), (1, 14), (14, 14), (7, 1), (7, 14), (1, 7), (14, 7)):
    put(g, p[0], p[1], "h")
shape("hull_face", g, {"p": "#3a6852", "q": "#5a8e70", "r": "#24453a"})
# the electrical age, tier 1 (ADR 0113): rubber, wire, boards, circuits, parts, machines
sp("raw_rubber", ("lump", "oil"))
sp("rubber", ("plate", "bomb"))
sp("insulated_wire", ("twine", "bomb"))
sp("coated_board", ("plate", "wood"))
sp("vacuum_tube", ("flask", "glass"))
sp("resistor", ("nugget", "terracotta"))
sp("primitive_circuit", ("spell_card", None, {"g": "#2a5a30", "G": "#3e7e46", "k": "#1c3e22"}), ("nugget", "copper"))
sp("iron_screw", ("nugget", "iron"))
sp("machine_hull", ("hull_face", "iron"), cube=True)
sp("electric_motor", ("flower_head", "copper"))
sp("electric_piston", ("plate", "iron"), ("stick", "copper"))
sp("lathe", ("machine", "iron", {"e": "#f2d488", "f": "#b8863a", "k": "#d8a850", "K": "#8a5e22"}))
sp("bending_press", ("machine", "iron", {"e": "#c4c8cc", "f": "#6a6e74", "k": "#e8b830", "K": "#24232a"}))
sp("wiremill", ("machine", "iron", {"e": "#f6b98a", "f": "#c0663a", "k": "#e08a55", "K": "#8a4424"}))
sp("macerator", ("machine", "iron", {"e": "#b0aaa0", "f": "#6a6460", "k": "#8a8a96", "K": "#50505a"}))   # ADR 0117
sp("alloy_smelter", ("machine", "iron", {"e": "#ffe27a", "f": "#ec6418", "k": "#b05a34", "K": "#8a3e22"}))
sp("assembler", ("machine", "iron", {"e": "#7ed43a", "f": "#2e7a10", "k": "#e8b830", "K": "#24232a"}))
# ADR 0131: more LV machines (the machine face, an accent per machine) and their parts
sp("extractor", ("machine", "iron", {"e": "#c8e86a", "f": "#5a8a1e", "k": "#d8a850", "K": "#8a5e22"}))
sp("cutter", ("machine", "iron", {"e": "#eceff2", "f": "#6a6e74", "k": "#e8b830", "K": "#24232a"}))
sp("extruder", ("machine", "iron", {"e": "#ffb45a", "f": "#c0401a", "k": "#9ea2a8", "K": "#3a3c46"}))
sp("canning_machine", ("machine", "iron", {"e": "#e4e8ee", "f": "#7a8290", "k": "#e08a55", "K": "#8a4424"}))
sp("electric_compressor", ("machine", "iron", {"e": "#b8c8e0", "f": "#4a5a7a", "k": "#9ea2a8", "K": "#3a3c46"}))
sp("electrolyzer", ("machine", "iron", {"e": "#9ae0f0", "f": "#2a6a8a", "k": "#3a3c46", "K": "#1a1c24"}))
# ADR 0141: the scrubber (gravel-grey face, sulfur-yellow accent) and a bucket of fouled water
sp("scrubber", ("machine", "iron", {"e": "#c8c0a0", "f": "#6a6450", "k": "#e8d040", "K": "#7a6a18"}))
sp("fouled_water_bucket", ("bucket", "iron"), ("bucket_fill", None, {"q": "#5a5c32", "Q": "#9a9a5a"}))
PALETTES["resin"] = ramp("#3a1a0e", "#8a4a12", "#c8781e", "#e8a032", "#fcd870")
PALETTES["tin"] = ramp("#2e3442", "#7a8494", "#aab2be", "#ccd2da", "#f2f2ea")
PALETTES["carbon"] = ramp("#0e0e18", "#24242e", "#3a3a46", "#56565e", "#80786e")
sp("sticky_resin", ("ball", "resin"))
sp("tin_can", ("jar", "tin"))
sp("canned_meat", ("jar", "tin"), ("pot_fill", None, {"q": "#8a2a22", "k": "#d05a40"}))
sp("canned_fish", ("jar", "tin"), ("pot_fill", None, {"q": "#5a7a8a", "k": "#a8c4d0"}))
sp("canned_vegetables", ("jar", "tin"), ("pot_fill", None, {"q": "#3a7a22", "k": "#e0802a"}))
sp("steel_rod", ("stick", "steel"))
sp("carbon_electrode", ("stick", "carbon"))
sp("battery_box", ("hull_face", "steel", {"p": "#9ca6b2", "q": "#c4ccd6", "r": "#5a6370"}), cube=True)
sp("electric_lamp", ("lantern", "iron"))
sp("pump", ("machine", "iron", {"e": "#7ac8f0", "f": "#2a6a8a", "k": "#d07a40", "K": "#8a4424"}))   # a water window, the copper outlet
sp("multimeter", ("tape_measure", "iron", {"c": "#1c3e22", "k": "#7ed43a", "t": "#f4e870", "T": "#b8a830"}))   # ADR 0122/0123: a dial in an iron case
# circuit control and protection (ADR 0123) as their models draw them (ADR 0129): an iron base (palette) with
# bronze lugs (b) at both ends, and on it a copper blade (c, C lit) closed into its jaw on slate (s, S) with a
# wooden grip (w, W); a riveted box with a fuse cartridge (t) behind a dark window (k); a green breaker (p, q)
# with its hazard lever (y, Y) and a flag window
g = new()
rect(g, 1, 10, 14, 13, "m"); rect(g, 1, 10, 14, 10, "l"); rect(g, 1, 13, 14, 13, "d")
rect(g, 0, 11, 1, 12, "b"); rect(g, 14, 11, 15, 12, "b")
rect(g, 3, 8, 12, 9, "s"); rect(g, 3, 8, 12, 8, "S")
rect(g, 3, 5, 4, 7, "c"); rect(g, 11, 5, 12, 7, "c")
rect(g, 4, 5, 11, 6, "c"); rect(g, 4, 5, 11, 5, "C")
rect(g, 11, 3, 14, 6, "w"); rect(g, 11, 3, 14, 3, "W")
put(g, 2, 12, "h"); put(g, 13, 12, "h")
shape("knife_switch", outline(g), {"s": "#2e3038", "S": "#4a4e5a", "c": "#c0663a", "C": "#f6b98a", "w": "#6e4424",
                                   "W": "#b07a46", "b": "#a8793a"})
g = new()
rect(g, 2, 3, 13, 13, "m"); rect(g, 2, 3, 13, 4, "l"); rect(g, 13, 5, 13, 13, "d"); rect(g, 2, 13, 12, 13, "d")
rect(g, 0, 8, 1, 9, "b"); rect(g, 14, 8, 15, 9, "b")
rect(g, 4, 7, 11, 10, "k")
rect(g, 5, 8, 10, 9, "t"); put(g, 5, 8, "c"); put(g, 5, 9, "c"); put(g, 10, 8, "c"); put(g, 10, 9, "c")
for p in ((3, 5), (12, 5), (3, 12), (12, 12)):
    put(g, p[0], p[1], "h")
shape("fuse_box", outline(g), {"k": "#16181e", "t": "#ccd2da", "c": "#c0663a", "b": "#a8793a"})
g = new()
rect(g, 2, 5, 13, 13, "m"); rect(g, 3, 6, 12, 12, "p"); rect(g, 3, 6, 12, 6, "q")
rect(g, 2, 5, 13, 5, "l"); rect(g, 13, 6, 13, 13, "d"); rect(g, 2, 13, 12, 13, "d")
rect(g, 0, 9, 1, 10, "b"); rect(g, 14, 9, 15, 10, "b")
rect(g, 7, 1, 8, 4, "y"); rect(g, 7, 1, 8, 1, "Y")
rect(g, 5, 9, 10, 10, "k"); rect(g, 6, 9, 7, 10, "r")
shape("breaker", outline(g), {"p": "#3a6852", "q": "#5a8e70", "y": "#e8b830", "Y": "#fff0b0", "k": "#16181e",
                              "r": "#e03a20", "b": "#a8793a"})
# a paper capacitor: a waxed paper roll (p, q lit, P shade) with dark wax ends (e) and copper leads (c)
g = new()
rect(g, 3, 5, 12, 10, "p"); rect(g, 3, 5, 12, 5, "q"); rect(g, 3, 10, 12, 10, "P")
rect(g, 3, 5, 3, 10, "e"); rect(g, 12, 5, 12, 10, "e")
for x in (6, 9):
    rect(g, x, 6, x, 9, "P")                  # the wraps
rect(g, 0, 7, 2, 8, "c"); rect(g, 13, 7, 15, 8, "c")
shape("capacitor", outline(g), {"p": "#d8c8a0", "q": "#f2e6c4", "P": "#a89870", "e": "#5a3a20", "c": "#c0663a"})
sp("capacitor", ("capacitor", "iron"))   # ADR 0123
# ADR 0161: a blueprint, a rolled hide sheet (p, q lit, P shade, rolls r at the ends); written, it carries pale lines (w)
g = new()
rect(g, 2, 3, 13, 12, "p"); rect(g, 2, 3, 13, 3, "q"); rect(g, 2, 12, 13, 12, "P")
rect(g, 1, 2, 2, 13, "r"); rect(g, 13, 2, 14, 13, "r")
shape("blueprint", outline(g), {"p": "#3a6ab8", "q": "#5a8ad8", "P": "#2a4a88", "r": "#c8b08a"})
rect(g, 4, 5, 11, 5, "w"); rect(g, 4, 5, 4, 10, "w"); rect(g, 11, 5, 11, 10, "w"); rect(g, 4, 10, 11, 10, "w")
rect(g, 4, 7, 7, 7, "w"); rect(g, 9, 8, 10, 9, "w")
shape("written_blueprint", outline(g), {"p": "#3a6ab8", "q": "#5a8ad8", "P": "#2a4a88", "r": "#c8b08a", "w": "#e8f0ff"})
sp("blueprint", ("blueprint", "iron"))
sp("written_blueprint", ("written_blueprint", "iron"))
sp("builder", ("machine", "iron", {"e": "#f2e088", "f": "#b89a3a", "k": "#e8b830", "K": "#24232a"}))
sp("fuse", ("twine", "tin"))
sp("knife_switch", ("knife_switch", "iron"))
sp("fuse_box", ("fuse_box", "iron"))
sp("breaker", ("breaker", "iron"))
# the small sources and the meter (ADR 0123) as their models draw them (ADR 0129)
import math as _m
g = new()   # the water wheel: rims and spokes (palette = its wood), an iron hub (i), timber legs, a green dynamo
for y in range(16):
    for x in range(16):
        r = _m.hypot(x + 0.5 - 7.5, y + 0.5 - 7.0)
        a = _m.atan2(y + 0.5 - 7.0, x + 0.5 - 7.5)
        if r < 1.8:
            put(g, x, y, "i")
        elif 5.0 < r < 6.6:
            put(g, x, y, "d")
        elif r <= 5.0 and (a % (_m.pi / 4)) < 0.32:
            put(g, x, y, "m")
rect(g, 2, 12, 3, 15, "d"); rect(g, 12, 12, 13, 15, "d")
rect(g, 12, 3, 15, 6, "p"); rect(g, 12, 3, 15, 3, "q")
shape("water_wheel", outline(g), {"i": "#2e3034", "p": "#3a6852", "q": "#5a8e70"})
g = new()   # the wind turbine: three blades (palette = steel) round a hub, hazard tips, a green nacelle, the post
for y in range(16):
    for x in range(16):
        r = _m.hypot(x + 0.5 - 7.5, y + 0.5 - 5.5)
        if r > 5.6:
            continue
        a = _m.atan2(y + 0.5 - 5.5, x + 0.5 - 7.5) + _m.pi / 2
        off = a % (2 * _m.pi / 3)
        off = min(off, 2 * _m.pi / 3 - off)
        if r < 1.3:
            put(g, x, y, "i")
        elif off * r < 1.1:
            put(g, x, y, "y" if r > 4.4 else "l")
rect(g, 7, 7, 8, 15, "m"); rect(g, 8, 7, 8, 15, "d")
rect(g, 9, 4, 11, 6, "p"); rect(g, 5, 14, 10, 15, "i")
shape("wind_turbine", outline(g), {"i": "#2e3034", "y": "#e8b830", "p": "#3a6852"})
g = new()   # the hand crank: a green dynamo (p, q, r) between iron flanges (palette) on a base, the arm and grip
rect(g, 2, 13, 13, 15, "m"); rect(g, 2, 13, 13, 13, "l")
rect(g, 3, 5, 11, 12, "p"); rect(g, 3, 5, 11, 6, "q"); rect(g, 3, 11, 11, 12, "r")
rect(g, 2, 5, 2, 12, "d"); rect(g, 12, 5, 12, 12, "d")
for y in (6, 9, 11):
    put(g, 2, y, "h"); put(g, 12, y, "h")
rect(g, 6, 3, 8, 4, "c")
rect(g, 13, 8, 13, 9, "d"); rect(g, 14, 3, 14, 9, "d"); rect(g, 13, 1, 15, 3, "w")
shape("hand_crank", outline(g), {"p": "#3a6852", "q": "#5a8e70", "r": "#24453a", "c": "#c0663a", "w": "#7a4a26"})
g = new()   # the meter panel: an iron case (palette), a cream face, the volt and amp bars by zone, coloured caps
rect(g, 1, 1, 14, 14, "m"); rect(g, 1, 1, 14, 1, "l"); rect(g, 1, 1, 1, 14, "l"); rect(g, 14, 2, 14, 14, "d"); rect(g, 2, 14, 14, 14, "d")
rect(g, 3, 3, 12, 12, "f")
for x0, top in ((4, 5), (9, 9)):
    rect(g, x0, 4, x0 + 2, 10, "k")
    for y in range(top, 11):
        put(g, x0, y, "g" if y > 7 else ("a" if y > 5 else "e")); put(g, x0 + 1, y, "g" if y > 7 else ("a" if y > 5 else "e"))
        put(g, x0 + 2, y, "g" if y > 7 else ("a" if y > 5 else "e"))
    for y in (4, 6, 8, 10):
        put(g, x0 + 3, y, "k")
rect(g, 4, 11, 6, 11, "v"); rect(g, 9, 11, 11, 11, "c")
for p in ((2, 2), (13, 2), (2, 13), (13, 13)):
    put(g, p[0], p[1], "h")
shape("meter_panel", outline(g), {"f": "#ece6d2", "k": "#22232a", "g": "#4a9a5a", "a": "#e8b830", "e": "#e03a20",
                                  "v": "#ffe27a", "c": "#c0663a"})
g = new()   # a lead-acid cell: a black case (palette = carbon) in three cells, a grey lid, red + and dark - posts
rect(g, 3, 5, 12, 14, "m"); rect(g, 3, 5, 12, 5, "l"); rect(g, 12, 6, 12, 14, "d"); rect(g, 3, 14, 11, 14, "d")
for x in (6, 9):
    rect(g, x, 6, x, 13, "d")
rect(g, 2, 3, 13, 4, "s"); rect(g, 2, 3, 13, 3, "S")
rect(g, 4, 1, 5, 2, "r"); rect(g, 10, 1, 11, 2, "k")
shape("lead_cell", outline(g), {"s": "#7a8290", "S": "#aab2be", "r": "#e03a20", "k": "#22232a"})
sp("thin_cable", ("twine", "copper"))   # ADR 0123 round 2
sp("lead_acid_cell", ("lead_cell", "carbon"))
sp("water_wheel", ("water_wheel", "wood"))
sp("wind_turbine", ("wind_turbine", "steel"))
sp("hand_crank", ("hand_crank", "iron"))
sp("daylight_switch", ("fuse_box", "iron", {"t": "#ffe27a"}))   # its photocell in the window
sp("meter_panel", ("meter_panel", "iron"))
# ADR 0160: the contraption drives
g = new()   # the lift winch: an iron frame (palette) round a rope drum (b, rope r) hanging a plank platform (w)
rect(g, 1, 1, 2, 8, "m"); rect(g, 13, 1, 14, 8, "m"); rect(g, 1, 1, 14, 2, "l"); rect(g, 2, 8, 13, 8, "d")
rect(g, 4, 3, 11, 6, "b"); rect(g, 4, 3, 11, 3, "B"); rect(g, 4, 6, 11, 6, "k")
for x in (5, 7, 9):
    rect(g, x, 4, x, 5, "r")
rect(g, 7, 9, 8, 11, "r")
rect(g, 2, 12, 13, 14, "w"); rect(g, 2, 12, 13, 12, "W"); put(g, 7, 12, "r"); put(g, 8, 12, "r")
shape("lift_winch", outline(g), {"b": "#6e4424", "B": "#9a6a3a", "k": "#3e2614", "r": "#d8c08a", "w": "#a87a44", "W": "#c89a5c"})
g = new()   # the drawbridge hinge: a plank span (w) swung half up off an iron hinge block (palette), a chain (c)
rect(g, 1, 10, 5, 14, "m"); rect(g, 1, 10, 5, 10, "l"); rect(g, 5, 11, 5, 14, "d")
for i in range(9):
    rect(g, 5 + i, 9 - i, 6 + i, 10 - i, "w")
    put(g, 6 + i, 10 - i, "W")
line(g, 3, 9, 3, 2, "c"); line(g, 3, 2, 13, 2, "c"); line(g, 13, 2, 13, 1, "c")
put(g, 3, 12, "h")
shape("drawbridge_hinge", outline(g), {"w": "#a87a44", "W": "#7a5430", "c": "#8a8e98"})
g = new()   # the crane: a steel mast and jib (palette) on a base, a rope (r) and a yellow hook (y)
rect(g, 4, 3, 5, 13, "m"); rect(g, 4, 3, 4, 13, "l")
rect(g, 1, 2, 14, 3, "m"); rect(g, 1, 2, 14, 2, "l")
line(g, 6, 4, 10, 2, "d")
rect(g, 2, 13, 8, 15, "d"); rect(g, 2, 13, 8, 13, "m")
rect(g, 12, 4, 12, 9, "r")
rect(g, 11, 10, 13, 10, "y"); put(g, 13, 11, "y"); put(g, 13, 12, "y"); put(g, 12, 12, "y"); put(g, 11, 11, "y")
shape("crane", outline(g), {"r": "#d8c08a", "y": "#e8b830"})
sp("lift_winch", ("lift_winch", "iron"))
sp("drawbridge_hinge", ("drawbridge_hinge", "iron"))
sp("crane", ("crane", "steel"))
sp("coke_oven", ("bricks_face", None), cube=True)
sp("coal_coke", ("charcoal_lump", "coal"))
sp("steam_turbine", ("steam_hammer", "steel", {"e": "#e07434", "f": "#b0501e", "k": "#6a2a10"}))
sp("iron_gear", ("flower_head", "iron"))
sp("glass_lens", ("ball", "tin"))
sp("copper_coil", ("twine", "copper"))
sp("steam_cell", ("flask", "steel"))
# the miner's helmet (bug report 2026-10-28 reference): copper dome, navy straps and wide brim, yellow lamp
g = ellipse(new(), 8, 10.5, 6.6, 8.2)
for y in range(10, N):
    for x in range(N):
        put(g, x, y, ".")
g = shade(g)
for y in range(N):
    if get(g, 9, y) != ".":
        put(g, 9, y, "n")
for x in range(N):
    if get(g, x, 4) != ".":
        put(g, x, 4, "n")
rect(g, 0, 10, 15, 10, "n")
rect(g, 1, 11, 14, 11, "k")
rect(g, 3, 5, 7, 9, "n")
rect(g, 4, 6, 6, 8, "y")
put(g, 4, 6, "w")
put(g, 6, 8, "Y")
shape("miner_helmet", outline(g), {"n": "#2c3448", "k": "#1a2030", "y": "#ffd660", "Y": "#f0a030", "w": "#fff6d0"})
sp("miners_helmet", ("miner_helmet", "copper"))
sp("travelers_backpack", ("bag", "leather"), ("strips", "leather"))
sp("steam_drill", H, ("pickaxe_head", "steel"), ("flask", "steel"))
sp("boiler_suit", ("apron", "iron", {"s": "#2e3034"}))
# the jetpack (bug report 2026-10-28 reference): two slate tanks with red caps and bands on a back
# plate with leather straps, flame under the nozzles
PALETTES["slate"] = ramp("#1e2630", "#3a4654", "#5a6878", "#7a8a9c", "#a4b4c4")
g = new()
rect(g, 0, 2, 4, 12)
rect(g, 11, 2, 15, 12)
g = shade(g)
rect(g, 5, 4, 10, 11, "m")
rect(g, 6, 5, 9, 10, "q")
for x0 in (0, 11):
    rect(g, x0 + 1, 2, x0 + 3, 2, "r")
    rect(g, x0, 9, x0 + 4, 9, "r")
    rect(g, x0 + 1, 4, x0 + 3, 7, "q")
    put(g, x0 + 2, 5, "l")
    rect(g, x0 + 1, 13, x0 + 3, 13, "Y")
    put(g, x0 + 2, 13, "y")
    put(g, x0 + 2, 14, "Y")
for x in (6, 9):
    rect(g, x, 3, x, 12, "s")
    put(g, x, 12, "S")
shape("jetpack", outline(g), {"q": "#2a3440", "r": "#de524c", "s": "#8a4a30", "S": "#5a2c1c", "y": "#ffe08a", "Y": "#f0a030"})
sp("jetpack", ("jetpack", "slate"))
sp("surveyors_stake", ("stick", "wood"), ("nugget", "copper"))
sp("compressor", ("crucible", "iron"), ("pipe", "bronze"))
sp("hopper", ("bowl", "iron"), ("pipe", "bronze"))
sp("chute", ("bowl", "wood"))
sp("belt", ("strips", "leather"), ("stick", "iron"))
# the rail (bug report 2026-10-28_08-32-11: "ugly"): two iron rails, bright heads, on four wooden sleepers
g = new()
for y0 in (1, 5, 9, 13):
    rect(g, 1, y0, 14, y0 + 1, "w")
    rect(g, 1, y0 + 1, 14, y0 + 1, "W")
for x0 in (4, 10):
    rect(g, x0, 0, x0 + 1, 15, "i")
    rect(g, x0, 0, x0, 15, "j")
shape("rail_track", outline(g), {"w": "#8a5a32", "W": "#5e3a1e", "i": "#4a4e56", "j": "#b4b8c0"})
sp("rail", ("rail_track", "iron"))
# the minecart (bug report 2026-10-28 reference): a grey open iron box, light rim, dark inside,
# rivet spots on the side, wheels peeking out below
g = rect(new(), 1, 7, 14, 13)
g = shade(g)
rect(g, 1, 4, 14, 6, "h")
rect(g, 3, 5, 12, 6, "i")
rect(g, 4, 6, 11, 6, "j")
rect(g, 2, 3, 13, 3, "l")
for (x, y) in [(3, 9), (6, 10), (9, 9), (12, 11), (4, 12), (10, 12), (7, 8)]:
    put(g, x, y, "d")
rect(g, 3, 14, 4, 14, "k")
rect(g, 11, 14, 12, 14, "k")
shape("minecart", outline(g), {"i": "#3a3e44", "j": "#5a5e64", "k": "#1a1a1c"})
sp("minecart", ("minecart", "iron"))
# the chest cart: the minecart under a banded wooden lid with a brass latch
g = rect(new(), 1, 2, 14, 6)
g = shade(g)
for x in (4, 11):
    rect(g, x, 2, x, 6, "k")
put(g, 7, 5, "y")
put(g, 8, 5, "y")
shape("cart_lid", outline(g), {"k": "#3a3e44", "y": "#e0a040"})
sp("chest_cart", ("minecart", "iron"), ("cart_lid", "wood"))

# movement toys (ADR 0151): a delta-wing glider on its stick frame, a rope coil on its stake, a three-pronged hook
g = poly(new(), [(8, 1), (15.5, 10), (8, 8), (0.5, 10)])
shape("glider_wing", outline(shade(g)))
g = new()
line(g, 8, 2, 8, 12, "d")
line(g, 3, 9, 13, 9, "m")
line(g, 8, 12, 5, 14, "d")
line(g, 8, 12, 11, 14, "d")
rect(g, 5, 14, 11, 14, "l")
shape("glider_frame", g)
sp("glider", ("glider_wing", "scraped_hide"), ("glider_frame", "wood"))
g = ellipse(new(), 7.0, 9.5, 6.2, 5.0)
for y in range(N):
    for x in range(N):
        if ((x + 0.5 - 7.0) / 2.6) ** 2 + ((y + 0.5 - 9.5) / 1.8) ** 2 < 1.0:
            g[y][x] = "."
g = shade(g)
for y in range(N):   # the turns of the coil
    for x in range(N):
        if g[y][x] == "m" and (x + y) % 3 == 0:
            g[y][x] = "d"
shape("rope_coil", outline(g))
shape("rope_stake", outline(line(new(), 13, 1, 13, 14, "m")))
sp("climbing_rope", ("rope_coil", "twine"), ("rope_stake", "wood"))
g = new()
line(g, 8, 2, 8, 10)
line(g, 7, 10, 5, 9)
line(g, 5, 8, 4, 5)
line(g, 9, 10, 11, 9)
line(g, 11, 8, 12, 5)
rect(g, 7, 1, 9, 1)
shape("hook_prongs", outline(shade(g)))
g = new()
line(g, 8, 11, 7, 13, "m")
line(g, 6, 14, 3, 15, "m")
shape("hook_rope", g)
sp("grappling_hook", ("hook_rope", "twine"), ("hook_prongs", "iron"))

# hand-painted overrides: game/art/items/<item_id>.png (16x16) replaces the generated sprite (game/art/README.md)
ART = os.path.join(REPO, "game", "art", "items")
if os.path.isdir(ART):
    from PIL import Image
    KEYS = [c for c in map(chr, range(33, 0x300)) if c.isprintable() and c not in '."\\']   # one colour key per distinct colour
    item_ids = {it["id"] for it in json.load(open(os.path.join(REPO, "game", "data", "items.json"), encoding="utf8"))["items"]}
    for f in sorted(os.listdir(ART)):
        if not f.endswith(".png"):
            continue
        item = f[:-4]
        if item not in item_ids:
            raise SystemExit("game/art/items/%s: no item %r in items.json" % (f, item))
        im = Image.open(os.path.join(ART, f)).convert("RGBA")
        if im.size != (N, N):
            raise SystemExit("game/art/items/%s: must be %dx%d, is %dx%d" % (f, N, N, *im.size))
        keys, grid = {}, []
        for y in range(N):
            r = ""
            for x in range(N):
                p = im.getpixel((x, y))
                if p[3] == 0:
                    r += "."
                    continue
                h = "#%02x%02x%02x" % p[:3] + ("%02x" % p[3] if p[3] < 255 else "")
                r += keys.setdefault(h, KEYS[len(keys)])
            grid.append(r)
        S["art_" + item] = {"colors": {v: k for k, v in keys.items()}, "rows": grid}
        SP[item] = dict(SP.get(item, {}), layers=[{"shape": "art_" + item}])
        print("art override", f)

path = os.path.join(REPO, "game", "data", "item_sprites.json")
open(path, "w", encoding="utf8", newline="\n").write(dump())
print("wrote", path, len(SP), "sprites", len(S), "shapes")

# contact sheet
if not SHEET:
    sys.exit(0)
from PIL import Image, ImageDraw

Z = 4
CW, CH = 16 * Z + 8, 16 * Z + 14
cols_n = 12
keys = [k for k in SP if k in sys.argv[2].split(",")] if len(sys.argv) > 2 else list(SP)   # [sheet.png] [id,id,...]
sheet = Image.new("RGBA", (cols_n * CW, ((len(keys) + cols_n - 1) // cols_n) * CH), (62, 62, 70, 255))
dr = ImageDraw.Draw(sheet)
for i, k in enumerate(keys):
    px = image(k)
    im = Image.new("RGBA", (16, 16))
    for y in range(N):
        for x in range(N):
            im.putpixel((x, y), px[y][x])
    im = im.resize((16 * Z, 16 * Z), Image.NEAREST)
    cx, cy = (i % cols_n) * CW + 4, (i // cols_n) * CH + 2
    sheet.alpha_composite(im, (cx, cy))
    dr.text((cx, cy + 16 * Z), k[:12], fill=(230, 230, 230, 255))
sheet.save(SHEET)
print("sheet", SHEET, sheet.size)
