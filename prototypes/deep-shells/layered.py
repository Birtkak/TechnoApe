"""Variant L (layered, no visible seam): tables and pictures. `python -I layered.py`.
Rewrites the block between <!-- L tables --> markers in README.md and writes out/layered-*.png."""
import math, os, re, sys
import numpy as np
from PIL import Image, ImageDraw

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import shells as S
import pictures as P

N = 6144
R = S.radius(N)
RI = 0.19 * R
RCS = [("0.546 R (Earth)", 0.546 * R), ("R/sqrt2 (large core)", R / math.sqrt(2))]
LIQ = np.array([255, 175, 40], np.uint8)
CORE = ((176, 176, 188), (118, 118, 132))
ISTEP = 1 / R  # angular width of a mantle column (mean size r/R)


# ---------------------------------------------------------------- tables
def mantle_rows():
    rows = ["| Rc | Rc (m) | block at the bottom (m) | layers surface to Rc | cells | blocks a m3 at the bottom | depth of the mantle (m) |", "|---|---|---|---|---|---|---|"]
    for name, rc in RCS:
        L = math.ceil(math.log(R / rc) / math.log(1 + 1 / R))
        rows.append(f"| {name} | {rc:,.0f} | {rc / R:.3f} | {L:,} | {6 * N * N * L:.3g} | {(R / rc) ** 3:.2f} | {R - rc:,.0f} |")
    return rows


def hidden(rc):
    """Variant A's halving shells started at Rc, cut at Ri (they carry the liquid's columns)."""
    out, k, top = [], 0, rc
    while top > RI:
        bot = max(top / 2, RI)
        n, q = N // 2 ** k, 2 ** k / R
        L = math.ceil(math.log(top / bot) / math.log(1 + q))
        out.append((k, top, bot, n, q * top, q * bot, L, 6 * n * n * L))
        k, top = k + 1, top / 2
    return out


def hidden_rows():
    rows = ["| Rc | hidden shell | top r (m) | bottom r (m) | columns a face edge | chunks | block width top / bottom (m) | layers | cells |", "|---|---|---|---|---|---|---|---|---|"]
    for name, rc in RCS:
        for k, top, bot, n, wt, wb, L, c in hidden(rc):
            rows.append(f"| {name} | {k} | {top:,.0f} | {bot:,.0f} | {n} | {n // 32} | {wt:.3f} / {wb:.3f} | {L:,} | {c:.3g} |")
    return rows


def gravity_rows():
    rows = ["| where | r (m) | g as a fraction of the surface's |", "|---|---|---|"]
    for name, r in [("surface", R)] + [(f"Rc {n}", rc) for n, rc in RCS] + [("Ri = 0.19 R", RI), ("400 m", 400), ("100 m", 100), ("10 m", 10)]:
        rows.append(f"| {name} | {r:,.0f} | {r / R:.4f} |")
    return rows


def stairs(theta_deg, span=40.0, du=0.05, dw=0.0005):
    """The top of a ball of 1 m cubes (centre within Ri) in the plane through the centre and two grid axes,
    seen from a point whose normal is theta from an axis. Returns the profile h(u): height above the tangent
    plane at the sample point, u along the tangent."""
    th = math.radians(theta_deg)
    d, t = np.array([math.cos(th), math.sin(th)]), np.array([-math.sin(th), math.cos(th)])
    pt = RI * d
    u = np.arange(-span, span, du)
    w = np.arange(4.0, -4.0, -dw)
    h = np.full(len(u), np.nan)
    for i0 in range(0, len(u), 100):
        uu = u[i0:i0 + 100]
        xy = pt[None, None, :] + uu[:, None, None] * t[None, None, :] + w[None, :, None] * d[None, None, :]
        ix, iz = np.floor(xy[..., 0]), np.floor(xy[..., 1])
        solid = (ix + .5) ** 2 + (iz + .5) ** 2 <= RI ** 2
        first = np.argmax(solid, axis=1)
        h[i0:i0 + 100] = np.where(solid.any(axis=1), w[first], np.nan)
    return u, h


