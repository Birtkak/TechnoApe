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
# ore samples (ADR draft-ore-samples-and-prospecting): a lump of each, in its mineral's colour; the block it looks
# like as an item
SAMPLES = [("copper nugget", "copper"), ("tin pebble", "cassiterite"), ("iron pebble", "hematite"), ("coal lump", "coal"), ("lead pebble", "galena"), ("gold nugget", "gold quartz"), ("pitchblende pebble", "pitchblende")]
RAMPS.update({"verdigris": rich_rgb(165, 0.5, 0.1), "copper nugget": rich_rgb(22, 0.75, 0.04), "tin pebble": rich_rgb(25, 0.25, -0.03), "iron pebble": rich_rgb(8, 0.55, 0.0), "coal lump": rich_rgb(220, 0.06, -0.05), "lead pebble": rich_rgb(215, 0.1, 0.06), "gold nugget": rich_rgb(46, 0.95, 0.08), "pitchblende pebble": rich_rgb(35, 0.15, -0.04)})


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

    def write(self, colours, note, item=None):
        h = max(1, self.y + self.row)
        img = Image.new("RGBA", (self.w, h), (0, 0, 0, 0))
        for (x, y), c in self.cells.items():
            img.putpixel((x, y), c)
        img.save(OUT / f"{self.name}.png")
        lines = [f"# A draft {self.name.split('.')[0]} (Claude's, listed in data/drafts.txt): clutter, ADR 0082. Painted",
                 f"# boxes at 32 px a metre ({note}); made by tools/paint_clutter.py.",
                 f"colours = [\"{colours}\"]"] + ([f"item = {item}"] if item else []) + ["", "[[part]]", "name = \"body\"", "box = [1, 1, 1]", "voxels = \".\""]
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


