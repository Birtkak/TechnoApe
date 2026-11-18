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
        self.before = {r["name"] for r in self.rows}  # another tool's colours (paint_clutter.py's ramps) stay
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


def put_in(c, x, y, name):
    """Like put, but clipped: no wrap."""
    if 0 <= y < c.shape[0] and 0 <= x < c.shape[1]:
        c[y, x] = name


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


def edge_holes(rng, share, n=2, wide=1.0, size=N):
    """Where a crown block opens: up to `share` of its texels (a variant has from none to all of it), in `n` ragged
    clusters that each start on the block's edge and wander in, `wide` > 1 stretching them sideways. Holes on a
    regular spot each block read as polka dots; at the edge they break the cube's outline instead."""
    N = size  # noqa: N806 (the block's size, 16, or flora's 32)
    m = np.zeros((N, N), bool)
    want = int(round(share * N * N * rng.uniform(0.0, 1.0)))
    starts = []
    for _ in range(n):
        t, e = int(rng.integers(N)), int(rng.integers(4))
        starts.append([(t, 0), (t, N - 1), (0, t), (N - 1, t)][e])
    k = 0
    while m.sum() < want and k < 4000:
        x, y = starts[k % len(starts)]
        for _ in range(int(rng.integers(2, 6))):  # a short ragged walk from the cluster's own texels
            m[y % N, x % N] = True
            if rng.random() < wide / (1 + wide):
                x += int(rng.choice((-1, 1)))
            else:
                y += int(rng.choice((-1, 1)))
        k += 1
    return m


def leaves(rng, r, holes=0.06, cell=4, n=2):
    """Calm cutout leaves: two close tones in soft clumps, the lit tone on clump tops only, a few holes in one or
    two blobs. A hole shows the dark inside of the crown (fancy) or the leaf colour darker (fast, the default),
    so many scattered holes read as a black checker from afar."""
    f = field(rng, cell, 0.2)
    mask = ~edge_holes(rng, holes, n)
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


# Bare winter crowns (the developer, 2026-11-13: twigs must grow out of a log or branch, not float). Every stroke
# starts on wood: the painting's wood is its BOTTOM edge, and the four variants are roles the crown's shader picks
# and turns so that edge faces the neighbouring log or branch (ADR 0090): root, root off-centre, through, tips.
WINTER_ROLES = ("root", "root off-centre", "through", "tips")


