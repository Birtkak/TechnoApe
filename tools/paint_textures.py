#!/usr/bin/env python3
"""Paint every block's texture into data/textures/ from data/palette.toml (ADR 0084).

    python -I tools/paint_textures.py            # every block with a recipe
    python -I tools/paint_textures.py sand log   # only these
    python -I tools/paint_textures.py --preview out/tex-preview.png   # also a tiled contact sheet

The style (the developer's call, 2026-11-12): calm and readable. Each block takes three or four colours of one
ramp (dark, shade, mid, lit, light), low contrast between neighbours, and a material shape instead of noise.
A run for some blocks still builds every recipe, so the palette gains any step the whole set needs.
A ramp step the palette lacks is added to data/palette.toml the first time a texture uses it (`shade` between
dark and mid, `lit` a third of the way from mid to light; a block with no ramp gets one from its blocks.toml
colour), so a style change is an edit here and a run. Every file it writes goes on data/drafts.txt.
gravel.png is the developer's and water is not a block texture here: neither is touched.
"""
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
        # colours are unique in the palette: the nearest free blue, searched outward
        for d in (0, 1, -1, 2, -2, 3, -3, 4, -4, 5, -5, 6, -6, 7, -7, 8, -8):
            hx = "#%02x%02x%02x" % (rgb[0], rgb[1], min(255, max(0, rgb[2] + d)))
            if hx not in taken:
                break
        else:
            raise SystemExit(f"no free colour near {rgb} for '{name}'")
        row = {"name": name, "rgb": hx, "ramp": ramp}
        same = [i for i, r in enumerate(self.rows) if r.get("ramp") == ramp]
        lum = luma
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


def luma(h):
    """The Studio's brightness (crates/studio/src/ramp.rs `luma`), so the two order a ramp alike."""
    return sum(int(h[i:i + 2], 16) * w for i, w in ((1, .299), (3, .587), (5, .114)))


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
            c[y, x] = (r["l"] if rng.random() < 0.35 else r["m"]) if y == 0 else (r["m"] if y < h - 1 else r["s"])
        if h < 5 and rng.random() < 0.7:
            c[h, x] = soil["d"]
    return c


def dirt(rng, r):
    c = grain(rng, r, "sml", (0.28, 0.5, 0.22), 4, 0.45)
    specks(c, rng, r["d"], 5, 1)
    return c


def blobs(rng, share):
    """Where `share` of the texels open: round blobs about one or two random centres (wrapping), a rough rim.
    A smooth noise field would do, but can be lowest along a whole row: a slit across the block."""
    ys, xs = np.mgrid[0:N, 0:N]
    d = np.full((N, N), 99.0)
    for _ in range(int(rng.integers(1, 3))):
        cx, cy = rng.random() * N, rng.random() * N
        dx, dy = np.abs(xs - cx), np.abs(ys - cy)
        d = np.minimum(d, np.hypot(np.minimum(dx, N - dx), np.minimum(dy, N - dy)))
    d = d + rng.random((N, N)) * 0.8
    return d < np.quantile(d, share)


def leaves(rng, r, holes=0.06, cell=4):
    """Calm cutout leaves: two close tones in soft clumps, the lit tone on clump tops only, a few holes in one or
    two blobs. A hole shows the dark inside of the crown (fancy) or the leaf colour darker (fast, the default),
    so many scattered holes read as a black checker from afar."""
    f = field(rng, cell, 0.2)
    mask = ~blobs(rng, holes)
    c = levels(f, [r["s"], r["m"], r["l"]], (0.35, 0.5, 0.15), mask)
    for y in range(N):
        for x in range(N):
            if mask[y, x] and not mask[(y - 1) % N, x]:
                c[y, x] = r["s"]  # under a hole: in shadow
    c[~mask] = None
    return c


def needles(rng, r, holes=0.06):
    c = np.full((N, N), r["s"], object)
    for _ in range(22):  # short slanted needles on a shaded twig ground
        x, y = int(rng.integers(N)), int(rng.integers(N))
        d = int(rng.choice((-1, 1)))
        for i in range(3):
            put(c, x + i * d, y + i, r["m"] if i else r["l"])
    f = field(rng, 4, 0.2)
    c[f < np.quantile(f, holes)] = None
    return c


