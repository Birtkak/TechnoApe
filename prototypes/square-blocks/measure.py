# How square are the blocks, and how big, on the equi-angular cube-sphere (ADR 0003, w = 0), on the conformal one
# (w = 1) and on blends between them? The game's standard planet: N = 6144 columns a face edge, 1 m blocks.
#   python -I measure.py            -> results.json, images/sheet_blocks.png, images/sheet_maps.png
#   python -I measure.py table      -> conformal_face.npy again from ../sphere-grid/out/harmonic.tab (smooth.exe)
# The conformal grid is the old spike's table (prototypes/sphere-grid/smooth.cpp, K = 192 lattice a face edge),
# face 0 only, bilinear between its points as in warp.h.
# shortcut: one table cell is 32 blocks, so nothing closer than about 30 m to a cube corner is resolved;
# the size there is read off the r^(1/3) law fitted further out. A finer table before any engine work.
import json, os, sys
import numpy as np
from PIL import Image, ImageDraw

HERE = os.path.dirname(os.path.abspath(__file__))
N = 6144
R = N / np.sqrt(2 * np.pi / 3)  # 4245 m: the mean block top is 1 m2
WS = [0.0, 0.25, 0.5, 0.75, 1.0]

if len(sys.argv) > 1 and sys.argv[1] == "table":
    raw = np.fromfile(os.path.join(HERE, "../sphere-grid/out/harmonic.tab"), dtype=np.float64, offset=4)
    k1 = int(round(np.sqrt(raw.size / 18)))
    np.save(os.path.join(HERE, "conformal_face.npy"), raw.reshape(6, k1, k1, 3)[0])
    sys.exit(0)

D = np.load(os.path.join(HERE, "conformal_face.npy"))  # [ia][ib] the move from equi-angular to conformal
K = D.shape[0] - 1


def unit(p):
    return p / np.linalg.norm(p, axis=-1, keepdims=True)


def grid(a, b, w):
    """A point of the face, a and b in [-1, 1] across it, on the unit sphere."""
    a, b = np.asarray(a, float), np.asarray(b, float)
    p = unit(np.stack([np.ones_like(a), np.tan(a * np.pi / 4), np.tan(b * np.pi / 4)], -1))
    x, y = (a + 1) * 0.5 * K, (b + 1) * 0.5 * K
    i, j = np.clip(x.astype(int), 0, K - 1), np.clip(y.astype(int), 0, K - 1)
    fx, fy = (x - i)[..., None], (y - j)[..., None]
    d = D[i, j] * (1 - fx) * (1 - fy) + D[i + 1, j] * fx * (1 - fy) + D[i, j + 1] * (1 - fx) * fy + D[i + 1, j + 1] * fx * fy
    return unit(p + w * d)


def blocks(a, b, w):
    """Per block with its low corner at (a, b): the two top edges in metres in the tangent plane (2x2 each)."""
    o = grid(a, b, w)
    ei = (grid(a + 2 / N, b, w) - o) * R
    ej = (grid(a, b + 2 / N, w) - o) * R
    return o, ei, ej


def shape(ei, ej):
    """Edge lengths, the corner's angle off 90 degrees, the top's stretch (1 a square) and its size (sqrt of area)."""
    li, lj = np.linalg.norm(ei, axis=-1), np.linalg.norm(ej, axis=-1)
    dot = (ei * ej).sum(-1)
    skew = np.abs(np.degrees(np.arccos(np.clip(dot / (li * lj), -1, 1))) - 90)
    area = np.sqrt(np.maximum((li * lj) ** 2 - dot**2, 1e-30))
    # singular values of the 2x2 map from a unit square to the block's top
    s = li**2 + lj**2
    big = np.sqrt((s + np.sqrt(np.maximum(s**2 - 4 * area**2, 0))) / 2)
    small = area / big
    return li, lj, skew, big, small, np.sqrt(area)


