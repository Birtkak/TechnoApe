#!/usr/bin/env python3
"""Make the first clutter models (ADR 0082): data/models/<name>.toml as painted boxes and <name>.png their painting,
at 32 pixels a metre, from data/palette.toml's colours only (ADR 0081). Every file goes on data/drafts.txt.

    python -I tools/paint_clutter.py

The style: clear shapes (the developer, 2026-11-12), in the flora's rich eight-tone ramps (2026-11-13): dark at the
root and in shade, lit at the tips and on top, no painted outlines (real shadow does that). A model sits
in its cell with its corner at 0 (x and z 0..32 px, y up from the ground), roughly centred, as the mesher turns it
about the cell's middle. A plane is a box with no thickness on one axis; its painting has empty pixels.
"""
import hashlib
import sys
from pathlib import Path

import numpy as np
from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
DATA = ROOT / "data"
OUT = DATA / "models"
sys.path.insert(0, str(ROOT / "tools"))
from paint_textures import MEADOW, Palette, rich_rgb, to8  # noqa: E402  the flora's ramps, one source

RED = [tuple(v / 255 for v in q) for q in [(70, 8, 14), (110, 14, 20), (150, 22, 24), (185, 30, 30), (215, 48, 40),
                                          (235, 80, 60), (245, 120, 95), (255, 185, 160)]]
RAMPS = {  # rich ramps, eight tones dark to light, shared with tools/paint_textures.py where the name is the same
    "meadow green": MEADOW, "fern": rich_rgb(104, 0.9, -0.04), "berry bush": rich_rgb(104, 0.85, -0.06),
    "berry": RED, "mushroom cap": RED, "mushroom stem": rich_rgb(40, 0.3, 0.22),
    "red petal": rich_rgb(2, 0.85, 0.08), "yellow petal": rich_rgb(50, 0.95, 0.15),
    "fallen leaves": rich_rgb(30, 0.85, 0.02), "twig": rich_rgb(28, 0.45, 0.0),
    "pebble": rich_rgb(210, 0.08, 0.08), "flint": rich_rgb(225, 0.18, -0.02),
    "flint cortex": rich_rgb(42, 0.22, 0.24),
}


def ramps():
    pal = Palette(DATA / "palette.toml")
    for base, tones in RAMPS.items():
        for i, c in enumerate(tones):
            if not pal.rgb(f"{base} r{i + 1}"):
                pal.add(f"{base} r{i + 1}", to8(np.asarray(c, float)), f"{base} rich")
    if pal.added:
        pal.save()
    return {r["name"]: r["rgb"] for r in pal.rows}


PAL = ramps()


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


def R(base, t):
    """Tone t (0 dark .. 7 light, clamped) of a rich ramp `base`."""
    return f"{base} r{min(7, max(0, int(round(t)))) + 1}"


def rock(ramp, seed, lift=0.0):
    """A stone or a stick: the top lit, the sides from shade at the ground to light at their top, soft noise, a few
    glints and pits; the bottom in shade, not black (real shadow darkens it)."""
    def p(face, x, y, w, h):
        n = (rnd(seed, face, x, y) - 0.5) * 1.6
        if face == "top":
            t = 5.2 - 1.2 * (x + y) / max(1, w + h) + n
        elif face == "bottom":
            t = 2 + n * 0.5
        else:
            t = 2.4 + 2.6 * (1 - y / max(1, h - 1)) + n
        r = rnd(seed, face, x, y, "g")
        t = 7 if r < 0.05 and face != "bottom" else (t - 2 if r > 0.95 else t)
        return R(ramp, t + lift)
    return p


def flint(seed):
    """A flint shard (ADR 0097): a dark glassy grey-black face flaked in shallow ripples with a glint, a pale chalky
    cortex on one long edge of the top and the upper half of the sides; calm, so it reads on grass and on gravel."""
    def p(face, x, y, w, h):
        n = (rnd(seed, face, x, y) - 0.5) * 0.8
        if face == "bottom":
            return R("flint", 1)
        if face == "top":
            if y >= h - 1 or (y >= h - 2 and rnd(seed, "c", x) < 0.5):  # the cortex rind along the far edge
                return R("flint cortex", 5 + n)
            ripple = (x + 2 * y) % 4 == 0  # conchoidal ripples, a shade lighter
            glint = x == w // 2 and y == 1
            return R("flint", 6 if glint else (3 if ripple else 2) + n)
        if y < h // 2:  # sides: the cortex above, the glassy core below
            return R("flint cortex", 4 + n)
        return R("flint", 1.5 + n)
    return p


