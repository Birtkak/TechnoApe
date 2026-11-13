#!/usr/bin/env python3
"""Make the placeholder paintings of the small plants (ADR 0091): data/plants/<name>.png, one upright plane each,
at 32 pixels a metre (the plant is as wide and high as its painting), from data/palette.toml's colours only, every
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


def blades(img, n, seed, dark, mid, lit, lean=0.25, low=0.35, head=None):
    """n tapering blades from the bottom edge, dark at the root, lit at the tip; `head` paints a seed head on top."""
    w, h = img.size
    for b in range(n):
        x0 = (b + 0.5) * w / n + (rnd(seed, b) - 0.5) * w / n * 0.8
        top = h * (low + (1 - low) * rnd(seed, b, "h"))
        bend = (rnd(seed, b, "l") - 0.5) * 2 * lean
        for yy in range(int(top)):
            cx = x0 + bend * yy
            half = max(0.6, 1.3 * (1 - yy / top))
            for x in range(w):
                if abs(x + 0.5 - cx) <= half:
                    c = dark if yy < top * 0.3 else (lit if yy > top * 0.75 else mid)
                    img.putpixel((x, h - 1 - yy), rgb(c))
        if head:
            cx = x0 + bend * top
            for k in range(6):
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
            if yy > 1 and yy % 2 < 1 and abs(x + 0.5 - cx) <= reach:
                img.putpixel((x, y), rgb(lit if abs(x + 0.5 - cx) > reach - 1.5 else mid))


def flowers(img, seed, stem, leaf, head, eye):
    w, h = img.size
    for b in range(4):
        fx = 2 + b * (w - 4) / 3 + (rnd(seed, b, "x") - 0.5) * 2
        top = int(h * (0.5 + 0.45 * rnd(seed, b)))
        for yy in range(top):
            x = int(fx)
            img.putpixel((x, h - 1 - yy), rgb(stem))
            if yy in (top // 3, top // 2) and 0 <= x + (1 if yy == top // 3 else -1) < w:
                img.putpixel((x + (1 if yy == top // 3 else -1), h - 1 - yy), rgb(leaf))
        for dx in (-1, 0, 1):
            for dy in (-1, 0, 1):
                if abs(dx) + abs(dy) <= 1 + (rnd(seed, b, dx, dy) < 0.5):
                    x, y = int(fx) + dx, h - 1 - top + dy
                    if 0 <= x < w and 0 <= y < h:
                        img.putpixel((x, y), rgb(eye if dx == dy == 0 else head))


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

    g = ("grass dark", "grass", "grass light")
    im = Image.new("RGBA", (32, 14), (0, 0, 0, 0))
    blades(im, 9, "short", *g)
    save("short grass", im, "short blades")
    im = Image.new("RGBA", (24, 10), (0, 0, 0, 0))
    blades(im, 6, "short.1", *g)
    save("short grass.1", im, "a sparser tuft")
    im = Image.new("RGBA", (32, 32), (0, 0, 0, 0))
    blades(im, 10, "tall", *g, lean=0.18, low=0.5)
    save("tall grass", im, "tall blades bending")
    im = Image.new("RGBA", (32, 26), (0, 0, 0, 0))
    blades(im, 8, "tall.1", *g, lean=0.22, low=0.45)
    save("tall grass.1", im, "a lower clump")
    im = Image.new("RGBA", (32, 40), (0, 0, 0, 0))
    blades(im, 9, "wheat", "dry grass dark", "grass", "dry grass", lean=0.12, low=0.6, head=("dry grass light", "autumn gold"))
    save("wild wheat", im, "stalks with seed heads")
    im = Image.new("RGBA", (32, 22), (0, 0, 0, 0))
    frond(im, "jungle grass shade", "jungle grass", "jungle grass lit")
    save("fern", im, "a frond, three crossed")
    im = Image.new("RGBA", (16, 64), (0, 0, 0, 0))
    blades(im, 5, "reeds", "grass dark", "grass", "grass light", lean=0.05, low=0.7, head=("log dark", "log"))
    save("reeds", im, "upright stalks with brown heads")
    for alt, (head, eye) in enumerate([("lamp", "autumn gold"), ("snow", "lamp"), ("blue lamp", "snow")]):
        im = Image.new("RGBA", (24, 18), (0, 0, 0, 0))
        flowers(im, ("flowers", alt), "grass dark", "grass", head, eye)
        save("flowers" + (f".{alt}" if alt else ""), im, f"stems under {head} heads")
    for look, dots, share in [("", None, 0.0), (".bloom", "snow", 0.08), (".fruit", "coral", 0.1)]:
        im = Image.new("RGBA", (40, 32), (0, 0, 0, 0))
        bush(im, "bush", "shrub dark", "shrub", "grass light", dots, share)
        save("berry bush" + look, im, {"": "leaves", ".bloom": "white blossom (late spring)", ".fruit": "red berries (late summer)"}[look])
    return out


if __name__ == "__main__":
    files = make()
    drafts = DATA / "drafts.txt"
    have = drafts.read_text(encoding="utf-8").splitlines()
    drafts.write_text("\n".join(have + [f for f in files if f not in have]) + "\n", encoding="utf-8")
    print(f"{len(files)} files")