def stair_rows():
    rows = ["| angle from a grid axis | largest step (m, peak to peak of the sawtooth along down, 2 m window) | mean step (m) | floor tilt, mean (deg from level) | share of the walk tilted over 45 deg | floor tilts that cover over 5 % of the walk (deg) | rms about the mean line (m) |", "|---|---|---|---|---|---|---|"]
    for th in (0, 15, 30, 45):
        u, h = stairs(th)
        du = u[1] - u[0]
        res = h - np.polyval(np.polyfit(u, h, 2), u)  # the ball's own curve removed
        win = np.lib.stride_tricks.sliding_window_view(res, int(2 / du))[::5]
        p2p = win.max(axis=1) - win.min(axis=1)
        dh = np.diff(h)
        ang = np.degrees(np.arctan(np.abs(dh[np.abs(dh) < 6 * du]) / du))
        modes = [t for t in range(0, 91) if (ang.round() == t).mean() > 0.05]
        rows.append(f"| {th} | {p2p.max():.2f} | {p2p.mean():.2f} | {ang.mean():.1f} | {(ang > 45.5).mean() * 100:.0f} % | {', '.join(map(str, modes))} | {np.sqrt(np.mean(res ** 2)):.2f} |")
    return rows


def build():
    out = ["### Variant L tables (`python -I layered.py`)", "", f"Standard planet, n = {N}, R = {R:,.1f} m, Ri = 0.19 R = {RI:,.1f} m.", "",
           "**Crust and mantle: one grid, no seam** (block at r is r/R m wide and thick):", ""] + mantle_rows()
    out += ["", "**Hidden shells under the liquid** (variant A's halving, started at Rc, stopped at Ri; seams at Rc/2, ...):", ""] + hidden_rows()
    out += ["", "**Gravity**, falling linearly from the surface's g at R to 0 at the centre:", ""] + gravity_rows()
    out += ["", f"**Stair steps of the inner core** (a ball of 1 m cubes whose centres lie within {RI:.1f} m; the plane through the centre and two grid axes, 80 m of walk along the tangent):", ""] + stair_rows()
    return "\n".join(out) + "\n"


# ---------------------------------------------------------------- pictures
def hdr(im, ppm, title, fig):
    P.furniture(im, ppm, title, fig)


def sample_plane(theta_deg, W=1200, H=800, ppm=50, wlo=-6.0):
    """World (x, z) of every pixel for a view with local down pointing down the picture, at the ball's surface
    point whose normal is theta from the x axis. Returns x, z, u, w, d, t, P0."""
    th = math.radians(theta_deg)
    d, t = np.array([math.cos(th), math.sin(th)]), np.array([-math.sin(th), math.cos(th)])
    p0 = RI * d
    px, py = np.meshgrid(np.arange(W) + .5, np.arange(H) + .5)
    u, w = (px - W / 2) / ppm, wlo + (H - py) / ppm
    x = p0[0] + u * t[0] + w * d[0]
    z = p0[1] + u * t[1] + w * d[1]
    return x, z, u, w, p0


