"""Images + the Cartesian-grid numbers for p16. Run after measure.exe (reads out/*.cells, out/grid_results.json).
Writes images/*.png and results.json. Needs numpy + Pillow.  python -I render.py"""
import json
import math
import os

import numpy as np
from PIL import Image, ImageDraw, ImageFont

HERE = os.path.dirname(os.path.abspath(__file__))
OUT, IMG = os.path.join(HERE, "out"), os.path.join(HERE, "images")
os.makedirs(IMG, exist_ok=True)
R = 300.0
BLOCK = 0.5
SS = 2  # supersampling
FONT = ImageFont.load_default()


def basis(d):
    d = np.asarray(d, float); d /= np.linalg.norm(d)
    ref = np.array([0, 0, 1.0]) if abs(d[2]) < 0.9 else np.array([0, 1.0, 0])
    u = np.cross(ref, d); u /= np.linalg.norm(u)
    return d, u, np.cross(d, u)


def color(v):
    """Cell area / mean area: blue = small, white = mean, red = big (log2 clamped to +-1)."""
    t = max(-1.0, min(1.0, math.log2(max(v, 1e-6)) / 0.6))
    if t < 0:
        return (int(255 * (1 + t * 0.75)), int(255 * (1 + t * 0.45)), 255)
    return (255, int(255 * (1 - t * 0.6)), int(255 * (1 - t * 0.75)))


def load(name):
    cells = []
    with open(os.path.join(OUT, name)) as f:
        for line in f:
            x = list(map(float, line.split()))
            cells.append((x[0], np.array(x[1:]).reshape(-1, 3)))
    return cells


def render(cells, view, path, title, size=700, radius_m=None):
    d, u, v = basis(view)
    W = size * SS
    img = Image.new("RGB", (W, W), (24, 26, 32))
    dr = ImageDraw.Draw(img)
    if radius_m is None:  # orbit: the whole unit sphere
        scale, cx, cy = W * 0.46, 0.0, 0.0
    else:
        scale, cx, cy = W / 2 / (radius_m / R), 0.0, 0.0
    centre = d if radius_m else np.zeros(3)
    for val, poly in cells:
        if np.dot(poly.mean(0), d) <= 0.02:
            continue
        rel = poly - centre
        pts = [(W / 2 + scale * (np.dot(p, u) - cx), W / 2 - scale * (np.dot(p, v) - cy)) for p in rel]
        dr.polygon(pts, fill=color(val), outline=(30, 30, 40), width=max(1, SS))
    img = img.resize((size, size), Image.LANCZOS)
    dr = ImageDraw.Draw(img)
    dr.rectangle([0, 0, size, 18], fill=(0, 0, 0))
    dr.text((6, 3), title, fill=(255, 255, 255), font=FONT)
    if radius_m:  # 1 m scale bar
        px = size / 2 / radius_m
        dr.rectangle([10, size - 16, 10 + px, size - 10], fill=(255, 255, 255))
        dr.text((14 + px, size - 20), "1 m", fill=(255, 255, 255), font=FONT)
    img.save(os.path.join(IMG, path))


