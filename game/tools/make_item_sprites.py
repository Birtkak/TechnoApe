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


def ramp(o, d, m, l, h):
    return {"o": o, "d": d, "m": m, "l": l, "h": h}


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


def tint(h, f):
    h = h.lstrip("#")
    c = [int(h[i:i + 2], 16) for i in (0, 2, 4)]
    c = [max(0, min(255, round(v * f))) for v in c]
    return "#%02x%02x%02x" % tuple(c)


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
shape("handle", body(diag(2, 13, 10)))
shape("short_handle", outline(art([
    "................", "................", "................", "................",
    "................", "................", "................", "................",
    "................", "......l.........", ".....lmd........", "....lmd.........",
    "...lmd..........", "..lmd...........", "..md............", "................"])))
shape("stick", body(diag(2, 13, 11)))

# pickaxe head: top arm mirrored across the anti-diagonal
g = new()
for x in range(4, 11):
    put(g, x, 1, "#")
for x in range(2, 13):
    put(g, x, 2, "#")
for x in range(9, 14):
    put(g, x, 3, "#")
shape("pickaxe_head", body(mirror_diag(g)))

# axe head: wedge across the handle tip, blade toward the upper left
g = poly(new(), tool_pts([(-1.8, -1.4), (-1.8, 1.4), (2.4, 1.6), (6.6, 4.9), (7.3, 0.0), (6.6, -4.9), (2.4, -1.6)]))
g = shade(g)
for y in range(N):
    for x in range(N):
        if g[y][x] != "." and (x + 0.5 - TIP[0]) * NRM[0] + (y + 0.5 - TIP[1]) * NRM[1] > 5.2:
            g[y][x] = "h"   # honed edge
shape("axe_head", outline(g))

# shovel: rounded spade beyond the tip
c = (TIP[0] + 1.5 * U[0], TIP[1] + 1.5 * U[1])
shape("shovel_head", body(ellipse(new(), c[0], c[1], 3.6, 2.9, ang=-45)))

# hammer: block across the tip
shape("hammer_head", body(poly(new(), tool_pts([(-3.8, -1.9), (-3.8, 2.1), (3.8, 2.1), (3.8, -1.9)], (11.5, 4.5)))))

# saw: blade along the handle direction with teeth on the lower-right side
g = new()
B0 = (6.0, 10.0)
for y in range(N):
    for x in range(N):
        px, py = x + 0.5 - B0[0], y + 0.5 - B0[1]
        t = px * U[0] + py * U[1]
        s = px * NRM[0] + py * NRM[1]
        if 0.0 <= t <= 10.5 and -0.3 <= s <= 2.2:
            put(g, x, y, "#")
g = shade(g)
for y in range(N):
    for x in range(N):
        px, py = x + 0.5 - B0[0], y + 0.5 - B0[1]
        t = px * U[0] + py * U[1]
        s = px * NRM[0] + py * NRM[1]
        if 0.5 <= t <= 10.0 and -1.5 <= s < -0.3 and x % 2 == 0 and g[y][x] == ".":
            put(g, x, y, "l")   # teeth (x + y is constant along the blade, so step on x)
shape("saw_blade", outline(g))

# knife: tapered blade, spine on the upper-left side
shape("knife_blade", body(poly(new(), tool_pts([(-1.2, 0.0), (1.1, 0.0), (1.1, 2.0), (0.2, 7.0), (-0.4, 8.0), (-1.2, 5.5)], (6.5, 9.5)))))
# chisel: narrow straight bar with a flat edge
shape("chisel_blade", body(poly(new(), tool_pts([(-0.9, 0.0), (0.9, 0.0), (0.9, 6.5), (-0.9, 6.5)], (6.5, 9.5)))))

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
shape("ingot", outline(art([
    "................", "................", "................", "................",
    ".....hhhhhh.....", "....hhhhhhhh....", "...llllllllll...", "..llllllllllll..",
    "..lmmmmmmmmmmd..", "..lmmmmmmmmmmd..", "..mmmmmmmmmmmd..", "..dddddddddddd..",
    "................", "................", "................", "................"])))
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
shape("hoe_head", body(poly(new(), tool_pts([(-0.6, -1.4), (5.2, -1.4), (5.2, 0.9), (-0.6, 0.9)]))))
g = compose(line(new(), 3, 3, 10, 10, "#", 2), line(new(), 12, 3, 5, 10, "#", 2))
g = shade(g)
ring = compose(ellipse(new(), 4, 12.5, 2.4, 2.4, "r"), ellipse(new(), 11.5, 12.5, 2.4, 2.4, "r"))
for (cx, cy) in [(4, 12.5), (11.5, 12.5)]:
    ellipse(ring, cx, cy, 1.1, 1.1, ".")