# the check: the equi-angular numbers known from the maths (ADR 0003 scaled to 1 m blocks, and the in-game shots)
_, ei, ej = blocks(np.array([0.0, 1 - 2 / N, 1 - 2 / N]), np.array([0.0, 0.0, 1 - 2 / N]), 0.0)
li, lj, skew, big, small, size = shape(ei, ej)
assert abs(li[0] - 1.085) < 0.002 and abs(lj[0] - 1.085) < 0.002, (li[0], lj[0])  # face centre
assert abs(lj[1] - 0.767) < 0.002 and abs(big[1] / small[1] - 1.414) < 0.003, (lj[1], big[1] / small[1])  # edge middle
assert abs(skew[2] - 30) < 0.1, skew[2]  # the corner's diamond

# every 8th block of a face (the faces are alike)
c = (2 * np.arange(4, N, 8) - N) / N
A, B = np.meshgrid(c, c, indexing="ij")
corner = unit(np.array([1.0, 1.0, 1.0]))
results, maps = {"N": N, "R_m": round(R, 1), "table_K": K, "grids": {}}, {}
for w in WS:
    o, ei, ej = blocks(A, B, w)
    li, lj, skew, big, small, size = shape(ei, ej)
    top = big / small  # the top face: 1 a square, 1.41 the equi-angular edge, 1.73 its corner diamond
    box = np.maximum(big, 1.0) / np.minimum(small, 1.0)  # the whole block, its height 1 m
    wt = size**2 / (size**2).sum()  # by ground covered
    # metres to the nearest cube corner, along the ground
    oc = np.abs(o)
    far = np.arccos(np.clip(oc @ corner, -1, 1)) * R
    share = lambda m: round(float(wt[m].sum()) * 100, 1)
    # the size law near a corner, fitted on the diagonal between 60 and 300 m out
    dg = np.arange(len(c))
    fd, sd = far[dg, dg], size[dg, dg]
    fit = (fd > 60) & (fd < 300) & (dg < len(c) // 2)
    slope, icpt = np.polyfit(np.log(fd[fit]), np.log(sd[fit]), 1)
    reach = lambda m: round(float(np.exp((np.log(m) - icpt) / slope)), 1) if slope > 0.05 else 0.0
    results["grids"][f"w{w:.2f}"] = {
        "block_size_m": {"face_centre": round(float(size[len(c) // 2, len(c) // 2]), 3),
                         "edge_middle": round(float(size[-1, len(c) // 2]), 3),
                         "smallest_beyond_30m_of_a_corner": round(float(size[far > 30].min()), 3),
                         "largest": round(float(size.max()), 3)},
        "top_stretch": {"worst": round(float(top.max()), 2), "pct_over_1.1": share(top > 1.1), "pct_over_1.25": share(top > 1.25)},
        "skew_deg": {"worst": round(float(skew.max()), 1), "pct_over_5": share(skew > 5), "pct_over_15": share(skew > 15)},
        "block_stretch_with_1m_height": {"worst_beyond_30m": round(float(box[far > 30].max()), 2), "pct_over_1.1": share(box > 1.1),
                                         "pct_over_1.25": share(box > 1.25), "pct_over_1.5": share(box > 1.5)},
        "size_pct": {"under_0.9m": share(size < 0.9), "under_0.75m": share(size < 0.75), "under_0.5m": share(size < 0.5)},
        "corner_law": {"size_exponent": round(float(slope), 3),
                       "m_from_corner_where_block_is": {"0.75m": reach(0.75), "0.5m": reach(0.5), "0.25m": reach(0.25)}},
    }
    maps[w] = (top, box, size)
json.dump(results, open(os.path.join(HERE, "results.json"), "w"), indent=1)
print(json.dumps(results))


# ---- pictures
def ramp(v, lo, hi):
    """lo green, half way yellow, hi and over red."""
    t = np.clip((v - lo) / (hi - lo), 0, 1)[..., None]
    g, y, r = np.array([60, 160, 80.0]), np.array([235, 215, 70.0]), np.array([210, 50, 45.0])
    return np.where(t < 0.5, g + (y - g) * t * 2, y + (r - y) * (t * 2 - 1)).astype(np.uint8)


os.makedirs(os.path.join(HERE, "images"), exist_ok=True)
S, PAD, TOP = 300, 10, 34
rows = [("top face stretch: green square, red 1.5 to 1", lambda m: ramp(m[0], 1.0, 1.5)),
        ("whole block stretch, height 1 m: green a cube, red 1.5 to 1", lambda m: ramp(m[1], 1.0, 1.5)),
        ("block size: green 1.09 m, red 0.5 m and under", lambda m: ramp(1.09 - m[2], 0.0, 0.59))]
names = {0.0: "equi-angular (now)", 0.25: "blend 0.25", 0.5: "blend 0.5", 0.75: "blend 0.75", 1.0: "conformal"}
sheet = Image.new("RGB", (PAD + len(WS) * (S + PAD), len(rows) * (S + TOP) + PAD), (24, 24, 28))
dr = ImageDraw.Draw(sheet)
for r, (title, f) in enumerate(rows):
    for k, w in enumerate(WS):
        x, y = PAD + k * (S + PAD), r * (S + TOP) + TOP
        sheet.paste(Image.fromarray(f(maps[w])).resize((S, S), Image.BILINEAR), (x, y))
        dr.text((x, y - 26), names[w], fill=(255, 255, 255))
        dr.text((x, y - 13), title, fill=(170, 170, 170))
sheet.save(os.path.join(HERE, "images", "sheet_maps.png"))

# checkerboards from straight above, all at one scale: 13 x 13 blocks, a white 1 m square for size
PX, HALF, CELL = 20, 6, 300
spots = [("face centre", 0.0, 0.0), ("3/4 of the way to an edge", 0.75, 0.0), ("face edge", 1 - 14 / N, 0.0),
         ("300 m from a corner", 300, None), ("60 m from a corner", 60, None)]
cols = [0.0, 0.5, 1.0]


def diagonal(metres, w):
    """a = b on the face's diagonal that many real metres from the corner on this grid."""
    aa = np.linspace(0.5, 1, 40001)
    return float(aa[np.argmin(np.abs(np.arccos(np.clip(grid(aa, aa, w) @ corner, -1, 1)) * R - metres))])
sheet = Image.new("RGB", (PAD + len(cols) * (CELL + PAD), len(spots) * (CELL + TOP) + PAD), (24, 24, 28))
dr = ImageDraw.Draw(sheet)
for r, (label, sa, sb) in enumerate(spots):
    for k, w in enumerate(cols):
        a0, b0 = (sa, sb) if sb is not None else (diagonal(sa, w),) * 2
        ii = np.arange(-HALF, HALF + 2)
        a, b = np.meshgrid(a0 + 2 * ii / N, b0 + 2 * ii / N, indexing="ij")
        p = grid(a, b, w)
        mid = unit(p[HALF:HALF + 2, HALF:HALF + 2].mean((0, 1)))
        # the tangent plane's axes: u along the grid's first axis at the middle, v square to it
        u = p[HALF + 1, HALF] - p[HALF, HALF]
        u = unit(u - mid * (u @ mid))
        v = np.cross(mid, u)
        xy = np.stack([(p - mid) @ u, (p - mid) @ v], -1) * R * PX
        x0, y0 = PAD + k * (CELL + PAD) + CELL // 2, r * (CELL + TOP) + TOP + CELL // 2
        tile = Image.new("RGB", (CELL, CELL), (16, 30, 44))
        td = ImageDraw.Draw(tile)
        for i in range(2 * HALF + 1):
            for j in range(2 * HALF + 1):
                quad = [xy[i, j], xy[i + 1, j], xy[i + 1, j + 1], xy[i, j + 1]]
                td.polygon([(CELL / 2 + q[0], CELL / 2 - q[1]) for q in quad],
                           fill=(120, 140, 190) if (i + j) % 2 else (205, 205, 150), outline=(40, 40, 40))
        td.rectangle([6, CELL - 6 - PX, 6 + PX, CELL - 6], outline=(255, 255, 255))
        sheet.paste(tile, (x0 - CELL // 2, y0 - CELL // 2))
        _, ei, ej = blocks(np.array(a0), np.array(b0), w)
        li, lj, skew, big, small, size = shape(ei, ej)
        dr.text((x0 - CELL // 2, y0 - CELL // 2 - 26), f"{names[w]}: {label}", fill=(255, 255, 255))
        dr.text((x0 - CELL // 2, y0 - CELL // 2 - 13), f"{li:.2f} x {lj:.2f} m, corner {90 - skew:.0f} deg", fill=(170, 170, 170))
sheet.save(os.path.join(HERE, "images", "sheet_blocks.png"))
