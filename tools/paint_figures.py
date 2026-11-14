#!/usr/bin/env python3
"""Paint the figures (ADR 0116): the ape, boar, deer, hare and snake, the furnace and the leaf as painted boxes at
32 px a metre (a voxel 4 px), from data/palette.toml's colours only (ADR 0081). Every file goes on data/drafts.txt.

    python -I tools/paint_figures.py

Each model's file is rewritten in place: every part keeps its name, parent, at, pivot, box, clips, points and
states (the walk rig, the bare arm and the furnace's life read them); its voxels become empty and its surface is
`[[part.boxes]]` filling the volume its voxels filled, plus small boxes where a part is not a plain box (snouts,
ears, tusks, fingers, feet, tails). The house style of tools/paint_clutter.py: rich eight-tone ramps, dark at the
root and in shade, lit on top, no painted outlines, calm. A recipe here is `paint(s)` over a pixel `s` that knows
its face and where it is in its part (pixels), so features (eyes, a chest) are placed in 3D.
"""
import re
import sys
import tomllib
from pathlib import Path

from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parent))
import paint_clutter as pc  # noqa: E402  the Model's net, the ramps, the palette, one source
from paint_clutter import DATA, OUT, R, rnd  # noqa: E402
from paint_textures import Palette, rich_rgb, to8  # noqa: E402

W = 128  # engine MAX_PAINT: a painting at most 128 px a side
MAX_SIDE = 63  # a box over 63 px a side is not drawn on the instance path (ADR 0114)
RAMPS = {
    "ape fur": rich_rgb(20, 0.62, 0.0), "ape skin": rich_rgb(26, 0.6, 0.12),
    "boar bristle": rich_rgb(24, 0.32, -0.05), "boar snout": rich_rgb(8, 0.38, 0.12), "tusk": rich_rgb(45, 0.28, 0.30),
    "deer coat": rich_rgb(30, 0.62, 0.05), "deer pale": rich_rgb(38, 0.28, 0.26),
    "hare fur": rich_rgb(32, 0.26, 0.05), "hare pale": rich_rgb(40, 0.12, 0.32), "hare pink": rich_rgb(355, 0.45, 0.22),
    "snake scale": rich_rgb(95, 0.55, 0.02), "snake belly": rich_rgb(60, 0.38, 0.22),
    "furnace stone": rich_rgb(25, 0.08, 0.06), "ember": rich_rgb(22, 1.0, 0.12), "door copper": rich_rgb(20, 0.72, 0.05),
    "eye": rich_rgb(30, 0.15, -0.03),
}


def ramps():
    pal = Palette(DATA / "palette.toml")
    for base, tones in RAMPS.items():
        for i, c in enumerate(tones):
            if not pal.rgb(f"{base} r{i + 1}"):
                pal.add(f"{base} r{i + 1}", to8(c), f"{base} rich")
    if pal.added:
        pal.save()
    pc.PAL.update({r["name"]: r["rgb"] for r in pal.rows})


class S:
    """One painted pixel: its face (+x -x top bottom +z -z), where it is in its box (`q`) and in its part (`p`),
    pixels, and its box's size."""
    __slots__ = ("f", "q", "p", "size")

    def __init__(self, f, q, p, size):
        self.f, self.q, self.p, self.size = f, q, p, size

    def up(self):
        """0 at the box's foot, 1 at its top."""
        return self.q[1] / max(1, self.size[1])


FACES = ["+x", "-x", "top", "bottom", "+z", "-z"]


