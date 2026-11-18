"""Pictures for the shells prototype: `python -I pictures.py` writes out/*.png (Pillow + numpy)."""
import math, os, sys
import numpy as np
from PIL import Image, ImageDraw, ImageFont

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import shells as S

OUT = os.path.join(HERE, "out")
os.makedirs(OUT, exist_ok=True)
N, R = 6144, S.radius(6144)
AIR = np.array([22, 24, 32], np.uint8)
# (light, dark) per shell parity so a seam shows as a change of colour as well as of size
PAL = [((226, 200, 150), (170, 140, 100)), ((160, 205, 220), (100, 145, 170))]
SOLID = np.array([70, 70, 78], np.uint8)


def font(sz):
    for f in ("arial.ttf", "DejaVuSans.ttf"):
        try:
            return ImageFont.truetype(f, sz)
        except OSError:
            pass
    return ImageFont.load_default(sz)


def colour(par, shellk):
    pal = PAL[shellk % 2]
    return np.where(par[..., None] == 0, np.array(pal[0], np.uint8), np.array(pal[1], np.uint8))


def layer_of(r, s):
    return np.floor(np.log(s["top"] / r) / (math.log(s["top"] / s["bot"]) / s["L"])).astype(np.int64)


def figure(d, x, y, ppm):
    """1.8 m player, feet at pixel (x, y): white with a black outline so it reads on light and dark."""
    u = ppm
    def ln(pts, w=3, c=(255, 255, 255)):
        d.line([(x + a * u, y - b * u) for a, b in pts], fill=(0, 0, 0), width=w + 3, joint="curve")
        d.line([(x + a * u, y - b * u) for a, b in pts], fill=c, width=w, joint="curve")
    ln([(-0.15, 0), (0, 0.9), (0.15, 0)])            # legs
    ln([(0, 0.9), (0, 1.5)], 5)                       # torso
    ln([(-0.3, 1.0), (0, 1.45), (0.3, 1.0)])          # arms
    hx, hy, hr = x, y - 1.65 * u, 0.15 * u
    d.ellipse([hx - hr - 1.5, hy - hr - 1.5, hx + hr + 1.5, hy + hr + 1.5], fill=(0, 0, 0))
    d.ellipse([hx - hr, hy - hr, hx + hr, hy + hr], fill=(255, 255, 255))


def furniture(im, ppm, title, figxy=None):
    d = ImageDraw.Draw(im)
    if figxy:
        figure(d, figxy[0], figxy[1], ppm)
    f = font(18)
    x0, y0 = 20, im.height - 34
    d.rectangle([x0 - 8, y0 - 26, x0 + ppm + 70, y0 + 28], fill=(0, 0, 0))
    d.line([(x0, y0), (x0 + ppm, y0)], fill=(255, 255, 255), width=4)
    for xx in (x0, x0 + ppm):
        d.line([(xx, y0 - 8), (xx, y0 + 8)], fill=(255, 255, 255), width=3)
    d.text((x0 + ppm + 10, y0 - 11), "1 m", fill=(255, 255, 255), font=f)
    d.text((x0, y0 - 24), "player 1.8 m", fill=(255, 255, 255), font=font(14))
    d.rectangle([0, 0, im.width, 34], fill=(0, 0, 0))
    d.text((10, 6), title, fill=(255, 255, 255), font=f)


def section(variant, path):
    """Radial section across the first seam (shell 0 over shell 1), 24 x 16 m, 50 px a metre."""
    sh = S.shells(N, variant)
    up, lo = sh[0], sh[1]
    rs = up["bot"]
    W, H, ppm = 1200, 800, 50
    px, py = np.meshgrid(np.arange(W) + 0.5, np.arange(H) + 0.5)
    x, y = (px - W / 2) / ppm, (H / 2 - py) / ppm + rs
    r, a = np.hypot(x, y), np.arctan2(x, y)
    img = np.empty((H, W, 3), np.uint8)
    dug = np.zeros((H, W), bool)
    ql = lo["wtop"] / lo["top"]
    for s, mask in ((up, r >= rs), (lo, r < rs)):
        q = s["wtop"] / s["top"]
        col, j = np.floor(a / q).astype(np.int64), layer_of(r, s)
        img[mask] = colour((col + j) % 2, s["k"])[mask]
        t = (col >= -ql / q) & (col <= ql / q - 1)  # the 2 lower cells = 2 x ql wide
        dug |= mask & t & ((j < 5) | (s is up))
    img[dug] = AIR
    im = Image.fromarray(img)
    d = ImageDraw.Draw(im)
    ys = H / 2
    d.line([(0, ys), (W, ys)], fill=(220, 40, 40), width=3)
    f = font(17)
    lbl = lambda yy, t: (d.rectangle([W - 330, yy - 2, W - 6, yy + 22], fill=(0, 0, 0)), d.text((W - 324, yy), t, fill=(255, 255, 255), font=f))
    lbl(ys - 56, f"upper cell {up['wbot']:.2f} m wide, thick")
    lbl(ys - 28, f"seam (red) at r = {rs:,.0f} m")
    lbl(ys + 6, f"lower cell {lo['wtop']:.2f} m wide, thick")
    jf = 5
    rf = lo["top"] * math.exp(-jf * math.log(lo["top"] / lo["bot"]) / lo["L"])
    fy = H / 2 - (rf - rs) * ppm
    furniture(im, ppm, f"Variant {variant}: seam {up['k']}/{lo['k']}, dug 2 lower cells ({2 * lo['wtop']:.2f} m) wide, 5 layers below the seam", (W / 2 - 4 * ppm, fy))
    im.save(path)


