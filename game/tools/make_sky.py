"""Bake the night sky textures from data/sky (ADR 0025).

  assets/textures/sky_stars.png      4096x2048 RGBA, equirectangular in equatorial
                                     coordinates: u = right ascension / 360 deg,
                                     v = (90 deg - declination) / 180 deg.
                                     RGB = stars (colour from B-V, brightness from
                                     magnitude, sqrt-encoded), A = constellation lines.
  assets/textures/sky_milkyway.png   1024x512 L, same mapping: the Milky Way's glow
                                     from galactic latitude/longitude, a bulge
                                     towards Sagittarius and the Great Rift's dust.

The sky shader turns a view direction into (RA, Dec) with the sidereal basis
from Calendar.celestial_to_world() and samples these.
Run: python game/tools/make_sky.py
"""
import json
import math
import os
import random

from PIL import Image, ImageDraw, ImageFilter

HERE = os.path.dirname(__file__)
DATA = os.path.join(HERE, "..", "data", "sky")
OUT = os.path.join(HERE, "..", "assets", "textures")
W, H = 4096, 2048
MW_W, MW_H = 1024, 512


def bv_to_rgb(bv):
    """B-V colour index -> sRGB-ish star colour (blue-white O/B to orange-red M)."""
    stops = [(-0.4, (155, 176, 255)), (0.0, (202, 216, 255)), (0.3, (248, 247, 255)),
             (0.6, (255, 244, 234)), (0.9, (255, 225, 190)), (1.3, (255, 204, 140)), (2.0, (255, 170, 100))]
    bv = max(stops[0][0], min(stops[-1][0], bv))
    for (a, ca), (b, cb) in zip(stops, stops[1:]):
        if bv <= b:
            t = (bv - a) / (b - a)
            return tuple(ca[i] + (cb[i] - ca[i]) * t for i in range(3))
    return stops[-1][1]


def to_px(ra, dec, w, h):
    return ra / 360.0 * w, (90.0 - dec) / 180.0 * h


def bake_stars():
    stars = json.load(open(os.path.join(DATA, "stars.json"), encoding="utf-8"))["stars"]
    acc = [[0.0, 0.0, 0.0] for _ in range(W * H)]
    for ra, dec, mag, bv in stars:
        # brightness: flux relative to magnitude 6.5, compressed so dim stars still register
        v = max(0.0, min(1.0, ((6.6 - mag) / 8.0))) ** 1.5
        sigma = 0.55 + 0.32 * max(0.0, 3.5 - mag)          # bright stars spread over more pixels
        col = bv_to_rgb(bv)
        x0, y0 = to_px(ra, dec, W, H)
        stretch = 1.0 / max(0.08, math.cos(math.radians(dec)))   # equirect: wider towards the poles
        rx = int(math.ceil(3 * sigma * stretch)) + 1
        ry = int(math.ceil(3 * sigma)) + 1
        for dy in range(-ry, ry + 1):
            y = int(y0) + dy
            if y < 0 or y >= H:
                continue
            for dx in range(-rx, rx + 1):
                x = int(x0) + dx
                fx = (x + 0.5 - x0) / stretch
                fy = y + 0.5 - y0
                g = math.exp(-(fx * fx + fy * fy) / (2 * sigma * sigma))
                if g < 0.01:
                    continue
                k = ((x % W) + y * W)
                a = acc[k]
                for i in range(3):
                    a[i] += v * g * col[i] / 255.0
    img = Image.new("RGBA", (W, H))
    px = img.load()
    for y in range(H):
        row = y * W
        for x in range(W):
            a = acc[row + x]
            if a[0] or a[1] or a[2]:
                # sqrt encoding: the shader squares it back
                px[x, y] = (min(255, int(math.sqrt(min(1.0, a[0])) * 255)), min(255, int(math.sqrt(min(1.0, a[1])) * 255)),
                            min(255, int(math.sqrt(min(1.0, a[2])) * 255)), 0)
    # constellation lines in alpha, drawn as short segments along great circles
    lines = Image.new("L", (W, H), 0)
    d = ImageDraw.Draw(lines)
    cons = json.load(open(os.path.join(DATA, "constellations.json"), encoding="utf-8"))["constellations"]
    for c in cons:
        for seg in c["lines"]:
            for (ra1, de1), (ra2, de2) in zip(seg, seg[1:]):
                p1 = _vec(ra1, de1)
                p2 = _vec(ra2, de2)
                n = 24
                pts = []
                for i in range(n + 1):
                    t = i / n
                    v = [p1[j] * (1 - t) + p2[j] * t for j in range(3)]
                    ln = math.sqrt(sum(q * q for q in v))
                    ra = math.degrees(math.atan2(v[1], v[0])) % 360.0
                    de = math.degrees(math.asin(v[2] / ln))
                    pts.append(to_px(ra, de, W, H))
                for a, b in zip(pts, pts[1:]):
                    if abs(a[0] - b[0]) > W / 2:
                        continue   # crosses RA 0: the neighbouring segments cover it
                    d.line([a, b], fill=150, width=2)
    lines = lines.filter(ImageFilter.GaussianBlur(0.6))
    img.putalpha(lines)
    img.save(os.path.join(OUT, "sky_stars.png"), optimize=True)
    print("stars", len(stars), "constellations", len(cons))


