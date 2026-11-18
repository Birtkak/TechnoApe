"""Shell tables for digging to the centre. `python -I shells.py` prints the tables and rewrites the block
between the <!-- tables --> markers in README.md. Pure arithmetic, no engine code."""
import math, os, re

CS = 32  # cells per chunk edge, crates/engine/src/world/mod.rs
PLANETS = [("small", 768), ("standard", 6144), ("huge", 12288), ("Luna (standard)", 1664)]  # Luna n: bake/luna.rs
HERE = os.path.dirname(os.path.abspath(__file__))


def radius(n):  # body.rs: 6 n^2 columns cover 4 pi R^2
    return n / math.sqrt(2 * math.pi / 3)


def shells(n, variant, chunk_rule=True, min_n=1):
    """Shells from the surface down. A shell k has n/2^k columns a face edge; a cell at radius r is
    2^k * r / R wide and thick (mean size, as body.rs). With chunk_rule the list stops when n_k is no
    longer a multiple of CS (a whole number of chunks); without it, when n_k would drop under min_n."""
    R, out, k = radius(n), [], 0
    while True:
        nk = n / 2 ** k
        if nk != int(nk) or (chunk_rule and nk % CS) or nk < min_n:
            break
        if variant == "A":
            top, bot = R / 2 ** k, R / 2 ** (k + 1)
        else:
            top = R if k == 0 else R / (math.sqrt(2) * 2 ** (k - 1))
            bot = R / (math.sqrt(2) * 2 ** k)
        q = 2 ** k / R
        L = math.ceil(math.log(top / bot) / math.log(1 + q))
        out.append(dict(k=k, top=top, bot=bot, n=int(nk), wtop=q * top, wbot=q * bot, L=L,
                        cells=6 * int(nk) ** 2 * L, chunks=nk / CS))
        k += 1
    return out


def fmt(x):
    return f"{x:,.1f}" if x < 1000 else f"{x:,.0f}"


def table(n, variant):
    R, sh = radius(n), shells(n, variant)
    rows = ["| shell | top r (m) | bottom r (m) | columns a face edge | chunks a face edge | block width top / bottom (m) | layers | cells |",
            "|---|---|---|---|---|---|---|---|"]
    for s in sh:
        rows.append(f"| {s['k']} | {fmt(s['top'])} | {fmt(s['bot'])} | {s['n']} | {s['chunks']:g} | {s['wtop']:.2f} / {s['wbot']:.2f} | {s['L']:,} | {s['cells']:.3g} |")
    tl, tc = sum(s["L"] for s in sh), sum(s["cells"] for s in sh)
    nxt = sh[-1]["n"] / 2
    rows.append(f"| total | {fmt(R)} | {fmt(sh[-1]['bot'])} | | | | {tl:,} | {tc:.3g} |")
    note = (f"n = {n}, R = {R:,.1f} m. {len(sh)} shells, {tl:,} layers from the surface, {tc:.3g} cells. "
            f"The shell after the last would have {nxt:g} columns a face edge = {nxt / CS:g} chunks, not a whole number. "
            f"**Ball left at the centre: radius {sh[-1]['bot']:.1f} m** ({R - sh[-1]['bot']:,.0f} m below the surface).")
    return "\n".join(rows) + "\n\n" + note


def centre(n, variant="A"):
    """Numbers for the three ways to fill the ball under the last chunk-sized shell."""
    sh = shells(n, variant)
    rc, last = sh[-1]["bot"], sh[-1]
    R = radius(n)
    s = 2 ** last["k"] / R  # width per metre of radius in the last shell
    out = {"rc": rc}
    out["a_vol"] = 4 / 3 * math.pi * rc ** 3
    out["a_share"] = out["a_vol"] / (4 / 3 * math.pi * R ** 3)
    # (b) cube whose face centres touch the ball: half side h, N = 2h whole blocks an edge
    h = math.floor(rc)
    N = 2 * h
    out["b_h"], out["b_N"], out["b_blocks"], out["b_corner"] = h, N, N ** 3, h * math.sqrt(3)
    # a shell cell where it meets the cube face, radial projection onto the plane at distance h:
    # radius rho = hypot(h, x, y), cosθ = h/rho, cell width w = s*rho, footprint on the face = w^2/cosθ
    # (w across the tilt, w/cosθ along it); a cube block is 1 m x 1 m
    pts = {"face centre": (0, 0), "mid of an edge": (h, 0), "cube corner": (h, h)}
    b = {}
    for name, (x, y) in pts.items():
        rho = math.sqrt(h * h + x * x + y * y)
        c = h / rho
        b[name] = dict(rho=rho, w=s * rho, along=s * rho / c, area=(s * rho) ** 2 / c, cols_per_block=1 / (s * rho))
    out["b"] = b
    # cells of the shell cut by the cube: thickness left between the cube face and the shell's top (last.top)
    out["b_shell_left"] = {name: last["top"] - v["rho"] for name, v in b.items()}
    # (c) keep halving to 3 columns a face edge
    sc = shells(n, variant, chunk_rule=False, min_n=3)
    out["c_shells"] = len(sc) - len(sh)
    out["c_bottom"] = sc[-1]["bot"]
    out["c_layers"] = sum(x["L"] for x in sc[len(sh):])
    out["c_cells"] = sum(x["cells"] for x in sc[len(sh):])
    out["c_list"] = sc[len(sh):]
    return out


def build():
    parts = ["### Shell tables (`python -I shells.py`)", ""]
    for name, n in PLANETS:
        for v, desc in (("A", "seams at R/2, R/4, ..."), ("B", "seams at R/sqrt2, R/(2 sqrt2), ...")):
            parts += [f"**{name}, variant {v} ({desc})**", "", table(n, v), ""]
    parts += ["### The centre ball, variant A (`python -I shells.py`)", ""]
    parts += ["| planet | ball radius (m) | (a) ball volume (m3), share of planet | (b) cube edge (blocks), blocks, corner at (m) | (c) shells to 3 columns, extra layers, cells, last radius (m) |", "|---|---|---|---|---|"]
    for name, n in PLANETS:
        c = centre(n)
        parts.append(f"| {name} | {c['rc']:.1f} | {c['a_vol']:.3g}, {c['a_share']:.1e} | {c['b_N']}, {c['b_blocks']:,}, {c['b_corner']:.1f} | {c['c_shells']}, {c['c_layers']}, {c['c_cells']:.3g}, {c['c_bottom']:.2f} |")
    parts += ["", "Option (b), standard planet: the cube's face against the innermost shell (cell = the shell's cell where it is cut by the cube face):", "",
              "| where on the cube face | distance from the centre (m) | shell cell width (m) | along the tilt (m) | footprint on the face (m2, a block is 1) | shell cells across one block edge | shell left above that point (m) |", "|---|---|---|---|---|---|---|"]
    c = centre(6144)
    for name, v in c["b"].items():
        parts.append(f"| {name} | {v['rho']:.1f} | {v['w']:.2f} | {v['along']:.2f} | {v['area']:.2f} | {v['cols_per_block']:.2f} | {c['b_shell_left'][name]:.1f} |")
    parts.append("")
    return "\n".join(parts)


if __name__ == "__main__":
    text = build()
    print(text)
    p = os.path.join(HERE, "README.md")
    if os.path.exists(p):
        s = open(p, encoding="utf-8").read()
        s = re.sub(r"<!-- tables -->.*<!-- /tables -->", lambda m: "<!-- tables -->\n" + text + "\n<!-- /tables -->", s, flags=re.S)
        open(p, "w", encoding="utf-8").write(s)
