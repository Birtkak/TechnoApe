"""Item sprites for the rock kinds, clutter, fish and furs (ADR 0057, 0058).

make_item_sprites.py calls add(globals()) just before it validates and writes, so these shapes use
its primitives and palettes and the hunk in that file stays one line.
"""


def add(ns):
    new, put, ellipse, poly, line = ns["new"], ns["put"], ns["ellipse"], ns["poly"], ns["line"]
    shade, outline, body, compose, ramp = ns["shade"], ns["outline"], ns["body"], ns["compose"], ns["ramp"]
    shape, sp, PALETTES = ns["shape"], ns["sp"], ns["PALETTES"]

    # rock kinds: the loose stone in each kind's colours (atlas_rocks.STONES, dark to light)
    for k, r in {
        "granite": ("#3b2f2b", "#6e5850", "#ad8b7e", "#c7a99c", "#e0cbc1"),
        "diorite": ("#3e3d3a", "#77766f", "#b7b6b0", "#dcdad4", "#f1f0eb"),
        "limestone": ("#4e4636", "#7d7259", "#b0a385", "#d3c8ab", "#e9e0c9"),
        "slate": ("#1f2228", "#353a42", "#4e5560", "#6c7480", "#858d99"),
        "marble": ("#6d6a65", "#a8a59f", "#d6d3cc", "#efede8", "#fbfaf7"),
        "chalk": ("#7d7663", "#b3ac97", "#ddd7c6", "#f2eee3", "#fdfbf5"),
        "basalt": ("#141417", "#26262b", "#3b3b41", "#55555d", "#6b6b74"),
    }.items():
        PALETTES["rock_" + k] = ramp(*r)
        sp(k + "_stone", ("rock", "rock_" + k))
    PALETTES["pyrite"] = ramp("#3b3220", "#8a7a44", "#c9b467", "#e2cf7c", "#f6ebae")
    PALETTES["acorn"] = ramp("#2c1f10", "#5a3e1c", "#8a6a34", "#a8844a", "#c8a466")
    PALETTES["roasted"] = ramp("#1c1008", "#3a2210", "#5a3a1c", "#74502a", "#94703e")
    PALETTES["hazel"] = ramp("#2e1d10", "#5a3a1a", "#8a5c2e", "#b98a4e", "#d6ae72")
    PALETTES["cone"] = ramp("#24160b", "#3e2713", "#6f4721", "#9a6a37", "#cfa86c")
    PALETTES["driftwood"] = ramp("#4a443a", "#7a7266", "#a39b8c", "#c4bcac", "#e2dccf")
    PALETTES["antler"] = ramp("#3e2f1f", "#7d6548", "#bba786", "#e0d3b8", "#f6efe0")
    PALETTES["ice"] = ramp("#3a5a7a", "#7ea8d0", "#b3d6f4", "#d6ebfb", "#ffffff")
    PALETTES["crystal"] = ramp("#2a0f3e", "#5a2482", "#8a44b8", "#b88ae6", "#f0e2ff")
    PALETTES["fish"] = ramp("#1e3040", "#3e6078", "#6a90a8", "#9cbccc", "#dceaf0")
    PALETTES["cooked_fish"] = ramp("#3a200c", "#7a4a22", "#a86e3a", "#c89058", "#e8c08a")
    PALETTES["fur"] = ramp("#3a1a08", "#7a3c14", "#b0601e", "#d0843a", "#ecb878")
    PALETTES["strawberry"] = ramp("#4a0a10", "#8a1418", "#c41e26", "#e8473c", "#ff9a80")

    # wild strawberry: a rounded cone, yellow seeds, a green star of leaves on top
    g = shade(poly(new(), [(3.5, 6), (12.5, 6), (12, 9.5), (8, 14.5), (4, 9.5)]))
    for (x, y) in [(5, 8), (8, 7), (10, 8), (6, 10), (9, 10), (7, 12), (11, 9)]:
        put(g, x, y, "s")
    g = outline(g)
    for (x, y) in [(5, 5), (6, 4), (7, 5), (8, 4), (9, 5), (10, 4), (11, 5), (8, 3), (8, 2)]:
        put(g, x, y, "g")
    shape("strawberry", g, {"s": "#f6e27a", "g": "#3e7e2a"})
    sp("wild_strawberry", ("strawberry", "strawberry"))

    # hazelnut: a round brown shell, a pale scar at the bottom, a point on top
    g = shade(ellipse(new(), 8, 9, 5.2, 5))
    for (x, y) in [(6, 13), (7, 13), (8, 13), (9, 13), (7, 12), (8, 12)]:
        put(g, x, y, "p")
    put(g, 8, 3, "d")
    shape("hazelnut", outline(g), {"p": "#e8d6a8"})
    sp("hazelnut", ("hazelnut", "hazel"))

    # acorn: a scaly cap over a smooth nut with a tip
    nut = shade(ellipse(new(), 8, 10, 4.2, 4.5))
    put(nut, 8, 14, "d")
    cap = ellipse(new(), 8, 6, 5.4, 2.8, "c")
    for (x, y) in [(4, 6), (6, 5), (8, 6), (10, 5), (12, 6), (5, 7), (9, 7), (7, 4)]:
        put(cap, x, y, "C")
    put(cap, 8, 2, "C")
    put(cap, 8, 3, "c")
    shape("acorn", outline(compose(nut, cap)), {"c": "#7c6a3a", "C": "#56482a"})
    sp("acorn", ("acorn", "acorn"))
    sp("roasted_acorn", ("acorn", "roasted", {"c": "#3a2a14", "C": "#24180c"}))

    # pine cone: an oval of scales, lit and shaded in rows
    g = shade(ellipse(new(), 8, 8.5, 4.2, 6.2))
    for y in range(3, 15, 2):
        for x in range(4 + (y // 2) % 2, 13, 2):
            if g[y][x] != ".":
                g[y][x] = "d"
    shape("pine_cone", outline(g))
    sp("pine_cone", ("pine_cone", "cone"))

    # driftwood: a bleached, crooked branch with a broken fork
    g = new()
    for (x0, y0, x1, y1) in [(2, 13, 6, 9), (6, 9, 9, 8), (9, 8, 13, 3), (8, 8, 9, 4)]:
        line(g, x0, y0, x1, y1, "#", 2)
    shape("driftwood", body(g))
    sp("driftwood", ("driftwood", "driftwood"))

    # antler: a curved beam with three tines
    g = new()
    for (x0, y0, x1, y1) in [(3, 14, 5, 9), (5, 9, 8, 5), (8, 5, 12, 2), (5, 10, 2, 7), (7, 6, 6, 2), (9, 4, 13, 5)]:
        line(g, x0, y0, x1, y1, "#", 2)
    shape("antler", body(g))
    sp("antler", ("antler", "antler"))

    sp("ice_shard", ("crystal", "ice"))
    sp("cave_crystal", ("crystal", "crystal"))
    sp("pyrite", ("nugget", "pyrite"))

    # fish: a body with a tail fin, an eye, a darker back
    g = shade(compose(ellipse(new(), 7, 8.5, 5.5, 3.2), poly(new(), [(11, 8.5), (15, 5), (15, 12)])))
    for x in range(3, 11):
        if g[6][x] != ".":
            g[6][x] = "d"
    put(g, 4, 8, "e")
    shape("fish", outline(g), {"e": "#101418"})
    sp("raw_fish", ("fish", "fish"))
    sp("cooked_fish", ("fish", "cooked_fish"))
    sp("fur", ("hide", "fur"))