def sheet(names, path, cols):
    ims = [Image.open(os.path.join(IMG, n)) for n in names]
    w, h = ims[0].size
    rows = (len(ims) + cols - 1) // cols
    s = Image.new("RGB", (w * cols, h * rows), (0, 0, 0))
    for k, im in enumerate(ims):
        s.paste(im, ((k % cols) * w, (k // cols) * h))
    s.save(os.path.join(IMG, path))


# ------------------------------------------------ (a) hex and (b) cube-sphere views
patch_dir = np.array([1, 0.35, 0.2]) / np.linalg.norm([1, 0.35, 0.2])
phi = (1 + 5 ** 0.5) / 2
ico = [np.array(p, float) / np.linalg.norm(p) for p in
       [(0, -1, -phi), (-1, -phi, 0), (-phi, 0, -1)]]  # first three icosahedron vertices in grid.h order
views = [
    ("cube_patch.cells", patch_dir, "cube_patch.png", "(b) cube-sphere equi-angular, R=300 m: typical surface (18 m across)"),
    ("cube_seam.cells", np.array([1, 1, 0.0]), "cube_seam.png", "(b) cube-sphere: face seam (edge middle)"),
    ("cube_corner.cells", np.array([1, 1, 1.0]), "cube_corner.png", "(b) cube-sphere: corner (3 faces, 3 cells per vertex)"),
]
for f, d, p, t in views:
    render(load(f), d, p, t, radius_m=9)
for f, p, t in [("cube_orbit_gnomonic.cells", "cube_orbit_gnomonic.png", "(b) gnomonic, N=12, from orbit"),
                ("cube_orbit_equiangular.cells", "cube_orbit_equiangular.png", "(b) equi-angular, N=12, from orbit"),
                ("cube_orbit_nowicki.cells", "cube_orbit_nowicki.png", "(b) Nowicki, N=12, from orbit")]:
    render(load(f), [1, 0.8, 0.6], p, t)
# Hex views: the dumps were centred with the same formulas in measure.cpp, recompute the centres from the cells.
for f, p, t in [("hex_patch.cells", "hex_patch.png", "(a) geodesic hex, R=300 m: typical surface (18 m across)"),
                ("hex_seam.cells", "hex_seam.png", "(a) geodesic hex: icosahedron edge"),
                ("hex_pentagon.cells", "hex_pentagon.png", "(a) geodesic hex: pentagon (12 per planet)")]:
    cells = load(f)
    c = sum(poly.mean(0) for _, poly in cells); c /= np.linalg.norm(c)
    render(cells, c, p, t, radius_m=9)
hex_orbit = load("hex_orbit.cells")
pent = next(poly.mean(0) for val, poly in hex_orbit if len(poly) == 5)
render(hex_orbit, pent + np.array([0.3, 0.2, 0.1]), "hex_orbit.png", "(a) geodesic hex, F=8, from orbit (pentagon near centre)")


# ------------------------------------------------ (c) rounded cube and (d) Cartesian + radial gravity
def tilt_deg(n):
    """Angle between the local up (surface normal / radial) and the nearest grid axis."""
    n = n / np.linalg.norm(n, axis=-1, keepdims=True)
    return np.degrees(np.arccos(np.clip(np.abs(n).max(-1), -1, 1)))


rng = np.random.default_rng(1)
sph = rng.normal(size=(2_000_000, 3))
t_sphere = tilt_deg(sph)


def tilt_summary(t, w=None):
    w = np.ones_like(t) if w is None else w
    w = w / w.sum()
    return {"mean_deg": round(float((t * w).sum()), 2), "max_deg": round(float(t.max()), 2),
            **{f"frac_gt{x}": round(float(w[t > x].sum()), 4) for x in (2, 5, 15, 30)}}


cart = {"tilt_sphere": tilt_summary(t_sphere)}
rounded = {}
for k in (0.25, 0.5, 0.75):  # rounding radius as a fraction of the half-size
    rc, a = k, 1 - k
    A_face, A_edge, A_corner = 6 * (2 * a) ** 2, 12 * 2 * a * math.pi / 2 * rc, 4 * math.pi * rc ** 2
    phi_e = rng.uniform(0, math.pi / 2, 1_000_000)  # area-uniform across a quarter cylinder
    t_edge = np.degrees(np.minimum(phi_e, math.pi / 2 - phi_e))
    t_all = np.concatenate([np.zeros(1000), t_edge, t_sphere[:1_000_000]])
    w = np.concatenate([np.full(1000, A_face / 1000), np.full(t_edge.size, A_edge / t_edge.size),
                        np.full(1_000_000, A_corner / 1_000_000)])
    rounded[f"k{k}"] = {"flat_area_frac": round(A_face / (A_face + A_edge + A_corner), 4),
                        "radius_variation": round((math.sqrt(3) * a + rc) / (a + rc), 4),
                        "tilt": tilt_summary(t_all, w)}


def tilt_image(path, title, k=None, size=700):
    """Orthographic view coloured by tilt (0 = blocks level with gravity, 54.7 = worst)."""
    W = size
    d, u, v = basis([1, 0.8, 0.6])
    ys, xs = np.mgrid[0:W, 0:W]
    px, py = (xs - W / 2) / (W * 0.4), -(ys - W / 2) / (W * 0.4)
    o = px[..., None] * u + py[..., None] * v + 3 * d  # ray origins, marching along -d
    if k is None:
        sdf = lambda p: np.linalg.norm(p, axis=-1) - 1
    else:
        a, rc = 1 - k, k
        sdf = lambda p: np.linalg.norm(np.maximum(np.abs(p) - a, 0), axis=-1) - rc
    t = np.zeros((W, W))
    for _ in range(80):
        t += sdf(o - t[..., None] * d)
    p = o - t[..., None] * d
    hit = np.abs(sdf(p)) < 1e-3
    n = p if k is None else p - np.clip(p, -(1 - k), 1 - k)
    tl = tilt_deg(n + 1e-12)
    c = np.clip(tl / 54.7, 0, 1)[..., None]
    rgb = (1 - c) * np.array([60, 170, 90]) + c * np.array([230, 60, 50])
    shade = np.clip(np.einsum("ijk,k->ij", n / (np.linalg.norm(n, axis=-1, keepdims=True) + 1e-12), d), 0.25, 1)[..., None]
    img = np.where(hit[..., None], rgb * shade, np.array([24, 26, 32])).astype(np.uint8)
    im = Image.fromarray(img)
    dr = ImageDraw.Draw(im)
    dr.rectangle([0, 0, size, 18], fill=(0, 0, 0))
    dr.text((6, 3), title, fill=(255, 255, 255), font=FONT)
    dr.text((6, size - 16), "green = blocks level with gravity, red = 54.7 deg off", fill=(255, 255, 255), font=FONT)
    im.save(os.path.join(IMG, path))


def section_image(path, title, k=None, size=700, span=24.0):
    """Cut through z = 0, 0.5 m blocks, around the worst place for the grid (45 deg / the rounded edge)."""
    W = size
    if k is None:
        centre = np.array([R, R]) / math.sqrt(2)
        inside = lambda x, y: np.hypot(x, y) <= R
        up = centre / np.linalg.norm(centre)
    else:
        a, rc = (1 - k) * R, k * R
        up = np.array([math.cos(math.radians(10)), math.sin(math.radians(10))])  # 10 deg into the rounding
        centre = np.array([a, a]) + rc * up
        inside = lambda x, y: np.hypot(np.maximum(np.abs(x) - a, 0), np.maximum(np.abs(y) - a, 0)) <= rc
    img = Image.new("RGB", (W, W), (150, 190, 230))
    dr = ImageDraw.Draw(img)
    s = W / span
    x0, y0 = centre - span / 2
    for i in range(int(span / BLOCK) + 2):
        for j in range(int(span / BLOCK) + 2):
            bx = math.floor(x0 / BLOCK) * BLOCK + i * BLOCK
            by = math.floor(y0 / BLOCK) * BLOCK + j * BLOCK
            if inside(bx + BLOCK / 2, by + BLOCK / 2):
                X, Y = (bx - x0) * s, W - (by - y0) * s
                dr.rectangle([X, Y - BLOCK * s, X + BLOCK * s, Y], fill=(110, 160, 70), outline=(60, 90, 40))
    cx, cy = W / 2, W / 2
    dr.line([cx, cy, cx + up[0] * 120, cy - up[1] * 120], fill=(200, 20, 20), width=4)
    dr.text((cx + up[0] * 125, cy - up[1] * 125), "up (gravity)", fill=(150, 0, 0), font=FONT)
    tang = np.array([-up[1], up[0]])  # a level floor, as gravity sees it
    dr.line([cx - tang[0] * W, cy + tang[1] * W, cx + tang[0] * W, cy - tang[1] * W], fill=(250, 250, 250), width=2)
    dr.rectangle([0, 0, W, 18], fill=(0, 0, 0))
    dr.text((6, 3), title, fill=(255, 255, 255), font=FONT)
    dr.text((6, W - 16), "white = a level floor for the player; green = 0.5 m blocks; 24 m across", fill=(0, 0, 0), font=FONT)
    img.save(os.path.join(IMG, path))


tilt_image("cartesian_orbit.png", "(d) Cartesian grid + radial gravity: block tilt vs up")
section_image("cartesian_section.png", "(d) Cartesian grid at 45 deg latitude, R=300 m")
tilt_image("rounded_orbit.png", "(c) rounded cube k=0.5 (6 gravity faces): block tilt vs up", k=0.5)
section_image("rounded_section.png", "(c) rounded cube k=0.5: 10 deg into a rounded edge (flat faces are perfectly level)", k=0.5)

sheet(["hex_orbit.png", "cube_orbit_equiangular.png", "rounded_orbit.png", "cartesian_orbit.png"], "sheet_orbit.png", 2)
sheet(["hex_patch.png", "hex_seam.png", "hex_pentagon.png", "cube_patch.png", "cube_seam.png", "cube_corner.png"],
      "sheet_surface.png", 3)
sheet(["cube_orbit_gnomonic.png", "cube_orbit_equiangular.png", "cube_orbit_nowicki.png"], "sheet_projections.png", 3)
sheet(["cartesian_section.png", "rounded_section.png"], "sheet_sections.png", 2)

# ------------------------------------------------ radial layering of (a)/(b): cells grow with radius
layering = {}
for Rp in (300, 1000):
    lo, hi = Rp - 64, Rp + 30  # crust bottom .. terrain top (ADR 0170 benchmark)
    layering[f"R{Rp}"] = {"width_at_crust_bottom_m": round(BLOCK * lo / Rp, 3), "width_at_terrain_top_m": round(BLOCK * hi / Rp, 3),
                          "ratio_over_band": round(hi / lo, 3),
                          "layers_over_band_geometric": math.ceil(math.log(hi / lo) / math.log(1 + BLOCK / Rp)),
                          "layers_over_band_uniform": int((hi - lo) / BLOCK)}

res = json.load(open(os.path.join(OUT, "grid_results.json")))
res["cartesian_radial_gravity"] = cart
res["rounded_cube"] = {"note": "k = rounding radius / half-size; 0 = cube, 1 = sphere", **rounded}
res["radial_layering"] = layering
json.dump(res, open(os.path.join(HERE, "results.json"), "w"), indent=1)
print(json.dumps({"cartesian": cart, "rounded": rounded, "layering": layering}, indent=1))
