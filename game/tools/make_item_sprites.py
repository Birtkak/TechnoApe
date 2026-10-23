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
shape("bucket", g, {"i": "#1a1c20", "r": "#46494e"})
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
sp("copper_chisel", SH, ("chisel_blade", "copper"))
sp("copper_chisel_head", ("chisel_blade", "copper"))
sp("copper_prospecting_pick", H, ("prospecting_head", "copper"))
sp("copper_prospecting_pick_head", ("prospecting_head", "copper"))

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
sp("oil_bucket", ("bucket", "iron"), ("bucket_fill", None, {"q": "#1a1410", "Q": "#5a4a3a"}))
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
sp("torch", ("torch", "handle"))
sp("oak_sapling", ("sapling", "leaf"))
sp("wand_basic", ("wand_stick", "handle"), ("wand_gem", "gem"))
sp("fire_bomb", ("fire_bomb", "bomb"))
sp("bricks", "bricks_face", cube=True)
sp("coal_block", ("coal_block_face", "coal"), cube=True)
sp("glass", ("glass_face", "glass"), cube=True)
CARD = {"projectile": "#5a3428", "modifier": "#24406e", "multicast": "#2a5a30", "utility": "#6a5620"}
for item, glyph, kind in [("spell_spark_bolt", "spark", "projectile"), ("spell_fire_bolt", "fire", "projectile"),
                          ("spell_bomb", "bomb", "projectile"), ("spell_digging_bolt", "dig", "projectile"),
                          ("spell_speed_up", "speed", "modifier"), ("spell_freeze_charge", "freeze", "modifier"),
                          ("spell_water_trail", "water", "modifier"), ("spell_double_cast", "double", "multicast"),
                          ("spell_light", "light", "utility")]:
    bg = CARD[kind]
    sp(item, ("spell_card", None, {"g": bg, "G": tint(bg, 1.35), "k": tint(bg, 0.7)}), "glyph_" + glyph)


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