def grow(c, rng, r, x, y, lean, thick, steps, forks):
    """A limb from (x, y) upward: `thick` texels wide (a dark side right, bark mid left), one thinner as it
    rises, forking thinner twigs off itself that lean out; a thin twig ends in a lit tip."""
    fx = float(x)
    for i in range(steps):
        w = max(1, thick - (i * thick) // max(1, steps))  # tapers toward the far edge
        cx = int(round(fx))
        if not 0 <= cx < N:
            break  # off the painting: a fork from out there would start in mid-air when it leaned back in
        for k in range(w):
            put_in(c, cx - w // 2 + k, y, r["s"] if w == 1 else (r["d"] if k == w - 1 else r["m"]))
        if w == 1 and i == steps - 1:
            put_in(c, cx, y, r["l"])
        if forks and i >= 2 and rng.random() < 0.3 and y > 1:
            side = -1 if rng.random() < 0.5 else 1
            grow(c, rng, r, cx + side, y - 1, side * (0.5 + rng.random() * 0.5), max(1, w - 1),
                 int(rng.integers(3, max(4, min(y, steps - i) + 1))), forks - 1)
        fx += float(np.clip(lean + (rng.random() - 0.5) * 0.4, -1, 1))  # one texel a row at most: no gaps
        y -= 1
        if y < 0:
            break


def bare(rng, r, role):
    """One winter role on a 16 x 16 canvas, wood at the bottom edge (row 15)."""
    c = np.empty((N, N), object)
    if role in ("root", "root off-centre"):
        x0, lean = (7, (rng.random() - 0.5) * 0.3) if role == "root" else (4, 0.35)
        grow(c, rng, r, x0, N - 1, lean, 3, 14 if role == "root" else 13, 3)
        # two limbs leave the stem low (rows 12 and 10), so the lower half fans out on its own: a face with the
        # wood straight behind it shows only that half, folded about the middle (ADR 0090)
        for y, side in ((12, -1), (10, 1)):
            x = next((x for x in range(N) if c[y, x] is not None), x0) if side < 0 else                 max((x for x in range(N) if c[y, x] is not None), default=x0)
            grow(c, rng, r, x + side, y - 1, side * (0.7 + rng.random() * 0.4), 2, int(rng.integers(6, 10)), 1)
    elif role == "through":  # a limb from the bottom edge to the top edge at x 8 on every block: blocks chain
        forks = set(int(y) for y in rng.choice(np.arange(3, N - 2), 2, replace=False))
        for y in range(N):
            x = 8 + int(round(np.sin(y / (N - 1) * np.pi) * 1.5))
            put_in(c, x, y, r["m"])
            put_in(c, x + 1, y, r["d"])
            if 2 < y < N - 2 and (y in forks or rng.random() < 0.2):
                side = -1 if rng.random() < 0.5 else 1
                grow(c, rng, r, x + (side if side < 0 else 2), y - 1, side * 0.7, 1, int(rng.integers(3, 7)), 1)
    else:  # tips: a few fine twigs off the edge, short and open
        for x in (3, 8, 12):
            grow(c, rng, r, x + int(rng.integers(-1, 2)), N - 1, (rng.random() - 0.5) * 0.8, 1,
                 int(rng.integers(5, 10)), 1)
    # every texel reaches the wood edge through its neighbours (eight ways): nothing floats
    seen = {(N - 1, x) for x in range(N) if c[N - 1, x] is not None}
    todo = list(seen)
    while todo:
        y, x = todo.pop()
        for dy in (-1, 0, 1):
            for dx in (-1, 0, 1):
                q = (y + dy, x + dx)
                if 0 <= q[0] < N and 0 <= q[1] < N and q not in seen and c[q] is not None:
                    seen.add(q)
                    todo.append(q)
    assert len(seen) == sum(v is not None for v in c.flat), f"winter {role}: a twig floats"
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


def holes_in(c, rng, share, n=2, wide=1.0):
    """Open up to `share` of the texels in ragged clusters from the block's edges."""
    c[edge_holes(rng, share, n, wide)] = None
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
    holes_in(c, rng, 0.1, 1)
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
    return holes_in(c, rng, 0.12, 5)


def willow_leaves(rng, r):
    """Hanging leaves: strands that hang from the block's top edge (so they run on from the block above), each lit
    at its head, ending at different lengths, with open slits between them: a curtain."""
    c = np.empty((N, N), object)
    k = np.random.default_rng(int(rng.integers(1 << 30)))
    for x in range(N):
        if k.random() < 0.18:
            continue  # a slit
        y0, length = 0, int(k.integers(11, 17))
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
    return holes_in(c, rng, 0.08, 2, 3.0)


def pine_needles(rng, r):
    """Needles in clumps: rosettes of rays round a dark twig end, lit at the tips, over a ground of dark needles;
    a few ragged holes at the edge."""
    c = levels(field(rng, 2, 0.4), [r["d"], r["s"]], (0.45, 0.55))
    for _ in range(7):
        cx, cy = int(rng.integers(N)), int(rng.integers(N))
        for dx, dy in ((1, 0), (-1, 0), (0, -1), (1, -1), (-1, -1), (1, 1), (-1, 1), (0, 1)):
            for i in (1, 2, 3):
                put(c, cx + dx * i, cy + dy * i, r["l"] if i == 3 and dy <= 0 else r["m"])
        put(c, cx, cy, r["d"])
    return holes_in(c, rng, 0.12, 3)


def jungle_leaves(rng, r):
    """Big broad leaves, dense and dark: three or four long ovals with a shaded midrib, lit at their tip."""
    c = levels(field(rng, 4, 0.2), [r["d"], r["s"]], (0.3, 0.7))
    for _ in range(4):
        cx, cy = int(rng.integers(N)), int(rng.integers(N))
        oval(c, cx, cy, 4, 2, r["m"], r["l"])
        for dx in range(-3, 4):
            put(c, cx + dx, cy, r["s"])
    return holes_in(c, rng, 0.05, 3)


def acacia_leaves(rng, r):
    """Fine feathery leaflets in flat layers: fine two-tone grain, the layer's top lit, slits open between."""
    f = field(rng, 2, 0.4)
    c = levels(f, [r["s"], r["m"]], (0.4, 0.6))
    ys = np.arange(N)[:, None]
    gap = field(rng, 4, 0.2) * 0.5 + np.sin((ys + rng.integers(8)) / 8 * 6.283) * 0.8
    open_ = gap < np.quantile(gap, 0.1)
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
    """Long narrow lance leaves in slanted sprays, light green, tips light, over a ground of shaded leaves."""
    c = levels(field(rng, 2, 0.4), [r["s"], r["m"]], (0.55, 0.45))
    for _ in range(11):
        x, y, d = int(rng.integers(N)), int(rng.integers(N)), int(rng.choice((-1, 1)))
        for i in range(7):  # a leaf: two texels wide in the middle, one at the ends
            put(c, x + i * d, y + i // 2, r["L"] if i == 6 else r["l"])
            if 1 < i < 5:
                put(c, x + i * d, y + i // 2 + 1, r["m"])
    return holes_in(c, rng, 0.1, 3, 2.0)


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


# ---------------------------------------------------------------- flora at 32 px a metre (the developer, 2026-11-13)
# "Still too low quality of details and colours": flora is painted at twice the blocks' density, 32 x 32 a face
# and 32 px a metre on plant planes, in RICH colour instead of the calm rule: eight tones a ramp, the lights warm
# toward yellow and the shadows cool, saturated, a dark accent inside every mass. Leaves are single leaf shapes in
# clumps lit from the top left, back clumps darker; grass is blades from a dark root to a bright tip; bark is a
# height field per species shaded by the same light. Written to data/textures/hd/ (the 16 x 16 files stay the
# fallback) and data/plants/.
HD = 32
LIGHT = np.array([-0.55, -0.65, 0.52]) / np.linalg.norm([-0.55, -0.65, 0.52])


def rich_rgb(hue, sat=1.0, light=0.0, n=8):
    """Eight tones dark to light. Lightness 0.09..0.58 (the references' range); the hue walks toward a warm
    yellow (55 degrees) in the lights and away from it in the shadows; saturation highest in the mids."""
    import colorsys
    out = []
    for i in range(n):
        t = i / (n - 1)
        lt = 0.09 + 0.49 * t ** 1.1 + light
        pull = (55 - hue) * 0.45 * t  # lights warm
        push = (hue - 55) * 0.25 * (1 - t) * (1 if hue > 55 else 0.6)  # shadows cool (greens) or redder (autumn)
        h = (hue + pull + push) % 360
        s = min(1.0, sat * (0.95 - 0.35 * abs(t - 0.45)))
        out.append(colorsys.hls_to_rgb(h / 360, min(0.92, max(0.04, lt)), s))
    return out


# the meadow's greens, from reference 4: a deep saturated green, yellow only in the lightest (the tips); little red
# in the mids, so the plants' climate tint (a multiply, red x0.75 blue x0.5 in a middling climate) keeps them green
MEADOW = [tuple(v / 255 for v in q) for q in [(6, 40, 4), (10, 58, 6), (16, 78, 9), (22, 96, 11), (30, 116, 14),
                                              (42, 138, 18), (80, 162, 22), (152, 184, 36)]]


def turned(ramp, deg):
    """A ramp with its hue turned by `deg` degrees (a variant a step off)."""
    import colorsys
    out = []
    for c in ramp:
        h, l, s_ = colorsys.rgb_to_hls(*c)
        out.append(colorsys.hls_to_rgb((h + deg / 360) % 1, l, s_))
    return out


def far(c, k=4):
    """A quarter-size painting: each 4x4 cell takes its most common colour where a quarter of it is filled, so
    the outline and the main colours stay and thin details drop out."""
    h, w = c.shape
    o = np.full(((h + k - 1) // k, (w + k - 1) // k), None, object)
    for j in range(o.shape[0]):
        for i in range(o.shape[1]):
            cell = [v for v in c[j * k:(j + 1) * k, i * k:(i + 1) * k].flat if v is not None]
            if len(cell) * 4 >= k * k:
                o[j, i] = max(sorted(set(cell)), key=cell.count)  # sorted: ties fall the same way every run
    return o


def hd_put(c, x, y, v, wrap=True):
    h, w = c.shape
    if wrap:
        c[y % h, x % w] = v
    elif 0 <= y < h and 0 <= x < w:
        c[y, x] = v


def stamp_leaf(c, x, y, tone, shape, flip, wrap=True):
    """One leaf: its body in `tone`, the upper-left rim one lighter, the lower-right rim and midrib darker.
    `shape` is a list of rows (strings, '#' the leaf, 'r' its midrib); `flip` mirrors it."""
    rows = [r[::-1] for r in shape] if flip else shape
    hgt = len(rows)

    def open_at(dy, dx):
        return not (0 <= dy < hgt and 0 <= dx < len(rows[dy])) or rows[dy][dx] == "."

    for dy, row in enumerate(rows):
        for dx, ch in enumerate(row):
            if ch == ".":
                continue
            if open_at(dy - 1, dx) or open_at(dy, dx - 1):
                t = tone + 1  # the rim toward the light
            elif open_at(dy + 1, dx) or open_at(dy, dx + 1) or ch == "r":
                t = tone - 1
            else:
                t = tone
            hd_put(c, x + dx, y + dy, max(1, min(7, t)), wrap)


LEAF_SHAPES = {  # small sprites of one leaf
    "oval": [[".##.", "####", ".##."], ["##..", "###.", ".###", "..##"], [".##", "###", "##."]],
    "lobed": [[".#.#.", "#####", ".###.", "#####", ".#.#."], ["#.#..", "####.", ".####", "..#.#"]],
    "small": [["##", "##"], [".#", "##", "#."], ["##."]],
    "broad": [["..######..", ".########.", "#rrrrrrrr#", ".########.", "..######.."],
              ["...####...", ".########.", "#rrrrrrrr#", "##########", ".#######.."]],
    "leaflet": [["#.#.#", "#####"], ["##.##"], ["#####"]],
    "lance": [["##.....", ".###...", "...###.", ".....##"], [".....##", "...###.", ".###...", "##....."]],
}


def leaf_mass(rng, shapes, size=(HD, HD), clumps=11, radius=(6, 10), flat=1.0, spacing=3, wrap=True, mask=None,
              holes=0.0, nholes=2):
    """Leaves in clumps with depth: a dark accent fills the mass (tone 0), clumps are drawn back to front, each lit
    as a ball from the top left (tones 1..7), a back clump two tones darker; inside a clump single leaves are
    stamped top to bottom so the lower overlap the upper. Returns a canvas of tone indices (None open)."""
    h, w = size
    c = np.full((h, w), 0, object)
    if mask is not None:
        c[~mask] = None
    inside = mask if mask is not None else np.ones((h, w), bool)
    drawn = 0
    # clumps until the dark accent shows only in gaps (about 8 % of the mass), the first `clumps` set back
    while drawn < clumps or (np.mean([c[i] != 0 for i in zip(*np.nonzero(inside))]) < 0.92 and drawn < clumps * 4):
        back = max(0.0, 1.0 - drawn / max(1, clumps - 1))  # 1 for the first drawn, 0 from the clumps-th on
        drawn += 1
        r = rng.uniform(*radius)
        cx, cy = rng.uniform(0, w), rng.uniform(0, h)
        leaves = []
        for gy in np.arange(-r, r + 1, spacing * 0.8):
            for gx in np.arange(-r * flat, r * flat + 1, spacing):
                x, y = gx + rng.uniform(-1, 1), gy + rng.uniform(-1, 1)
                q = (x / (r * flat)) ** 2 + (y / r) ** 2
                if q <= 1:
                    leaves.append((y, x, q))
        for y, x, q in sorted(leaves):
            n = np.array([x / (r * flat), y / r, np.sqrt(max(0.0, 1 - q))])
            lit = max(0.0, float(n @ LIGHT))
            # a leaf a step off only now and then: clumps a quarter block wide must read from 10 to 20 m, not speckle
            tone = 1 + round(lit * 5.5) - round(2 * back) + (int(rng.integers(-1, 2)) if rng.random() < 0.3 else 0)
            px, py = int(round(cx + x)), int(round(cy + y))
            if not wrap and mask is not None and not (0 <= py < h and 0 <= px < w and mask[py, px]):
                continue
            stamp_leaf(c, px, py, max(1, min(7, tone)), shapes[int(rng.integers(len(shapes)))], rng.random() < 0.5,
                       wrap)
    if mask is not None:
        c[~mask] = None
    if holes:
        c[edge_holes(rng, holes, nholes, 1.0, h)] = None
    return c


def willow_hd(rng):
    """A dense curtain from the top edge: strands two texels apart, each a run of narrow leaves (three long, one
    wide) that alternate left and right of their thread, darker deeper down and on the right; narrow slits."""
    c = np.full((HD, HD), None, object)
    for x in range(0, HD, 2):
        if rng.random() < 0.12:
            continue
        length, y0 = int(rng.integers(20, 29)), int(rng.integers(HD))  # tips at their own height, wrapping
        lift = int(rng.integers(-1, 2)) + (1 if (x // 2) % 3 == 0 else 0)
        for i in range(length):
            y = y0 + i
            t = max(1, min(7, 6 + lift - i // 8 - (1 if x % 4 else 0)))
            hd_put(c, x, y, max(0, t - 2))  # the thread, in shade
            if i % 3 != 2:  # a leaf three long on one side, then the other
                side = 1 if (i // 3 + x // 2) % 2 else -1
                hd_put(c, x + side, y, min(7, t + (1 if side < 0 and i % 3 == 0 else 0)))
                if i % 3 == 1:
                    hd_put(c, x - side, y, max(1, t - 1))  # the far leaf of the pair, in shade
    return c



def spruce_hd(rng):
    """Drooping sprays in tiers: each spray a twig 8 to 16 texels long at its own height, needles hanging below it,
    lit on top and in cool shade beneath, sprays overlapping; a dark accent between them."""
    c = np.full((HD, HD), 1, object)
    sprays = [(int(rng.integers(HD)), t0 + int(rng.integers(-2, 3)), int(rng.integers(8, 17)))
              for t0 in range(0, HD, 6) for _ in range(3)]
    for x0, y0, ln in sorted(sprays, key=lambda q: q[1]):
        droop = rng.uniform(0.15, 0.35)
        for i in range(ln):
            x = x0 + i
            y = y0 + int(round(droop * abs(i - ln / 2)))  # the spray arches down at both ends
            for k in range(int(rng.integers(4, 7))):
                tone = 6 - k + int(rng.integers(-1, 1))
                hd_put(c, x + (k // 2 if i > ln / 2 else -(k // 2)), y + k, max(1, min(7, tone)))
            hd_put(c, x, y, 7 if rng.random() < 0.35 else 6)
    c[edge_holes(rng, 0.06, 2, 3.0, HD)] = None
    return c



def pine_hd(rng):
    """Needle clumps: many overlapping rosettes of rays of uneven length from a dark twig end, the upper rays
    lit, the lower in shade, over a mid-dark ground of needles."""
    c = np.full((HD, HD), 2, object)
    for y in range(HD):
        for x in range(HD):
            if rng.random() < 0.35:
                c[y, x] = 1 if rng.random() < 0.6 else 3
    for _ in range(9):  # fewer, bigger rosettes: clusters that read from 20 m
        cx, cy = int(rng.integers(HD)), int(rng.integers(HD))
        for a in np.linspace(0, 6.283, 18, endpoint=False) + rng.uniform(0, 0.35):
            dx, dy = np.cos(a), np.sin(a)
            lit = max(0.0, -dx * 0.5 - dy * 0.85)
            for i in range(1, int(rng.integers(6, 10))):
                hd_put(c, cx + int(round(dx * i)), cy + int(round(dy * i)),
                       max(1, min(7, 2 + round(lit * 4.5) + (1 if i > 3 else 0))))
        hd_put(c, cx, cy, 0)
    c[edge_holes(rng, 0.08, 3, 1.0, HD)] = None
    return c



def palm_hd(rng):
    """Two or three fronds crossing the face at their own slant: a pale rib, leaflets of uneven length hanging
    from both sides, lit at the rib and shaded toward their tips, some leaflets missing."""
    c = np.full((HD, HD), None, object)
    for _ in range(int(rng.integers(2, 4))):
        y0, slope, side = int(rng.integers(HD)), rng.uniform(0.25, 0.75), 1 if rng.random() < 0.5 else -1
        rib = [(i, y0 + int(round(i * slope))) for i in range(HD)]
        for i, (x, y) in enumerate(rib):
            if i % 2 or rng.random() < 0.12:
                continue
            for down in (1, -1):
                ln = int(rng.integers(6, 14)) if down > 0 else int(rng.integers(3, 8))
                for k in range(1, ln):
                    t = 6 - (k * 5) // ln + (1 if i % 4 == 0 else 0)
                    hd_put(c, (x + side * k // 4) * side, y + down * k, max(1, min(7, t)))
                    if down > 0 and k < ln - 2:
                        hd_put(c, (x + side * k // 4) * side + 1, y + k, max(1, min(7, t - 2)))
        for x, y in rib:
            hd_put(c, x * side, y, 7)
    return c



def cactus_hd(rng, top):
    """A cactus face: ribs (four on a side, each lit on its left and in shade on its right, a dark groove between),
    areoles on the crests with pale spines (tones + 10); the top: the ribs meeting at a lighter crown."""
    ys, xs = np.mgrid[0:HD, 0:HD].astype(float)
    if top:
        a = np.arctan2(ys - 15.5, xs - 15.5)
        r = np.hypot(ys - 15.5, xs - 15.5)
        rib = 0.5 + 0.5 * np.cos(a * 8)
        c = np.clip(np.round(2 + rib * 3 + (16 - r) / 8 + rng.normal(0, 0.4, (HD, HD))), 1, 7).astype(int).astype(object)
        for k in range(8):
            for rr in (5, 10, 14):
                x, y = 15.5 + rr * np.cos(k * np.pi / 4), 15.5 + rr * np.sin(k * np.pi / 4)
                hd_put(c, int(x), int(y), 10 + 6, False)
        return c
    u = xs % 8 / 8  # across a rib; the same in every variant, so the ribs meet at the seams
    t = 2 + 4 * np.sin(u * np.pi) - 1.2 * u + rng.normal(0, 0.35, (HD, HD))
    c = np.clip(np.round(t), 1, 7).astype(int).astype(object)
    c[u < 0.13] = 1
    for x in range(HD):
        if abs(u[0, x] - 0.45) < 0.07:
            y0 = int(rng.integers(6))
            for y in range(y0, HD, 6):
                hd_put(c, x, y, 10 + 6)
                hd_put(c, x - 1, y - 1, 10 + 4)
                hd_put(c, x + 1, y - 1, 10 + 5)
    return c


def hd_twigs(rng, role):
    """Winter at 32: limbs from the bottom edge (ADR 0090's roles), tones 1..7 from the bark ramp: the limb's
    left side lit, its right side dark, thinning to lit tips; every texel reaches the wood edge."""
    c = np.full((HD, HD), None, object)

    def grow(x, y, lean, thick, steps, forks):
        fx = float(x)
        for i in range(steps):
            w = max(1, thick - (i * thick) // max(1, steps))
            cx = int(round(fx))
            if not 0 <= cx < HD:
                break
            for k in range(w):
                hd_put(c, cx - w // 2 + k, y, 5 if k == 0 and w > 1 else (1 if k == w - 1 and w > 1 else 3), False)
            if w == 1 and i == steps - 1:
                hd_put(c, cx, y, 6, False)
            if forks and i >= 2 and rng.random() < 0.22 and y > 2:
                side = -1 if rng.random() < 0.5 else 1
                grow(cx + side, y - 1, side * (0.5 + rng.random() * 0.6), max(1, w - 1),
                     int(rng.integers(5, max(6, min(y, steps - i) + 1))), forks - 1)
            fx += float(np.clip(lean + (rng.random() - 0.5) * 0.4, -1, 1))  # one texel a row at most: no gaps
            y -= 1
            if y < 0:
                break

    if role in ("root", "root off-centre"):
        x0, lean = (15, rng.uniform(-0.1, 0.1)) if role == "root" else (8, 0.35)
        grow(x0, HD - 1, lean, 5, 28, 4)
        for y, side in ((25, -1), (21, 1), (17, -1)):  # low limbs: the folded lower half fans on its own
            row = [x for x in range(HD) if c[y, x] is not None]
            x = (min(row) if side < 0 else max(row)) if row else x0
            grow(x + side, y - 1, side * rng.uniform(0.7, 1.1), 3, int(rng.integers(10, 16)), 2)
    elif role == "through":
        for y in range(HD):
            x = 16 + int(round(np.sin(y / (HD - 1) * np.pi) * 2.5))
            hd_put(c, x - 1, y, 5, False)
            hd_put(c, x, y, 3, False)
            hd_put(c, x + 1, y, 1, False)
            if 3 < y < HD - 3 and rng.random() < 0.18:
                side = -1 if rng.random() < 0.5 else 1
                grow(x + 2 * side, y - 1, side * 0.8, 2, int(rng.integers(6, 12)), 1)
    else:
        for x in (5, 13, 21, 27):
            grow(x + int(rng.integers(-2, 3)), HD - 1, rng.uniform(-0.4, 0.4), 2, int(rng.integers(10, 20)), 2)
    seen = {(HD - 1, x) for x in range(HD) if c[HD - 1, x] is not None}
    todo = list(seen)
    while todo:
        y, x = todo.pop()
        for dy in (-1, 0, 1):
            for dx in (-1, 0, 1):
                q = (y + dy, x + dx)
                if 0 <= q[0] < HD and 0 <= q[1] < HD and q not in seen and c[q] is not None:
                    seen.add(q)
                    todo.append(q)
    assert len(seen) == sum(v is not None for v in c.flat), f"winter {role} at 32: a twig floats"
    return c


# bark: a height field per species, shaded by the light from the left: a ridge's left side lit, its right in shade


def shade_height(hf, rng, base=3.2, relief=3.0, slope=6.0, jitter=0.6):
    left = np.roll(hf, 1, axis=1)
    right = np.roll(hf, -1, axis=1)
    up = np.roll(hf, 1, axis=0)
    t = base + relief * (hf - 0.5) * 2 + slope * ((right - left) * -0.5 + (hf - up) * -0.3) + rng.normal(0, jitter, hf.shape)
    return np.clip(np.round(t), 0, 7).astype(int).astype(object)


def wnoise(rng, cell, size=HD):
    """Tileable smooth noise of `size`, features about `cell` texels."""
    n = max(1, size // cell)
    g = rng.random((n, n))
    ys, xs = np.mgrid[0:size, 0:size] / (size / n)
    x0, y0 = np.floor(xs).astype(int), np.floor(ys).astype(int)
    tx, ty = xs - x0, ys - y0
    tx, ty = tx * tx * (3 - 2 * tx), ty * ty * (3 - 2 * ty)
    a, b = g[y0 % n, x0 % n], g[y0 % n, (x0 + 1) % n]
    c, d = g[(y0 + 1) % n, x0 % n], g[(y0 + 1) % n, (x0 + 1) % n]
    return (a + (b - a) * tx) * (1 - ty) + (c + (d - c) * tx) * ty


def calm(c, lo, hi):
    """Squeeze a bark's tones 0..7 into lo..hi: close browns or greys, the furrows only a little darker, so a trunk
    reads as bark from 9 m and does not draw the eye from the crown (the overseer, 2026-11-13: oak read as zebra)."""
    return np.vectorize(lambda t: None if t is None else int(round(lo + (hi - lo) * t / 7)), otypes=[object])(c)


def bark_hd(rng, kind):
    ys, xs = np.mgrid[0:HD, 0:HD].astype(float)
    if kind == "ridged":  # oak, broadleaf: ridges of uneven width along the trunk, furrows that wander and part
        streak = wnoise(rng, 3)[(ys.astype(int) // 6) % HD, xs.astype(int)]  # noise stretched along the trunk
        warp = wnoise(rng, 8) * 4
        hf = 0.45 * (0.5 + 0.5 * np.sin((xs + warp) / HD * 6.283 * 4)) + 0.55 * streak
        for _ in range(8):  # ridges broken across
            x, y = int(rng.integers(HD)), int(rng.integers(HD))
            hf[y, x:x + 3] *= 0.6
        return calm(shade_height(hf, rng, 3.4, 2.4, 3.0, 0.4), 1, 4)
    if kind == "diamond":  # willow: furrows crossing on the slant
        warp = wnoise(rng, 8) * 3
        u = (xs * 2 + ys + warp) % 16
        v = (xs * 2 - ys + warp) % 16
        hf = np.minimum(np.minimum(u, 16 - u), np.minimum(v, 16 - v)) / 8
        return calm(shade_height(hf * 0.6 + wnoise(rng, 4) * 0.4, rng, 3.4, 2.4, 3.0, 0.4), 2, 5)
    if kind in ("plates", "small plates"):  # pine, spruce: plates stretched along the trunk, each its own height, deep furrows
        n, stretch = (12, 0.55) if kind == "plates" else (22, 0.5)
        px, py, lvl = rng.uniform(0, HD, n), rng.uniform(0, HD, n), rng.uniform(0.45, 0.95, n)
        dx = np.abs(xs[..., None] - px + rng.normal(0, 0.6, (HD, HD, 1)))
        dy = np.abs(ys[..., None] - py)
        d = np.hypot(np.minimum(dx, HD - dx), np.minimum(dy, HD - dy) * stretch)  # wraps, so the bark tiles
        o = np.sort(d, axis=2)
        near = np.argmin(d, axis=2)
        edge = np.clip((o[..., 1] - o[..., 0]) / (2.2 if kind == "plates" else 1.6), 0, 1)
        hf = lvl[near] * (0.15 + 0.85 * edge) + wnoise(rng, 4) * 0.12
        return calm(shade_height(hf, rng, 3.0, 3.4, 4.0, 0.35), 1, 5)
    if kind == "birch":  # white bark: light tones, black lenticels across, dark patches, a faint peel
        c = np.clip(np.round(5.6 + wnoise(rng, 8) * 1.4 + rng.normal(0, 0.35, (HD, HD))), 4, 7).astype(int).astype(object)
        for _ in range(11):  # lenticels grey-dark, not black: they speckle at 9 m instead of striping
            x, y, n = int(rng.integers(HD)), int(rng.integers(HD)), int(rng.integers(3, 8))
            for i in range(n):
                hd_put(c, x + i, y, 2)
                if rng.random() < 0.4:
                    hd_put(c, x + i, y + 1, 3)
        for _ in range(3):
            x, y = int(rng.integers(HD)), int(rng.integers(HD))
            for dy in range(-2, 3):
                for dx in range(-3, 4):
                    if dx * dx / 9 + dy * dy / 4 <= 1:
                        hd_put(c, x + dx, y + dy, 2 if abs(dx) + abs(dy) > 2 else 1)
        return c
    if kind == "palm":  # leaf-scar rings every eight rows, a lip lit on top, fibre between
        ring = (ys % 8) / 8
        hf = 0.35 + 0.6 * ring + wnoise(rng, 2) * 0.15
        hf[(ys % 8) == 0] = 0.95
        return calm(shade_height(hf, rng, 3.4, 2.4, 3.0), 2, 5)
    if kind == "culm":  # bamboo: a round stem lit on the left, a node every sixteen rows
        hf = np.sin(xs / HD * np.pi) * 0.6 + 0.2
        c = np.clip(np.round(5.8 - xs / HD * 3 + rng.normal(0, 0.3, (HD, HD))), 2, 7).astype(int).astype(object)
        y0 = int(rng.integers(16))
        for y in (y0, y0 + 16):  # a node: a lit lip over a shaded groove, a step each, not a stripe
            c[y % HD, :] = 6
            c[(y + 1) % HD, :] = 3
        return c
    # "smooth": jungle and acacia, a pale smooth bark with soft bumps and a few cracks
    fibre = 0.5 + 0.5 * np.sin((xs + wnoise(rng, 4) * 5) / HD * 6.283 * 7)  # long fibres, soft bumps over them
    hf = wnoise(rng, 8) * 0.35 + wnoise(rng, 4) * 0.2 + fibre * 0.45
    c = calm(shade_height(hf, rng, 3.8, 2.6, 4.0, 0.4), 2, 5)
    for _ in range(3):
        x, y = int(rng.integers(HD)), int(rng.integers(HD))
        for i in range(int(rng.integers(5, 12))):
            hd_put(c, x, y + i, 1)
            x += int(rng.choice((-1, 0, 0, 1)))
    return c


def end_hd(rng, hollow=False):
    """A log's end at 32: rings of wood tones round a pith, a bark rim (tones 8.. are the bark's: returned as
    ('w', t) or ('b', t))."""
    ys, xs = np.mgrid[0:HD, 0:HD]
    cx, cy = 15.5 + rng.uniform(-1, 1), 15.5 + rng.uniform(-1, 1)
    d = np.hypot(xs - cx, ys - cy) + (wnoise(rng, 8) - 0.5) * 1.2
    c = np.empty((HD, HD), object)
    for y in range(HD):
        for x in range(HD):
            v = d[y, x]
            if hollow:
                c[y, x] = ("b", 3 if v > 14 else 5) if v > 12.5 else (("w", 0) if v < 10 else ("w", 4))
            elif v > 14.6:
                c[y, x] = ("b", 1 if v > 15.6 else 3)
            elif v < 1.5:
                c[y, x] = ("w", 1)
            else:
                ring = (v / 2.2) % 1
                c[y, x] = ("w", 2 if ring < 0.22 else (5 if int(v / 2.2) % 2 else 6) - (1 if x + y > 31 else 0))
    a = rng.uniform(0, 6.28)
    for t in range(3, 14):
        c[int(round(cy + np.sin(a) * t)) % HD, int(round(cx + np.cos(a) * t)) % HD] = ("w", 0)
    return c


# plants at 32 px a metre: tones are indices into the plant's ramp, None open; nothing wraps


def envelope(rng, w, peaks=(2, 4), low=0.04):
    """A tuft's outline across its width, 0..1: two or three bulges of their own height with dents between, low
    at the sides, so no tuft has a flat top or straight sides and every variant has its own."""
    xs = np.arange(w)
    env = np.full(w, 0.0)
    for _ in range(int(rng.integers(*peaks))):
        cx, wd = rng.uniform(0.3, 0.7) * w, rng.uniform(0.2, 0.32) * w
        env = np.maximum(env, rng.uniform(0.55, 1.0) * np.exp(-((xs - cx) / wd) ** 2))
    env = env / env.max() * np.sin(np.pi * (xs + 0.5) / w) ** 0.7  # falls to the sides: a fountain, not a wall
    return low + (1 - low) * env / env.max()


def blades_hd(rng, w, h, n, lo, hi, lean=0.35, wide=0.25, low=0.04):
    """`n` blades over the whole width, their heights following the tuft's own outline (`envelope`): a back
    layer, darker, and a lighter front layer; each blade from a dark root to a bright tip; outer blades lean
    out, so the sides are not straight; a quarter two texels wide at the root."""
    c = np.full((h, w), None, object)
    env = envelope(rng, w, low=low)
    # roots in the middle 80 %: the outer blades lean out to the sides, so no side is cut straight
    xs = [int(round(w * 0.1 + x * 0.8)) for x in range(w)] + [int(rng.integers(w // 10, w - w // 10)) for _ in range(max(0, n - w))]
    rng.shuffle(xs)
    half = len(xs) // 2
    for k, x in enumerate(xs):
        front = k >= half  # the back layer first, the front over it
        bh = max(2, int(round((lo * env[x] ** 0.5 + (hi - lo) * env[x]) * rng.uniform(0.6, 1.0))))  # low at the sides too
        lift = rng.uniform(-0.3, 1.2) if front else rng.uniform(-1.8, -0.6)
        out = (x - (w - 1) / 2) / (w / 2)  # -1 at the left side, 1 at the right
        d, fx = out * lean * 1.4 + rng.uniform(-lean, lean) * 0.5, float(x)
        two = rng.random() < wide
        for i in range(bh):
            t = i / max(1, bh - 1)
            # mostly mid green, the tip lit; the root only a step down, as the plants' shader darkens it to x0.45
            tone = int(round(2 + t ** 1.7 * 5 + lift * (0.4 + t * 0.6)))
            px = int(round(fx))
            hd_put(c, px, h - 1 - i, max(1, min(7, tone)), False)
            if two and i < bh * 0.4:
                hd_put(c, px + 1, h - 1 - i, max(1, min(7, tone - 1)), False)
            fx += d * t * 1.8
    return c


def ears_hd(rng, w, h):
    """Wild wheat: a bushy tuft, green blades low with dry straw blades among them, over it many stems arching
    out, each nodding ear a run of grains left and right with awns up (the gold ramp, tones + 10)."""
    c = blades_hd(rng, w, h, w * 2, h // 5, h // 2)
    for x in rng.choice(np.arange(w), w // 2, replace=False):  # dry blades among the green
        bh, fx, d = int(rng.integers(h // 4, h * 2 // 3)), float(x), rng.uniform(-0.5, 0.5)
        for i in range(bh):
            hd_put(c, int(round(fx)), h - 1 - i, 10 + min(7, 2 + i * 5 // bh), False)
            fx += d * i / bh
    for x in sorted(rng.choice(np.arange(3, w - 3), 13, replace=False), key=lambda _: rng.random()):
        sh = int(rng.integers(h // 2, h))
        lean = rng.uniform(0.2, 0.7) * (1 if x > w / 2 else -1)  # arching away from the tuft's middle
        ear = int(rng.integers(7, 12))
        fx = float(x)
        for i in range(sh):
            y, px = h - 1 - i, int(round(fx))
            if i >= sh - ear:  # the ear: grains alternate left and right, an awn up from each
                k = i - (sh - ear)
                side = -1 if k % 2 else 1
                hd_put(c, px, y, 10 + (5 if k % 2 else 3), False)
                hd_put(c, px + side, y, 10 + (7 if side < 0 else 4), False)
                if k % 2 and rng.random() < 0.7:
                    hd_put(c, px + 2 * side, y - 1, 10 + 6, False)
            else:
                hd_put(c, px, y, 10 + (2 if i < 4 else 3), False)
            fx += lean * (i / sh) ** 2 * 1.5
    return c


def fern_hd(rng, w, h):
    """Fronds arching out of the middle, each rib rising and drooping to its tip, leaflets in pairs that shorten
    toward the tip, upper leaflets lit, lower in shade; the outer fronds behind."""
    c = np.full((h, w), None, object)
    fronds = [(-1, 0.5, 0.5), (1, 0.5, 0.5), (-1, 0.36, 0.78), (1, 0.36, 0.78), (-1, 0.18, 0.95), (1, 0.18, 0.95),
              (-1, 0.05, 1.0)]
    fronds = [f for f in fronds if rng.random() < 0.85] or fronds  # each fern its own count of fronds
    tilt = rng.uniform(-0.15, 0.15)  # and its own lean, so a stand of ferns is not stamped
    for side, span, top in fronds:
        span = span * w * rng.uniform(0.7, 1.0) * (1 + tilt * side)
        top = top * (h - 1) * rng.uniform(0.75, 1.0)
        pts = []
        for i in range(61):
            t = i / 60
            x, y = (w - 1) / 2 + side * span * t, h - 1 - top * (1 - ((t - 0.55) / 0.55) ** 2)
            pts.append((int(round(x)), int(round(y)), t))
        for k, (x, y, t) in enumerate(pts):
            if k % 2 == 0:
                leaf = int(round(4.5 * (1 - t) + 0.5))
                for j in range(1, leaf + 1):
                    hd_put(c, x + side * (j // 3), y - j, 4 + (2 if j > leaf - 2 else 1) + (1 if top > h * 0.7 else 0) - 1, False)
                    hd_put(c, x - side * (j // 3), y + j, 3 + (1 if j < leaf else 0), False)
            hd_put(c, x, y, 2 if t < 0.15 else 3, False)  # the rib dark only at the root: no outline against the light
    return c


def flowers_hd(rng, w, h, n=6):
    """Low leaves and stems in green, each stem with a flower head of petals (tones + 10): a ring round a dark
    centre, the upper-left petals lit."""
    c = blades_hd(rng, w, h, w * 2, 2, h * 2 // 3, 0.4)
    for k in sorted(range(n), key=lambda _: rng.random()):  # big and small heads at their own heights
        x = 2 + (k * (w - 4)) // n + int(rng.integers(max(1, (w - 4) // n)))
        sh = int(rng.integers(h // 2, h - 2))
        big = 5 if k % 2 == 0 else 2
        for i in range(sh):
            hd_put(c, x, h - 1 - i, 2 if i < 3 else 4, False)
        for j in range(2):  # two leaves on the stem
            yy = h - 3 - j * 3
            hd_put(c, x - 1 - j, yy, 5, False)
            hd_put(c, x + 1 + j, yy - 1, 3, False)
        y = h - sh - 1
        for dy in range(-2, 3):
            for dx in range(-2, 3):
                q = dx * dx + dy * dy
                if q <= big:
                    hd_put(c, x + dx, y + dy, 10 + (1 if q == 0 else (7 if dx + dy < -1 else (5 if dx + dy <= 0 else 3))), False)
    return c


REED_KINDS = ("tall with heads", "tall", "short and leaning", "sparse, one broken")


def reeds_hd(rng, w, h, kind=0):
    """A reed stand of one of four kinds (`REED_KINDS`) that clearly differ: stalks two texels wide of uneven height
    (lit on the left), a long leaf or two arching off each, greens that vary with a few dry tan stalks (tones + 10),
    cattails on the tallest of the first kind (tones + 20); short blades low behind; the base dark."""
    c = np.full((h, w), None, object)
    tall = kind in (0, 1)
    if kind != 3:  # an under-layer of short blades behind the stalks
        under = blades_hd(rng, w, h, w * 2, 2, h // (4 if tall else 3), 0.3)
        c[under != None] = [max(1, t - 1) for t in under[under != None]]  # noqa: E711
    n = {0: 9, 1: 10, 2: 9, 3: 5}[kind]
    lean_all = rng.choice((-1, 1)) * rng.uniform(0.18, 0.3) if kind == 2 else 0.0
    stalks = []
    for k in range(n):
        x = w * (0.12 + 0.76 * (k + rng.uniform(0.1, 0.9)) / n)
        sh = int(h * (rng.uniform(0.55, 1.0) if tall else rng.uniform(0.3, 0.62)))
        dry = rng.random() < (0.35 if kind == 3 else 0.18)
        stalks.append((sh, x, dry))
    broken = int(rng.integers(n)) if kind == 3 else -1
    for k, (sh, x, dry) in enumerate(sorted(stalks)):  # short first: the tall stand in front
        base = 10 if dry else 0
        lift = rng.uniform(-0.8, 0.8)
        lean = lean_all + rng.uniform(-0.05, 0.05)
        kink = int(sh * rng.uniform(0.5, 0.7)) if k == broken % n and kind == 3 else None
        fx, fy = float(x), float(h - 1)
        dx, dy = lean, -1.0
        for i in range(sh):
            t = i / max(1, sh - 1)
            tone = int(round(1 + t * 4.5 + lift)) if i > h * 0.08 else 1  # the dark base
            px, py = int(round(fx)), int(round(fy))
            hd_put(c, px, py, base + max(1, min(7, tone + 1)), False)
            hd_put(c, px + 1, py, base + max(1, min(7, tone - 1)), False)
            if kink is not None and i == kink:  # the broken stalk folds over and hangs down to one side
                dx, dy = (1.0 if x < w / 2 else -1.0) * 0.8, 0.45
            fx += dx
            fy += dy
        top = (int(round(fx)), int(round(fy)))
        if kind == 0 and sh > h * 0.75 and not dry:  # a cattail on the tallest
            for j in range(2, 2 + int(h * 0.13)):
                hd_put(c, top[0], top[1] + j, 20 + (5 if j < 4 else 3), False)
                hd_put(c, top[0] + 1, top[1] + j, 20 + 2, False)
            hd_put(c, top[0], top[1] + 1, 20 + 6, False)
        for _ in range(int(rng.integers(1, 3))):  # long leaves arching off the stalk
            y0 = h - 1 - int(sh * rng.uniform(0.15, 0.5))
            side = rng.choice((-1, 1))
            lx, ly, ldx = float(x) + (1 if side > 0 else 0), float(y0), side * rng.uniform(0.25, 0.5)
            for i in range(int(sh * rng.uniform(0.35, 0.6))):
                hd_put(c, int(round(lx)), int(round(ly)), base + max(1, min(7, 3 + i // 6 + int(lift))), False)
                lx += ldx
                ly -= max(0.15, 1.0 - i * 0.06)  # up, bending out flatter toward the tip (never back down: no hoops)
    return c


def darker(c, k=2):
    """The same painting k tones darker in every ramp: the back row of a bed, so a stand has depth front to back."""
    return np.vectorize(lambda t: None if t is None else (t // 10) * 10 + max(0, t % 10 - k), otypes=[object])(c)



def bush_hd(rng, w, h, look=None):
    """A round bush of single leaves in clumps on the ground (rich, like the reference's berry bush); `look`
    'fruit' or 'bloom' adds berries or blossom (tones + 10)."""
    ys, xs = np.mgrid[0:h, 0:w]
    edge = wnoise(rng, 4, max(w, h))[:h, :w]
    mask = np.zeros((h, w), bool)
    lobes = int(rng.integers(2, 4))  # two or three bulges with dents between: no round blob, each variant its own
    for k in range(lobes):
        cx = w * (0.5 + (k - (lobes - 1) / 2) * rng.uniform(0.26, 0.34))
        r = rng.uniform(0.28, 0.36) * w
        cy = h - r * rng.uniform(0.95, 1.35)
        mask |= np.hypot(xs - cx, (ys - cy) * 1.1) + (edge - 0.5) * 6 <= r
    mask[h - 1] = mask[h - 2]  # stands on the ground
    c = leaf_mass(rng, LEAF_SHAPES["oval"], (h, w), 9, (6, 10), 1.0, 3, False, mask)
    for y, x in zip(*np.nonzero(mask)):  # the rim against the air takes a mid tone: no painted outline
        if c[y, x] is not None and c[y, x] < 3 and any(
                not (0 <= y + dy < h and 0 <= x + dx < w) or c[y + dy, x + dx] is None
                for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1))):
            c[y, x] = 3
    for y, x in zip(*np.nonzero(mask)):  # clearly darker underneath and inside: the lower third loses up to two tones
        if c[y, x] is not None:
            c[y, x] = max(1, c[y, x] - int(round(2.2 * max(0.0, (y / h - 0.45) / 0.55) ** 1.3)))
    k = np.random.default_rng(int(rng.integers(1 << 30)))
    if look:
        for _ in range(w * h // 30):
            x, y = int(k.integers(1, w - 2)), int(k.integers(2, h - 2))
            if c[y, x] is None or c[y + 1, x + 1] is None:
                continue
            if look == "fruit":  # a berry: two by two, a glint top left, a dark under it
                c[y, x], c[y, x + 1], c[y + 1, x], c[y + 1, x + 1] = 17, 15, 14, 12
            else:
                for dx, dy in ((0, -1), (-1, 0), (1, 0), (0, 1)):
                    c[y + dy, x + dx] = 16 if dx + dy < 0 else 14
                c[y, x] = 11
    return c


# ---------------------------------------------------------------- mining (ADR 0093 rubble, ADR 0095 halo)


def pieces(rng, n, size=N):
    """Broken rock: n pieces over the face (wrapping), each texel's piece, and how deep it is inside it (the gap
    between pieces where that is under one texel)."""
    px, py = rng.uniform(0, size, n), rng.uniform(0, size, n) * rng.uniform(0.8, 1.0)
    ys, xs = np.mgrid[0:size, 0:size].astype(float)
    dx = np.abs(xs[..., None] - px)
    dy = np.abs(ys[..., None] - py)
    d = np.hypot(np.minimum(dx, size - dx), np.minimum(dy, size - dy) * 1.15)
    o = np.sort(d, axis=2)
    return np.argmin(d, axis=2), o[..., 1] - o[..., 0]


def rubble(rng, src, gap, pal):
    """A rock's rubble from its own painting: six to eight broken pieces, each a shifted cut of the rock (its
    grain and colours), a lit rim up and left, a shaded rim down and right, one piece in three a step darker,
    and dark gaps between them, so it reads loose at a glance next to the solid wall."""
    vals, counts = np.unique(src.astype(str), return_counts=True)
    names = sorted((v for v, n in zip(vals, counts) if n >= src.size * 0.06), key=lambda k: luma(pal.rgb(k)))  # the rock's
    # own ramp; rare colours (ore specks, crystals) stay themselves, so a rim never turns gold or green
    step = {k: names[max(0, i - 1)] for i, k in enumerate(names)}
    brighter = {k: names[min(len(names) - 1, i + 1)] for i, k in enumerate(names)}
    if sum(luma(pal.rgb(k)) for k in names) / len(names) < 51:  # a dark rock (coal, basalt): its rims catch the
        sheen = max(vals, key=lambda k: luma(pal.rgb(k)))  # light in its lightest colour, or the pieces vanish
        brighter = {k: sheen for k in names}
    piece, depth = pieces(rng, int(rng.integers(6, 9)))
    shift = {k: (int(rng.integers(N)), int(rng.integers(N))) for k in np.unique(piece)}
    darkp = {k for k in shift if rng.random() < 0.33}
    c = np.empty((N, N), object)
    for y in range(N):
        for x in range(N):
            k = piece[y, x]
            if depth[y, x] < 1.0:  # the gap between pieces
                c[y, x] = gap
                continue
            ox, oy = shift[k]
            v = src[(y + oy) % N, (x + ox) % N]
            if k in darkp:
                v = step.get(v, v)
            up, left = piece[(y - 1) % N, x] != k or depth[(y - 1) % N, x] < 1.0, piece[y, (x - 1) % N] != k or depth[y, (x - 1) % N] < 1.0
            down = piece[(y + 1) % N, x] != k or depth[(y + 1) % N, x] < 1.0
            if up or left:
                v = brighter.get(v, v)
            elif down and v in step:
                v = step[step[v]]
            c[y, x] = v
    return c


# ---------------------------------------------------------------- the blocks

# Names the other flora sessions choose (their lists in the main checkout's out/, flora.md): where a list is
# missing, `<species> leaves`, `<species> log` and the plant's own name. A painting whose block does not exist yet,
# or whose look has no season file (`@fruit`), goes to out/flora/pending/ instead of data/textures/.
PENDING = ROOT / "out" / "flora" / "pending"
TAGS = {"spring", "autumn", "winter"}
# late summer's `@fruit` (ADR 0090) once the engine's season list names it; until then fruit looks wait
_seasons = next((l for l in (ROOT / "crates/engine/src/blocks.rs").read_text(encoding="utf-8").splitlines()
                 if "SEASONS" in l and "const" in l), "")
if '"fruit"' in _seasons or '"fruit"' in (ROOT / "crates/engine/src/textures.rs").read_text(encoding="utf-8"):
    TAGS.add("fruit")


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
                tex(f"{name}{at}", 4, lambda g, v, n=name: bare(np.random.default_rng(seed_of(n, v)), R("log"), WINTER_ROLES[v]))
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
    open_crowns = ("palm", "willow")  # fronds and curtains: open, but continuous shapes reaching the block's edge
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
            tex(f"{leaf_b}@winter", 4, lambda g, v, b=leaf_b, br=t.get("twigs", bark_r): bare(np.random.default_rng(seed_of(b, v)), br, WINTER_ROLES[v]))
        if "fruit" in t:  # late summer: no season file holds it yet (flora.md part A), so it waits in out/
            tex(f"{leaf_b}@fruit", 4, lambda g, v, r=leaf_r, leaf=leaf, f=t["fruit"]: f(leaf(r, v), g))
        tex(f"{log_b}_side", 3, lambda g, v, br=bark_r, fn=t["bark"], b=log_b: fn(g, br, seed_of(b, "bark")))
        tex(log_b, 2, lambda g, v, w=wood_r, br=bark_r, h=t.get("hollow", False): end_rings(g, w, br, h))

    for stem, cs in out.items():  # a cube of leaves stays mostly closed: the crowns' planes give the ragged outline
        if stem.split("@")[0].endswith("leaves") and "@winter" not in stem and stem.split()[0] not in open_crowns:
            for c in cs:
                assert sum(v is None for v in c.flat) <= 0.15 * N * N, f"{stem}: over 15 % open"

    # flora at 32 px a metre in rich colour (the developer, 2026-11-13): rich ramps of eight tones, named
    # `<base> r1`..`r8`, added to the palette when a written file uses them
    def RR(base, hue, sat=1.0, light=0.0, rgb=None):
        names = []
        for i, c in enumerate(rgb or rich_rgb(hue, sat, light)):
            nm = f"{base} r{i + 1}"
            hx = "#%02x%02x%02x" % tuple(to8(np.asarray(c, float)))
            if pal.rgb(nm) and pal.rgb(nm)[:5] != hx[:5]:  # a rich ramp changed here: its colours follow (blue may be nudged)
                pal.rows = [r for r in pal.rows if r["name"] != nm]
            if not pal.rgb(nm):
                pal.add(nm, to8(np.asarray(c, float)), f"{base} rich")
            names.append(nm)
        return names

    def named_canvas(c, ramps):
        """Tone indices to palette names: 0..7 the first ramp, 10..17 the second; ('w'|'b', t) wood or bark."""
        o = np.empty(c.shape, object)
        for idx, v in np.ndenumerate(c):
            if v is None:
                continue
            if isinstance(v, tuple):
                o[idx] = ramps[v[0]][v[1]]
            else:
                o[idx] = ramps[v // 10][v % 10]
        return o

    def hd(stem, n, fn):
        out[f"hd:{stem}"] = [fn(np.random.default_rng(seed_of("hd", stem, v)), v) for v in range(n)]

    BIRCH = [(0.10, 0.09, 0.08), (0.22, 0.20, 0.18), (0.42, 0.40, 0.37), (0.64, 0.62, 0.58), (0.78, 0.76, 0.71),
             (0.86, 0.84, 0.79), (0.92, 0.90, 0.85), (0.97, 0.96, 0.92)]
    sp_hd = {  # leaf painter, leaf hue/sat/light, bark kind, bark hue/sat/light, wood hue/sat/light, seasons
        "broadleaf": dict(leaf=lambda g: leaf_mass(g, LEAF_SHAPES["oval"], holes=0.06), hue=(100, 0.9, 0.0),
                          bark="ridged", barkc=(28, 0.35, 0.0), woodc=(34, 0.55, 0.12), spring=(92, 0.9, 0.03),
                          autumn=(30, 0.95, 0.02), blossom=True, fruit=("apple", 2, 0.95, 0.04, 3)),
        "oak": dict(leaf=lambda g: leaf_mass(g, LEAF_SHAPES["lobed"], radius=(7, 11), spacing=4, holes=0.08, nholes=1),
                    hue=(112, 0.85, -0.02), bark="ridged", barkc=(32, 0.18, 0.02), woodc=(32, 0.5, 0.1),
                    spring=(102, 0.85, 0.02), autumn=(26, 0.7, 0.0), fruit=("acorn", 36, 0.7, 0.06, 2)),
        "birch": dict(leaf=lambda g: leaf_mass(g, LEAF_SHAPES["small"], clumps=9, radius=(7, 10), spacing=2,
                                               holes=0.1, nholes=4), hue=(84, 0.9, 0.03), bark="birch",
                      barkrgb=BIRCH, woodc=(42, 0.4, 0.2), spring=(78, 0.9, 0.05), autumn=(48, 0.95, 0.05),
                      catkins=True, twigs="log"),
        "willow": dict(leaf=willow_hd, hue=(84, 0.78, 0.05), bark="diamond", barkc=(35, 0.15, 0.0),
                       woodc=(40, 0.45, 0.14), spring=(82, 0.6, 0.06), autumn=(58, 0.8, 0.04), catkins=True),
        "spruce": dict(leaf=spruce_hd, hue=(150, 0.6, -0.03), bark="small plates", barkc=(20, 0.35, -0.03),
                       woodc=(38, 0.5, 0.16)),
        "pine": dict(leaf=pine_hd, hue=(135, 0.6, 0.0), bark="plates", barkc=(18, 0.5, 0.02), woodc=(36, 0.55, 0.15)),
        "jungle": dict(leaf=lambda g: leaf_mass(g, LEAF_SHAPES["broad"], clumps=9, radius=(8, 12), spacing=5,
                                                holes=0.04, nholes=3), hue=(118, 1.0, 0.0), bark="smooth",
                       barkc=(40, 0.2, 0.08), woodc=(30, 0.5, 0.12), fruit=("tropical fruit", 28, 1.0, 0.08, 3)),
        "acacia": dict(leaf=lambda g: leaf_mass(g, LEAF_SHAPES["small"], clumps=7, radius=(7, 11), flat=1.6,
                                                spacing=3, holes=0.1, nholes=3), hue=(74, 0.72, 0.06), bark="smooth",
                       barkc=(30, 0.18, 0.0), woodc=(22, 0.6, 0.1)),
        "palm": dict(leaf=palm_hd, hue=(90, 0.9, 0.02), bark="palm", barkc=(38, 0.35, 0.05), woodc=(40, 0.45, 0.15),
                     fruit=("coconut", 28, 0.5, -0.02, 5)),
        "bamboo": dict(leaf=lambda g: leaf_mass(g, LEAF_SHAPES["lance"], clumps=8, radius=(8, 11), spacing=4,
                                                holes=0.08, nholes=3), hue=(92, 0.95, 0.02), bark="culm",
                       barkc=(85, 0.6, 0.05), woodc=(55, 0.35, 0.2), hollow=True),
    }
    blossom_r = RR("blossom", 340, 0.75, 0.3)
    catkin_r = RR("catkin", 58, 0.8, 0.12)

    def blooms(c, g, ramp_kind):
        """Spring over a crown: five-petal flowers (blossom) or hanging tassels (catkins), on leaves only."""
        for _ in range(14 if ramp_kind == "blossom" else 12):
            x, y = int(g.integers(HD)), int(g.integers(HD))
            if c[y, x] is None:
                continue
            if ramp_kind == "blossom":
                for dx, dy in ((0, -1), (-1, 0), (1, 0), (0, 1), (1, -1)):
                    hd_put(c, x + dx, y + dy, 17 if dx + dy < 0 else 15)
                hd_put(c, x, y, 13)
            else:
                for i in range(5):
                    hd_put(c, x, y + i, 10 + (7 if i == 0 else (5 if i < 3 else 3)))
                    if i % 2:
                        hd_put(c, x + 1, y + i, 14)
        return c

    def fruits(c, g, size):
        for _ in range(9 if size < 4 else 4):
            x, y = int(g.integers(HD)), int(g.integers(HD))
            for dy in range(size):
                for dx in range(size):
                    q = (dx - (size - 1) / 2) ** 2 + (dy - (size - 1) / 2) ** 2
                    if q <= (size / 2) ** 2 + 0.3:
                        hd_put(c, x + dx, y + dy, 10 + (7 if dx + dy == 0 else (5 if dx + dy < size else 2)))
        return c

    for sp, t in sp_hd.items():
        leaf_b = f"{sp} leaves"
        log_b = "log" if sp == "broadleaf" else (sp if sp in named else f"{sp} log")
        leaf_r = RR(leaf_b, *t["hue"])
        bark_r = RR(f"{sp} bark", 0, rgb=t["barkrgb"]) if "barkrgb" in t else RR(f"{sp} bark", *t["barkc"])
        wood_r = RR(f"{sp} wood", *t["woodc"])
        paint = t["leaf"]
        hd(leaf_b, 4, lambda g, v, r=leaf_r, b=leaf_b, f=paint: named_canvas(f(np.random.default_rng(seed_of(b, v))), [r]))
        if "spring" in t:
            sr = RR(f"{leaf_b} spring", *t["spring"])
            extra = "blossom" if t.get("blossom") else ("catkin" if t.get("catkins") else None)
            hd(f"{leaf_b}@spring", 4, lambda g, v, r=sr, b=leaf_b, f=paint, e=extra: named_canvas(
                blooms(f(np.random.default_rng(seed_of(b, v))), g, e) if e else f(np.random.default_rng(seed_of(b, v))),
                [r, blossom_r if e == "blossom" else catkin_r]))
        if "autumn" in t:
            ar = RR(f"{leaf_b} autumn", *t["autumn"])
            hd(f"{leaf_b}@autumn", 4, lambda g, v, r=ar, b=leaf_b, f=paint: named_canvas(
                f(np.random.default_rng(seed_of(b, v))), [r]))
            tw = RR("log bark", *sp_hd["broadleaf"]["barkc"]) if t.get("twigs") == "log" else bark_r
            hd(f"{leaf_b}@winter", 4, lambda g, v, r=tw, b=leaf_b: named_canvas(
                calm(hd_twigs(np.random.default_rng(seed_of(b, "winter", v)), WINTER_ROLES[v]), 1, 4), [r]))  # one tree, one bark family
        if "fruit" in t:
            name, hue, sat, light, size = t["fruit"]
            fr = RR(name, hue, sat, light)
            hd(f"{leaf_b}@fruit", 4, lambda g, v, r=leaf_r, b=leaf_b, f=paint, fr=fr, size=size: named_canvas(
                fruits(f(np.random.default_rng(seed_of(b, v))), g, size), [r, fr]))
        hd(f"{log_b}_side", 3, lambda g, v, r=bark_r, k=t["bark"]: named_canvas(bark_hd(g, k), [r]))
        hd(log_b, 2, lambda g, v, w=wood_r, r=bark_r, h=t.get("hollow", False): named_canvas(end_hd(g, h), {"w": w, "b": r}))
    shrub_r = RR("shrub", 102, 0.9, 0.0)  # the trees' green family: a shrub in full sun read lime at 92
    hd("shrub", 4, lambda g, v: named_canvas(leaf_mass(np.random.default_rng(seed_of("shrub", v)), LEAF_SHAPES["oval"],
                                                       radius=(5, 8), holes=0.08), [shrub_r]))
    hd("shrub@winter", 4, lambda g, v: named_canvas(
        calm(hd_twigs(np.random.default_rng(seed_of("shrub", "winter", v)), WINTER_ROLES[v]), 1, 4), [RR("log bark", 28, 0.35, 0.0)]))
    for season, ramp in (("spring", RR("shrub spring", 94, 0.9, 0.03)), ("autumn", RR("shrub autumn", 30, 0.85, 0.0))):
        hd(f"shrub@{season}", 4, lambda g, v, r=ramp: named_canvas(leaf_mass(np.random.default_rng(seed_of("shrub", v)),
                                                                             LEAF_SHAPES["oval"], radius=(5, 8), holes=0.08), [r]))
    # the reed block: a tuft of blades a block high, its own outline per variant; seasons recolour the same blades
    for season, ramp in (("", RR("meadow green", 0, rgb=turned(MEADOW, 0))), ("@spring", RR("reed spring", 96, 0.85, 0.05)),
                         ("@autumn", RR("straw", 46, 0.75, 0.08)), ("@winter", RR("reed winter", 40, 0.3, 0.1))):
        # the four variants are the four reed kinds; winter three (cattails, leaning, broken), as ADR 0090 picks
        # winter variants by role only in a painting of exactly four
        kinds = (0, 2, 3) if season == "@winter" else (0, 1, 2, 3)
        hd(f"reed{season}", len(kinds), lambda g, v, r=ramp, kinds=kinds: named_canvas(
            reeds_hd(np.random.default_rng(seed_of("reed", kinds[v])), HD, HD, kinds[v]),
            [r, RR("straw", 46, 0.75, 0.08), RR("cattail", 22, 0.5, -0.02)]))
    cactus_r, spine_r = RR("cactus", 128, 0.55, -0.02), RR("cactus spine", 50, 0.3, 0.25)
    hd("cactus_side", 3, lambda g, v: named_canvas(cactus_hd(g, False), [cactus_r, spine_r]))
    hd("cactus", 2, lambda g, v: named_canvas(cactus_hd(g, True), [cactus_r, spine_r]))

    # plants at 32 px a metre: two variants each besides the first (`.1`, `.2`), another shape and a hue a step off
    # short grass 0.75 m and full, its fourth outline (`.3`) the young tuft of 0.4 m, one in four (bug
    # 2026-11-13_14-24-52, ADR 0099)
    plant_sizes = {"short grass": (32, 24), "short grass.3": (32, 13), "tall grass": (32, 32), "wild wheat": (32, 40), "fern": (48, 22),
                   "reeds": (32, 64), "flowers": (32, 16), "berry bush": (40, 40)}
    hues = (0, -7, 6)
    plant_sizes["meadow ground"] = (96, 32)

    def plant(name, fn):
        w, h = plant_sizes.get(name) or plant_sizes[name.split(".")[0]]
        out[f"plant:{name}"] = [fn(np.random.default_rng(seed_of("plant", name)), w, h)]
        if "." not in name and name not in ("meadow ground", "flowers"):  # the far painting, per kind of mass:
            # 8 px a metre; single flowers have none (the developer: masses only)
            out[f"plant:{name}.far"] = [far(out[f"plant:{name}"][0])]

    straw = [RR("straw", 46 + d, 0.75, 0.08) for d in hues]
    cattail = RR("cattail", 22, 0.5, -0.02)
    plant_sizes["reeds"] = (32, 64)
    # the fourth reed kind (sparse, one broken stalk), and the back row: the tall kind without heads, two tones darker
    plant("reeds.3", lambda g, w, h: named_canvas(reeds_hd(g, w, h, 3), [RR("meadow green 1", 0, rgb=turned(MEADOW, -7)),
                                                                         straw[0], cattail]))
    plant("reeds.back", lambda g, w, h: named_canvas(darker(reeds_hd(np.random.default_rng(seed_of("plant", "reeds.1")), w, h, 1)),
                                                     [RR("meadow green", 0, rgb=turned(MEADOW, 0)), straw[0], cattail]))
    for k, d in enumerate(hues):
        sfx = f".{k}" if k else ""
        G = RR("meadow green" + (f" {k}" if k else ""), 0, rgb=turned(MEADOW, d))
        plant(f"short grass{sfx}", lambda g, w, h, G=G: named_canvas(blades_hd(g, w, h, w * 4, 3, h, 0.45, 0.15), [G]))
        plant(f"tall grass{sfx}", lambda g, w, h, G=G: named_canvas(blades_hd(g, w, h, w * 3, h // 3, h, 0.45), [G]))
        plant(f"wild wheat{sfx}", lambda g, w, h, G=G, S=straw[k]: named_canvas(ears_hd(g, w, h), [G, S]))
        plant(f"fern{sfx}", lambda g, w, h, F=RR("fern" + (f" {k}" if k else ""), 104 + d, 0.9, -0.04):
              named_canvas(fern_hd(g, w, h), [F]))
        plant(f"reeds{sfx}", lambda g, w, h, G=G, k=k: named_canvas(reeds_hd(g, w, h, k), [G, straw[0], cattail]))
    for name, (lo, lean) in (("short grass", (3, 0.45)), ("tall grass", (32 // 3, 0.45))):  # a fourth outline where stands repeat
        plant(f"{name}.3", lambda g, w, h, lo=lo, lean=lean: named_canvas(blades_hd(g, w, h, w * 3, lo, h, lean, 0.15),
                                                                      [RR("meadow green 2", 0, rgb=turned(MEADOW, 6))]))
    def meadow_ground(g, w, h, G, P, S):
        """The grass top where plants thin out (three tiles side by side, each tiling): mottled clumps of the
        meadow's greens, blade tips as lit specks, a few petals and straw (tones + 10, + 20)."""
        tiles = []
        for _ in range(3):
            n = wnoise(g, 6) * 0.6 + wnoise(g, 3) * 0.4
            c = np.clip(np.round(1.5 + n * 4.5 + g.normal(0, 0.5, (HD, HD))), 1, 6).astype(int).astype(object)
            for _ in range(40):  # blade tips
                x, y = int(g.integers(HD)), int(g.integers(HD))
                c[y, x], c[(y + 1) % HD, x] = 6, 4
            for _ in range(4):  # flowers and seed heads
                x, y = int(g.integers(HD)), int(g.integers(HD))
                c[y, x] = (10 + 5) if g.random() < 0.6 else (20 + 5)
                c[y, (x + 1) % HD] = (10 + 3) if g.random() < 0.6 else (20 + 3)
            tiles.append(c)
        return named_canvas(np.concatenate(tiles, axis=1), [G, P, S])

    petals = [RR("red petal", 2, 0.85, 0.08), RR("yellow petal", 50, 0.95, 0.15), RR("blue petal", 240, 0.6, 0.12)]
    for k, pr in enumerate(petals):
        G = RR("meadow green" + (f" {k}" if k else ""), 0, rgb=turned(MEADOW, hues[k]))
        plant(f"flowers{f'.{k}' if k else ''}", lambda g, w, h, G=G, pr=pr: named_canvas(flowers_hd(g, w, h), [G, pr]))
    plant("meadow ground", lambda g, w, h: meadow_ground(g, w, h, RR("meadow green", 0, rgb=turned(MEADOW, 0)),
                                                          petals[1], straw[0]))
    bush_r = RR("berry bush", 104, 0.85, -0.06)
    plant("berry bush", lambda g, w, h: named_canvas(bush_hd(np.random.default_rng(seed_of("berry bush")), w, h), [bush_r]))
    plant("berry bush.1", lambda g, w, h: named_canvas(bush_hd(g, w, h), [RR("berry bush 1", 98, 0.85, -0.05)]))
    plant("berry bush.2", lambda g, w, h: named_canvas(bush_hd(g, w, h), [RR("berry bush 2", 110, 0.85, -0.07)]))
    plant("berry bush.bloom", lambda g, w, h: named_canvas(
        bush_hd(np.random.default_rng(seed_of("berry bush")), w, h, "bloom"), [bush_r, blossom_r]))
    plant("berry bush.fruit", lambda g, w, h: named_canvas(
        bush_hd(np.random.default_rng(seed_of("berry bush")), w, h, "fruit"), [bush_r, RR("berry", 0, rgb=[tuple(v / 255 for v in q) for q in [(70, 8, 14), (110, 14, 20), (150, 22, 24), (185, 30, 30), (215, 48, 40),
                                                     (235, 80, 60), (245, 120, 95), (255, 185, 160)]])]))


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
    pd = R("peridotite")  # the mantle's rock (ADR 0152): gabbro's grain, darker and greener
    rock("peridotite", lambda g, v: granite(g, v, pd, 5, 7))

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

    # the ore halo (ADR 0095): the host rock altered round an ore body, each its own look, read at a glance
    host = R("halo grey", (0.50, 0.49, 0.47))
    rust, ochre, pits = R("limonite", (0.62, 0.32, 0.12)), R("ochre", (0.80, 0.58, 0.26)), R("weathered pyrite", (0.30, 0.20, 0.13))

    def iron_stained(g, v):
        """Grey rock blotched and streaked rust to ochre, dark brown pits where pyrite weathered out."""
        c = grain(g, host, "sml", (0.3, 0.45, 0.25), 4, 0.35)
        f = field(g, 4, 0.35)  # blotches, not bands
        c[f > 0.5] = ochre["m"]
        c[f > 0.62] = rust["m"]
        c[f > 0.78] = rust["d"]
        for _ in range(2):  # stains run down from the blotches
            x, y = int(g.integers(N)), int(g.integers(N))
            walk(c, g, rust["s"], x, y, int(g.integers(4, 8)), 0, 1, 0.2)
        specks(c, g, pits["m"], 3)
        return c
    tex("iron-stained rock", 3, iron_stained)

    mal, azu = R("malachite smear", (0.20, 0.62, 0.38)), R("azurite", (0.20, 0.38, 0.72))

    def copper_stained(g, v):
        """Grey rock cracked, green malachite smeared along the cracks, a few small blue azurite patches."""
        c = grain(g, host, "sml", (0.25, 0.5, 0.25), 4, 0.35)
        for _ in range(3):
            vert = g.random() < 0.5
            x, y = int(g.integers(N)), int(g.integers(N))
            for _ in range(int(g.integers(6, 11))):
                put(c, x, y, host["d"])
                for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
                    if g.random() < 0.45:
                        put(c, x + dx, y + dy, mal["m"] if g.random() < 0.7 else mal["l"])
                if g.random() < 0.3:
                    x, y = (x + g.choice((-1, 1)), y) if vert else (x, y + g.choice((-1, 1)))
                x, y = (x, y + 1) if vert else (x + 1, y)
        for _ in range(2):
            specks(c, g, azu["m"], 1, 2)
        return c
    tex("copper-stained rock", 3, copper_stained)

    qhost, quartz = R("veined host", (0.36, 0.36, 0.37)), R("milky quartz", (0.93, 0.92, 0.88))

    def quartz_veined(g, v):
        """Dark grey host crossed by thin milky quartz veinlets, one and two texels, a crisscross stockwork."""
        c = grain(g, qhost, "dsm", (0.2, 0.5, 0.3), 4, 0.3)
        for k in range(int(g.integers(4, 6))):
            x, y = int(g.integers(N)), int(g.integers(N))
            sx, sy = ((1, int(g.choice((-1, 0, 1)))) if k % 2 else (int(g.choice((-1, 0, 1))), 1))
            wide = g.random() < 0.4
            for _ in range(int(g.integers(6, 13))):
                put(c, x, y, quartz["m"] if g.random() < 0.65 else quartz["s"])
                if wide:
                    put(c, x + (0 if sx else 1), y + (1 if sx else 0), quartz["s"])
                if g.random() < 0.25:  # the vein jogs
                    x, y = x + (0 if sx else int(g.choice((-1, 1)))), y + (int(g.choice((-1, 1))) if sx else 0)
                x, y = x + sx, y + sy
        return c
    tex("quartz-veined rock", 3, quartz_veined)

    gre, glint, cass = R("greisen"), R("mica glint", (0.97, 0.97, 0.92)), R("cassiterite speck", (0.18, 0.17, 0.18))

    def greisen(g, v):
        """Pale grey-green, coarse, glittering with quartz and light mica, small dark specks."""
        c = grain(g, gre, "sml", (0.3, 0.4, 0.3), 2, 0.6)
        specks(c, g, glint["m"], 9)
        specks(c, g, cass["m"], 5)
        return c
    tex("greisen", 3, greisen)

    bl = R("bleached rock")

    def bleached(g, v):
        """The host gone pale: cream to off-white, chalky, a faint ghost of the grain."""
        c = grain(g, bl, "smL", (0.15, 0.6, 0.25), 4, 0.25)
        specks(c, g, bl["s"], 6, 2)
        return c
    tex("bleached rock", 3, bleached)

    do, spar = R("dolomite"), R("calcite spar", (0.95, 0.94, 0.90))

    def dolomite(g, v):
        """Buff to tan sugary carbonate, white calcite spar in small vugs (a dark under-edge)."""
        c = grain(g, do, "sml", (0.25, 0.5, 0.25), 2, 0.7)
        for _ in range(2):
            x, y = int(g.integers(N)), int(g.integers(N))
            for dx, dy in ((0, 0), (1, 0), (0, 1)):
                put(c, x + dx, y + dy, spar["m"])
            put(c, x, y + 2, do["d"])
            put(c, x + 1, y + 1, do["d"])
        return c
    tex("dolomite", 3, dolomite)

    # rubble (ADR 0093): each rock's and hard ore's broken pieces, from its own painting
    gap = R("rubble gap", (0.12, 0.11, 0.10))["m"]
    for b in blocks:
        if "rubble" in b and b["name"] in out:
            src = out[b["name"]]
            tex(f"{b['name']} rubble", 3, lambda g, v, src=src: rubble(g, src[v % len(src)], gap, pal))
    for stem in [k for k in out if k.startswith("hd:")]:
        block = stem[3:].split("@")[0].removesuffix("_side")
        if block.endswith("leaves") or block in ("shrub", "reed", "cactus"):
            out["hdfar:" + stem[3:]] = [far(v) for v in out[stem]]  # 8 px a face: the outline and main colours
    return out




def write(pal, stem, variants, cutout, folder=OUT):
    N = variants[0].shape[0]  # noqa: N806 (16 for blocks, 32 for flora's hd paintings)
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
    """A plant's plane, 16 px a metre; open texels alpha 0."""
    h, w = c.shape
    img = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    px = img.load()
    for y in range(h):
        for x in range(w):
            if c[y, x] is None:
                continue
            hx = pal.rgb(c[y, x])
            assert hx, f"{path.name}: '{c[y, x]}' is not in the palette"
            px[x, y] = tuple(int(hx[i:i + 2], 16) for i in (1, 3, 5)) + (255,)
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
    names = {b["name"] for b in blocks} | {f"{b['name']} rubble" for b in blocks if "rubble" in b}
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
        if stem.startswith(("hd:", "hdfar:")):  # flora at 32: data/textures/hd/ (ADR 0090), its far ones in hd/far/
            sub = "hd/far" if stem.startswith("hdfar:") else "hd"
            stem = stem.split(":", 1)[1]
            block = stem.split("@")[0].removesuffix("_top").removesuffix("_side").removesuffix("_bottom")
            if want and block not in want:
                continue
            tag = stem.split("@")[1] if "@" in stem else None
            ok = block in names and not (tag and tag not in TAGS)
            folder = OUT / sub if ok else PENDING / sub
            folder.mkdir(parents=True, exist_ok=True)
            p, k = write(pal, stem, variants, True, folder)
            (paths if ok else pending).append(p)
            used |= k if ok else set()
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
    early = set(pal.added) - used - pal.before
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