def _vec(ra, dec):
    r, d = math.radians(ra), math.radians(dec)
    return (math.cos(d) * math.cos(r), math.cos(d) * math.sin(r), math.sin(d))


# J2000 equatorial -> galactic rotation (rows)
GAL = ((-0.0548755604, -0.8734370902, -0.4838350155),
       (0.4941094279, -0.4448296300, 0.7469822445),
       (-0.8676661490, -0.1980763734, 0.4559837762))


def _noise2(x, y, seed):
    """Value noise on a lattice, smooth; enough for dust lanes."""
    def h(i, j):
        n = (i * 374761393 + j * 668265263 + seed * 2246822519) & 0xFFFFFFFF
        n = (n ^ (n >> 13)) * 1274126177 & 0xFFFFFFFF
        return ((n ^ (n >> 16)) & 0xFFFF) / 65535.0
    xi, yi = math.floor(x), math.floor(y)
    fx, fy = x - xi, y - yi
    fx, fy = fx * fx * (3 - 2 * fx), fy * fy * (3 - 2 * fy)
    a, b, c, e = h(xi, yi), h(xi + 1, yi), h(xi, yi + 1), h(xi + 1, yi + 1)
    return (a + (b - a) * fx) * (1 - fy) + (c + (e - c) * fx) * fy


def bake_milkyway():
    img = Image.new("L", (MW_W, MW_H))
    px = img.load()
    for y in range(MW_H):
        dec = 90.0 - (y + 0.5) / MW_H * 180.0
        for x in range(MW_W):
            ra = (x + 0.5) / MW_W * 360.0
            v = _vec(ra, dec)
            g = [sum(GAL[r][k] * v[k] for k in range(3)) for r in range(3)]
            b = math.degrees(math.asin(max(-1.0, min(1.0, g[2]))))
            l = math.radians(math.degrees(math.atan2(g[1], g[0])))
            toward_centre = 0.5 + 0.5 * math.cos(l)
            width = 6.0 + 7.0 * toward_centre ** 2
            band = math.exp(-(b / width) ** 2) * (0.35 + 0.65 * toward_centre ** 1.5)
            bulge = math.exp(-((math.degrees(l) % 360 + 180) % 360 - 180) ** 2 / (2 * 14.0 ** 2)) * math.exp(-(b / 9.0) ** 2) * 0.5
            # clumps and the dark rift along the plane (strongest from Cygnus to Sagittarius)
            n = 0.0
            amp, f = 0.5, 1.0
            for o in range(4):
                n += amp * _noise2(math.degrees(l) / 12.0 * f, b / 6.0 * f, 7 + o)
                amp *= 0.5
                f *= 2.03
            rift = math.exp(-((b + 1.0) / 2.2) ** 2) * (0.55 if -90 < math.degrees(l) % 360 - 360 * (math.degrees(l) % 360 > 180) < 60 else 0.2)
            val = (band + bulge) * (0.65 + 0.7 * n) * (1.0 - rift * (0.6 + 0.4 * n))
            px[x, y] = max(0, min(255, int(val * 255)))
    img = img.filter(ImageFilter.GaussianBlur(1.0))
    img.save(os.path.join(OUT, "sky_milkyway.png"), optimize=True)
    print("milky way baked")


if __name__ == "__main__":
    random.seed(1)
    bake_milkyway()
    bake_stars()
