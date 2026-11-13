#!/usr/bin/env python3
"""Make the first clutter models (ADR 0082): data/models/<name>.toml as painted boxes and <name>.png their painting,
at 32 pixels a metre, from data/palette.toml's colours only (ADR 0081). Every file goes on data/drafts.txt.

    python -I tools/paint_clutter.py

The style (the developer's call, 2026-11-12): calm and readable, few colours a piece, clear shapes. A model sits
in its cell with its corner at 0 (x and z 0..32 px, y up from the ground), roughly centred, as the mesher turns it
about the cell's middle. A plane is a box with no thickness on one axis; its painting has empty pixels.
"""
import hashlib
import tomllib
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
DATA = ROOT / "data"
OUT = DATA / "models"
PAL = {c["name"]: c["rgb"] for c in tomllib.loads((DATA / "palette.toml").read_text(encoding="utf-8"))["colour"]}


def rgb(name):
    h = PAL[name]
    return (int(h[1:3], 16), int(h[3:5], 16), int(h[5:7], 16), 255)


def rnd(*k):
    return int.from_bytes(hashlib.blake2b(repr(k).encode(), digest_size=4).digest(), "little") / 2**32


class Model:
    """Boxes laid out in one painting, shelf by shelf."""

    def __init__(self, name, w=64):
        self.name, self.w = name, w
        self.boxes, self.cells = [], {}
        self.x = self.y = self.row = 0

    def box(self, at, size, paint, turn=(0, 0, 0)):
        sx, sy, sz = size
        nw, nh = 2 * (sx + sz), sz + sy
        if self.x + nw > self.w:
            self.x, self.y, self.row = 0, self.y + self.row, 0
        u, v = self.x, self.y
        self.x += nw
        self.row = max(self.row, nh)
        self.boxes.append((at, size, turn, (u, v)))
        # the net (crates/engine/src/models.rs `Bx::rects`): +x, -x, +y, -y, +z, -z; a plane uses its positive face's
        rects = [(u + sz + sx, v + sz, sz, sy, "side"), (u, v + sz, sz, sy, "side"), (u + sz, v, sx, sz, "top"), (u + sz + sx, v, sx, sz, "bottom"), (u + sz, v + sz, sx, sy, "side"), (u + 2 * sz + sx, v + sz, sx, sy, "side")]
        thin = [a for a in range(3) if size[a] == 0]
        for k, (x0, y0, w, h, face) in enumerate(rects):
            if thin and k != 2 * thin[0]:
                continue
            for y in range(h):
                for x in range(w):
                    c = paint(face, x, y, w, h)
                    if c:
                        self.cells[(x0 + x, y0 + y)] = rgb(c)

    def write(self, colours, note):
        h = max(1, self.y + self.row)
        img = Image.new("RGBA", (self.w, h), (0, 0, 0, 0))
        for (x, y), c in self.cells.items():
            img.putpixel((x, y), c)
        img.save(OUT / f"{self.name}.png")
        lines = [f"# A draft {self.name.split('.')[0]} (Claude's, listed in data/drafts.txt): clutter, ADR 0082. Painted",
                 f"# boxes at 32 px a metre ({note}); made by tools/paint_clutter.py.",
                 f"colours = [\"{colours}\"]", "", "[[part]]", "name = \"body\"", "box = [1, 1, 1]", "voxels = \".\""]
        for at, size, turn, uv in self.boxes:
            lines += ["", "[[part.boxes]]", f"at = [{', '.join(str(float(a)) for a in at)}]", f"size = [{', '.join(map(str, size))}]"]
            if any(turn):
                lines.append(f"turn = [{', '.join(str(float(t)) for t in turn)}]")
            lines.append(f"uv = [{uv[0]}, {uv[1]}]")
        (OUT / f"{self.name}.toml").write_text("\n".join(lines) + "\n", encoding="utf-8")
        return [f"data/models/{self.name}.toml", f"data/models/{self.name}.png"]


def solid(top, side, bottom=None, speck=None, seed=""):
    """A box painted one colour a face kind, a few specks of another on its sides and top."""
    def p(face, x, y, w, h):
        base = {"top": top, "side": side, "bottom": bottom or side}[face]
        if speck and face != "bottom" and rnd(seed, face, x, y) < 0.18:
            return speck
        return base
    return p


def blades(dark, mid, lit, n, seed, lean=0.25):
    """A plane of grass blades: n tapering blades from the bottom edge, dark at the root, lit at the tip."""
    def p(face, x, y, w, h):
        for b in range(n):
            x0 = (b + 0.5) * w / n + (rnd(seed, b) - 0.5) * w / n * 0.8
            top = h * (0.35 + 0.65 * rnd(seed, b, "h"))
            t = (h - y) / h  # 0 at the root
            if h - y > top:
                continue
            cx = x0 + (rnd(seed, b, "l") - 0.5) * 2 * lean * (h - y)
            if abs(x + 0.5 - cx) <= max(0.6, 1.2 * (1 - (h - y) / top)):
                return dark if t < 0.3 else (lit if (h - y) > top * 0.75 else mid)
        return None
    return p


def frond(dark, mid, lit, seed):
    """A fern's plane: a rib from the bottom middle, leaflets either side shortening to the tip."""
    def p(face, x, y, w, h):
        cx, yy = w / 2, h - y  # height over the ground
        if abs(x + 0.5 - cx) < 0.6 and yy <= h - 1:
            return dark
        reach = (w / 2 - 1) * (1 - yy / h) ** 0.6
        if yy > 2 and yy % 2 < 1 and abs(x + 0.5 - cx) <= reach:
            return lit if abs(x + 0.5 - cx) > reach - 1.5 else mid
        return None
    return p