def axe(seed, top, cortex=False):
    """A knapped hand axe's flaked face (ADR 0110), `top` pixels under its point: dark flint, a pale ridge up the
    middle, flake scars curving away from it, the cut edge catching the light; `cortex`: the chalk rind on the butt."""
    def p(face, x, y, w, h):
        n = (rnd(seed, face, x, y + top) - 0.5) * 0.8
        if cortex and (y >= 1 or rnd(seed, "c", x) < 0.5):
            return R("flint cortex", 4.5 + n)
        if face != "side":
            return R("flint", 2 + n)
        mid = abs(x - w // 2)
        t = 4.5 if x in (0, w - 1) else 5 if mid == 0 else 3.5 if (mid + y + top) % 3 == 0 else 2.2
        return R("flint", t + n)
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


def spotted(base, ramp, share, tone=5):
    """`base`'s painting with a share of its pixels (never the bottom's) stained in `ramp`: verdigris on native
    copper, the yellow crust on pitchblende."""
    def make(seed):
        b = base(seed)
        def p(face, x, y, w, h):
            if face != "bottom" and rnd(seed, face, x // 2, y // 2, "stain") < share:
                return R(ramp, tone + (rnd(seed, face, x, y) - 0.5) * 2)
            return b(face, x, y, w, h)
        return p
    return make


def crystal(ramp, dim=0.0):
    """Flat crystal faces: each face one calm tone (the top brightest, the sides stepping darker round the box), its
    upper and left edge a line of light, no noise: galena's cut look, and with `dim` tones off cassiterite's dark one."""
    def make(seed):
        def p(face, x, y, w, h):
            if face == "bottom":
                return R(ramp, 1 - dim)
            if face == "top":
                return R(ramp, 7 - dim / 2 if x == 0 or y == 0 else 5.5 - dim)
            base = 2.5 + 2 * rnd(seed, "face", w, h)
            return R(ramp, 6.5 - dim / 2 if y == 0 or x == 0 else base - dim - 1.2 * y / max(1, h - 1))
        return p
    return make


def glossy(ramp):
    """Dull black with a hard shine: dark all over, thin diagonal streaks of light where a broken face catches
    the sun (coal, pitchblende)."""
    def make(seed):
        def p(face, x, y, w, h):
            n = (rnd(seed, face, x, y) - 0.5) * 0.8
            if face == "bottom":
                return R(ramp, 0.5)
            k = int(rnd(seed, face, "k") * 5)
            return R(ramp, 6.5 if (x + y + k) % 6 == 0 and rnd(seed, face, x, y, "s") < 0.7 else (2.4 if face == "top" else 1.4) + n)
        return p
    return make


def banded(ramp, other):
    """Wavy beds of two rocks, a few pixels each (banded iron: red hematite and grey chert)."""
    def make(seed):
        def p(face, x, y, w, h):
            n = (rnd(seed, face, x, y) - 0.5) * 1.2
            bed = (y + (x + int(3 * rnd(seed, face))) // 4) % 4
            lit = 5 if face == "top" else 2 if face == "bottom" else 4.2 - 1.8 * y / max(1, h - 1)
            return R(other if bed == 0 else ramp, lit + (1 if bed == 2 else 0) + n)
        return p
    return make


def lump(name, lift=0.0):
    return lambda seed: rock(name, seed, lift)


# The ore samples (ADR 0138), each to be told from the others at a glance on the ground and in a slot: its boxes
# (x, y, z, sx, sy, sz, turn, painting) and a note for its file.
ORE = {
    "copper nugget": ([(11, 0, 12, 8, 3, 7, (0, 25, 5), spotted(lump("copper nugget"), "verdigris", 0.16)),
                       (13, 2, 14, 5, 3, 4, (0, 60, 0), spotted(lump("copper nugget", 0.8), "verdigris", 0.1)),
                       (17, 1, 11, 4, 3, 3, (0, 10, 8), lump("copper nugget", 1.2))], "knobbly native copper, green where it weathered"),
    "tin pebble": ([(12, 0, 12, 8, 3, 8, (0, 20, 0), crystal("tin pebble", 2.5)), (13, 3, 13, 6, 2, 6, (0, 20, 0), crystal("tin pebble", 2.5)),
                    (14, 5, 14, 4, 2, 4, (0, 20, 0), crystal("tin pebble", 2.5)), (15, 7, 15, 2, 1, 2, (0, 20, 0), crystal("tin pebble", 2.5))], "a squat dark crystal stepping to a point"),
    "iron pebble": ([(11, 0, 12, 10, 3, 8, (0, 15, 0), banded("iron pebble", "pebble")), (12, 3, 13, 8, 2, 6, (0, 15, 0), banded("iron pebble", "pebble"))], "a rounded stone in red and grey beds"),
    "coal lump": ([(11, 0, 11, 9, 5, 8, (0, 30, 4), glossy("coal lump")), (15, 4, 13, 5, 3, 5, (6, 55, 0), glossy("coal lump"))], "a blocky black lump with a hard shine"),
    "lead pebble": ([(11, 0, 13, 6, 6, 6, (0, 10, 0), crystal("lead pebble")), (16, 0, 10, 5, 5, 5, (0, 35, 0), crystal("lead pebble")),
                     (14, 5, 13, 4, 4, 4, (0, 50, 0), crystal("lead pebble"))], "silver cubes grown into each other"),
    "gold nugget": ([(13, 0, 13, 6, 3, 5, (0, 30, 6), lump("gold nugget", 1.5)), (15, 2, 14, 3, 2, 3, (0, 70, 0), lump("gold nugget", 2.2))], "a small bright nugget"),
    "pitchblende pebble": ([(11, 0, 12, 8, 4, 7, (0, 25, 0), spotted(glossy("pitchblende pebble"), "yellow petal", 0.07, 6)),
                            (13, 3, 13, 5, 3, 4, (0, 40, 0), spotted(glossy("pitchblende pebble"), "yellow petal", 0.07, 6)),
                            (17, 2, 15, 3, 3, 3, (0, 0, 0), glossy("pitchblende pebble"))], "black rounded lumps with a yellow crust"),
}


def metal(seed, edge=None):
    """Cast copper (ADR 0139): warm and calm, lit from the top left with one bright streak down each side face, the
    bottom rows in shade, a rare pit from the mould; `edge`: "x+", "x-" or "y+", the side whose last pixels are a
    worked edge and catch the light (a blade, a pick's point)."""
    def p(face, x, y, w, h):
        n = (rnd(seed, face, x, y) - 0.5) * 0.7
        if face == "top":
            return R("copper nugget", 5.6 + n)
        if face == "bottom":
            return R("copper nugget", 1.8 + n * 0.5)
        t = 4.6 - 2.2 * y / max(1, h - 1) - 0.8 * x / max(1, w - 1) + n
        if w > 2 and x == max(1, w // 4):  # the streak
            t += 1.4
        if (edge == "x+" and x >= w - 1) or (edge == "x-" and x == 0) or (edge == "y+" and y == 0):
            t = 6.6 + n
        if rnd(seed, face, x, y, "pit") > 0.97:
            t -= 2
        return R("copper nugget", t)
    return p


# The copper age's things (ADR 0139), each standing as the hand holds it, its face to +z (the eye in hand): the
# head's boxes (x, y, z, sx, sy, sz, edge), the handle's height (a stick 2 px thick up the middle; 0 for none) and
# the size of its cell as an item, metres. A head alone is its tool's head set down on the ground.
COPPER = {
    "pick": ([(11, 23, 14.5, 10, 4, 3, None), (6, 21, 15, 5, 3, 2, None), (21, 21, 15, 5, 3, 2, None),
              (3, 19, 15, 3, 2, 2, "x-"), (26, 19, 15, 3, 2, 2, "x+")], 27, 0.9),
    "axe": ([(13, 19, 14.5, 9, 7, 3, None), (22, 17, 15, 4, 11, 2, "x+")], 27, 0.9),
    "shovel": ([(11, 18, 15, 10, 2, 2, None), (11, 20, 15.5, 10, 5, 1, None), (12, 25, 15.5, 8, 2, 1, None),
                (13, 27, 15.5, 6, 2, 1, None), (14, 29, 15.5, 4, 1, 1, "y+")], 19, 0.9),
    "knife": ([(14.5, 7, 15.5, 3, 8, 1, "x+"), (15, 15, 15.5, 2, 3, 1, "x+"), (15.5, 18, 15.5, 1, 2, 1, "y+")], 7, 0.5),
    "hammer": ([(10, 21, 13.5, 12, 6, 5, None)], 26, 0.9),
}
HEADS = {"pick": "copper pick head", "axe": "copper axe head", "shovel": "copper shovel head", "knife": "copper knife blade", "hammer": "copper hammer head"}


def copper():
    written = []
    for kind, (boxes, handle, cell) in COPPER.items():
        low = min(b[1] for b in boxes)
        for name, shaft, drop, size in [(f"copper {kind}", handle, 0, cell), (HEADS[kind], 0, low, 0.45)]:
            m = Model(name)
            if shaft:
                m.box((15, 0, 15), (2, shaft, 2), rock("twig", (name, "shaft")))
            for k, (x, y, z, sx, sy, sz, edge) in enumerate(boxes):
                m.box((x, y - drop, z), (sx, sy, sz), metal((kind, k), edge))
            written += m.write("copper", f"cast copper{', on a stick' if shaft else ', the head alone'}", item=size)
    m = Model("copper ingot")
    m.box((10, 0, 13), (12, 4, 6), metal("ingot"))
    m.box((11, 4, 14), (10, 1, 4), metal("ingot top"))
    written += m.write("copper", "a cast bar, narrower on top", item=0.4)
    return written


def make():
    written = copper()
    # (grass tuft, fern and flowers became plants of planes: tools/paint_plants.py, ADR 0091)
    m = Model("fallen leaves")
    for k, (x, z) in enumerate([(6, 8), (16, 14), (11, 20), (20, 6)]):
        m.box((x, 0.5, z), (6, 0, 5), leaf("fallen leaves", k), (0, 40 * k, 0))
    written += m.write("autumn orange", "flat leaves on the ground")
    # sizes that read from standing height (bug 2026-11-13_14-24-52, ADR 0099): a fallen branch 0.6-0.9 m with a
    # side twig, stones 0.2-0.35 m
    m = Model("twigs")
    for k, (x, z, n, w, t) in enumerate([(3, 12, 28, 2, 20), (9, 20, 20, 2, -35), (14, 6, 12, 1, 70)]):
        m.box((x, 0, z), (n, w, w), rock("twig", ("twig", k)), (0, t, 0))
    written += m.write("log", "a fallen branch and sticks crossing")
    for alt, stones in enumerate([[(6, 8, 10, 5, 8), (19, 16, 7, 4, 7), (12, 22, 5, 3, 4)], [(8, 10, 11, 6, 9), (21, 7, 6, 3, 5)]]):
        m = Model("pebbles" + (f".{alt}" if alt else ""))
        for k, (x, z, sx, sy, sz) in enumerate(stones):
            m.box((x, 0, z), (sx, sy, sz), rock("pebble", ("pebble", alt, k)), (0, 25 * k, 0))
        written += m.write("stone", "a few stones")
    m = Model("stick")
    m.box((3, 1, 15), (26, 2, 2), rock("twig", "stick"), (0, 15, 4))
    written += m.write("log", "one stick, lying tilted")
    m = Model("loose stone")
    m.box((10, 0, 10), (12, 6, 10), rock("pebble", "stone", -0.5), (0, 20, 6))
    written += m.write("stone", "a stone of two fists, tilted")
    m = Model("flint")
    m.box((13, 0, 13), (6, 2, 5), flint("flint"), (8, 35, 0))
    # a cell 0.6 m as an item (ADR 0114): a 12 cm shard in hand and in a pile; the ground's clutter keeps its 19 cm
    written += m.write("slate", "a dark shard", item=0.6)
    m = Model("mushroom")
    m.box((15, 0, 15), (2, 4, 2), rock("mushroom stem", "stem", 1.0))
    m.box((13, 4, 13), (6, 2, 6), cap("mushroom cap", "mushroom stem", "cap"))
    written += m.write("coral", "a stem under a red cap")
    m = Model("berries")
    for turn in (0, 90):
        m.box((9, 0, 16), (14, 10, 0), bush("berry bush", "berry", turn), (0, turn, 0))
    written += m.write("shrub", "a low bush with red berries")
    # the hand axe (ADR 0110), standing point up as the hand holds it, its flaked face to +z (the eye in hand)
    m = Model("hand axe")
    for x, y, z, sx, sy, sz in [(12, 0, 14.5, 8, 3, 3), (10, 3, 14, 12, 5, 4), (11, 8, 14, 10, 5, 4), (12.5, 13, 14.5, 7, 4, 3), (14, 17, 15, 4, 3, 2), (15, 20, 15, 2, 2, 2)]:
        m.box((x, y, z), (sx, sy, sz), axe("hand axe", 22 - y - sy, y == 0))
    # one cell 0.3 m as an item (ADR 0114): about 21 cm long in hand and lying in a pile
    written += m.write("slate", "a knapped flint teardrop, point up", item=0.3)
    for name, look in SAMPLES:
        m = Model(name)
        shape, note = ORE[name]
        for k, (x, y, z, sx, sy, sz, turn, paint) in enumerate(shape):
            m.box((x, y, z), (sx, sy, sz), paint((name, k)), turn)
        written += m.write(look, note, item=0.4)
    return written


if __name__ == "__main__":
    files = make()
    drafts = DATA / "drafts.txt"
    have = drafts.read_text(encoding="utf-8").splitlines()
    drafts.write_text("\n".join(have + [f for f in files if f not in have]) + "\n", encoding="utf-8")
    print(f"{len(files)} files")