def bush(leaf, berry, seed):
    """A low bush plane: leaves in soft clumps lit from the top left, berries with a glint; no outline."""
    def p(face, x, y, w, h):
        dx, dy = (x + 0.5 - w / 2) / (w / 2), (h - y - 0.5) / h
        if dx * dx + (dy - 0.45) ** 2 / 0.3 > 0.55 - (rnd(seed, x, y, "e") - 0.5) * 0.15:
            return None
        if rnd(seed, x // 2, y // 2, "b") < 0.12 and (x + y) % 2 == 0:
            return R(berry, 6 if rnd(seed, x, y, "g") < 0.3 else 4)
        clump = rnd(seed, x // 3, y // 3)
        return R(leaf, 2 + 2.5 * dy - 1.2 * dx + clump * 2.5 + (rnd(seed, x, y) - 0.5))
    return p


def leaf(ramp, seed):
    """A fallen leaf: lit toward its top-left rim, a vein down the middle in shade."""
    def p(face, x, y, w, h):
        dx, dy = (x + 0.5 - w / 2) / (w / 2), (y + 0.5 - h / 2) / (h / 2)
        if dx * dx + dy * dy > 1.0:
            return None
        if abs(dx) < 0.2:
            return R(ramp, 2.5)
        return R(ramp, 4.5 - 1.5 * (dx + dy) + (rnd(seed, x, y) - 0.5))
    return p


def cap(ramp, spots, seed):
    """A mushroom cap: lit on top with pale spots, its rim in shade, the gills under it the stem's pale shade."""
    def p(face, x, y, w, h):
        if face == "bottom":
            return R(spots, 3 + ((x + y) % 2))
        if face == "top":
            if rnd(seed, x, y, "s") < 0.12:
                return R(spots, 7)
            d = abs(x + 0.5 - w / 2) / (w / 2) + abs(y + 0.5 - h / 2) / (h / 2)
            return R(ramp, 6.5 - 2.5 * d + (rnd(seed, x, y) - 0.5))
        return R(ramp, 4 - 2 * y / max(1, h - 1))
    return p


def make():
    written = []
    # (grass tuft, fern and flowers became plants of planes: tools/paint_plants.py, ADR 0091)
    m = Model("fallen leaves")
    for k, (x, z) in enumerate([(6, 8), (16, 14), (11, 20), (20, 6)]):
        m.box((x, 0.5, z), (6, 0, 5), leaf("fallen leaves", k), (0, 40 * k, 0))
    written += m.write("autumn orange", "flat leaves on the ground")
    m = Model("twigs")
    for k, (x, z, t) in enumerate([(6, 12, 20), (10, 18, -35), (14, 8, 70)]):
        m.box((x, 0, z), (14, 1, 1), rock("twig", ("twig", k)), (0, t, 0))
    written += m.write("log", "thin sticks crossing")
    for alt, stones in enumerate([[(8, 10, 4, 2, 3), (18, 16, 3, 2, 3), (13, 22, 2, 1, 2)], [(10, 12, 3, 2, 4), (19, 9, 2, 1, 2)]]):
        m = Model("pebbles" + (f".{alt}" if alt else ""))
        for k, (x, z, sx, sy, sz) in enumerate(stones):
            m.box((x, 0, z), (sx, sy, sz), rock("pebble", ("pebble", alt, k)), (0, 25 * k, 0))
        written += m.write("stone", "a few small stones")
    m = Model("stick")
    m.box((7, 0.5, 15), (18, 1, 1), rock("twig", "stick"), (0, 15, 4))
    written += m.write("log", "one stick, lying tilted")
    m = Model("loose stone")
    m.box((12, 0, 12), (8, 4, 7), rock("pebble", "stone", -0.5), (0, 20, 6))
    written += m.write("stone", "a stone the size of a fist, tilted")
    m = Model("flint")
    m.box((13, 0, 13), (6, 2, 5), flint("flint"), (8, 35, 0))
    written += m.write("slate", "a dark shard")
    m = Model("mushroom")
    m.box((15, 0, 15), (2, 4, 2), rock("mushroom stem", "stem", 1.0))
    m.box((13, 4, 13), (6, 2, 6), cap("mushroom cap", "mushroom stem", "cap"))
    written += m.write("coral", "a stem under a red cap")
    m = Model("berries")
    for turn in (0, 90):
        m.box((9, 0, 16), (14, 10, 0), bush("berry bush", "berry", turn), (0, turn, 0))
    written += m.write("shrub", "a low bush with red berries")
    return written


if __name__ == "__main__":
    files = make()
    drafts = DATA / "drafts.txt"
    have = drafts.read_text(encoding="utf-8").splitlines()
    drafts.write_text("\n".join(have + [f for f in files if f not in have]) + "\n", encoding="utf-8")
    print(f"{len(files)} files")