def flowers(stem, head, eye, seed):
    def p(face, x, y, w, h):
        for b in range(3):
            fx = 1.5 + b * (w - 3) / 2
            top = 4 + int(rnd(seed, b) * (h - 5))
            yy = h - 1 - y
            if abs(x - fx) < 0.6 and yy < top:
                return stem
            if abs(x - fx) <= 1.1 and abs(yy - top) <= 1.1:
                return eye if (abs(x - fx) < 0.6 and abs(yy - top) < 0.6) else head
        return None
    return p


def bush(dark, mid, berry, seed):
    def p(face, x, y, w, h):
        dx, dy = (x + 0.5 - w / 2) / (w / 2), (h - y - 0.5) / h
        if dx * dx + (dy - 0.45) ** 2 / 0.3 > 0.55:
            return None
        r = rnd(seed, x, y)
        if r < 0.1:
            return berry
        return mid if r < 0.6 else dark
    return p


def leaf(c, vein):
    def p(face, x, y, w, h):
        dx, dy = (x + 0.5 - w / 2) / (w / 2), (y + 0.5 - h / 2) / (h / 2)
        if dx * dx + dy * dy > 1.0:
            return None
        return vein if abs(dx) < 0.2 else c
    return p


def make():
    written = []
    # grass tuft: two crossed planes of blades, and an alternate of three
    for alt, (n, planes) in enumerate([(5, [(0, 0), (90, 0)]), (4, [(30, 1), (100, 2), (160, 3)])]):
        m = Model("grass tuft" + (f".{alt}" if alt else ""))
        for turn, s in planes:
            m.box((10, 0, 16), (12, 10, 0), blades("grass shade", "grass", "grass lit", n, ("tuft", alt, s)), (0, turn, 0))
        written += m.write("grass", "crossed planes of blades")
    m = Model("fern")
    for turn in (0, 60, 120):
        m.box((6, 0, 16), (20, 12, 0), frond("jungle grass shade", "jungle grass", "jungle grass lit", turn), (-20, turn, 0))
    written += m.write("jungle grass", "three fronds leaning out")
    m = Model("fallen leaves")
    for k, (x, z, c) in enumerate([(6, 8, "autumn rust"), (16, 14, "autumn orange"), (11, 20, "autumn gold"), (20, 6, "autumn rust")]):
        m.box((x, 0.5, z), (6, 0, 5), leaf(c, "autumn dark"), (0, 40 * k, 0))
    written += m.write("autumn orange", "flat leaves on the ground")
    m = Model("twigs")
    for k, (x, z, t) in enumerate([(6, 12, 20), (10, 18, -35), (14, 8, 70)]):
        m.box((x, 0, z), (14, 1, 1), solid("log lit", "log", "log dark", seed=k), (0, t, 0))
    written += m.write("log", "thin sticks crossing")
    m = Model("flowers")
    for turn, head in ((0, "lamp"), (90, "snow")):
        m.box((12, 0, 16), (8, 10, 0), flowers("grass shade", head, "autumn gold", turn), (0, turn, 0))
    written += m.write("lamp", "crossed planes of stems and heads")
    for alt, stones in enumerate([[(8, 10, 4, 2, 3), (18, 16, 3, 2, 3), (13, 22, 2, 1, 2)], [(10, 12, 3, 2, 4), (19, 9, 2, 1, 2)]]):
        m = Model("pebbles" + (f".{alt}" if alt else ""))
        for k, (x, z, sx, sy, sz) in enumerate(stones):
            m.box((x, 0, z), (sx, sy, sz), solid("stone lit", "stone", "stone shade", seed=k), (0, 25 * k, 0))
        written += m.write("stone", "a few small stones")
    m = Model("stick")
    m.box((7, 0.5, 15), (18, 1, 1), solid("log lit", "log", "log dark", "log wood", "stick"), (0, 15, 4))
    written += m.write("log", "one stick, lying tilted")
    m = Model("loose stone")
    m.box((12, 0, 12), (8, 4, 7), solid("stone lit", "stone", "stone shade", "stone shade", "stone"), (0, 20, 6))
    written += m.write("stone", "a stone the size of a fist, tilted")
    m = Model("flint")
    m.box((13, 0, 13), (6, 2, 5), solid("slate light", "slate", "slate dark", "slate lit", "flint"), (8, 35, 0))
    written += m.write("slate", "a dark shard")
    m = Model("mushroom")
    m.box((15, 0, 15), (2, 4, 2), solid("limestone light", "limestone", seed="stem"))
    m.box((13, 4, 13), (6, 2, 6), solid("coral", "coral shade", "limestone", "limestone light", "cap"))
    written += m.write("coral", "a stem under a red cap")
    m = Model("berries")
    for turn in (0, 90):
        m.box((9, 0, 16), (14, 10, 0), bush("shrub dark", "shrub", "coral", turn), (0, turn, 0))
    written += m.write("shrub", "a low bush with red berries")
    return written


if __name__ == "__main__":
    files = make()
    drafts = DATA / "drafts.txt"
    have = drafts.read_text(encoding="utf-8").splitlines()
    drafts.write_text("\n".join(have + [f for f in files if f not in have]) + "\n", encoding="utf-8")
    print(f"{len(files)} files")