class Figure(pc.Model):
    """pc.Model's net, its boxes per named part, packed tallest first into one painting."""

    def __init__(self, name):
        super().__init__(name, W)
        self.want = []  # (part, at, size, paint)
        self.parts = {}
        self.quads = 0

    def add(self, part, at, size, paint):
        assert max(size) <= MAX_SIDE, f"{self.name} {part}: a box {size} over {MAX_SIDE} px; split it"
        self.want.append((part, at, size, paint))

    def lay(self):
        for part, at, size, paint in sorted(self.want, key=lambda b: -(b[2][2] + b[2][1])):
            n = len(self.boxes)
            self.box3(at, size, paint)
            self.parts.setdefault(part, []).append(self.boxes[n])
        assert self.y + self.row <= W, f"{self.name}: the painting is {self.y + self.row} px high, over {W}"

    def box3(self, at, size, paint):
        sx, sy, sz = size
        nw, nh = 2 * (sx + sz), sz + sy
        if self.x + nw > self.w:
            self.x, self.y, self.row = 0, self.y + self.row, 0
        u, v = self.x, self.y
        self.x += nw
        self.row = max(self.row, nh)
        self.boxes.append((at, size, (0, 0, 0), (u, v)))
        # the net (crates/engine/src/models.rs `Bx::rects`) and each face's corner and edges (`Bx::pixel_quads`)
        rects = [(u + sz + sx, v + sz, sz, sy), (u, v + sz, sz, sy), (u + sz, v, sx, sz), (u + sz + sx, v, sx, sz), (u + sz, v + sz, sx, sy), (u + 2 * sz + sx, v + sz, sx, sy)]
        frames = [((sx, sy, sz), (0, 0, -sz), (0, -sy, 0)), ((0, sy, 0), (0, 0, sz), (0, -sy, 0)), ((0, sy, 0), (sx, 0, 0), (0, 0, sz)),
                  ((0, 0, sz), (sx, 0, 0), (0, 0, -sz)), ((0, sy, sz), (sx, 0, 0), (0, -sy, 0)), ((sx, sy, 0), (-sx, 0, 0), (0, -sy, 0))]
        thin = [a for a in range(3) if size[a] == 0]
        for k, ((x0, y0, w, h), (o, eu, ev)) in enumerate(zip(rects, frames)):
            if thin and k != 2 * thin[0]:
                continue
            for y in range(h):
                for x in range(w):
                    q = tuple(o[a] + eu[a] * (x + 0.5) / w + ev[a] * (y + 0.5) / h for a in range(3))
                    c = paint(S(FACES[k], q, tuple(q[a] + at[a] for a in range(3)), size))
                    if c:
                        self.cells[(x0 + x, y0 + y)] = pc.rgb(c)
                        self.quads += 2 if thin else 1

    def write(self):
        """The model's own file with its voxels emptied and its boxes after each part's voxels."""
        self.lay()
        path = OUT / f"{self.name}.toml"
        text = path.read_text(encoding="utf-8").replace("\r\n", "\n")
        text = re.sub(r"\n\[\[part\.boxes\]\]\n(?:[a-z]+ = .*\n)*", "\n", text)  # a run before
        sizes = {p["name"]: p["box"] for p in tomllib.loads(text)["part"]}
        assert set(self.parts) <= set(sizes), f"{self.name}: boxes on parts it lacks: {set(self.parts) - set(sizes)}"

        def part(m):
            name = m.group("n")
            sx, sy, sz = sizes[name]
            grid = "\n\n".join("\n".join("." * sx for _ in range(sz)) for _ in range(sy))
            out = m.group("head") + f'voxels = """\n{grid}\n"""\n'
            for at, size, _, uv in self.parts.get(name, []):
                out += f"\n[[part.boxes]]\nat = [{', '.join(str(float(a)) for a in at)}]\nsize = [{', '.join(map(str, size))}]\nuv = [{uv[0]}, {uv[1]}]\n"
            return out
        text, n = re.subn(r'(?P<head>\[\[part\]\]\nname = "(?P<n>[^"]+)"\n(?:(?!voxels)[a-z]+ = .*\n)*)voxels = (?:"""\n(?:.*\n)*?"""|"[^"\n]*")\n', part, text)
        assert n == len(sizes), f"{self.name}: {n} parts rewritten of {len(sizes)}"
        note = "# Painted boxes at 32 px a metre (ADR 0116), made by tools/paint_figures.py; the voxels stay empty."
        if note not in text:
            lines = text.split("\n")
            k = next(i for i, l in enumerate(lines) if not l.startswith("#"))
            text = "\n".join(lines[:k] + [note] + lines[k:])
        text = re.sub(r"\n{3,}", "\n\n", text)
        path.write_text(text, encoding="utf-8", newline="\n")
        img = Image.new("RGBA", (self.w, max(1, self.y + self.row)), (0, 0, 0, 0))
        for (x, y), c in self.cells.items():
            img.putpixel((x, y), c)
        img.save(OUT / f"{self.name}.png")
        print(f"{self.name}: {len(self.boxes)} boxes, {self.quads} quads, painting {img.size[0]} x {img.size[1]}")
        return [f"data/models/{self.name}.toml", f"data/models/{self.name}.png"]


