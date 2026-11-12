#!/usr/bin/env python3
"""Paint every block's texture into data/textures/ from data/palette.toml (ADR 0084).

    python -I tools/paint_textures.py            # every block with a recipe
    python -I tools/paint_textures.py sand log   # only these
    python -I tools/paint_textures.py --preview out/tex-preview.png   # also a tiled contact sheet

The style (the developer's call, 2026-11-12): calm and readable. Each block takes three or four colours of one
ramp (dark, shade, mid, lit, light), low contrast between neighbours, and a material shape instead of noise.
A ramp step the palette lacks is added to data/palette.toml the first time a texture uses it (`shade` between
dark and mid, `lit` a third of the way from mid to light; a block with no ramp gets one from its blocks.toml
colour), so a style change is an edit here and a run. Every file it writes goes on data/drafts.txt.
gravel.png is the developer's and water is not a block texture here: neither is touched.
"""
import colorsys
import hashlib
import sys
import tomllib
from pathlib import Path

import numpy as np
from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
DATA = ROOT / "data"
OUT = DATA / "textures"
N = 16
SKIP = {"air", "gravel", "water"}

# ---------------------------------------------------------------- palette


class Palette:
    def __init__(self, path):
        self.path = path
        text = path.read_text(encoding="utf-8")
        self.header = [l for l in text.splitlines() if l.startswith("#")]
        self.rows = tomllib.loads(text)["colour"]
        self.added = []

    def rgb(self, name):
        for r in self.rows:
            if r["name"] == name:
                return r["rgb"]
        return None

    def ramp_of(self, name):
        return [r for r in self.rows if r.get("ramp") == name]

    def add(self, name, rgb, ramp):
        taken = {r["rgb"] for r in self.rows}
        c = list(rgb)
        while "#%02x%02x%02x" % tuple(c) in taken:  # colours are unique in the palette
            c[2] = c[2] + 1 if c[2] < 255 else c[2] - 3
        hx = "#%02x%02x%02x" % tuple(c)
        row = {"name": name, "rgb": hx, "ramp": ramp}
        same = [i for i, r in enumerate(self.rows) if r.get("ramp") == ramp]
        lum = lambda h: sum(int(h[i:i + 2], 16) * w for i, w in ((1, .3), (3, .59), (5, .11)))
        at = len(self.rows)
        if same:  # a ramp runs dark to light in file order
            at = same[-1] + 1
            for i in same:
                if lum(self.rows[i]["rgb"]) > lum(hx):
                    at = i
                    break
        self.rows.insert(at, row)
        self.added.append(name)
        return name

    def save(self):
        out = "\n".join(self.header) + "\n"
        for r in self.rows:
            out += f'\n[[colour]]\nname = "{r["name"]}"\nrgb = "{r["rgb"]}"\n'
            if r.get("ramp"):
                out += f'ramp = "{r["ramp"]}"\n'
        self.path.write_text(out, encoding="utf-8", newline="\n")


def hex3(h):
    return np.array([int(h[i:i + 2], 16) for i in (1, 3, 5)], float) / 255


def to8(c):
    return [int(round(v)) for v in np.clip(np.asarray(c) * 255, 0, 255)]


class Ramp:
    """Five steps of one material by key: d dark, s shade, m mid, l lit, L light. A missing step is added to
    the palette when first asked for."""
    KEYS = "dsmlL"

    def __init__(self, pal, base, colour=None, names=None, into=None):
        self.pal, self.base, self.colour = pal, base, colour
        self.names = names or {}
        self.into = into or base  # the ramp new steps join

    def __getitem__(self, k):
        if k in self.names:
            return self.names[k]
        p, b = self.pal, self.base
        own = {"d": f"{b} dark", "m": b, "L": f"{b} light", "s": f"{b} shade", "l": f"{b} lit"}[k]
        if p.rgb(own):
            self.names[k] = own
            return own
        if self.colour is None and p.rgb(b) and p.rgb(f"{b} dark") and p.rgb(f"{b} light"):
            d, m, L = hex3(p.rgb(f"{b} dark")), hex3(p.rgb(b)), hex3(p.rgb(f"{b} light"))
            c = {"s": d + (m - d) * 0.55, "l": m + (L - m) * 0.33}[k]
        else:
            m = np.asarray(self.colour if self.colour is not None else hex3(p.rgb(b)), float)
            if k == "m":
                c = m
            elif k in "ds":
                f = {"d": 0.64, "s": 0.82}[k]  # darker, leaning cool
                c = m * f * np.array([0.97, 0.99, 1.05]) ** (1 if k == "s" else 2)
            else:
                f = {"l": 0.17, "L": 0.36}[k]  # lighter, leaning warm
                c = (m + (1 - m) * f) * np.array([1.02, 1.01, 0.96]) ** (1 if k == "l" else 2)
        self.names[k] = self.pal.add(own, to8(c), self.into)
        return own


# ---------------------------------------------------------------- drawing helpers


def seed_of(*parts):
    return int.from_bytes(hashlib.sha256("/".join(map(str, parts)).encode()).digest()[:8], "little")