def branches(rng, r, limbs=2, twigs=5):
    """Bare winter crown: a few limbs rising through the block, twigs forking off them, the rest open."""
    c = np.empty((N, N), object)
    for _ in range(limbs):
        x, y, lean = int(rng.integers(N)), N - 1, int(rng.choice((-1, 1)))
        pts = []
        for _ in range(N):  # a limb leans out as it rises, as a crown spreads
            put(c, x, y, r["s"])
            pts.append((x, y))
            if rng.random() < 0.45:
                x += lean
                put(c, x, y, r["s"])
            y -= 1
        for _ in range(twigs):
            x, y = pts[int(rng.integers(2, len(pts)))]
            d = int(rng.choice((-1, 1)))
            for i in range(1, int(rng.integers(3, 6))):
                put(c, x + i * d, y - i // 2 - (i % 2), r["m"])
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


def lamp(rng, r):
    """A frame two texels wide (dark outside, shade inside) round an even glow of soft lit and light patches:
    no core, so lamps side by side, whose joined frames the shader reads from inside (ADR 0085), glow as one
    surface. The frame's width is the shader's JOIN_FRAME."""
    ys, xs = np.mgrid[0:N, 0:N]
    edge = np.minimum(np.minimum(xs, ys), np.minimum(15 - xs, 15 - ys))
    c = levels(field(rng, 4, 0.1), [r["m"], r["l"], r["L"]], (0.08, 0.62, 0.3))
    c[edge == 1] = r["s"]
    c[edge == 0] = r["d"]
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


# ---------------------------------------------------------------- flora (docs/design/flora.md part D)
# Each species reads apart by its shape first (needles in tiers or clumps, round, lobed, hanging, broad, frond,
# lance leaves; white, plated, ridged, ringed, jointed bark), its colour second. Holes gather in blobs or run
# along the shape (a willow's strands, a frond's leaflets): scattered holes read as a checker (ADR 0084).


def holes_in(c, rng, share):
    """Open `share` of the texels in one or two round blobs."""
    c[blobs(rng, share)] = None
    return c


def oval(c, cx, cy, rx, ry, name, top=None):
    """A filled ellipse (wrapping); its upper-left rim in `top` (lit from above)."""
    for dy in range(-ry, ry + 1):
        for dx in range(-rx, rx + 1):
            q = (dx / (rx + 0.5)) ** 2 + (dy / (ry + 0.5)) ** 2
            if q <= 1:
                put(c, cx + dx, cy + dy, top if top and q > 0.45 and dx + dy < 0 else name)


def oak_leaves(rng, r):
    """Big lobed clumps, deep green: broad patches (cell 8) with notched rims round one blob of hole."""
    c = levels(field(rng, 8, 0.15), [r["s"], r["m"], r["l"]], (0.4, 0.45, 0.15))
    holes_in(c, rng, 0.08)
    for y in range(N):  # notches: every other texel on a hole's rim darker, a lobed edge
        for x in range(N):
            if c[y, x] is not None and (x + y) % 2 and any(c[(y + dy) % N, (x + dx) % N] is None
                                                           for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1))):
                c[y, x] = r["s"]
    return c


def birch_leaves(rng, r):
    """Small round leaves, light and airy: two-texel leaves lit on top over a mid ground, more open than others."""
    c = levels(field(rng, 2, 0.3), [r["s"], r["m"]], (0.35, 0.65))
    for _ in range(20):
        x, y = int(rng.integers(N)), int(rng.integers(N))
        put(c, x, y, r["l"])
        put(c, x + 1, y, r["l"] if rng.random() < 0.5 else r["L"])
        put(c, x, y + 1, r["m"])
    return holes_in(c, rng, 0.12)


def willow_leaves(rng, r):
    """Hanging leaves: strands that fall from the top of the block, each lit at its head, with long open slits
    between them, so a crown reads as a curtain."""
    c = np.empty((N, N), object)
    k = np.random.default_rng(int(rng.integers(1 << 30)))
    for x in range(N):
        if k.random() < 0.22:
            continue  # a slit
        y0, length = int(k.integers(N)), int(k.integers(10, 17))
        tone = r[str(k.choice(list("sm")))]
        for i in range(length):
            put(c, x, y0 + i, r["l"] if i == 0 else (r["s"] if i == length - 1 else tone))
    return c


def spruce_needles(rng, r):
    """Needles in drooping tiers: every five rows a spray whose top edge is lit and whose needles hang below it."""
    c = np.full((N, N), r["s"], object)
    for t, y0 in enumerate((1, 6, 11)):
        off = int(rng.integers(N))
        for x in range(N):
            droop = int(round(1.2 * np.sin((x + off) / 16 * 6.283 * 2)))
            put(c, x, y0 + droop, r["l"])
            put(c, x, y0 + droop + 1, r["m"])
            if (x + t) % 2:
                put(c, x, y0 + droop + 2, r["m"])
    return holes_in(c, rng, 0.06)


def pine_needles(rng, r):
    """Needles in clumps: rosettes of rays round a dark twig end, lit at the tips; open where no clump reaches."""
    c = np.empty((N, N), object)
    k = 0
    while k < 9:
        cx, cy = int(rng.integers(N)), int(rng.integers(N))
        k += 1
        for dy in range(-3, 4):  # the clump's body
            for dx in range(-3, 4):
                if dx * dx + dy * dy <= 8:
                    put(c, cx + dx, cy + dy, r["s"])
        for dx, dy in ((1, 0), (-1, 0), (0, -1), (1, -1), (-1, -1), (1, 1), (-1, 1), (0, 1)):
            for i in (1, 2, 3):
                put(c, cx + dx * i, cy + dy * i, r["l"] if i == 3 and dy <= 0 else r["m"])
        put(c, cx, cy, r["d"])
    return c


def jungle_leaves(rng, r):
    """Big broad leaves, dense and dark: three or four long ovals with a shaded midrib, lit at their tip."""
    c = levels(field(rng, 4, 0.2), [r["d"], r["s"]], (0.3, 0.7))
    for _ in range(4):
        cx, cy = int(rng.integers(N)), int(rng.integers(N))
        oval(c, cx, cy, 4, 2, r["m"], r["l"])
        for dx in range(-3, 4):
            put(c, cx + dx, cy, r["s"])
    return holes_in(c, rng, 0.04)


