"""Sprites of the four Stone Age stages (ADR 0059), added to make_item_sprites.py's shapes,
palettes and sprites by add(globals()) just before it writes the JSON. Kept in its own file so
the parts that edit make_item_sprites.py merge cleanly. Stub items another part defines (part E's
ochre, pyrite, antler, fish ...) only get a sprite here when that part has not drawn one.
"""


def add(G):
    new, art, line, ellipse, rect, poly = G["new"], G["art"], G["line"], G["ellipse"], G["rect"], G["poly"]
    shade, outline, body, compose, put = G["shade"], G["outline"], G["body"], G["compose"], G["put"]
    shape, sp, ramp, PALETTES, SP, S = G["shape"], G["sp"], G["ramp"], G["PALETTES"], G["SP"], G["S"]
    H, SH, tool_pts = G["H"], G["SH"], G["tool_pts"]

    PALETTES.update({
        "birch": ramp("#3a3630", "#8a8478", "#d8d4c8", "#ece8de", "#ffffff"),
        "tar": ramp("#060403", "#140e0a", "#241812", "#3a2a1e", "#6a5040"),
        "tempered": ramp("#1a1010", "#4a2e2a", "#6e4640", "#946660", "#c8a098"),
        "fungus": ramp("#2a1a0a", "#5e3e1c", "#8a5e2c", "#ad7c40", "#d0a468"),
        "fur": ramp("#1e140c", "#4a3424", "#6e5038", "#8e6c4c", "#b49070"),
        "bear": ramp("#100a06", "#2c1e14", "#44301e", "#5e442c", "#806040"),
        "linen": ramp("#5a5444", "#a49c86", "#cac2aa", "#ded8c2", "#f4f0e2"),
        "ochre_red": ramp("#3a0e06", "#7a2210", "#a8301e", "#c84a30", "#e8806a"),
        "ochre_yellow": ramp("#4a3008", "#8a6014", "#c08a26", "#d8a840", "#f0d080"),
        "malachite": ramp("#0a2a14", "#1c5a30", "#2e8a4a", "#4cb06a", "#90dca8"),
        "pyrite": ramp("#3a3008", "#7a6a1c", "#b49a34", "#d8c050", "#fff09a"),
        "fat": ramp("#5a5030", "#b0a470", "#d8cc98", "#ece2b4", "#fcf6dc"),
        "grain": ramp("#4a3410", "#8a6a28", "#b8923c", "#d4b05a", "#f0d690"),
        "grain_dark": ramp("#3a2410", "#704a20", "#9a6a34", "#b8884a", "#dcb47a"),
        "flaxseed": ramp("#2a1a0a", "#5a3a1a", "#7e5428", "#9e7038", "#c49a5c"),
        "flour": ramp("#6a645a", "#bab4a6", "#e2dccc", "#f0ece0", "#ffffff"),
        "dough": ramp("#6a5a40", "#b8a47c", "#dcc89e", "#ece0be", "#fcf4e0"),
        "dried": ramp("#2a0c08", "#5a1c14", "#7e2c20", "#9a4232", "#c06a58"),
        "smoked": ramp("#1a0c06", "#3a2010", "#5a341c", "#7a4a2a", "#a06a44"),
        "salted": ramp("#5a3a3a", "#a87070", "#cc9a96", "#e2bcb8", "#f8e4e0"),
        "fish": ramp("#1a2a3a", "#3e5a74", "#6a8aa6", "#94b2ca", "#d4e4f0"),
        "salt": ramp("#7a7a80", "#c4c4cc", "#e4e4ec", "#f2f2f8", "#ffffff"),
        "antler": ramp("#3a2c1c", "#7a6446", "#a88c66", "#c8ac84", "#e8d4b0"),
        "chalk": ramp("#6a6a64", "#b8b8ae", "#dcdcd2", "#ececE4", "#ffffff"),
        "floorstone": ramp("#08080c", "#1c1c24", "#30303c", "#4a4a5a", "#8484a0"),
        "greenstone": ramp("#14201a", "#2c4434", "#46664e", "#64886a", "#9cc0a0"),
    })

    def tool(st, base=(12.0, 4.0)):
        return poly(new(), tool_pts(st, base))

    # ---------- shapes ----------
    # a shelf fungus: a half disc with growth rings
    g = shade(ellipse(new(), 8, 10, 6.2, 4.0))
    for y in range(10, 16):
        for x in range(16):
            g[y][x] = "."
    for x in range(3, 13):
        put(g, x, 9, "l")
    for x in range(4, 12, 2):
        put(g, x, 7, "d")
    shape("bracket", outline(g))
    # a needle: a thin diagonal with an eye at the top
    g = line(new(), 3, 13, 12, 4, "m")
    put(g, 12, 3, "l")
    put(g, 13, 3, "l")
    put(g, 13, 4, "l")
    put(g, 12, 4, ".")
    shape("needle", outline(g))
    # a fish hook: a shank and a barbed bend
    shape("hook", outline(art([
        "................", "........ll......", ".........l......", ".........m......",
        ".........m......", ".........m......", ".........m......", ".........m......",
        "...l.....m......", "...ml....m......", "....m....m......", "....md..dm......",
        ".....dddd.......", "................", "................", "................"])))
    # a harpoon head: a point with barbs down one side
    shape("harpoon_head", body(art([
        "..............#.", ".............##.", "............###.", "...........##...",
        "..........###...", ".........##.#...", "........###.....", ".......##.#.....",
        "......###.......", ".....##.#.......", "....###.........", "...##...........",
        "..##............", "................", "................", "................"])))
    # an antler: a beam with tines
    shape("antler", body(art([
        "................", "..#.......#.....", "..#.......#.....", "..##.....##.....",
        "...#....##......", "...##..##....#..", "....####....##..", ".....###...##...",
        "......###.##....", ".......####.....", "........##......", ".........##.....",
        "..........##....", "...........##...", "............#...", "................"])))
    # clothing
    shape("hood", body(art([
        "................", ".....######.....", "....########....", "...##########...",
        "..############..", "..####....####..", "..###......###..", "..###......###..",
        "..###......###..", "..###......###..", ".####......####.", ".####......####.",
        "#####......#####", "#####......#####", "................", "................"])))
    shape("cloak", body(art([
        "................", ".....######.....", "....##....##....", "....#......#....",
        "...###....###...", "...##########...", "..############..", "..############..",
        "..############..", ".##############.", ".##############.", ".##############.",
        "################", "################", "................", "................"])))
    shape("boots", body(art([
        "................", "................", "...####...####..", "...####...####..",
        "...####...####..", "...####...####..", "...####...####..", "...####...####..",
        "...####...####..", "...#####..#####.", "..######.######.", "..######.######.",
        "..######.######.", "................", "................", "................"])))
    shape("tunic", body(art([
        "................", "....##....##....", "..######..######", ".################",
        "################", "###.##########.#", "....##########..", "....##########..",
        "....##########..", "....##########..", "....##########..", "....##########..",
        "....##########..", "................", "................", "................"])))
    # a spear-thrower: a stick with a hook at the far end and a grip
    shape("atlatl", outline(compose(line(new(), 2, 13, 12, 3, "m", 2), art([
        "............ll..", "............ll..", "................", "................",
        "................", "................", "................", "................",
        "................", "................", "...dd...........", "..dd............",
        "................", "................", "................", "................"]))))
    # a fat lamp: a hollowed stone, the fat, a flame
    shape("lamp", outline(art([
        "................", "................", "................", ".......f........",
        "......fyf.......", "......fyf.......", ".......w........", "...hhhhwhhhhh...",
        "..lllcccccclll..", "..lmmmmmmmmmmd..", "...mmmmmmmmmd...", "....dddddddd....",
        "................", "................", "................", "................"])),
        {"f": "#f08020", "y": "#ffe070", "w": "#3a2a1a", "c": "#e8dcae"})
    # a pigment bowl (paint): a stone dish with colour in it
    shape("paint_bowl", outline(art([
        "................", "................", "................", "................",
        "................", "...hhhhhhhhhh...", "..lppppppppppd..", "..lpppqqpppppd..",
        "..lmpppppppppd..", "...mmmmmmmmmd...", "....dddddddd....", "................",
        "................", "................", "................", "................"])))
    # microlith: a tiny bladelet
    shape("microlith", body(poly(new(), [(6, 11), (9, 4), (11, 5), (8, 12)])))
    # an arrow: shaft, flint tip, feathers
    shape("arrow", outline(compose(line(new(), 3, 12, 11, 4, "s"), art([
        ".............t..", "............ttt.", "...........tt...", "................",
        "................", "................", "................", "................",
        "................", "...f............", "..ff............", ".fff.f..........",
        "..fff...........", "...f............", "................", "................"]))),
        {"s": "#9a6a34", "t": "#474b57", "f": "#ecece4"})
    shape("stave", body(line(new(), 1, 14, 14, 1, "#", 2)))
    shape("bow", outline(art([
        "..........mmd...", ".........m...s..", "........m.....s.", ".......m......s.",
        "......m.......s.", ".....m........s.", "....m.........s.", "...m..........s.",
        "..m...........s.", "..m..........s..", "..m.........s...", "...m......ss....",
        "....mm..ss......", "......ss........", "................", "................"])),
        {"s": "#d8ccaa"})
    # adze head: a blade across the handle, edge downward
    shape("adze_head", body(tool([(-1.6, -1.2), (-1.6, 1.4), (3.2, 2.2), (6.2, 1.6), (6.2, -0.6), (3.2, -1.8)])))
    # a dugout canoe from the side
    shape("canoe", body(art([
        "................", "................", "................", "................",
        "................", "#..............#", "##............##", ".##############.",
        ".##############.", "..############..", "...##########...", "................",
        "................", "................", "................", "................"])))
    shape("rack", outline(art([
        "................", ".dddddddddddddd.", ".m.s..s..s..s.m.", ".m.s..s..s..s.m.",
        ".m.r..r..r..r.m.", ".m.r..r..r..r.m.", ".m.r..r..r..r.m.", ".m............m.",
        ".m............m.", ".m............m.", ".m............m.", ".m............m.",
        ".m............m.", ".d............d.", "................", "................"])),
        {"s": "#c8aa70", "r": "#8a2a24"})
    shape("fish", body(art([
        "................", "................", "................", "................",
        "................", "....######...##.", "..##########.##.", ".############...",
        ".############...", "..##########.##.", "....######...##.", "................",
        "................", "................", "................", "................"])))
    shape("basket", outline(art([
        "................", "................", "....mlmlmlml....", "...lmlmlmlmlm...",
        "..mlmlmlmlmlmlm.", "..lmlmlmlmlmlml.", "..mlmlmlmlmlmlm.", "..lmlmlmlmlmlml.",
        "..mlmlmlmlmlmlm.", "...lmlmlmlmlml..", "....dddddddddd..", "................",
        "................", "................", "................", "................"])))
    shape("weir", outline(art([
        "................", "..m...m...m...m.", "..m...m...m...m.", ".lmllllmllllmllm",
        "..m...m...m...m.", ".llmllllmllllmll", "..m...m...m...m.", ".lmllllmllllmllm",
        "..m...m...m...m.", ".llmllllmllllmll", "..m...m...m...m.", "..m...m...m...m.",
        "..d...d...d...d.", "................", "................", "................"])))
    shape("net", outline(art([
        "................", ".m.m.m.m.m.m.m..", "m.m.m.m.m.m.m.m.", ".m.m.m.m.m.m.m..",
        "m.m.m.m.m.m.m.m.", ".m.m.m.m.m.m.m..", "m.m.m.m.m.m.m.m.", ".m.m.m.m.m.m.m..",
        "m.m.m.m.m.m.m.m.", ".m.m.m.m.m.m.m..", "m.m.m.m.m.m.m.m.", ".d.d.d.d.d.d.d..",
        "................", "................", "................", "................"])))
    shape("fishing_line", outline(compose(line(new(), 2, 14, 13, 2, "m", 1), art([
        "................", "..............s.", "..............s.", "..............s.",
        "..............s.", "..............s.", "..............s.", "..............s.",
        ".............hs.", "..............h.", "................", "................",
        "................", "................", "................", "................"]))),
        {"s": "#e0d8c0", "h": "#d8ceb0"})
    shape("polished_head", body(tool([(-1.8, -1.6), (-1.8, 1.6), (3.0, 2.2), (6.8, 1.4), (7.4, 0.0), (6.8, -1.4), (3.0, -2.2)])))
    shape("flax", outline(art([
        "...b.....b......", "..bbb...bbb.....", "...b..b..b......", "...g.bbb.g...b..",
        "...g..b..g..bbb.", "...g..g..g...b..", "...g..g..g...g..", "...g..g..g...g..",
        "....g.g.g...g...", "....g.g.g..g....", ".....ggg.gg.....", "......ggg.......",
        "......ggg.......", "......ggg.......", "................", "................"])),
        {"b": "#4a78d0", "g": "#5a8834"})
    shape("flatbread", body(ellipse(new(), 8, 9, 6.5, 4.0)))
    shape("whorl", body(compose(ellipse(new(), 8, 9, 5, 3.5))))
    shape("spindle", outline(compose(line(new(), 8, 1, 8, 14, "s"), art([
        "................", "................", "................", "................",
        "................", "................", "................", "................",
        "....wwwwwwww....", "...wwwwwwwwww...", "....wwwwwwww....", "................",
        "................", "................", "................", "................"]))),
        {"s": "#9a6a34", "w": "#b05a34"})
    shape("loom", outline(art([
        "................", ".dddddddddddddd.", ".m.l.l.l.l.l.lm.", ".m.l.l.l.l.l.lm.",
        ".mllllllllllllm.", ".mllllllllllllm.", ".m.l.l.l.l.l.lm.", ".m.l.l.l.l.l.lm.",
        ".m.l.l.l.l.l.lm.", ".m.l.l.l.l.l.lm.", ".m.s.s.s.s.s.sm.", ".m............m.",
        ".m............m.", ".d............d.", "................", "................"])),
        {"l": "#ded8c2", "s": "#808080"})
    shape("cloth", body(art([
        "................", "................", "...##########...", "..############..",
        "..############..", "..############..", "..############..", "..############..",
        "..############..", "..############..", "..###########...", "...#########....",
        "................", "................", "................", "................"])))
    shape("wedge", body(poly(new(), [(3, 12), (12, 3), (13, 5), (5, 13)])))
    shape("beads", outline(art([
        "................", "................", "....ll....ll....", "...lmmd..lmmd...",
        "...mmmd..mmmd...", "....dd....dd....", "................", "................",
        "....ll....ll....", "...lmmd..lmmd...", "...mmmd..mmmd...", "....dd....dd....",
        "................", "................", "................", "................"])))
    shape("box", body(art([
        "................", "................", "................", "...##########...",
        "..############..", "..############..", "..############..", "..############..",
        "..############..", "..############..", "..############..", "..############..",
        "...##########...", "................", "................", "................"])))
    shape("menhir", body(art([
        "......####......", ".....######.....", ".....#######....", "....########....",
        "....########....", "....#########...", "....#########...", "...##########...",
        "...##########...", "...##########...", "...###########..", "..############..",
        "..############..", ".##############.", "################", "................"])))

    # ---------- sprites ----------
    sp("birch_bark", ("bark", "birch"))
    sp("birch_tar", ("lump", "tar"))
    sp("tempered_flint", ("flint_shard", "tempered"))
    sp("tinder_fungus", ("bracket", "fungus"))
    sp("fire_ember", ("bracket", "fungus"), ("sparkle", "fire"))
    sp("pyrite_striker", ("rock", "pyrite"), ("flint_shard", "flint"))
    sp("bone_needle", ("needle", "bone"))
    sp("bone_fish_hook", ("hook", "bone"))
    sp("bone_harpoon_head", ("harpoon_head", "bone"))
    sp("bone_harpoon", H, ("harpoon_head", "bone"))
    sp("antler_pick", ("antler", "antler"))
    sp("fur_pelt", ("hide", "fur"))
    sp("fur_hood", ("hood", "fur"))
    sp("fur_cloak", ("cloak", "fur"))
    sp("fur_boots", ("boots", "fur"))
    sp("leather_cap", ("hood", "leather"))
    sp("leather_tunic", ("cloak", "leather"))
    sp("leather_boots", ("boots", "leather"))
    sp("copper_helmet", ("hood", "copper"))
    sp("copper_chestplate", ("cloak", "copper"))
    sp("copper_boots", ("boots", "copper"))
    sp("atlatl", ("atlatl", "wood"))
    sp("animal_fat", ("lump", "fat"))
    sp("fat_lamp", ("lamp", "stone"))
    for name, pal in (("red", "ochre_red"), ("yellow", "ochre_yellow"), ("black", "coal"), ("green", "malachite")):
        sp(name + "_pigment", ("dust_pile", pal))
        c = PALETTES[pal]
        sp(name + "_paint", ("paint_bowl", "stone", {"p": c["m"], "q": c["h"]}))
    sp("microlith", ("microlith", "flint"))
    sp("arrow", ("arrow", "handle"))
    sp("bow_stave", ("stave", "wood"))
    sp("bow", ("bow", "wood"))
    sp("flint_adze_head", ("adze_head", "flint"))
    sp("flint_adze", H, ("adze_head", "flint"))
    sp("dugout_canoe", ("canoe", "wood"))
    sp("drying_rack", ("rack", "wood"))
    sp("dried_meat", ("steak", "dried"))
    sp("smoked_meat", ("steak", "smoked"))
    sp("salted_meat", ("steak", "salted"))
    sp("dried_fish", ("fish", "dried"))
    sp("smoked_fish", ("fish", "smoked"))
    sp("basket_trap", ("basket", "straw"))
    sp("fish_weir", ("weir", "wood"))
    sp("fishing_rod", ("fishing_line", "handle"))
    sp("fishing_net", ("net", "twine"))
    sp("axe_roughout", ("flint_shard", "flint"))
    sp("polished_axe_head", ("polished_head", "greenstone"))
    sp("polished_stone_axe", H, ("polished_head", "greenstone"))
    sp("einkorn_seeds", ("seeds", "grain"))
    sp("emmer_seeds", ("seeds", "grain_dark"))
    sp("flax_seeds", ("seeds", "flaxseed"))
    sp("einkorn", ("sheaf", "grain"))
    sp("emmer", ("sheaf", "grain_dark"))
    sp("flax", ("flax", "leaf"))
    sp("flour", ("dust_pile", "flour"))
    sp("flatbread_dough", ("ball", "dough"))
    sp("flatbread", ("flatbread", "bread"))
    sp("retted_flax", ("sheaf", "rot"))
    sp("flax_fiber", ("fiber", "linen"))
    sp("raw_spindle_whorl", ("whorl", "clay"))
    sp("spindle_whorl", ("whorl", "terracotta"))
    sp("drop_spindle", ("spindle", "handle"))
    sp("linen_thread", ("twine", "linen"))
    sp("loom", ("loom", "wood"))
    sp("linen_cloth", ("cloth", "linen"))
    sp("linen_tunic", ("tunic", "linen"))
    sp("linen_bag", ("bag", "linen"))
    sp("brine_pot", ("cooking_pot", "terracotta"), ("pot_fill", None, {"q": "#3a78d8", "k": "#c0e0ff"}))
    sp("salt_pot", ("cooking_pot", "terracotta"), ("pot_fill", None, {"q": "#c4c4cc", "k": "#ffffff"}))
    sp("salt", ("dust_pile", "salt"))
    sp("lever", ("stave", "handle"))
    sp("wooden_wedge", ("wedge", "wood"))
    sp("copper_awl", ("needle", "copper"))
    sp("copper_beads", ("beads", "copper"))
    sp("flint_dagger_blade", ("knife_blade", "flint"))
    sp("flint_dagger", SH, ("knife_blade", "flint"))
    sp("bark_box", ("box", "birch"))
    sp("grass_cape", ("cloak", "straw"))
    sp("bearskin_cap", ("hood", "bear"))
    sp("standing_stone", ("menhir", "stone"))
    # stubs: part E's items, unless part E drew them
    for item, layers in (("pyrite", [("ore_rock", "stone", {"s": "#b49a34", "S": "#fff09a"})]), ("red_ochre", [("lump", "ochre_red")]),
                         ("yellow_ochre", [("lump", "ochre_yellow")]), ("antler", [("antler", "antler")]),
                         ("raw_fish", [("fish", "fish")]), ("chalk_stone", [("rock", "chalk")]),
                         ("floorstone_flint", [("flint_shard", "floorstone")])):
        if item not in SP:
            sp(item, *layers)