def vnoise(rng, cell):
    """Tileable smooth noise over 16 x 16, features about `cell` texels."""
    n = max(1, N // cell)
    g = rng.random((n, n))
    ys, xs = np.mgrid[0:N, 0:N] / (N / n)
    x0, y0 = np.floor(xs).astype(int), np.floor(ys).astype(int)
    tx, ty = xs - x0, ys - y0
    tx, ty = tx * tx * (3 - 2 * tx), ty * ty * (3 - 2 * ty)
    a, b = g[y0 % n, x0 % n], g[y0 % n, (x0 + 1) % n]
    c, d = g[(y0 + 1) % n, x0 % n], g[(y0 + 1) % n, (x0 + 1) % n]
    return (a + (b - a) * tx) * (1 - ty) + (c + (d - c) * tx) * ty


def field(rng, cell=4, rough=0.4):
    f = vnoise(rng, cell) * (1 - rough) + rng.random((N, N)) * rough
    return (f - f.min()) / (np.ptp(f) + 1e-9)


def levels(f, names, shares, mask=None):
    """Names by rank of f: the lowest share of pixels the first name, and so on (exact shares)."""
    out = np.empty((N, N), object)
    idx = np.argwhere(mask) if mask is not None else np.argwhere(np.ones((N, N), bool))
    order = idx[np.argsort(f[tuple(idx.T)], kind="stable")]
    cuts = np.round(np.cumsum(shares) / sum(shares) * len(order)).astype(int)
    lo = 0
    for name, hi in zip(names, cuts):
        for y, x in order[lo:hi]:
            out[y, x] = name
        lo = hi
    return out


def put(c, x, y, name):
    c[y % N, x % N] = name


def walk(c, rng, name, x, y, steps, dx, dy, wobble=0.35):
    """A connected line from (x, y) going (dx, dy) on the whole, a side step now and then: a crack or a vein."""
    for _ in range(steps):
        put(c, x, y, name)
        if rng.random() < wobble:
            if dx:
                y += rng.choice((-1, 1))
            else:
                x += rng.choice((-1, 1))
            put(c, x, y, name)
        x, y = x + dx, y + dy
    return x, y


def cracks(c, rng, name, n, lo=3, hi=6, vertical=None):
    for _ in range(n):
        v = rng.random() < 0.5 if vertical is None else vertical
        walk(c, rng, name, int(rng.integers(N)), int(rng.integers(N)), int(rng.integers(lo, hi + 1)),
             0 if v else 1, 1 if v else 0)


def specks(c, rng, name, n, size=1, mask=None):
    for _ in range(n):
        x, y = int(rng.integers(N)), int(rng.integers(N))
        if mask is not None and not mask[y, x]:
            continue
        for dx, dy in [(0, 0), (1, 0), (0, 1), (1, 1)][: {1: 1, 2: int(rng.integers(2, 4)), 3: 4}[size]]:
            put(c, x + dx, y + dy, name)
    return c


def grain(rng, r, keys="sml", shares=(0.25, 0.5, 0.25), cell=4, rough=0.4):
    return levels(field(rng, cell, rough), [r[k] for k in keys], shares)


def bands(seed, f, plan, wobble=0, cell=8):
    """Strata: f shifted per row by the bands of `plan` [(rows, offset), ...] summing 16 rows; the same plan
    in every variant so a cliff's bands run on across blocks. `wobble` texels of fold along x."""
    rng = np.random.default_rng(seed)
    off = np.concatenate([np.full(rows, o, float) for rows, o in plan])
    w = np.round((vnoise(rng, cell)[0] - 0.5) * 2 * wobble).astype(int) if wobble else np.zeros(N, int)
    ys = (np.arange(N)[:, None] + w[None, :]) % N
    return f + off[ys]


# ---------------------------------------------------------------- recipes: (rng, ramp, ...) -> 16 x 16 names


def rock_top(rng, r, cracks_n=2, keys="sml", shares=(0.25, 0.5, 0.25), cell=4, rough=0.35):
    c = grain(rng, r, keys, shares, cell, rough)
    cracks(c, rng, r["d"], cracks_n)
    return c


def rock_side(rng, r, seed, plan, wobble=1, cracks_n=1, keys="sml", shares=(0.25, 0.5, 0.25), cell=4,
              rough=0.3, lines=()):
    f = bands(seed, field(rng, cell, rough), plan, wobble)
    c = levels(f, [r[k] for k in keys], shares)
    # bedding lines: a row of the darker step with gaps, at fixed rows
    for row in lines:
        for x in range(N):
            if rng.random() < 0.8:
                put(c, x, row, r["s"] if rng.random() < 0.75 else r["d"])
    cracks(c, rng, r["d"], cracks_n, 2, 4, vertical=True)
    return c


def foliated(rng, r, seed, cycles, amp, keys="sml", glint=0, line="d"):
    """Wavy bands (gneiss, schist): a sine across y bent by a sine across x, tiling both ways."""
    k = np.random.default_rng(seed)
    ph, ph2 = k.random() * 6.28, k.random() * 6.28
    ys, xs = np.mgrid[0:N, 0:N]
    yy = ys + amp * np.sin(2 * np.pi * xs / N + ph)
    f = 0.65 * (0.5 + 0.5 * np.sin(2 * np.pi * cycles * yy / N + ph2)) + 0.35 * field(rng, 2, 0.5)
    c = levels(f, [r[x] for x in keys], (0.3, 0.42, 0.28))
    if line:
        dark = f < np.quantile(f, 0.07)
        c[dark] = r[line]
    specks(c, rng, r["L"], glint)
    return c


def crystals(c, rng, r, n, shape="blob", core="m", rim="d", glint="L", sizes=(2, 3)):
    """Ore on its host: n crystals or nuggets, each a core with a darker rim on its lower right and a glint."""
    for _ in range(n):
        x, y = int(rng.integers(N)), int(rng.integers(N))
        s = int(rng.choice(sizes))
        if shape == "cube":
            for dy in range(s):
                for dx in range(s):
                    put(c, x + dx, y + dy, r[core])
            for d in range(s):
                put(c, x + s, y + d + 1, r[rim])
                put(c, x + d + 1, y + s, r[rim])
            put(c, x, y, r[glint])
        else:
            pts = [(0, 0), (1, 0), (0, 1), (1, 1), (2, 1), (1, 2)][: s * 2]
            for dx, dy in pts:
                put(c, x + dx, y + dy, r[core])
            for dx, dy in pts:
                if (dx + 1, dy) not in pts:
                    put(c, x + dx + 1, y + dy, r[rim]) if rng.random() < 0.6 else None
            put(c, x, y, r[glint])
    return c


def pebbly(rng, mix, extras=()):
    """The developer's gravel style: every texel one of a few stone tones, by weight; `extras` [(name, n,
    size)] scatter coloured grains over it."""
    names, w = zip(*mix)
    p = np.array(w, float) / sum(w)
    c = np.empty((N, N), object)
    pick = rng.choice(len(names), size=(N, N), p=p)
    for y in range(N):
        for x in range(N):
            c[y, x] = names[pick[y, x]]
    for name, n, size in extras:
        specks(c, rng, name, n, size)
    return c


def pebbles(rng, r, n=7, ground="s", tones="ml", rim="d"):
    """Rounded stones on a ground: each a blob of a tone, lit at the top left, shaded at the bottom right."""
    c = grain(rng, r, "ds" if ground == "s" else "sm", (0.3, 0.7), 2, 0.5)
    for _ in range(n):
        cx, cy = rng.random() * N, rng.random() * N
        rad = 1.2 + rng.random() * 1.3
        tone = r[str(rng.choice(list(tones)))]
        for dy in range(-3, 4):
            for dx in range(-3, 4):
                d = ((dx) ** 2 + (dy) ** 2) ** 0.5
                if d <= rad:
                    x, y = int(cx) + dx, int(cy) + dy
                    put(c, x, y, r["l"] if dx + dy < -rad * 0.6 else tone)
                elif d <= rad + 0.8 and dx + dy > 0:
                    put(c, int(cx) + dx, int(cy) + dy, r[rim])
    return c


def grass_top(rng, r):
    # a calm ground of two tones in wide patches; blades are short dark strokes, a few with a lit tip
    c = grain(rng, r, "sm", (0.3, 0.7), 4, 0.45)
    for k in range(18):
        x, y = int(rng.integers(N)), int(rng.integers(N))
        put(c, x, y + 1, r["s"])
        put(c, x, y + 2, r["s"])
        put(c, x, y, r["l"] if k % 3 == 0 else r["m"])
    return c


def grass_side(rng, r, soil, seed):
    """Soil below; the grass hangs over the top edge in tufts of 2 to 5 rows, its tips lighter, a shade
    under it on the soil."""
    c = dirt(rng, soil)
    k = np.random.default_rng(seed)
    hang = np.clip(np.round(2 + 3 * vnoise(k, 4)[0] + rng.integers(-1, 2, N) * 0.6), 2, 5).astype(int)
    for x in range(N):
        h = int(hang[x])
        for y in range(h):
            c[y, x] = r["l"] if y == 0 else (r["m"] if y < h - 1 else r["s"])
        if h < 5 and rng.random() < 0.7:
            c[h, x] = soil["d"]
    return c


def dirt(rng, r):
    c = grain(rng, r, "sml", (0.28, 0.5, 0.22), 4, 0.45)
    specks(c, rng, r["d"], 5, 1)
    return c


def leaves(rng, r, holes=0.28, cell=3, keys="sml", rough=0.3, dark=0.08):
    f = field(rng, cell, rough)
    mask = f >= np.quantile(f, holes)
    c = levels(f, [r[k] for k in keys], (0.3, 0.45, 0.25), mask)
    # the edge of a clump under a hole is in shadow
    for y in range(N):
        for x in range(N):
            if mask[y, x] and not mask[(y - 1) % N, x] and rng.random() < 0.6:
                c[y, x] = r["s"]
            if mask[y, x] and not mask[(y + 1) % N, x] and rng.random() < dark * 6:
                c[y, x] = r["d"]
    c[~mask] = None
    return c


def needles(rng, r, holes=0.16):
    c = np.full((N, N), r["s"], object)
    for _ in range(26):  # short slanted needles on a dark twig ground
        x, y = int(rng.integers(N)), int(rng.integers(N))
        d = int(rng.choice((-1, 1)))
        for i in range(3):
            put(c, x + i * d, y + i, r["m"] if i else r["l"])
    f = field(rng, 3, 0.5)
    c[f < np.quantile(f, holes)] = None
    specks(c, rng, r["d"], 6)
    return c


def blades(rng, r, fill=0.55):
    c = np.empty((N, N), object)
    for x in range(N):
        if rng.random() > fill:
            continue
        top, tone = int(rng.integers(N)), r[str(rng.choice(list("sml")))]
        length = int(rng.integers(9, 16))
        for i in range(length):
            put(c, x, top + i, r["l"] if i == 0 else tone)
    return c


def rings(rng, wood, bark, centre=(7.5, 7.5)):
    ys, xs = np.mgrid[0:N, 0:N]
    cx, cy = centre[0] + rng.random() - 0.5, centre[1] + rng.random() - 0.5
    d = np.hypot(xs - cx, ys - cy) + (vnoise(rng, 4) - 0.5) * 0.7
    c = np.empty((N, N), object)
    for y in range(N):
        for x in range(N):
            v = d[y, x]
            if v > 7.4:
                c[y, x] = bark["s"] if v < 8.6 else bark["d"]
            elif v < 0.9:
                c[y, x] = wood["s"]
            else:
                ring = int(v / 1.5)
                c[y, x] = wood["s"] if (v / 1.5) % 1 < 0.28 else (wood["m"] if ring % 2 else wood["l"])
    a = rng.random() * 6.28  # a check: a crack from the pith outward
    for t in range(2, 7):
        put(c, int(round(cx + np.cos(a) * t)), int(round(cy + np.sin(a) * t)), wood["d"])
    return c


def bark(rng, r, seed):
    k = np.random.default_rng(seed)
    col = vnoise(k, 4)[0]
    c = np.empty((N, N), object)
    noise = field(rng, 4, 0.3)
    for x in range(N):
        for y in range(N):
            v = col[x] * 0.85 + noise[y, x] * 0.15
            c[y, x] = r["s"] if v < 0.35 else (r["m"] if v < 0.78 else r["l"])
    for x in range(N):  # furrows: broken dark runs down the bark
        if col[x] < 0.3 or rng.random() < 0.12:
            y = int(rng.integers(N))
            for i in range(int(rng.integers(4, 11))):
                put(c, x, y + i, r["d"])
    return c


def lamp(r):
    ys, xs = np.mgrid[0:N, 0:N]
    d = np.hypot(xs - 7.5, ys - 7.5)
    edge = np.minimum(np.minimum(xs, ys), np.minimum(15 - xs, 15 - ys))
    c = np.empty((N, N), object)
    for y in range(N):
        for x in range(N):
            e, v = edge[y, x], d[y, x]
            c[y, x] = (r["d"] if e == 0 else r["s"] if e == 1 else
                       r["L"] if v < 2.6 else r["l"] if v < 4.4 else r["m"])
    for x, y in ((1, 1), (14, 1), (1, 14), (14, 14)):
        c[y, x] = r["m"]
    return c


def plate(rng, r):
    c = grain(rng, r, "sm", (0.3, 0.7), 16, 0.15)
    for y in range(N):
        if y % 3 == 1:
            for x in range(N):
                if rng.random() < 0.5:
                    c[y, x] = r["s"]
    c[0, :] = r["l"]
    c[:, 0] = r["l"]
    c[15, :] = r["d"]
    c[:, 15] = r["d"]
    return c


# ---------------------------------------------------------------- the blocks


def build(pal, blocks):
    defs = {b["name"]: b for b in blocks}

    def R(base, colour=None, names=None):
        if colour is None and base in defs and not pal.ramp_of(base) and not pal.rgb(base):
            colour = defs[base]["colour"]
        # shortcut: the engine's palette test holds the grass ramp to three colours, so grass's new steps
        # make a ramp of their own; join them to "grass" when that test allows more
        return Ramp(pal, base, colour, names, "grass steps" if base == "grass" else None)

    def season_ramp(name, s):
        b = defs[name]
        if s == "summer":
            return R("grass") if name == "grass" else R(name, b["colour"])
        return R(f"{name} {s}", b[s]["colour"])

    out = {}  # file stem -> list of variant canvases

    def tex(stem, n, fn):
        out[stem] = [fn(np.random.default_rng(seed_of(stem, v)), v) for v in range(n)]

    # soils and loose ground
    dirt_r = R("dirt")
    tex("dirt", 3, lambda g, v: dirt(g, dirt_r))
    gravel_mix = [("sandstone dark", 0), ("tin gravel", 230), ("schist", 227), ("quartzite dark", 210),
                  ("shale", 107), ("gold gravel", 103), ("shale light", 86), ("chalcopyrite", 35),
                  ("regolith dark", 25)]
    gravel_mix = [(m, w) for m, w in gravel_mix if w]
    sand = R("sand")

    def sand_face(g, v, top):
        c = grain(g, sand, "sml", (0.22, 0.56, 0.22), 2, 0.6)
        if top:  # wind ripples: faint lit crests every few rows
            for y0 in (2, 7, 12):
                for x in range(N):
                    put(c, x, y0 + int(round(np.sin(x / 16 * 6.283 + v) * 0.8)), sand["l"])
        specks(c, g, sand["d"], 4)
        specks(c, g, gravel_mix[0][0], 2)
        return c
    tex("sand", 3, lambda g, v: sand_face(g, v, False))
    tex("sand_top", 3, lambda g, v: sand_face(g, v, True))

    snow = R("snow")
    tex("snow", 3, lambda g, v: specks(specks(grain(g, snow, "ml", (0.6, 0.4), 4, 0.2), g, snow["s"], 3), g, snow["L"], 3))
    ice = R("ice")

    def ice_face(g, v):
        c = grain(g, ice, "sml", (0.2, 0.6, 0.2), 8, 0.1)
        x0 = int(g.integers(N))
        for i in range(N):  # one long crack, white, running corner to corner so it meets itself
            put(c, x0 + i, i, ice["L"])
        specks(c, g, ice["L"], 3)
        specks(c, g, ice["s"], 2, 2)
        return c
    tex("ice", 2, ice_face)
    clay = R("clay")
    tex("clay", 2, lambda g, v: rock_top(g, clay, 0, "sml", (0.25, 0.55, 0.2), 8, 0.2))
    tex("clay_side", 2, lambda g, v: rock_side(g, clay, seed_of("clay"), [(5, 0), (3, 0.25), (6, 0), (2, -0.25)],
                                                0, 0, cell=8, rough=0.2))
    mud = R("mud")
    tex("mud", 3, lambda g, v: specks(grain(g, mud, "dsm", (0.2, 0.55, 0.25), 4, 0.3), g, mud["l"], 4))

    # gravels: the developer's gravel with coloured grains in it
    tex("copper gravel", 4, lambda g, v: pebbly(g, gravel_mix, [(R("copper gravel")["d"], 4, 2),
                                                                 (R("copper")["s"], 2, 1)]))
    tex("tin gravel", 4, lambda g, v: pebbly(g, gravel_mix, [(R("tin gravel")["d"], 5, 2)]))
    tex("gold gravel", 4, lambda g, v: pebbly(g, gravel_mix, [(R("gold gravel")["d"], 2, 1)]))

    # the grasses: top with blades, a side that hangs over dirt; every season a block has
    for name, b in defs.items():
        if b.get("pattern") != "grass":
            continue
        for s in ("summer", "spring", "autumn", "winter"):
            if s != "summer" and s not in b:
                continue
            r = season_ramp(name, s)
            at = "" if s == "summer" else f"@{s}"
            tex(f"{name}_top{at}", 3, lambda g, v, r=r: grass_top(g, r))
            tex(f"{name}_side{at}", 3, lambda g, v, r=r: grass_side(g, r, dirt_r, seed_of(name, "hang")))

    # leaves and plants (cutouts)
    leaf_look = {
        "spruce leaves": lambda g, r: needles(g, r),
        "broadleaf leaves": lambda g, r: leaves(g, r, 0.26, 3),
        "jungle leaves": lambda g, r: leaves(g, r, 0.2, 5),
        "acacia leaves": lambda g, r: leaves(g, r, 0.36, 2),
        "shrub": lambda g, r: leaves(g, r, 0.32, 3),
        "reed": lambda g, r: blades(g, r),
    }
    autumn = {"d": "autumn dark", "s": "autumn rust", "m": "autumn orange", "l": "autumn gold",
              "L": "autumn gold"}
    for name, fn in leaf_look.items():
        b = defs[name]
        for s in ("summer", "spring", "autumn", "winter"):
            if s != "summer" and s not in b:
                continue
            if name == "broadleaf leaves" and s == "autumn":
                r = Ramp(pal, "autumn", names=dict(autumn))
            else:
                r = R(name, b["colour"]) if s == "summer" else R(f"{name} {s}", b[s]["colour"])
            at = "" if s == "summer" else f"@{s}"
            # every season of a block from one seed: the same leaves change colour, winter loses some
            if s == "winter" and name != "reed":
                tex(f"{name}{at}", 2, lambda g, v, r=r: leaves(np.random.default_rng(seed_of(name, v)), r, 0.6, 3))
            else:
                tex(f"{name}{at}", 2, lambda g, v, r=r, fn=fn: fn(np.random.default_rng(seed_of(name, v)), r))

    cactus = R("cactus")

    def cactus_side(g, v):
        c = np.empty((N, N), object)
        for x in range(N):
            k = "smls"[x % 4]
            c[:, x] = cactus[k]
        for _ in range(6):  # spines on the ridges
            put(c, 2 + 4 * int(g.integers(4)), int(g.integers(N)), cactus["L"])
        return c
    tex("cactus_side", 1, cactus_side)
    tex("cactus", 1, lambda g, v: specks(grain(g, cactus, "sml", (0.3, 0.45, 0.25), 4, 0.2), g, cactus["L"], 3))
    coral = R("coral")
    tex("coral", 2, lambda g, v: pebbles(g, coral, 8, "s", "ml"))

    # wood
    log = R("log")
    wood = R("log wood", [0.74, 0.58, 0.38])
    tex("log", 2, lambda g, v: rings(g, wood, log))
    tex("log_side", 3, lambda g, v: bark(g, log, seed_of("log", "bark")))

    # lamps, metal
    tex("lamp", 1, lambda g, v: lamp(R("lamp")))
    tex("blue lamp", 1, lambda g, v: lamp(R("blue lamp")))
    tex("copper", 1, lambda g, v: plate(g, R("copper")))

    # rocks: told apart by shape, not only by colour
    def rock(name, top, side=None, n=3):
        tex(name, n, top)
        if side:
            tex(f"{name}_side", n, side)

    st = R("stone")
    rock("stone", lambda g, v: rock_top(g, st, 1),
         lambda g, v: rock_side(g, st, seed_of("stone"), [(6, 0), (4, 0.2), (6, -0.1)], 1, 1))
    gr = R("granite")  # coarse speckle: dark mica and pale feldspar in grains of two texels

    def granite(g, v, r=gr, dark=6, pale=7):
        c = grain(g, r, "sm", (0.35, 0.65), 4, 0.4)
        specks(c, g, r["d"], dark, 2)
        specks(c, g, r["l"], pale, 2)
        return c
    rock("granite", granite)
    gn = R("gneiss")
    rock("gneiss", lambda g, v: foliated(g, gn, seed_of("gneiss"), 2, 1.6, "sml", 0, "d"))
    ss = R("sandstone")
    rock("sandstone", lambda g, v: rock_top(g, ss, 0, "sml", (0.2, 0.55, 0.25), 2, 0.5),
         lambda g, v: rock_side(g, ss, seed_of("sandstone"), [(5, 0.1), (4, -0.15), (4, 0.25), (3, -0.05)], 0, 0,
                                "sml", (0.25, 0.5, 0.25), 2, 0.4, lines=(4, 12)))
    sh = R("shale")

    def shale_side(g, v):
        c = grain(g, sh, "sm", (0.4, 0.6), 4, 0.3)
        for y in range(0, N, 2):  # thin flaky laminae: short dark dashes on every other row
            x = int(g.integers(N))
            while x < 40:
                ln = int(g.integers(3, 8))
                for i in range(ln):
                    put(c, x + i, y, sh["d"] if y % 4 == 0 else sh["l"])
                x += ln + int(g.integers(2, 6))
        return c
    rock("shale", lambda g, v: rock_top(g, sh, 1, "sml", (0.3, 0.5, 0.2)), shale_side)
    ls = R("limestone")

    def limestone_top(g, v):
        c = rock_top(g, ls, 1, "sml", (0.2, 0.6, 0.2), 8, 0.3)
        x, y = int(g.integers(3, 13)), int(g.integers(3, 13))  # a shell
        for dx, dy in ((0, 0), (1, -1), (2, -1), (3, 0), (1, 0), (2, 0)):
            put(c, x + dx, y + dy, ls["s"] if dy else ls["l"])
        return c
    rock("limestone", limestone_top,
         lambda g, v: rock_side(g, ls, seed_of("limestone"), [(7, 0), (5, 0.12), (4, -0.1)], 0, 1, "sml",
                                (0.2, 0.6, 0.2), 8, 0.3, lines=(6, 11)))
    sl = R("slate")

    def slate(g, v):
        c = grain(g, sl, "sml", (0.25, 0.55, 0.2), 8, 0.2)
        for x in (3, 10):  # cleavage: long straight partings
            for y in range(N):
                if g.random() < 0.85:
                    put(c, x + (1 if (y // 6) % 2 and x == 10 else 0), y, sl["d"])
        cracks(c, g, sl["s"], 1, 3, 5, vertical=False)
        return c
    rock("slate", slate)
    mb = R("marble")

    def marble(g, v):
        c = grain(g, mb, "ml", (0.6, 0.4), 8, 0.15)
        x0 = int(g.integers(N))
        for i in range(N):  # a grey vein that wraps corner to corner
            put(c, x0 + i + int(round(np.sin(i / 16 * 6.283 * 2) * 1.2)), i, mb["s"])
        cracks(c, g, mb["s"], 1, 3, 5)
        specks(c, g, mb["d"], 2)
        return c
    rock("marble", marble)
    qz = R("quartzite")

    def quartzite(g, v):
        c = grain(g, qz, "sml", (0.15, 0.6, 0.25), 8, 0.25)
        cracks(c, g, qz["L"], 2, 4, 7)
        specks(c, g, qz["L"], 4)
        return c
    rock("quartzite", quartzite)
    sc = R("schist")
    rock("schist", lambda g, v: foliated(g, sc, seed_of("schist"), 4, 1.2, "sml", 7, "d"))
    tf = R("tuff")
    rock("tuff", lambda g, v: specks(pebbles(g, tf, 5, "m", "ls", "s"), g, tf["d"], 6))
    an = R("andesite")

    def andesite(g, v):
        c = grain(g, an, "sml", (0.3, 0.5, 0.2), 4, 0.3)
        specks(c, g, an["L"], 9, 1)
        specks(c, g, an["L"], 3, 2)
        specks(c, g, an["d"], 3, 1)
        return c
    rock("andesite", andesite)
    bs = R("basalt")

    def basalt_side(g, v):
        c = grain(g, bs, "sml", (0.3, 0.5, 0.2), 4, 0.3)
        for x0, jog in ((0, 6), (8, 11)):  # columns: two joints, each with one step
            for y in range(N):
                put(c, x0 + (1 if y >= jog else 0), y, bs["d"])
        specks(c, g, bs["d"], 3)
        return c

    def basalt_top(g, v):
        c = grain(g, bs, "sml", (0.3, 0.5, 0.2), 4, 0.3)
        for x in range(N):  # the column tops: a net of joints
            put(c, x, 0, bs["d"])
            put(c, x, 8 + (1 if x > 8 else 0), bs["d"])
        for y in range(N):
            put(c, 4 if y < 8 else 12, y, bs["d"])
        specks(c, g, bs["d"], 4)
        return c
    rock("basalt", basalt_top, basalt_side)
    gb = R("gabbro")
    rock("gabbro", lambda g, v: granite(g, v, gb, 4, 9))

    # ores, each on its host
    def on(host_fn, host_seed_v):
        return host_fn(np.random.default_rng(seed_of(*host_seed_v)), host_seed_v[1])

    ma = R("malachite")

    def malachite(g, v):
        c = rock_top(g, st, 1)
        for _ in range(3):  # green crusts in bands
            cx, cy, rad = g.random() * N, g.random() * N, 2 + g.random() * 1.6
            for dy in range(-4, 5):
                for dx in range(-4, 5):
                    d = (dx * dx + dy * dy) ** 0.5
                    if d <= rad:
                        put(c, int(cx) + dx, int(cy) + dy, ma["l"] if d < 0.8 else (ma["m"] if int(d * 1.6) % 2 else ma["s"]))
        return c
    tex("malachite", 3, malachite)
    cp = R("chalcopyrite")
    tex("chalcopyrite", 3, lambda g, v: crystals(andesite(g, v), g, cp, 5, "cube", "L", "d", "L", (2,)))
    cs = R("cassiterite")
    tex("cassiterite", 3, lambda g, v: crystals(granite(g, v, gr, 4, 6), g, cs, 5, "cube", "d", "m", "L", (2,)))
    ga = R("galena")
    tex("galena", 3, lambda g, v: crystals(limestone_top(g, v), g, ga, 3, "cube", "m", "d", "L", (3,)))
    pb = R("pitchblende")

    def pitchblende(g, v):
        c = granite(g, v, gr, 4, 6)
        for _ in range(4):
            x, y = int(g.integers(N)), int(g.integers(N))
            for dx, dy in ((-1, 0), (3, 1), (0, -1), (2, 2), (1, -1), (-1, 2)):
                put(c, x + dx, y + dy, pb["L"]) if g.random() < 0.7 else None
            for dx, dy in ((0, 0), (1, 0), (2, 0), (0, 1), (1, 1), (2, 1), (1, 2)):
                put(c, x + dx, y + dy, pb["m"])
            put(c, x, y, pb["l"])
        return c
    tex("pitchblende", 3, pitchblende)
    gq = R("gold quartz")

    def gold_quartz(g, v):
        c = grain(g, gq, "dm", (0.3, 0.7), 4, 0.35)
        for _ in range(4):
            x, y = int(g.integers(N)), int(g.integers(N))
            put(c, x, y, gq["L"])
            if g.random() < 0.6:
                put(c, x + 1, y, gq["L"])
                put(c, x + 1, y + 1, "autumn gold")
        return c
    tex("gold quartz", 3, gold_quartz)
    co = R("coal")

    def coal(g, v, side):
        c = grain(g, co, "dm", (0.4, 0.6), 4, 0.3)
        for _ in range(5):  # a glossy break catches the light in short streaks
            x, y = int(g.integers(N)), int(g.integers(N))
            for i in range(int(g.integers(2, 5))):
                put(c, x + i, y, co["l"] if side or i % 2 == 0 else co["s"])
        return c
    tex("coal", 3, lambda g, v: coal(g, v, False))
    tex("coal_side", 3, lambda g, v: coal(g, v, True))
    he = R("hematite")
    tex("hematite_side", 3, lambda g, v: foliated(g, he, seed_of("hematite"), 3, 0.7, "sml", 0, ""))
    tex("hematite", 3, lambda g, v: rock_top(g, he, 1, "sml", (0.25, 0.5, 0.25), 4, 0.35))
    bx = R("bauxite")
    tex("bauxite", 3, lambda g, v: pebbles(g, bx, 7, "m", "ml", "d"))
    bi = R("bog iron")

    def bog_iron(g, v):
        c = dirt(g, R("mud"))
        for _ in range(5):
            x, y = int(g.integers(N)), int(g.integers(N))
            for dx, dy in ((0, 0), (1, 0), (0, 1), (1, 1), (2, 1)):
                put(c, x + dx, y + dy, bi["m"])
            put(c, x, y, bi["L"])
            put(c, x + 1, y + 2, bi["d"])
        return c
    tex("bog iron", 3, bog_iron)
    go = R("gossan")
    tex("gossan", 3, lambda g, v: rock_top(g, go, 2, "sml", (0.3, 0.45, 0.25), 4, 0.35))

    # Luna
    rg = R("regolith")

    def dust(g, v, r, rocks=3):
        c = grain(g, r, "sml", (0.15, 0.62, 0.23), 4, 0.3)
        for _ in range(rocks):
            x, y = int(g.integers(N)), int(g.integers(N))
            put(c, x, y, r["l"])
            put(c, x + 1, y, r["m"])
            put(c, x, y + 1, r["d"])
            put(c, x + 1, y + 1, r["d"])
        return c
    tex("regolith", 3, lambda g, v: dust(g, v, rg))
    tex("mare regolith", 3, lambda g, v: dust(g, v, R("mare regolith")))
    tex("fresh regolith", 3, lambda g, v: dust(g, v, R("fresh regolith"), 2))
    ao = R("anorthosite")

    def anorthosite(g, v):
        c = rock_top(g, ao, 1, "sml", (0.25, 0.5, 0.25), 4, 0.3)
        for _ in range(6):  # plagioclase laths
            x, y = int(g.integers(N)), int(g.integers(N))
            h = g.random() < 0.5
            for i in range(3):
                put(c, x + (i if h else 0), y + (0 if h else i), ao["L"])
        return c
    tex("anorthosite", 3, anorthosite)
    mbs = R("mare basalt")
    tex("mare basalt", 3, lambda g, v: specks(rock_top(g, mbs, 1, "sml", (0.3, 0.5, 0.2)), g, mbs["d"], 6))
    fe = R("iron-nickel")

    def iron_nickel(g, v):
        c = grain(g, fe, "sm", (0.4, 0.6), 8, 0.2)
        for y in range(N):  # Widmanstatten: crossing bands of the metal's two crystals
            for x in range(N):
                if (x + y) % 8 == 0 or (x - y) % 8 == 3:
                    c[y, x] = fe["l"]
                elif (x + y) % 8 == 1:
                    c[y, x] = fe["d"]
        return c
    tex("iron-nickel", 2, iron_nickel)
    kr = R("kreep")
    tex("kreep", 3, lambda g, v: specks(specks(rock_top(g, kr, 1), g, kr["L"], 5), g, kr["d"], 4))
    return out




def write(pal, stem, variants, cutout):
    img = Image.new("RGBA", (N * len(variants), N))
    px = img.load()
    rgb = {}
    for v, c in enumerate(variants):
        for y in range(N):
            for x in range(N):
                name = c[y, x]
                if name is None:
                    assert cutout, f"{stem}: an empty texel on an opaque block"
                    px[v * N + x, y] = (0, 0, 0, 0)
                    continue
                if name not in rgb:
                    h = pal.rgb(name)
                    assert h, f"{stem}: '{name}' is not in the palette"
                    rgb[name] = tuple(int(h[i:i + 2], 16) for i in (1, 3, 5))
                px[v * N + x, y] = rgb[name] + (255,)
    path = OUT / f"{stem}.png"
    img.save(path)
    return path, len(set(rgb))


def preview(paths, dest):
    """A contact sheet: each texture tiled 4 x 4 with its variants mixed, 3x, a row of up to 8."""
    tiles = []
    for p in paths:
        im = Image.open(p).convert("RGBA")
        n = im.width // N
        rng = np.random.default_rng(1)
        t = Image.new("RGBA", (N * 4, N * 4), (255, 0, 255, 255))
        for j in range(4):
            for i in range(4):
                v = int(rng.integers(n))
                t.alpha_composite(im.crop((v * N, 0, v * N + N, N)), (i * N, j * N))
        tiles.append(t.resize((N * 12, N * 12), Image.NEAREST))
    cols = 8
    sheet = Image.new("RGB", (cols * 200, ((len(tiles) + cols - 1) // cols) * 200), (40, 40, 44))
    for k, t in enumerate(tiles):
        sheet.paste(t, ((k % cols) * 200 + 4, (k // cols) * 200 + 4), t)
    Path(dest).parent.mkdir(parents=True, exist_ok=True)
    sheet.save(dest)


def main(argv):
    want = [a for a in argv if not a.startswith("--")]
    prev = argv[argv.index("--preview") + 1] if "--preview" in argv else None
    if prev:
        want.remove(prev)
    pal = Palette(DATA / "palette.toml")
    blocks = tomllib.loads((DATA / "blocks.toml").read_text(encoding="utf-8"))["block"]
    cut = {b["name"] for b in blocks if b.get("see") == "cutout"}
    out = build(pal, blocks)
    OUT.mkdir(exist_ok=True)
    painted, paths = set(), []
    for stem, variants in out.items():
        block = stem.split("@")[0].removesuffix("_top").removesuffix("_side").removesuffix("_bottom")
        if want and block not in want:
            continue
        p, k = write(pal, stem, variants, block in cut)
        paths.append(p)
        painted.add(block)
        if k > 6:
            print(f"note: {stem} uses {k} colours", file=sys.stderr)
    pal.save()
    drafts = DATA / "drafts.txt"
    have = drafts.read_text(encoding="utf-8").split("\n") if drafts.exists() else []
    have = [h for h in have if h.strip()]
    for p in paths:
        rel = p.relative_to(ROOT).as_posix()
        if rel not in have:
            have.append(rel)
    drafts.write_text("\n".join(have) + "\n", encoding="utf-8", newline="\n")
    missing = [b["name"] for b in blocks if b["name"] not in painted | SKIP and b.get("solid", True)]
    print(f"{len(paths)} files, {len(painted)} blocks, {len(pal.added)} palette colours added")
    if missing and not want:
        print("no recipe yet: " + ", ".join(missing), file=sys.stderr)
    if prev:
        preview(paths, prev)


if __name__ == "__main__":
    main(sys.argv[1:])