def acacia_leaves(rng, r):
    """Fine feathery leaflets in flat layers: fine two-tone grain, the layer's top lit, slits open between."""
    f = field(rng, 2, 0.4)
    c = levels(f, [r["s"], r["m"]], (0.4, 0.6))
    ys = np.arange(N)[:, None]
    gap = field(rng, 4, 0.2) * 0.5 + np.sin((ys + rng.integers(8)) / 8 * 6.283) * 0.8
    open_ = gap < np.quantile(gap, 0.16)
    for y in range(N):
        for x in range(N):
            if not open_[y, x] and open_[(y - 1) % N, x]:
                c[y, x] = r["l"]  # the top of a layer, in the sun
    c[open_] = None
    return c


def palm_fronds(rng, r):
    """A frond across the block: a pale rib on the slant and long leaflets hanging from it every other texel,
    lit at the rib, shade at the tip, open between: the crown reads as a drooping feather."""
    c = np.empty((N, N), object)
    off = int(rng.integers(N))
    for i in range(0, N, 2):
        x0, y0 = i, (off + i // 2) % N  # the rib climbs one texel every two
        for k in range(1, 8):
            put(c, x0 + (k // 4), y0 + k, r["l"] if k < 3 else (r["m"] if k < 6 else r["s"]))
    for i in range(N):
        put(c, i, (off + i // 2) % N, r["L"])
    return c[:, ::-1].copy() if rng.random() < 0.5 else c


def bamboo_leaves(rng, r):
    """Long narrow lance leaves in slanted sprays, light green, tips light."""
    c = np.empty((N, N), object)
    for _ in range(13):
        x, y, d = int(rng.integers(N)), int(rng.integers(N)), int(rng.choice((-1, 1)))
        for i in range(7):  # a leaf: two texels wide in the middle, one at the ends
            put(c, x + i * d, y + i // 2, r["L"] if i == 6 else r["m"])
            if 1 < i < 5:
                put(c, x + i * d, y + i // 2 + 1, r["s"])
    return c


def birch_bark(rng, r, seed):
    """White bark with black lenticels: short dark dashes across, a few dark patches, faint grey grain."""
    c = levels(field(rng, 4, 0.2), [r["l"], r["L"]], (0.4, 0.6))
    k = np.random.default_rng(seed)
    for _ in range(12):
        x, y = int(rng.integers(N)), int(rng.integers(N))
        for i in range(int(rng.integers(2, 5))):
            put(c, x + i, y, r["d"])
    for _ in range(2):  # a dark knot patch where a branch fell
        x, y = int(k.integers(N)), int(rng.integers(N))
        oval(c, x, y, 1, 1, r["s"])
        put(c, x, y, r["d"])
    return c


def plated_bark(rng, r, seed, w=4, h=6):
    """Bark of plates (pine: big, spruce: small), each plate one tone with a lit top, parted by dark furrows;
    columns of uneven width, plate ends staggered."""
    c = np.empty((N, N), object)
    k = np.random.default_rng(seed)
    x0 = 0
    while x0 < N:
        cw = int(k.integers(w - 1, w + 2))
        y = int(k.integers(h))
        while y < N + h:
            ph = int(k.integers(h - 1, h + 3))
            tone = r[str(k.choice(list("sml"), p=(0.3, 0.45, 0.25)))]
            for yy in range(y, y + ph):
                for x in range(x0, min(N, x0 + cw)):
                    put(c, x, yy, r["d"] if x == x0 or yy == y + ph - 1 else (r["l"] if yy == y and tone != r["l"] else tone))
            y += ph
        x0 += cw
    # the last column wraps onto the first: what the loop left before the first plate start
    for y in range(N):
        for x in range(N):
            if c[y, x] is None:
                c[y, x] = r["s"]
    return c


def ridged_bark(rng, r, seed):
    """Oak: deep vertical ridges broken across into blocks, ridges lit, furrows dark."""
    k = np.random.default_rng(seed)
    col = vnoise(k, 4)[0]
    c = np.empty((N, N), object)
    for x in range(N):
        for y in range(N):
            c[y, x] = r["d"] if col[x] < 0.3 else (r["s"] if col[x] < 0.5 else (r["m"] if col[x] < 0.8 else r["l"]))
    for _ in range(6):  # breaks across the ridges
        x, y = int(rng.integers(N)), int(rng.integers(N))
        for i in range(3):
            put(c, x + i, y, r["d"])
    return c


def diamond_bark(rng, r, seed):
    """Willow: furrows crossing on the slant into long diamonds."""
    c = levels(field(rng, 4, 0.3), [r["s"], r["m"], r["l"]], (0.3, 0.5, 0.2))
    off = int(np.random.default_rng(seed).integers(N))
    for y in range(N):
        for x in range(N):
            if (x * 2 + y + off) % 16 < 2 or (x * 2 - y + off) % 16 < 2:
                c[y, x] = r["d"]
    return c


def mossy_bark(rng, r, moss):
    """Jungle: smooth pale bark with green moss in soft patches."""
    c = levels(field(rng, 4, 0.2), [r["s"], r["m"], r["l"]], (0.25, 0.55, 0.2))
    f = field(rng, 4, 0.1)
    c[f > np.quantile(f, 0.8)] = moss["s"]
    c[f > np.quantile(f, 0.93)] = moss["m"]
    cracks(c, rng, r["d"], 1, 4, 7, True)
    return c


def ringed_bark(rng, r):
    """Palm: rings of leaf scars every four rows, each a dark line under a lit lip, criss-cross fibre between."""
    c = levels(field(rng, 2, 0.3), [r["s"], r["m"]], (0.4, 0.6))
    for y0 in (0, 4, 8, 12):
        for x in range(N):
            c[y0, x] = r["l"]
            c[(y0 + 3) % N, x] = r["d"] if x % 5 else r["s"]
    return c


def culm(rng, r, seed):
    """Bamboo: a green stem with a node every eight rows (a light ring over a dark line), faint stripes."""
    c = np.empty((N, N), object)
    for x in range(N):
        c[:, x] = r["s"] if x % 8 in (0, 7) else (r["l"] if x % 8 in (3, 4) else r["m"])
    y0 = int(np.random.default_rng(seed).integers(8))
    for y in (y0, y0 + 8):
        c[y % N, :] = r["L"]
        c[(y + 1) % N, :] = r["d"]
    return c


def end_rings(rng, wood, bark, hollow=False):
    """A log's end; `hollow`: bamboo's tube, a ring of culm round a dark hole."""
    if not hollow:
        return rings(rng, wood, bark)
    ys, xs = np.mgrid[0:N, 0:N]
    d = np.hypot(xs - 7.5, ys - 7.5)
    c = np.full((N, N), wood["m"], object)
    c[d > 5.5] = bark["m"]
    c[d > 7.2] = bark["s"]
    c[d < 4.6] = wood["d"]
    return c


def bloom(c, rng, petal, centre, n=9):
    """Blossom over a crown: small flowers of four petals round a centre, on the leaves only."""
    for _ in range(n):
        x, y = int(rng.integers(N)), int(rng.integers(N))
        for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
            if c[(y + dy) % N, (x + dx) % N] is not None:
                put(c, x + dx, y + dy, petal)
        put(c, x, y, centre)
    return c


def fruit(c, rng, skin, shine, n=6, size=2):
    """Fruit hanging in a crown: round fruit of `size`, a light glint top left."""
    for _ in range(n):
        x, y = int(rng.integers(N)), int(rng.integers(N))
        for dy in range(size):
            for dx in range(size):
                put(c, x + dx, y + dy, skin)
        put(c, x, y, shine)
    return c


# small plants (flora.md part B, ADR 0091): one upright plane a file in data/plants/, 32 px a metre, each its own
# size. They are painted at half that, 16 texels a metre like the blocks, and written doubled, so a plant's
# texels match the ground's. A canvas is (height, width), empty where the plane is open; its bottom row stands
# on the ground. Green texels (g above r and b) take the climate's tint in the game, others keep their colour.


def put_in(c, x, y, name):
    """Like put, but clipped: a plane does not wrap."""
    if 0 <= y < c.shape[0] and 0 <= x < c.shape[1]:
        c[y, x] = name


def tufts(rng, r, w, h, n, lo, hi, lean=0.25):
    """Blades from the bottom row, `lo`..`hi` tall, leaning as they rise, dark at the foot, lit at the tip."""
    c = np.empty((h, w), object)
    for _ in range(n):
        x, bh = int(rng.integers(w)), int(rng.integers(lo, hi + 1))
        d, fx = float(rng.choice((-lean, lean))), float(x)
        for i in range(bh):
            put_in(c, int(round(fx)), h - 1 - i, r["l"] if i == bh - 1 else (r["s"] if i < bh // 3 else r["m"]))
            fx += d * (i / bh)
    return c


def ears(rng, r, straw, w, h, n=5):
    """Wild wheat: straw stems with an ear of alternating light and lit texels over a few green blades."""
    c = tufts(rng, r, w, h, 7, 3, h // 2)
    for _ in range(n):
        x, sh = 1 + int(rng.integers(w - 2)), int(rng.integers(h * 2 // 3, h + 1))
        for i in range(sh):
            y = h - 1 - i
            if i >= sh - 5:  # the ear
                put_in(c, x, y, straw["L"] if i % 2 else straw["l"])
                put_in(c, x + (1 if i % 2 else -1), y, straw["m"])
            else:
                put_in(c, x, y, straw["s"] if i < 3 else straw["m"])
    return c


def fern(rng, r, w, h):
    """Fronds arching out of one foot: each rib rises to its top at 0.6 of its length and droops after it,
    leaflets above (lit at the tip) and below that shorten towards the end."""
    c = np.empty((h, w), object)
    for side, span, top in ((-1, 0.48, 0.75), (1, 0.48, 0.75), (-1, 0.25, 0.95), (1, 0.25, 0.95)):
        span, top = span * w, top * (h - 1)
        span += float(rng.random())
        for i in range(25):
            t = i / 24
            x, y = (w - 1) / 2 + side * span * t, h - 1 - top * (1 - ((t - 0.6) / 0.6) ** 2)
            ox, oy = int(round(x)), int(round(y))
            put_in(c, ox, oy, r["s"])
            leaf = int(round(2 * (1 - t)))
            for k in range(1, leaf + 1):
                put_in(c, ox, oy - k, r["l"] if k == leaf else r["m"])
                put_in(c, ox, oy + k, r["m"] if k < leaf else r["s"])
    return c


def flowers(rng, r, petals, w, h, n=3):
    """Stems with a leaf each and a head of petals (one ramp a stem): a cross, a lit one, a dark centre."""
    c = tufts(rng, r, w, h, 4, 2, h // 2)
    for k in range(n):
        x, sh = 2 + (k * (w - 4)) // n + int(rng.integers(max(1, (w - 4) // n))), int(rng.integers(h - 3, h))
        for i in range(sh):
            put_in(c, x, h - 1 - i, r["s"] if i < 2 else r["m"])
        put_in(c, x - 1, h - 3, r["m"])
        petal = petals[k % len(petals)]
        y = h - sh
        for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
            put_in(c, x + dx, y + dy, petal["m"])
        put_in(c, x - 1, y - 1, petal["l"])
        put_in(c, x, y, petal["d"])
    return c


def reeds(rng, r, head, w, h):
    """Tall straight stems, two with a brown cattail head."""
    c = tufts(rng, r, w, h, w // 2 + 1, h // 2, h, 0.06)
    for k in range(2):
        x, top = 1 + k * (w // 2) + int(rng.integers(max(1, w // 2 - 2))), int(rng.integers(1, 5))
        for y in range(top, h):
            put_in(c, x, y, r["s"] if y > h - 4 else r["m"])
        for y in range(top + 1, top + 6):
            put_in(c, x, y, head["m"])
            put_in(c, x + 1, y, head["s"])
        put_in(c, x, top + 1, head["l"])
    return c


def bush(rng, r, w, h, look=None):
    """A round bush of small leaves on the ground, a few holes; `look` (ramp, kind): berries or blossom."""
    c = np.empty((h, w), object)
    ys, xs = np.mgrid[0:h, 0:w]
    k = np.random.default_rng(int(rng.integers(1 << 30)))
    f = k.random((h, w))
    shape = np.hypot((xs - (w - 1) / 2) / (w / 2), (ys - h * 0.62) / (h * 0.62)) + (f - 0.5) * 0.18
    tone = (xs - w / 2) * -0.04 + (ys - h / 2) * -0.06 + f * 0.5  # lit up and to the left
    inside = shape <= 1
    order = np.argsort(tone[inside])
    names = np.empty(order.size, object)
    cut = np.round(np.cumsum((0.35, 0.5, 0.15)) * order.size).astype(int)
    names[order[:cut[0]]], names[order[cut[0]:cut[1]]], names[order[cut[1]:]] = r["s"], r["m"], r["l"]
    c[inside] = names
    c[(f < 0.06) & (shape < 0.75)] = None
    if look:
        ramp, kind = look
        for _ in range(w * h // 28):
            x, y = int(k.integers(1, w - 1)), int(k.integers(1, h - 1))
            if c[y, x] is None:
                continue
            if kind == "fruit":
                put_in(c, x, y, ramp["m"])
                put_in(c, x + 1, y, ramp["s"])
                put_in(c, x, y - 1, ramp["L"])
            else:
                for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
                    if 0 <= y + dy < h and 0 <= x + dx < w and c[y + dy, x + dx] is not None:
                        c[y + dy, x + dx] = ramp["m"]
                put_in(c, x, y, ramp["l"])
    return c


# ---------------------------------------------------------------- the blocks

# Names the other flora sessions choose (their lists in the main checkout's out/, flora.md): where a list is
# missing, `<species> leaves`, `<species> log` and the plant's own name. A painting whose block does not exist yet,
# or whose look has no season file (`@fruit`), goes to out/flora/pending/ instead of data/textures/.
PENDING = ROOT / "out" / "flora" / "pending"
TAGS = {"spring", "autumn", "winter"}


def flora_list(name):
    """The rows of a sibling session's list (this checkout's out/ or the main checkout's), comments dropped."""
    for d in (ROOT / "out", ROOT.parents[2] / "out"):
        f = d / name
        if f.exists():
            return [l for l in f.read_text(encoding="utf-8").splitlines() if l.strip() and not l.startswith("#")]
    return []


def tree_blocks():
    """Block names from the trees session's list: `<id> <name>  <words>` (the name ends at two spaces)."""
    return {l.split(None, 1)[1].split("  ")[0].strip() for l in flora_list("flora-tree-blocks.txt")}


def plant_sizes():
    """Each plant painting's size in pixels from the plants session's list: `data/plants/<name>.png<TAB>W x H px`."""
    out = {}
    for l in flora_list("flora-plant-paintings.txt"):
        path, _, dims = l.partition("\t")
        w, _, h = dims.replace("px", "").partition("x")
        out[path.removeprefix("data/plants/").removesuffix(".png")] = (int(w), int(h))
    return out



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
    gravel_mix = [("tin gravel", 230), ("schist", 227), ("quartzite dark", 210),
                  ("shale", 107), ("gold gravel", 103), ("shale light", 86), ("chalcopyrite", 35),
                  ("regolith dark", 25)]
    sand = R("sand")

    def sand_face(g, v, top):
        c = grain(g, sand, "sml", (0.22, 0.56, 0.22), 2, 0.6)
        if top:  # wind ripples: faint lit crests every few rows
            for y0 in (2, 7, 12):
                for x in range(N):
                    put(c, x, y0 + int(round(np.sin(x / 16 * 6.283 + v) * 0.8)), sand["l"])
        specks(c, g, sand["d"], 4)
        specks(c, g, "sandstone dark", 2)
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

    # shrub and reed: the old cutout blocks
    leaf_look = {
        "shrub": lambda g, r: leaves(g, r, 0.08, 3),
        "reed": lambda g, r: blades(g, r),
    }
    for name, fn in leaf_look.items():
        b = defs[name]
        for s in ("summer", "spring", "autumn", "winter"):
            if s != "summer" and s not in b:
                continue
            r = R(name, b["colour"]) if s == "summer" else R(f"{name} {s}", b[s]["colour"])
            at = "" if s == "summer" else f"@{s}"
            if s == "winter" and name != "reed":  # a bare shrub: twigs in the bark's colours
                tex(f"{name}{at}", 2, lambda g, v, n=name: branches(np.random.default_rng(seed_of(n, v)), R("log"), 1, 3))
            else:
                tex(f"{name}{at}", 2, lambda g, v, r=r, fn=fn, n=name: fn(np.random.default_rng(seed_of(n, v)), r))

    # trees by species (docs/design/flora.md): leaves in every season the species has, bark, the log's end.
    # Leaf colours sit near the mid green a climate tint will multiply (part B), species apart by a step of hue.
    blossom, catkin = R("blossom", [0.95, 0.80, 0.84]), R("catkin", [0.84, 0.80, 0.38])

    def catkins(c, g, k, n=8, wide=1):
        for _ in range(n):  # short tassels hanging from the leaves
            x, y = int(g.integers(N)), int(g.integers(N))
            for i in range(3):
                for w in range(wide):
                    put(c, x + w, y + i, k["L"] if i == 0 else (k["m"] if i == 1 else k["s"]))
        return c

    autumn_orange = Ramp(pal, "autumn orange", names={"d": "autumn rust", "L": "autumn gold"}, into="autumn")
    species = {
        # leaf painter, leaf colour by season (None: blocks.toml's), bark painter, bark, wood, looks
        "broadleaf": dict(leaf=leaves, bark=bark, log="log", barkc=None, woodc=[0.74, 0.58, 0.38],
                          spring=lambda c, g: bloom(c, g, blossom["m"], catkin["m"]),
                          autumn=autumn_orange,
                          fruit=lambda c, g: fruit(c, g, R("apple", [0.78, 0.18, 0.12])["m"],
                                                   R("apple")["L"], 6, 2)),
        "oak": dict(leaf=oak_leaves, bark=ridged_bark, colour=[0.18, 0.36, 0.13], barkc=[0.38, 0.35, 0.30],
                    woodc=[0.70, 0.52, 0.32], spring_colour=[0.32, 0.50, 0.17], autumn_colour=[0.62, 0.38, 0.14],
                    fruit=lambda c, g: fruit(c, g, R("acorn", [0.62, 0.46, 0.20])["m"], R("acorn")["l"], 7, 2)),
        "birch": dict(leaf=birch_leaves, twigs=R("log"), bark=birch_bark, colour=[0.40, 0.56, 0.22], barkc=[0.86, 0.85, 0.80],
                      bark_names={"d": "log dark"}, woodc=[0.88, 0.80, 0.62], spring_colour=[0.46, 0.62, 0.24],
                      spring=lambda c, g: catkins(c, g, catkin, 7), autumn_colour=[0.86, 0.68, 0.18]),
        "willow": dict(leaf=willow_leaves, bark=diamond_bark, colour=[0.46, 0.56, 0.36], barkc=[0.40, 0.36, 0.30],
                       woodc=[0.80, 0.70, 0.52], spring_colour=[0.48, 0.60, 0.26],
                       spring=lambda c, g: catkins(c, g, catkin, 6, 2), autumn_colour=[0.70, 0.64, 0.24]),
        "spruce": dict(leaf=spruce_needles, bark=lambda g, r, s: plated_bark(g, r, s, 4, 5),
                       barkc=[0.32, 0.22, 0.17], woodc=[0.82, 0.68, 0.48]),
        "pine": dict(leaf=pine_needles, bark=lambda g, r, s: plated_bark(g, r, s, 5, 6), colour=[0.18, 0.36, 0.24],
                     barkc=[0.50, 0.28, 0.18], woodc=[0.84, 0.68, 0.44]),
        "jungle": dict(leaf=jungle_leaves, bark=lambda g, r, s: mossy_bark(g, r, R("moss", [0.30, 0.42, 0.18])),
                       barkc=[0.55, 0.50, 0.40], woodc=[0.74, 0.56, 0.38],
                       fruit=lambda c, g: fruit(c, g, R("tropical fruit", [0.90, 0.52, 0.14])["m"],
                                                R("tropical fruit")["L"], 5, 2)),
        "acacia": dict(leaf=acacia_leaves, bark=lambda g, r, s: mossy_bark(g, r, R("acacia underbark", [0.70, 0.42, 0.22])),
                       barkc=[0.44, 0.40, 0.35], woodc=[0.78, 0.48, 0.30]),
        "palm": dict(leaf=palm_fronds, bark=lambda g, r, s: ringed_bark(g, r), colour=[0.40, 0.58, 0.18],
                     barkc=[0.56, 0.46, 0.32], woodc=[0.80, 0.70, 0.50],
                     fruit=lambda c, g: fruit(c, g, R("coconut", [0.42, 0.28, 0.14])["m"], R("coconut")["l"], 3, 3)),
        "bamboo": dict(leaf=bamboo_leaves, bark=culm, colour=[0.40, 0.62, 0.20], barkc=[0.46, 0.62, 0.26],
                       woodc=[0.86, 0.82, 0.62], hollow=True),
    }
    named = tree_blocks()
    for sp, t in species.items():
        leaf_b = f"{sp} leaves"
        log_b = t.get("log") or (sp if sp in named else f"{sp} log")  # bamboo's stalk is `bamboo`
        known = defs.get(leaf_b)
        leaf_r = R(leaf_b, known["colour"] if known else t["colour"])
        bark_r = R(f"{sp} bark", t["barkc"], t.get("bark_names")) if t["barkc"] else R("log")
        wood_r = R("log wood", t["woodc"]) if sp == "broadleaf" else R(f"{sp} wood", t["woodc"])
        leaf = lambda r, v, fn=t["leaf"], b=leaf_b: fn(np.random.default_rng(seed_of(b, v)), r)
        tex(leaf_b, 4, lambda g, v, r=leaf_r, leaf=leaf: leaf(r, v))
        if "spring_colour" in t or "spring" in t:
            sr = R(f"{leaf_b} spring", (known or {}).get("spring", {}).get("colour") or t.get("spring_colour", t.get("colour")))
            add = t.get("spring", lambda c, g: c)
            tex(f"{leaf_b}@spring", 4, lambda g, v, r=sr, leaf=leaf, add=add: add(leaf(r, v), g))
        if "autumn" in t or "autumn_colour" in t:
            ar = t.get("autumn") or R(f"{leaf_b} autumn", (known or {}).get("autumn", {}).get("colour") or t["autumn_colour"])
            tex(f"{leaf_b}@autumn", 4, lambda g, v, r=ar, leaf=leaf: leaf(r, v))
            # deciduous: bare branches in winter, in the species' bark
            tex(f"{leaf_b}@winter", 4, lambda g, v, b=leaf_b, br=t.get("twigs", bark_r): branches(np.random.default_rng(seed_of(b, v)), br))
        if "fruit" in t:  # late summer: no season file holds it yet (flora.md part A), so it waits in out/
            tex(f"{leaf_b}@fruit", 4, lambda g, v, r=leaf_r, leaf=leaf, f=t["fruit"]: f(leaf(r, v), g))
        tex(f"{log_b}_side", 3, lambda g, v, br=bark_r, fn=t["bark"], b=log_b: fn(g, br, seed_of(b, "bark")))
        tex(log_b, 2, lambda g, v, w=wood_r, br=bark_r, h=t.get("hollow", False): end_rings(g, w, br, h))

    # small plants (flora.md part B): the grass block's own ramp, so a meadow and its ground are one field and one
    # tint fits both; straw, petals, cattails and fruit are not green and keep their colour under the tint
    G = R("grass")
    straw, cattail, berry = R("straw", [0.80, 0.72, 0.40]), R("cattail", [0.42, 0.28, 0.15]), R("berry", [0.76, 0.12, 0.16])
    pet = {k: R(f"{k} petal", rgb) for k, rgb in {"red": [0.80, 0.18, 0.14], "yellow": [0.92, 0.78, 0.20],
                                                    "blue": [0.44, 0.42, 0.82], "white": [0.92, 0.92, 0.86]}.items()}
    shrub = R("shrub", defs["shrub"]["colour"])
    size = plant_sizes()

    def plant(name, fn):
        w, h = size.get(name, (32, 32))
        out[f"plant:{name}"] = [fn(np.random.default_rng(seed_of("plant", name)), w // 2, h // 2)]

    plant("short grass", lambda g, w, h: tufts(g, G, w, h, w * 2 // 3, 2, h))
    plant("short grass.1", lambda g, w, h: tufts(g, G, w, h, w * 2 // 3, 2, h))
    plant("tall grass", lambda g, w, h: tufts(g, G, w, h, w // 2 + 2, h * 3 // 5, h, 0.4))
    plant("tall grass.1", lambda g, w, h: tufts(g, G, w, h, w // 2 + 2, h * 3 // 5, h, 0.4))
    plant("wild wheat", lambda g, w, h: ears(g, G, straw, w, h))
    plant("fern", lambda g, w, h: fern(g, G, w, h))
    plant("reeds", lambda g, w, h: reeds(g, G, cattail, w, h))
    plant("flowers", lambda g, w, h: flowers(g, G, [pet["red"], pet["white"]], w, h))
    plant("flowers.1", lambda g, w, h: flowers(g, G, [pet["yellow"]], w, h))
    plant("flowers.2", lambda g, w, h: flowers(g, G, [pet["blue"], pet["white"]], w, h))
    # the bush's three looks from one seed: the same leaves, then blossom (late spring), then berries
    plant("berry bush", lambda g, w, h: bush(np.random.default_rng(seed_of("berry bush")), shrub, w, h))
    plant("berry bush.bloom", lambda g, w, h: bush(np.random.default_rng(seed_of("berry bush")), shrub, w, h,
                                                   (blossom, "bloom")))
    plant("berry bush.fruit", lambda g, w, h: bush(np.random.default_rng(seed_of("berry bush")), shrub, w, h,
                                                   (berry, "fruit")))


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

    # lamps, metal
    tex("lamp", 4, lambda g, v: lamp(g, R("lamp")))
    tex("blue lamp", 4, lambda g, v: lamp(g, R("blue lamp")))
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

    def granite(g, v, r=gr, dark=5, pale=6):
        c = grain(g, r, "sm", (0.3, 0.7), 4, 0.25)
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
            start = x
            while x < start + N:
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




def write(pal, stem, variants, cutout, folder=OUT):
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
    path = folder / f"{stem}.png"
    img.save(path)
    return path, set(rgb)


def write_plane(pal, path, c):
    """A plant's plane, each texel doubled (32 px a metre); open texels alpha 0."""
    h, w = c.shape
    img = Image.new("RGBA", (w * 2, h * 2), (0, 0, 0, 0))
    px = img.load()
    for y in range(h):
        for x in range(w):
            if c[y, x] is None:
                continue
            hx = pal.rgb(c[y, x])
            assert hx, f"{path.name}: '{c[y, x]}' is not in the palette"
            rgb = tuple(int(hx[i:i + 2], 16) for i in (1, 3, 5)) + (255,)
            for dy in (0, 1):
                for dx in (0, 1):
                    px[x * 2 + dx, y * 2 + dy] = rgb
    img.save(path)
    return path, {n for n in c.flat if n is not None}


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
    prev = None
    if "--preview" in argv:
        k = argv.index("--preview")
        if k + 1 >= len(argv):
            raise SystemExit("--preview needs a file to write")
        prev = argv[k + 1]
    if prev:
        want.remove(prev)
    pal = Palette(DATA / "palette.toml")
    blocks = tomllib.loads((DATA / "blocks.toml").read_text(encoding="utf-8"))["block"]
    cut = {b["name"] for b in blocks if b.get("see") == "cutout"}
    out = build(pal, blocks)
    OUT.mkdir(exist_ok=True)
    painted, paths, pending, used = set(), [], [], set()
    names = {b["name"] for b in blocks}
    for stem, variants in out.items():
        if stem.startswith("plant:"):  # data/plants/ once the plants branch made it, else waiting
            name = stem.removeprefix("plant:")
            if want and name.split(".")[0] not in want:
                continue
            folder = DATA / "plants" if (DATA / "plants").is_dir() else PENDING / "plants"
            folder.mkdir(parents=True, exist_ok=True)
            p, k = write_plane(pal, folder / f"{name}.png", variants[0])
            (paths if folder.parent == DATA else pending).append(p)
            used |= k if folder.parent == DATA else set()
            continue
        block = stem.split("@")[0].removesuffix("_top").removesuffix("_side").removesuffix("_bottom")
        if want and block not in want:
            continue
        tag = stem.split("@")[1] if "@" in stem else None
        if block not in names or (tag and tag not in TAGS):
            PENDING.mkdir(parents=True, exist_ok=True)
            pending.append(write(pal, stem, variants, True, PENDING)[0])
            continue
        p, k = write(pal, stem, variants, block in cut)
        paths.append(p)
        painted.add(block)
        used |= k
        if len(k) > 6:
            print(f"note: {stem} uses {len(k)} colours", file=sys.stderr)
    # a colour only waiting paintings use joins the palette when their block does
    early = set(pal.added) - used
    pal.rows = [r for r in pal.rows if r["name"] not in early]
    pal.added = [a for a in pal.added if a not in early]
    for name in {r.get("ramp") for r in pal.rows if r["name"] in pal.added}:
        steps = [luma(r["rgb"]) for r in pal.ramp_of(name)]
        assert steps == sorted(steps), f"ramp '{name}' does not run dark to light"
    pal.save()
    drafts = DATA / "drafts.txt"
    have = drafts.read_text(encoding="utf-8").split("\n") if drafts.exists() else []
    have = [h for h in have if h.strip()]
    for p in ([DATA / "palette.toml"] if pal.added else []) + paths:
        rel = p.relative_to(ROOT).as_posix()
        if rel not in have:
            have.append(rel)
    drafts.write_text("\n".join(have) + "\n", encoding="utf-8", newline="\n")
    missing = [b["name"] for b in blocks if b["name"] not in painted | SKIP and b.get("solid", True)]
    print(f"{len(paths)} files, {len(painted)} blocks, {len(pal.added)} palette colours added, "
          f"{len(pending)} waiting in {PENDING.relative_to(ROOT).as_posix()}")
    if missing and not want:
        print("no recipe yet: " + ", ".join(missing), file=sys.stderr)
    if prev:
        preview(paths, prev)


if __name__ == "__main__":
    main(sys.argv[1:])