def core_surface(theta, drained, path):
    W, H, ppm = 1200, 800, 50
    x, z, u, w, p0 = sample_plane(theta)
    ix, iz = np.floor(x), np.floor(z)
    solid = (ix + .5) ** 2 + (iz + .5) ** 2 <= RI ** 2
    img = np.empty((H, W, 3), np.uint8)
    img[:] = P.AIR if drained else LIQ
    par = ((ix + iz) % 2 == 0)
    img[solid] = np.where(par[solid][:, None], CORE[0], CORE[1]).astype(np.uint8)
    # ground under the player (u = 0): first solid going down
    col = solid[:, W // 2]
    top = int(np.argmax(col))
    fy = top
    im = Image.fromarray(img)
    what = "drained (air)" if drained else "liquid metal, no cell lines"
    hdr(im, ppm, f"Inner core surface, {theta} deg off a grid axis, down is down the picture; above: {what}", (W / 2, fy))
    im.save(path)


def centre_pic(path):
    W = H = 1200
    ppm = W / 40
    px, py = np.meshgrid(np.arange(W) + .5, np.arange(H) + .5)
    x, z = (px - W / 2) / ppm, (H / 2 - py) / ppm
    ix, iz = np.floor(x), np.floor(z)
    img = np.where(((ix + iz) % 2 == 0)[..., None], CORE[0], CORE[1]).astype(np.uint8)
    img[(z >= -1) & (z < 1)] = P.AIR  # a 2 m shaft through the centre, along x
    im = Image.fromarray(img)
    d = ImageDraw.Draw(im)
    d.text((W / 2 + 70, H / 2 - 150), "centre: g = 0", fill=(255, 255, 255), font=P.font(22))
    d.text((W / 2 + 70, H / 2 - 120), "2 m shaft along x, perfect 1 m cubes (Ri = 807 m)", fill=(255, 255, 255), font=P.font(16))
    hdr(im, ppm, "Inner core, the innermost 40 m: a tunnel through the centre, the player weightless", (W / 2, H / 2 + 0.9 * ppm))
    im.save(path)


def mantle_bottom(path):
    W, H, ppm = 1200, 800, 50
    rc = RCS[0][1]
    px, py = np.meshgrid(np.arange(W) + .5, np.arange(H) + .5)
    x, y = (px - W / 2) / ppm, rc + 5 + (H / 2 - py) / ppm
    r, a = np.hypot(x, y), np.arctan2(x, y)
    q = ISTEP
    j = np.floor(np.log(np.maximum(r, 1) / rc) / math.log(1 + q)).astype(np.int64)
    col = np.floor(a / q).astype(np.int64)
    img = P.colour((col + j) % 2, 0)
    img = np.asarray(img, np.uint8).copy()
    img[r < rc] = LIQ
    dug = (col >= -2) & (col <= 1) & (j >= 1)
    img[dug] = P.AIR
    im = Image.fromarray(img)
    rf = rc * (1 + q)
    fy = H / 2 - (rf - (rc + 5)) * ppm * -1 if False else (H / 2) + (5 - (rf - rc)) * ppm
    d = ImageDraw.Draw(im)
    for yy, t in ((60, f"block {rc / R:.3f} m wide and thick (Rc = {rc:,.0f} m)"), (90, "liquid outer core below the lowest block layer")):
        d.rectangle([W - 460, yy - 2, W - 6, yy + 22], fill=(0, 0, 0))
        d.text((W - 454, yy), t, fill=(255, 255, 255), font=P.font(17))
    hdr(im, ppm, "Mantle's bottom (variant L, Rc = 0.546 R): 2.2 m tunnel in 0.55 m blocks, liquid below", (W / 2, fy))
    im.save(path)


def built(path):
    """Drained outer core at the first hidden seam (Rc/2): a wall and a platform a player built."""
    W, H, ppm = 1200, 800, 50
    rc = RCS[0][1]
    rs = rc / 2
    up = dict(top=rc, bot=rs, L=math.ceil(math.log(2) / math.log(1 + 1 / R)), q=1 / R, k=0)
    lo = dict(top=rs, bot=RI, L=math.ceil(math.log(rs / RI) / math.log(1 + 2 / R)), q=2 / R, k=1)
    px, py = np.meshgrid(np.arange(W) + .5, np.arange(H) + .5)
    x, y = (px - W / 2) / ppm, rs + (H / 2 - py) / ppm
    r, a = np.hypot(x, y), np.arctan2(x, y)
    img = np.empty((H, W, 3), np.uint8)
    img[:] = P.AIR
    for s, m in ((up, r >= rs), (lo, r < rs)):
        s["bot"] = s["bot"] if s is lo else s["bot"]
        col = np.floor(a / s["q"]).astype(np.int64)
        j = P.layer_of(r, s)
        wall = (col >= -5) & (col <= 4) if s is up else (col >= -2) & (col <= 1)
        plat = (s is lo) & (col >= -21) & (col <= -5) & (j >= 6) & (j <= 7)
        built_ = m & (wall | plat)
        img[built_] = P.colour((col + j) % 2, s["k"])[built_]
    im = Image.fromarray(img)
    d = ImageDraw.Draw(im)
    for xx in range(0, W, 14):
        d.line([(xx, H / 2), (xx + 6, H / 2)], fill=(90, 90, 110), width=1)
    d.rectangle([W - 560, 56, W - 6, 112], fill=(0, 0, 0))
    d.text((W - 554, 60), f"hidden seam at r = {rs:,.0f} m (dashed): upper blocks {up['q'] * rs:.2f} m, lower {lo['q'] * rs:.2f} m", fill=(255, 255, 255), font=P.font(16))
    d.text((W - 554, 84), "nothing shows it except what was built across it", fill=(255, 255, 255), font=P.font(16))
    rf = lo["top"] * math.exp(-6 * math.log(lo["top"] / lo["bot"]) / lo["L"])
    fy = H / 2 + (rs - rf) * ppm
    hdr(im, ppm, "Drained outer core, variant L: a wall and a platform built across the first hidden seam", (W / 2 - 7 * ppm, fy))
    im.save(path)


def whole(path):
    W, H = 1200, 900
    im = Image.new("RGB", (W, H), (18, 20, 28))
    d = ImageDraw.Draw(im)
    rad = 270
    sc = rad / R
    for i, (name, rc) in enumerate(RCS):
        cx, cy = 300 + 600 * i, 300
        for r, c in ((R, (92, 82, 66)), (rc, tuple(LIQ)), (RI, CORE[0])):
            d.ellipse([cx - r * sc, cy - r * sc, cx + r * sc, cy + r * sc], fill=c)
        d.text((cx - 150, 8), f"Variant L, Rc = {name}", fill=(255, 255, 255), font=P.font(18))
        f = P.font(15)
        y = 590
        for yy, c, t in (
            (y, (92, 82, 66), f"crust + mantle: r {R:,.0f} to {rc:,.0f} m, one grid, blocks {1:.2f} to {rc / R:.3f} m"),
            (y + 30, tuple(LIQ), f"outer core: r {rc:,.0f} to {RI:,.0f} m, liquid metal, 2 hidden shells"),
            (y + 60, CORE[0], f"inner core: r {RI:,.0f} m, Cartesian 1.00 m cubes")):
            d.rectangle([cx - 285, yy + 3, cx - 267, yy + 21], fill=c)
            d.text((cx - 257, yy + 2), t, fill=(255, 255, 255), font=f)
        d.text((cx - 285, y + 95), f"g at Rc {rc / R:.2f}, at Ri 0.19 of the surface's; liquid has no cells to draw", fill=(170, 170, 170), font=P.font(13))
    im.save(path)


if __name__ == "__main__":
    text = build()
    print(text)
    p = os.path.join(HERE, "README.md")
    s = open(p, encoding="utf-8").read()
    s = re.sub(r"<!-- L tables -->.*<!-- /L tables -->", lambda m: "<!-- L tables -->\n" + text + "\n<!-- /L tables -->", s, flags=re.S)
    open(p, "w", encoding="utf-8").write(s)
    o = os.path.join(HERE, "out")
    whole(os.path.join(o, "layered-whole.png"))
    mantle_bottom(os.path.join(o, "layered-mantle-bottom.png"))
    for th, nm in ((0, "axis"), (45, "45")):
        core_surface(th, False, os.path.join(o, f"layered-core-{nm}.png"))
        core_surface(th, True, os.path.join(o, f"layered-core-{nm}-drained.png"))
    centre_pic(os.path.join(o, "layered-centre.png"))
    built(os.path.join(o, "layered-built.png"))
