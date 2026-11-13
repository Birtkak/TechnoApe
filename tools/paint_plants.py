#!/usr/bin/env python3
"""Make the placeholder paintings of the small plants (ADR 0091): data/plants/<name>.png, one upright plane each,
at 32 pixels a metre (the plant is as wide and high as its painting; a column's painting is 32 px wide so
neighbours merge), with variants `.1` and `.2` the game picks by the place's hash,, from data/palette.toml's colours only, every
pixel whole or empty. The game crosses two to four of these planes in the cell (`planes` in data/clutter.toml) and
tints their green pixels by the climate. Every file goes on data/drafts.txt; the textures session repaints them in
the calm style.

    python -I tools/paint_plants.py
"""
import hashlib
import tomllib
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
DATA = ROOT / "data"
OUT = DATA / "plants"
PAL = {c["name"]: c["rgb"] for c in tomllib.loads((DATA / "palette.toml").read_text(encoding="utf-8"))["colour"]}


def rgb(name):
    h = PAL[name]
    return (int(h[1:3], 16), int(h[3:5], 16), int(h[5:7], 16), 255)


def rnd(*k):
    return int.from_bytes(hashlib.blake2b(repr(k).encode(), digest_size=4).digest(), "little") / 2**32


def blades(img, n, seed, dark, mid, lit, lean=0.25, low=0.35, head=None, root="grass dark", tip=None):
    """n tapering blades from the bottom edge, dark at the root, lit at the tip; `head` paints a seed head on top."""
    w, h = img.size
    for b in range(n):
        x0 = (b + 0.5) * w / n + (rnd(seed, b) - 0.5) * w / n * 0.8
        top = h * (low + (1 - low) * rnd(seed, b, "h"))
        bend = (rnd(seed, b, "l") - 0.5) * 2 * lean
        for yy in range(int(top)):
            cx = x0 + bend * yy
            half = max(0.5, 1.1 * (1 - yy / top))
            for x in range(w):
                if abs(x + 0.5 - cx) <= half:
                    f = yy / top
                    c = root if f < 0.12 else dark if f < 0.35 else mid if f < 0.7 else lit if (f < 0.9 or not tip) else tip
                    img.putpixel((x, h - 1 - yy), rgb(c))
        if head:
            cx = x0 + bend * top
            for k in range(10):
                yy, dx = int(top) - k, (k % 2) * 2 - 1
                for x in (int(cx), int(cx + dx)):
                    if 0 <= x < w and 0 <= h - 1 - yy < h:
                        img.putpixel((x, h - 1 - yy), rgb(head[k % len(head)]))


def frond(img, dark, mid, lit):
    w, h = img.size
    for y in range(h):
        yy = h - 1 - y
        cx = w / 2
        for x in range(w):
            if abs(x + 0.5 - cx) < 0.6 and yy <= h - 2:
                img.putpixel((x, y), rgb(dark))
                continue
            reach = (w / 2 - 1) * (1 - yy / h) ** 0.6 * min(1.0, yy / 4)
            if yy > 1 and yy % 3 < 2 and abs(x + 0.5 - cx) <= reach:
                img.putpixel((x, y), rgb(lit if abs(x + 0.5 - cx) > reach - 1.5 else mid))