# ---------------------------------------------------------------- recipes

def coat(ramp, seed, base=3.0, top=2.4, streak=0.7):
    """Fur or hide: lit on top, from shade at a box's foot to light at its top, soft strands down the sides."""
    def p(s):
        x, y, z = (int(v) for v in s.p)
        n = (rnd(seed, s.f, x, y, z) - 0.5) * 0.9
        if s.f == "top":
            t = base + top + (rnd(seed, "t", x // 2, z // 2) - 0.5) * streak
        elif s.f == "bottom":
            t = base - 1.2
        else:
            t = base - 0.8 + 2.0 * s.up() + (rnd(seed, "st", x, z, y // 3) - 0.5) * streak * 1.6
        return R(ramp, t + n * 0.5)
    return p


def over(base, *rules):
    """`base` with features on top: each rule a function of the pixel giving a colour or None."""
    def p(s):
        for r in rules:
            c = r(s)
            if c:
                return c
        return base(s)
    return p


def eye(face, cx, cy, r=1.0, glint=True):
    """A dark eye on `face` centred at (cx, cy) in the face's own two axes (x or z across, y up), with a glint."""
    def p(s):
        if s.f != face:
            return None
        a = s.p[0] if face in ("+z", "-z") else s.p[2]
        if abs(a - cx) <= r and abs(s.p[1] - cy) <= r:
            return R("eye", 6 if glint and a < cx and s.p[1] > cy else 0.5)
        return None
    return p


def ape(m):
    fur, skin = coat("ape fur", "ape"), coat("ape skin", "ape skin", base=3.4, streak=0.3)
    # body 16 x 16 x 8: a lighter chest on the front
    chest = lambda s: R("ape skin", 2.6 + 2.0 * s.up() + (rnd("ch", int(s.p[0]), int(s.p[1])) - 0.5) * 0.6) if s.f == "+z" and 3 <= s.p[0] <= 13 and 3 <= s.p[1] <= 13 and (s.p[0] - 8) ** 2 / 30 + (s.p[1] - 9) ** 2 / 45 < 1 else None
    m.add("body", (0, 0, 0), (16, 16, 8), over(fur, chest))
    # head 16 x 12 x 12: a pale face with eyes under a dark brow, a muzzle with nostrils and a mouth, ears at the sides
    def face(s):
        if s.f != "+z":
            return None
        x, y = s.p[0], s.p[1]
        if 7 <= y <= 8 and 2.5 <= x <= 13.5 and not 6.5 <= x <= 9.5:
            return R("ape fur", 0.8)  # the brow
        if (x - 8) ** 2 / 36 + (y - 4.5) ** 2 / 20 > 1 and y < 7:
            return None
        if y >= 9:
            return None
        return R("ape skin", 3.2 + 0.25 * y + (rnd("face", int(x), int(y)) - 0.5) * 0.4)
    eyes = lambda s: (eye("+z", 4.5, 5.5, 1.0)(s) or eye("+z", 11.5, 5.5, 1.0)(s))
    m.add("head", (0, 0, 0), (16, 12, 12), over(fur, eyes, face))
    muzzle = lambda s: (R("ape skin", 0.6) if s.f == "+z" and (abs(s.p[1] - 3.5) < 0.6 and abs(s.p[0] - 8) < 3 or s.p[1] > 4.5 and s.p[1] < 5.5 and s.p[0] in (6.5, 9.5)) else None)
    m.add("head", (4, 1, 12), (8, 5, 2), over(skin, muzzle, lambda s: R("ape skin", 4.5) if s.f == "top" else None))
    for x in (-2, 16):
        m.add("head", (x, 5, 5), (2, 4, 3), over(fur, lambda s: R("ape skin", 2.5) if s.f in ("+z", "-z") else None))
    # arms 4 x 20 x 8: a furred forearm to a cuff, a palm, four fingers along z and a thumb in front
    for arm, inner in (("arm_l", "-x"), ("arm_r", "+x")):
        m.add(arm, (0, 8, 0), (4, 12, 8), fur)
        knuckles = lambda s, inner=inner: R("ape skin", 4.8) if s.f not in (inner, "bottom") and s.q[1] < 1.2 and int(s.p[2]) % 2 == 0 else None
        m.add(arm, (0, 3, 0), (4, 5, 8), over(coat("ape skin", arm, base=2.6, streak=0.2), knuckles))
        for k in range(4):
            m.add(arm, (0.5, 0, k * 2), (3, 3, 2), over(coat("ape skin", (arm, k), base=2.4 - 0.2 * k, streak=0.2), lambda s: R("ape skin", 5) if s.f == "bottom" else None))
        tx = 3 if inner == "+x" else -1
        m.add(arm, (tx, 3, 6), (2, 3, 2), coat("ape skin", (arm, "thumb"), base=3.0, streak=0.2))
    # legs 8 x 16 x 8: fur to the ankle, a long dark foot with its toes forward
    for leg in ("leg_l", "leg_r"):
        m.add(leg, (0, 3, 0), (8, 13, 8), fur)
        toes = lambda s: R("ape skin", 1.0) if s.f in ("+z", "top") and s.p[2] > 9 and int(s.p[0]) % 2 == 1 else None
        m.add(leg, (0, 0, -1), (8, 3, 11), over(coat("ape skin", (leg, "foot"), base=2.0, top=1.6, streak=0.2), toes))


def boar(m):
    hide = coat("boar bristle", "boar", base=2.2, streak=1.2)
    ridge = lambda s: R("boar bristle", 0.8 + rnd("ridge", int(s.p[2])) * 0.8) if s.f == "top" and abs(s.p[0] - 8) < 2.5 else None
    belly = lambda s: R("boar bristle", 3.6) if s.f in ("+x", "-x", "+z", "-z") and s.p[1] < 2.5 else None
    m.add("body", (0, 0, 0), (16, 12, 24), over(hide, ridge, belly))
    eyes = lambda s: (eye("+z", 3.5, 8.5, 0.6, False)(s) or eye("+z", 12.5, 8.5, 0.6, False)(s))
    m.add("head", (0, 0, 0), (16, 12, 12), over(hide, eyes, ridge))
    nostrils = lambda s: R("boar snout", 0.5) if s.f == "+z" and abs(s.p[1] - 3.5) < 1 and abs(abs(s.p[0] - 8) - 1.5) < 0.6 else None
    m.add("head", (4, 1, 12), (8, 6, 4), over(coat("boar snout", "snout", base=3.2, streak=0.2), nostrils, lambda s: R("boar snout", 5.2) if s.f == "+z" else None))
    for x in (2, 13):
        m.add("head", (x, 2, 13), (1, 5, 1), coat("tusk", ("tusk", x), base=4.2, top=1.5, streak=0.2))
    for x in (1, 12):
        m.add("head", (x, 12, 2), (3, 3, 2), coat("boar bristle", ("ear", x), base=1.6, streak=0.3))
    for leg in ("leg_fl", "leg_fr", "leg_bl", "leg_br"):
        hoof = lambda s: R("eye", 1) if s.p[1] < 1.5 and s.f != "top" else None
        m.add(leg, (0, 0, 0), (4, 8, 4), over(coat("boar bristle", leg, base=1.8, streak=0.6), hoof))
    m.add("tail", (1, 1, 2), (2, 2, 6), coat("boar bristle", "tail", base=1.6, streak=0.4))
    m.add("tail", (1, 0, 0), (2, 3, 2), coat("boar bristle", "tuft", base=0.8, streak=0.4))


def deer(m):
    tan = coat("deer coat", "deer", base=3.0, streak=0.6)
    pale_belly = lambda s: R("deer pale", 4.5 + s.up()) if s.f == "bottom" or (s.f in ("+x", "-x") and s.p[1] < 2.5) else None
    rump = lambda s: R("deer pale", 5 + (rnd("rump", int(s.p[0]), int(s.p[1])) - 0.5)) if s.f == "-z" and (s.p[0] - 8) ** 2 / 42 + (s.p[1] - 6) ** 2 / 30 < 1 else None
    m.add("body", (0, 0, 0), (16, 12, 32), over(tan, rump, pale_belly))
    throat = lambda s: R("deer pale", 4.6) if s.f == "+z" and 2 <= s.p[0] <= 6 else None
    m.add("neck", (0, 0, 0), (8, 12, 8), over(tan, throat))
    eyes = lambda s: (eye("+x", 5.5, 6.5, 0.8)(s) or eye("-x", 5.5, 6.5, 0.8)(s))
    nose = lambda s: R("eye", 1.2) if s.f in ("+z", "top") and s.p[2] > 10.5 and s.p[1] > 2 else None
    chin = lambda s: R("deer pale", 4.8) if s.f in ("+z", "bottom", "+x", "-x") and s.p[1] < 1.5 else None
    m.add("head", (0, 0, 0), (8, 8, 12), over(tan, eyes, nose, chin))
    m.add("head", (0, 8, 0), (8, 4, 5), tan)
    inner = lambda s: R("deer pale", 3.6) if s.f == "+z" else None
    for x in (-1, 7):
        m.add("head", (x, 10, 1), (2, 5, 1), over(coat("deer coat", ("ear", x), base=2.6, streak=0.2), inner))
    for leg in ("leg_fl", "leg_fr", "leg_bl", "leg_br"):
        lower = lambda s: R("deer coat", 1.6 + s.p[1] / 10) if s.p[1] < 9 and s.p[1] >= 1.5 and s.f != "top" else None
        hoof = lambda s: R("eye", 1) if s.p[1] < 1.5 else None
        m.add(leg, (0, 0, 0), (4, 20, 4), over(tan, hoof, lower))
    m.add("tail", (2, -1, 1), (4, 5, 2), over(coat("deer pale", "tail", base=5.0, streak=0.2), lambda s: R("deer coat", 3.6) if s.f in ("top", "-z") and s.p[1] > 2 else None))


def hare(m):
    fur = coat("hare fur", "hare", base=3.0, streak=0.8)
    belly = lambda s: R("hare pale", 4.6) if s.f == "bottom" or (s.f in ("+x", "-x", "+z") and s.p[1] < 2.5) else None
    m.add("body", (0, 0, 0), (8, 8, 12), over(fur, belly))
    eyes = lambda s: (eye("+x", 5.5, 5.5, 0.6)(s) or eye("-x", 5.5, 5.5, 0.6)(s))
    nose = lambda s: R("hare pink", 3.5) if s.f == "+z" and abs(s.p[0] - 4) < 1 and abs(s.p[1] - 3.5) < 0.6 else None
    muzzle = lambda s: R("hare pale", 5) if s.f == "+z" and s.p[1] < 3.5 and abs(s.p[0] - 4) < 2.5 else None
    m.add("head", (0, 0, 0), (8, 8, 8), over(fur, eyes, nose, muzzle))
    pink = lambda s: R("hare pink", 3.2 + 1.4 * s.up()) if s.f == "+z" and 0.8 < s.q[0] < s.size[0] - 0.8 and s.q[1] > 1 else None
    tip = lambda s: R("hare fur", 0.8) if s.q[1] > s.size[1] - 1.5 else None
    for x in (0.5, 4.5):
        m.add("ears", (x, 0, 1), (3, 11, 2), over(coat("hare fur", ("ear", x), base=3.2, streak=0.2), tip, pink))
    for leg in ("leg_fl", "leg_fr", "leg_bl", "leg_br"):
        m.add(leg, (0, 0, 0), (4, 4, 4), over(coat("hare fur", leg, base=2.4, streak=0.3), lambda s: R("hare pale", 4) if s.p[1] < 1 else None))
    m.add("tail", (2, 0, 1), (4, 4, 3), coat("hare pale", "tail", base=5.2, top=1.2, streak=0.3))


def snake(m):
    # each part's place along the body at rest (pixels from the tail's end), so the diamonds run on across parts
    along = {"head": 24, "neck": 16, "middle": 8, "tail": 0}

    def scales(part):
        def p(s):
            z = s.p[2] + along[part]
            n = (rnd("snake", part, s.f, int(s.p[0]), int(s.p[1]), int(s.p[2])) - 0.5) * 0.6
            if s.f == "bottom" or (s.f in ("+x", "-x") and s.p[1] < 1.5):
                return R("snake belly", 4.2 + ((int(z) % 2) * 0.6) + n)
            d = abs(s.p[0] - 4) + abs((z % 8) - 4) if s.f == "top" else 4 + abs((z % 8) - 4) - (s.p[1] - 1)
            if d < 2.2:
                return R("snake scale", 0.6 + n)  # the diamond's dark heart
            if d < 3.2:
                return R("snake belly", 5.4 + n)  # its pale rim
            return R("snake scale", (4.4 if s.f == "top" else 2.4 + 2 * s.up()) + n)
        return p
    eyes = lambda s: (eye("+x", 6.5, 5.5, 0.6)(s) or eye("-x", 6.5, 5.5, 0.6)(s))
    mouth = lambda s: R("snake scale", 0.4) if s.f == "+z" and abs(s.p[1] - 2.5) < 0.5 else None
    m.add("head", (0, 0, 0), (8, 7, 8), over(scales("head"), eyes, mouth))
    m.add("neck", (0.5, 0, 0), (7, 7, 8), scales("neck"))
    m.add("middle", (0.5, 0, 0), (7, 6, 8), scales("middle"))
    m.add("tail", (1, 0, 4), (6, 5, 4), scales("tail"))
    m.add("tail", (2, 0, 0), (4, 3, 4), scales("tail"))


def furnace(m):
    def stone(seed, soot=False):
        """Stone blocks in courses 6 px high, 10 long, offset a half each course, with lighter mortar; soot over the
        mouth on the front; the top lit."""
        def p(s):
            x, y, z = s.p
            along = x if s.f in ("+z", "-z", "top", "bottom") else z
            if s.f in ("top", "bottom"):
                row, col = int(z) // 6, (int(x) + (int(z) // 6) * 5) % 10
                edge = int(z) % 6 == 0 or col == 0
            else:
                row, col = int(y) // 6, (int(along) + (int(y) // 6) * 5) % 10
                edge = int(y) % 6 == 0 or col == 0
            n = (rnd(seed, s.f, int(x), int(y), int(z)) - 0.5) * 0.8 + (rnd(seed, "b", row, (int(along) + row * 5) // 10, s.f) - 0.5) * 1.4
            t = (5.0 if s.f == "top" else 1.2 if s.f == "bottom" else 2.4 + 1.6 * y / 20) + n
            if edge:
                t = 5.4 + n * 0.3
            if soot and s.f in ("-z",) and 6 <= x <= 26 and y >= 10:
                t -= 2.6 * max(0.0, 1 - abs(x - 16) / 11) * (1.2 - (y - 10) / 12)
            return R("furnace stone", t)
        return p
    def mouth(s):
        # the mouth's back, seen through the open door: embers low, glow above, soot black at its rim
        x, y = s.p[0], s.p[1]
        if s.f == "-z" and 8 <= x <= 24 and 4 <= y <= 16:
            if y < 8:
                return R("ember", 5 + (rnd("ember", int(x), int(y)) - 0.5) * 3)
            return R("ember", 3.2 - (y - 8) / 4 + (rnd("glow", int(x), int(y)) - 0.5))
        return None
    inner = lambda s: R("furnace stone", 0.3) if (s.f == "+x" and 7 <= s.p[0] <= 8 or s.f == "-x" and 24 <= s.p[0] <= 25 or s.f == "bottom" and s.p[1] >= 15 or s.f == "top" and s.p[1] <= 5) and s.p[2] < 4.5 else None
    m.add("body", (0, 0, 4), (32, 20, 28), over(stone("back"), mouth))
    m.add("body", (0, 0, 0), (32, 4, 4), over(stone("sill"), inner))
    m.add("body", (0, 16, 0), (32, 4, 4), over(stone("lintel", True), inner))
    m.add("body", (0, 4, 0), (8, 12, 4), over(stone("left", True), inner))
    m.add("body", (24, 4, 0), (8, 12, 4), over(stone("right", True), inner))
    m.add("body", (20, 20, 20), (8, 6, 8), over(coat("door copper", "chimney", base=2.6, streak=0.4), lambda s: R("eye", 0.3) if s.f == "top" and 1 < s.q[0] < 7 and 1 < s.q[2] < 7 else None))
    def door(s):
        x, y = s.q[0], s.q[1]
        if s.f == "-z" and 4 <= x <= 12 and 5 <= y <= 7:
            return R("eye", 0.6)  # the slot
        if s.f == "-z" and (x < 1.5 or x > 14.5 or y < 1.5 or y > 10.5):
            return R("door copper", 3.0 + (rnd("rivet", int(x), int(y)) < 0.25) * 2.5)
        return None
    m.add("door", (0, 0, 0), (16, 12, 4), over(coat("door copper", "door", base=3.6, streak=0.5), door))


def leaf(m):
    """The `leaves` effect's leaf (data/effects.toml): a flat broadleaf with a vein, lit toward its tip."""
    def p(s):
        x, z = s.p[0], s.p[2]
        dx, dz = (x - 6) / 6, (z - 4) / 4
        if dx * dx + dz * dz > 1:
            return None
        if abs(dz) < 0.2:
            return R("broadleaf leaves", 2.4)
        return R("broadleaf leaves", 3.6 + 1.8 * dx + (rnd("leaf", int(x), int(z)) - 0.5))
    m.add("leaf", (0, 2, 0), (12, 0, 8), p)


def make():
    ramps()
    written, total = [], 0
    for name, recipe in [("ape", ape), ("boar", boar), ("deer", deer), ("hare", hare), ("snake", snake), ("furnace", furnace), ("leaf", leaf)]:
        m = Figure(name)
        recipe(m)
        written += m.write()
        total += m.quads
    print(f"all figures: {total} quads")
    return written


if __name__ == "__main__":
    files = make()
    drafts = DATA / "drafts.txt"
    have = drafts.read_text(encoding="utf-8").splitlines()
    drafts.write_text("\n".join(have + [f for f in files if f not in have]) + "\n", encoding="utf-8")