shape("shears", outline(compose(ring, g)), {"r": "#8a3a2a"})
shape("chair", outline(art([
    "................", "...d............", "...dl...........", "...dl...........",
    "...dl...........", "...dlll.........", "...dl...........", "...dl...........",
    "...dlwwwwwww....", "...dlmmmmmmmm...", "...dddddddddd...", "...d.......d....",
    "...d.......d....", "...d.......d....", "...d.......d....", "................"])), {"w": "#ecece4"})

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
    sp(m + "_saw", SH, ("saw_blade", m))
    sp(m + "_saw_head", ("saw_blade", m))
    sp(m + "_hammer", H, ("hammer_head", m))
    sp(m + "_hammer_head", ("hammer_head", m))
sp("flint_shovel", H, ("shovel_head", "flint"))
sp("stone_hammer", H, ("hammer_head", "stone"))
for m in ("flint", "copper", "iron"):
    sp(m + "_knife", SH, ("knife_blade", m))
for m in ("copper", "iron"):
    sp(m + "_knife_head", ("knife_blade", m))
sp("copper_saw_blade", ("saw_blade", "copper"))   # smithed (ADR 0042)
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
sp("berry_compote", ("bowl", "terracotta"), ("bowl_fill", None, {"q": "#6a1e48", "k": "#3a4ea0"}))
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
sp("wheat_seeds", ("seeds", "seed"))
sp("wheat", ("sheaf", "straw"))
sp("carrot", ("carrot", "carrot"))
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
sp("pan", ("pan", "terracotta"))
sp("raw_pan", ("pan", "clay", {"i": "#3a3e46", "j": "#5a606a"}))
sp("storage_pot", ("jar", "terracotta"))
sp("raw_storage_pot", ("jar", "clay", {"i": "#3a3e46"}))
sp("pot_of_stew", ("cooking_pot", "terracotta"), ("pot_fill", None, {"q": "#7a4a22", "k": "#d07a30"}))
sp("stew", ("bowl", "terracotta"), ("bowl_fill", None, {"q": "#7a4a22", "k": "#d07a30"}))
sp("daub", ("ball", "daub"))
sp("firewood", ("firewood", "wood"))
for m, item in (("wood", "wooden_hoe"), ("stone", "stone_hoe"), ("iron", "iron_hoe")):
    sp(item, H, ("hoe_head", m))
sp("shears", ("shears", "iron"))
sp("torch", ("torch", "handle"))
sp("oak_sapling", ("sapling", "leaf"))
sp("dandelion", ("flower_stem", "leaf"), ("flower_head", None, {"p": "#f4d23c", "c": "#d08a1c"}))
sp("poppy", ("flower_stem", "leaf"), ("flower_head", None, {"p": "#d8302a", "c": "#2a1a14"}))
sp("cornflower", ("flower_stem", "leaf"), ("flower_head", None, {"p": "#4a72e0", "c": "#24306a"}))
sp("oxeye_daisy", ("flower_stem", "leaf"), ("flower_head", None, {"p": "#f4f2ea", "c": "#e8b820"}))
sp("allium", ("flower_stem", "leaf"), ("allium_head", None, ramp("#3a1450", "#6a2a8a", "#9446b8", "#b46ad4", "#dca8f0")))
sp("wand_basic", ("wand_stick", "handle"), ("wand_gem", "gem"))
sp("fire_bomb", ("fire_bomb", "bomb"))
sp("laser_cutter", ("laser_cutter", "steel"))
sp("crater_rod", ("wand_stick", "iron"), ("wand_gem", "lava"))
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
keys = list(SP)
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