def flowers(img, seed, stem, leaf, head, eye):
    """five stems with leaves under round heads two pixels across the middle"""
    w, h = img.size
    for b in range(5):
        fx = 3 + b * (w - 6) / 4 + (rnd(seed, b, "x") - 0.5) * 3
        top = int(h * (0.45 + 0.4 * rnd(seed, b)))
        for yy in range(top):
            x = int(fx)
            img.putpixel((x, h - 1 - yy), rgb(stem))
            for ly, dx in ((top // 4, 1), (top // 2, -1)):
                if yy in (ly, ly + 1) and 0 <= x + dx < w:
                    img.putpixel((x + dx, h - 1 - yy), rgb(leaf))
        for dx in range(-2, 3):
            for dy in range(-2, 3):
                if dx * dx + dy * dy <= 4 + (rnd(seed, b, dx, dy) < 0.4):
                    x, y = int(fx) + dx, h - 1 - top + dy
                    if 0 <= x < w and 0 <= y < h:
                        img.putpixel((x, y), rgb(eye if abs(dx) + abs(dy) <= 1 and dx * dy == 0 and dx == 0 and dy == 0 else head))


def bush(img, seed, dark, mid, lit, dots=None, share=0.0):
    w, h = img.size
    for y in range(h):
        for x in range(w):
            dx, dy = (x + 0.5 - w / 2) / (w / 2), (h - y - 0.5) / h
            if dx * dx + (dy - 0.5) ** 2 / 0.28 > 0.95 or rnd(seed, x, y, "gap") < 0.12:
                continue
            r = rnd(seed, x, y)
            if dots and r < share and dy > 0.15:
                c = dots
            else:
                c = lit if dy > 0.6 and r < 0.5 else (mid if r < 0.65 else dark)
            img.putpixel((x, y), rgb(c))


def make():
    OUT.mkdir(exist_ok=True)
    out = []

    def save(name, img, what):
        img.save(OUT / f"{name}.png")
        out.append(f"data/plants/{name}.png")
        print(f"{name}: {img.size[0]} x {img.size[1]} px, {what}")

    g = ("grass shade", "grass", "grass lit")
    v = ["", ".1", ".2"]

    def new(w, h):
        return Image.new("RGBA", (w, h), (0, 0, 0, 0))

    for k, alt in enumerate(v):
        im = new(32, 13)
        blades(im, 9 + k, ("short", k), *g, tip="grass spring lit")
        save("short grass" + alt, im, "short blades")
        im = new(32, 32)
        blades(im, 10 + 2 * k, ("tall", k), *g, lean=0.18 + 0.03 * k, low=0.45, tip="meadow grass lit")
        save("tall grass" + alt, im, "tall blades bending")
        im = new(32, 40)
        blades(im, 8 + k, ("wheat", k), "grass shade", "meadow grass", "dry grass", lean=0.1, low=0.6,
               head=("dry grass light", "autumn gold", "dry grass"), tip="dry grass light")
        save("wild wheat" + alt, im, "stalks with seed heads")
        im = new(48, 22)
        frond(im, "jungle grass shade", "jungle grass", "jungle grass lit")
        save("fern" + alt, im, "a frond, three crossed")
        im = new(32, 64)
        blades(im, 8 + k, ("reeds", k), "reed shade", "reed", "reed lit", lean=0.05, low=0.65, head=("log dark", "log", "log shade"))
        save("reeds" + alt, im, "upright stalks with brown heads")
        head, eye = [("lamp", "autumn gold"), ("snow", "lamp"), ("blue lamp", "snow")][k]
        im = new(32, 16)
        flowers(im, ("flowers", k), "grass shade", "grass", head, eye)
        save("flowers" + alt, im, f"stems under {head} heads")
        im = new(40, 40)
        bush(im, ("bush", k), "grass shade", "grass", "grass lit")
        save("berry bush" + alt, im, "leaves")
    for look, dots, share in [(".bloom", "snow", 0.08), (".fruit", "coral", 0.1)]:
        im = new(40, 40)
        bush(im, ("bush", 0), "grass shade", "grass", "grass lit", dots, share)
        save("berry bush" + look, im, {".bloom": "white blossom (late spring)", ".fruit": "red berries (late summer)"}[look])
    return out


if __name__ == "__main__":
    files = make()
    drafts = DATA / "drafts.txt"
    have = drafts.read_text(encoding="utf-8").splitlines()
    drafts.write_text("\n".join(have + [f for f in files if f not in have]) + "\n", encoding="utf-8")
    print(f"{len(files)} files")