def centre(opt, path):
    """Section through the planet's centre (plane y = 0, faces +-x and +-z), the inner 80 m, variant A."""
    sh = S.shells(N, "A", chunk_rule=False, min_n=3)
    last_chunk = S.shells(N, "A")[-1]
    rc = last_chunk["bot"]
    W = H = 1200
    ppm = W / 80
    px, py = np.meshgrid(np.arange(W) + 0.5, np.arange(H) + 0.5)
    x, z = (px - W / 2) / ppm, (H / 2 - py) / ppm
    r = np.hypot(x, z)
    img = np.empty((H, W, 3), np.uint8)
    img[:] = SOLID
    big = np.maximum(abs(x), abs(z)) + 1e-12
    alpha = np.arctan(np.minimum(abs(x), abs(z)) / big) * np.sign(np.where(abs(x) >= abs(z), z * np.sign(x), x * np.sign(z)))
    face = np.where(abs(x) >= abs(z), np.where(x > 0, 0, 1), np.where(z > 0, 2, 3))
    use = sh if opt == "c" else [s for s in sh if s["n"] % S.CS == 0]
    for s in use:
        m = (r < s["top"]) & (r >= s["bot"])
        if s is use[-1] and opt != "c":
            m = (r < s["top"]) & (r >= (s["bot"] - 5 if opt == "b" else s["bot"]))  # (b): shell reaches the cube
        col = np.floor((alpha + math.pi / 4) / (math.pi / 2 / s["n"])).astype(np.int64)
        j = layer_of(np.maximum(r, 1e-9), s)
        img[m] = colour((col + j + face) % 2, s["k"])[m]
    if opt == "b":
        h = math.floor(rc)
        cube = (abs(x) < h) & (abs(z) < h)
        img[cube] = colour((np.floor(x).astype(np.int64) + np.floor(z).astype(np.int64)) % 2, 0)[cube] // 1
        img[cube] = np.where(((np.floor(x) + np.floor(z)) % 2 == 0)[cube][:, None], (205, 190, 215), (150, 130, 170)).astype(np.uint8)
    elif opt == "a":
        img[r < rc] = SOLID
    img[(abs(x) < 1) & (z > 0) & ((r >= rc) if opt == "a" else True)] = AIR
    im = Image.fromarray(img)
    d = ImageDraw.Draw(im)
    title = {"a": f"(a) solid ball r = {rc:.1f} m, undiggable; shaft stops on it",
             "b": f"(b) Cartesian cube of 1 m blocks (66 x 66 x 66, violet), shell 6 meets its faces",
             "c": f"(c) keep halving: shells down to 3 columns, last ball r = {sh[-1]['bot']:.2f} m"}[opt]
    fy = H / 2 - (rc if opt == "a" else 0) * ppm
    furniture(im, ppm, title + " (inner 80 m, variant A)", (W / 2, fy))
    if opt == "a":
        ImageDraw.Draw(im).text((W / 2 - 150, H / 2 + 10), "solid, not dug: 33 m radius", fill=(255, 255, 255), font=font(22))
    im.save(path)


def whole(path):
    W, H = 1200, 980
    im = Image.new("RGB", (W, H), (18, 20, 28))
    d = ImageDraw.Draw(im)
    cols = [(230, 80, 70), (240, 160, 50), (230, 220, 70), (110, 210, 90), (60, 200, 220), (90, 130, 250), (200, 100, 240), (255, 255, 255)]
    rad = 270
    sc = rad / R
    for i, v in enumerate(("A", "B")):
        cx, cy = 300 + 600 * i, 310
        sh = S.shells(N, v)
        d.ellipse([cx - rad, cy - rad, cx + rad, cy + rad], fill=(70, 62, 52))
        for k, s in enumerate(sh):
            rr = s["bot"] * sc
            d.ellipse([cx - rr, cy - rr, cx + rr, cy + rr], fill=(70, 62, 52) if k % 2 else (92, 82, 66), outline=cols[k], width=2)
        d.text((cx - 90, 10), f"Variant {v}: {'seams at R/2, R/4...' if v == 'A' else 'seams at R/sqrt2, R/2sqrt2...'}", fill=(255, 255, 255), font=font(18))
        d.text((cx - 70, cy - rad - 22 + 22), "", font=font(12))
        # scale: the ball under the last seam
        y = 640
        d.text((cx - 280, y - 26), f"R = {R:,.0f} m (n = {N}), the surface is the outer circle; centre ball r = {sh[-1]['bot']:.1f} m", fill=(200, 200, 200), font=font(14))
        for k, s in enumerate(sh):
            yy = y + k * 28
            d.rectangle([cx - 280, yy + 3, cx - 262, yy + 21], fill=cols[k])
            d.text((cx - 252, yy + 2), f"shell {k}: r {s['top']:,.0f} to {s['bot']:,.1f} m, blocks {s['wtop']:.2f} to {s['wbot']:.2f} m, {s['n']} columns", fill=(255, 255, 255), font=font(15))
        d.text((cx - 280, y + len(sh) * 28 + 4), "ring colour = bottom (seam) of that shell; inner rings are only a few px", fill=(170, 170, 170), font=font(13))
    im.save(path)


if __name__ == "__main__":
    section("A", os.path.join(OUT, "section-A.png"))
    section("B", os.path.join(OUT, "section-B.png"))
    for o in "abc":
        centre(o, os.path.join(OUT, f"centre-{o}.png"))
    whole(os.path.join(OUT, "whole.png"))
    print("ok")
